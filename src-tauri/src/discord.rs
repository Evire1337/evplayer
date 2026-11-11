//! "Listening to …" in the Discord profile, with "Ask to join" on it. Talks to the Discord
//! desktop app over its local RPC pipe from one worker thread: it writes a command and reads
//! frames until the answer to that command comes. Events (a friend asks to join; this
//! person accepted an invite) can arrive in between and while nothing is being sent, so a
//! connected worker also looks at the pipe twice a second.

use std::io::{Read, Write};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceState {
    pub title: String,
    pub artist: String,
    pub artwork: Option<String>,
    pub url: Option<String>,
    pub started_at: Option<f64>,
    pub ends_at: Option<f64>,
    pub paused: Option<bool>,
    pub status_shows: Option<String>,
    /// where a paused song stands, shown in words (Discord can only show a running clock)
    pub position_ms: Option<f64>,
    pub duration_ms: Option<f64>,
    /// the session this person is in, shown as "2 of 16"
    pub party: Option<Party>,
    /// a page that takes a friend into that session (the "Слушать вместе" button)
    pub join_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Party {
    pub id: String,
    pub size: u32,
    pub max: u32,
    /// for Discord's own "Ask to join" (handed to the friend's app when let in); Discord
    /// shows no link buttons next to it, so the app leaves it out
    pub join: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct Status {
    pub status: &'static str,
    pub user: Option<DiscordUser>,
    pub error: Option<String>,
}

/// what Discord tells the app on its own
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiscordEvent {
    /// a friend pressed "Ask to join"
    Request { user: DiscordUser },
    /// this person was let into a friend's session: the friend's join secret
    Join { secret: String },
}

const OP_HANDSHAKE: i32 = 0;
const OP_FRAME: i32 = 1;
const OP_CLOSE: i32 = 2;

pub fn encode_frame(op: i32, payload: &Value) -> Vec<u8> {
    let json = serde_json::to_vec(payload).unwrap_or_default();
    let mut out = Vec::with_capacity(json.len() + 8);
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(json.len() as i32).to_le_bytes());
    out.extend_from_slice(&json);
    out
}

fn text(value: &str) -> String {
    if value.chars().count() < 2 {
        format!("{value}  ")
    } else {
        value.chars().take(127).collect()
    }
}

/// 83 000 ms -> "1:23"
fn clock(ms: f64) -> String {
    let s = (ms.max(0.0) / 1000.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// plain: without the newer status_display_type (older Discord builds); lone: no link
/// buttons next to a party (in case Discord does not take both)
pub fn activity_for(state: &PresenceState, plain: bool, lone: bool, now_ms: f64) -> Value {
    let paused = state.paused.unwrap_or(false);
    let artist = if state.artist.is_empty() { "EvPlayer" } else { &state.artist };
    // paused: no timestamps (a clock would run on), so where the song stands is said in words
    let line = match (paused, state.position_ms, state.duration_ms) {
        (true, Some(at), Some(of)) if of > 0.0 => format!("На паузе {} / {} · {artist}", clock(at), clock(of)),
        (true, ..) => format!("На паузе · {artist}"),
        _ => artist.to_string(),
    };
    let mut activity = json!({
        "type": 2,
        "details": text(&state.title),
        "state": text(&line),
        "assets": { "large_text": text(&state.title) },
    });
    if !plain {
        let shows = match state.status_shows.as_deref() {
            Some("artist") => 1,
            Some("app") => 0,
            _ => 2,
        };
        activity["status_display_type"] = json!(shows);
    }
    if paused {
        // Discord runs a clock of its own for an activity without one; a clock that ends at
        // this moment stands still at the paused position
        if let (Some(at), Some(_)) = (state.position_ms, state.started_at) {
            activity["timestamps"] = json!({ "start": (now_ms - at).round() as i64, "end": now_ms.round() as i64 });
        }
    } else if let Some(start) = state.started_at {
            // Discord rejects the whole activity if a timestamp is not a whole number
            let mut ts = json!({ "start": start.round() as i64 });
            if let Some(end) = state.ends_at {
                ts["end"] = json!(end.round() as i64);
            }
            activity["timestamps"] = ts;
    }
    if let Some(art) = state.artwork.as_deref().filter(|a| a.starts_with("https:")) {
        activity["assets"]["large_image"] = json!(art);
    }
    let mut secret = false;
    if let Some(party) = &state.party {
        let size = party.size.max(1);
        activity["party"] = json!({ "id": party.id, "size": [size, party.max.max(size)] });
        if let Some(join) = party.join.as_deref().filter(|j| !j.is_empty()) {
            activity["secrets"] = json!({ "join": join });
            secret = true;
        }
    }
    // up to two link buttons under the status (seen by others, not by the person themself)
    let https = |u: &Option<String>| u.clone().filter(|u| u.starts_with("https:") && u.len() <= 512);
    let mut buttons = Vec::new();
    if let Some(url) = https(&state.url) {
        buttons.push(json!({ "label": "Слушать в EvPlayer", "url": url }));
    }
    if let Some(url) = https(&state.join_url) {
        buttons.push(json!({ "label": "Слушать вместе", "url": url }));
    }
    if !buttons.is_empty() && !(secret && lone) {
        activity["buttons"] = json!(buttons);
    }
    activity
}

fn user_of(u: &Value) -> Option<DiscordUser> {
    let id = u["id"].as_str()?;
    Some(DiscordUser {
        id: id.to_string(),
        username: u["username"].as_str().unwrap_or("").to_string(),
        global_name: u["global_name"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        avatar: u["avatar"].as_str().map(|a| {
            let ext = if a.starts_with("a_") { "gif" } else { "png" };
            format!("https://cdn.discordapp.com/avatars/{id}/{a}.{ext}?size=128")
        }),
    })
}

pub fn event_of(data: &Value) -> Option<DiscordEvent> {
    if data["cmd"] != "DISPATCH" {
        return None;
    }
    match data["evt"].as_str()? {
        "ACTIVITY_JOIN_REQUEST" => Some(DiscordEvent::Request { user: user_of(&data["data"]["user"])? }),
        "ACTIVITY_JOIN" => Some(DiscordEvent::Join { secret: data["data"]["secret"].as_str()?.to_string() }),
        _ => None,
    }
}

enum Msg {
    AppId(Option<String>),
    Update(Option<PresenceState>),
    /// let a friend in (true) or turn them down
    Answer(String, bool),
    Quit,
}

trait Pipe: Read + Write + Send {
    /// whether something waits to be read (without waiting for it)
    fn pending(&mut self) -> bool;
}

#[cfg(windows)]
impl Pipe for std::fs::File {
    fn pending(&mut self) -> bool {
        use std::os::windows::io::AsRawHandle;
        let mut available = 0u32;
        // SAFETY: a valid pipe handle owned by this file; no buffer is read into
        let ok = unsafe {
            windows_sys::Win32::System::Pipes::PeekNamedPipe(
                self.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        ok != 0 && available > 0
    }
}

#[cfg(unix)]
impl Pipe for std::os::unix::net::UnixStream {
    fn pending(&mut self) -> bool {
        let mut byte = [0u8; 1];
        if self.set_nonblocking(true).is_err() {
            return false;
        }
        let seen = self.peek(&mut byte);
        let _ = self.set_nonblocking(false);
        matches!(seen, Ok(n) if n > 0)
    }
}

fn open_pipe() -> Option<Box<dyn Pipe>> {
    for i in 0..10 {
        #[cfg(windows)]
        {
            let path = format!(r"\\?\pipe\discord-ipc-{i}");
            if let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(path) {
                return Some(Box::new(f));
            }
        }
        #[cfg(unix)]
        {
            let base = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
                .iter()
                .find_map(|k| std::env::var(k).ok())
                .unwrap_or_else(|| "/tmp".into());
            for dir in [base.clone(), format!("{base}/app/com.discordapp.Discord"), format!("{base}/snap.discord")] {
                if let Ok(s) = std::os::unix::net::UnixStream::connect(format!("{dir}/discord-ipc-{i}")) {
                    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
                    return Some(Box::new(s));
                }
            }
        }
    }
    None
}

fn read_frame(pipe: &mut dyn Pipe) -> Option<(i32, Value)> {
    let mut header = [0u8; 8];
    pipe.read_exact(&mut header).ok()?;
    let op = i32::from_le_bytes(header[0..4].try_into().ok()?);
    let len = i32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
    if len > 1 << 20 {
        return None;
    }
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body).ok()?;
    Some((op, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

type Listener = Arc<Mutex<Option<Box<dyn Fn(DiscordEvent) + Send>>>>;

/// the open pipe: commands and their answers, with events handed on as they come
struct Conn {
    pipe: Box<dyn Pipe>,
    nonce: u64,
    listener: Listener,
}

impl Conn {
    fn hand_on(&self, data: &Value) {
        if let Some(event) = event_of(data) {
            if let Some(listen) = self.listener.lock().unwrap().as_ref() {
                listen(event);
            }
        }
    }

    /// send a command and wait for its answer (None: the pipe is gone)
    fn call(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> Option<Value> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        let mut body = json!({ "cmd": cmd, "args": args, "nonce": nonce });
        if let Some(evt) = evt {
            body["evt"] = json!(evt);
        }
        self.pipe.write_all(&encode_frame(OP_FRAME, &body)).ok()?;
        loop {
            let (op, data) = read_frame(self.pipe.as_mut())?;
            if op == OP_CLOSE {
                return None;
            }
            // Discord echoes the nonce; some builds leave it out of the answer
            if data["nonce"].as_str() == Some(nonce.as_str()) || (data["cmd"] == cmd && data["nonce"].is_null()) {
                return Some(data);
            }
            self.hand_on(&data);
        }
    }

    /// read whatever came in on its own; false when the pipe closed
    fn drain(&mut self) -> bool {
        while self.pipe.pending() {
            match read_frame(self.pipe.as_mut()) {
                Some((OP_FRAME, data)) => self.hand_on(&data),
                _ => return false,
            }
        }
        true
    }
}

pub struct Discord {
    tx: Sender<Msg>,
    status: Arc<Mutex<Status>>,
    listener: Listener,
}

impl Discord {
    pub fn new() -> Self {
        let (tx, rx) = channel::<Msg>();
        let status = Arc::new(Mutex::new(Status { status: "off", user: None, error: None }));
        let listener: Listener = Arc::new(Mutex::new(None));
        let shared = status.clone();
        let events = listener.clone();
        std::thread::spawn(move || {
            let mut app_id: Option<String> = None;
            let mut state: Option<PresenceState> = None;
            let mut conn: Option<Conn> = None;
            let mut plain = false;
            let mut lone = false;
            let mut was_paused = false;
            // the activity has to be sent again (new state, or just connected)
            let mut dirty = false;
            let mut answers: Vec<(String, bool)> = Vec::new();
            let set = |s: &'static str, user: Option<DiscordUser>, error: Option<String>| {
                *shared.lock().unwrap() = Status { status: s, user, error };
            };
            let lost = |conn: &mut Option<Conn>| {
                *conn = None;
                let mut st = shared.lock().unwrap();
                if st.status != "error" {
                    *st = Status { status: "no-discord", user: None, error: None };
                }
            };
            loop {
                // connect when needed (Discord may be started later: try again every 15 s)
                if conn.is_none() {
                    if let Some(id) = &app_id {
                        set("connecting", None, None);
                        match open_pipe() {
                            None => set("no-discord", None, None),
                            Some(mut p) => {
                                let ok = p.write_all(&encode_frame(OP_HANDSHAKE, &json!({ "v": 1, "client_id": id }))).is_ok();
                                match ok.then(|| read_frame(p.as_mut())).flatten() {
                                    Some((OP_FRAME, data)) if data["evt"] == "READY" => {
                                        let mut c = Conn { pipe: p, nonce: 0, listener: events.clone() };
                                        // "Ask to join": the requests, and the invites this person accepts
                                        let subscribed = c.call("SUBSCRIBE", json!({}), Some("ACTIVITY_JOIN_REQUEST")).is_some()
                                            && c.call("SUBSCRIBE", json!({}), Some("ACTIVITY_JOIN")).is_some();
                                        if subscribed {
                                            set("connected", user_of(&data["data"]["user"]), None);
                                            conn = Some(c);
                                            dirty = true;
                                            was_paused = false;
                                        } else {
                                            set("no-discord", None, None);
                                        }
                                    }
                                    Some((OP_CLOSE, data)) => {
                                        let msg = data["message"].as_str().unwrap_or("Discord закрыл соединение").to_string();
                                        set("error", None, Some(msg));
                                    }
                                    _ => set("no-discord", None, None),
                                }
                            }
                        }
                    }
                }
                if let Some(c) = conn.as_mut() {
                    if !c.drain() {
                        lost(&mut conn);
                        continue;
                    }
                }
                // friends let in or turned down
                if let Some(c) = conn.as_mut() {
                    for (user_id, accept) in answers.drain(..) {
                        let cmd = if accept { "SEND_ACTIVITY_JOIN_INVITE" } else { "CLOSE_ACTIVITY_REQUEST" };
                        if c.call(cmd, json!({ "user_id": user_id }), None).is_none() {
                            break;
                        }
                    }
                }
                // send the current activity
                if dirty {
                    if let Some(c) = conn.as_mut() {
                        dirty = false;
                        // just paused: drop the playing activity first, so Discord starts the paused
                        // one fresh instead of carrying on the playing one's clock
                        let now_paused = state.as_ref().is_some_and(|s| s.paused.unwrap_or(false));
                        let pid = std::process::id();
                        if now_paused && !was_paused && c.call("SET_ACTIVITY", json!({ "pid": pid, "activity": null }), None).is_none() {
                            lost(&mut conn);
                            continue;
                        }
                        was_paused = now_paused;
                        let mut resend = true;
                        while resend {
                            resend = false;
                            let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0);
                            let activity = state.as_ref().map(|s| activity_for(s, plain, lone, now_ms));
                            match c.call("SET_ACTIVITY", json!({ "pid": pid, "activity": activity }), None) {
                                None => {
                                    lost(&mut conn);
                                    break;
                                }
                                Some(data) if data["evt"] == "ERROR" => {
                                    let msg = data["data"]["message"].as_str().unwrap_or("Discord не принял статус").to_string();
                                    // older Discord builds reject the newer "status_display_type" field
                                    let lower = msg.to_lowercase();
                                    if !plain && lower.contains("status_display_type") {
                                        plain = true;
                                        resend = true;
                                    } else if !lone && lower.contains("button") {
                                        // link buttons are not taken next to "Ask to join": that one wins
                                        lone = true;
                                        resend = true;
                                    } else {
                                        shared.lock().unwrap().error = Some(msg);
                                    }
                                }
                                Some(_) => shared.lock().unwrap().error = None,
                            }
                        }
                    }
                }
                let wait = if conn.is_some() {
                    Duration::from_millis(500)
                } else if app_id.is_some() {
                    Duration::from_secs(15)
                } else {
                    Duration::from_secs(3600)
                };
                let mut handle = |m: Msg| -> bool {
                    match m {
                        Msg::AppId(id) => {
                            if id != app_id {
                                app_id = id;
                                conn = None;
                                if app_id.is_none() {
                                    set("off", None, None);
                                }
                            }
                        }
                        Msg::Update(s) => {
                            state = s;
                            dirty = true;
                        }
                        Msg::Answer(user, accept) => answers.push((user, accept)),
                        Msg::Quit => return false,
                    }
                    true
                };
                match rx.recv_timeout(wait) {
                    Ok(m) => {
                        if !handle(m) {
                            return;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                // several updates in a row: only the last one matters
                while let Ok(m) = rx.try_recv() {
                    if !handle(m) {
                        return;
                    }
                }
            }
        });
        Self { tx, status, listener }
    }

    /// who gets the events (the page)
    pub fn on_event(&self, listen: impl Fn(DiscordEvent) + Send + 'static) {
        *self.listener.lock().unwrap() = Some(Box::new(listen));
    }

    pub fn set_app_id(&self, id: Option<String>) {
        let clean = id.map(|s| s.trim().to_string()).filter(|s| s.len() >= 15 && s.len() <= 22 && s.chars().all(|c| c.is_ascii_digit()));
        let _ = self.tx.send(Msg::AppId(clean));
    }

    pub fn update(&self, state: Option<PresenceState>) {
        let _ = self.tx.send(Msg::Update(state));
    }

    /// a friend's "Ask to join": let them in, or not
    pub fn answer(&self, user_id: String, accept: bool) {
        if !user_id.is_empty() && user_id.chars().all(|c| c.is_ascii_digit()) {
            let _ = self.tx.send(Msg::Answer(user_id, accept));
        }
    }

    pub fn status(&self) -> Status {
        let st = self.status.lock().unwrap().clone();
        Status { user: if st.status == "connected" { st.user } else { None }, ..st }
    }

    pub fn quit(&self) {
        let _ = self.tx.send(Msg::Quit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song() -> PresenceState {
        PresenceState {
            title: "A".into(),
            artist: "B".into(),
            artwork: Some("https://i1.sndcdn.com/x.jpg".into()),
            url: Some("https://soundcloud.com/x".into()),
            started_at: Some(1000.4),
            ends_at: Some(2000.6),
            paused: Some(false),
            status_shows: Some("artist".into()),
            position_ms: Some(83_000.0),
            duration_ms: Some(200_000.0),
            party: None,
            join_url: None,
        }
    }

    #[test]
    fn frame_layout() {
        let f = encode_frame(1, &json!({ "a": 1 }));
        assert_eq!(&f[0..4], &1i32.to_le_bytes());
        assert_eq!(&f[4..8], &7i32.to_le_bytes());
        assert_eq!(&f[8..], br#"{"a":1}"#);
    }

    #[test]
    fn activity_rounds_times_and_pads_short_text() {
        let s = song();
        let a = activity_for(&s, false, false, 0.0);
        assert_eq!(a["details"], "A  ");
        assert_eq!(a["timestamps"]["start"], 1000);
        assert_eq!(a["timestamps"]["end"], 2001);
        assert_eq!(a["status_display_type"], 1);
        assert!(activity_for(&s, true, false, 0.0).get("status_display_type").is_none());
    }

    #[test]
    fn paused_clock_stands_still_and_says_where_it_stands() {
        let a = activity_for(&PresenceState { paused: Some(true), ..song() }, false, false, 1_000_000.0);
        // a clock that has already ended: Discord shows it stopped at the paused position
        assert_eq!(a["timestamps"]["start"], 1_000_000 - 83_000);
        assert_eq!(a["timestamps"]["end"], 1_000_000);
        assert_eq!(a["state"], "На паузе 1:23 / 3:20 · B");
        let no_timeline = PresenceState { paused: Some(true), started_at: None, ..song() };
        assert!(activity_for(&no_timeline, false, false, 1_000_000.0).get("timestamps").is_none());
    }

    #[test]
    fn two_link_buttons_and_the_party_size() {
        let party = Party { id: "evp:JAM-7K9P-2QXZ".into(), size: 2, max: 16, join: None };
        let s = PresenceState { party: Some(party), join_url: Some("https://evire1337.github.io/evplayer/join/?code=JAM-7K9P-2QXZ".into()), ..song() };
        let a = activity_for(&s, false, false, 0.0);
        assert_eq!(a["party"]["size"], json!([2, 16]));
        assert!(a.get("secrets").is_none());
        assert_eq!(a["buttons"][0]["label"], "Слушать в EvPlayer");
        assert_eq!(a["buttons"][1]["label"], "Слушать вместе");
        // with Discord's own "Ask to join" the buttons go only if Discord turns them down
        let asked = PresenceState { party: Some(Party { id: "p".into(), size: 1, max: 16, join: Some("JAM-7K9P-2QXZ~pass".into()) }), ..s.clone() };
        assert_eq!(activity_for(&asked, false, false, 0.0)["secrets"]["join"], "JAM-7K9P-2QXZ~pass");
        assert!(activity_for(&asked, false, true, 0.0).get("buttons").is_none());
        assert_eq!(activity_for(&song(), false, true, 0.0)["buttons"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn reads_join_events() {
        let request = json!({ "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST", "data": { "user": { "id": "42", "username": "friend" } } });
        assert!(matches!(event_of(&request), Some(DiscordEvent::Request { user }) if user.id == "42"));
        let join = json!({ "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN", "data": { "secret": "JAM-7K9P2Q" } });
        assert_eq!(event_of(&join), Some(DiscordEvent::Join { secret: "JAM-7K9P2Q".into() }));
        assert_eq!(event_of(&json!({ "cmd": "SET_ACTIVITY" })), None);
    }
}

#[cfg(all(test, unix))]
mod pipe_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};

    fn read(stream: &mut UnixStream) -> (i32, Value) {
        let mut h = [0u8; 8];
        stream.read_exact(&mut h).unwrap();
        let len = i32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).unwrap();
        (i32::from_le_bytes(h[0..4].try_into().unwrap()), serde_json::from_slice(&body).unwrap())
    }

    /// answer a command the way Discord does: with its nonce
    fn reply(stream: &mut UnixStream, to: &Value) {
        stream.write_all(&encode_frame(OP_FRAME, &json!({ "cmd": to["cmd"], "nonce": to["nonce"], "data": {} }))).unwrap();
    }

    /// a pretend Discord: subscriptions, the paused state without a clock, a join request
    #[test]
    fn talks_to_discord() {
        let dir = std::env::temp_dir().join(format!("evplayer-discord-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
        let listener = UnixListener::bind(dir.join("discord-ipc-0")).unwrap();
        let discord = Discord::new();
        let (got, events) = std::sync::mpsc::channel();
        discord.on_event(move |e| {
            let _ = got.send(e);
        });
        discord.set_app_id(Some("1564485226267279360".into()));
        let (mut s, _) = listener.accept().unwrap();
        let (op, hello) = read(&mut s);
        assert_eq!(op, OP_HANDSHAKE);
        assert_eq!(hello["client_id"], "1564485226267279360");
        s.write_all(&encode_frame(OP_FRAME, &json!({ "evt": "READY", "data": { "user": { "id": "1", "username": "u" } } }))).unwrap();
        for evt in ["ACTIVITY_JOIN_REQUEST", "ACTIVITY_JOIN"] {
            let (_, sub) = read(&mut s);
            assert_eq!(sub["cmd"], "SUBSCRIBE");
            assert_eq!(sub["evt"], evt);
            reply(&mut s, &sub);
        }
        // first activity after READY (nothing playing yet)
        let (_, first) = read(&mut s);
        assert_eq!(first["cmd"], "SET_ACTIVITY");
        reply(&mut s, &first);
        let playing = PresenceState {
            title: "Song".into(),
            artist: "Artist".into(),
            artwork: None,
            url: None,
            started_at: Some(1000.0),
            ends_at: Some(5000.0),
            paused: Some(false),
            status_shows: None,
            position_ms: Some(1000.0),
            duration_ms: Some(4000.0),
            party: None,
            join_url: None,
        };
        discord.update(Some(playing.clone()));
        let (_, a) = read(&mut s);
        assert!(a["args"]["activity"]["timestamps"].is_object());
        reply(&mut s, &a);
        discord.update(Some(PresenceState { paused: Some(true), ..playing }));
        let (_, cleared) = read(&mut s);
        assert!(cleared["args"]["activity"].is_null());
        reply(&mut s, &cleared);
        let (_, b) = read(&mut s);
        let act = &b["args"]["activity"];
        let ts = &act["timestamps"];
        assert_eq!(ts["end"].as_i64().unwrap() - ts["start"].as_i64().unwrap(), 1000, "{act}");
        assert!(act["state"].as_str().unwrap().contains("паузе"), "{act}");
        reply(&mut s, &b);
        // a friend asks to join while nothing is being sent; the answer goes back
        s.write_all(&encode_frame(OP_FRAME, &json!({ "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST", "data": { "user": { "id": "42", "username": "friend" } } }))).unwrap();
        let event = events.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(event, DiscordEvent::Request { ref user } if user.id == "42"));
        discord.answer("42".into(), true);
        let (_, invite) = read(&mut s);
        assert_eq!(invite["cmd"], "SEND_ACTIVITY_JOIN_INVITE");
        assert_eq!(invite["args"]["user_id"], "42");
        reply(&mut s, &invite);
        discord.quit();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
