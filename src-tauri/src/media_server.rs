//! Songs are played by the page's <audio> element, and not every browser engine can play
//! from an app-defined address scheme (WebKitGTK cannot; WebView2 is picky about ranges).
//! A tiny HTTP server on 127.0.0.1 works everywhere: it streams the user's own files from
//! disk and passes song streams from the music services through, both with seeking
//! (Range) support and marked readable for the equalizer.
//!
//! Every address carries a random secret, so other programs (or web pages in a browser)
//! on this computer cannot use the server.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::library::Library;
use crate::proxy::is_open_host;

pub struct MediaServer {
    /// http://127.0.0.1:PORT/SECRET
    pub base: String,
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn cors() -> Vec<Header> {
    vec![
        header("Access-Control-Allow-Origin", "*"),
        header("Access-Control-Allow-Headers", "*"),
        header("Access-Control-Expose-Headers", "Content-Length, Content-Range, Accept-Ranges"),
    ]
}

fn reply_text(req: Request, status: u16, text: &str) {
    let mut res = Response::from_string(text).with_status_code(StatusCode(status));
    for h in cors() {
        res.add_header(h);
    }
    let _ = req.respond(res);
}

fn range_of(req: &Request) -> Option<String> {
    req.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.as_str().to_string())
}

/// `bytes=a-b` → (start, end) for a file of `size` bytes; no Range means the whole file
fn file_range(range: Option<&str>, size: u64) -> Option<(u64, u64)> {
    let Some(value) = range else { return Some((0, size.saturating_sub(1))) };
    let rest = value.trim().strip_prefix("bytes=")?;
    let (a, b) = rest.split_once('-')?;
    let last = size.saturating_sub(1);
    match (a.trim(), b.trim()) {
        ("", "") => None,
        ("", suffix) => Some((size.saturating_sub(suffix.parse().ok()?), last)),
        (s, "") => Some((s.parse().ok()?, last)),
        (s, e) => Some((s.parse().ok()?, e.parse::<u64>().ok()?.min(last))),
    }
}

fn serve_file(req: Request, path: &Path) {
    let Ok(mut file) = File::open(path) else { return reply_text(req, 404, "file moved or deleted") };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let had_range = range_of(&req).is_some();
    let Some((start, end)) = file_range(range_of(&req).as_deref(), size) else { return reply_text(req, 416, "bad range") };
    if size == 0 || start > end || start >= size {
        let mut res = Response::empty(416);
        res.add_header(header("Content-Range", &format!("bytes */{size}")));
        return drop(req.respond(res));
    }
    if file.seek(SeekFrom::Start(start)).is_err() {
        return reply_text(req, 500, "could not read the file");
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let len = end - start + 1;
    let mut headers = cors();
    headers.push(header("Content-Type", crate::library::mime(&ext)));
    headers.push(header("Accept-Ranges", "bytes"));
    let status = if had_range {
        headers.push(header("Content-Range", &format!("bytes {start}-{end}/{size}")));
        206
    } else {
        200
    };
    let res = Response::new(StatusCode(status), headers, file.take(len), Some(len as usize), None);
    let _ = req.respond(res);
}

fn blocking_client() -> &'static reqwest::blocking::Client {
    static CLIENT: std::sync::OnceLock<reqwest::blocking::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(None)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36")
            .build()
            .expect("http client")
    })
}

/// how much of a YouTube song is asked from googlevideo at a time
const GOOGLE_PIECE: u64 = 1024 * 1024;

fn is_googlevideo(url: &url::Url) -> bool {
    url.host_str().map(|h| h.to_ascii_lowercase().ends_with(".googlevideo.com")).unwrap_or(false)
}

/// googlevideo gives a song in pieces, asked as `&range=a-b` in the address, to the app it
/// handed the address to. Turn the player's `Range: bytes=a-b` into one such piece:
/// (a, b, whole length). None when the address does not say how long the song is.
fn google_piece(url: &url::Url, range: Option<&str>) -> Option<(u64, u64, u64)> {
    let total: u64 = url.query_pairs().find(|(k, _)| k == "clen")?.1.parse().ok()?;
    if total == 0 {
        return None;
    }
    let (start, wanted_end) = match range.and_then(|r| r.trim().strip_prefix("bytes=")) {
        None => (0, None),
        Some(rest) => {
            let (a, b) = rest.split_once('-')?;
            match (a.trim(), b.trim()) {
                ("", suffix) => (total.saturating_sub(suffix.parse().ok()?), None),
                (s, "") => (s.parse().ok()?, None),
                (s, e) => (s.parse().ok()?, Some(e.parse::<u64>().ok()?)),
            }
        }
    };
    if start >= total {
        return None;
    }
    let end = wanted_end.unwrap_or(u64::MAX).min(start + GOOGLE_PIECE - 1).min(total - 1);
    Some((start, end, total))
}

/// googlevideo expects the app the address was made for; the address names it (`c=`)
fn google_user_agent(url: &url::Url) -> Option<&'static str> {
    match url.query_pairs().find(|(k, _)| k == "c")?.1.as_ref() {
        "ANDROID_VR" => Some("com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip"),
        "VISIONOS" => Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15"),
        _ => None,
    }
}

fn serve_google(req: Request, mut url: url::Url) {
    let Some((start, end, total)) = google_piece(&url, range_of(&req).as_deref()) else {
        return reply_text(req, 416, "bad range");
    };
    // the piece is asked in the address; a Range header on top of it is refused
    let kept: Vec<(String, String)> = url.query_pairs().filter(|(k, _)| k != "range").map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    url.query_pairs_mut().clear().extend_pairs(kept).append_pair("range", &format!("{start}-{end}"));
    let mut upstream = blocking_client().get(url.clone());
    if let Some(agent) = google_user_agent(&url) {
        upstream = upstream.header("User-Agent", agent);
    }
    let res = match upstream.send() {
        Ok(r) => r,
        Err(e) => return reply_text(req, 502, &e.to_string()),
    };
    if !res.status().is_success() {
        return reply_text(req, res.status().as_u16(), "YouTube refused the piece");
    }
    let len = end - start + 1;
    let mut headers = cors();
    let mime = res.headers().get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("audio/webm").to_string();
    headers.push(header("Content-Type", &mime));
    headers.push(header("Accept-Ranges", "bytes"));
    headers.push(header("Content-Range", &format!("bytes {start}-{end}/{total}")));
    let _ = req.respond(Response::new(StatusCode(206), headers, res, Some(len as usize), None));
}

/// a song stream from a music service, passed through piece by piece as it arrives
fn serve_remote(req: Request, encoded: &str) {
    let decoded = percent_encoding::percent_decode_str(encoded).decode_utf8_lossy().to_string();
    let Ok(url) = url::Url::parse(&decoded) else { return reply_text(req, 400, "bad address") };
    if !is_open_host(&url) {
        return reply_text(req, 403, "host not allowed");
    }
    if is_googlevideo(&url) && google_piece(&url, range_of(&req).as_deref()).is_some() {
        return serve_google(req, url);
    }
    let mut upstream = blocking_client().get(url);
    if let Some(r) = range_of(&req) {
        upstream = upstream.header("Range", r);
    }
    let res = match upstream.send() {
        Ok(r) => r,
        Err(e) => return reply_text(req, 502, &e.to_string()),
    };
    let status = res.status().as_u16();
    let mut headers = cors();
    for name in ["Content-Type", "Content-Range", "Accept-Ranges"] {
        if let Some(v) = res.headers().get(name).and_then(|v| v.to_str().ok()) {
            headers.push(header(name, v));
        }
    }
    let len = res.content_length().map(|l| l as usize);
    let _ = req.respond(Response::new(StatusCode(status), headers, res, len, None));
}

impl MediaServer {
    pub fn start(library: Arc<Library>) -> Option<Self> {
        let server = Arc::new(Server::http("127.0.0.1:0").ok()?);
        let port = server.server_addr().to_ip()?.port();
        let secret: String = {
            let mut bytes = [0u8; 16];
            rand::Rng::fill(&mut rand::thread_rng(), &mut bytes[..]);
            bytes.iter().map(|b| format!("{b:02x}")).collect()
        };
        let prefix = format!("/{secret}/");
        // a few workers: two decks, a cover or two, seeking — never many at once
        for _ in 0..6 {
            let server = server.clone();
            let library = library.clone();
            let prefix = prefix.clone();
            std::thread::spawn(move || {
                for req in server.incoming_requests() {
                    if *req.method() == Method::Options {
                        let mut res = Response::empty(204);
                        for h in cors() {
                            res.add_header(h);
                        }
                        let _ = req.respond(res);
                        continue;
                    }
                    let path = req.url().split('?').next().unwrap_or("").to_string();
                    let Some(rest) = path.strip_prefix(&prefix) else {
                        reply_text(req, 404, "not found");
                        continue;
                    };
                    match rest.split_once('/') {
                        Some(("track", id)) => {
                            let id = percent_encoding::percent_decode_str(id).decode_utf8_lossy().to_string();
                            match library.get(&id) {
                                Some(record) => serve_file(req, Path::new(&record.path)),
                                None => reply_text(req, 404, "unknown track"),
                            }
                        }
                        Some(("remote", encoded)) => serve_remote(req, encoded),
                        _ => reply_text(req, 404, "not found"),
                    }
                }
            });
        }
        Some(Self { base: format!("http://127.0.0.1:{port}/{secret}") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_pieces() {
        let url = |s: &str| url::Url::parse(s).unwrap();
        let u = url("https://rr1.googlevideo.com/videoplayback?c=ANDROID_VR&clen=3000000&itag=251");
        assert_eq!(google_piece(&u, None), Some((0, GOOGLE_PIECE - 1, 3_000_000)));
        assert_eq!(google_piece(&u, Some("bytes=0-")), Some((0, GOOGLE_PIECE - 1, 3_000_000)));
        assert_eq!(google_piece(&u, Some("bytes=100-199")), Some((100, 199, 3_000_000)));
        assert_eq!(google_piece(&u, Some("bytes=2900000-")), Some((2_900_000, 2_999_999, 3_000_000)));
        assert_eq!(google_piece(&u, Some("bytes=-1000")), Some((2_999_000, 2_999_999, 3_000_000)));
        assert_eq!(google_piece(&u, Some("bytes=3000000-")), None);
        assert_eq!(google_piece(&url("https://rr1.googlevideo.com/videoplayback?itag=251"), None), None);
        assert!(google_user_agent(&u).unwrap().contains("oculus"));
        assert!(google_user_agent(&url("https://x.googlevideo.com/v?c=WEB")).is_none());
    }

    #[test]
    fn ranges() {
        assert_eq!(file_range(None, 1000), Some((0, 999)));
        assert_eq!(file_range(Some("bytes=0-"), 1000), Some((0, 999)));
        assert_eq!(file_range(Some("bytes=100-199"), 1000), Some((100, 199)));
        assert_eq!(file_range(Some("bytes=-100"), 1000), Some((900, 999)));
        assert_eq!(file_range(Some("bytes=900-5000"), 1000), Some((900, 999)));
    }

    #[test]
    fn serves_a_file_with_ranges_and_refuses_strangers() {
        let dir = std::env::temp_dir().join("evplayer-media-test");
        let _ = std::fs::create_dir_all(&dir);
        let song = dir.join("Artist - Song.mp3");
        std::fs::write(&song, vec![7u8; 5000]).unwrap();
        let library = Library::new(dir.join("data"));
        let (added, _) = library.add(&[song.clone()]);
        let id = added.added[0].id.clone();
        let server = MediaServer::start(library).unwrap();
        let client = reqwest::blocking::Client::new();
        let url = format!("{}/track/{}", server.base, percent_encoding::utf8_percent_encode(&id, percent_encoding::NON_ALPHANUMERIC));
        let res = client.get(&url).header("Range", "bytes=100-199").send().unwrap();
        assert_eq!(res.status().as_u16(), 206);
        assert_eq!(res.headers()["content-range"], "bytes 100-199/5000");
        assert_eq!(res.headers()["access-control-allow-origin"], "*");
        assert_eq!(res.bytes().unwrap().len(), 100);
        let whole = client.get(&url).send().unwrap();
        assert_eq!(whole.status().as_u16(), 200);
        assert_eq!(whole.bytes().unwrap().len(), 5000);
        let port_only = server.base.rsplit_once('/').unwrap().0.to_string();
        assert_eq!(client.get(format!("{port_only}/wrong/track/{id}")).send().unwrap().status().as_u16(), 404);
        let blocked = client.get(format!("{}/remote/{}", server.base, "https%3A%2F%2Fexample.com%2F")).send().unwrap();
        assert_eq!(blocked.status().as_u16(), 403);
    }
}
