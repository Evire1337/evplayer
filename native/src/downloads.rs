//! Songs saved for offline listening (src/offline): a queue worked two at a time, each song's
//! sound taken from the same sources the player would use (so a SoundCloud song with no stream
//! is saved from its YouTube Music copy), written by the shared offline.rs into the folder the
//! Tauri build uses. The player plays a saved copy first, without the network.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crate::audio::source::Spec;
use crate::model::Track;
use crate::player::resolve::Resolve;

/// songs fetched at the same time
const AT_ONCE: usize = 2;

#[derive(Clone, Default)]
pub struct Status {
    /// waiting or being fetched: id → (bytes done, bytes total)
    pub running: HashMap<String, (u64, u64)>,
    pub waiting: usize,
    pub failed: Vec<(String, String)>,
    pub done_count: usize,
}

struct Inner {
    queue: VecDeque<Track>,
    status: Status,
    /// songs queued by a followed list topping itself up: a failure is remembered for them
    auto: std::collections::HashSet<String>,
}

pub struct Downloads {
    inner: Mutex<Inner>,
    wake: Condvar,
    offline: Arc<crate::offline::Offline>,
    resolver: Arc<crate::player::resolve::Resolver>,
}

impl Downloads {
    pub fn new(offline: Arc<crate::offline::Offline>, resolver: Arc<crate::player::resolve::Resolver>) -> Arc<Self> {
        let me = Arc::new(Self { inner: Mutex::new(Inner { queue: VecDeque::new(), status: Status::default(), auto: Default::default() }), wake: Condvar::new(), offline, resolver });
        for _ in 0..AT_ONCE {
            let w = me.clone();
            let _ = std::thread::Builder::new().name("download".into()).spawn(move || w.worker());
        }
        me
    }

    pub fn is_saved(&self, id: &str) -> bool {
        self.offline.path_of(id).is_some()
    }

    /// add songs to the queue (those saved or queued already are skipped); how many were added
    pub fn add(&self, tracks: &[Track]) -> usize {
        let mut g = self.inner.lock().unwrap();
        let mut n = 0;
        for t in tracks {
            let queued = g.queue.iter().any(|q| q.id == t.id) || g.status.running.contains_key(&t.id);
            if queued || self.offline.path_of(&t.id).is_some() || t.source == crate::model::Source::Local {
                continue;
            }
            g.queue.push_back(t.clone());
            n += 1;
        }
        g.status.waiting = g.queue.len();
        g.status.failed.retain(|(id, _)| !tracks.iter().any(|t| &t.id == id));
        drop(g);
        self.wake.notify_all();
        crate::events::repaint();
        n
    }

    /// songs from followed lists topping themselves up (failures are not tried again)
    pub fn add_auto(&self, tracks: &[Track]) -> usize {
        let mut g = self.inner.lock().unwrap();
        let mut n = 0;
        for t in tracks {
            let queued = g.queue.iter().any(|q| q.id == t.id) || g.status.running.contains_key(&t.id);
            if queued || self.offline.path_of(&t.id).is_some() || t.source == crate::model::Source::Local {
                continue;
            }
            g.auto.insert(t.id.clone());
            g.queue.push_back(t.clone());
            n += 1;
        }
        g.status.waiting = g.queue.len();
        drop(g);
        self.wake.notify_all();
        crate::events::repaint();
        n
    }

    /// the folder the saved songs are in
    pub fn folder(&self) -> std::path::PathBuf {
        self.offline.folder().to_path_buf()
    }

    /// every saved song deleted (they stay in the lists and play from the network)
    pub fn clear(&self) {
        self.cancel_all();
        self.offline.clear();
        crate::events::repaint();
    }

    /// copies of saved songs in `folder` as "Artist - Title" with tags and cover
    pub fn export(&self, tracks: &[Track], folder: &std::path::Path) -> Result<crate::offline::ExportResult, String> {
        let items: Vec<crate::offline::ExportItem> = tracks.iter().map(|t| crate::offline::ExportItem { id: t.id.clone(), title: t.title.clone(), artist: t.artist_line(), artwork: t.artwork.clone() }).collect();
        self.offline.export(&items, folder)
    }

    pub fn remove(&self, ids: &[String]) {
        self.offline.remove(ids);
        crate::events::repaint();
    }

    pub fn cancel_all(&self) {
        let mut g = self.inner.lock().unwrap();
        g.queue.clear();
        g.status.waiting = 0;
        let running: Vec<String> = g.status.running.keys().cloned().collect();
        drop(g);
        for id in running {
            self.offline.cancel(&id);
        }
        crate::events::repaint();
    }

    pub fn status(&self) -> Status {
        self.inner.lock().unwrap().status.clone()
    }

    pub fn saved(&self) -> Vec<crate::offline::OfflineEntry> {
        self.offline.list()
    }

    fn worker(self: Arc<Self>) {
        loop {
            let track = {
                let mut g = self.inner.lock().unwrap();
                loop {
                    if let Some(t) = g.queue.pop_front() {
                        g.status.waiting = g.queue.len();
                        g.status.running.insert(t.id.clone(), (0, 0));
                        break t;
                    }
                    g = self.wake.wait(g).unwrap();
                }
            };
            let result = self.fetch(&track);
            let mut g = self.inner.lock().unwrap();
            g.status.running.remove(&track.id);
            let auto = g.auto.remove(&track.id);
            match result {
                Ok(()) => g.status.done_count += 1,
                Err(e) => g.status.failed.push((track.id.clone(), format!("{} — {e}", track.title))),
            }
            let failed = auto && !self.is_saved(&track.id);
            drop(g);
            if failed {
                crate::keep::mark_failed(std::slice::from_ref(&track.id));
            }
            crate::events::repaint();
        }
    }

    fn fetch(&self, track: &Track) -> Result<(), String> {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut last = "Не нашлось, откуда скачать".to_string();
        for c in self.resolver.clone().candidates(track, cancel.clone()) {
            let c = match c {
                Ok(c) => c,
                Err(e) => {
                    last = e;
                    continue;
                }
            };
            let source = match c.spec {
                Spec::Http { url, .. } => crate::offline::Source::File { url },
                Spec::Hls { url } => crate::offline::Source::Hls { url },
                Spec::File(_) => return Ok(()),
            };
            let id = track.id.clone();
            let progress = |done: u64, total: u64| {
                if let Ok(mut g) = self.inner.try_lock() {
                    g.status.running.insert(id.clone(), (done, total));
                }
            };
            match self.offline.fetch(&track.id, &source, crate::proxy::is_open_host, &progress) {
                Ok(_) => return Ok(()),
                Err(e) => last = e,
            }
            if cancel.load(Ordering::Relaxed) {
                break;
            }
        }
        Err(last)
    }
}
