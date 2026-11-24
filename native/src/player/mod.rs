//! The player: the queue, which deck plays what, getting the next song ready, crossfades,
//! skipping a song that will not play. The same behaviour as src/player/engine.ts, with the
//! two <audio> elements replaced by the engine's two decks (audio/engine.rs).
//!
//! Everything the person does comes here as a method call (from the window, a global key, the
//! tray, the media keys). A small thread looks after the player while it plays: ten times a
//! second it notes the position, starts the next song's preparation near the end, notices the
//! end, the armed deck that took over, a source that failed. While nothing plays it sleeps.

pub mod matching;
pub mod queue;
pub mod resolve;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::audio::decode::Track as Deck;
use crate::audio::engine::Engine;
use crate::model::{Source, Track};
use crate::state;
use crate::store::state::{PlaySource, Session, HISTORY_AFTER_MS};
pub use queue::Repeat;
use resolve::{Candidate, Resolve};

/// a source that has not started playing by then counts as not working
const START_TIMEOUT: Duration = Duration::from_secs(20);
/// a song that cannot be played gives way to the next one after this long
const SKIP_BROKEN_AFTER: Duration = Duration::from_millis(500);
/// this many broken songs in a row stop the player (no internet, a service down)
const MAX_FAILED_IN_A_ROW: u32 = 5;
/// how long before the end the next song starts loading
const PREPARE_BEFORE_MS: f64 = 25_000.0;
/// the sound card is let go after the player has been paused this long
const SUSPEND_AFTER: Duration = Duration::from_secs(8);
/// "previous" a few seconds in restarts the song (like most players)
const RESTART_AFTER_MS: f64 = 4000.0;
const TICK: Duration = Duration::from_millis(100);

/// A song's length: the decoder's, unless it is far shorter than the one the service gave
/// (a stream's first piece read for the whole song). Taken as it was, the player faded into
/// the next song or moved on after a few seconds.
fn length_of(decoded: Option<f64>, known: f64) -> f64 {
    match decoded {
        Some(d) if known > 0.0 && d < known * 0.8 => known,
        Some(d) => d,
        None => known,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Error,
}

/// what the window shows of the player
#[derive(Clone, Debug, Default)]
pub struct View {
    pub queue: Vec<Track>,
    pub index: usize,
    pub status: Status,
    pub position_ms: f64,
    pub duration_ms: f64,
    pub volume: f32,
    pub muted: bool,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub error: Option<String>,
    /// the original did not play: a copy of the song from this service is playing
    pub route: Option<Source>,
    /// "opus 160 кбит/с"
    pub quality: Option<String>,
    /// part of the song fetched (0..1)
    pub buffered: f32,
    /// the network is slower than the song
    pub buffering: bool,
}

impl View {
    pub fn current(&self) -> Option<&Track> {
        self.queue.get(self.index)
    }
}

struct Prepared {
    index: usize,
    track: Track,
    deck: Arc<Deck>,
    route: Option<Source>,
    quality: Option<String>,
    armed: bool,
}

struct Core {
    view: View,
    engine: Engine,
    active: usize,
    /// the load in progress; a load whose token is not the current one gave way to another
    token: u64,
    cancel: Arc<AtomicBool>,
    deck: Option<Arc<Deck>>,
    prepared: Option<Prepared>,
    preparing: Option<(u64, usize, String)>,
    /// a song whose preparation failed: it starts the full way at its turn
    prepare_skip: Option<String>,
    /// a crossfade until then: the other deck is let go afterwards
    fading_until: Option<Instant>,
    switched_seen: u64,
    resume_at: f64,
    failed_in_a_row: u32,
    skip_at: Option<(Instant, u64)>,
    paused_since: Option<Instant>,
    // listening: how much of the current song was actually heard
    heard_ms: f64,
    last_pos: f64,
    counted: bool,
    source: Option<PlaySource>,
    source_ids: std::collections::HashSet<String>,
    radio_busy: bool,
    play_when_extended: bool,
    first_track: bool,
    last_saved: String,
    last_save: Instant,
    /// volume levelling of the song playing
    leveler: crate::audio::dsp::loudness::Leveler,
    leveler_track: Option<String>,
    leveler_at: Instant,
    /// the last look at the output device
    device_check: Instant,
    /// the listening session of the song playing (statistics count a session once)
    session: String,
    session_track: Option<Track>,
}

/// what a person asks the player to do (src/player/engine.ts PlayerAction): during a listening
/// session these go to the session first (jam/engine.rs), which may take them over
#[derive(Clone, Debug)]
pub enum PlayerAction {
    PlayQueue { tracks: Vec<Track>, start: usize },
    PlayIndex(usize),
    Toggle,
    Next,
    Prev,
    Seek(f64),
    Add { track: Track, next: bool },
    Remove(usize),
    Move(usize, usize),
    ClearUpcoming,
    Shuffle,
    Repeat,
}

/// returns true when it took the action over (the player then does nothing itself)
pub type Steer = Arc<dyn Fn(&PlayerAction) -> bool + Send + Sync>;

thread_local! {
    /// the session's own calls on this thread are not steered back to it
    static UNSTEERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// do `f` without the session taking the actions over (the session itself plays this way)
pub fn unsteered<R>(f: impl FnOnce() -> R) -> R {
    let was = UNSTEERED.with(|u| u.replace(true));
    let r = f();
    UNSTEERED.with(|u| u.set(was));
    r
}

pub struct Player {
    /// a listening session steering what is pressed
    steer: Mutex<Option<Steer>>,
    /// a guest of a session: the host's player moves on, this one only follows it (no song
    /// is prepared or started here by itself)
    follower: AtomicBool,
    core: Mutex<Core>,
    wake: Condvar,
    resolver: Arc<dyn Resolve>,
    /// what the person sees changed (the window is told to draw)
    notify: Box<dyn Fn() + Send + Sync>,
}

impl Player {
    pub fn new(resolver: Arc<dyn Resolve>, notify: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        let st = state::get();
        let mut engine = Engine::new();
        let volume = st.volume();
        engine.set_volume(volume);
        engine.set_speed(st.effects.read().unwrap().speed as f32);
        let core = Core {
            view: View { volume, index: 0, ..Default::default() },
            engine,
            active: 0,
            token: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            deck: None,
            prepared: None,
            preparing: None,
            prepare_skip: None,
            fading_until: None,
            switched_seen: 0,
            resume_at: 0.0,
            failed_in_a_row: 0,
            skip_at: None,
            paused_since: None,
            heard_ms: 0.0,
            last_pos: -1.0,
            counted: false,
            source: None,
            source_ids: Default::default(),
            radio_busy: false,
            play_when_extended: false,
            first_track: true,
            last_saved: String::new(),
            last_save: Instant::now(),
            leveler: Default::default(),
            leveler_track: None,
            leveler_at: Instant::now(),
            device_check: Instant::now(),
            session: String::new(),
            session_track: None,
        };
        let me = Arc::new(Self { steer: Mutex::new(None), follower: AtomicBool::new(false), core: Mutex::new(core), wake: Condvar::new(), resolver, notify: Box::new(notify) });
        me.apply_sound();
        let watcher = Arc::downgrade(&me);
        let _ = std::thread::Builder::new().name("player".into()).spawn(move || watch(watcher));
        me
    }

    /// a copy of what the window shows
    pub fn view(&self) -> View {
        let mut c = self.core.lock().unwrap();
        c.refresh_position();
        c.view.clone()
    }

    /// the current song without copying the queue
    pub fn current(&self) -> Option<Track> {
        let c = self.core.lock().unwrap();
        c.view.current().cloned()
    }

    pub fn status(&self) -> Status {
        self.core.lock().unwrap().view.status
    }

    /// the song's length, without copying the queue
    pub fn duration_ms(&self) -> f64 {
        self.core.lock().unwrap().view.duration_ms
    }

    pub fn position_ms(&self) -> f64 {
        let mut c = self.core.lock().unwrap();
        c.refresh_position();
        c.view.position_ms
    }

    /// how far the heard sound is behind the position, in ms (lyrics wait for it)
    pub fn latency_ms(&self) -> f64 {
        self.core.lock().unwrap().engine.latency_ms()
    }

    pub fn device(&self) -> String {
        self.core.lock().unwrap().engine.device.clone()
    }

    fn changed(&self) {
        self.wake.notify_all();
        (self.notify)();
    }

    // ---- a listening session -----------------------------------------------------------------

    /// a session steers the player (None: back to normal); `follower`: this copy follows a host
    pub fn steer_with(&self, steer: Option<Steer>, follower: bool) {
        *self.steer.lock().unwrap() = steer;
        self.follower.store(follower, Ordering::Relaxed);
        if follower {
            self.core.lock().unwrap().drop_prepared();
        }
    }

    /// the session took the action over
    fn steered(&self, a: PlayerAction) -> bool {
        if UNSTEERED.with(|u| u.get()) {
            return false;
        }
        let steer = self.steer.lock().unwrap().clone();
        steer.is_some_and(|s| s(&a))
    }

    fn following(&self) -> bool {
        self.follower.load(Ordering::Relaxed)
    }

    /// the queue as a host shows it, without starting anything (a guest's view)
    pub fn show_queue(&self, queue: Vec<Track>, index: usize) {
        let mut c = self.core.lock().unwrap();
        c.view.queue = queue;
        c.view.index = index;
        drop(c);
        self.changed();
    }

    /// the song playing reached its end (a guest waits for the host's next song)
    pub fn at_end(&self) -> bool {
        self.core.lock().unwrap().deck.as_ref().is_some_and(|d| d.ended.load(Ordering::Relaxed))
    }

    /// what a session's host watches, without copying the queue: (a hash of the queue's songs,
    /// index, status, position)
    pub fn fingerprint(&self) -> (u64, usize, Status, f64) {
        use std::hash::{Hash, Hasher};
        let mut c = self.core.lock().unwrap();
        c.refresh_position();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        c.view.queue.len().hash(&mut h);
        for t in &c.view.queue {
            t.id.hash(&mut h);
        }
        (h.finish(), c.view.index, c.view.status, c.view.position_ms)
    }

    /// the whole queue and where it is, without copying the view twice
    pub fn queue_state(&self) -> (Vec<Track>, usize, Status, f64) {
        let mut c = self.core.lock().unwrap();
        c.refresh_position();
        (c.view.queue.clone(), c.view.index, c.view.status, c.view.position_ms)
    }

    // ---- what the person does --------------------------------------------------------------

    /// play `tracks` from `start`; `source` is the playlist or album they come from
    pub fn play_queue(self: &Arc<Self>, tracks: Vec<Track>, start: usize, source: Option<PlaySource>) {
        if tracks.is_empty() || self.steered(PlayerAction::PlayQueue { tracks: tracks.clone(), start }) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        c.source_ids = if source.is_some() { tracks.iter().map(|t| t.id.clone()).collect() } else { Default::default() };
        c.source = source;
        c.resume_at = 0.0;
        let start = start.min(tracks.len() - 1);
        let (queue, index) = if c.view.shuffle { (queue::shuffled(&tracks, Some(&tracks[start].id)), 0) } else { (tracks, start) };
        c.view.queue = queue;
        c.view.index = index;
        self.load(&mut c);
        drop(c);
        self.changed();
    }

    /// A song in a list was pressed. The song already on goes on from where it is, but the
    /// list becomes the queue (so "next" plays the next song of that list).
    pub fn play_from(self: &Arc<Self>, tracks: Vec<Track>, index: usize, source: Option<PlaySource>) {
        let Some(track) = tracks.get(index).cloned() else { return };
        if self.steered(PlayerAction::PlayQueue { tracks: tracks.clone(), start: index }) {
            return;
        }
        let same = self.core.lock().unwrap().view.current().map(|t| t.id == track.id).unwrap_or(false);
        if !same {
            return self.play_queue(tracks, index, source);
        }
        {
            let mut c = self.core.lock().unwrap();
            if !queue::same_songs(&c.view.queue, &tracks) {
                let (q, i) = if c.view.shuffle { (queue::shuffled(&tracks, Some(&track.id)), 0) } else { (tracks.clone(), index) };
                c.view.queue = q;
                c.view.index = i;
                c.source_ids = if source.is_some() { tracks.iter().map(|t| t.id.clone()).collect() } else { Default::default() };
                c.source = source;
                c.drop_prepared();
            }
        }
        self.toggle();
    }

    /// last session's queue, shown paused until play is pressed
    pub fn restore(&self, s: Session) {
        let mut c = self.core.lock().unwrap();
        let index = s.index.max(0) as usize;
        let Some(track) = s.queue.get(index).cloned() else { return };
        c.view.queue = s.queue;
        c.view.index = index;
        c.view.status = Status::Idle;
        c.view.position_ms = s.position_ms;
        c.view.duration_ms = track.duration_ms;
        c.resume_at = if s.position_ms > 3000.0 { s.position_ms } else { 0.0 };
        drop(c);
        self.changed();
    }

    pub fn play_index(self: &Arc<Self>, index: usize) {
        if self.steered(PlayerAction::PlayIndex(index)) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        let Some(track) = c.view.queue.get(index).cloned() else { return };
        c.resume_at = 0.0;
        // "next" onto the song that is already waiting: switch at once
        if c.prepared.as_ref().is_some_and(|p| p.index == index && p.track.id == track.id && p.deck.ready.load(Ordering::Relaxed)) {
            self.transition(&mut c, 0.0);
        } else {
            c.view.index = index;
            self.load(&mut c);
        }
        drop(c);
        self.changed();
    }

    pub fn toggle(self: &Arc<Self>) {
        if self.steered(PlayerAction::Toggle) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        match c.view.status {
            Status::Playing => {
                c.finish_fade();
                c.engine.set_paused(true);
                c.view.status = Status::Paused;
                c.paused_since = Some(Instant::now());
            }
            Status::Paused => {
                if c.engine.ensure_running().is_ok() {
                    c.engine.set_paused(false);
                    c.view.status = if c.deck.as_ref().is_some_and(|d| d.ready.load(Ordering::Relaxed)) { Status::Playing } else { Status::Loading };
                    c.paused_since = None;
                }
            }
            Status::Idle | Status::Error => {
                if c.view.current().is_some() {
                    self.load(&mut c);
                }
            }
            Status::Loading => {}
        }
        drop(c);
        self.changed();
    }

    pub fn next(self: &Arc<Self>) {
        if self.steered(PlayerAction::Next) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        self.advance(&mut c, true);
        drop(c);
        self.changed();
    }

    pub fn prev(self: &Arc<Self>) {
        if self.steered(PlayerAction::Prev) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        c.refresh_position();
        if c.view.position_ms > RESTART_AFTER_MS {
            c.seek_to(0.0);
        } else if let Some(i) = queue::prev_index(c.view.queue.len(), c.view.index, c.view.repeat) {
            drop(c);
            return self.play_index(i);
        }
        drop(c);
        self.changed();
    }

    pub fn seek(&self, ms: f64) {
        if self.steered(PlayerAction::Seek(ms)) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        c.seek_to(ms);
        drop(c);
        self.changed();
    }

    pub fn seek_by(&self, delta_ms: f64) {
        let mut c = self.core.lock().unwrap();
        c.refresh_position();
        let to = (c.view.position_ms + delta_ms).clamp(0.0, c.view.duration_ms.max(0.0));
        if !UNSTEERED.with(|u| u.get()) && self.steer.lock().unwrap().is_some() {
            drop(c);
            return self.seek(to);
        }
        c.seek_to(to);
        drop(c);
        self.changed();
    }

    pub fn set_volume(&self, v: f32) {
        let mut c = self.core.lock().unwrap();
        let v = v.clamp(0.0, 1.0);
        c.view.volume = v;
        c.view.muted = false;
        c.engine.set_volume(v);
        drop(c);
        state::get().save_volume(v);
        self.changed();
    }

    pub fn toggle_mute(&self) {
        let mut c = self.core.lock().unwrap();
        c.view.muted = !c.view.muted;
        let v = if c.view.muted { 0.0 } else { c.view.volume };
        c.engine.set_volume(v);
        drop(c);
        self.changed();
    }

    pub fn toggle_shuffle(&self) {
        if self.steered(PlayerAction::Shuffle) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        if !c.view.shuffle {
            let first = c.view.current().map(|t| t.id.clone());
            c.view.queue = queue::shuffled(&c.view.queue, first.as_deref());
            c.view.index = 0;
            c.view.shuffle = true;
        } else {
            c.view.shuffle = false;
        }
        c.drop_prepared();
        drop(c);
        self.changed();
    }

    pub fn cycle_repeat(&self) {
        if self.steered(PlayerAction::Repeat) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        c.view.repeat = c.view.repeat.next();
        // the next song was armed to follow; with "repeat one" the same one comes again
        c.drop_prepared();
        drop(c);
        self.changed();
    }

    /// put a song right after the current one, or at the end
    pub fn add(self: &Arc<Self>, track: Track, next: bool) {
        if self.steered(PlayerAction::Add { track: track.clone(), next }) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        if c.view.queue.is_empty() {
            drop(c);
            return self.play_queue(vec![track], 0, None);
        }
        if next {
            let at = c.view.index + 1;
            c.view.queue.insert(at, track);
        } else {
            c.view.queue.push(track);
        }
        c.check_prepared();
        drop(c);
        self.changed();
    }

    pub fn remove(&self, index: usize) {
        if self.steered(PlayerAction::Remove(index)) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        if index == c.view.index || index >= c.view.queue.len() {
            return;
        }
        c.view.queue.remove(index);
        if index < c.view.index {
            c.view.index -= 1;
        }
        c.check_prepared();
        drop(c);
        self.changed();
    }

    pub fn move_song(&self, from: usize, to: usize) {
        if self.steered(PlayerAction::Move(from, to)) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        let index = c.view.index;
        c.view.index = queue::move_in_queue(&mut c.view.queue, index, from, to);
        c.check_prepared();
        drop(c);
        self.changed();
    }

    pub fn clear_upcoming(&self) {
        if self.steered(PlayerAction::ClearUpcoming) {
            return;
        }
        let mut c = self.core.lock().unwrap();
        let keep = (c.view.index + 1).min(c.view.queue.len());
        c.view.queue.truncate(keep);
        c.drop_prepared();
        drop(c);
        self.changed();
    }

    /// a station from one song: the song, then similar ones
    pub fn start_radio(self: &Arc<Self>, seed: Track) {
        if self.steered(PlayerAction::PlayQueue { tracks: vec![seed.clone()], start: 0 }) {
            return;
        }
        self.play_queue(vec![seed.clone()], 0, None);
        let me = self.clone();
        std::thread::spawn(move || {
            let mut exclude = std::collections::HashSet::new();
            exclude.insert(seed.id.clone());
            let more = me.resolver.similar(&seed, &exclude);
            let mut c = me.core.lock().unwrap();
            if c.view.current().is_some_and(|t| t.id == seed.id) {
                c.view.queue.extend(more);
            }
            drop(c);
            me.changed();
        });
    }

    /// "Скорость": the speed of every song now (linked: the pitch moves with it)
    pub fn apply_speed(&self, _speed: f64) {
        self.apply_sound();
    }

    /// the effects and the equalizer as they are set now, sent to the sound card's thread
    pub fn apply_sound(&self) {
        let mut c = self.core.lock().unwrap();
        c.apply_sound();
    }

    /// the effects' processing chain on the sound card's thread
    pub fn with_chain<R>(&self, f: impl FnOnce(&mut crate::audio::dsp::Chain) -> R) -> Option<R> {
        let c = self.core.lock().unwrap();
        let mixer = c.engine.mixer().clone();
        drop(c);
        let mut m = mixer.lock().ok()?;
        Some(f(&mut m.chain))
    }

    /// save the queue now (on quit)
    pub fn save_session(&self) {
        let mut c = self.core.lock().unwrap();
        c.save_session(true);
        c.finish_listen();
    }

    /// quit: silence first, so the sound card is not cut mid-sample
    pub fn shutdown(&self) {
        let mut c = self.core.lock().unwrap();
        c.engine.set_paused(true);
        c.cancel.store(true, Ordering::Relaxed);
        drop(c);
        std::thread::sleep(Duration::from_millis(30));
        let mut c = self.core.lock().unwrap();
        c.engine.unload(0);
        c.engine.unload(1);
        c.engine.suspend();
    }

    // ---- loading -----------------------------------------------------------------------------

    /// start the current song from its first working source
    fn load(self: &Arc<Self>, c: &mut Core) {
        let Some(track) = c.view.current().cloned() else { return };
        c.token += 1;
        c.cancel.store(true, Ordering::Relaxed);
        c.cancel = Arc::new(AtomicBool::new(false));
        c.finish_fade();
        c.drop_prepared();
        c.prepare_skip = None;
        c.skip_at = None;
        let active = c.active;
        c.engine.unload(active);
        c.deck = None;
        c.view.status = Status::Loading;
        c.view.position_ms = c.resume_at;
        c.view.duration_ms = track.duration_ms;
        c.view.error = None;
        c.view.route = None;
        c.view.quality = None;
        c.view.buffered = 0.0;
        self.begin(c, &track);
        if let Err(e) = c.engine.ensure_running() {
            c.view.status = Status::Error;
            c.view.error = Some(format!("Звук не выводится: {e}"));
            return;
        }
        c.engine.set_paused(false);
        c.paused_since = None;
        let token = c.token;
        let cancel = c.cancel.clone();
        let me = self.clone();
        let _ = std::thread::Builder::new().name("load".into()).spawn(move || me.load_worker(track, token, cancel));
    }

    fn load_worker(self: Arc<Self>, track: Track, token: u64, cancel: Arc<AtomicBool>) {
        let mut last_error: Option<String> = None;
        let mut started = false;
        for candidate in self.resolver.clone().candidates(&track, cancel.clone()) {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let candidate = match candidate {
                Ok(c) => c,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };
            let deck = {
                let mut c = self.core.lock().unwrap();
                if c.token != token {
                    return;
                }
                let active = c.active;
                let at = std::mem::take(&mut c.resume_at);
                let deck = c.engine.load(active, candidate.spec.clone(), at);
                c.engine.set_gain(active, 1.0);
                c.engine.play(active, true);
                c.deck = Some(deck.clone());
                deck
            };
            // wait until it really plays, fails, or takes too long
            let since = Instant::now();
            let outcome = loop {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(e) = deck.error() {
                    break Err(e);
                }
                if deck.ready.load(Ordering::Relaxed) {
                    break Ok(());
                }
                if since.elapsed() > START_TIMEOUT {
                    break Err("Трек слишком долго не начинается".to_string());
                }
                std::thread::sleep(Duration::from_millis(30));
            };
            match outcome {
                Ok(()) => {
                    let mut c = self.core.lock().unwrap();
                    if c.token != token {
                        return;
                    }
                    c.failed_in_a_row = 0;
                    c.view.status = if c.engine.shared.paused.load(Ordering::Relaxed) { Status::Paused } else { Status::Playing };
                    c.view.route = candidate.via.as_ref().map(|t| t.source);
                    c.view.quality = candidate.quality.clone().or_else(|| Some(deck.codec.lock().unwrap().clone()).filter(|s| !s.is_empty()));
                    let known = c.view.current().map(|t| t.duration_ms).unwrap_or(0.0);
                    c.view.duration_ms = length_of(deck.duration_ms(), known.max(c.view.duration_ms));
                    started = true;
                    drop(c);
                    self.changed();
                    break;
                }
                Err(e) => {
                    eprintln!("[player] this source did not play, trying the next: {e}");
                    last_error = Some(e);
                }
            }
        }
        if !started && !cancel.load(Ordering::Relaxed) {
            let mut c = self.core.lock().unwrap();
            if c.token == token {
                let msg = last_error.unwrap_or_else(|| "Не нашлось, откуда взять звук этой песни".into());
                c.failed(&msg);
            }
            drop(c);
            self.changed();
        }
    }

    /// what happens when a song starts, however it started
    fn begin(self: &Arc<Self>, c: &mut Core, track: &Track) {
        c.finish_listen();
        c.session = format!("{:x}{:04x}", crate::store::state::now_ms() as u64, rand::random::<u16>());
        c.session_track = Some(track.clone());
        c.level_for(&track.id);
        c.heard_ms = 0.0;
        c.last_pos = -1.0;
        c.counted = false;
        if !c.first_track {
            // effects that are not kept reset with the song
            let fx = state::get().effects.read().unwrap().clone();
            if !fx.keep && !fx.is_neutral() {
                state::get().set_effects(|e| *e = crate::store::state::Effects { keep: false, ..Default::default() });
                c.apply_sound();
            }
        }
        c.first_track = false;
        // a song near the end of the queue: similar songs are added quietly
        let near_end = c.view.index + 2 >= c.view.queue.len();
        if near_end && c.view.repeat == Repeat::Off && state::get().settings.read().unwrap().autoplay {
            self.extend_with_similar(c, false);
        }
        let _ = track;
    }

    fn advance(self: &Arc<Self>, c: &mut Core, manual: bool) {
        match queue::next_index(c.view.queue.len(), c.view.index, c.view.repeat, manual) {
            None => {
                if state::get().settings.read().unwrap().autoplay {
                    self.extend_with_similar(c, true);
                } else if manual && c.view.queue.len() > 1 {
                    c.view.index = 0;
                    self.load(c);
                } else {
                    c.engine.set_paused(true);
                    c.view.status = Status::Paused;
                }
            }
            Some(i) if i == c.view.index && !manual => {
                c.seek_to(0.0);
                c.engine.set_paused(false);
                c.view.status = Status::Playing;
            }
            Some(i) => {
                if c.prepared.as_ref().is_some_and(|p| p.index == i && p.deck.ready.load(Ordering::Relaxed)) {
                    self.transition(c, 0.0);
                } else {
                    c.view.index = i;
                    c.resume_at = 0.0;
                    self.load(c);
                }
            }
        }
    }

    /// add similar songs after the queue; `play`: jump to the first of them (the queue ran out)
    fn extend_with_similar(self: &Arc<Self>, c: &mut Core, play: bool) {
        let Some(seed) = c.view.current().cloned() else { return };
        if play {
            c.play_when_extended = true;
        }
        if c.radio_busy {
            return;
        }
        c.radio_busy = true;
        let from = c.view.index.saturating_sub(100);
        let mut exclude: std::collections::HashSet<String> = c.view.queue[from..].iter().map(|t| t.id.clone()).collect();
        exclude.extend(c.view.queue[from..].iter().map(resolve::song_key));
        let index_then = c.view.index;
        let me = self.clone();
        std::thread::spawn(move || {
            let more = me.resolver.similar(&seed, &exclude);
            let mut c = me.core.lock().unwrap();
            c.radio_busy = false;
            let play = std::mem::take(&mut c.play_when_extended);
            if more.is_empty() {
                if play {
                    c.engine.set_paused(true);
                    c.view.status = Status::Paused;
                    c.view.error = Some("Не нашлось похожих треков".into());
                }
            } else {
                c.view.queue.extend(more);
                if play && c.view.index == index_then {
                    c.view.index = index_then + 1;
                    c.resume_at = 0.0;
                    me.load(&mut c);
                }
            }
            drop(c);
            me.changed();
        });
    }

    /// get the next song ready on the other deck
    fn prepare_next(self: &Arc<Self>, c: &mut Core) {
        if c.view.repeat == Repeat::One || c.prepared.is_some() || c.preparing.is_some() {
            return;
        }
        let Some(index) = queue::next_index(c.view.queue.len(), c.view.index, c.view.repeat, false) else { return };
        if index == c.view.index {
            return;
        }
        let track = c.view.queue[index].clone();
        if c.prepare_skip.as_deref() == Some(track.id.as_str()) {
            return;
        }
        c.preparing = Some((c.token, index, track.id.clone()));
        let token = c.token;
        let cancel = c.cancel.clone();
        let me = self.clone();
        std::thread::spawn(move || {
            let first = me.resolver.clone().candidates(&track, cancel.clone()).find_map(|r| r.ok());
            let mut c = me.core.lock().unwrap();
            let still = c.token == token && c.preparing.as_ref().is_some_and(|(_, i, id)| *i == index && *id == track.id);
            c.preparing = None;
            if !still {
                return;
            }
            let Some(Candidate { spec, via, quality }) = first else {
                c.prepare_skip = Some(track.id.clone());
                return;
            };
            let spare = 1 - c.active;
            let deck = c.engine.load(spare, spec, 0.0);
            c.engine.set_gain(spare, 1.0);
            // with no crossfade the next song follows the end itself, between two samples
            let crossfade = state::get().settings.read().unwrap().crossfade;
            let armed = crossfade <= 0.0;
            c.engine.arm(spare, armed);
            c.prepared = Some(Prepared { index, track, deck, route: via.map(|t| t.source), quality, armed });
        });
    }

    /// hand over to the prepared song: at once (fade 0) or fading over `fade` seconds
    fn transition(self: &Arc<Self>, c: &mut Core, fade: f64) {
        let Some(p) = c.prepared.take() else { return };
        if c.view.queue.get(p.index).map(|t| t.id != p.track.id).unwrap_or(true) {
            let spare = 1 - c.active;
            c.engine.unload(spare);
            return self.advance(c, false);
        }
        let old = c.active;
        let new = 1 - old;
        c.active = new;
        c.token += 1;
        c.cancel.store(true, Ordering::Relaxed);
        c.cancel = Arc::new(AtomicBool::new(false));
        c.engine.arm(new, false);
        if fade > 0.2 {
            c.engine.set_gain(new, 0.0);
            c.engine.play(new, true);
            c.engine.fade(new, 1.0, fade as f32);
            c.engine.fade(old, 0.0, fade as f32);
            c.fading_until = Some(Instant::now() + Duration::from_secs_f64(fade + 0.15));
        } else {
            c.engine.set_gain(new, 1.0);
            c.engine.play(new, true);
            c.engine.unload(old);
        }
        c.engine.set_paused(false);
        c.deck = Some(p.deck.clone());
        c.view.index = p.index;
        c.view.status = Status::Playing;
        c.view.position_ms = 0.0;
        c.view.duration_ms = length_of(p.deck.duration_ms(), p.track.duration_ms);
        c.view.error = None;
        c.view.route = p.route;
        c.view.quality = p.quality.or_else(|| Some(p.deck.codec.lock().unwrap().clone()).filter(|s| !s.is_empty()));
        c.failed_in_a_row = 0;
        self.begin(c, &p.track);
    }

    /// the armed deck took over by itself at the end of the song (no crossfade)
    fn took_over(self: &Arc<Self>, c: &mut Core) {
        let Some(p) = c.prepared.take() else { return };
        let old = c.active;
        c.active = 1 - old;
        c.engine.unload(old);
        c.token += 1;
        c.deck = Some(p.deck.clone());
        c.view.index = p.index;
        c.view.status = Status::Playing;
        c.view.position_ms = 0.0;
        c.view.duration_ms = length_of(p.deck.duration_ms(), p.track.duration_ms);
        c.view.error = None;
        c.view.route = p.route;
        c.view.quality = p.quality.or_else(|| Some(p.deck.codec.lock().unwrap().clone()).filter(|s| !s.is_empty()));
        self.begin(c, &p.track);
    }

    /// one look at the player while it plays; false when there is nothing to look after
    fn tick(self: &Arc<Self>) -> bool {
        let mut c = self.core.lock().unwrap();
        c.engine.collect();
        let mut changed = false;
        // the armed deck took over
        let switched = c.engine.shared.switched.load(Ordering::Relaxed);
        if switched != c.switched_seen {
            c.switched_seen = switched;
            if c.prepared.as_ref().is_some_and(|p| p.armed) {
                self.took_over(&mut c);
                changed = true;
            }
        }
        if let Some(until) = c.fading_until {
            if Instant::now() >= until {
                c.finish_fade();
            }
        }
        if let Some((at, token)) = c.skip_at {
            if Instant::now() >= at {
                c.skip_at = None;
                if token == c.token && c.view.status == Status::Error && !self.following() {
                    self.advance(&mut c, true);
                    changed = true;
                }
            }
        }
        let playing = c.view.status == Status::Playing;
        if let Some(deck) = c.deck.clone() {
            if playing {
                let before = c.view.position_ms;
                c.refresh_position();
                let pos = c.view.position_ms;
                c.heard(pos);
                c.level_tick();
                if (pos - before).abs() > 250.0 {
                    changed = true;
                }
                // the song broke off in the middle
                if deck.ended.load(Ordering::Relaxed) {
                    if let Some(e) = deck.error() {
                        c.failed(&e);
                        changed = true;
                    } else if c.prepared.as_ref().is_some_and(|p| p.armed) {
                        // the sound card's thread switches by itself; nothing to do here
                    } else if self.following() {
                        // a guest: the host's player moves on, and this one follows it
                    } else {
                        self.advance(&mut c, false);
                        changed = true;
                    }
                } else {
                    let known = c.view.current().map(|t| t.duration_ms).unwrap_or(0.0);
                    let duration = length_of(deck.duration_ms(), known);
                    let left = duration - pos;
                    let fade = state::get().settings.read().unwrap().crossfade.max(0.0);
                    if duration > 0.0 && left < PREPARE_BEFORE_MS + fade * 1000.0 && c.prepared.is_none() && c.fading_until.is_none() && !self.following() {
                        self.prepare_next(&mut c);
                    }
                    let ready = c.prepared.as_ref().is_some_and(|p| p.deck.ready.load(Ordering::Relaxed));
                    if fade > 0.0 && ready && duration > 0.0 && left <= fade * 1000.0 && c.view.repeat != Repeat::One && c.fading_until.is_none() {
                        self.transition(&mut c, (left / 1000.0).min(fade));
                        changed = true;
                    }
                }
                let buffering = deck.starved.load(Ordering::Relaxed);
                if buffering != c.view.buffering {
                    c.view.buffering = buffering;
                    changed = true;
                }
                if let Some(r) = deck.remote.lock().unwrap().as_ref() {
                    c.view.buffered = r.fetched();
                } else {
                    c.view.buffered = 1.0;
                }
            }
        }
        // headphones unplugged or another output chosen in the system: the stream moves to the
        // new device; with "pause when headphones are unplugged" the music stops instead of
        // going on from the speakers (the page did this with devicechange)
        if c.device_check.elapsed() > Duration::from_secs(2) && c.engine.is_running() {
            c.device_check = Instant::now();
            if c.engine.output_changed() {
                let pause = state::get().settings.read().unwrap().pause_on_device_change;
                if pause && c.view.status == Status::Playing {
                    c.engine.set_paused(true);
                    c.view.status = Status::Paused;
                    c.paused_since = Some(Instant::now());
                } else if c.view.status == Status::Playing {
                    let _ = c.engine.ensure_running();
                }
                changed = true;
            }
        }
        // a long pause lets the sound card go
        if let Some(since) = c.paused_since {
            if since.elapsed() > SUSPEND_AFTER && c.engine.is_running() {
                c.engine.suspend();
            }
        }
        if c.last_save.elapsed() > Duration::from_secs(10) {
            c.save_session(false);
        }
        let busy = matches!(c.view.status, Status::Playing | Status::Loading) || c.skip_at.is_some() || c.fading_until.is_some() || (c.paused_since.is_some() && c.engine.is_running());
        drop(c);
        if changed {
            (self.notify)();
        }
        busy
    }
}

impl Core {
    /// the effects and the equalizer as the sound card's thread needs them
    fn apply_sound(&mut self) {
        use crate::audio::dsp::{eq, BandParams, Params};
        let st = state::get();
        let fx = st.effects.read().unwrap().clone();
        let e = st.eq.read().unwrap().clone();
        let mut p = Params::default();
        if e.enabled {
            for (slot, b) in p.bands.iter_mut().zip(e.bands.iter()) {
                *slot = BandParams { kind: b.kind, frequency: b.frequency as f32, gain: if b.enabled { b.gain as f32 } else { 0.0 }, q: b.q as f32 };
            }
        }
        // with clip protection the level comes down by most of the boost; a limiter catches the rest
        let headroom = if e.enabled { (if e.protect { -eq::peak_boost_db(&e.bands) * 0.75 } else { 0.0 }) + e.preamp } else { 0.0 };
        p.preamp_db = headroom as f32;
        p.limiter = e.protect && (e.enabled || e.normalize);
        p.normalize = e.normalize;
        // the speed reads the song faster; unlinked, the shifter puts the pitch where it is asked
        let speed = fx.speed.clamp(0.5, 2.0);
        p.pitch = if fx.linked { 1.0 } else { (2f64.powf(fx.pitch / 12.0) / speed) as f32 };
        p.reverb = if fx.reverb { fx.reverb_amount.clamp(0.0, 1.0) as f32 } else { 0.0 };
        self.engine.set_speed(speed as f32);
        self.engine.set_fx(p);
        if !e.normalize {
            self.engine.shared.level_db.store(0f32.to_bits(), Ordering::Relaxed);
        }
    }

    /// a new song: the last one's loudness is remembered, this one starts from its own
    fn level_for(&mut self, id: &str) {
        let st = state::get();
        if let Some(prev) = &self.leveler_track {
            if self.leveler.settled() {
                if let Some(db) = self.leveler.loudness_db() {
                    st.remember_loudness(prev, db);
                }
            }
        }
        self.leveler = crate::audio::dsp::loudness::Leveler::new(st.remembered_loudness(id));
        self.leveler_track = Some(id.to_string());
        self.leveler_at = Instant::now();
        let g = self.leveler.gain_db(crate::audio::dsp::loudness::TARGET_DB) as f32;
        self.engine.shared.level_db.store(g.to_bits(), Ordering::Relaxed);
    }

    /// a look at the level while the song plays (the page did it every 150 ms, then every second)
    fn level_tick(&mut self) {
        if !state::get().eq.read().unwrap().normalize {
            return;
        }
        let every = if self.leveler.settled() { Duration::from_secs(1) } else { Duration::from_millis(150) };
        let dt = self.leveler_at.elapsed();
        if dt < every {
            return;
        }
        self.leveler_at = Instant::now();
        let power = f32::from_bits(self.engine.shared.level_power.load(Ordering::Relaxed)) as f64;
        self.leveler.update(power, dt.as_millis().min(2000) as f64);
        let g = self.leveler.gain_db(crate::audio::dsp::loudness::TARGET_DB) as f32;
        self.engine.shared.level_db.store(g.to_bits(), Ordering::Relaxed);
    }

    fn refresh_position(&mut self) {
        if let Some(d) = &self.deck {
            if d.ready.load(Ordering::Relaxed) && matches!(self.view.status, Status::Playing | Status::Paused) {
                self.view.position_ms = d.position_ms();
            }
        }
    }

    fn seek_to(&mut self, ms: f64) {
        if !ms.is_finite() {
            return;
        }
        let ms = ms.max(0.0);
        match &self.deck {
            Some(_) if matches!(self.view.status, Status::Playing | Status::Paused | Status::Loading) => {
                let active = self.active;
                self.engine.seek(active, ms);
            }
            // not started yet (a restored session): it starts there
            _ => self.resume_at = ms,
        }
        self.last_pos = ms;
        self.view.position_ms = ms;
        // the song moved away from its end: what was prepared to follow waits again
        if self.prepared.as_ref().is_some_and(|p| p.armed) {
            let spare = 1 - self.active;
            self.engine.arm(spare, true);
        }
    }

    fn heard(&mut self, pos: f64) {
        if self.last_pos >= 0.0 {
            let delta = pos - self.last_pos;
            // seeks are not listening: only small steps forward count
            if delta > 0.0 && delta < 3000.0 {
                self.heard_ms += delta;
            }
        }
        self.last_pos = pos;
        if !self.counted && self.heard_ms >= HISTORY_AFTER_MS {
            self.counted = true;
            let Some(track) = self.view.current().cloned() else { return };
            let st = state::get();
            if !st.settings.read().unwrap().no_history {
                let from = self.source.clone().filter(|_| self.source_ids.contains(&track.id));
                st.update_history(|h| h.push(&track, from.as_ref()));
            }
            crate::stats::record(&track, &self.session, self.heard_ms);
        }
    }

    /// the song is left: its listened time goes into the statistics (once counted)
    fn finish_listen(&mut self) {
        if self.counted {
            if let Some(t) = &self.session_track {
                crate::stats::record(t, &self.session, self.heard_ms);
            }
        }
    }

    fn finish_fade(&mut self) {
        if self.fading_until.take().is_some() {
            let active = self.active;
            self.engine.set_gain(active, 1.0);
            self.engine.unload(1 - active);
        }
    }

    fn drop_prepared(&mut self) {
        self.preparing = None;
        if self.prepared.take().is_some() && self.fading_until.is_none() {
            let spare = 1 - self.active;
            self.engine.unload(spare);
        }
    }

    /// the queue changed under the prepared song: get the right one ready instead
    fn check_prepared(&mut self) {
        let next = queue::next_index(self.view.queue.len(), self.view.index, self.view.repeat, false);
        let ok = self.prepared.as_ref().map(|p| Some(p.index) == next && self.view.queue.get(p.index).map(|t| t.id == p.track.id).unwrap_or(false)).unwrap_or(true);
        if !ok {
            self.drop_prepared();
        }
    }

    /// the song cannot be played: say why and go on with the next one half a second later
    fn failed(&mut self, message: &str) {
        self.failed_in_a_row += 1;
        let message = message.trim_end_matches(['.', ' ']);
        if self.failed_in_a_row >= MAX_FAILED_IN_A_ROW {
            self.failed_in_a_row = 0;
            self.view.status = Status::Error;
            self.view.error = Some(format!("{message}. Подряд не включилось {MAX_FAILED_IN_A_ROW} треков — проверьте интернет"));
            return;
        }
        self.view.status = Status::Error;
        self.view.error = Some(format!("{message} — включаю следующий"));
        self.skip_at = Some((Instant::now() + SKIP_BROKEN_AFTER, self.token));
    }

    fn save_session(&mut self, force: bool) {
        self.last_save = Instant::now();
        if self.view.queue.is_empty() {
            return;
        }
        self.refresh_position();
        let q = &self.view.queue;
        let key = format!("{}|{}|{}|{}|{}", q.len(), q[0].id, q[q.len() - 1].id, self.view.index, (self.view.position_ms / 5000.0).round());
        if key == self.last_saved && !force {
            return;
        }
        self.last_saved = key;
        // a long queue is cut around the current song so the file stays small
        let start = self.view.index.saturating_sub(50);
        let end = (start + 300).min(q.len());
        let s = Session { queue: q[start..end].to_vec(), index: (self.view.index - start) as i64, position_ms: self.view.position_ms };
        state::get().save_session(&s);
    }
}

/// the player's own thread: looks after it while it plays, sleeps while it does not
fn watch(player: std::sync::Weak<Player>) {
    loop {
        let Some(p) = player.upgrade() else { return };
        let busy = p.tick();
        if busy {
            drop(p);
            std::thread::sleep(TICK);
        } else {
            // nothing to look after: sleep until something is done with the player
            let c = p.core.lock().unwrap();
            let _ = p.wake.wait_timeout(c, Duration::from_secs(30)).unwrap();
        }
    }
}

#[cfg(test)]
mod length_tests {
    #[test]
    fn a_length_far_shorter_than_the_song_s_is_not_believed() {
        // the first ten-second piece of a three-minute song
        assert_eq!(super::length_of(Some(10_000.0), 191_000.0), 191_000.0);
        // a copy a little shorter or longer than the original: its own length
        assert_eq!(super::length_of(Some(186_000.0), 191_000.0), 186_000.0);
        assert_eq!(super::length_of(Some(200_000.0), 191_000.0), 200_000.0);
        assert_eq!(super::length_of(None, 191_000.0), 191_000.0);
        assert_eq!(super::length_of(Some(10_000.0), 0.0), 10_000.0);
    }
}
