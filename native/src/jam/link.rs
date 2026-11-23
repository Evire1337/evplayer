//! The wire of a session (src/jam/link.ts): public Nostr relays. Every message is a short-lived
//! event (the relays pass it on and forget it), tagged with a value made from the session code
//! and encrypted with a key made from it too. Each copy of the program signs with its own key,
//! so who sent a message is known for certain; an event whose signature does not hold is
//! dropped.
//!
//! One thread per relay, only while a session (or the public list) is open: it connects,
//! listens on the session's tag, sends what is given to it, and connects again when the
//! connection drops. Nothing of this runs when no session is on.

use std::collections::{HashSet, VecDeque};
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::crypto::{self, Keys};

/// ephemeral events (20000–29999) are passed on and not stored
pub const KIND: u32 = 20_001;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// how long a relay's thread waits for a message before it sends what is queued
const POLL: Duration = Duration::from_millis(40);
const PING_EVERY: Duration = Duration::from_secs(30);
const SEEN_LIMIT: usize = 500;

/// public relays the session talks through (the same list as src/jam/types.ts)
pub const RELAYS: [&str; 5] = ["wss://relay.primal.net", "wss://relay.snort.social", "wss://relay.damus.io", "wss://nostr.oxtr.dev", "wss://purplerelay.com"];

pub type OnMessage = Box<dyn Fn(Value, String) + Send + Sync>;

struct Shared {
    keys: Keys,
    /// the session's tag and key
    room: Mutex<Option<(String, [u8; 32])>>,
    /// the subscription sent to a relay when it (re)connects
    req: Mutex<Option<String>>,
    seen: Mutex<(HashSet<String>, VecDeque<String>)>,
    on_message: OnMessage,
    connected: AtomicUsize,
    stop: AtomicBool,
}

pub struct Link {
    shared: Arc<Shared>,
    outs: Vec<Sender<String>>,
}

fn now_s() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Link {
    /// connect to the relays; an error when none answers
    pub fn open(on_message: OnMessage) -> Result<Self, String> {
        let shared = Arc::new(Shared { keys: Keys::generate(), room: Mutex::new(None), req: Mutex::new(None), seen: Mutex::new((HashSet::new(), VecDeque::new())), on_message, connected: AtomicUsize::new(0), stop: AtomicBool::new(false) });
        let mut outs = Vec::new();
        let tried = Arc::new(AtomicUsize::new(0));
        for url in RELAYS {
            let (tx, rx) = channel::<String>();
            outs.push(tx);
            let sh = shared.clone();
            let tried = tried.clone();
            let _ = std::thread::Builder::new().name("jam-relay".into()).spawn(move || relay(url, sh, rx, tried));
        }
        // wait until one relay is in, or all have tried
        let start = Instant::now();
        while shared.connected.load(Ordering::Relaxed) == 0 && tried.load(Ordering::Relaxed) < RELAYS.len() && start.elapsed() < CONNECT_TIMEOUT + Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(30));
        }
        let me = Self { shared, outs };
        if me.shared.connected.load(Ordering::Relaxed) == 0 {
            me.close();
            return Err("Не удалось подключиться к серверам сессий. Проверьте интернет.".into());
        }
        Ok(me)
    }

    /// this copy's key in the session
    pub fn id(&self) -> String {
        self.shared.keys.id.clone()
    }

    /// listen on the session with this code (and no longer on the last one)
    pub fn join(&self, code: &str) {
        let (tag, key) = crypto::room_secrets(code);
        *self.shared.room.lock().unwrap() = Some((tag.clone(), key));
        let since = now_s().saturating_sub(5);
        let req = json!(["REQ", "jam", { "kinds": [KIND], "#t": [tag], "since": since }]).to_string();
        *self.shared.req.lock().unwrap() = Some(req.clone());
        for o in &self.outs {
            let _ = o.send(json!(["CLOSE", "jam"]).to_string());
            let _ = o.send(req.clone());
        }
    }

    /// say something to the session (signed, encrypted, to every relay)
    pub fn send(&self, msg: &Value) {
        let Some((tag, key)) = self.shared.room.lock().unwrap().clone() else { return };
        let Ok(content) = crypto::encrypt(&msg.to_string(), &key) else { return };
        let event = self.shared.keys.event(KIND, now_s(), json!([["t", tag]]), &content);
        if let Some(id) = event["id"].as_str() {
            remember(&self.shared, id);
        }
        let text = json!(["EVENT", event]).to_string();
        for o in &self.outs {
            let _ = o.send(text.clone());
        }
    }

    /// text only `to` can read (the new code, when the session moves)
    pub fn seal(&self, text: &str, to: &str) -> Option<String> {
        crypto::encrypt(text, &self.shared.keys.conversation(to)?).ok()
    }

    pub fn unseal(&self, sealed: &str, from: &str) -> Option<String> {
        crypto::decrypt(sealed, &self.shared.keys.conversation(from)?)
    }

    pub fn close(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.close();
    }
}

/// the event was seen (it comes from every relay once); true when it is new
fn remember(sh: &Shared, id: &str) -> bool {
    let mut g = sh.seen.lock().unwrap();
    if !g.0.insert(id.to_string()) {
        return false;
    }
    g.1.push_back(id.to_string());
    if g.1.len() > SEEN_LIMIT {
        if let Some(old) = g.1.pop_front() {
            g.0.remove(&old);
        }
    }
    true
}

fn receive(sh: &Shared, text: &str) {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return };
    if v[0].as_str() != Some("EVENT") {
        return;
    }
    let e = &v[2];
    let (Some(id), Some(from), Some(content)) = (e["id"].as_str(), e["pubkey"].as_str(), e["content"].as_str()) else { return };
    if from == sh.keys.id || e["kind"].as_u64() != Some(KIND as u64) {
        return;
    }
    let Some((tag, key)) = sh.room.lock().unwrap().clone() else { return };
    let tagged = e["tags"].as_array().is_some_and(|t| t.iter().any(|x| x[0].as_str() == Some("t") && x[1].as_str() == Some(tag.as_str())));
    if !tagged || !remember(sh, id) {
        return;
    }
    if !crypto::verify(e) {
        return;
    }
    // not from this session (or the session moved since)
    let Some(plain) = crypto::decrypt(content, &key) else { return };
    let Ok(msg) = serde_json::from_str::<Value>(&plain) else { return };
    if super::protocol::is_message(&msg) {
        (sh.on_message)(msg, from.to_string());
    }
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

fn connect(url: &str) -> Result<Socket, String> {
    let u = url::Url::parse(url).map_err(|e| e.to_string())?;
    let host = u.host_str().ok_or("no host")?;
    let addr = (host, u.port_or_known_default().unwrap_or(443)).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let tcp = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(CONNECT_TIMEOUT)).ok();
    tcp.set_write_timeout(Some(CONNECT_TIMEOUT)).ok();
    tcp.set_nodelay(true).ok();
    let (mut ws, _) = tungstenite::client_tls_with_config(url, tcp, None, None).map_err(|e| e.to_string())?;
    let sock = match ws.get_mut() {
        MaybeTlsStream::NativeTls(t) => t.get_mut(),
        MaybeTlsStream::Plain(t) => t,
        _ => return Err("unknown stream".into()),
    };
    sock.set_read_timeout(Some(POLL)).map_err(|e| e.to_string())?;
    Ok(ws)
}

fn relay(url: &'static str, sh: Arc<Shared>, rx: Receiver<String>, tried: Arc<AtomicUsize>) {
    let mut backoff = Duration::from_secs(2);
    let mut first = true;
    while !sh.stop.load(Ordering::Relaxed) {
        let ws = connect(url);
        if first {
            first = false;
            tried.fetch_add(1, Ordering::Relaxed);
        }
        let mut ws = match ws {
            Ok(ws) => ws,
            Err(_) => {
                // what was meant for this relay meanwhile is dropped: the others carry it
                while rx.try_recv().is_ok() {}
                let until = Instant::now() + backoff;
                while Instant::now() < until && !sh.stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(100));
                }
                backoff = (backoff * 2).min(Duration::from_secs(30));
                continue;
            }
        };
        backoff = Duration::from_secs(2);
        sh.connected.fetch_add(1, Ordering::Relaxed);
        if let Some(req) = sh.req.lock().unwrap().clone() {
            let _ = ws.send(Message::Text(req.into()));
        }
        let mut last_ping = Instant::now();
        let alive = loop {
            if sh.stop.load(Ordering::Relaxed) {
                break false;
            }
            let mut broken = false;
            while let Ok(out) = rx.try_recv() {
                if ws.send(Message::Text(out.into())).is_err() {
                    broken = true;
                    break;
                }
            }
            if broken {
                break true;
            }
            if last_ping.elapsed() > PING_EVERY {
                last_ping = Instant::now();
                if ws.send(Message::Ping(Vec::new().into())).is_err() {
                    break true;
                }
            }
            match ws.read() {
                Ok(Message::Text(t)) => receive(&sh, t.as_str()),
                Ok(Message::Close(_)) => break true,
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(_) => break true,
            }
        };
        sh.connected.fetch_sub(1, Ordering::Relaxed);
        if !alive {
            let _ = ws.close(None);
            let _ = ws.flush();
            break;
        }
    }
}
