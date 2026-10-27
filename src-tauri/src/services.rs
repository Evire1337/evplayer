//! Small network jobs the page cannot do itself: the SoundCloud client id and the
//! "Sign in with Spotify" flow.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::proxy::client;

// ---- SoundCloud ---------------------------------------------------------------------
// SoundCloud's public web player uses a client id that it ships inside its own scripts.
// We read it the same way the site does when it loads, and keep it for a day.

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Serialize, Deserialize)]
struct Cached {
    id: String,
    at: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn extract_client_id(script: &str) -> Option<String> {
    regex::Regex::new(r#"client_id\s*[:=]\s*"([a-zA-Z0-9]{32})""#).unwrap().captures(script).map(|c| c[1].to_string())
}

pub fn script_urls(html: &str) -> Vec<String> {
    regex::Regex::new(r#"<script[^>]+src="(https://a-v2\.sndcdn\.com/assets/[^"]+\.js)""#)
        .unwrap()
        .captures_iter(html)
        .map(|c| c[1].to_string())
        .collect()
}

async fn text(url: &str) -> Result<String, String> {
    let res = client().get(url).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("{url} answered {}", res.status()));
    }
    res.text().await.map_err(|e| e.to_string())
}

pub async fn soundcloud_client_id(data_dir: PathBuf, force: bool) -> Result<String, String> {
    let file = data_dir.join("soundcloud.json");
    let cached: Option<Cached> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok());
    if let Some(c) = &cached {
        if !force && now_ms().saturating_sub(c.at) < DAY_MS {
            return Ok(c.id.clone());
        }
    }
    let html = text("https://soundcloud.com/").await?;
    // the id usually sits in one of the last bundles
    for url in script_urls(&html).into_iter().rev() {
        if let Some(id) = extract_client_id(&text(&url).await.unwrap_or_default()) {
            let _ = std::fs::create_dir_all(&data_dir);
            let _ = std::fs::write(&file, serde_json::to_vec(&Cached { id: id.clone(), at: now_ms() }).unwrap_or_default());
            return Ok(id);
        }
    }
    cached.map(|c| c.id).ok_or_else(|| "SoundCloud client id not found".into())
}

// ---- Spotify ------------------------------------------------------------------------
// The official authorization-code flow with PKCE, the one Spotify made for apps that cannot
// keep a secret. The user signs in on spotify.com in their own browser; Spotify sends the
// browser back to a small server on this computer with a one-time code, which is swapped
// for a token. No password ever passes through EvPlayer.

pub const REDIRECT_PORT: u16 = 43821;
const SCOPES: &str = "playlist-read-private playlist-read-collaborative user-library-read user-read-private";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyTokens {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

fn redirect_uri() -> String {
    format!("http://127.0.0.1:{REDIRECT_PORT}/callback")
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 48];
    rand::Rng::fill(&mut rand::thread_rng(), &mut bytes[..]);
    let verifier = b64url(&bytes);
    let challenge = b64url(&sha2::Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

async fn token(form: &[(&str, &str)], old_refresh: Option<&str>) -> Result<SpotifyTokens, String> {
    let res = client().post("https://accounts.spotify.com/api/token").form(form).send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    let json: serde_json::Value = res.json().await.unwrap_or_default();
    let Some(access) = json["access_token"].as_str() else {
        let msg = json["error_description"].as_str().or(json["error"].as_str()).map(str::to_string).unwrap_or(format!("Spotify ответил {status}"));
        return Err(msg);
    };
    Ok(SpotifyTokens {
        access_token: access.to_string(),
        refresh_token: json["refresh_token"].as_str().or(old_refresh).unwrap_or("").to_string(),
        expires_at: now_ms() + (json["expires_in"].as_u64().unwrap_or(3600).saturating_sub(60)) * 1000,
    })
}

const PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>EvPlayer</title><body style="margin:0;height:100vh;display:grid;place-items:center;background:#06201d;color:#eef7f4;font:16px system-ui"><div style="text-align:center"><h2>{title}</h2><p style="opacity:.7">{text}</p></div></body>"#;

fn answer(stream: &mut std::net::TcpStream, ok: bool, title: &str, text: &str) {
    let body = PAGE.replace("{title}", title).replace("{text}", text);
    let status = if ok { "200 OK" } else { "400 Bad Request" };
    let _ = write!(stream, "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
}

/// opens the Spotify sign-in page and waits (up to 5 minutes) for the user to come back
pub async fn spotify_sign_in(client_id: String, open: impl FnOnce(String)) -> Result<SpotifyTokens, String> {
    let listener = TcpListener::bind(("127.0.0.1", REDIRECT_PORT)).map_err(|e| format!("Не получилось открыть порт {REDIRECT_PORT}: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let (verifier, challenge) = pkce_pair();
    let state = b64url(&rand::random::<u128>().to_le_bytes());
    let mut auth = url::Url::parse("https://accounts.spotify.com/authorize").unwrap();
    auth.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri())
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge)
        .append_pair("scope", SCOPES)
        .append_pair("state", &state);
    open(auth.to_string());

    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if Instant::now() > deadline {
            return Err("Вход в Spotify не завершён за 5 минут".into());
        }
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
        };
        let _ = stream.set_nonblocking(false);
        let mut line = String::new();
        let _ = BufReader::new(&stream).read_line(&mut line);
        let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
        let url = url::Url::parse(&format!("http://127.0.0.1{target}")).unwrap();
        if url.path() != "/callback" {
            let _ = write!(stream, "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
            continue;
        }
        let get = |k: &str| url.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.to_string());
        let denied = get("error").as_deref() == Some("access_denied");
        let code = match (get("state"), get("code")) {
            (Some(s), Some(c)) if s == state => c,
            _ => {
                answer(&mut stream, false, "Вход не выполнен", if denied { "Вы отменили вход." } else { "Попробуйте ещё раз из EvPlayer." });
                return Err(if denied { "Вход в Spotify отменён".into() } else { "Spotify не вернул код входа".into() });
            }
        };
        let uri = redirect_uri();
        let result = token(
            &[("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", &uri), ("client_id", &client_id), ("code_verifier", &verifier)],
            None,
        )
        .await;
        match &result {
            Ok(_) => answer(&mut stream, true, "Готово", "Spotify подключён к EvPlayer. Эту вкладку можно закрыть."),
            Err(e) => answer(&mut stream, false, "Вход не выполнен", e),
        }
        return result;
    }
}

pub async fn spotify_refresh(client_id: String, refresh_token: String) -> Result<SpotifyTokens, String> {
    token(&[("grant_type", "refresh_token"), ("refresh_token", &refresh_token), ("client_id", &client_id)], Some(&refresh_token)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_client_id() {
        assert_eq!(extract_client_id(r#"x={client_id:"abcdefghijklmnopqrstuvwxyz012345"}"#).as_deref(), Some("abcdefghijklmnopqrstuvwxyz012345"));
        assert_eq!(extract_client_id("nothing"), None);
        let html = r#"<script crossorigin src="https://a-v2.sndcdn.com/assets/0-abc.js"></script><script src="https://a-v2.sndcdn.com/assets/49-def.js"></script>"#;
        assert_eq!(script_urls(html).len(), 2);
    }

    #[test]
    fn pkce_challenge_is_the_hash_of_the_verifier() {
        let (v, c) = pkce_pair();
        assert_eq!(c, b64url(&sha2::Sha256::digest(v.as_bytes())));
        assert!(v.len() >= 43);
    }
}
