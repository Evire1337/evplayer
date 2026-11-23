//! Spotify's "Download your data" export (src/library/spotifyExport.ts): any account can ask
//! for it, Premium or not. The files hold names only, no sound; the songs are found on
//! YouTube Music and SoundCloud when imported.
//!   YourLibrary.json                       liked songs ("tracks": [{artist, album, track, uri}])
//!   Playlist1.json / Playlists1.json …     playlists ("playlists": [{name, items: [{track: {…}}]}])
//!   StreamingHistory_music_0.json          short history ([{artistName, trackName, msPlayed}])
//!   Streaming_History_Audio_*.json         extended history ([{master_metadata_*, ms_played}])
//! The zip Spotify sends is read as it is (only its .json files).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::Path;

use serde_json::Value;

use crate::import::Named;

/// songs of the history kept for "Самое слушаемое"
const TOP_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Liked,
    Top,
    Playlist,
}

#[derive(Clone, Debug)]
pub struct List {
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub tracks: Vec<Named>,
}

const LIKED: &str = "Любимые из Spotify";
const TOP: &str = "Самое слушаемое в Spotify";
const FROM: &str = "Импортировано из Spotify";

/// the .json files of a zip, by their names (folders dropped); stored and deflated entries
pub fn unzip_json(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let u16_at = |o: usize| bytes.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    // the end of the central directory: the last "PK\5\6"
    let end = (0..bytes.len().saturating_sub(21)).rev().find(|&i| bytes[i..].starts_with(&[0x50, 0x4b, 5, 6])).ok_or("это не zip-архив")?;
    let count = u16_at(end + 10).ok_or("zip повреждён")?;
    let mut at = u32_at(end + 16).ok_or("zip повреждён")?;
    let mut out = BTreeMap::new();
    for _ in 0..count {
        if !bytes.get(at..).is_some_and(|b| b.starts_with(&[0x50, 0x4b, 1, 2])) {
            return Err("zip повреждён".into());
        }
        let method = u16_at(at + 10).unwrap_or(0);
        let size = u32_at(at + 20).unwrap_or(0);
        let name_len = u16_at(at + 28).unwrap_or(0);
        let extra_len = u16_at(at + 30).unwrap_or(0);
        let comment_len = u16_at(at + 32).unwrap_or(0);
        let local = u32_at(at + 42).unwrap_or(0);
        let name = String::from_utf8_lossy(bytes.get(at + 46..at + 46 + name_len).unwrap_or_default()).to_string();
        at += 46 + name_len + extra_len + comment_len;
        if !name.to_lowercase().ends_with(".json") {
            continue;
        }
        let (Some(lname), Some(lextra)) = (u16_at(local + 26), u16_at(local + 28)) else { continue };
        let start = local + 30 + lname + lextra;
        let Some(data) = bytes.get(start..start + size) else { continue };
        let text = match method {
            0 => String::from_utf8_lossy(data).to_string(),
            8 => {
                let mut s = String::new();
                if flate2::read::DeflateDecoder::new(data).read_to_string(&mut s).is_err() {
                    continue;
                }
                s
            }
            _ => continue,
        };
        out.insert(name.rsplit('/').next().unwrap_or(&name).to_string(), text);
    }
    Ok(out)
}

/// the chosen files (json, or the whole zip as Spotify sends it): name → text
pub fn read_files(paths: &[std::path::PathBuf]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for p in paths {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let lower = name.to_lowercase();
        if lower.ends_with(".zip") {
            if let Ok(entries) = std::fs::read(p).map_err(|e| e.to_string()).and_then(|b| unzip_json(&b)) {
                out.extend(entries);
            }
        } else if lower.ends_with(".json") {
            if let Ok(t) = std::fs::read_to_string(p) {
                out.insert(name, t);
            }
        }
    }
    out
}

fn s(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).map(|x| x.trim().to_string()).unwrap_or_default()
}

fn named(title: String, artist: String) -> Option<Named> {
    if title.is_empty() {
        return None;
    }
    Some(Named { title, artists: if artist.is_empty() { vec![] } else { vec![artist] }, duration_ms: None })
}

fn dedupe(list: Vec<Named>) -> Vec<Named> {
    let mut seen = HashSet::new();
    list.into_iter().filter(|t| seen.insert(format!("{}|{}", t.title, t.artists.first().map(String::as_str).unwrap_or("")).to_lowercase())).collect()
}

pub fn parse(files: &BTreeMap<String, String>) -> Vec<List> {
    let mut out = Vec::new();
    let mut history: HashMap<String, (Named, f64, usize)> = HashMap::new();
    for (name, text) in files {
        let Ok(json) = serde_json::from_str::<Value>(text) else { continue };
        let lname = name.to_lowercase();
        // liked songs
        if let (Some(tracks), true) = (json.get("tracks").and_then(Value::as_array), lname.contains("library")) {
            let list = dedupe(tracks.iter().filter_map(|r| named(s(r.get("track")), s(r.get("artist")))).collect());
            if !list.is_empty() {
                out.push(List { kind: Kind::Liked, title: LIKED.into(), description: FROM.into(), tracks: list });
            }
            continue;
        }
        // playlists
        if let Some(lists) = json.get("playlists").and_then(Value::as_array) {
            for p in lists {
                let items = p.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                let list = dedupe(items.iter().filter_map(|row| row.get("track").filter(|t| !t.is_null())).filter_map(|t| named(s(t.get("trackName")), s(t.get("artistName")))).collect());
                let title = Some(s(p.get("name"))).filter(|t| !t.is_empty()).unwrap_or_else(|| "Spotify".into());
                let description = Some(s(p.get("description"))).filter(|d| !d.is_empty()).unwrap_or_else(|| FROM.into());
                if !list.is_empty() {
                    out.push(List { kind: Kind::Playlist, title, description, tracks: list });
                }
            }
            continue;
        }
        // listening history (short or extended format): songs heard at least 30 s, longest first
        if let (Some(rows), true) = (json.as_array(), lname.contains("history")) {
            for r in rows {
                let title = Some(s(r.get("trackName"))).filter(|t| !t.is_empty()).unwrap_or_else(|| s(r.get("master_metadata_track_name")));
                let artist = Some(s(r.get("artistName"))).filter(|t| !t.is_empty()).unwrap_or_else(|| s(r.get("master_metadata_album_artist_name")));
                let ms = r.get("msPlayed").or_else(|| r.get("ms_played")).and_then(Value::as_f64).unwrap_or(0.0);
                if ms < 30_000.0 {
                    continue;
                }
                let key = format!("{title}|{artist}").to_lowercase();
                let order = history.len();
                if let Some(t) = named(title, artist) {
                    history.entry(key).or_insert((t, 0.0, order)).1 += ms;
                }
            }
        }
    }
    if !history.is_empty() {
        let mut rows: Vec<(Named, f64, usize)> = history.into_values().collect();
        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.2.cmp(&b.2)));
        out.push(List { kind: Kind::Top, title: TOP.into(), description: FROM.into(), tracks: rows.into_iter().take(TOP_LIMIT).map(|r| r.0).collect() });
    }
    out.sort_by_key(|l| l.kind);
    out
}

/// file picking: the export's zip or its JSON files
pub fn is_export_file(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("zip") || e.eq_ignore_ascii_case("json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const LIBRARY: &str = r#"{"tracks":[{"artist":"Daft Punk","album":"Discovery","track":"One More Time","uri":"spotify:track:1"},{"artist":"Daft Punk","album":"Discovery","track":"One More Time","uri":"spotify:track:1"},{"artist":"Burial","album":"Untrue","track":"Archangel","uri":"spotify:track:2"}]}"#;
    const PLAYLISTS: &str = r#"{"playlists":[{"name":"Ночь","items":[{"track":{"trackName":"Archangel","artistName":"Burial","albumName":"Untrue","trackUri":"spotify:track:2"}},{"track":null,"episode":{}}]},{"name":"Empty","items":[]}]}"#;
    const HISTORY: &str = r#"[{"artistName":"A","trackName":"Short","msPlayed":1000},{"artistName":"B","trackName":"Long","msPlayed":200000},{"master_metadata_album_artist_name":"C","master_metadata_track_name":"Ext","ms_played":500000}]"#;

    #[test]
    fn turns_likes_playlists_and_history_into_playlists() {
        let files: BTreeMap<String, String> = [("YourLibrary.json", LIBRARY), ("Playlist1.json", PLAYLISTS), ("StreamingHistory_music_0.json", HISTORY)].into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        let out = parse(&files);
        let shape: Vec<(Kind, &str, usize)> = out.iter().map(|l| (l.kind, l.title.as_str(), l.tracks.len())).collect();
        assert_eq!(shape, vec![(Kind::Liked, LIKED, 2), (Kind::Top, TOP, 2), (Kind::Playlist, "Ночь", 1)]);
        assert_eq!(out[1].tracks[0].title, "Ext");
        assert_eq!((out[0].tracks[0].title.as_str(), out[0].tracks[0].artists.clone()), ("One More Time", vec!["Daft Punk".to_string()]));
    }

    /// a zip as Spotify sends it: a folder, a deflated JSON and a file that is not JSON
    fn zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let (mut out, mut central) = (Vec::new(), Vec::new());
        for (name, data, deflate) in entries {
            let body = if *deflate {
                let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            let offset = out.len() as u32;
            let method: u16 = if *deflate { 8 } else { 0 };
            out.extend([0x50, 0x4b, 3, 4, 20, 0, 0, 0]);
            out.extend(method.to_le_bytes());
            out.extend([0u8; 8]);
            out.extend((body.len() as u32).to_le_bytes());
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes());
            out.extend([0, 0]);
            out.extend(name.as_bytes());
            out.extend(&body);
            central.extend([0x50, 0x4b, 1, 2, 20, 0, 20, 0, 0, 0]);
            central.extend(method.to_le_bytes());
            central.extend([0u8; 8]);
            central.extend((body.len() as u32).to_le_bytes());
            central.extend((data.len() as u32).to_le_bytes());
            central.extend((name.len() as u16).to_le_bytes());
            central.extend([0u8; 12]);
            central.extend(offset.to_le_bytes());
            central.extend(name.as_bytes());
        }
        let at = out.len() as u32;
        out.extend(&central);
        out.extend([0x50, 0x4b, 5, 6, 0, 0, 0, 0]);
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((central.len() as u32).to_le_bytes());
        out.extend(at.to_le_bytes());
        out.extend([0, 0]);
        out
    }

    #[test]
    fn reads_the_zip_spotify_sends() {
        let z = zip(&[("Spotify Account Data/YourLibrary.json", LIBRARY.as_bytes(), true), ("Spotify Account Data/ReadMeFirst.pdf", b"x", false), ("Spotify Account Data/Playlist1.json", PLAYLISTS.as_bytes(), false)]);
        let files = unzip_json(&z).unwrap();
        assert_eq!(files.keys().collect::<Vec<_>>(), vec!["Playlist1.json", "YourLibrary.json"]);
        assert_eq!(files["YourLibrary.json"], LIBRARY);
        assert!(unzip_json(b"not a zip").is_err());
    }
}
