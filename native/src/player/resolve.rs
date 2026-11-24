//! Where a song's sound comes from, best first (src/player/streams.ts): a copy saved for
//! offline, your own file, the service the song is from, and when that will not play, the same
//! recording on the other service. The player tries the next way when one does not play.
//! Also the similar songs for autoplay and radio (src/player/radio.ts).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::matching::{copy_order, is_original, is_same_recording, match_score, search_query_for, simplify, split_title};
use crate::audio::source::Spec;
use crate::model::{Source, Track};
use crate::services::{soundcloud, youtube};

pub use super::matching::song_key;

/// one way to get a song's sound
pub struct Candidate {
    pub spec: Spec,
    /// the same song on the other service, playing because the original did not
    pub via: Option<Track>,
    /// "opus 160 кбит/с"
    pub quality: Option<String>,
}

pub type Candidates = Box<dyn Iterator<Item = Result<Candidate, String>> + Send>;

pub trait Resolve: Send + Sync {
    fn candidates(self: Arc<Self>, track: &Track, cancel: Arc<AtomicBool>) -> Candidates;
    fn similar(&self, seed: &Track, exclude: &HashSet<String>) -> Vec<Track>;
}

/// how many uploads of the same song are tried (one can be protected, another not)
const COPIES_TRIED: usize = 3;
/// how long the look for an uncensored twin may delay the start of a song
const TWIN_WAIT: Duration = Duration::from_millis(2500);
/// remembered similar songs are good this long
const SIMILAR_TTL: Duration = Duration::from_secs(30 * 60);

pub struct Resolver {
    pub library: Arc<crate::library::Library>,
    pub offline: Arc<crate::offline::Offline>,
    yt_matches: Mutex<HashMap<String, Option<Track>>>,
    sc_matches: Mutex<HashMap<String, Vec<Track>>>,
    twins: Mutex<HashMap<String, Option<Track>>>,
    similar_cache: Mutex<HashMap<String, (Instant, Vec<Track>)>>,
}

impl Resolver {
    pub fn new(library: Arc<crate::library::Library>, offline: Arc<crate::offline::Offline>) -> Arc<Self> {
        Arc::new(Self { library, offline, yt_matches: Default::default(), sc_matches: Default::default(), twins: Default::default(), similar_cache: Default::default() })
    }

    /// the song's recording on YouTube Music
    fn youtube_match(&self, track: &Track) -> Option<Track> {
        if let Some(m) = self.yt_matches.lock().unwrap().get(&track.id) {
            return m.clone();
        }
        // with the artist first; then the title alone (an uploader's name is often not the artist's)
        let mut queries = vec![search_query_for(track)];
        let alone = simplify(&split_title(track).0);
        if !queries.contains(&alone) {
            queries.push(alone);
        }
        let mut found = None;
        for q in queries.into_iter().filter(|q| !q.is_empty()) {
            let Ok(list) = youtube::search(&q) else { continue };
            let mut ranked: Vec<Track> = list.into_iter().filter(|t| is_same_recording(track, t, true)).collect();
            ranked.sort_by(|a, b| copy_order(track, a, b));
            if let Some(t) = ranked.into_iter().next() {
                found = Some(t);
                break;
            }
        }
        self.yt_matches.lock().unwrap().insert(track.id.clone(), found.clone());
        found
    }

    /// uploads of the same song on SoundCloud, most alike first
    fn soundcloud_matches(&self, track: &Track) -> Vec<Track> {
        if let Some(m) = self.sc_matches.lock().unwrap().get(&track.id) {
            return m.clone();
        }
        let found = soundcloud::search(&search_query_for(track)).map(|r| r.0).unwrap_or_default();
        let mut scored: Vec<(f64, Track)> = found.into_iter().filter(|t| !t.preview && is_same_recording(track, t, false)).map(|t| (match_score(track, &t), t)).collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let out: Vec<Track> = scored.into_iter().take(COPIES_TRIED).map(|x| x.1).collect();
        self.sc_matches.lock().unwrap().insert(track.id.clone(), out.clone());
        out
    }

    /// The uncensored version of a YouTube Music song that is not marked as one. Songs saved by
    /// older versions carry no mark, and the censored version is often listed first.
    pub(crate) fn uncensored_twin(&self, track: &Track) -> Option<Track> {
        if track.explicit || track.source != Source::Youtube {
            return None;
        }
        if let Some(t) = self.twins.lock().unwrap().get(&track.id) {
            return t.clone();
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let t = track.clone();
        std::thread::spawn(move || {
            let found = youtube::search(&search_query_for(&t)).ok().map(|list| {
                // a length of 0 (songs from artist pages and the queue come without one) is not compared
                let close = |x: &Track| t.duration_ms <= 0.0 || x.duration_ms <= 0.0 || (x.duration_ms - t.duration_ms).abs() <= 4000.0;
                let mut c: Vec<Track> = list.into_iter().filter(|x| x.explicit && x.id != t.id && is_original(&t, x) && close(x)).collect();
                c.sort_by(|a, b| match_score(&t, b).partial_cmp(&match_score(&t, a)).unwrap_or(std::cmp::Ordering::Equal));
                c.into_iter().next()
            });
            let _ = tx.send(found);
        });
        match rx.recv_timeout(TWIN_WAIT) {
            // an answer that came is remembered (also "none"); one that did not is asked again next time
            Ok(Some(found)) => {
                self.twins.lock().unwrap().insert(track.id.clone(), found.clone());
                found
            }
            _ => None,
        }
    }

    fn youtube_candidate(&self, video_id: &str, via: Option<Track>) -> Result<Candidate, String> {
        let d = youtube::resolve_audio(video_id)?;
        Ok(Candidate { quality: Some(d.quality()), spec: Spec::Http { url: d.url, user_agent: Some(d.user_agent.to_string()) }, via })
    }

    fn soundcloud_candidates(&self, id: &str, via: Option<Track>, out: &mut Vec<Result<Candidate, String>>, cancel: &AtomicBool) -> bool {
        match soundcloud::streams(id) {
            Ok(list) => {
                let mut any = false;
                for t in list {
                    if cancel.load(Ordering::Relaxed) {
                        return any;
                    }
                    match soundcloud::stream_url(&t) {
                        Ok(s) => {
                            let spec = if s.hls { Spec::Hls { url: s.url } } else { Spec::Http { url: s.url, user_agent: None } };
                            out.push(Ok(Candidate { spec, via: via.clone(), quality: Some(s.quality) }));
                            any = true;
                        }
                        Err(e) => out.push(Err(e)),
                    }
                }
                any
            }
            Err(e) => {
                out.push(Err(e));
                false
            }
        }
    }
}

/// A list made a step at a time: each step runs only when the player asks for the next way
/// (the next step is often a search on another service, worth doing only when needed).
struct Steps {
    ready: std::collections::VecDeque<Result<Candidate, String>>,
    steps: std::collections::VecDeque<Box<dyn FnOnce(&mut Vec<Result<Candidate, String>>) + Send>>,
    cancel: Arc<AtomicBool>,
}

impl Iterator for Steps {
    type Item = Result<Candidate, String>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(c) = self.ready.pop_front() {
                return Some(c);
            }
            let step = self.steps.pop_front()?;
            let mut out = Vec::new();
            step(&mut out);
            self.ready.extend(out);
        }
    }
}

impl Resolve for Resolver {
    fn candidates(self: Arc<Self>, track: &Track, cancel: Arc<AtomicBool>) -> Candidates {
        let mut ready = std::collections::VecDeque::new();
        // a copy saved for offline comes first: no waiting for a service, no connection needed
        if let Some(path) = self.offline.path_of(&track.id) {
            ready.push_back(Ok(Candidate { spec: Spec::File(path), via: None, quality: None }));
        }
        let mut steps: std::collections::VecDeque<Box<dyn FnOnce(&mut Vec<Result<Candidate, String>>) + Send>> = Default::default();
        let t = track.clone();
        match track.source {
            Source::Local => {
                match self.library.get(&track.id) {
                    Some(r) => ready.push_back(Ok(Candidate { spec: Spec::File(r.path.into()), via: None, quality: None })),
                    None => ready.push_back(Err("Файл перемещён или удалён".into())),
                }
            }
            Source::Soundcloud => {
                let c1 = cancel.clone();
                let t1 = t.clone();
                let me = self.clone();
                steps.push_back(Box::new(move |out| {
                    me.soundcloud_candidates(&t1.id, None, out, &c1);
                }));
                let me = self.clone();
                steps.push_back(Box::new(move |out| {
                    // the full recording from YouTube Music, with the SoundCloud cover and names
                    match me.youtube_match(&t) {
                        // the copy itself may be the censored version: its uncensored twin plays instead
                        Some(copy) => match youtube::video_id_of(&me.uncensored_twin(&copy).map(|x| x.id).unwrap_or_else(|| copy.id.clone())).map(str::to_string) {
                            Some(vid) => out.push(me.youtube_candidate(&vid, Some(copy)).map_err(|e| format!("копия с YouTube Music не включилась: {e}"))),
                            None => out.push(Err("на YouTube Music эта песня не нашлась".into())),
                        },
                        None if t.preview => out.push(Err("SoundCloud даёт только 30-секундный отрывок, а на YouTube Music эта песня не нашлась".into())),
                        None => out.push(Err("на YouTube Music эта песня не нашлась".into())),
                    }
                }));
            }
            Source::Youtube => {
                let t1 = t.clone();
                let me = self.clone();
                steps.push_back(Box::new(move |out| {
                    // a song saved before the marks existed may be the censored version
                    let twin = me.uncensored_twin(&t1);
                    let id = twin.as_ref().map(|x| x.id.clone()).unwrap_or_else(|| t1.id.clone());
                    if let Some(vid) = youtube::video_id_of(&id) {
                        out.push(me.youtube_candidate(vid, None));
                    }
                }));
                let c1 = cancel.clone();
                let me = self.clone();
                steps.push_back(Box::new(move |out| {
                    let copies = me.soundcloud_matches(&t);
                    if copies.is_empty() {
                        out.push(Err("Не получилось включить ни YouTube, ни копии с SoundCloud".into()));
                    }
                    for copy in copies {
                        if c1.load(Ordering::Relaxed) {
                            return;
                        }
                        let id = copy.id.clone();
                        me.soundcloud_candidates(&id, Some(copy), out, &c1);
                    }
                }));
            }
        }
        Box::new(Steps { ready, steps, cancel })
    }

    fn similar(&self, seed: &Track, exclude: &HashSet<String>) -> Vec<Track> {
        let cached = self.similar_cache.lock().unwrap().get(&seed.id).filter(|(at, _)| at.elapsed() < SIMILAR_TTL).map(|x| x.1.clone());
        let all = match cached {
            Some(a) => a,
            None => {
                let (yt, sc) = std::thread::scope(|s| {
                    let yt = s.spawn(|| {
                        let id = if seed.source == Source::Youtube { Some(seed.id.clone()) } else { youtube::search(&search_query_for(seed)).ok().and_then(|l| l.into_iter().next()).map(|t| t.id) };
                        id.and_then(|id| youtube::radio(&id).ok()).unwrap_or_default()
                    });
                    let sc = s.spawn(|| if seed.source == Source::Soundcloud { soundcloud::related(&seed.id).unwrap_or_default() } else { vec![] });
                    (yt.join().unwrap_or_default(), sc.join().unwrap_or_default())
                });
                let mut all = interleave(yt, sc);
                if all.is_empty() {
                    if let Some(a) = seed.artists.first() {
                        // last resort: more by the same artist
                        let name = simplify(&a.name);
                        let y = youtube::search(&name).unwrap_or_default();
                        let s = soundcloud::search(&name).map(|r| r.0).unwrap_or_default();
                        all = interleave(y, s);
                    }
                }
                let keep = !crate::state::get().settings.read().unwrap().no_service_cache;
                if !all.is_empty() && keep {
                    self.similar_cache.lock().unwrap().insert(seed.id.clone(), (Instant::now(), all.clone()));
                }
                all
            }
        };
        let mut skip = exclude.clone();
        skip.insert(seed.id.clone());
        skip.insert(song_key(seed));
        unique_songs(all, &skip).into_iter().take(25).collect()
    }
}

fn interleave(a: Vec<Track>, b: Vec<Track>) -> Vec<Track> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut a, mut b) = (a.into_iter(), b.into_iter());
    loop {
        match (a.next(), b.next()) {
            (None, None) => return out,
            (x, y) => {
                out.extend(x);
                out.extend(y);
            }
        }
    }
}

/// the same song uploaded twice counts once
pub fn unique_songs(list: Vec<Track>, exclude: &HashSet<String>) -> Vec<Track> {
    let mut seen = HashSet::new();
    list.into_iter()
        .filter(|t| {
            if exclude.contains(&t.id) {
                return false;
            }
            let key = song_key(t);
            !exclude.contains(&key) && seen.insert(key)
        })
        .collect()
}
