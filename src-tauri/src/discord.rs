//! "Listening to …" in the Discord profile. Talks to the Discord desktop app over its local
//! RPC pipe. Discord only answers what it is asked (no subscriptions), so a single worker
//! thread writes a command and reads the answer; nothing else touches the pipe.

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
}

#[derive(Clone, Debug, Serialize)]
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

pub fn activity_for(state: &PresenceState, plain: bool) -> Value {
    let mut activity = json!({
        "type": 2,
        "details": text(&state.title),
        "state": text(if state.artist.is_empty() { "EvPlayer" } else { &state.artist }),
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
    if !state.paused.unwrap_or(false) {
        if let Some(start) = state.started_at {
            // Discord rejects the whole activity if a timestamp is not a whole number
            let mut ts = json!({ "start": start.round() as i64 });
            if let Some(end) = state.ends_at {
                ts["end"] = json!(end.round() as i64);
            }
            activity["timestamps"] = ts;
        }
    }
    if let Some(art) = state.artwork.as_deref().filter(|a| a.starts_with("https:")) {
        activity["assets"]["large_image"] = json!(art);
    }
    if let Some(url) = state.url.as_deref().filter(|u| u.starts_with("https:")) {
        activity["buttons"] = json!([{ "label": "Слушать", "url": url }]);
    }
    activity
}

enum Msg {
    AppId(Option<String>),
    Update(Option<PresenceState>),
    Quit,
}

trait Pipe: Read + Write + Send {}
impl<T: Read + Write + Send> Pipe for T {}

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

pub struct Discord {
    tx: Sender<Msg>,
    status: Arc<Mutex<Status>>,
}

impl Discord {
    pub fn new() -> Self {
        let (tx, rx) = channel::<Msg>();
        let status = Arc::new(Mutex::new(Status { status: "off", user: None, error: None }));
        let shared = status.clone();
        std::thread::spawn(move || {
            let mut app_id: Option<String> = None;
            let mut state: Option<PresenceState> = None;
            let mut pipe: Option<Box<dyn Pipe>> = None;
            let mut plain = false;
            let mut nonce = 0u64;
            let set = |s: &'static str, user: Option<DiscordUser>, error: Option<String>| {
                *shared.lock().unwrap() = Status { status: s, user, error };
            };
            loop {
                // connect when needed (Discord may be started later: try again every 15 s)
                if pipe.is_none() {
                    if let Some(id) = &app_id {
                        set("connecting", None, None);
                        match open_pipe() {
                            None => set("no-discord", None, None),
                            Some(mut p) => {
                                let ok = p.write_all(&encode_frame(OP_HANDSHAKE, &json!({ "v": 1, "client_id": id }))).is_ok();
                                match ok.then(|| read_frame(p.as_mut())).flatten() {
                                    Some((OP_FRAME, data)) if data["evt"] == "READY" => {
                                        let u = &data["data"]["user"];
                                        let user = u["id"].as_str().map(|uid| DiscordUser {
                                            id: uid.to_string(),
                                            username: u["username"].as_str().unwrap_or("").to_string(),
                                            global_name: u["global_name"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
                                            avatar: u["avatar"].as_str().map(|a| {
                                                let ext = if a.starts_with("a_") { "gif" } else { "png" };
                                                format!("https://cdn.discordapp.com/avatars/{uid}/{a}.{ext}?size=128")
                                            }),
                                        });
                                        set("connected", user, None);
                                        pipe = Some(p);
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
                // send the current activity
                if let Some(p) = pipe.as_mut() {
                    let mut resend = true;
                    while resend {
                        resend = false;
                        nonce += 1;
                        let activity = state.as_ref().map(|s| activity_for(s, plain));
                        let frame = encode_frame(OP_FRAME, &json!({
                            "cmd": "SET_ACTIVITY",
                            "args": { "pid": std::process::id(), "activity": activity },
                            "nonce": nonce.to_string(),
                        }));
                        let reply = if p.write_all(&frame).is_ok() { read_frame(p.as_mut()) } else { None };
                        match reply {
                            None => {
                                pipe = None;
                                let mut st = shared.lock().unwrap();
                                if st.status != "error" {
                                    *st = Status { status: "no-discord", user: None, error: None };
                                }
                                break;
                            }
                            Some((_, data)) if data["evt"] == "ERROR" => {
                                let msg = data["data"]["message"].as_str().unwrap_or("Discord не принял статус").to_string();
                                // older Discord builds reject the newer "status_display_type" field
                                if !plain && msg.to_lowercase().contains("status_display_type") {
                                    plain = true;
                                    resend = true;
                                } else {
                                    shared.lock().unwrap().error = Some(msg);
                                }
                            }
                            Some(_) => shared.lock().unwrap().error = None,
                        }
                    }
                }
                let wait = if pipe.is_none() && app_id.is_some() { Duration::from_secs(15) } else { Duration::from_secs(3600) };
                match rx.recv_timeout(wait) {
                    Ok(Msg::AppId(id)) => {
                        if id != app_id {
                            app_id = id;
                            pipe = None;
                            if app_id.is_none() {
                                set("off", None, None);
                            }
                        }
                    }
                    Ok(Msg::Update(s)) => state = s,
                    Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                // several updates in a row: only the last one matters
                while let Ok(m) = rx.try_recv() {
                    match m {
                        Msg::AppId(id) if id != app_id => {
                            app_id = id;
                            pipe = None;
                        }
                        Msg::Update(s) => state = s,
                        Msg::Quit => return,
                        _ => {}
                    }
                }
            }
        });
        Self { tx, status }
    }

    pub fn set_app_id(&self, id: Option<String>) {
        let clean = id.map(|s| s.trim().to_string()).filter(|s| s.len() >= 15 && s.len() <= 22 && s.chars().all(|c| c.is_ascii_digit()));
        let _ = self.tx.send(Msg::AppId(clean));
    }

    pub fn update(&self, state: Option<PresenceState>) {
        let _ = self.tx.send(Msg::Update(state));
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

    #[test]
    fn frame_layout() {
        let f = encode_frame(1, &json!({ "a": 1 }));
        assert_eq!(&f[0..4], &1i32.to_le_bytes());
        assert_eq!(&f[4..8], &7i32.to_le_bytes());
        assert_eq!(&f[8..], br#"{"a":1}"#);
    }

    #[test]
    fn activity_rounds_times_and_pads_short_text() {
        let s = PresenceState {
            title: "A".into(),
            artist: "B".into(),
            artwork: Some("https://i1.sndcdn.com/x.jpg".into()),
            url: Some("https://soundcloud.com/x".into()),
            started_at: Some(1000.4),
            ends_at: Some(2000.6),
            paused: Some(false),
            status_shows: Some("artist".into()),
        };
        let a = activity_for(&s, false);
        assert_eq!(a["details"], "A  ");
        assert_eq!(a["timestamps"]["start"], 1000);
        assert_eq!(a["timestamps"]["end"], 2001);
        assert_eq!(a["status_display_type"], 1);
        assert!(activity_for(&s, true).get("status_display_type").is_none());
        let paused = activity_for(&PresenceState { paused: Some(true), ..s }, false);
        assert!(paused.get("timestamps").is_none());
    }
}
