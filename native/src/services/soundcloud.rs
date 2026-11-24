//! SoundCloud through its public api-v2, with the client id its own site ships (as
//! src/services/soundcloud/api.ts and mappers.ts). A rejected id is fetched anew once.

use std::sync::Mutex;

use serde_json::Value;

use super::http::{get_text, n, s};
use crate::model::{Artist, ArtistSummary, CollectionKind, CollectionSummary, Source, Track};

const API: &str = "https://api-v2.soundcloud.com";
/// previews are 30 seconds long
const PREVIEW_MS: f64 = 31_000.0;
const DAY_MS: f64 = 24.0 * 3600.0 * 1000.0;

static CLIENT_ID: Mutex<Option<String>> = Mutex::new(None);

/// SoundCloud's own client id, read from its site's scripts and kept for a day in
/// soundcloud.json (the file the Tauri build keeps too)
fn client_id(force: bool) -> Result<String, String> {
    let own = crate::state::get().settings.read().unwrap().soundcloud_client_id.trim().to_string();
    if !own.is_empty() {
        return Ok(own);
    }
    if !force {
        if let Some(id) = CLIENT_ID.lock().unwrap().clone() {
            return Ok(id);
        }
    }
    let file = crate::state::get().data_dir.join("soundcloud.json");
    let cached: Option<(String, f64)> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| Some((v.get("id")?.as_str()?.to_string(), v.get("at")?.as_f64()?)));
    if let Some((id, at)) = &cached {
        if !force && crate::store::state::now_ms() - at < DAY_MS {
            *CLIENT_ID.lock().unwrap() = Some(id.clone());
            return Ok(id.clone());
        }
    }
    let html = get_text("https://soundcloud.com/")?;
    // the id usually sits in one of the last bundles
    for url in crate::services_shared::script_urls(&html).into_iter().rev() {
        if let Some(id) = get_text(&url).ok().and_then(|t| crate::services_shared::extract_client_id(&t)) {
            let _ = std::fs::write(&file, serde_json::json!({ "id": id, "at": crate::store::state::now_ms() }).to_string());
            *CLIENT_ID.lock().unwrap() = Some(id.clone());
            return Ok(id);
        }
    }
    cached.map(|c| c.0).ok_or_else(|| "SoundCloud: client id не найден".to_string())
}

/// GET from api-v2; a rejected client id is refreshed once
pub fn get(path: &str, params: &[(&str, &str)]) -> Result<Value, String> {
    crate::services::region::guard_soundcloud()?;
    for attempt in 0..2 {
        let base = if path.starts_with("http") { path.to_string() } else { format!("{API}{path}") };
        let mut url = url::Url::parse(&base).map_err(|e| e.to_string())?;
        {
            let mut q = url.query_pairs_mut();
            for (k, v) in params {
                q.append_pair(k, v);
            }
            q.append_pair("client_id", &client_id(attempt > 0)?);
        }
        let res = super::http::get(url.as_str(), &[])?;
        let status = res.status().as_u16();
        if status == 401 || status == 403 {
            continue;
        }
        if !res.status().is_success() {
            return Err(format!("SoundCloud ответил {status}"));
        }
        return res.json().map_err(|e| e.to_string());
    }
    Err("SoundCloud отклонил запрос".into())
}

/// SoundCloud serves "-large" (100px) artwork by default; ask for a bigger one
pub fn artwork_url(raw: &Value, size: u32) -> Option<String> {
    let url = s(raw)?;
    // SoundCloud's grey placeholder has no bigger sizes (they 404)
    if url.contains("default_avatar") || url.contains("placeholder") {
        return None;
    }
    let re = regex::Regex::new(r"-(large|t\d+x\d+|crop|original)(\.\w+)$").ok()?;
    Some(re.replace(&url, format!("-t{size}x{size}$2")).to_string())
}

fn is_preview_only(raw: &Value) -> bool {
    if raw.get("policy").and_then(Value::as_str) == Some("SNIP") {
        return true;
    }
    match (raw.get("full_duration").and_then(n), raw.get("duration").and_then(n)) {
        (Some(full), Some(given)) => given <= PREVIEW_MS && full > given + 5000.0,
        _ => false,
    }
}

pub fn map_track(raw: &Value) -> Option<Track> {
    if raw.get("kind").and_then(Value::as_str) != Some("track") {
        return None;
    }
    let id = raw.get("id").and_then(n)?;
    let title = raw.get("title").and_then(s)?;
    let user = raw.get("user");
    let meta = raw.get("publisher_metadata");
    let artist = meta.and_then(|m| m.get("artist")).and_then(s).or_else(|| user.and_then(|u| u.get("username")).and_then(s)).unwrap_or_else(|| "SoundCloud".into());
    let user_id = user.and_then(|u| u.get("id")).and_then(n);
    Some(Track {
        id: format!("sc:{}", id as u64),
        source: Source::Soundcloud,
        title,
        artists: vec![Artist { id: user_id.map(|u| format!("scu:{}", u as u64)).unwrap_or_else(|| "unknown".into()), name: artist }],
        duration_ms: raw.get("full_duration").and_then(n).or_else(|| raw.get("duration").and_then(n)).unwrap_or(0.0),
        artwork: raw.get("artwork_url").and_then(|a| artwork_url(a, 500)).or_else(|| user.and_then(|u| u.get("avatar_url")).and_then(|a| artwork_url(a, 500))),
        plays: raw.get("playback_count").and_then(n),
        permalink: raw.get("permalink_url").and_then(s),
        preview: is_preview_only(raw),
        explicit: false,
    })
}

pub fn map_user(raw: &Value) -> Option<ArtistSummary> {
    if raw.get("kind").and_then(Value::as_str) != Some("user") {
        return None;
    }
    let id = raw.get("id").and_then(n)?;
    Some(ArtistSummary { id: format!("scu:{}", id as u64), name: raw.get("username").and_then(s)?, avatar: raw.get("avatar_url").and_then(|a| artwork_url(a, 300)), followers: raw.get("followers_count").and_then(n) })
}

pub fn map_collection(raw: &Value) -> Option<CollectionSummary> {
    let kind = raw.get("kind").and_then(Value::as_str)?;
    if kind != "playlist" && kind != "system-playlist" {
        return None;
    }
    let id = raw.get("id").and_then(n)?;
    let title = raw.get("title").and_then(s)?;
    let user = raw.get("user");
    let set_type = raw.get("set_type").and_then(Value::as_str).unwrap_or("");
    let album = raw.get("is_album").and_then(Value::as_bool) == Some(true) || set_type == "album" || set_type == "ep";
    let year = ["release_date", "published_at", "created_at"].iter().find_map(|k| raw.get(*k).and_then(Value::as_str)).and_then(|d| d.get(..4)?.parse().ok());
    let first_cover = raw.get("tracks").and_then(Value::as_array).and_then(|t| t.iter().find_map(|x| x.get("artwork_url").and_then(|a| artwork_url(a, 500))));
    let user_id = user.and_then(|u| u.get("id")).and_then(n);
    let username = user.and_then(|u| u.get("username")).and_then(s);
    Some(CollectionSummary {
        id: format!("scp:{}", id as u64),
        kind: if album { CollectionKind::Album } else { CollectionKind::Playlist },
        title,
        subtitle: username.clone().unwrap_or_else(|| "SoundCloud".into()),
        // a playlist without its own picture shows its first song's cover on SoundCloud
        artwork: raw.get("artwork_url").and_then(|a| artwork_url(a, 500)).or(first_cover),
        year,
        track_count: raw.get("track_count").and_then(Value::as_u64).map(|c| c as u32),
        artist: user_id.map(|u| Artist { id: format!("scu:{}", u as u64), name: username.unwrap_or_default() }),
    })
}

fn collection(v: &Value) -> Vec<&Value> {
    v.get("collection").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default()
}

pub fn search(query: &str) -> Result<(Vec<Track>, Vec<ArtistSummary>), String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok((vec![], vec![]));
    }
    let (tracks, users) = std::thread::scope(|sc| {
        let t = sc.spawn(|| get("/search/tracks", &[("q", q), ("limit", "30")]));
        let u = sc.spawn(|| get("/search/users", &[("q", q), ("limit", "6")]).unwrap_or(Value::Null));
        (t.join().unwrap_or_else(|_| Err("поиск не удался".into())), u.join().unwrap_or(Value::Null))
    });
    let tracks = tracks?;
    Ok((collection(&tracks).into_iter().filter_map(map_track).collect(), collection(&users).into_iter().filter_map(map_user).collect()))
}

pub fn search_collections(query: &str, kind: CollectionKind) -> Result<Vec<CollectionSummary>, String> {
    let path = if kind == CollectionKind::Album { "/search/albums" } else { "/search/playlists_without_albums" };
    Ok(collection(&get(path, &[("q", query), ("limit", "20")])?).into_iter().filter_map(map_collection).collect())
}

/// SoundCloud answers 404 to the "top" chart since November 2026; only "trending" is left
pub fn charts() -> Result<Vec<Track>, String> {
    let body = get("/charts", &[("kind", "trending"), ("genre", "soundcloud:genres:all-music"), ("limit", "20")])?;
    Ok(collection(&body).into_iter().filter_map(|row| row.get("track").and_then(map_track)).collect())
}

pub fn related(track_id: &str) -> Result<Vec<Track>, String> {
    let id = track_id.trim_start_matches("sc:");
    Ok(collection(&get(&format!("/tracks/{id}/related"), &[("limit", "25")])?).into_iter().filter_map(map_track).collect())
}

pub struct ScUser {
    pub artist: Option<ArtistSummary>,
    pub tracks: Vec<Track>,
    pub popular: Vec<Track>,
}

pub fn user(user_id: &str) -> Result<ScUser, String> {
    let id = user_id.trim_start_matches("scu:").trim_start_matches("sc:");
    let (u, t, p) = std::thread::scope(|sc| {
        let u = sc.spawn(|| get(&format!("/users/{id}"), &[]));
        let t = sc.spawn(|| get(&format!("/users/{id}/tracks"), &[("limit", "50")]));
        let p = sc.spawn(|| get(&format!("/users/{id}/toptracks"), &[("limit", "10")]).unwrap_or(Value::Null));
        (u.join().unwrap_or_else(|_| Err("".into())), t.join().unwrap_or_else(|_| Err("".into())), p.join().unwrap_or(Value::Null))
    });
    let u = u?;
    let t = t?;
    Ok(ScUser { artist: map_user(&u), tracks: collection(&t).into_iter().filter_map(map_track).collect(), popular: collection(&p).into_iter().filter_map(map_track).collect() })
}

/// full track objects for ids (playlists carry only ids for most of their songs)
fn tracks_by_ids(ids: &[u64]) -> Result<Vec<Track>, String> {
    let mut out = Vec::new();
    for chunk in ids.chunks(50) {
        let joined = chunk.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let body = get("/tracks", &[("ids", &joined)])?;
        let found: Vec<Track> = body.as_array().map(|a| a.iter().filter_map(map_track).collect()).unwrap_or_default();
        for id in chunk {
            if let Some(t) = found.iter().find(|t| t.id == format!("sc:{id}")) {
                out.push(t.clone());
            }
        }
    }
    Ok(out)
}

fn ids_of(body: &Value) -> Vec<u64> {
    body.get("tracks").and_then(Value::as_array).map(|a| a.iter().filter_map(|t| t.get("id").and_then(Value::as_u64)).collect()).unwrap_or_default()
}

pub fn playlist(id: &str) -> Result<(Option<CollectionSummary>, Vec<Track>), String> {
    let body = get(&format!("/playlists/{}", id.trim_start_matches("scp:")), &[])?;
    Ok((map_collection(&body), tracks_by_ids(&ids_of(&body))?))
}

/// a soundcloud.com link: a track, a playlist/album or a profile
pub fn resolve_url(link: &str) -> Result<(String, Vec<Track>, Option<String>), String> {
    let body = get("/resolve", &[("url", link)])?;
    match body.get("kind").and_then(Value::as_str) {
        Some("track") => {
            let t = map_track(&body);
            Ok((t.as_ref().map(|t| t.title.clone()).unwrap_or_else(|| "SoundCloud".into()), t.into_iter().collect(), None))
        }
        Some("playlist" | "system-playlist") => {
            let tracks = tracks_by_ids(&ids_of(&body))?;
            let art = map_collection(&body).and_then(|c| c.artwork).or_else(|| tracks.iter().find_map(|t| t.artwork.clone()));
            Ok((body.get("title").and_then(s).unwrap_or_else(|| "SoundCloud".into()), tracks, art))
        }
        Some("user") => {
            let id = body.get("id").and_then(Value::as_u64).ok_or("нет профиля")?;
            let u = user(&id.to_string())?;
            Ok((u.artist.map(|a| a.name).unwrap_or_else(|| "SoundCloud".into()), u.tracks, None))
        }
        _ => Err("Ссылка SoundCloud не похожа на трек или плейлист".into()),
    }
}

// ---- the sound -------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Transcoding {
    pub url: String,
    pub protocol: String,
    pub mime: String,
    pub preset: String,
    pub quality: String,
}

pub fn transcodings(raw: &Value) -> Vec<Transcoding> {
    let full = raw.get("full_duration").and_then(n);
    let Some(list) = raw.pointer("/media/transcodings").and_then(Value::as_array) else { return vec![] };
    list.iter()
        .filter_map(|t| {
            let url = t.get("url").and_then(s)?;
            let protocol = t.pointer("/format/protocol").and_then(s)?;
            if protocol != "progressive" && protocol != "hls" {
                return None;
            }
            // a preview is not the song (some are not marked as one: they are told by their length)
            let snipped = t.get("snipped").and_then(Value::as_bool) == Some(true) || matches!((full, t.get("duration").and_then(n)), (Some(f), Some(l)) if l <= PREVIEW_MS && f > l + 5000.0);
            if snipped {
                return None;
            }
            Some(Transcoding { url, protocol, mime: t.pointer("/format/mime_type").and_then(s).unwrap_or_default(), preset: t.get("preset").and_then(s).unwrap_or_default(), quality: t.get("quality").and_then(s).unwrap_or_default() })
        })
        .collect()
}

/// SoundCloud's adaptive playlist ("abr_sq", "audio/mpegurl"): a list of other playlists, which
/// SoundCloud answers with 404 — tried last, so a song does not start with a dead request
fn is_adaptive(t: &Transcoding) -> bool {
    t.preset.to_lowercase().starts_with("abr") || t.mime.contains("mpegurl")
}

/// how many kbit/s a format carries: from its preset ("aac_160k"), else what SoundCloud uses for
/// that codec (mp3 128, Opus 64)
fn kbps_of(t: &Transcoding) -> u32 {
    if let Some(c) = regex::Regex::new(r"(?i)(\d{2,3})k").ok().and_then(|r| r.captures(&t.preset)) {
        return c[1].parse().unwrap_or(0);
    }
    if t.mime.contains("opus") || t.mime.contains("ogg") {
        64
    } else if t.mime.contains("mp4") || t.mime.contains("aac") {
        160
    } else {
        128
    }
}

/// Best sound first: the better encoding (Go+), then by bitrate (AAC 160 over mp3 128 over AAC 96
/// over Opus 64 — the native decoder plays them all, so the browser's "mp3 first" does not
/// apply), a single file before HLS at the same rate; the adaptive playlist last.
pub fn rank(mut list: Vec<Transcoding>) -> Vec<Transcoding> {
    list.sort_by_key(|t| (is_adaptive(t), t.quality != "hq", std::cmp::Reverse(kbps_of(t)), t.protocol != "progressive"));
    list
}

pub fn quality_of(t: &Transcoding) -> String {
    let codec = if t.mime.contains("opus") || t.mime.contains("ogg") {
        "opus"
    } else if t.mime.contains("mp4") || t.mime.contains("aac") || t.preset.to_lowercase().starts_with("aac") {
        "aac"
    } else {
        "mp3"
    };
    let named = regex::Regex::new(r"(?i)(\d{2,3})k").ok().and_then(|r| r.captures(&t.preset).map(|c| c[1].to_string()));
    match named {
        Some(rate) => format!("{codec} {rate} кбит/с"),
        None => format!("{codec} ~{} кбит/с", match codec {
            "opus" => "64",
            "aac" => "160",
            _ => "128",
        }),
    }
}

pub struct ScStream {
    pub url: String,
    pub hls: bool,
    pub quality: String,
}

/// every format of a song the player can play, best first, each asked only when the one before
/// did not work out
pub fn streams(track_id: &str) -> Result<Vec<Transcoding>, String> {
    let id = track_id.trim_start_matches("sc:");
    let track = get(&format!("/tracks/{id}"), &[])?;
    let auth = track.get("track_authorization").and_then(s);
    let list = rank(transcodings(&track));
    if list.is_empty() {
        let listed: Vec<String> = track
            .pointer("/media/transcodings")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|t| Some(format!("{} {}", t.pointer("/format/protocol")?.as_str()?, t.pointer("/format/mime_type")?.as_str()?.split(';').next()?.replace("audio/", "")))).collect())
            .unwrap_or_default();
        return Err(if listed.is_empty() { "SoundCloud не дал ни одного формата для этого трека".into() } else { format!("SoundCloud отдаёт этот трек только в защищённом виде или отрывком (форматы: {})", listed.join(", ")) });
    }
    Ok(list.into_iter().map(|mut t| {
        if let Some(a) = &auth {
            t.url = format!("{}{}track_authorization={}", t.url, if t.url.contains('?') { "&" } else { "?" }, a);
        }
        t
    }).collect())
}

/// the address of one format's sound (SoundCloud hands it out for a short while)
pub fn stream_url(t: &Transcoding) -> Result<ScStream, String> {
    let body = get(&t.url, &[])?;
    let url = body.get("url").and_then(s).ok_or("SoundCloud не отдал поток")?;
    Ok(ScStream { url, hls: t.protocol == "hls", quality: quality_of(t) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_a_track_and_asks_for_bigger_covers() {
        let raw = json!({ "kind": "track", "id": 123, "title": " Song ", "duration": 30000, "full_duration": 200000, "policy": "SNIP",
            "user": { "id": 9, "username": "uploader", "avatar_url": "https://i1.sndcdn.com/avatars-x-large.jpg" },
            "publisher_metadata": { "artist": "Real Artist" }, "artwork_url": "https://i1.sndcdn.com/artworks-abc-large.jpg", "playback_count": 7 });
        let t = map_track(&raw).unwrap();
        assert_eq!(t.id, "sc:123");
        assert_eq!(t.title, "Song");
        assert_eq!(t.artists[0].name, "Real Artist");
        assert_eq!(t.artists[0].id, "scu:9");
        assert_eq!(t.duration_ms, 200000.0);
        assert!(t.preview);
        assert_eq!(t.artwork.as_deref(), Some("https://i1.sndcdn.com/artworks-abc-t500x500.jpg"));
        assert_eq!(artwork_url(&json!("https://a1.sndcdn.com/images/default_avatar_large.png"), 500), None);
    }

    #[test]
    fn ranks_formats_and_skips_previews() {
        let raw = json!({ "full_duration": 200000, "media": { "transcodings": [
            { "url": "u1", "preset": "opus_0_0", "format": { "protocol": "hls", "mime_type": "audio/ogg; codecs=\"opus\"" } },
            { "url": "u2", "preset": "mp3_0_0", "format": { "protocol": "hls", "mime_type": "audio/mpeg" } },
            { "url": "u3", "preset": "mp3_0_1", "format": { "protocol": "progressive", "mime_type": "audio/mpeg" } },
            { "url": "u4", "preset": "aac_160k", "quality": "hq", "format": { "protocol": "hls", "mime_type": "audio/mp4; codecs=\"mp4a.40.2\"" } },
            { "url": "u5", "snipped": true, "format": { "protocol": "progressive", "mime_type": "audio/mpeg" } },
            { "url": "u6", "duration": 30000, "format": { "protocol": "progressive", "mime_type": "audio/mpeg" } },
            { "url": "u7", "format": { "protocol": "ctr-encrypted-hls", "mime_type": "audio/mp4" } }
        ] } });
        let order: Vec<String> = rank(transcodings(&raw)).into_iter().map(|t| t.url).collect();
        assert_eq!(order, ["u4", "u3", "u2", "u1"]);
        let q = rank(transcodings(&raw));
        assert_eq!(quality_of(&q[0]), "aac 160 кбит/с");
        assert_eq!(quality_of(&q[1]), "mp3 ~128 кбит/с");
    }

    #[test]
    fn the_best_sound_first_and_the_adaptive_playlist_last() {
        // what SoundCloud lists for most songs today
        let raw = json!({ "media": { "transcodings": [
            { "url": "abr", "preset": "abr_sq", "quality": "sq", "format": { "protocol": "hls", "mime_type": "audio/mpegurl" } },
            { "url": "aac160", "preset": "aac_160k", "quality": "sq", "format": { "protocol": "hls", "mime_type": "audio/mp4; codecs=\"mp4a.40.2\"" } },
            { "url": "aac96", "preset": "aac_96k", "quality": "lq", "format": { "protocol": "hls", "mime_type": "audio/mp4; codecs=\"mp4a.40.2\"" } },
            { "url": "mp3", "preset": "mp3_1_0", "format": { "protocol": "progressive", "mime_type": "audio/mpeg" } },
            { "url": "opus", "preset": "opus_0_0", "format": { "protocol": "hls", "mime_type": "audio/ogg; codecs=\"opus\"" } }
        ] } });
        let order: Vec<String> = rank(transcodings(&raw)).into_iter().map(|t| t.url).collect();
        assert_eq!(order, ["aac160", "mp3", "aac96", "opus", "abr"]);
    }

    #[test]
    fn maps_collections() {
        let raw = json!({ "kind": "playlist", "id": 5, "title": "Mix", "set_type": "ep", "release_date": "2021-03-01T00:00:00Z", "track_count": 4, "user": { "id": 1, "username": "u" },
            "tracks": [{ "id": 1, "artwork_url": "https://i1.sndcdn.com/artworks-q-large.jpg" }] });
        let c = map_collection(&raw).unwrap();
        assert_eq!(c.id, "scp:5");
        assert_eq!(c.kind, CollectionKind::Album);
        assert_eq!(c.year, Some(2021));
        assert_eq!(c.artwork.as_deref(), Some("https://i1.sndcdn.com/artworks-q-t500x500.jpg"));
    }
}
