//! Songs kept on this computer for listening without a connection. The page decides what to
//! keep and hands over the address a song would be played from; this side fetches it (a
//! plain file, an HLS playlist joined piece by piece, or a YouTube address asked in
//! pieces), names the file by what is really inside it and records it in a small index.
//! The media server plays the kept copies back (`/offline/<id>`), seeking included.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::media_server::{blocking_client, google_piece, google_piece_url, google_user_agent, is_googlevideo, playlist_pieces};

/// a file smaller than this is an error page or a preview stub, not a song
const MIN_SONG_BYTES: u64 = 20 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineEntry {
    pub id: String,
    pub file: String,
    pub ext: String,
    pub size: u64,
    pub saved_at: String,
}

/// where a song's bytes come from
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Source {
    /// one address with the whole file (YouTube addresses are asked in pieces)
    File { url: String },
    /// an HLS playlist whose pieces are laid one after another
    Hls { url: String },
}

pub struct Offline {
    dir: PathBuf,
    index: Mutex<BTreeMap<String, OfflineEntry>>,
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

/// the file name for a song id: readable part + a short hash, so two ids never share a file
pub fn file_stem(id: &str) -> String {
    let readable: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).take(40).collect();
    let hash = sha1_smol::Sha1::from(id.as_bytes()).digest().to_string();
    format!("{readable}-{}", &hash[..8])
}

/// the kind of audio from the first bytes of a file
pub fn sniff_ext(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"OggS") {
        Some("ogg")
    } else if head.starts_with(b"fLaC") {
        Some("flac")
    } else if head.len() >= 8 && &head[4..8] == b"ftyp" {
        Some("m4a")
    } else if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some("webm")
    } else if head.starts_with(b"ID3") {
        Some("mp3")
    } else if head.len() >= 2 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0 {
        // the layer bits are 00 in AAC frames (ADTS) and 01..11 in MPEG audio
        Some(if (head[1] >> 1) & 3 == 0 { "aac" } else { "mp3" })
    } else {
        None
    }
}

struct Cancelled;

enum Failure {
    Cancelled,
    Other(String),
}

impl From<String> for Failure {
    fn from(text: String) -> Self {
        Failure::Other(text)
    }
}

impl From<&str> for Failure {
    fn from(text: &str) -> Self {
        Failure::Other(text.to_string())
    }
}

impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        Failure::Other(e.to_string())
    }
}

impl From<Cancelled> for Failure {
    fn from(_: Cancelled) -> Self {
        Failure::Cancelled
    }
}

fn check(stop: &AtomicBool) -> Result<(), Cancelled> {
    if stop.load(Ordering::Relaxed) {
        Err(Cancelled)
    } else {
        Ok(())
    }
}

/// copy a reply into the file, noticing a cancel between reads
fn pour(mut from: impl Read, to: &mut File, stop: &AtomicBool, mut counted: impl FnMut(u64)) -> Result<(), Failure> {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        check(stop)?;
        let n = from.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        to.write_all(&buf[..n])?;
        counted(n as u64);
    }
}

fn get(url: &url::Url, allowed: fn(&url::Url) -> bool) -> Result<reqwest::blocking::RequestBuilder, String> {
    if !allowed(url) {
        return Err("этот адрес не разрешён".into());
    }
    let mut request = blocking_client().get(url.clone());
    if let Some(agent) = google_user_agent(url) {
        request = request.header("User-Agent", agent);
    }
    Ok(request)
}

fn ok_reply(res: reqwest::blocking::Response) -> Result<reqwest::blocking::Response, String> {
    if res.status().is_success() {
        Ok(res)
    } else {
        Err(format!("сервис ответил {}", res.status().as_u16()))
    }
}

fn fetch_file(url: &url::Url, out: &mut File, stop: &AtomicBool, allowed: fn(&url::Url) -> bool, progress: &dyn Fn(u64, u64)) -> Result<(), Failure> {
    // YouTube hands a song out only in pieces
    if is_googlevideo(url) {
        if let Some((_, _, total)) = google_piece(url, None) {
            let mut start = 0u64;
            while start < total {
                check(stop)?;
                let (a, b, _) = google_piece(url, Some(&format!("bytes={start}-"))).ok_or("плохой диапазон")?;
                let reply = ok_reply(get(&google_piece_url(url, a, b), allowed)?.send().map_err(|e| e.to_string())?)?;
                pour(reply, out, stop, |_| {})?;
                start = b + 1;
                progress(start, total);
            }
            return Ok(());
        }
    }
    let reply = ok_reply(get(url, allowed)?.send().map_err(|e| e.to_string())?)?;
    let total = reply.content_length().unwrap_or(0);
    let done = std::cell::Cell::new(0u64);
    pour(reply, out, stop, |n| {
        done.set(done.get() + n);
        progress(done.get(), total);
    })?;
    // some servers do not say how long the file is: the end is known once it has arrived
    progress(done.get(), done.get());
    Ok(())
}

fn fetch_hls(url: &url::Url, out: &mut File, stop: &AtomicBool, allowed: fn(&url::Url) -> bool, progress: &dyn Fn(u64, u64)) -> Result<(), Failure> {
    let text = ok_reply(get(url, allowed)?.send().map_err(|e| e.to_string())?)?.text().map_err(|e| e.to_string())?;
    let pieces = playlist_pieces(&text, url).map_err(|e| format!("поток нельзя сохранить: {e}"))?;
    if !pieces.iter().all(allowed) {
        return Err(Failure::Other("часть потока на неразрешённом адресе".into()));
    }
    let count = pieces.len() as u64;
    for (i, piece) in pieces.iter().enumerate() {
        check(stop)?;
        let reply = ok_reply(get(piece, allowed)?.send().map_err(|e| e.to_string())?)?;
        pour(reply, out, stop, |_| {})?;
        progress(i as u64 + 1, count);
    }
    Ok(())
}

impl Offline {
    pub fn new(data: &Path) -> Arc<Self> {
        let dir = data.join("offline");
        let _ = fs::create_dir_all(&dir);
        let stored: BTreeMap<String, OfflineEntry> =
            fs::read_to_string(dir.join("index.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        // a file the user deleted by hand is not kept any more; leftovers of a broken download go too
        let index: BTreeMap<String, OfflineEntry> = stored.into_iter().filter(|(_, e)| dir.join(&e.file).is_file()).collect();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|e| e == "part") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let this = Arc::new(Self { dir, index: Mutex::new(index), running: Mutex::new(HashMap::new()) });
        this.save_index();
        this
    }

    pub fn folder(&self) -> &Path {
        &self.dir
    }

    fn save_index(&self) {
        let text = serde_json::to_string(&*self.index.lock().unwrap()).unwrap_or_else(|_| "{}".into());
        let tmp = self.dir.join("index.json.tmp");
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, self.dir.join("index.json"));
        }
    }

    pub fn list(&self) -> Vec<OfflineEntry> {
        self.index.lock().unwrap().values().cloned().collect()
    }

    pub fn path_of(&self, id: &str) -> Option<PathBuf> {
        self.index.lock().unwrap().get(id).map(|e| self.dir.join(&e.file))
    }

    pub fn remove(&self, ids: &[String]) {
        for id in ids {
            self.cancel(id);
            if let Some(entry) = self.index.lock().unwrap().remove(id) {
                let _ = fs::remove_file(self.dir.join(entry.file));
            }
        }
        self.save_index();
    }

    pub fn clear(&self) {
        let ids: Vec<String> = self.index.lock().unwrap().keys().cloned().collect();
        self.remove(&ids);
    }

    pub fn cancel(&self, id: &str) {
        if let Some(flag) = self.running.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// keep one song; `progress(done, total)` is called as it arrives (total 0 = unknown)
    pub fn fetch(&self, id: &str, source: &Source, allowed: fn(&url::Url) -> bool, progress: &dyn Fn(u64, u64)) -> Result<OfflineEntry, String> {
        if let Some(entry) = self.index.lock().unwrap().get(id) {
            if self.dir.join(&entry.file).is_file() {
                return Ok(entry.clone());
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(id) {
                return Err("этот трек уже скачивается".into());
            }
            running.insert(id.to_string(), stop.clone());
        }
        let stem = file_stem(id);
        let part = self.dir.join(format!("{stem}.part"));
        let outcome = self.write(id, &stem, &part, source, &stop, allowed, progress);
        self.running.lock().unwrap().remove(id);
        match outcome {
            Ok(entry) => {
                self.index.lock().unwrap().insert(id.to_string(), entry.clone());
                self.save_index();
                Ok(entry)
            }
            Err(failure) => {
                let _ = fs::remove_file(&part);
                Err(match failure {
                    Failure::Cancelled => "отменено".to_string(),
                    Failure::Other(text) => text,
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write(&self, id: &str, stem: &str, part: &Path, source: &Source, stop: &AtomicBool, allowed: fn(&url::Url) -> bool, progress: &dyn Fn(u64, u64)) -> Result<OfflineEntry, Failure> {
        let mut out = File::create(part)?;
        match source {
            Source::File { url } => fetch_file(&url::Url::parse(url).map_err(|e| e.to_string())?, &mut out, stop, allowed, progress)?,
            Source::Hls { url } => fetch_hls(&url::Url::parse(url).map_err(|e| e.to_string())?, &mut out, stop, allowed, progress)?,
        }
        out.flush()?;
        drop(out);
        let size = fs::metadata(part)?.len();
        if size < MIN_SONG_BYTES {
            return Err(Failure::Other("сервис вернул не песню, а заглушку".into()));
        }
        let mut head = [0u8; 16];
        let read = File::open(part)?.read(&mut head)?;
        let ext = sniff_ext(&head[..read]).ok_or_else(|| Failure::Other("неизвестный формат аудио".to_string()))?;
        let file = format!("{stem}.{ext}");
        fs::rename(part, self.dir.join(&file))?;
        Ok(OfflineEntry { id: id.to_string(), file, ext: ext.to_string(), size, saved_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true) })
    }
}

/// one kept song to be copied out as an ordinary, named and tagged file
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artwork: Option<String>,
}

/// what a copy-out did: how many songs landed in the folder, how many were not kept any more
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub copied: usize,
    pub missing: usize,
    pub folder: String,
}

/// "Artist - Title" as a file name every system accepts
pub fn export_name(artist: &str, title: &str) -> String {
    let raw = if artist.trim().is_empty() { title.trim().to_string() } else { format!("{} - {}", artist.trim(), title.trim()) };
    let clean: String = raw.chars().map(|c| if c.is_control() || "<>:\"/\|?*"\|?*".contains(c) { '_' } else { c }).collect();
    let clean = clean.trim().trim_end_matches('.').trim();
    let clean: String = clean.chars().take(120).collect();
    if clean.is_empty() { "track".into() } else { clean }
}

/// a free name in `folder`: "x.mp3", then "x (2).mp3", ...
fn free_path(folder: &Path, name: &str, ext: &str) -> PathBuf {
    let mut path = folder.join(format!("{name}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{name} ({n}).{ext}"));
        n += 1;
    }
    path
}

/// writes title, artist and cover into the copy; formats the tag library cannot write to
/// (WebM from YouTube) are left with just the readable name
fn tag_copy(path: &Path, item: &ExportItem) {
    use lofty::config::WriteOptions;
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::picture::{MimeType, Picture, PictureType};
    use lofty::prelude::Accessor;
    use lofty::tag::Tag;
    let Ok(mut tagged) = lofty::read_from_path(path) else { return };
    if tagged.primary_tag().is_none() {
        let kind = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(kind));
    }
    let Some(tag) = tagged.primary_tag_mut() else { return };
    tag.set_title(item.title.clone());
    if !item.artist.trim().is_empty() {
        tag.set_artist(item.artist.clone());
    }
    if let Some(bytes) = item.artwork.as_deref().and_then(cover_bytes) {
        let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { MimeType::Png } else { MimeType::Jpeg };
        tag.push_picture(Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes));
    }
    let _ = tagged.save_to_path(path, WriteOptions::default());
}

/// the cover's bytes (at most a few MB), or nothing when the picture cannot be fetched
fn cover_bytes(address: &str) -> Option<Vec<u8>> {
    let url = url::Url::parse(address).ok()?;
    let res = get(&url, crate::proxy::is_open_host).ok()?.send().ok()?;
    let res = ok_reply(res).ok()?;
    let mut bytes = Vec::new();
    res.take(5 * 1024 * 1024).read_to_end(&mut bytes).ok()?;
    (bytes.len() > 100).then_some(bytes)
}

impl Offline {
    /// copies kept songs into `folder` as "Artist - Title.ext" with tags; the kept originals
    /// stay where they are
    pub fn export(&self, items: &[ExportItem], folder: &Path) -> Result<ExportResult, String> {
        fs::create_dir_all(folder).map_err(|e| format!("не удалось открыть папку: {e}"))?;
        let mut result = ExportResult { folder: folder.to_string_lossy().into_owned(), ..Default::default() };
        for item in items {
            let Some(from) = self.path_of(&item.id) else {
                result.missing += 1;
                continue;
            };
            let ext = from.extension().and_then(|e| e.to_str()).unwrap_or("mp3").to_string();
            let to = free_path(folder, &export_name(&item.artist, &item.title), &ext);
            if fs::copy(&from, &to).is_err() {
                result.missing += 1;
                continue;
            }
            tag_copy(&to, item);
            result.copied += 1;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn anything(_: &url::Url) -> bool {
        true
    }

    /// a tiny server: /song = 40 KB of mp3-looking bytes, /list.m3u8 and /p0..p2 = pieces
    fn serve() -> (String, Arc<AtomicUsize>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", server.server_addr().to_ip().unwrap().port());
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        std::thread::spawn(move || {
            for req in server.incoming_requests() {
                counter.fetch_add(1, Ordering::Relaxed);
                let path = req.url().to_string();
                let body: Vec<u8> = match path.as_str() {
                    "/song" => [b"ID3".to_vec(), vec![1u8; 40 * 1024]].concat(),
                    "/stub" => b"nope".to_vec(),
                    "/list.m3u8" => b"#EXTM3U\n#EXT-X-TARGETDURATION:5\n#EXTINF:5,\np0\n#EXTINF:5,\np1\n#EXTINF:5,\np2\n#EXT-X-ENDLIST\n".to_vec(),
                    "/locked.m3u8" => b"#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\n#EXTINF:5,\np0\n".to_vec(),
                    "/p0" => [b"OggS".to_vec(), vec![2u8; 9000]].concat(),
                    "/p1" | "/p2" => vec![3u8; 9000],
                    _ => Vec::new(),
                };
                let code = if body.is_empty() { 404 } else { 200 };
                let _ = req.respond(tiny_http::Response::from_data(body).with_status_code(code));
            }
        });
        (base, hits)
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("evplayer-offline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_and_sniffing() {
        assert_ne!(file_stem("sc:1"), file_stem("sc_1"));
        assert!(file_stem("yt:ab-_C").starts_with("yt_ab-_C-"));
        assert_eq!(sniff_ext(b"OggS\0\0"), Some("ogg"));
        assert_eq!(sniff_ext(b"ID3\x04"), Some("mp3"));
        assert_eq!(sniff_ext(&[0xFF, 0xFB, 0x90]), Some("mp3"));
        assert_eq!(sniff_ext(&[0xFF, 0xF1, 0x50]), Some("aac"));
        assert_eq!(sniff_ext(b"\0\0\0\x20ftypM4A "), Some("m4a"));
        assert_eq!(sniff_ext(&[0x1A, 0x45, 0xDF, 0xA3, 1]), Some("webm"));
        assert_eq!(sniff_ext(b"<html>"), None);
    }

    #[test]
    fn keeps_a_file_lists_it_and_forgets_it() {
        let (base, _) = serve();
        let dir = temp("file");
        let offline = Offline::new(&dir);
        let seen = Mutex::new(Vec::new());
        let entry = offline.fetch("sc:1", &Source::File { url: format!("{base}/song") }, anything, &|d, t| seen.lock().unwrap().push((d, t))).unwrap();
        assert_eq!(entry.ext, "mp3");
        assert_eq!(entry.size, 3 + 40 * 1024);
        assert_eq!(seen.lock().unwrap().last().copied(), Some((3 + 40 * 1024, 3 + 40 * 1024)));
        assert!(offline.path_of("sc:1").unwrap().is_file());
        // a second request for the same song costs nothing
        let again = offline.fetch("sc:1", &Source::File { url: format!("{base}/nowhere") }, anything, &|_, _| {}).unwrap();
        assert_eq!(again.file, entry.file);
        // the index survives a restart
        assert_eq!(Offline::new(&dir).list().len(), 1);
        offline.remove(&["sc:1".to_string()]);
        assert!(offline.list().is_empty());
        assert!(!dir.join("offline").join(&entry.file).exists());
    }

    #[test]
    fn joins_hls_pieces_and_refuses_protected_or_stub() {
        let (base, _) = serve();
        let offline = Offline::new(&temp("hls"));
        let entry = offline.fetch("sc:2", &Source::Hls { url: format!("{base}/list.m3u8") }, anything, &|_, _| {}).unwrap();
        assert_eq!((entry.ext.as_str(), entry.size), ("ogg", 4 + 9000 + 9000 + 9000));
        assert!(offline.fetch("sc:3", &Source::Hls { url: format!("{base}/locked.m3u8") }, anything, &|_, _| {}).unwrap_err().contains("нельзя сохранить"));
        assert!(offline.fetch("sc:4", &Source::File { url: format!("{base}/stub") }, anything, &|_, _| {}).unwrap_err().contains("заглушку"));
        // a failed song leaves nothing behind
        assert_eq!(offline.list().len(), 1);
        let leftovers = fs::read_dir(offline.folder()).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "part")).count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn only_allowed_addresses() {
        let offline = Offline::new(&temp("hosts"));
        let err = offline.fetch("sc:5", &Source::File { url: "https://example.com/a".into() }, crate::proxy::is_open_host, &|_, _| {}).unwrap_err();
        assert!(err.contains("не разрешён"));
    }

    #[test]
    fn export_names_are_safe_and_unique() {
        assert_eq!(export_name("AC/DC", "Who: Made?"), "AC_DC - Who_ Made_");
        assert_eq!(export_name("", "Solo."), "Solo");
        assert_eq!(export_name(" ", " "), "track");
        let dir = temp("export_names");
        fs::write(dir.join("a.mp3"), b"x").unwrap();
        assert_eq!(free_path(&dir, "a", "mp3"), dir.join("a (2).mp3"));
    }

    #[test]
    fn cancel_stops_a_running_download() {
        let (base, _) = serve();
        let offline = Offline::new(&temp("cancel"));
        let o2 = offline.clone();
        // cancel as soon as the first byte is reported
        let result = offline.fetch("sc:6", &Source::File { url: format!("{base}/song") }, anything, &|_, _| o2.cancel("sc:6"));
        assert_eq!(result.unwrap_err(), "отменено");
        assert!(offline.list().is_empty());
    }
}
