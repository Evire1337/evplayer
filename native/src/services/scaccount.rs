//! Your SoundCloud account (src/services/soundcloud/account.ts, oauth.ts and mirror.ts).
//!
//! Connected by profile link, EvPlayer reads the public side of it (likes, playlists) into
//! your library; no password is involved. Signed in with SoundCloud's own OAuth (an app of
//! your own), "full sync" mirrors what you like and the playlists you make here to the
//! account, and removes there what you remove here.
//!
//! The mirror works by comparison, not by recording clicks: evplayer.scmirror.v1 remembers
//! what was last written to SoundCloud (the baseline, in the Tauri build's shape, so the two
//! builds continue each other's work); every pass compares the library with it and sends
//! only the difference. A pass that stops halfway (offline, rate limit) continues from where
//! the baseline says it got to. Playlists imported from SoundCloud are left alone, and so is
//! anything on SoundCloud that EvPlayer did not create.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::model::Track;
use crate::services::soundcloud;
use crate::store::state::{LikedTrack as Like, Playlist};

const API: &str = "https://api.soundcloud.com";
const MIRROR_KEY: &str = "evplayer.scmirror.v1";

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ---- the public profile ---------------------------------------------------------------------

/// "soundcloudAccount" in the settings, as the Tauri build keeps it
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default)]
    pub permalink: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub followers: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub likes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracks: Option<f64>,
    #[serde(default)]
    pub sync_likes: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<String>,
}

pub fn account() -> Option<Account> {
    crate::state::get().extra("soundcloudAccount").and_then(|v| serde_json::from_value(v).ok())
}

pub fn save_account(a: Option<&Account>) {
    crate::state::get().set_extra("soundcloudAccount", a.and_then(|a| serde_json::to_value(a).ok()).unwrap_or(Value::Null));
}

/// "Подключить": a profile link or a name
pub fn connect(input: &str) -> Result<(), String> {
    let value = input.trim();
    let url = if value.starts_with("http://") || value.starts_with("https://") { value.to_string() } else { format!("https://soundcloud.com/{}", value.trim_start_matches('@')) };
    let body = soundcloud::get("/resolve", &[("url", &url)])?;
    let user = soundcloud::map_user(&body).ok_or("По этой ссылке нет профиля SoundCloud")?;
    save_account(Some(&Account {
        id: user.id,
        name: user.name,
        avatar: user.avatar,
        permalink: body.get("permalink_url").and_then(Value::as_str).unwrap_or(&url).to_string(),
        followers: user.followers,
        likes: body.get("likes_count").and_then(Value::as_f64),
        tracks: body.get("track_count").and_then(Value::as_f64),
        sync_likes: true,
        last_sync: None,
    }));
    Ok(())
}

pub fn disconnect() {
    save_account(None);
}

fn stamp() {
    if let Some(mut a) = account() {
        a.last_sync = Some(crate::store::state::now_iso());
        save_account(Some(&a));
    }
}

/// your public likes on SoundCloud (up to 1000)
fn user_likes(user_id: &str) -> Result<Vec<Track>, String> {
    let id = user_id.trim_start_matches("scu:").trim_start_matches("sc:");
    let mut out = Vec::new();
    let mut next = Some(format!("/users/{id}/track_likes"));
    let mut first = true;
    while let Some(path) = next.take() {
        if out.len() >= 1000 {
            break;
        }
        let body = if first { soundcloud::get(&path, &[("limit", "200")])? } else { soundcloud::get(&path, &[])? };
        first = false;
        for row in body.get("collection").and_then(Value::as_array).cloned().unwrap_or_default() {
            if let Some(t) = row.get("track").and_then(soundcloud::map_track) {
                out.push(t);
            }
        }
        next = body.get("next_href").and_then(Value::as_str).map(str::to_string);
    }
    out.truncate(1000);
    Ok(out)
}

/// your public likes into "Понравившиеся"; how many were new
pub fn import_likes() -> Result<usize, String> {
    let Some(a) = account() else { return Ok(0) };
    let tracks = user_likes(&a.id)?;
    let added = crate::state::get().update_library(|l| l.add_likes(&tracks));
    stamp();
    Ok(added)
}

/// your SoundCloud playlists as EvPlayer playlists; importing again adds new songs; (new, updated)
pub fn import_playlists(step: &dyn Fn(usize, usize)) -> Result<(usize, usize), String> {
    let Some(a) = account() else { return Ok((0, 0)) };
    let id = a.id.trim_start_matches("scu:").to_string();
    let body = soundcloud::get(&format!("/users/{id}/playlists_without_albums"), &[("limit", "50")]).or_else(|_| soundcloud::get(&format!("/users/{id}/playlists"), &[("limit", "50")]))?;
    let lists: Vec<_> = body.get("collection").and_then(Value::as_array).map(|c| c.iter().filter_map(soundcloud::map_collection).collect()).unwrap_or_default();
    let (mut created, mut updated) = (0, 0);
    for (i, summary) in lists.iter().enumerate() {
        step(i, lists.len());
        let (full, tracks) = soundcloud::playlist(&summary.id)?;
        let icon = full.as_ref().and_then(|f| f.artwork.clone()).or_else(|| summary.artwork.clone()).or_else(|| tracks.iter().find_map(|t| t.artwork.clone()));
        crate::state::get().update_library(|l| {
            if let Some(existing) = l.playlists.iter().find(|p| p.source.as_deref() == Some(summary.id.as_str())).map(|p| p.id.clone()) {
                if l.add_to_playlist(&existing, &tracks) > 0 {
                    updated += 1;
                }
                // the cover follows SoundCloud unless you set your own picture (a data: url)
                if let (Some(icon), Some(p)) = (&icon, l.playlist_mut(&existing)) {
                    if !p.icon.as_deref().is_some_and(|i| i.starts_with("data:")) {
                        p.icon = Some(icon.clone());
                    }
                }
            } else if !tracks.is_empty() {
                let pid = l.create_playlist(&summary.title, Some("Из вашего SoundCloud"), Some(&summary.id));
                if let Some(p) = l.playlist_mut(&pid) {
                    p.icon = icon.clone();
                }
                l.add_to_playlist(&pid, &tracks);
                created += 1;
            }
        });
    }
    stamp();
    Ok((created, updated))
}

/// on start: likes added on SoundCloud since last time (at most once an hour)
pub fn sync_on_start() {
    let Some(a) = account().filter(|a| a.sync_likes) else { return };
    let last = a.last_sync.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|t| t.timestamp_millis() as u64).unwrap_or(0);
    if now_ms().saturating_sub(last) < 60 * 60_000 {
        return;
    }
    let _ = std::thread::Builder::new().name("sc-likes".into()).spawn(|| {
        std::thread::sleep(Duration::from_secs(8));
        let _ = import_likes();
    });
}

// ---- signing in (OAuth) ------------------------------------------------------------------------

/// "soundcloudLogin" in the settings
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Login {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

pub fn login() -> Option<Login> {
    crate::state::get().extra("soundcloudLogin").and_then(|v| serde_json::from_value(v).ok())
}

fn save_login(l: Option<&Login>) {
    crate::state::get().set_extra("soundcloudLogin", l.and_then(|l| serde_json::to_value(l).ok()).unwrap_or(Value::Null));
}

fn app_keys() -> Result<(String, String), String> {
    let s = crate::state::get().settings.read().unwrap();
    let id = s.soundcloud_client_id.trim().to_string();
    let secret = s.rest.get("soundcloudClientSecret").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if id.is_empty() || secret.is_empty() {
        return Err("Нужны client_id и client_secret вашего приложения SoundCloud".into());
    }
    Ok((id, secret))
}

fn block_on<T>(f: impl std::future::Future<Output = T>) -> Result<T, String> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    Ok(rt.block_on(f))
}

fn tokens_of(v: Value) -> (String, String, u64) {
    (v.get("accessToken").and_then(Value::as_str).unwrap_or_default().to_string(), v.get("refreshToken").and_then(Value::as_str).unwrap_or_default().to_string(), v.get("expiresAt").and_then(Value::as_u64).unwrap_or(0))
}

/// "Войти через OAuth": SoundCloud's sign-in page in your browser (blocking)
pub fn sign_in() -> Result<(), String> {
    let (id, secret) = app_keys()?;
    let tokens = block_on(crate::services_shared::soundcloud_sign_in(id, secret, |url| crate::dialogs::open_path(std::path::Path::new(&url))))??;
    let (access_token, refresh_token, expires_at) = tokens_of(serde_json::to_value(&tokens).map_err(|e| e.to_string())?);
    save_login(Some(&Login { id: String::new(), name: "SoundCloud".into(), avatar: None, access_token, refresh_token, expires_at }));
    let me = call("GET", "/me", None)?;
    if let Some(mut l) = login() {
        l.id = me.get("id").and_then(Value::as_u64).map(|i| i.to_string()).or_else(|| me.get("urn").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
        l.name = me.get("username").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("SoundCloud").to_string();
        l.avatar = me.get("avatar_url").and_then(Value::as_str).map(str::to_string);
        save_login(Some(&l));
    }
    Ok(())
}

pub fn sign_out() {
    save_login(None);
    crate::state::get().set_extra("soundcloudFullSync", Value::Bool(false));
}

/// a failed call: the sign-in is gone (stop the pass) or one request failed (go on)
pub enum CallError {
    Auth(String),
    Api(u16, String),
}

impl From<CallError> for String {
    fn from(e: CallError) -> String {
        match e {
            CallError::Auth(m) | CallError::Api(_, m) => m,
        }
    }
}

/// only one renewal at a time: a refresh token works once
static REFRESHING: Mutex<()> = Mutex::new(());

fn access_token(force: bool) -> Result<String, CallError> {
    let _one = REFRESHING.lock().unwrap();
    let l = login().ok_or_else(|| CallError::Auth("Вы не вошли в SoundCloud".into()))?;
    if !force && now_ms() < l.expires_at {
        return Ok(l.access_token);
    }
    let (id, secret) = app_keys().map_err(CallError::Auth)?;
    let fresh = block_on(crate::services_shared::soundcloud_refresh(id, secret, l.refresh_token.clone())).map_err(CallError::Auth)?;
    let fresh = fresh.map_err(|e| CallError::Auth(format!("Вход в SoundCloud устарел, войдите заново в настройках ({e})")))?;
    let (access_token, refresh_token, expires_at) = tokens_of(serde_json::to_value(&fresh).unwrap_or_default());
    save_login(Some(&Login { access_token: access_token.clone(), refresh_token, expires_at, ..l }));
    Ok(access_token)
}

fn call_once(method: &str, path: &str, body: Option<&Value>) -> Result<Value, CallError> {
    crate::services::region::guard_soundcloud().map_err(|e| CallError::Api(0, e))?;
    let mut renewed = false;
    for attempt in 0.. {
        let token = access_token(false)?;
        let url = format!("{API}{path}");
        let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut req = crate::services::http::client().request(m, &url).header("authorization", format!("OAuth {token}")).header("accept", "application/json; charset=utf-8");
        if let Some(b) = body {
            req = req.header("content-type", "application/json").body(b.to_string());
        }
        let res = req.send().map_err(|e| CallError::Api(0, e.to_string()))?;
        let status = res.status().as_u16();
        if status == 429 && attempt < 4 {
            let wait = res.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(5u64).min(60);
            std::thread::sleep(Duration::from_secs(wait));
            continue;
        }
        if status == 401 && !renewed {
            renewed = true;
            access_token(true)?;
            continue;
        }
        if status == 401 {
            return Err(CallError::Auth("SoundCloud не принял вход, войдите заново в настройках".into()));
        }
        if !(200..300).contains(&status) {
            return Err(CallError::Api(status, format!("SoundCloud ответил {status}")));
        }
        let text = res.text().unwrap_or_default();
        return Ok(if text.trim().is_empty() { json!({}) } else { serde_json::from_str(&text).unwrap_or(json!({})) });
    }
    unreachable!()
}

/// one call to api.soundcloud.com on behalf of the account; waits out "too many requests"
/// and renews an expired token once
pub fn call(method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
    call_once(method, path, body).map_err(String::from)
}

// ---- the full sync ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Known {
    #[serde(rename = "scId")]
    pub sc_id: String,
    pub signature: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Baseline {
    pub likes: Vec<String>,
    pub playlists: BTreeMap<String, Known>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct MirrorFile {
    base: Baseline,
    last_run: Option<String>,
}

fn load() -> MirrorFile {
    crate::store::kv::read_state(&crate::state::get().kv, MIRROR_KEY).unwrap_or_default()
}

fn store(m: &MirrorFile) {
    crate::store::kv::write_state(&crate::state::get().kv, MIRROR_KEY, m, 0);
}

fn commit(f: impl FnOnce(&mut Baseline)) {
    let mut m = load();
    f(&mut m.base);
    store(&m);
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Like(String),
    Unlike(String),
    Create(String),
    Update(String, String),
    Drop(String, String),
}

/// "sc:123" → "123"
pub fn sc_track_id(id: &str) -> Option<&str> {
    id.strip_prefix("sc:").filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// imported from SoundCloud: lives there already, not ours to create or delete
pub fn is_sc_import(p: &Playlist) -> bool {
    p.source.as_deref().is_some_and(|s| s.starts_with("scp:"))
}

pub fn track_ids(p: &Playlist) -> Vec<String> {
    let mut seen = HashSet::new();
    p.items.iter().filter_map(|i| sc_track_id(&i.track.id)).filter(|id| seen.insert(id.to_string())).map(str::to_string).collect()
}

/// everything that has to match on SoundCloud: title, description and the ordered songs (the
/// same text the page makes, so a baseline written by either build is understood)
pub fn signature_of(p: &Playlist) -> String {
    serde_json::to_string(&json!([p.title, p.description.clone().unwrap_or_default(), track_ids(p)])).unwrap_or_default()
}

/// what must be sent so that SoundCloud matches the library; likes go oldest first
pub fn plan(likes: &[Like], playlists: &[Playlist], base: &Baseline) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut wanted: HashMap<String, String> = HashMap::new();
    for l in likes {
        if let Some(id) = sc_track_id(&l.track.id) {
            wanted.insert(id.to_string(), l.added_at.clone());
        }
    }
    let have: HashSet<&String> = base.likes.iter().collect();
    let mut new: Vec<(&String, &String)> = wanted.iter().filter(|(id, _)| !have.contains(id)).collect();
    new.sort_by(|a, b| a.1.cmp(b.1));
    steps.extend(new.into_iter().map(|(id, _)| Step::Like(id.clone())));
    for t in &base.likes {
        if !wanted.contains_key(t) {
            steps.push(Step::Unlike(t.clone()));
        }
    }
    let mine: Vec<&Playlist> = playlists.iter().filter(|p| !is_sc_import(p)).collect();
    let ids: HashSet<&str> = mine.iter().map(|p| p.id.as_str()).collect();
    for p in &mine {
        match base.playlists.get(&p.id) {
            None => steps.push(Step::Create(p.id.clone())),
            Some(k) if k.signature != signature_of(p) => steps.push(Step::Update(p.id.clone(), k.sc_id.clone())),
            _ => {}
        }
    }
    for (pid, k) in &base.playlists {
        if !ids.contains(pid.as_str()) {
            steps.push(Step::Drop(pid.clone(), k.sc_id.clone()));
        }
    }
    steps
}

/// songs and playlists as urns (the guide) or plain numbers (the OpenAPI file): urns first,
/// and the style that worked is kept for the rest of the run
static URN: AtomicBool = AtomicBool::new(true);

fn reference(kind: &str, id: &str) -> String {
    if URN.load(Ordering::Relaxed) {
        format!("soundcloud:{kind}:{id}")
    } else {
        id.to_string()
    }
}

fn either(job: &dyn Fn() -> Result<Value, CallError>) -> Result<Value, CallError> {
    match job() {
        Err(CallError::Api(s, m)) if [400, 404, 422].contains(&s) => {
            let previous = URN.load(Ordering::Relaxed);
            URN.store(!previous, Ordering::Relaxed);
            job().inspect_err(|_| URN.store(previous, Ordering::Relaxed)).map_err(|e| match e {
                CallError::Api(0, _) => CallError::Api(s, m),
                other => other,
            })
        }
        other => other,
    }
}

fn playlist_body(p: &Playlist) -> Value {
    let urn = URN.load(Ordering::Relaxed);
    let tracks: Vec<Value> = track_ids(p).into_iter().map(|id| if urn { json!({ "urn": format!("soundcloud:tracks:{id}") }) } else { json!({ "id": id.parse::<u64>().unwrap_or(0) }) }).collect();
    json!({ "playlist": { "title": p.title, "description": p.description.clone().unwrap_or_default(), "sharing": "private", "tracks": tracks } })
}

fn run_step(step: &Step, by_id: &HashMap<String, Playlist>) -> Result<(), CallError> {
    match step {
        Step::Like(t) => {
            either(&|| call_once("POST", &format!("/likes/tracks/{}", reference("tracks", t)), None))?;
            commit(|b| {
                if !b.likes.contains(t) {
                    b.likes.push(t.clone());
                }
            });
        }
        Step::Unlike(t) => {
            match either(&|| call_once("DELETE", &format!("/likes/tracks/{}", reference("tracks", t)), None)) {
                // already gone from SoundCloud: nothing left to remove
                Err(CallError::Api(404, _)) | Ok(_) => {}
                Err(e) => return Err(e),
            }
            commit(|b| b.likes.retain(|x| x != t));
        }
        Step::Create(pid) => {
            let Some(p) = by_id.get(pid) else { return Ok(()) };
            let signature = signature_of(p);
            let made = either(&|| call_once("POST", "/playlists", Some(&playlist_body(p))))?;
            let sc_id = made.get("id").and_then(Value::as_u64).map(|i| i.to_string()).or_else(|| made.get("urn").and_then(Value::as_str).and_then(|u| u.rsplit(':').next()).filter(|n| n.chars().all(|c| c.is_ascii_digit())).map(str::to_string)).ok_or_else(|| CallError::Api(0, "SoundCloud не вернул номер созданного плейлиста".into()))?;
            commit(|b| {
                b.playlists.insert(pid.clone(), Known { sc_id, signature });
            });
        }
        Step::Update(pid, sc_id) => {
            let Some(p) = by_id.get(pid) else { return Ok(()) };
            let signature = signature_of(p);
            match either(&|| call_once("PUT", &format!("/playlists/{}", reference("playlists", sc_id)), Some(&playlist_body(p)))) {
                // deleted on SoundCloud meanwhile: forget it, the next pass creates it again
                Err(CallError::Api(404, _)) => commit(|b| {
                    b.playlists.remove(pid);
                }),
                Err(e) => return Err(e),
                Ok(_) => commit(|b| {
                    b.playlists.insert(pid.clone(), Known { sc_id: sc_id.clone(), signature });
                }),
            }
        }
        Step::Drop(pid, sc_id) => {
            match either(&|| call_once("DELETE", &format!("/playlists/{}", reference("playlists", sc_id)), None)) {
                Err(CallError::Api(404, _)) | Ok(_) => {}
                Err(e) => return Err(e),
            }
            commit(|b| {
                b.playlists.remove(pid);
            });
        }
    }
    Ok(())
}

pub fn full_sync() -> bool {
    crate::state::get().extra("soundcloudFullSync").and_then(|v| v.as_bool()).unwrap_or(false)
}

fn enabled() -> bool {
    full_sync() && login().is_some()
}

/// what the settings show about the mirror
#[derive(Clone, Debug, Default)]
pub struct MirrorStatus {
    pub running: bool,
    pub error: Option<String>,
    pub last_run: Option<String>,
}

static STATUS: Mutex<MirrorStatus> = Mutex::new(MirrorStatus { running: false, error: None, last_run: None });
static AGAIN: AtomicBool = AtomicBool::new(false);

pub fn status() -> MirrorStatus {
    let mut s = STATUS.lock().unwrap().clone();
    if s.last_run.is_none() {
        s.last_run = load().last_run;
    }
    s
}

fn patch(f: impl FnOnce(&mut MirrorStatus)) {
    f(&mut STATUS.lock().unwrap());
    crate::events::repaint();
}

/// one pass: compares and sends the difference; passes asked for meanwhile fold into one more
pub fn sync_now() {
    if !enabled() {
        return;
    }
    {
        let mut s = STATUS.lock().unwrap();
        if s.running {
            AGAIN.store(true, Ordering::Relaxed);
            return;
        }
        s.running = true;
        s.error = None;
    }
    crate::events::repaint();
    let result = (|| -> Result<(), String> {
        loop {
            AGAIN.store(false, Ordering::Relaxed);
            let (likes, playlists) = {
                let l = crate::state::get().library.read().unwrap();
                (l.likes.clone(), l.playlists.clone())
            };
            let steps = plan(&likes, &playlists, &load().base);
            let by_id: HashMap<String, Playlist> = playlists.into_iter().map(|p| (p.id.clone(), p)).collect();
            let mut failed: Option<String> = None;
            for step in &steps {
                if !enabled() {
                    return Ok(());
                }
                match run_step(step, &by_id) {
                    Ok(()) => {}
                    Err(CallError::Auth(m)) => return Err(m),
                    Err(CallError::Api(_, m)) => {
                        failed.get_or_insert(m);
                    }
                }
                std::thread::sleep(Duration::from_millis(400));
            }
            if let Some(f) = failed {
                patch(|s| s.error = Some(f));
            }
            if !AGAIN.load(Ordering::Relaxed) {
                break;
            }
        }
        let mut m = load();
        m.last_run = Some(crate::store::state::now_iso());
        store(&m);
        patch(|s| s.last_run = m.last_run.clone());
        Ok(())
    })();
    if let Err(e) = result {
        patch(|s| s.error = Some(e));
    }
    patch(|s| s.running = false);
}

/// when the next pass is due (a few seconds after the last change in the library)
static DUE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
static WAKE: std::sync::Condvar = std::sync::Condvar::new();

/// the library changed, or full sync was turned on: a pass after `ms`
pub fn schedule(ms: u64) {
    if !enabled() {
        return;
    }
    *DUE.lock().unwrap() = Some(std::time::Instant::now() + Duration::from_millis(ms));
    WAKE.notify_all();
}

/// the thread that runs the passes (sleeps until one is due); a first pass 10 s after the start
pub fn start() {
    sync_on_start();
    let _ = std::thread::Builder::new().name("sc-mirror".into()).spawn(|| {
        schedule(10_000);
        loop {
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
            sync_now();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::state::PlaylistItem;

    fn t(id: &str) -> Track {
        Track { id: id.into(), title: id.into(), ..Default::default() }
    }

    fn pl(id: &str, items: &[&str], source: Option<&str>) -> Playlist {
        Playlist { id: id.into(), title: format!("P {id}"), description: None, icon: None, created_at: String::new(), updated_at: String::new(), source: source.map(str::to_string), items: items.iter().map(|i| PlaylistItem { id: format!("i{i}"), track: t(i), added_at: String::new() }).collect() }
    }

    #[test]
    fn the_plan_sends_only_the_difference() {
        let likes = vec![Like { track: t("sc:2"), added_at: "2026-01-02".into() }, Like { track: t("sc:1"), added_at: "2026-01-01".into() }, Like { track: t("yt:x"), added_at: "2026-01-03".into() }];
        let mine = pl("p1", &["sc:5", "yt:z", "sc:6", "sc:5"], None);
        let imported = pl("p2", &["sc:9"], Some("scp:77"));
        let mut base = Baseline { likes: vec!["2".into(), "3".into()], ..Default::default() };
        base.playlists.insert("gone".into(), Known { sc_id: "44".into(), signature: "x".into() });
        let steps = plan(&likes, &[mine.clone(), imported], &base);
        assert_eq!(steps, vec![Step::Like("1".into()), Step::Unlike("3".into()), Step::Create("p1".into()), Step::Drop("gone".into(), "44".into())]);
        assert_eq!(track_ids(&mine), vec!["5", "6"]);
        // the page's own signature text
        assert_eq!(signature_of(&mine), r#"["P p1","",["5","6"]]"#);
        base.playlists.clear();
        base.playlists.insert("p1".into(), Known { sc_id: "8".into(), signature: signature_of(&mine) });
        assert!(plan(&[], &[mine], &Baseline { likes: vec![], playlists: base.playlists }).is_empty());
    }
}
