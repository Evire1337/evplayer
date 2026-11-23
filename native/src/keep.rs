//! Lists kept offline as a whole (src/offline/scopes.ts, store.ts and sync.ts). A "scope" is
//! the likes ("likes") or one of your playlists (its id). Lists marked to follow stay complete:
//! when songs are added to them, or the app starts, the missing ones are fetched, but not the
//! ones that could not be kept before (asking for such a song by hand tries again). Playlists
//! that were deleted drop out. Kept in evplayer.offline.v1 in the page's shape.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::model::{Source, Track};
use crate::store::kv;

const KEY: &str = "evplayer.offline.v1";
pub const LIKES: &str = "likes";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct KeepState {
    pub lists: Vec<String>,
    /// songs that could not be kept (no format that plays, gone from the service)
    pub failed: Vec<String>,
}

pub fn get() -> KeepState {
    kv::read_state(&crate::state::get().kv, KEY).unwrap_or_default()
}

fn put(s: &KeepState) {
    kv::write_state(&crate::state::get().kv, KEY, s, 0);
    crate::events::repaint();
}

fn put_and_follow(s: &KeepState) {
    put(s);
    changed();
}

pub fn add(scopes: &[String]) {
    let mut s = get();
    for sc in scopes {
        if !s.lists.contains(sc) {
            s.lists.push(sc.clone());
        }
    }
    put_and_follow(&s);
}

pub fn remove(scopes: &[String]) {
    let mut s = get();
    s.lists.retain(|l| !scopes.contains(l));
    put(&s);
}

/// songs that failed while a list was being topped up: not tried again on their own
pub fn mark_failed(ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let mut s = get();
    for id in ids {
        if !s.failed.contains(id) {
            s.failed.push(id.clone());
        }
    }
    put(&s);
}

/// services' songs only: files from this computer need no copy
pub fn keepable(t: &Track) -> bool {
    matches!(t.source, Source::Soundcloud | Source::Youtube)
}

pub fn title_of(scope: &str) -> String {
    if scope == LIKES {
        return crate::i18n::t("Понравившиеся");
    }
    crate::state::get().library.read().unwrap().playlists.iter().find(|p| p.id == scope).map(|p| p.title.clone()).unwrap_or_else(|| crate::i18n::t("Плейлист"))
}

/// the songs of a scope that can be kept, without repeats
pub fn scope_tracks(scope: &str) -> Vec<Track> {
    let lib = crate::state::get().library.read().unwrap();
    let all: Vec<&Track> = if scope == LIKES { lib.likes.iter().map(|l| &l.track).collect() } else { lib.playlists.iter().find(|p| p.id == scope).map(|p| p.items.iter().map(|i| &i.track).collect()).unwrap_or_default() };
    let mut seen = HashSet::new();
    all.into_iter().filter(|t| keepable(t) && seen.insert(t.id.clone())).cloned().collect()
}

/// every song of several scopes, once
pub fn tracks_of(scopes: &[String]) -> Vec<Track> {
    let mut seen = HashSet::new();
    scopes.iter().flat_map(|s| scope_tracks(s)).filter(|t| seen.insert(t.id.clone())).collect()
}

/// (songs, of them saved)
pub fn status(scope: &str, saved: &HashSet<String>) -> (usize, usize) {
    let t = scope_tracks(scope);
    (t.len(), t.iter().filter(|t| saved.contains(&t.id)).count())
}

/// the likes and every playlist
pub fn all_scopes() -> Vec<String> {
    let lib = crate::state::get().library.read().unwrap();
    std::iter::once(LIKES.to_string()).chain(lib.playlists.iter().map(|p| p.id.clone())).collect()
}

/// the missing songs of the followed lists, for the downloads queue (and lists of deleted
/// playlists forgotten)
pub fn top_up_tracks() -> Vec<Track> {
    let s = get();
    if s.lists.is_empty() {
        return vec![];
    }
    let known: HashSet<String> = all_scopes().into_iter().collect();
    let gone: Vec<String> = s.lists.iter().filter(|l| !known.contains(*l)).cloned().collect();
    if !gone.is_empty() {
        remove(&gone);
    }
    let wanted: Vec<String> = s.lists.iter().filter(|l| known.contains(*l)).cloned().collect();
    let failed: HashSet<&String> = s.failed.iter().collect();
    tracks_of(&wanted).into_iter().filter(|t| !failed.contains(&t.id)).collect()
}

/// "3 трека"
pub fn songs(n: usize) -> String {
    crate::i18n::tracks(n)
}

/// when the followed lists are looked at next (4 s after the last change, as the page did)
static DUE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
static WAKE: std::sync::Condvar = std::sync::Condvar::new();
const SETTLE: std::time::Duration = std::time::Duration::from_secs(4);

/// the library or the followed lists changed: top up a moment later
pub fn changed() {
    *DUE.lock().unwrap() = Some(std::time::Instant::now() + SETTLE);
    WAKE.notify_all();
}

/// A thread that keeps the followed lists complete, also while the window is hidden. It sleeps
/// until something changes; at the start it looks once.
pub fn start(downloads: std::sync::Arc<crate::downloads::Downloads>) {
    changed();
    let _ = std::thread::Builder::new().name("keep".into()).spawn(move || loop {
        let mut due = DUE.lock().unwrap();
        loop {
            match *due {
                None => due = WAKE.wait(due).unwrap(),
                Some(t) => {
                    let now = std::time::Instant::now();
                    if now >= t {
                        break;
                    }
                    due = WAKE.wait_timeout(due, t - now).unwrap().0;
                }
            }
        }
        *due = None;
        drop(due);
        let missing = top_up_tracks();
        if !missing.is_empty() {
            downloads.add_auto(&missing);
        }
    });
}
