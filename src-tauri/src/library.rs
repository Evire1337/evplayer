//! The user's own audio files. The index (paths + tags) lives in the data folder; the audio
//! itself is never copied, it is streamed straight from disk through the `evmedia` scheme
//! with range support so seeking works. The index file is the same one the Electron
//! version wrote, so nothing has to be added again after the move.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::prelude::{Accessor, ItemKey};
use serde::{Deserialize, Serialize};
use tauri::http::{header, HeaderValue, Request, Response, StatusCode};

pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "mp4", "aac", "ogg", "oga", "opus", "wav", "webm"];

pub fn mime(ext: &str) -> &'static str {
    match ext {
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "m4a" | "mp4" => "audio/mp4",
        "aac" => "audio/aac",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "webm" => "audio/webm",
        "jpg" => "image/jpeg",
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTrackRecord {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artists: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lossless: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits_per_sample: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsupported: Option<bool>,
    #[serde(default)]
    pub has_cover: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub edited: bool,
    pub size: u64,
    pub added_at: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct LocalTrackEdit {
    pub title: Option<String>,
    pub artists: Option<Vec<String>>,
    /// Some(None) = remove the album
    #[serde(default, deserialize_with = "nullable")]
    pub album: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    pub cover: Option<Option<String>>,
}

fn nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Serialize, Deserialize, Default)]
struct IndexFile {
    version: u32,
    tracks: Vec<LocalTrackRecord>,
    #[serde(default)]
    folders: Vec<String>,
}

#[derive(Serialize)]
pub struct AddResult {
    pub added: Vec<LocalTrackRecord>,
    pub skipped: usize,
}

/// the same id the Electron version made: sha1 of the lower-cased absolute path
pub fn local_track_id(path: &Path) -> String {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let text = abs.to_string_lossy().to_lowercase();
    let hex = sha1_smol::Sha1::from(text.as_bytes()).digest().to_string();
    format!("local:{}", &hex[..20])
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

pub fn title_from_file_name(path: &Path) -> (String, Option<String>) {
    let base = path.file_stem().map(|s| s.to_string_lossy().replace('_', " ")).unwrap_or_default();
    let base = base.trim().to_string();
    let numbered = regex::Regex::new(r"^(?:\d{1,3}[\s.\-]+)?(.+?)\s+-\s+(.+)$").unwrap();
    if let Some(c) = numbered.captures(&base) {
        return (c[2].trim().to_string(), Some(c[1].trim().to_string()));
    }
    let lead = regex::Regex::new(r"^\d{1,3}[\s.\-]+").unwrap();
    let stripped = lead.replace(&base, "").to_string();
    (if stripped.is_empty() { base } else { stripped }, None)
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

fn codec_name(file_type: FileType, path: &Path) -> (Option<String>, Option<bool>, bool) {
    match file_type {
        FileType::Flac => (Some("FLAC".into()), Some(true), false),
        FileType::Wav => (Some("PCM".into()), Some(true), false),
        FileType::Aiff => (Some("PCM".into()), Some(true), false),
        FileType::WavPack => (Some("WavPack".into()), Some(true), false),
        FileType::Ape => (Some("Monkey's Audio".into()), Some(true), true),
        FileType::Mpeg => (Some("MPEG 1 Layer 3".into()), Some(false), false),
        FileType::Aac => (Some("AAC".into()), Some(false), false),
        FileType::Opus => (Some("Opus".into()), Some(false), false),
        FileType::Vorbis => (Some("Vorbis I".into()), Some(false), false),
        FileType::Speex => (Some("Speex".into()), Some(false), true),
        FileType::Mp4 => {
            let alac = fs::File::open(path)
                .ok()
                .and_then(|mut f| lofty::mp4::Mp4File::read_from(&mut f, lofty::config::ParseOptions::new()).ok())
                .map(|m| matches!(m.properties().codec(), lofty::mp4::Mp4Codec::ALAC))
                .unwrap_or(false);
            if alac {
                (Some("ALAC".into()), Some(true), true)
            } else {
                (Some("AAC".into()), Some(false), false)
            }
        }
        _ => (None, None, false),
    }
}

pub struct Library {
    data_dir: PathBuf,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    loaded: bool,
    tracks: HashMap<String, LocalTrackRecord>,
    folders: BTreeSet<String>,
}

impl Library {
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self { data_dir, state: Mutex::new(State::default()) })
    }

    fn index_path(&self) -> PathBuf {
        self.data_dir.join("local-tracks.json")
    }

    fn cover_dir(&self) -> PathBuf {
        self.data_dir.join("local-covers")
    }

    fn with_state<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        let mut st = self.state.lock().unwrap();
        if !st.loaded {
            st.loaded = true;
            if let Ok(text) = fs::read_to_string(self.index_path()) {
                if let Ok(index) = serde_json::from_str::<IndexFile>(&text) {
                    for t in index.tracks {
                        st.tracks.insert(t.id.clone(), t);
                    }
                    st.folders.extend(index.folders);
                }
            }
        }
        f(&mut st)
    }

    fn save(&self, st: &State) {
        let _ = fs::create_dir_all(&self.data_dir);
        let body = IndexFile { version: 1, tracks: st.tracks.values().cloned().collect(), folders: st.folders.iter().cloned().collect() };
        let tmp = self.index_path().with_extension("json.tmp");
        if fs::write(&tmp, serde_json::to_vec(&body).unwrap_or_default()).is_ok() {
            let _ = fs::rename(&tmp, self.index_path());
        }
    }

    pub fn list(&self) -> Vec<LocalTrackRecord> {
        let mut list = self.with_state(|st| st.tracks.values().cloned().collect::<Vec<_>>());
        list.sort_by(|a, b| b.added_at.cmp(&a.added_at));
        list
    }

    pub fn get(&self, id: &str) -> Option<LocalTrackRecord> {
        self.with_state(|st| st.tracks.get(id).cloned())
    }

    pub fn folders(&self) -> Vec<String> {
        self.with_state(|st| st.folders.iter().cloned().collect())
    }

    /// read the tags of one file; a broken file still gets a record from its name
    pub fn read_file(&self, path: &Path) -> Option<LocalTrackRecord> {
        if !is_audio(path) {
            return None;
        }
        let meta = fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let id = local_track_id(path);
        let (fallback_title, fallback_artist) = title_from_file_name(path);
        let mut record = LocalTrackRecord {
            id: id.clone(),
            path: path.to_string_lossy().to_string(),
            title: fallback_title,
            artists: fallback_artist.into_iter().collect(),
            album: None,
            duration_ms: 0,
            codec: None,
            lossless: None,
            sample_rate: None,
            bits_per_sample: None,
            bitrate: None,
            unsupported: None,
            has_cover: false,
            edited: false,
            size: meta.len(),
            added_at: now_iso(),
        };
        let Ok(tagged) = lofty::read_from_path(path) else { return Some(record) };
        let props = tagged.properties();
        record.duration_ms = props.duration().as_millis() as u64;
        record.sample_rate = props.sample_rate();
        record.bits_per_sample = props.bit_depth();
        record.bitrate = props.overall_bitrate().or(props.audio_bitrate()).map(|k| k * 1000);
        let (codec, lossless, unsupported) = codec_name(tagged.file_type(), path);
        record.codec = codec;
        record.lossless = lossless;
        if unsupported {
            record.unsupported = Some(true);
        }
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(title) = tag.title().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
                record.title = title;
            }
            let mut artists: Vec<String> = tag.get_strings(&ItemKey::TrackArtist).flat_map(split_artists).collect();
            artists.dedup();
            if !artists.is_empty() {
                record.artists = artists;
            }
            record.album = tag.album().map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
            if let Some(picture) = tag.pictures().first() {
                let ext = match picture.mime_type() {
                    Some(lofty::picture::MimeType::Png) => "png",
                    _ => "jpg",
                };
                let _ = fs::create_dir_all(self.cover_dir());
                if fs::write(self.cover_dir().join(format!("{}.{ext}", &id[6..])), picture.data()).is_ok() {
                    record.has_cover = true;
                }
            }
        }
        Some(record)
    }

    pub fn collect_audio_files(entries: &[PathBuf], depth: usize, out: &mut Vec<PathBuf>) {
        for entry in entries {
            let Ok(meta) = fs::metadata(entry) else { continue };
            if meta.is_dir() {
                if depth > 8 {
                    continue;
                }
                let children: Vec<PathBuf> = fs::read_dir(entry).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
                Self::collect_audio_files(&children, depth + 1, out);
            } else if is_audio(entry) {
                out.push(entry.clone());
            }
        }
    }

    /// returns the folders that became watched
    pub fn add(&self, paths: &[PathBuf]) -> (AddResult, Vec<String>) {
        let mut new_folders = Vec::new();
        self.with_state(|st| {
            for p in paths {
                if p.is_dir() {
                    let abs = std::path::absolute(p).unwrap_or_else(|_| p.clone()).to_string_lossy().to_string();
                    if st.folders.insert(abs.clone()) {
                        new_folders.push(abs);
                    }
                }
            }
        });
        let mut files = Vec::new();
        Self::collect_audio_files(paths, 0, &mut files);
        let mut added = Vec::new();
        let mut skipped = 0;
        for file in files {
            let Some(mut record) = self.read_file(&file) else {
                skipped += 1;
                continue;
            };
            if let Some(existing) = self.get(&record.id) {
                record.added_at = existing.added_at.clone();
                if existing.edited {
                    // keep what the user typed in over the tags in the file
                    record.title = existing.title;
                    record.artists = existing.artists;
                    record.album = existing.album;
                    record.has_cover = existing.has_cover;
                    record.edited = true;
                }
            }
            added.push(record);
        }
        self.with_state(|st| {
            for r in &added {
                st.tracks.insert(r.id.clone(), r.clone());
            }
            if !added.is_empty() || !new_folders.is_empty() {
                self.save(st);
            }
        });
        (AddResult { added, skipped }, new_folders)
    }

    pub fn update(&self, id: &str, patch: LocalTrackEdit) -> Option<LocalTrackRecord> {
        let stem = id.trim_start_matches("local:").to_string();
        let cover_dir = self.cover_dir();
        self.with_state(|st| {
            let record = st.tracks.get_mut(id)?;
            if let Some(title) = patch.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                record.title = title.chars().take(300).collect();
            }
            if let Some(artists) = patch.artists {
                record.artists = artists.into_iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).take(20).collect();
            }
            if let Some(album) = patch.album {
                record.album = album.map(|a| a.trim().chars().take(300).collect::<String>()).filter(|a| !a.is_empty());
            }
            if let Some(cover) = patch.cover {
                let _ = fs::remove_file(cover_dir.join(format!("{stem}.jpg")));
                let _ = fs::remove_file(cover_dir.join(format!("{stem}.png")));
                record.has_cover = false;
                let re = regex::Regex::new(r"(?i)^data:image/(png|jpe?g|webp);base64,(.+)$").unwrap();
                if let Some(c) = cover.as_deref().and_then(|c| re.captures(c)) {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&c[2]) {
                        let ext = if c[1].eq_ignore_ascii_case("png") { "png" } else { "jpg" };
                        let _ = fs::create_dir_all(&cover_dir);
                        record.has_cover = fs::write(cover_dir.join(format!("{stem}.{ext}")), bytes).is_ok();
                    }
                }
            }
            record.edited = true;
            let out = record.clone();
            self.save(st);
            Some(out)
        })
    }

    pub fn remove(&self, ids: &[String]) {
        let cover_dir = self.cover_dir();
        self.with_state(|st| {
            for id in ids {
                st.tracks.remove(id);
                let stem = id.trim_start_matches("local:");
                let _ = fs::remove_file(cover_dir.join(format!("{stem}.jpg")));
                let _ = fs::remove_file(cover_dir.join(format!("{stem}.png")));
            }
            self.save(st);
        });
    }

    pub fn unwatch(&self, folder: &str) {
        self.with_state(|st| {
            st.folders.remove(folder);
            self.save(st);
        });
    }

    /// add files that appeared in the folder and drop ones that were deleted; true if anything changed
    pub fn rescan(&self, folder: &str) -> bool {
        let mut files = Vec::new();
        Self::collect_audio_files(&[PathBuf::from(folder)], 0, &mut files);
        let present: std::collections::HashSet<String> = files.iter().map(|f| local_track_id(f)).collect();
        let prefix = format!("{}{}", folder.to_lowercase().trim_end_matches(['/', '\\']), std::path::MAIN_SEPARATOR);
        let mut changed = false;
        self.with_state(|st| {
            let gone: Vec<String> = st
                .tracks
                .values()
                .filter(|r| r.path.to_lowercase().starts_with(&prefix) && !present.contains(&r.id))
                .map(|r| r.id.clone())
                .collect();
            for id in gone {
                st.tracks.remove(&id);
                changed = true;
            }
        });
        for file in files {
            let id = local_track_id(&file);
            if self.get(&id).is_some() {
                continue;
            }
            if let Some(record) = self.read_file(&file) {
                self.with_state(|st| st.tracks.insert(record.id.clone(), record));
                changed = true;
            }
        }
        if changed {
            self.with_state(|st| self.save(st));
        }
        changed
    }

    /// answers evmedia://localhost/track%2F<id> and cover%2F<id>
    pub fn handle(&self, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
        let raw = request.uri().path().trim_start_matches('/');
        let decoded = percent_encoding::percent_decode_str(raw).decode_utf8_lossy().to_string();
        let (kind, id) = decoded.split_once('/').unwrap_or(("", ""));
        match kind {
            "cover" => {
                for ext in ["jpg", "png"] {
                    let file = self.cover_dir().join(format!("{}.{ext}", id.trim_start_matches("local:")));
                    if let Ok(data) = fs::read(&file) {
                        let mut res = Response::builder().status(200).body(data).unwrap();
                        let h = res.headers_mut();
                        h.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime(ext)));
                        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("max-age=31536000"));
                        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
                        return res;
                    }
                }
                not_found("no cover")
            }
            "track" => match self.get(id) {
                Some(record) => serve_file(Path::new(&record.path), request),
                None => not_found("unknown track"),
            },
            _ => not_found("unknown address"),
        }
    }
}

fn split_artists(value: &str) -> Vec<String> {
    value.split(['\0', ';']).map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect()
}

fn not_found(text: &str) -> Response<Vec<u8>> {
    let mut res = Response::builder().status(StatusCode::NOT_FOUND).body(text.as_bytes().to_vec()).unwrap();
    res.headers_mut().insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    res
}

/// how much of a file one answer carries when the player asks for "the rest"
const FILE_CHUNK: u64 = 4 * 1024 * 1024;

pub fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
    let rest = value.trim().strip_prefix("bytes=")?;
    let (a, b) = rest.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", "") => return None,
        ("", suffix) => (size.saturating_sub(suffix.parse().ok()?), size.saturating_sub(1)),
        (s, "") => {
            let s: u64 = s.parse().ok()?;
            (s, (s + FILE_CHUNK - 1).min(size.saturating_sub(1)))
        }
        (s, e) => (s.parse().ok()?, e.parse::<u64>().ok()?.min(size.saturating_sub(1))),
    };
    Some((start, end))
}

fn serve_file(path: &Path, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Ok(mut file) = fs::File::open(path) else { return not_found("file moved or deleted") };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let range = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).map(str::to_string).unwrap_or_else(|| "bytes=0-".into());
    let Some((start, end)) = parse_range(&range, size) else { return not_found("bad range") };
    if start > end || start >= size {
        let mut res = Response::builder().status(StatusCode::RANGE_NOT_SATISFIABLE).body(Vec::new()).unwrap();
        res.headers_mut().insert(header::CONTENT_RANGE, HeaderValue::from_str(&format!("bytes */{size}")).unwrap());
        return res;
    }
    let mut buf = vec![0u8; (end - start + 1) as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut buf).is_err() {
        return not_found("could not read the file");
    }
    let mut res = Response::builder().status(StatusCode::PARTIAL_CONTENT).body(buf).unwrap();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime(&ext)));
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::CONTENT_RANGE, HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")).unwrap());
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    h.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_static("content-length, content-range, accept-ranges"));
    res
}

/// watches the folders added as a whole; a burst of changes is answered with one rescan
pub struct Watcher {
    _inner: Mutex<Option<notify::RecommendedWatcher>>,
}

impl Watcher {
    pub fn start(library: Arc<Library>, on_change: impl Fn() + Send + Sync + 'static) -> Arc<Watcher> {
        use notify::{RecursiveMode, Watcher as _};
        let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(e) = event {
                for p in e.paths {
                    let _ = tx.send(p);
                }
            }
        })
        .ok();
        let watcher = Arc::new(Watcher { _inner: Mutex::new(watcher) });
        let folders = library.folders();
        if let Some(w) = watcher._inner.lock().unwrap().as_mut() {
            for f in &folders {
                let _ = w.watch(Path::new(f), RecursiveMode::Recursive);
            }
        }
        let lib = library.clone();
        std::thread::spawn(move || {
            // catch up with changes made while the app was closed
            let mut changed = false;
            for f in &folders {
                changed |= lib.rescan(f);
            }
            if changed {
                on_change();
            }
            loop {
                let Ok(first) = rx.recv() else { break };
                let mut touched = vec![first];
                // files are often copied in bursts: wait for things to settle
                while let Ok(p) = rx.recv_timeout(Duration::from_secs(2)) {
                    touched.push(p);
                }
                let folders = lib.folders();
                let mut changed = false;
                for f in folders.iter().filter(|f| touched.iter().any(|p| p.to_string_lossy().to_lowercase().starts_with(&f.to_lowercase()))) {
                    changed |= lib.rescan(f);
                }
                if changed {
                    on_change();
                }
            }
        });
        watcher
    }

    pub fn watch(&self, folder: &str) {
        use notify::{RecursiveMode, Watcher as _};
        if let Some(w) = self._inner.lock().unwrap().as_mut() {
            let _ = w.watch(Path::new(folder), RecursiveMode::Recursive);
        }
    }

    pub fn unwatch(&self, folder: &str) {
        use notify::Watcher as _;
        if let Some(w) = self._inner.lock().unwrap().as_mut() {
            let _ = w.unwatch(Path::new(folder));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_files() {
        assert_eq!(title_from_file_name(Path::new("/m/03 - Artist B - Second Song.flac")), ("Second Song".into(), Some("Artist B".into())));
        assert_eq!(title_from_file_name(Path::new("/m/Artist - Song.mp3")), ("Song".into(), Some("Artist".into())));
        assert_eq!(title_from_file_name(Path::new("/m/07. Intro.mp3")), ("Intro".into(), None));
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=990-", 1000), Some((990, 999)));
        assert_eq!(parse_range("bytes=0-", 100 * 1024 * 1024), Some((0, FILE_CHUNK - 1)));
    }

    #[test]
    fn id_matches_the_electron_version() {
        // sha1("/music/a.mp3") — the Electron version hashed path.resolve(file).toLowerCase()
        let id = local_track_id(Path::new("/music/A.mp3"));
        let expect = sha1_smol::Sha1::from("/music/a.mp3").digest().to_string();
        assert_eq!(id, format!("local:{}", &expect[..20]));
    }
}
