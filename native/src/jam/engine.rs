//! A listening session (src/jam/engine.ts, the same rules and the same messages, so native
//! and Tauri copies share sessions). The host's player is the one that counts: it plays as
//! usual and tells everyone what it does a few times a second at most (and every few seconds
//! anyway). Guests follow it, and what a guest presses goes to the host as a request, so there
//! is one queue and one clock. Guests say they are still there every few seconds; whoever goes
//! quiet drops out of the list.
//!
//! Who gets in is the host's call: anyone with the code, only whoever the host lets in, or
//! nobody new. A session can also be put in the public list (a room everyone knows the key
//! of), where others see it with the number of people in it.
//!
//! The network and its threads exist only while a session (or the public list) is open; one
//! small thread keeps the session's clock (beats, hellos, timeouts) and sleeps between them.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::link::Link;
use super::protocol::{self, DRIFT_MS, LOBBY, MAX_PEOPLE};
use crate::model::{Source, Track};
use crate::player::{unsteered, Player, PlayerAction, Status};

/// the host repeats where it is this often, even when nothing changes
const BEAT: Duration = Duration::from_millis(4000);
/// and the whole visible queue every this many beats (a guest may have missed a change)
const FULL_EVERY: u64 = 6;
/// a listed session says it is there every this many beats
const ANNOUNCE_EVERY: u64 = 3;
const HERE: Duration = Duration::from_secs(10);
/// a guest not heard from for this long is gone
const GUEST_GONE: Duration = Duration::from_secs(40);
/// the host not heard from for this long: the session is over for this guest
const HOST_GONE: Duration = Duration::from_secs(30);
const JOIN_TIMEOUT: Duration = Duration::from_secs(12);
/// how long someone waits to be let in (and the host has to answer)
const KNOCK: Duration = Duration::from_secs(180);
/// a listed session not heard from for this long is taken off the list
const LISTED_FOR: Duration = Duration::from_secs(30);
/// a guest asks for the queue again at most this often
const RESYNC: Duration = Duration::from_secs(3);
/// about how long a message takes from the host to a guest through the relays (ms)
const LATENCY_MS: f64 = 150.0;
/// after a guest's own seek, the host's reports are not followed for this long
const SEEK_HOLD: Duration = Duration::from_millis(2500);
/// when a guest's song starts, it catches up with the host past this much (ms)
const START_DRIFT_MS: f64 = 250.0;
/// a few changes in a row go out as one message
const SEND_SOON: Duration = Duration::from_millis(120);
/// the session's clock looks at things this often
const TICK: Duration = Duration::from_millis(200);
const CHAT_KEPT: usize = 50;
const REACTIONS_KEPT: usize = 20;
const KEY: &str = "evplayer.jam.v1";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Control {
    #[default]
    Anyone,
    Host,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    #[default]
    Open,
    Ask,
    Closed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum JamStatus {
    #[default]
    Idle,
    Starting,
    Joining,
    Live,
}

/// how a session started here is set up (and stays set up for the next one)
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Setup {
    pub control: Control,
    pub access: Access,
    pub listed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub color: String,
    pub host: bool,
}

#[derive(Clone, Debug)]
pub struct Knock {
    pub id: String,
    pub name: String,
    pub color: String,
    pub at: Instant,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub from: String,
    pub text: String,
    pub at: Instant,
}

#[derive(Clone, Debug)]
pub struct Listed {
    pub code: String,
    pub host: String,
    pub people: usize,
    pub access: Access,
    pub song: Option<String>,
    pub seen: Instant,
}

/// what the window shows of a session (src/jam/store.ts)
#[derive(Clone, Debug, Default)]
pub struct View {
    pub open: bool,
    pub status: JamStatus,
    /// joining: asked the host, waiting to be let in
    pub waiting: bool,
    pub host: bool,
    pub code: String,
    pub control: Control,
    pub access: Access,
    pub listed: bool,
    pub people: Vec<Person>,
    pub knocks: Vec<Knock>,
    pub chat: Vec<Line>,
    pub reactions: Vec<Line>,
    /// who put a song in the queue, by track id
    pub added_by: HashMap<String, String>,
    pub lobby: Vec<Listed>,
    pub setup: Setup,
    pub browse: bool,
    pub name: String,
    pub color: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    name: String,
    color: String,
    setup: Setup,
    browse: bool,
}

#[derive(Default)]
struct Engine {
    link: Option<Arc<Link>>,
    lobby_link: Option<Arc<Link>>,
    browsing: bool,
    // host
    heard: HashMap<String, Instant>,
    banned: HashSet<String>,
    pass: String,
    rev: u64,
    sent_rev: Option<u64>,
    beats: u64,
    next_beat: Option<Instant>,
    send_at: Option<Instant>,
    watched: Option<(u64, usize, Status, f64, Instant)>,
    // guest
    host_id: String,
    last_from_host: Option<Instant>,
    offset: usize,
    queue_rev: Option<u64>,
    last_resync: Option<Instant>,
    before: Option<(Vec<Track>, usize, f64)>,
    join_deadline: Option<(Instant, String)>,
    warned_local: String,
    playing_id: String,
    seek_held_until: Option<Instant>,
    next_here: Option<Instant>,
}

pub struct Jam {
    view: Mutex<View>,
    engine: Mutex<Engine>,
    player: Arc<Player>,
    clock_running: std::sync::atomic::AtomicBool,
}

static JAM: OnceLock<Arc<Jam>> = OnceLock::new();

pub fn get() -> Option<&'static Arc<Jam>> {
    JAM.get()
}

fn toast(text: impl Into<String>) {
    crate::app::toast_later(text);
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

fn ticket() -> String {
    rand::random::<[u8; 12]>().iter().map(|b| format!("{b:02x}")).collect()
}

fn uid() -> String {
    format!("{:032x}", rand::random::<u128>())
}

pub fn init(player: Arc<Player>) -> &'static Arc<Jam> {
    JAM.get_or_init(|| Jam::new(player))
}

impl Jam {
    /// a session engine for this player (the program has one: `init`)
    pub fn new(player: Arc<Player>) -> Arc<Jam> {
        {
        let read: Option<Saved> = crate::store::kv::read_state(&crate::state::get().kv, KEY);
        // nothing saved yet: the public list is shown by default
        let browse = read.as_ref().map(|s| s.browse).unwrap_or(true);
        let saved = read.unwrap_or_default();
        let color = if saved.color.is_empty() { protocol::COLORS[rand::random::<usize>() % protocol::COLORS.len()].to_string() } else { saved.color };
        let view = View { name: saved.name, color, setup: saved.setup, browse, ..Default::default() };
        Arc::new(Jam { view: Mutex::new(view), engine: Mutex::new(Engine::default()), player, clock_running: false.into() })
        }
    }
}

impl Jam {
    pub fn view(&self) -> View {
        self.view.lock().unwrap().clone()
    }

    fn set(&self, f: impl FnOnce(&mut View)) {
        f(&mut self.view.lock().unwrap());
        crate::events::repaint();
        // the Discord status shows the session's size and its "Слушать вместе" button
        crate::presence::changed();
    }

    fn save(&self) {
        let v = self.view.lock().unwrap();
        let s = Saved { name: v.name.clone(), color: v.color.clone(), setup: v.setup, browse: v.browse };
        drop(v);
        crate::store::kv::write_state(&crate::state::get().kv, KEY, &s, 0);
    }

    pub fn set_open(self: &Arc<Self>, open: bool) {
        self.set(|v| v.open = open);
        if !open {
            self.browse(false);
        }
    }

    pub fn set_profile(&self, name: Option<String>, color: Option<String>) {
        self.set(|v| {
            if let Some(n) = name {
                v.name = n.chars().take(24).collect();
            }
            if let Some(c) = color {
                v.color = c;
            }
        });
        self.save();
    }

    pub fn set_browse(self: &Arc<Self>, on: bool) {
        self.set(|v| v.browse = on);
        self.save();
        self.browse(on);
    }

    fn my_id(&self) -> String {
        self.engine.lock().unwrap().link.as_ref().map(|l| l.id()).unwrap_or_default()
    }

    /// the pass for Discord invites (the host's, while the session is on)
    pub fn discord_pass(&self) -> String {
        if self.view.lock().unwrap().host {
            self.engine.lock().unwrap().pass.clone()
        } else {
            String::new()
        }
    }

    fn send(&self, msg: Value) {
        let link = self.engine.lock().unwrap().link.clone();
        if let Some(l) = link {
            l.send(&msg);
        }
    }

    fn name_of(&self, id: &str) -> String {
        self.view.lock().unwrap().people.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| crate::i18n::t("Кто-то"))
    }

    fn note_added_by(&self, track_id: &str, name: &str) {
        self.set(|v| {
            v.added_by.insert(track_id.to_string(), name.to_string());
        });
    }

    fn open_link(self: &Arc<Self>, code: &str) -> Result<Arc<Link>, String> {
        let existing = self.engine.lock().unwrap().link.clone();
        let link = match existing {
            Some(l) => l,
            None => {
                let me = Arc::downgrade(self);
                let l = Arc::new(Link::open(Box::new(move |msg, from| {
                    if let Some(j) = me.upgrade() {
                        j.receive(msg, from);
                    }
                }))?);
                self.engine.lock().unwrap().link = Some(l.clone());
                l
            }
        };
        link.join(code);
        Ok(link)
    }

    fn name_is_set(&self) -> bool {
        if self.view.lock().unwrap().name.trim().is_empty() {
            toast(crate::i18n::t("Сначала напишите своё имя: так вас увидят в сессии"));
            return false;
        }
        true
    }

    // ---- starting and leaving ------------------------------------------------------------------

    pub fn start(self: &Arc<Self>) {
        let v = self.view();
        if v.status != JamStatus::Idle || !self.name_is_set() {
            return;
        }
        self.set(|v| v.status = JamStatus::Starting);
        let me = self.clone();
        std::thread::spawn(move || {
            let code = protocol::new_code();
            let link = match me.open_link(&code) {
                Ok(l) => l,
                Err(e) => {
                    me.reset();
                    toast(e);
                    return;
                }
            };
            let v = me.view();
            let person = Person { id: link.id(), name: v.name.trim().to_string(), color: v.color.clone(), host: true };
            {
                let mut e = me.engine.lock().unwrap();
                e.pass = ticket();
                e.rev += 1;
                e.next_beat = Some(Instant::now() + BEAT);
            }
            me.set(|v| {
                v.status = JamStatus::Live;
                v.host = true;
                v.code = code.clone();
                v.people = vec![person];
                v.control = v.setup.control;
                v.access = v.setup.access;
                v.listed = v.setup.listed;
            });
            let weak = Arc::downgrade(&me);
            me.player.steer_with(Some(Arc::new(move |a| weak.upgrade().is_some_and(|j| j.steer_host(a)))), false);
            me.run_clock();
            if v.setup.listed {
                me.announce();
            }
        });
    }

    pub fn join(self: &Arc<Self>, input: &str) {
        let v = self.view();
        if v.status != JamStatus::Idle || !self.name_is_set() {
            return;
        }
        let code = protocol::normalize_code(input);
        if code.is_empty() {
            toast(crate::i18n::t("Это не похоже на код сессии: он выглядит как JAM-7K9P-2QXZ"));
            return;
        }
        self.set(|v| {
            v.status = JamStatus::Joining;
            v.code = code.clone();
        });
        let me = self.clone();
        let ticket = protocol::ticket_of(input);
        std::thread::spawn(move || {
            if let Err(e) = me.open_link(&code) {
                me.finish(Some(e));
                return;
            }
            let v = me.view();
            let mut msg = json!({ "t": "join", "name": v.name.trim(), "color": v.color });
            if let Some(t) = ticket {
                msg["ticket"] = t.into();
            }
            me.send(msg);
            me.engine.lock().unwrap().join_deadline = Some((Instant::now() + JOIN_TIMEOUT, crate::i18n::t("Сессия не нашлась. Проверьте код: организатор должен быть в сети.")));
            me.run_clock();
        });
    }

    /// leave (a guest), stop waiting to be let in, or end the session for everyone (the host)
    pub fn leave(self: &Arc<Self>) {
        let v = self.view();
        if v.status == JamStatus::Idle {
            return;
        }
        if v.status == JamStatus::Live || v.waiting {
            self.send(if v.host { json!({ "t": "end" }) } else { json!({ "t": "leave" }) });
            // time to get "I'm leaving" out before the connection closes
            std::thread::sleep(Duration::from_millis(600));
        }
        let message = if v.status != JamStatus::Live { None } else if v.host { Some(crate::i18n::t("Сессия завершена")) } else { Some(crate::i18n::t("Вы вышли из сессии")) };
        self.finish(message);
    }

    /// the program closes: a goodbye, so the others do not wait for the timeout
    pub fn goodbye(&self) {
        let v = self.view();
        if v.status == JamStatus::Live || v.waiting {
            self.send(if v.host { json!({ "t": "end" }) } else { json!({ "t": "leave" }) });
        }
        if v.host && v.listed {
            if let Some(l) = self.engine.lock().unwrap().lobby_link.clone() {
                l.send(&json!({ "t": "gone", "code": v.code }));
            }
        }
        std::thread::sleep(Duration::from_millis(400));
    }

    fn reset(&self) {
        self.set(|v| {
            v.status = JamStatus::Idle;
            v.waiting = false;
            v.host = false;
            v.code.clear();
            v.control = Control::Anyone;
            v.access = Access::Open;
            v.listed = false;
            v.people.clear();
            v.knocks.clear();
            v.chat.clear();
            v.reactions.clear();
            v.added_by.clear();
        });
    }

    /// stop everything here; a guest gets their own music back, paused
    fn finish(&self, message: Option<String>) {
        let v = self.view();
        let was_guest = !v.host && v.status == JamStatus::Live;
        if v.host && v.listed {
            self.unlist(&v.code);
        }
        let before = {
            let mut e = self.engine.lock().unwrap();
            if let Some(l) = e.link.take() {
                l.close();
            }
            let before = e.before.take();
            let lobby = e.lobby_link.take();
            let browsing = e.browsing;
            *e = Engine { lobby_link: lobby, browsing, ..Default::default() };
            before
        };
        self.player.steer_with(None, false);
        if was_guest {
            // the session's music stops; what this guest listened to before comes back
            unsteered(|| {
                if self.player.status() == Status::Playing {
                    self.player.toggle();
                }
                if let Some((queue, index, pos)) = before.filter(|b| !b.0.is_empty()) {
                    self.player.restore(crate::store::state::Session { queue, index: index as i64, position_ms: pos });
                }
            });
        }
        self.reset();
        if let Some(m) = message {
            toast(m);
        }
    }

    // ---- the clock -------------------------------------------------------------------------------

    fn run_clock(self: &Arc<Self>) {
        if self.clock_running.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        let _ = std::thread::Builder::new().name("jam".into()).spawn(move || {
            loop {
                std::thread::sleep(TICK);
                let v = me.view();
                let busy = v.status != JamStatus::Idle || me.engine.lock().unwrap().lobby_link.is_some();
                if !busy {
                    break;
                }
                me.tick(&v);
            }
            me.clock_running.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }

    fn tick(self: &Arc<Self>, v: &View) {
        let now = Instant::now();
        // the public list forgets sessions not heard from
        if v.lobby.iter().any(|l| now.duration_since(l.seen) > LISTED_FOR) {
            self.set(|v| v.lobby.retain(|l| now.duration_since(l.seen) <= LISTED_FOR));
        }
        if v.status == JamStatus::Joining {
            let deadline = self.engine.lock().unwrap().join_deadline.clone();
            if let Some((t, msg)) = deadline {
                if now >= t {
                    self.finish(Some(msg));
                }
            }
            return;
        }
        if v.status != JamStatus::Live {
            return;
        }
        if v.host {
            self.watch_player();
            let (send, beat) = {
                let e = self.engine.lock().unwrap();
                (e.send_at.is_some_and(|t| now >= t), e.next_beat.is_some_and(|t| now >= t))
            };
            if send {
                self.send_state();
            }
            if beat {
                self.beat(v);
            }
        } else {
            let (here, gone) = {
                let mut e = self.engine.lock().unwrap();
                let here = e.next_here.is_some_and(|t| now >= t);
                if here {
                    e.next_here = Some(now + HERE);
                }
                (here, e.last_from_host.is_some_and(|t| now.duration_since(t) > HOST_GONE))
            };
            if here {
                self.send(json!({ "t": "here" }));
            }
            if gone {
                self.finish(Some(crate::i18n::t("Связь с организатором пропала, сессия закрыта")));
            }
        }
    }

    // ---- the host --------------------------------------------------------------------------------

    /// the host's own actions go through as usual; who added a song is noted and told
    fn steer_host(&self, a: &PlayerAction) -> bool {
        let me = self.view.lock().unwrap().name.clone();
        let id = self.my_id();
        match a {
            PlayerAction::Add { track, .. } => {
                self.note_added_by(&track.id, &me);
                self.send(json!({ "t": "note", "text": format!("{me} добавляет «{}»", track.title), "by": id }));
            }
            PlayerAction::PlayQueue { tracks, start } => {
                if let Some(t) = tracks.get(*start) {
                    self.note_added_by(&t.id, &me);
                    self.send(json!({ "t": "note", "text": format!("{me} включает «{}»", t.title), "by": id }));
                }
            }
            _ => {}
        }
        false
    }

    /// anything the others need to know about is sent soon after it happens
    fn watch_player(&self) {
        let (hash, index, status, pos) = self.player.fingerprint();
        let now = Instant::now();
        let mut e = self.engine.lock().unwrap();
        let soon = match e.watched {
            None => true,
            Some((h, i, s, p, at)) => {
                if h != hash || i != index {
                    // the visible part of the queue follows the current song
                    e.rev += 1;
                }
                let expected = if s == Status::Playing { p + now.duration_since(at).as_millis() as f64 } else { p };
                let seeked = (pos - expected).abs() > 2500.0;
                h != hash || i != index || s != status || seeked
            }
        };
        e.watched = Some((hash, index, status, pos, now));
        if soon && e.send_at.is_none() {
            e.send_at = Some(now + SEND_SOON);
        }
    }

    fn send_soon(&self) {
        let mut e = self.engine.lock().unwrap();
        if e.send_at.is_none() {
            e.send_at = Some(Instant::now() + SEND_SOON);
        }
    }

    fn beat(self: &Arc<Self>, v: &View) {
        let beats = {
            let mut e = self.engine.lock().unwrap();
            e.beats += 1;
            e.next_beat = Some(Instant::now() + BEAT);
            if e.beats % FULL_EVERY == 0 {
                e.sent_rev = None;
            }
            e.beats
        };
        self.send_state();
        let now = Instant::now();
        // guests who went quiet, and people who waited too long to be let in
        let heard = self.engine.lock().unwrap().heard.clone();
        for p in v.people.iter().filter(|p| !p.host) {
            if heard.get(&p.id).is_none_or(|t| now.duration_since(*t) > GUEST_GONE) {
                self.drop_person(&p.id, Some(format!("{} {}", p.name, crate::i18n::t("отключился"))));
            }
        }
        if v.knocks.iter().any(|k| now.duration_since(k.at) > KNOCK) {
            self.set(|v| v.knocks.retain(|k| now.duration_since(k.at) <= KNOCK));
        }
        if v.listed && beats % ANNOUNCE_EVERY == 0 {
            self.announce();
        }
    }

    fn now_playing(&self) -> Value {
        let (queue, index, status, pos) = self.player.queue_state();
        json!({
            "index": index,
            "trackId": queue.get(index).map(|t| t.id.clone()),
            // a song still loading is not playing yet: guests start it when the host's starts
            "playing": status == Status::Playing,
            "positionMs": pos.round(),
            "at": now_ms(),
        })
    }

    fn send_state(&self) {
        let v = self.view();
        if !v.host || v.status != JamStatus::Live {
            return;
        }
        let (queue, index, _, _) = self.player.queue_state();
        let (rev, window) = {
            let mut e = self.engine.lock().unwrap();
            e.send_at = None;
            let w = if e.sent_rev != Some(e.rev) {
                let mut w = protocol::queue_window(&queue, index, e.rev, 60);
                let by: serde_json::Map<String, Value> = w["tracks"].as_array().map(|t| t.iter().filter_map(|t| t["id"].as_str()).filter_map(|id| v.added_by.get(id).map(|n| (id.to_string(), Value::String(n.clone())))).collect()).unwrap_or_default();
                w["by"] = Value::Object(by);
                e.sent_rev = Some(e.rev);
                Some(w)
            } else {
                None
            };
            (e.rev, w)
        };
        let mut msg = json!({ "t": "state", "code": v.code, "control": v.control, "access": v.access, "people": v.people, "now": self.now_playing(), "rev": rev });
        if let Some(w) = window {
            msg["queue"] = w;
        }
        self.send(msg);
    }

    fn drop_person(&self, id: &str, message: Option<String>) {
        self.engine.lock().unwrap().heard.remove(id);
        self.set(|v| v.people.retain(|p| p.id != id));
        if let Some(m) = message {
            toast(m);
        }
        self.send_soon();
    }

    fn refuse(&self, to: &str, reason: &str) {
        self.send(json!({ "t": "refuse", "to": to, "reason": reason }));
    }

    /// why this person cannot come in ("" when they can)
    fn kept_out(&self, id: &str, name: &str) -> String {
        let v = self.view.lock().unwrap();
        if name.is_empty() {
            return crate::i18n::t("Напишите своё имя, чтобы войти");
        }
        if v.people.len() >= MAX_PEOPLE {
            return crate::i18n::t("В сессии уже нет мест");
        }
        if v.people.iter().any(|p| p.id != id && protocol::same_name(&p.name, name)) {
            return format!("Имя «{name}» в этой сессии уже занято. Выберите другое.");
        }
        String::new()
    }

    fn host_receive(self: &Arc<Self>, msg: &Value, from: &str) {
        let v = self.view();
        let member = v.people.iter().any(|p| p.id == from);
        if member {
            self.engine.lock().unwrap().heard.insert(from.to_string(), Instant::now());
        }
        match msg["t"].as_str().unwrap_or("") {
            "join" => {
                let name: String = msg["name"].as_str().unwrap_or("").trim().chars().take(24).collect();
                let color = msg["color"].as_str().unwrap_or("#6ff0c4").to_string();
                let pass = self.engine.lock().unwrap().pass.clone();
                let invited = !pass.is_empty() && msg["ticket"].as_str() == Some(pass.as_str());
                if member {
                    return self.let_in(from, &name, &color);
                }
                if self.engine.lock().unwrap().banned.contains(from) && !invited {
                    return self.refuse(from, &crate::i18n::t("Организатор исключил вас из этой сессии"));
                }
                let reason = self.kept_out(from, &name);
                if !reason.is_empty() {
                    return self.refuse(from, &reason);
                }
                if !invited && v.access == Access::Closed {
                    return self.refuse(from, &crate::i18n::t("Организатор закрыл вход в эту сессию"));
                }
                if !invited && v.access == Access::Ask {
                    let known = v.knocks.iter().any(|k| k.id == from);
                    let (f, n, c) = (from.to_string(), name.clone(), color.clone());
                    self.set(|v| {
                        v.knocks.retain(|k| k.id != f);
                        v.knocks.push(Knock { id: f, name: n, color: c, at: Instant::now() });
                    });
                    self.send(json!({ "t": "wait", "to": from }));
                    if !known {
                        toast(format!("{name} {}", crate::i18n::t("просится в сессию")));
                    }
                    return;
                }
                self.let_in(from, &name, &color);
            }
            "leave" => {
                if member {
                    self.drop_person(from, Some(format!("{} {}", self.name_of(from), crate::i18n::t("вышел из сессии"))));
                } else {
                    let f = from.to_string();
                    self.set(|v| v.knocks.retain(|k| k.id != f));
                }
            }
            "resync" if member => {
                self.engine.lock().unwrap().sent_rev = None;
                self.send_soon();
            }
            "ask" if member => self.apply(&msg["req"], from),
            "chat" | "react" if member => self.show(msg, from),
            _ => {}
        }
    }

    fn let_in(&self, id: &str, name: &str, color: &str) {
        let already = self.view.lock().unwrap().people.iter().any(|p| p.id == id);
        let person = Person { id: id.to_string(), name: name.to_string(), color: color.to_string(), host: false };
        self.set(|v| {
            v.people.retain(|p| p.id != person.id);
            v.knocks.retain(|k| k.id != person.id);
            v.people.push(person);
        });
        {
            let mut e = self.engine.lock().unwrap();
            e.heard.insert(id.to_string(), Instant::now());
            // the newcomer needs the whole visible queue
            e.sent_rev = None;
        }
        if !already {
            toast(format!("{name} {}", crate::i18n::t("слушает вместе с вами")));
        }
        self.send_state();
    }

    /// the host lets in someone who asked
    pub fn admit(&self, id: &str) {
        let Some(k) = self.view.lock().unwrap().knocks.iter().find(|k| k.id == id).cloned() else { return };
        let reason = self.kept_out(id, &k.name);
        if !reason.is_empty() {
            let i = id.to_string();
            self.set(|v| v.knocks.retain(|x| x.id != i));
            self.refuse(id, &reason);
            toast(reason);
            return;
        }
        self.let_in(id, &k.name, &k.color);
    }

    /// the host does not let someone in
    pub fn turn_away(&self, id: &str) {
        let i = id.to_string();
        self.set(|v| v.knocks.retain(|k| k.id != i));
        self.refuse(id, &crate::i18n::t("Организатор не пустил вас в сессию"));
    }

    /// a guest's request, done on the host's player (which then tells everyone)
    fn apply(&self, req: &Value, from: &str) {
        let who = self.name_of(from);
        let what = req["do"].as_str().unwrap_or("");
        // the host decides, but a song may always be added
        if self.view.lock().unwrap().control == Control::Host && what != "add" {
            return;
        }
        let (queue, _, status, _) = self.player.queue_state();
        let at = |index: Option<u64>, id: &str| -> Option<usize> {
            let i = index.unwrap_or(u64::MAX) as usize;
            if queue.get(i).is_some_and(|t| t.id == id) {
                Some(i)
            } else {
                queue.iter().position(|t| t.id == id)
            }
        };
        let track_of = |v: &Value| serde_json::from_value::<Track>(v.clone()).ok();
        let p = self.player.clone();
        unsteered(|| match what {
            "toggle" => {
                if req["playing"].as_bool() != Some(status == Status::Playing) {
                    p.toggle();
                }
            }
            "next" => p.next(),
            "prev" => p.prev(),
            "seek" => p.seek(req["ms"].as_f64().unwrap_or(0.0)),
            "jump" => {
                if let Some(i) = at(req["index"].as_u64(), req["trackId"].as_str().unwrap_or("")) {
                    p.play_index(i);
                }
            }
            "play" => {
                let tracks: Vec<Track> = req["tracks"].as_array().map(|l| l.iter().filter_map(track_of).collect()).unwrap_or_default();
                let start = req["startIndex"].as_u64().unwrap_or(0) as usize;
                if let Some(t) = tracks.get(start) {
                    self.note_added_by(&t.id, &who);
                    let line = format!("{who} включает «{}»", t.title);
                    toast(line.clone());
                    self.send(json!({ "t": "note", "text": line, "by": from }));
                    p.play_queue(tracks.clone(), start, None);
                }
            }
            "add" => {
                if let Some(t) = track_of(&req["track"]) {
                    self.note_added_by(&t.id, &who);
                    let line = format!("{who} добавляет «{}»", t.title);
                    toast(line.clone());
                    self.send(json!({ "t": "note", "text": line, "by": from }));
                    p.add(t, req["next"].as_bool().unwrap_or(false));
                }
            }
            "remove" => {
                if let Some(i) = at(req["index"].as_u64(), req["trackId"].as_str().unwrap_or("")) {
                    p.remove(i);
                }
            }
            "move" => p.move_song(req["from"].as_u64().unwrap_or(0) as usize, req["to"].as_u64().unwrap_or(0) as usize),
            "clear" => p.clear_upcoming(),
            _ => {}
        });
    }

    // ---- the host's controls ---------------------------------------------------------------------

    pub fn set_control(&self, control: Control) {
        self.set(|v| v.setup.control = control);
        self.save();
        let v = self.view();
        if v.host && v.status == JamStatus::Live {
            self.set(|v| v.control = control);
            self.send_soon();
        }
    }

    /// who gets in from now on; closing turns away everyone still waiting
    pub fn set_access(self: &Arc<Self>, access: Access) {
        self.set(|v| v.setup.access = access);
        self.save();
        let v = self.view();
        if !v.host || v.status != JamStatus::Live {
            return;
        }
        self.set(|v| v.access = access);
        for k in &v.knocks {
            match access {
                Access::Closed => self.turn_away(&k.id),
                Access::Open => self.admit(&k.id),
                Access::Ask => {}
            }
        }
        if v.listed {
            if access == Access::Closed {
                self.unlist(&v.code);
            } else {
                self.announce();
            }
        }
        self.send_soon();
    }

    /// put the session in the public list, or take it off
    pub fn set_listed(self: &Arc<Self>, listed: bool) {
        self.set(|v| v.setup.listed = listed);
        self.save();
        let v = self.view();
        if !v.host || v.status != JamStatus::Live {
            return;
        }
        self.set(|v| v.listed = listed);
        if listed {
            self.announce();
        } else {
            self.unlist(&v.code);
        }
    }

    pub fn kick(&self, id: &str) {
        let v = self.view();
        let Some(p) = v.people.iter().find(|p| p.id == id).cloned() else { return };
        if !v.host || p.host {
            return;
        }
        self.engine.lock().unwrap().banned.insert(id.to_string());
        self.send(json!({ "t": "kick", "id": id }));
        self.drop_person(id, Some(format!("{} {}", p.name, crate::i18n::t("исключён из сессии"))));
    }

    /// A new code (and so a new link and Discord pass). The people in the session move along on
    /// their own; the old code leads nowhere any more.
    pub fn change_code(self: &Arc<Self>) {
        let v = self.view();
        if !v.host || v.status != JamStatus::Live {
            return;
        }
        let code = protocol::new_code();
        let Some(link) = self.engine.lock().unwrap().link.clone() else { return };
        for p in v.people.iter().filter(|p| !p.host) {
            if let Some(sealed) = link.seal(&code, &p.id) {
                link.send(&json!({ "t": "moved", "to": p.id, "sealed": sealed }));
            }
        }
        for k in &v.knocks {
            self.refuse(&k.id, &crate::i18n::t("Организатор сменил код сессии"));
        }
        link.join(&code);
        if v.listed {
            self.unlist(&v.code);
        }
        {
            let mut e = self.engine.lock().unwrap();
            e.banned.clear();
            e.pass = ticket();
            e.sent_rev = None;
        }
        let c = code.clone();
        self.set(|v| {
            v.code = c;
            v.knocks.clear();
        });
        self.send_state();
        if v.listed {
            self.announce();
        }
        toast(format!("{} {code}", crate::i18n::t("Новый код сессии:")));
    }

    // ---- the public list -------------------------------------------------------------------------

    fn lobby(self: &Arc<Self>) -> Option<Arc<Link>> {
        if let Some(l) = self.engine.lock().unwrap().lobby_link.clone() {
            return Some(l);
        }
        let me = Arc::downgrade(self);
        let l = Arc::new(
            Link::open(Box::new(move |msg, _| {
                if let Some(j) = me.upgrade() {
                    j.lobby_receive(&msg);
                }
            }))
            .ok()?,
        );
        l.join(LOBBY);
        self.engine.lock().unwrap().lobby_link = Some(l.clone());
        self.run_clock();
        Some(l)
    }

    /// the lobby is closed when nobody here looks at it or announces in it
    fn lobby_idle(&self) {
        let v = self.view();
        let mut e = self.engine.lock().unwrap();
        if e.browsing || (v.host && v.listed) {
            return;
        }
        if let Some(l) = e.lobby_link.take() {
            l.close();
        }
    }

    fn announce(self: &Arc<Self>) {
        let v = self.view();
        if !v.host || v.status != JamStatus::Live || !v.listed || v.access == Access::Closed {
            return;
        }
        let song = self.player.current().map(|t| format!("{} — {}", t.artists.first().map(|a| a.name.as_str()).unwrap_or(""), t.title));
        let me = self.clone();
        std::thread::spawn(move || {
            if let Some(l) = me.lobby() {
                l.send(&json!({ "t": "open", "code": v.code, "host": v.name, "people": v.people.len(), "access": v.access, "song": song }));
            }
        });
    }

    fn unlist(&self, code: &str) {
        if let Some(l) = self.engine.lock().unwrap().lobby_link.clone() {
            l.send(&json!({ "t": "gone", "code": code }));
        }
        self.lobby_idle();
    }

    /// show the public list (while the join screen is open) or stop
    pub fn browse(self: &Arc<Self>, on: bool) {
        self.engine.lock().unwrap().browsing = on;
        if !on {
            self.set(|v| v.lobby.clear());
            self.lobby_idle();
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            let _ = me.lobby();
        });
    }

    fn lobby_receive(&self, msg: &Value) {
        match msg["t"].as_str() {
            Some("gone") => {
                let code = msg["code"].as_str().unwrap_or("").to_string();
                self.set(|v| v.lobby.retain(|l| l.code != code));
            }
            Some("open") if self.engine.lock().unwrap().browsing => {
                let code = protocol::normalize_code(msg["code"].as_str().unwrap_or(""));
                if code.is_empty() || code == self.view.lock().unwrap().code {
                    return;
                }
                let host: String = msg["host"].as_str().unwrap_or("").chars().take(24).collect();
                let entry = Listed {
                    code: code.clone(),
                    host: if host.is_empty() { crate::i18n::t("Кто-то") } else { host },
                    people: (msg["people"].as_u64().unwrap_or(1) as usize).clamp(1, MAX_PEOPLE),
                    access: if msg["access"].as_str() == Some("ask") { Access::Ask } else { Access::Open },
                    song: msg["song"].as_str().map(|s| s.chars().take(120).collect()),
                    seen: Instant::now(),
                };
                self.set(|v| {
                    v.lobby.retain(|l| l.code != code);
                    v.lobby.push(entry);
                    v.lobby.sort_by(|a, b| b.people.cmp(&a.people));
                    v.lobby.truncate(50);
                });
            }
            _ => {}
        }
    }

    // ---- a guest ---------------------------------------------------------------------------------

    /// what a guest presses becomes a request to the host
    fn steer_guest(&self, a: &PlayerAction) -> bool {
        let v = self.view();
        let ask = |req: Value| self.send(json!({ "t": "ask", "req": req }));
        let offset = self.engine.lock().unwrap().offset;
        let (queue, _, status, _) = self.player.queue_state();
        match a {
            PlayerAction::Shuffle | PlayerAction::Repeat => {
                toast(crate::i18n::t("Перемешивание и повтор в сессии — у организатора"));
                return true;
            }
            PlayerAction::Add { track, next } => {
                self.note_added_by(&track.id, &v.name);
                ask(json!({ "do": "add", "track": track, "next": next }));
                toast(if *next { format!("«{}» сыграет следующим", track.title) } else { format!("«{}» добавлен в общую очередь", track.title) });
                return true;
            }
            _ => {}
        }
        if v.control == Control::Host {
            // a list pressed: its song goes into the queue instead
            if let PlayerAction::PlayQueue { tracks, start } = a {
                if let Some(t) = tracks.get(*start) {
                    self.note_added_by(&t.id, &v.name);
                    ask(json!({ "do": "add", "track": t, "next": false }));
                    toast(format!("Музыкой управляет организатор: «{}» добавлен в очередь", t.title));
                    return true;
                }
            }
            toast(crate::i18n::t("Музыкой в этой сессии управляет организатор"));
            return true;
        }
        match a {
            PlayerAction::Toggle => ask(json!({ "do": "toggle", "playing": status != Status::Playing })),
            PlayerAction::Next => ask(json!({ "do": "next" })),
            PlayerAction::Prev => ask(json!({ "do": "prev" })),
            PlayerAction::Seek(ms) => {
                ask(json!({ "do": "seek", "ms": ms }));
                // heard here at once; the host's next report then agrees
                self.engine.lock().unwrap().seek_held_until = Some(Instant::now() + SEEK_HOLD);
                let p = self.player.clone();
                let ms = *ms;
                unsteered(|| p.seek(ms));
            }
            PlayerAction::PlayIndex(i) => {
                if let Some(t) = queue.get(*i) {
                    ask(json!({ "do": "jump", "index": i + offset, "trackId": t.id }));
                }
            }
            PlayerAction::PlayQueue { tracks, start } => {
                let (list, s) = protocol::fit_tracks(tracks, *start);
                if let Some(t) = list.get(s) {
                    self.note_added_by(&t.id, &v.name);
                    ask(json!({ "do": "play", "tracks": list, "startIndex": s }));
                }
            }
            PlayerAction::Remove(i) => {
                if let Some(t) = queue.get(*i) {
                    ask(json!({ "do": "remove", "index": i + offset, "trackId": t.id }));
                }
            }
            PlayerAction::Move(from, to) => ask(json!({ "do": "move", "from": from + offset, "to": to + offset })),
            PlayerAction::ClearUpcoming => ask(json!({ "do": "clear" })),
            _ => {}
        }
        true
    }

    fn guest_receive(self: &Arc<Self>, msg: &Value, from: &str) {
        let v = self.view();
        let me = self.my_id();
        let from_host = {
            let mut e = self.engine.lock().unwrap();
            let h = e.host_id == from;
            if h {
                e.last_from_host = Some(Instant::now());
            }
            h
        };
        match msg["t"].as_str().unwrap_or("") {
            "state" => self.follow_state(msg, from),
            "refuse" => {
                if msg["to"].as_str() == Some(me.as_str()) && v.status == JamStatus::Joining {
                    self.finish(Some(msg["reason"].as_str().unwrap_or("").to_string()));
                }
            }
            "wait" => {
                if msg["to"].as_str() == Some(me.as_str()) && v.status == JamStatus::Joining {
                    self.set(|v| v.waiting = true);
                    self.engine.lock().unwrap().join_deadline = Some((Instant::now() + KNOCK, crate::i18n::t("Организатор так и не ответил")));
                }
            }
            "kick" => {
                if from_host && msg["id"].as_str() == Some(me.as_str()) {
                    self.finish(Some(crate::i18n::t("Организатор исключил вас из сессии")));
                }
            }
            "moved" => {
                if !from_host || msg["to"].as_str() != Some(me.as_str()) {
                    return;
                }
                let link = self.engine.lock().unwrap().link.clone();
                if let Some(l) = link {
                    let code = protocol::normalize_code(&l.unseal(msg["sealed"].as_str().unwrap_or(""), from).unwrap_or_default());
                    if !code.is_empty() {
                        let c = code.clone();
                        self.set(|v| v.code = c);
                        l.join(&code);
                    }
                }
            }
            "end" => {
                if from_host {
                    self.finish(Some(crate::i18n::t("Организатор завершил сессию")));
                }
            }
            "note" => {
                if from_host && msg["by"].as_str() != Some(me.as_str()) {
                    toast(msg["text"].as_str().unwrap_or("").chars().take(160).collect::<String>());
                }
            }
            "chat" | "react" => {
                if v.people.iter().any(|p| p.id == from) {
                    self.show(msg, from);
                }
            }
            _ => {}
        }
    }

    fn follow_state(self: &Arc<Self>, msg: &Value, from: &str) {
        let v = self.view();
        let me = self.my_id();
        let people: Vec<Person> = serde_json::from_value(msg["people"].clone()).unwrap_or_default();
        let host_id = self.engine.lock().unwrap().host_id.clone();
        // only the host speaks for the session: the key marked as the host in it
        if if host_id.is_empty() { !people.iter().any(|p| p.id == from && p.host) } else { from != host_id } {
            return;
        }
        let inside = people.iter().any(|p| p.id == me);
        if v.status == JamStatus::Joining {
            if !inside {
                return; // the host has not let us in yet
            }
            self.welcome(from);
        } else if !inside {
            self.finish(Some(crate::i18n::t("Вы больше не в этой сессии")));
            return;
        }
        let control: Control = serde_json::from_value(msg["control"].clone()).unwrap_or_default();
        let access: Access = serde_json::from_value(msg["access"].clone()).unwrap_or_default();
        let code = msg["code"].as_str().unwrap_or("").to_string();
        let index = msg["now"]["index"].as_u64().unwrap_or(0) as usize;
        self.set(|v| {
            v.people = people;
            v.control = control;
            v.access = access;
            if !code.is_empty() {
                v.code = code;
            }
        });
        let rev = msg["rev"].as_u64();
        if let Some(q) = msg.get("queue").filter(|q| q.is_object()) {
            let offset = q["offset"].as_u64().unwrap_or(0) as usize;
            let tracks: Vec<Track> = q["tracks"].as_array().map(|l| l.iter().filter_map(|t| serde_json::from_value(t.clone()).ok()).collect()).unwrap_or_default();
            {
                let mut e = self.engine.lock().unwrap();
                e.offset = offset;
                e.queue_rev = q["rev"].as_u64();
                e.last_from_host = Some(Instant::now());
            }
            if let Some(by) = q["by"].as_object() {
                let by: Vec<(String, String)> = by.iter().filter_map(|(k, n)| Some((k.clone(), n.as_str()?.to_string()))).collect();
                self.set(|v| v.added_by.extend(by));
            }
            self.player.show_queue(tracks, index.saturating_sub(offset));
        }
        // the queue changed and its new part has not come (yet, or at all): ask for it
        let (queue_rev, last_resync) = {
            let e = self.engine.lock().unwrap();
            (e.queue_rev, e.last_resync)
        };
        if rev != queue_rev {
            if last_resync.is_none_or(|t| t.elapsed() > RESYNC) {
                self.engine.lock().unwrap().last_resync = Some(Instant::now());
                self.send(json!({ "t": "resync" }));
            }
            return;
        }
        // where the host is counts from when the message came, by this computer's own clock:
        // the clocks of two computers can differ by seconds
        let mut now = msg["now"].clone();
        now["at"] = (now_ms() - LATENCY_MS).into();
        self.follow(&now);
    }

    fn welcome(self: &Arc<Self>, host: &str) {
        let (queue, index, _, pos) = self.player.queue_state();
        {
            let mut e = self.engine.lock().unwrap();
            e.join_deadline = None;
            e.host_id = host.to_string();
            e.before = Some((queue, index, pos));
            e.last_from_host = Some(Instant::now());
            e.next_here = Some(Instant::now() + HERE);
        }
        self.set(|v| {
            v.status = JamStatus::Live;
            v.host = false;
            v.waiting = false;
        });
        let weak = Arc::downgrade(self);
        self.player.steer_with(Some(Arc::new(move |a| weak.upgrade().is_some_and(|j| j.steer_guest(a)))), true);
        toast(format!("{} {}", crate::i18n::t("Вы слушаете вместе с"), self.name_of(host)));
    }

    /// play what the host plays, where the host is; and nothing else
    fn follow(&self, now: &Value) {
        let (queue, _, status, pos) = self.player.queue_state();
        let offset = self.engine.lock().unwrap().offset;
        let host_index = now["index"].as_u64().unwrap_or(0) as usize;
        let local = host_index.wrapping_sub(offset);
        let track = queue.get(local).cloned();
        let p = self.player.clone();
        // nothing of the host's that can play here: this player stays quiet rather than going
        // on with something of its own
        let quiet = || {
            if status == Status::Playing {
                unsteered(|| p.toggle());
            }
        };
        let Some(track) = track.filter(|t| now["trackId"].as_str() == Some(t.id.as_str())) else { return quiet() };
        if track.source == Source::Local {
            let mut e = self.engine.lock().unwrap();
            if e.warned_local != track.id {
                e.warned_local = track.id.clone();
                toast(format!("У организатора играет файл с его компьютера: «{}». Здесь его не послушать.", track.title));
            }
            drop(e);
            return quiet();
        }
        let target = protocol::position_now(now, now_ms(), track.duration_ms);
        let host_playing = now["playing"].as_bool().unwrap_or(false);
        let playing_id = self.engine.lock().unwrap().playing_id.clone();
        if playing_id != track.id || matches!(status, Status::Idle | Status::Error) {
            self.engine.lock().unwrap().playing_id = track.id.clone();
            unsteered(|| {
                if !host_playing {
                    if status == Status::Playing {
                        p.toggle();
                    }
                    p.restore(crate::store::state::Session { queue: queue.clone(), index: local as i64, position_ms: target });
                    return;
                }
                p.play_index(local);
            });
            // The song takes a moment to start: catch up with the host when it does. Closer than
            // the page does it (it caught up only past 1.5 s): a quarter of a second is the most a
            // guest is let behind at the start; later drift is left alone below DRIFT_MS, as a
            // seek is heard and small drift is not.
            if host_playing {
                let p = self.player.clone();
                let now = now.clone();
                let dur = track.duration_ms;
                std::thread::spawn(move || {
                    for _ in 0..80 {
                        std::thread::sleep(Duration::from_millis(50));
                        if p.status() == Status::Playing {
                            let behind = protocol::position_now(&now, now_ms(), dur) + p.latency_ms();
                            if (behind - p.position_ms()).abs() > START_DRIFT_MS {
                                unsteered(|| p.seek(behind));
                            }
                            break;
                        }
                    }
                });
            }
            return;
        }
        if status == Status::Loading {
            return;
        }
        // this copy ended a moment before the host's: wait for the next song instead of starting over
        if self.player.at_end() {
            return;
        }
        let held = self.engine.lock().unwrap().seek_held_until.is_some_and(|t| Instant::now() < t);
        unsteered(|| {
            if host_playing != (status == Status::Playing) {
                p.toggle();
            }
            // right after a seek here, the host's older position is not a reason to jump back
            if !held && (pos - target).abs() > DRIFT_MS {
                p.seek(target);
            }
        });
    }

    // ---- both ------------------------------------------------------------------------------------

    fn receive(self: &Arc<Self>, msg: Value, from: String) {
        let v = self.view();
        if matches!(v.status, JamStatus::Idle | JamStatus::Starting) {
            return;
        }
        if v.host {
            self.host_receive(&msg, &from);
        } else {
            self.guest_receive(&msg, &from);
        }
    }

    fn show(&self, msg: &Value, from: &str) {
        let name = self.name_of(from);
        if msg["t"].as_str() == Some("chat") {
            let text: String = msg["text"].as_str().unwrap_or("").chars().take(200).collect();
            self.set(|v| {
                v.chat.push(Line { from: name, text, at: Instant::now() });
                if v.chat.len() > CHAT_KEPT {
                    v.chat.remove(0);
                }
            });
        } else {
            let emoji: String = msg["emoji"].as_str().unwrap_or("").chars().take(8).collect();
            self.set(|v| {
                v.reactions.push(Line { from: name, text: emoji, at: Instant::now() });
                if v.reactions.len() > REACTIONS_KEPT {
                    v.reactions.remove(0);
                }
            });
        }
    }

    pub fn send_chat(&self, text: &str) {
        let clean: String = text.trim().chars().take(200).collect();
        let v = self.view();
        if clean.is_empty() || v.status != JamStatus::Live {
            return;
        }
        let c = clean.clone();
        self.set(|x| {
            x.chat.push(Line { from: v.name.clone(), text: c, at: Instant::now() });
            if x.chat.len() > CHAT_KEPT {
                x.chat.remove(0);
            }
        });
        self.send(json!({ "t": "chat", "id": uid(), "text": clean }));
    }

    pub fn react(&self, emoji: &str) {
        let v = self.view();
        if v.status != JamStatus::Live {
            return;
        }
        let e = emoji.to_string();
        self.set(|x| {
            x.reactions.push(Line { from: v.name.clone(), text: e, at: Instant::now() });
            if x.reactions.len() > REACTIONS_KEPT {
                x.reactions.remove(0);
            }
        });
        self.send(json!({ "t": "react", "id": uid(), "emoji": emoji }));
    }
}
