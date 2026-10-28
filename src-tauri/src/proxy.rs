//! The app talks to music services straight from its page. A browser would block those
//! replies (they are meant for the services' own sites), so requests to the services the
//! app uses go through this small proxy instead: the page asks
//! `evproxy://localhost/<encoded url>` (on Windows `http://evproxy.localhost/...`), Rust makes
//! the request and hands back the reply marked as readable by the app.
//! Only the hosts listed here are reachable; everything else is refused.

use std::sync::OnceLock;
use std::time::Duration;

use tauri::http::{header, HeaderMap, HeaderValue, Method, Request, Response, StatusCode};

/// the hosts the app uses; the same list lives in src/platform/net.ts
const OPEN_HOSTS: &[&str] = &[
    "*soundcloud.com",
    "*sndcdn.com",
    "*soundcloud.cloud",
    "*youtube.com",
    "*googlevideo.com",
    "*ytimg.com",
    "*ggpht.com",
    "*googleusercontent.com",
    "*lrclib.net",
    "*genius.com",
    "api.deezer.com",
    "open.spotify.com",
    "accounts.spotify.com",
    "api.spotify.com",
    "api-partner.spotify.com",
    "clienttoken.spotify.com",
    "open.spotifycdn.com",
    "api.music.yandex.net",
    "api.github.com",
];

/// a media file is handed over in pieces of this size, so playback starts without
/// waiting for the whole song
const MEDIA_CHUNK: u64 = 1024 * 1024;

pub fn is_open_host(url: &url::Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else { return false };
    let host = host.to_ascii_lowercase();
    OPEN_HOSTS.iter().any(|pattern| match pattern.strip_prefix('*') {
        Some(domain) => host == domain || host.ends_with(&format!(".{domain}")),
        None => host == *pattern,
    })
}

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .cookie_store(true)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36")
            .build()
            .expect("http client")
    })
}

fn cors(headers: &mut HeaderMap) {
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("*"));
    headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, HEAD, POST, PUT, DELETE, OPTIONS"));
    headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_static("*"));
}

fn plain(status: StatusCode, text: &str) -> Response<Vec<u8>> {
    let mut res = Response::builder().status(status).body(text.as_bytes().to_vec()).unwrap();
    cors(res.headers_mut());
    res
}

/// `bytes=N-` asks for the rest of the file: ask for one piece instead
pub fn limit_range(value: &str, chunk: u64) -> Option<String> {
    let rest = value.trim().strip_prefix("bytes=")?;
    let (start, end) = rest.split_once('-')?;
    if !end.trim().is_empty() || start.contains(',') {
        return None;
    }
    let start: u64 = start.trim().parse().ok()?;
    Some(format!("bytes={}-{}", start, start + chunk - 1))
}

/// the address the page asked for: everything after the first slash, percent-encoded
pub fn target_of(request: &Request<Vec<u8>>) -> Option<url::Url> {
    let path = request.uri().path().trim_start_matches('/');
    let decoded = percent_encoding::percent_decode_str(path).decode_utf8().ok()?;
    let mut url = url::Url::parse(&decoded).ok()?;
    // a query added after the encoded address (cache busting) belongs to the target
    if let Some(extra) = request.uri().query() {
        let joined = match url.query() {
            Some(q) if !q.is_empty() => format!("{q}&{extra}"),
            _ => extra.to_string(),
        };
        url.set_query(Some(&joined));
    }
    Some(url)
}

const SKIP_REQUEST_HEADERS: &[&str] = &["host", "origin", "referer", "connection", "content-length", "accept-encoding", "cookie"];
const PASS_RESPONSE_HEADERS: &[header::HeaderName] = &[
    header::CONTENT_TYPE,
    header::CONTENT_RANGE,
    header::ACCEPT_RANGES,
    header::CACHE_CONTROL,
    header::ETAG,
    header::LAST_MODIFIED,
    header::CONTENT_DISPOSITION,
];

pub async fn handle(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() == Method::OPTIONS {
        return plain(StatusCode::NO_CONTENT, "");
    }
    let Some(url) = target_of(&request) else { return plain(StatusCode::BAD_REQUEST, "bad address") };
    if !is_open_host(&url) {
        return plain(StatusCode::FORBIDDEN, "host not allowed");
    }
    let method = reqwest::Method::from_bytes(request.method().as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut builder = client().request(method, url);
    for (name, value) in request.headers() {
        let lower = name.as_str().to_ascii_lowercase();
        if SKIP_REQUEST_HEADERS.contains(&lower.as_str()) || lower.starts_with("sec-") {
            continue;
        }
        // the page cannot set a User-Agent itself: it sends the one it wants in this header
        if lower == "x-evplayer-user-agent" {
            builder = builder.header("user-agent", value.as_bytes());
            continue;
        }
        if lower == "range" {
            if let Some(limited) = value.to_str().ok().and_then(|v| limit_range(v, MEDIA_CHUNK)) {
                builder = builder.header("range", limited);
                continue;
            }
        }
        builder = builder.header(name.as_str(), value.as_bytes());
    }
    if !request.body().is_empty() {
        builder = builder.body(request.body().clone());
    }
    let upstream = match builder.send().await {
        Ok(res) => res,
        Err(e) => return plain(StatusCode::BAD_GATEWAY, &e.to_string()),
    };
    let status = upstream.status().as_u16();
    let mut out = Response::builder().status(status);
    let headers = out.headers_mut().unwrap();
    for name in PASS_RESPONSE_HEADERS {
        if let Some(value) = upstream.headers().get(name.as_str()) {
            if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                headers.insert(name.clone(), v);
            }
        }
    }
    cors(headers);
    let body = match upstream.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => return plain(StatusCode::BAD_GATEWAY, &e.to_string()),
    };
    out.body(body).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_hosts() {
        let ok = |s: &str| is_open_host(&url::Url::parse(s).unwrap());
        assert!(ok("https://api-v2.soundcloud.com/search?q=a"));
        assert!(ok("https://music.youtube.com/youtubei/v1/search"));
        assert!(ok("https://api.spotify.com/v1/me"));
        assert!(ok("https://api-partner.spotify.com/pathfinder/v1/query"));
        assert!(ok("https://clienttoken.spotify.com/v1/clienttoken"));
        assert!(ok("https://open.spotifycdn.com/cdn/build/web-player/web-player.abc.js"));
        assert!(!ok("https://evil.com/?soundcloud.com"));
        assert!(!ok("https://soundcloud.com.evil.com/"));
        assert!(!ok("http://api-v2.soundcloud.com/"));
        assert!(!ok("https://spotify.com/"));
    }

    #[test]
    fn open_ranges_become_pieces() {
        assert_eq!(limit_range("bytes=0-", 100).as_deref(), Some("bytes=0-99"));
        assert_eq!(limit_range("bytes=500-", 100).as_deref(), Some("bytes=500-599"));
        assert_eq!(limit_range("bytes=0-10", 100), None);
        assert_eq!(limit_range("bytes=-500", 100), None);
    }

    #[test]
    fn reads_the_encoded_target() {
        let req = Request::builder()
            .uri("http://evproxy.localhost/https%3A%2F%2Fapi-v2.soundcloud.com%2Fsearch%3Fq%3Da%2520b")
            .body(Vec::new())
            .unwrap();
        assert_eq!(target_of(&req).unwrap().as_str(), "https://api-v2.soundcloud.com/search?q=a%20b");
    }
}
