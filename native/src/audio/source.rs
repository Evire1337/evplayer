//! Where a song's bytes come from: a file, an address on a music service, an HLS playlist.
//!
//! A song from the network is fetched in pieces into a temporary file that the system deletes
//! by itself when the song is let go (it is opened delete-on-close and marked temporary, so it
//! normally never even reaches the disk). The decoder reads from that file: memory stays small
//! however long the song, seeking back costs nothing, and seeking ahead past what has arrived
//! asks for that piece first. googlevideo gives a song only in pieces asked as `&range=a-b`
//! with the user agent of the app the address was made for (media_server.rs explains).

use std::collections::VecDeque;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use symphonia::core::io::MediaSource;

use crate::media_server::{blocking_client, google_piece_url, google_user_agent, is_googlevideo};

/// how much is asked for at a time (googlevideo answers slowly to bigger pieces)
const PIECE: u64 = 512 * 1024;
const GOOGLE_PIECE: u64 = 1024 * 1024;
/// a reader waiting for a piece looks whether it was let go this often
const WAIT_STEP: Duration = Duration::from_millis(200);
/// a piece that does not come in this long fails the song (the player tries another source)
const PIECE_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Clone, Debug)]
pub enum Spec {
    File(PathBuf),
    /// a whole file at an address (SoundCloud mp3, YouTube's m4a or webm)
    Http { url: String, user_agent: Option<String> },
    /// an HLS media playlist; its pieces are joined into one stream
    Hls { url: String },
}

impl Spec {
    /// what the decoder is told about the format, from the address or the name
    pub fn extension(&self) -> Option<String> {
        let path = match self {
            Spec::File(p) => return p.extension().map(|e| e.to_string_lossy().to_lowercase()),
            Spec::Http { url, .. } | Spec::Hls { url } => url::Url::parse(url).ok()?.path().to_string(),
        };
        let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
        (ext.len() <= 5).then_some(ext)
    }
}

fn io_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

// ---- a piece-by-piece download into a temporary file --------------------------------------

struct State {
    /// which pieces are in the file
    have: Vec<bool>,
    /// the piece a reader is waiting for (fetched before anything else)
    want: Option<usize>,
    /// the download stopped for good with this
    error: Option<String>,
}

struct Remote {
    url: url::Url,
    user_agent: Option<String>,
    google: bool,
    total: u64,
    piece: u64,
    file: std::fs::File,
    state: Mutex<State>,
    arrived: Condvar,
    closed: AtomicBool,
}

#[cfg(windows)]
fn temp_file() -> io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_ATTRIBUTE_TEMPORARY: u32 = 0x100;
    const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
    let dir = std::env::temp_dir().join("EvPlayer");
    let _ = std::fs::create_dir_all(&dir);
    let name = format!("stream-{}-{:016x}.part", std::process::id(), rand::random::<u64>());
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .attributes(FILE_ATTRIBUTE_TEMPORARY)
        .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
        .open(dir.join(name))
}

#[cfg(not(windows))]
fn temp_file() -> io::Result<std::fs::File> {
    let path = std::env::temp_dir().join(format!("evplayer-stream-{:016x}.part", rand::random::<u64>()));
    let f = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&path)?;
    let _ = std::fs::remove_file(&path);
    Ok(f)
}

#[cfg(windows)]
fn write_at(f: &std::fs::File, buf: &[u8], at: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    let mut done = 0;
    while done < buf.len() {
        done += f.seek_write(&buf[done..], at + done as u64)?;
    }
    Ok(())
}

#[cfg(windows)]
fn read_at(f: &std::fs::File, buf: &mut [u8], at: u64) -> io::Result<usize> {
    use std::os::windows::fs::FileExt;
    f.seek_read(buf, at)
}

#[cfg(unix)]
fn write_at(f: &std::fs::File, buf: &[u8], at: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(f, buf, at)
}

#[cfg(unix)]
fn read_at(f: &std::fs::File, buf: &mut [u8], at: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(f, buf, at)
}

/// the length of a song at an address, and whether the first answer already holds the start
fn probe_length(url: &url::Url, user_agent: Option<&str>) -> Result<u64, String> {
    if is_googlevideo(url) {
        if let Some(total) = url.query_pairs().find(|(k, _)| k == "clen").and_then(|(_, v)| v.parse().ok()) {
            return Ok(total);
        }
    }
    let mut req = blocking_client().get(url.clone()).header("Range", "bytes=0-0");
    if let Some(ua) = user_agent {
        req = req.header("User-Agent", ua);
    }
    let res = req.send().map_err(|e| e.to_string())?;
    let status = res.status().as_u16();
    if !res.status().is_success() {
        return Err(format!("сервис ответил {status}"));
    }
    let from_range = res.headers().get("Content-Range").and_then(|v| v.to_str().ok()).and_then(|v| v.rsplit_once('/')).and_then(|(_, t)| t.trim().parse::<u64>().ok());
    match (status, from_range) {
        (206, Some(t)) => Ok(t),
        _ => res.content_length().filter(|l| *l > 1).ok_or_else(|| "длина потока неизвестна".to_string()),
    }
}

impl Remote {
    fn open(url: &str, user_agent: Option<String>) -> Result<Arc<Self>, String> {
        let url = url::Url::parse(url).map_err(|e| e.to_string())?;
        if !crate::proxy::is_open_host(&url) {
            return Err("адрес не из списка разрешённых".into());
        }
        let google = is_googlevideo(&url);
        let user_agent = user_agent.or_else(|| google_user_agent(&url).map(str::to_string));
        let total = probe_length(&url, user_agent.as_deref())?;
        let piece = if google { GOOGLE_PIECE } else { PIECE };
        let pieces = total.div_ceil(piece) as usize;
        let file = temp_file().map_err(|e| e.to_string())?;
        let me = Arc::new(Self {
            url,
            user_agent,
            google,
            total,
            piece,
            file,
            state: Mutex::new(State { have: vec![false; pieces], want: None, error: None }),
            arrived: Condvar::new(),
            closed: AtomicBool::new(false),
        });
        let worker = me.clone();
        std::thread::Builder::new().name("stream download".into()).spawn(move || worker.download()).map_err(|e| e.to_string())?;
        Ok(me)
    }

    fn fetch_piece(&self, i: usize) -> Result<(), String> {
        let start = i as u64 * self.piece;
        let end = (start + self.piece).min(self.total) - 1;
        let mut req = if self.google {
            blocking_client().get(google_piece_url(&self.url, start, end))
        } else {
            blocking_client().get(self.url.clone()).header("Range", format!("bytes={start}-{end}"))
        };
        if let Some(ua) = &self.user_agent {
            req = req.header("User-Agent", ua);
        }
        let mut res = req.timeout(PIECE_TIMEOUT).send().map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("сервис ответил {}", res.status().as_u16()));
        }
        // a server that ignored the range and sent the whole file from the start
        let whole = res.status().as_u16() == 200 && !self.google;
        let mut at = if whole { 0 } else { start };
        let stop = end + 1;
        let mut buf = vec![0u8; 64 * 1024];
        while at < stop {
            if self.closed.load(Ordering::Relaxed) {
                return Ok(());
            }
            let n = res.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            let n = n.min((stop - at) as usize);
            if at + n as u64 > start {
                let skip = start.saturating_sub(at) as usize;
                write_at(&self.file, &buf[skip..n], at.max(start)).map_err(|e| e.to_string())?;
            }
            at += n as u64;
        }
        if at < stop {
            return Err("поток оборвался".into());
        }
        Ok(())
    }

    /// pieces from where the reader is, the one it waits for first, until the song is all here
    fn download(self: Arc<Self>) {
        let mut next = 0usize;
        let mut failures = 0;
        loop {
            if self.closed.load(Ordering::Relaxed) {
                return;
            }
            let pick = {
                let mut st = self.state.lock().unwrap();
                let wanted = st.want.take().filter(|&w| !st.have[w]);
                match wanted {
                    Some(w) => Some(w),
                    None => (next..st.have.len()).find(|&i| !st.have[i]),
                }
            };
            let Some(i) = pick else { return };
            match self.fetch_piece(i) {
                Ok(()) => {
                    failures = 0;
                    next = i + 1;
                    self.state.lock().unwrap().have[i] = true;
                    self.arrived.notify_all();
                }
                Err(e) => {
                    failures += 1;
                    if failures >= 3 {
                        self.state.lock().unwrap().error = Some(e);
                        self.arrived.notify_all();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(400 * failures));
                }
            }
        }
    }

    /// wait until the piece holding `pos` is here
    fn wait_for(&self, pos: u64) -> io::Result<()> {
        let i = (pos / self.piece) as usize;
        let mut st = self.state.lock().unwrap();
        let mut waited = Duration::ZERO;
        while !st.have[i] {
            if let Some(e) = &st.error {
                return Err(io_err(e));
            }
            if self.closed.load(Ordering::Relaxed) {
                return Err(io_err("закрыт"));
            }
            if waited > PIECE_TIMEOUT * 2 {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "поток не идёт"));
            }
            st.want = Some(i);
            let (s, _) = self.arrived.wait_timeout(st, WAIT_STEP).unwrap();
            st = s;
            waited += WAIT_STEP;
        }
        Ok(())
    }

    /// how much of the song is here, 0..1 (for the progress bar's buffered part)
    pub fn fetched(&self) -> f32 {
        let st = self.state.lock().unwrap();
        let n = st.have.iter().filter(|h| **h).count();
        if st.have.is_empty() {
            0.0
        } else {
            n as f32 / st.have.len() as f32
        }
    }
}

pub struct RemoteReader {
    remote: Arc<Remote>,
    pos: u64,
}

impl Drop for RemoteReader {
    fn drop(&mut self) {
        self.remote.closed.store(true, Ordering::Relaxed);
        self.remote.arrived.notify_all();
    }
}

impl Read for RemoteReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let r = &self.remote;
        if self.pos >= r.total || buf.is_empty() {
            return Ok(0);
        }
        r.wait_for(self.pos)?;
        // up to the end of this piece
        let piece_end = ((self.pos / r.piece) + 1) * r.piece;
        let n = (buf.len() as u64).min(piece_end.min(r.total) - self.pos) as usize;
        let got = read_at(&r.file, &mut buf[..n], self.pos)?;
        self.pos += got as u64;
        Ok(got)
    }
}

impl Seek for RemoteReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let total = self.remote.total as i64;
        let next = match to {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(d) => total + d,
            SeekFrom::Current(d) => self.pos as i64 + d,
        };
        if next < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "before the start"));
        }
        self.pos = next as u64;
        Ok(self.pos)
    }
}

impl MediaSource for RemoteReader {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.remote.total)
    }
}

// ---- HLS -------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    /// EXT-X-MAP: the fMP4 header every piece needs in front of it
    pub init: Option<url::Url>,
    /// (address, seconds)
    pub pieces: Vec<(url::Url, f64)>,
}

/// an HLS media playlist; protected streams are refused
pub fn parse_playlist(text: &str, base: &url::Url) -> Result<Playlist, String> {
    if !text.trim_start().starts_with("#EXTM3U") {
        return Err("не плейлист".into());
    }
    let mut init = None;
    let mut pieces = Vec::new();
    let mut seconds = 0.0;
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#EXT-X-KEY:") {
            if !rest.contains("METHOD=NONE") {
                return Err("защищённый поток".into());
            }
        } else if let Some(rest) = line.strip_prefix("#EXT-X-MAP:") {
            let uri = rest.split("URI=\"").nth(1).and_then(|r| r.split('"').next()).ok_or("EXT-X-MAP без адреса")?;
            init = Some(base.join(uri).map_err(|e| e.to_string())?);
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            seconds = rest.split(',').next().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
        } else if line.starts_with("#EXT-X-STREAM-INF") || line.starts_with("#EXT-X-BYTERANGE") {
            return Err("такой вид HLS не поддерживается".into());
        } else if !line.is_empty() && !line.starts_with('#') {
            pieces.push((base.join(line).map_err(|e| e.to_string())?, seconds));
            seconds = 0.0;
        }
    }
    if pieces.is_empty() {
        return Err("пустой плейлист".into());
    }
    Ok(Playlist { init, pieces })
}

pub fn fetch_playlist(url: &str) -> Result<Playlist, String> {
    let url = url::Url::parse(url).map_err(|e| e.to_string())?;
    if !crate::proxy::is_open_host(&url) {
        return Err("адрес не из списка разрешённых".into());
    }
    let text = blocking_client().get(url.clone()).timeout(PIECE_TIMEOUT).send().and_then(|r| r.error_for_status()).and_then(|r| r.text()).map_err(|e| e.to_string())?;
    let list = parse_playlist(&text, &url)?;
    if !list.pieces.iter().all(|(u, _)| crate::proxy::is_open_host(u)) {
        return Err("кусок на неразрешённом адресе".into());
    }
    Ok(list)
}

/// the pieces of a playlist, from one on, read one after another as a single stream (the next
/// piece is fetched while the one before plays)
pub struct HlsReader {
    queue: VecDeque<url::Url>,
    current: Option<io::Cursor<Vec<u8>>>,
    ahead: Option<std::thread::JoinHandle<Result<Vec<u8>, String>>>,
}

fn get_piece(url: url::Url) -> Result<Vec<u8>, String> {
    let mut tries = 0;
    loop {
        let res = blocking_client().get(url.clone()).timeout(PIECE_TIMEOUT).send().and_then(|r| r.error_for_status()).and_then(|r| r.bytes());
        match res {
            Ok(b) => return Ok(b.to_vec()),
            Err(e) if tries >= 2 => return Err(e.to_string()),
            Err(_) => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

impl HlsReader {
    pub fn new(list: &Playlist, from: usize) -> Self {
        let mut queue: VecDeque<url::Url> = list.pieces.iter().skip(from).map(|(u, _)| u.clone()).collect();
        if let Some(init) = &list.init {
            queue.push_front(init.clone());
        }
        let mut me = Self { queue, current: None, ahead: None };
        me.start_next();
        me
    }

    fn start_next(&mut self) {
        if let Some(url) = self.queue.pop_front() {
            self.ahead = Some(std::thread::spawn(move || get_piece(url)));
        }
    }
}

impl Read for HlsReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if let Some(cur) = &mut self.current {
                let n = cur.read(buf)?;
                if n > 0 {
                    return Ok(n);
                }
                self.current = None;
            }
            let Some(next) = self.ahead.take() else { return Ok(0) };
            let data = next.join().map_err(|_| io_err("кусок не загрузился"))?.map_err(io_err)?;
            self.current = Some(io::Cursor::new(data));
            self.start_next();
        }
    }
}

impl Seek for HlsReader {
    fn seek(&mut self, _to: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "a joined HLS stream is read straight through"))
    }
}

impl MediaSource for HlsReader {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

/// where a playlist must be started to land on `seconds`: (piece, seconds at its start)
pub fn piece_at(list: &Playlist, seconds: f64) -> (usize, f64) {
    let mut start = 0.0;
    for (i, (_, d)) in list.pieces.iter().enumerate() {
        if start + d > seconds {
            return (i, start);
        }
        start += d;
    }
    (list.pieces.len().saturating_sub(1), start - list.pieces.last().map(|p| p.1).unwrap_or(0.0))
}

// ---- opening ---------------------------------------------------------------------------------

/// an opened source: what the decoder reads, and for HLS the playlist (to start it elsewhere)
pub struct Opened {
    pub media: Box<dyn MediaSource>,
    pub playlist: Option<Playlist>,
    /// seconds into the song at the start of `media` (HLS started from a later piece)
    pub starts_at: f64,
    pub remote: Option<Arc<RemoteHandle>>,
}

/// a look at a download in progress, for the interface
pub struct RemoteHandle(Arc<Remote>);

impl RemoteHandle {
    pub fn fetched(&self) -> f32 {
        self.0.fetched()
    }
}

pub fn open(spec: &Spec) -> Result<Opened, String> {
    match spec {
        Spec::File(path) => {
            let f = std::fs::File::open(path).map_err(|e| format!("файл не открылся: {e}"))?;
            Ok(Opened { media: Box::new(f), playlist: None, starts_at: 0.0, remote: None })
        }
        Spec::Http { url, user_agent } => {
            let remote = Remote::open(url, user_agent.clone())?;
            let handle = Arc::new(RemoteHandle(remote.clone()));
            Ok(Opened { media: Box::new(RemoteReader { remote, pos: 0 }), playlist: None, starts_at: 0.0, remote: Some(handle) })
        }
        Spec::Hls { url } => {
            let list = fetch_playlist(url)?;
            let reader = HlsReader::new(&list, 0);
            Ok(Opened { media: Box::new(reader), playlist: Some(list), starts_at: 0.0, remote: None })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_playlist() {
        let base = url::Url::parse("https://cf-hls-media.sndcdn.com/playlist/abc.128.mp3/playlist.m3u8?x=1").unwrap();
        let text = "#EXTM3U\n#EXT-X-TARGETDURATION:10\n#EXTINF:9.95,\nhttps://cf-hls-media.sndcdn.com/media/0/199/a.mp3?s=1\n#EXTINF:10.0,\n/media/199/398/b.mp3\n#EXT-X-ENDLIST\n";
        let p = parse_playlist(text, &base).unwrap();
        assert_eq!(p.pieces.len(), 2);
        assert_eq!(p.pieces[1].0.as_str(), "https://cf-hls-media.sndcdn.com/media/199/398/b.mp3");
        assert!((p.pieces[0].1 - 9.95).abs() < 1e-9);
        assert_eq!(p.init, None);
        let fmp4 = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:6,\nseg1.m4s\n";
        assert_eq!(parse_playlist(fmp4, &base).unwrap().init.unwrap().as_str(), "https://cf-hls-media.sndcdn.com/playlist/abc.128.mp3/init.mp4");
        assert!(parse_playlist("#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\nx.ts\n", &base).is_err());
    }

    #[test]
    fn finds_the_piece_for_a_time() {
        let u = url::Url::parse("https://a.sndcdn.com/x").unwrap();
        let list = Playlist { init: None, pieces: vec![(u.clone(), 10.0), (u.clone(), 10.0), (u, 5.0)] };
        assert_eq!(piece_at(&list, 0.0), (0, 0.0));
        assert_eq!(piece_at(&list, 12.5), (1, 10.0));
        assert_eq!(piece_at(&list, 24.0), (2, 20.0));
        assert_eq!(piece_at(&list, 99.0).0, 2);
    }

    #[test]
    fn names_the_format() {
        assert_eq!(Spec::Http { url: "https://cf-media.sndcdn.com/abc.128.mp3?Policy=x".into(), user_agent: None }.extension().as_deref(), Some("mp3"));
        assert_eq!(Spec::File(PathBuf::from("C:/m/Song.FLAC")).extension().as_deref(), Some("flac"));
    }

    /// a little range-serving host on this computer: the download writes the pieces into the
    /// temporary file and reads give back the same bytes, in order and after seeks
    #[test]
    fn downloads_in_pieces_and_reads_back() {
        let body: Vec<u8> = (0..(PIECE as usize * 2 + 1000)).map(|i| (i % 251) as u8).collect();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let data = body.clone();
        std::thread::spawn(move || {
            for req in server.incoming_requests() {
                let range = req.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.as_str().to_string()).unwrap_or_default();
                let (a, b) = range.trim_start_matches("bytes=").split_once('-').map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap_or(data.len() - 1))).unwrap_or((0, data.len() - 1));
                let b = b.min(data.len() - 1);
                let res = tiny_http::Response::from_data(data[a..=b].to_vec())
                    .with_status_code(206)
                    .with_header(tiny_http::Header::from_bytes(&b"Content-Range"[..], format!("bytes {a}-{b}/{}", data.len()).as_bytes()).unwrap());
                let _ = req.respond(res);
            }
        });
        // the whitelist keeps local addresses out; the test opens the remote by hand
        let url = url::Url::parse(&format!("http://127.0.0.1:{port}/song.mp3")).unwrap();
        let total = probe_length(&url, None).unwrap();
        assert_eq!(total, body.len() as u64);
        let pieces = total.div_ceil(PIECE) as usize;
        let remote = Arc::new(Remote {
            url,
            user_agent: None,
            google: false,
            total,
            piece: PIECE,
            file: temp_file().unwrap(),
            state: Mutex::new(State { have: vec![false; pieces], want: None, error: None }),
            arrived: Condvar::new(),
            closed: AtomicBool::new(false),
        });
        let worker = remote.clone();
        std::thread::spawn(move || worker.download());
        let mut r = RemoteReader { remote, pos: 0 };
        r.seek(SeekFrom::Start(PIECE + 10)).unwrap();
        let mut b = [0u8; 100];
        r.read_exact(&mut b).unwrap();
        assert_eq!(&b[..], &body[PIECE as usize + 10..PIECE as usize + 110]);
        r.seek(SeekFrom::Start(0)).unwrap();
        let mut all = Vec::new();
        r.read_to_end(&mut all).unwrap();
        assert_eq!(all, body);
    }
}
