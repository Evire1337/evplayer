//! Spotify (src/services/spotify/*): reading playlists, albums and "Liked Songs" to carry them
//! over (the songs are then found on YouTube Music and SoundCloud).
//!
//! Three ways in, best first, as the Tauri build has them:
//!  - your account ("Войти через Spotify", PKCE in your own browser; any account) or an app of
//!    your own with Client ID + Secret: Spotify's official Web API, any length;
//!  - the guest access of Spotify's web player: a whole public playlist without signing in.
//!    The token needs a one-time code (TOTP) whose secret ships in the web player's script, so
//!    the script is read first. Not an official API: when Spotify changes the player, the
//!    embed page below still works;
//!  - the embed page (import.rs): 100 songs.
//! Every request goes to the hosts in OPEN_HOSTS only.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

use crate::import::Named;
use crate::services::http;

const PLAYER_VERSION: &str = "1.3.2.216.g7f3fbea1";
const WEB_CLIENT_ID: &str = "d8a5ed958d274c2e8ee717e6a4b0971d";
const PATHFINDER_HASH: &str = "86dde7b9d9356e2369414647cf6950cfed96e778e129cfdfc99aea6c1613b3b0";
const PAGE_LIMIT: usize = 100;
const MAX_PAGES: usize = 60;
const PAGE_ATTEMPTS: usize = 3;
const API: &str = "https://api.spotify.com/v1";
/// the Client ID built into EvPlayer (the Tauri build has none either: your own is needed)
const DEFAULT_CLIENT_ID: &str = "";

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn stopped(cancel: Option<&AtomicBool>) -> Result<(), String> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        Err("Загрузка отменена".into())
    } else {
        Ok(())
    }
}

// ---- the one-time code -----------------------------------------------------------------

/// the newest `{secret:'…',version:N}` listed in the web player's script
pub fn extract_secret(js: &str) -> Option<(String, u32)> {
    let re = regex::Regex::new(r#"\{secret:(['"])((?:\\.|[^\\'"])*)['"],version:(\d+)\}"#).ok()?;
    let mut newest: Option<(String, u32)> = None;
    for m in re.captures_iter(js) {
        let raw = &m[2];
        let mut secret = String::new();
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(n) = chars.next() {
                    secret.push(n);
                }
            } else {
                secret.push(c);
            }
        }
        let version: u32 = m[3].parse().unwrap_or(0);
        if newest.as_ref().is_none_or(|n| version > n.1) {
            newest = Some((secret, version));
        }
    }
    newest
}

/// the key the player derives from the secret
pub fn secret_bytes(secret: &str) -> Vec<u8> {
    let mixed: String = secret.chars().enumerate().map(|(i, c)| (c as u32 ^ ((i as u32 % 33) + 9)).to_string()).collect();
    mixed.into_bytes()
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..20].copy_from_slice(&sha1_smol::Sha1::from(key).digest().bytes());
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = sha1_smol::Sha1::new();
    inner.update(&k.map(|b| b ^ 0x36));
    inner.update(msg);
    let mut outer = sha1_smol::Sha1::new();
    outer.update(&k.map(|b| b ^ 0x5c));
    outer.update(&inner.digest().bytes());
    outer.digest().bytes()
}

/// six digits, a new code every 30 seconds (RFC 6238 with SHA-1)
pub fn totp(key: &[u8], time_ms: u64) -> String {
    let counter = (time_ms / 1000 / 30).to_be_bytes();
    let h = hmac_sha1(key, &counter);
    let o = (h[19] & 0xf) as usize;
    let code = ((h[o] as u32 & 0x7f) << 24) | ((h[o + 1] as u32) << 16) | ((h[o + 2] as u32) << 8) | h[o + 3] as u32;
    format!("{:06}", code % 1_000_000)
}

// ---- the guest session -----------------------------------------------------------------

struct Session {
    access: String,
    client: String,
    until: u64,
}

static GUEST: Mutex<Option<Session>> = Mutex::new(None);

fn guest_token() -> Result<(String, u64), String> {
    let html = http::get_text("https://open.spotify.com/").map_err(|_| "Spotify сейчас недоступен, попробуйте позже".to_string())?;
    let script = regex::Regex::new(r#"src="(https://open\.spotifycdn\.com/cdn/build/(?:mobile-)?web-player/(?:mobile-)?web-player\.[a-f0-9]+\.js)""#).unwrap().captures(&html).map(|m| m[1].to_string()).ok_or("Spotify сейчас недоступен, попробуйте позже")?;
    let js = http::get_text(&script)?;
    let (secret, version) = extract_secret(&js).ok_or("Spotify сейчас недоступен, попробуйте позже")?;
    let key = secret_bytes(&secret);
    let server = http::get_json("https://open.spotify.com/api/server-time", &[]).ok().and_then(|v| v.get("serverTime").and_then(Value::as_f64));
    let url = format!(
        "https://open.spotify.com/api/token?reason=init&productType=web-player&totp={}&totpServer={}&totpVer={version}",
        totp(&key, now_ms()),
        server.map(|s| totp(&key, (s * 1000.0) as u64)).unwrap_or_else(|| "unavailable".into())
    );
    let body = http::get_json(&url, &[("accept", "application/json")]).map_err(|e| format!("Spotify не выдал гостевой доступ ({e})"))?;
    let token = body.get("accessToken").and_then(Value::as_str).filter(|t| !t.is_empty()).ok_or("Spotify не выдал гостевой доступ")?;
    let until = body.get("accessTokenExpirationTimestampMs").and_then(Value::as_f64).map(|v| v as u64).unwrap_or(now_ms() + 3_500_000);
    Ok((token.to_string(), until))
}

fn client_token() -> String {
    let body = json!({ "client_data": { "client_version": PLAYER_VERSION, "client_id": WEB_CLIENT_ID, "js_sdk_data": {
        "device_brand": "unknown", "device_model": "unknown", "os": "windows", "os_version": "unknown",
        "device_id": format!("{:032x}", rand::random::<u128>()), "device_type": "computer", "platform_identifier": "web-player" } } });
    http::post_json("https://clienttoken.spotify.com/v1/clienttoken", &[("accept", "application/json")], &body).ok().and_then(|v| v.pointer("/granted_token/token").and_then(Value::as_str).map(str::to_string)).unwrap_or_default()
}

fn session() -> Result<(String, String), String> {
    let mut g = GUEST.lock().unwrap();
    if let Some(s) = g.as_ref().filter(|s| now_ms() + 60_000 < s.until) {
        return Ok((s.access.clone(), s.client.clone()));
    }
    let (access, until) = guest_token()?;
    let client = client_token();
    *g = Some(Session { access: access.clone(), client: client.clone(), until });
    Ok((access, client))
}

/// one playlist row of the pathfinder reply as a song to look up
pub fn map_item(item: &Value) -> Option<Named> {
    let t = item.pointer("/itemV2/data")?;
    if t.get("__typename").and_then(Value::as_str) != Some("Track") {
        return None;
    }
    let title = t.get("name").and_then(Value::as_str)?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let artists = t.pointer("/artists/items").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.pointer("/profile/name").or_else(|| x.get("name")).and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
    Some(Named { title, artists, duration_ms: t.pointer("/trackDuration/totalMilliseconds").and_then(Value::as_f64) })
}

fn guest_page(id: &str, offset: usize, cancel: Option<&AtomicBool>) -> Result<Option<Value>, String> {
    let (mut access, mut client) = session()?;
    for attempt in 1..=PAGE_ATTEMPTS {
        stopped(cancel)?;
        let vars = json!({ "uri": format!("spotify:playlist:{id}"), "offset": offset, "limit": PAGE_LIMIT }).to_string();
        let ext = json!({ "persistedQuery": { "version": 1, "sha256Hash": PATHFINDER_HASH } }).to_string();
        let url = format!(
            "https://api-partner.spotify.com/pathfinder/v1/query?operationName=fetchPlaylistContents&variables={}&extensions={}",
            percent_encoding::utf8_percent_encode(&vars, percent_encoding::NON_ALPHANUMERIC),
            percent_encoding::utf8_percent_encode(&ext, percent_encoding::NON_ALPHANUMERIC)
        );
        let bearer = format!("Bearer {access}");
        let mut headers = vec![("authorization", bearer.as_str()), ("accept", "application/json"), ("app-platform", "WebPlayer"), ("spotify-app-version", PLAYER_VERSION)];
        if !client.is_empty() {
            headers.push(("client-token", client.as_str()));
        }
        let res = match http::get(&url, &headers) {
            Ok(r) => r,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(500 * attempt as u64));
                continue;
            }
        };
        match res.status().as_u16() {
            429 => {
                let wait = res.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(3u64).min(30);
                std::thread::sleep(Duration::from_secs(wait));
                continue;
            }
            401 | 403 => {
                *GUEST.lock().unwrap() = None;
                (access, client) = session()?;
                continue;
            }
            s if !(200..300).contains(&s) => {
                std::thread::sleep(Duration::from_millis(500 * attempt as u64));
                continue;
            }
            _ => {}
        }
        let body: Value = res.json().map_err(|e| e.to_string())?;
        if let Some(c) = body.pointer("/data/playlistV2/content") {
            return Ok(Some(c.clone()));
        }
    }
    Ok(None)
}

/// every song of a public playlist, any length, without signing in; (songs, total)
pub fn guest_playlist(id: &str, cancel: Option<&AtomicBool>, on_page: &dyn Fn(usize, usize)) -> Result<(Vec<Named>, Option<usize>), String> {
    let mut tracks = Vec::new();
    let mut total: Option<usize> = None;
    for n in 0..MAX_PAGES {
        let offset = n * PAGE_LIMIT;
        if total.is_some_and(|t| offset >= t) {
            break;
        }
        let Some(content) = guest_page(id, offset, cancel)? else {
            if n == 0 {
                return Err("Spotify не отдал плейлист без входа".into());
            }
            break;
        };
        total = content.get("totalCount").and_then(Value::as_u64).map(|t| t as usize).or(total);
        let items = content.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        if items.is_empty() {
            break;
        }
        tracks.extend(items.iter().filter_map(map_item));
        on_page(tracks.len(), total.unwrap_or(tracks.len()));
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok((tracks, total))
}

// ---- your account and the official Web API ----------------------------------------------

/// the account as the Tauri build keeps it in the settings ("spotifyAccount")
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

pub fn account() -> Option<Account> {
    crate::state::get().extra("spotifyAccount").and_then(|v| serde_json::from_value(v).ok())
}

fn save_account(a: Option<&Account>) {
    crate::state::get().set_extra("spotifyAccount", a.map(|a| serde_json::to_value(a).unwrap_or(Value::Null)).unwrap_or(Value::Null));
}

/// this build carries EvPlayer's own Spotify application
pub fn builtin_client_id() -> bool {
    !DEFAULT_CLIENT_ID.is_empty()
}

pub fn client_id() -> String {
    let s = crate::state::get().settings.read().unwrap().spotify_client_id.trim().to_string();
    if s.is_empty() {
        DEFAULT_CLIENT_ID.to_string()
    } else {
        s
    }
}

/// signed in, or an app of your own with its secret
pub fn has_access() -> bool {
    let s = crate::state::get().settings.read().unwrap();
    account().is_some() || (!s.spotify_client_id.trim().is_empty() && !s.spotify_client_secret.trim().is_empty())
}

fn block_on<T>(f: impl std::future::Future<Output = T>) -> Result<T, String> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    Ok(rt.block_on(f))
}

fn tokens_of(v: Value) -> (String, String, u64) {
    (v.get("accessToken").and_then(Value::as_str).unwrap_or_default().to_string(), v.get("refreshToken").and_then(Value::as_str).unwrap_or_default().to_string(), v.get("expiresAt").and_then(Value::as_u64).unwrap_or(0))
}

/// "Войти через Spotify": opens the sign-in page in your browser and waits (blocking)
pub fn sign_in() -> Result<(), String> {
    let id = client_id();
    if id.is_empty() {
        return Err("Нужен Client ID приложения Spotify — см. «Своё приложение» ниже".into());
    }
    let tokens = block_on(crate::services_shared::spotify_sign_in(id, |url| crate::dialogs::open_path(std::path::Path::new(&url))))??;
    let (access_token, refresh_token, expires_at) = tokens_of(serde_json::to_value(&tokens).map_err(|e| e.to_string())?);
    save_account(Some(&Account { name: "Spotify".into(), access_token, refresh_token, expires_at, ..Default::default() }));
    let me = get("/me", None)?;
    if let Some(mut a) = account() {
        a.name = me.get("display_name").and_then(Value::as_str).filter(|s| !s.is_empty()).or(me.get("id").and_then(Value::as_str)).unwrap_or("Spotify").to_string();
        a.avatar = me.pointer("/images/0/url").and_then(Value::as_str).map(str::to_string);
        a.product = me.get("product").and_then(Value::as_str).map(str::to_string);
        save_account(Some(&a));
    }
    Ok(())
}

pub fn sign_out() {
    save_account(None);
}

static APP_TOKEN: Mutex<Option<(String, u64, String)>> = Mutex::new(None);

fn access_token() -> Result<String, String> {
    if let Some(a) = account() {
        if now_ms() < a.expires_at {
            return Ok(a.access_token);
        }
        let fresh = block_on(crate::services_shared::spotify_refresh(client_id(), a.refresh_token.clone()))?.map_err(|_| "Вход в Spotify устарел — войдите заново в настройках".to_string())?;
        let (access_token, refresh_token, expires_at) = tokens_of(serde_json::to_value(&fresh).map_err(|e| e.to_string())?);
        save_account(Some(&Account { access_token: access_token.clone(), refresh_token, expires_at, ..a }));
        return Ok(access_token);
    }
    let (id, secret) = {
        let s = crate::state::get().settings.read().unwrap();
        (s.spotify_client_id.trim().to_string(), s.spotify_client_secret.trim().to_string())
    };
    if id.is_empty() || secret.is_empty() {
        return Err("Spotify не подключён: войдите через Spotify в Настройки → Сервисы".into());
    }
    let key = format!("{id}:{secret}");
    if let Some((t, until, k)) = APP_TOKEN.lock().unwrap().as_ref() {
        if *k == key && now_ms() < *until {
            return Ok(t.clone());
        }
    }
    use base64::Engine;
    let basic = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(&key));
    let res = http::client().post("https://accounts.spotify.com/api/token").header("authorization", basic).header("content-type", "application/x-www-form-urlencoded").body("grant_type=client_credentials").send().map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err("Spotify не принял Client ID / Client Secret — проверьте их в настройках".into());
    }
    let body: Value = res.json().map_err(|e| e.to_string())?;
    let token = body.get("access_token").and_then(Value::as_str).ok_or("Spotify не выдал доступ")?.to_string();
    let until = now_ms() + (body.get("expires_in").and_then(Value::as_u64).unwrap_or(3600).saturating_sub(60)) * 1000;
    *APP_TOKEN.lock().unwrap() = Some((token.clone(), until, key));
    Ok(token)
}

fn get(path: &str, cancel: Option<&AtomicBool>) -> Result<Value, String> {
    let url = if path.starts_with("http") { path.to_string() } else { format!("{API}{path}") };
    let mut renewed = false;
    for attempt in 0.. {
        stopped(cancel)?;
        let bearer = format!("Bearer {}", access_token()?);
        let res = http::get(&url, &[("authorization", bearer.as_str())])?;
        match res.status().as_u16() {
            // too many requests: wait as long as Spotify asks, then try again
            429 if attempt < 5 => {
                let wait = res.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(2u64).min(30);
                std::thread::sleep(Duration::from_secs(wait));
            }
            401 if !renewed && account().is_some() => {
                renewed = true;
                if let Some(mut a) = account() {
                    a.expires_at = 0;
                    save_account(Some(&a));
                }
            }
            404 => return Err("Spotify не отдаёт этот плейлист сторонним приложениям (так бывает с подборками, которые собирает сам Spotify)".into()),
            403 => return Err("Spotify не пустил: аккаунт не добавлен в приложение EvPlayer на developer.spotify.com (раздел User Management)".into()),
            s if !(200..300).contains(&s) => return Err(format!("Spotify ответил {s}")),
            _ => return res.json().map_err(|e| e.to_string()),
        }
    }
    unreachable!()
}

/// every page of a list (Spotify gives 50-100 items at a time)
fn all_pages(first: &str, cancel: Option<&AtomicBool>, on_page: &dyn Fn(usize, usize)) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut next = Some(first.to_string());
    while let Some(url) = next.take() {
        if out.len() >= 20_000 {
            break;
        }
        let page = get(&url, cancel)?;
        out.extend(page.get("items").and_then(Value::as_array).cloned().unwrap_or_default());
        on_page(out.len(), page.get("total").and_then(Value::as_u64).map(|t| t as usize).unwrap_or(out.len()));
        next = page.get("next").and_then(Value::as_str).map(str::to_string);
    }
    Ok(out)
}

fn named(t: &Value) -> Option<Named> {
    let name = t.get("name").and_then(Value::as_str).filter(|n| !n.is_empty())?;
    if t.get("type").and_then(Value::as_str) == Some("episode") {
        return None;
    }
    let artists = t.get("artists").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.get("name").and_then(Value::as_str).map(str::to_string)).filter(|n| !n.is_empty()).collect()).unwrap_or_default();
    Some(Named { title: name.to_string(), artists, duration_ms: t.get("duration_ms").and_then(Value::as_f64) })
}

/// every song of a playlist, album or track through the Web API; (title, songs, cover)
pub fn collection(kind: &str, id: &str, cancel: Option<&AtomicBool>, on_page: &dyn Fn(usize, usize)) -> Result<(String, Vec<Named>, Option<String>), String> {
    match kind {
        "track" => {
            let t = get(&format!("/tracks/{id}"), cancel)?;
            Ok((t.get("name").and_then(Value::as_str).unwrap_or("Spotify").to_string(), named(&t).into_iter().collect(), None))
        }
        "album" => {
            let album = get(&format!("/albums/{id}"), cancel)?;
            let items = all_pages(&format!("{API}/albums/{id}/tracks?limit=50"), cancel, on_page)?;
            Ok((album.get("name").and_then(Value::as_str).unwrap_or("Spotify").to_string(), items.iter().filter_map(named).collect(), album.pointer("/images/0/url").and_then(Value::as_str).map(str::to_string)))
        }
        _ => {
            let meta = get(&format!("/playlists/{id}?fields=name,images"), cancel)?;
            let items = all_pages(&format!("{API}/playlists/{id}/tracks?limit=100&offset=0&fields=next,total,items(track(name,duration_ms,type,artists(name)))"), cancel, on_page)?;
            Ok((meta.get("name").and_then(Value::as_str).unwrap_or("Spotify").to_string(), items.iter().filter_map(|i| i.get("track").and_then(named)).collect(), meta.pointer("/images/0/url").and_then(Value::as_str).map(str::to_string)))
        }
    }
}

/// "Liked Songs" of the signed-in account
pub fn liked_songs(cancel: Option<&AtomicBool>, on_page: &dyn Fn(usize, usize)) -> Result<Vec<Named>, String> {
    Ok(all_pages(&format!("{API}/me/tracks?limit=50"), cancel, on_page)?.iter().filter_map(|i| i.get("track").and_then(named)).collect())
}

/// your playlists: (id, name, songs, cover)
pub fn my_playlists(cancel: Option<&AtomicBool>) -> Result<Vec<(String, String, usize, Option<String>)>, String> {
    Ok(all_pages(&format!("{API}/me/playlists?limit=50"), cancel, &|_, _| {})?
        .iter()
        .filter_map(|p| Some((p.get("id")?.as_str()?.to_string(), p.get("name").and_then(Value::as_str).unwrap_or("Spotify").to_string(), p.pointer("/tracks/total").and_then(Value::as_u64).unwrap_or(0) as usize, p.pointer("/images/0/url").and_then(Value::as_str).map(str::to_string))))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_newest_secret_in_the_player_script() {
        let script = r#"let a=[{secret:'old',version:60},{secret:",7/*F(\"x",version:61}]"#;
        assert_eq!(extract_secret(script), Some((",7/*F(\"x".to_string(), 61)));
        assert_eq!(extract_secret("nothing here"), None);
    }

    #[test]
    fn makes_six_digit_codes_that_change_every_30_seconds() {
        let key = secret_bytes("abc");
        let a = totp(&key, 0);
        assert_eq!(a.len(), 6);
        assert_eq!(totp(&key, 29_000), a);
        // RFC 6238's own example for SHA-1 at 59 s: 94287082 → its last six digits
        assert_eq!(totp(b"12345678901234567890", 59_000), "287082");
    }

    #[test]
    fn turns_a_pathfinder_row_into_a_song_to_look_up() {
        let row = json!({ "itemV2": { "data": { "__typename": "Track", "name": " Song ", "artists": { "items": [{ "profile": { "name": "A" } }, { "profile": { "name": "B" } }] }, "trackDuration": { "totalMilliseconds": 123000 } } } });
        let n = map_item(&row).unwrap();
        assert_eq!((n.title.as_str(), n.artists.clone(), n.duration_ms), ("Song", vec!["A".to_string(), "B".to_string()], Some(123000.0)));
        assert!(map_item(&json!({ "itemV2": { "data": { "__typename": "Episode", "name": "x" } } })).is_none());
    }
}
