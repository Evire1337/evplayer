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

// ---- HLS pieces joined into one file ---------------------------------------------------
//
// Some SoundCloud songs come (unprotected) only as Opus in HLS: a playlist of pieces cut
// from one Ogg file. The HLS player in the page cannot put Ogg pieces together, but the
// <audio> element plays an Ogg file fine. So the pieces are joined back into that file
// here, with its full length known and Range support, so seeking works as with any file.

/// a joined stream: its pieces with their sizes, in order
struct Joined {
    parts: Vec<(String, u64)>,
    total: u64,
}

/// the pieces of an HLS media playlist; protected (EXT-X-KEY) or fMP4 (EXT-X-MAP) ones are
/// not for joining
fn playlist_pieces(text: &str, base: &url::Url) -> Result<Vec<url::Url>, String> {
    if !text.trim_start().starts_with("#EXTM3U") {
        return Err("not a playlist".into());
    }
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with("#EXT-X-KEY") && !line.contains("METHOD=NONE") {
            return Err("protected stream".into());
        }
        if line.starts_with("#EXT-X-MAP") || line.starts_with("#EXT-X-BYTERANGE") || line.starts_with("#EXT-X-STREAM-INF") {
            return Err("stream layout not supported".into());
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        out.push(base.join(line).map_err(|e| e.to_string())?);
    }
    if out.is_empty() {
        return Err("empty playlist".into());
    }
    Ok(out)
}

/// a piece's size, from the total in the Content-Range of a one-byte request
fn piece_size(url: &str) -> Result<u64, String> {
    let res = blocking_client().get(url).header("Range", "bytes=0-0").send().map_err(|e| e.to_string())?;
    let total = res
        .headers()
        .get("Content-Range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit_once('/'))
        .and_then(|(_, t)| t.trim().parse::<u64>().ok());
    match (res.status().as_u16(), total) {
        (206, Some(t)) => Ok(t),
        (200, _) => res.content_length().ok_or_else(|| "no length".to_string()),
        (code, _) => Err(format!("piece answered {code}")),
    }
}

fn joined_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, Arc<Joined>>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Arc<Joined>>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn load_joined(url: &url::Url, allowed: fn(&url::Url) -> bool) -> Result<Arc<Joined>, String> {
    let key = url.to_string();
    if let Some(j) = joined_cache().lock().unwrap().get(&key) {
        return Ok(j.clone());
    }
    let text = blocking_client().get(url.clone()).send().and_then(|r| r.error_for_status()).and_then(|r| r.text()).map_err(|e| e.to_string())?;
    let pieces = playlist_pieces(&text, url)?;
    if !pieces.iter().all(allowed) {
        return Err("piece on a host that is not allowed".into());
    }
    // sizes are asked a few at a time: a song has a few dozen pieces
    let urls: Vec<String> = pieces.iter().map(|u| u.to_string()).collect();
    let mut sizes = vec![0u64; urls.len()];
    for (chunk_urls, chunk_sizes) in urls.chunks(6).zip(sizes.chunks_mut(6)) {
        let results: Vec<Result<u64, String>> = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk_urls.iter().map(|u| scope.spawn(move || piece_size(u))).collect();
            handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("failed".into()))).collect()
        });
        for (slot, r) in chunk_sizes.iter_mut().zip(results) {
            *slot = r?;
        }
    }
    let total = sizes.iter().sum();
    let joined = Arc::new(Joined { parts: urls.into_iter().zip(sizes).collect(), total });
    let mut cache = joined_cache().lock().unwrap();
    if cache.len() > 20 {
        cache.clear();
    }
    cache.insert(key, joined.clone());
    Ok(joined)
}

/// reads the pieces that cover a byte range of the joined file, one after another
struct JoinedReader {
    /// (address, first byte, last byte) within each piece
    queue: std::collections::VecDeque<(String, u64, u64)>,
    current: Option<reqwest::blocking::Response>,
}

impl Read for JoinedReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.current.is_none() {
                let Some((url, a, b)) = self.queue.pop_front() else { return Ok(0) };
                let res = blocking_client()
                    .get(&url)
                    .header("Range", format!("bytes={a}-{b}"))
                    .send()
                    .and_then(|r| r.error_for_status())
                    .map_err(std::io::Error::other)?;
                self.current = Some(res);
            }
            let n = self.current.as_mut().unwrap().read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            self.current = None;
        }
    }
}

/// the pieces (and the bytes within them) for `start..=end` of the joined file
fn pieces_for(parts: &[(String, u64)], start: u64, end: u64) -> std::collections::VecDeque<(String, u64, u64)> {
    let mut out = std::collections::VecDeque::new();
    let mut offset = 0u64;
    for (url, size) in parts {
        let (from, to) = (offset, offset + size - 1);
        offset += size;
        if *size == 0 || to < start || from > end {
            continue;
        }
        out.push_back((url.clone(), start.max(from) - from, end.min(to) - from));
    }
    out
}

fn serve_joined(req: Request, encoded: &str, allowed: fn(&url::Url) -> bool) {
    let decoded = percent_encoding::percent_decode_str(encoded).decode_utf8_lossy().to_string();
    let Ok(url) = url::Url::parse(&decoded) else { return reply_text(req, 400, "bad address") };
    if !allowed(&url) {
        return reply_text(req, 403, "host not allowed");
    }
    let joined = match load_joined(&url, allowed) {
        Ok(j) => j,
        Err(e) => return reply_text(req, 502, &e),
    };
    let range = range_of(&req);
    let Some((start, end)) = file_range(range.as_deref(), joined.total).filter(|(s, e)| s <= e && *s < joined.total) else {
        let mut res = Response::empty(416);
        res.add_header(header("Content-Range", &format!("bytes */{}", joined.total)));
        let _ = req.respond(res);
        return;
    };
    let mut headers = cors();
    headers.push(header("Content-Type", "audio/ogg"));
    headers.push(header("Accept-Ranges", "bytes"));
    let status = if range.is_some() {
        headers.push(header("Content-Range", &format!("bytes {start}-{end}/{}", joined.total)));
        206
    } else {
        200
    };
    let reader = JoinedReader { queue: pieces_for(&joined.parts, start, end), current: None };
    let _ = req.respond(Response::new(StatusCode(status), headers, reader, Some((end - start + 1) as usize), None));
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
                        Some(("joined", encoded)) => serve_joined(req, encoded, is_open_host),
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
    fn joined_pieces() {
        let base = url::Url::parse("https://cdn.example/media/list.m3u8?x=1").unwrap();
        let list = "#EXTM3U
#EXT-X-TARGETDURATION:10
#EXTINF:10,
/media/0/100/a.opus?s=1
#EXTINF:10,
b.opus
#EXT-X-ENDLIST
";
        let p = playlist_pieces(list, &base).unwrap();
        assert_eq!(p[0].as_str(), "https://cdn.example/media/0/100/a.opus?s=1");
        assert_eq!(p[1].as_str(), "https://cdn.example/media/b.opus");
        assert!(playlist_pieces("#EXTM3U
#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"k\"
x.ts
", &base).is_err());
        assert!(playlist_pieces("#EXTM3U
#EXT-X-MAP:URI=\"init.mp4\"
x.m4s
", &base).is_err());
        let parts = vec![("a".to_string(), 100), ("b".to_string(), 50), ("c".to_string(), 30)];
        assert_eq!(pieces_for(&parts, 0, 179), [("a".into(), 0, 99), ("b".into(), 0, 49), ("c".into(), 0, 29)].into_iter().collect::<std::collections::VecDeque<(String, u64, u64)>>());
        assert_eq!(pieces_for(&parts, 120, 139), [("b".into(), 20, 39)].into_iter().collect::<std::collections::VecDeque<(String, u64, u64)>>());
        assert_eq!(pieces_for(&parts, 90, 160), [("a".into(), 90, 99), ("b".into(), 0, 49), ("c".into(), 0, 10)].into_iter().collect::<std::collections::VecDeque<(String, u64, u64)>>());
    }

    #[test]
    fn joins_pieces_with_ranges() {
        // a little HLS host: a playlist and three pieces
        let host = Server::http("127.0.0.1:0").unwrap();
        let port = host.server_addr().to_ip().unwrap().port();
        std::thread::spawn(move || {
            for req in host.incoming_requests() {
                let path = req.url().to_string();
                let body: Vec<u8> = match path.as_str() {
                    "/list.m3u8" => b"#EXTM3U\n#EXTINF:1,\np0\n#EXTINF:1,\np1\n#EXTINF:1,\np2\n#EXT-X-ENDLIST\n".to_vec(),
                    "/p0" => vec![b'a'; 10],
                    "/p1" => vec![b'b'; 20],
                    "/p2" => vec![b'c'; 5],
                    _ => vec![],
                };
                let size = body.len() as u64;
                let range = req.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.as_str().to_string());
                match file_range(range.as_deref(), size) {
                    Some((a, b)) if range.is_some() => {
                        let slice = body[a as usize..=b as usize].to_vec();
                        let res = Response::from_data(slice).with_status_code(206).with_header(header("Content-Range", &format!("bytes {a}-{b}/{size}")));
                        let _ = req.respond(res);
                    }
                    _ => {
                        let _ = req.respond(Response::from_data(body));
                    }
                }
            }
        });
        let library = Library::new(std::env::temp_dir().join("evplayer-joined-test"));
        let server = MediaServer::start(library).unwrap();
        // the real route only allows the music services; this test calls the handler's parts
        let playlist = url::Url::parse(&format!("http://127.0.0.1:{port}/list.m3u8")).unwrap();
        let joined = load_joined(&playlist, |_| true).unwrap();
        assert_eq!(joined.total, 35);
        let mut all = String::new();
        JoinedReader { queue: pieces_for(&joined.parts, 0, 34), current: None }.read_to_string(&mut all).unwrap();
        assert_eq!(all, format!("{}{}{}", "a".repeat(10), "b".repeat(20), "c".repeat(5)));
        let mut mid = String::new();
        JoinedReader { queue: pieces_for(&joined.parts, 8, 31), current: None }.read_to_string(&mut mid).unwrap();
        assert_eq!(mid, format!("aa{}cc", "b".repeat(20)));
        // and the route itself refuses hosts that are not music services
        let blocked = reqwest::blocking::get(format!("{}/joined/{}", server.base, "http%3A%2F%2F127.0.0.1%2Flist.m3u8")).unwrap();
        assert_eq!(blocked.status().as_u16(), 403);
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
