//! "Импорт по ссылке или коду" (src/library/importLink.ts, playlistCode.ts, trackList.ts,
//! matchTracks.ts, services/yandex/music.ts): a playlist, album or song link from a service, a
//! pasted list of songs (a VK page), or an EvPlayer code becomes one of your playlists or goes
//! to your likes. Services we cannot play (Spotify, Deezer, Yandex Music) give only names;
//! each song is looked up on YouTube Music, then SoundCloud, with the rules of
//! player/matching.rs, and songs without a confident match are left out instead of guessed.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde_json::Value;

use crate::model::{Artist, Source, Track};
use crate::player::matching::best_match;
use crate::services::http::{get_json, get_text};
use crate::services::{soundcloud, youtube};

#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Soundcloud,
    Youtube,
    Spotify,
    Deezer,
    Yandex,
    Apple,
    Code,
}

#[derive(Default, Debug)]
pub struct Fetched {
    pub title: String,
    pub description: Option<String>,
    pub tracks: Vec<Track>,
    pub named: Vec<Named>,
    pub artwork: Option<String>,
    pub icon: Option<String>,
    pub likes: bool,
    /// the source showed only part of the list
    pub truncated: bool,
}

// ---- playlist codes -----------------------------------------------------------------------------

const MAX_ICON: usize = 60_000;

/// characters that get into a code from a text file or a chat: byte-order mark, zero-width spaces
fn strip_invisible(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}')).collect()
}

fn code_body_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '/' | '=')
}

/// Every playlist code in a pasted text. Several codes: each line is its own; one code wrapped
/// over several lines is joined again.
pub fn extract_codes(text: &str) -> Vec<String> {
    let clean = strip_invisible(text);
    let starts: Vec<usize> = clean.match_indices("EVP1.").chain(clean.match_indices("EVP2.")).map(|(i, _)| i).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &at)| {
            let end = starts.get(n + 1).copied().unwrap_or(clean.len());
            let chunk = &clean[at..end];
            let head = &chunk[..5];
            let mut parts = Vec::new();
            for token in chunk[5..].split_whitespace() {
                if !token.chars().all(code_body_char) {
                    break;
                }
                parts.push(token);
                if starts.len() > 1 {
                    break;
                }
            }
            format!("{head}{}", parts.concat())
        })
        .collect()
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64(text: &str) -> Option<Vec<u8>> {
    let n: String = text.chars().filter(|c| !c.is_whitespace()).collect::<String>().trim_end_matches('=').replace('+', "-").replace('/', "_");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(n).ok()
}

pub struct Shared {
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub tracks: Vec<Track>,
    pub likes: bool,
}

/// a playlist as a line of text: EVP2.<base64url of deflate-raw json> (the page's own format)
pub fn encode_code(p: &Shared) -> String {
    let short = |s: Source| match s {
        Source::Soundcloud => "s",
        Source::Youtube => "y",
        Source::Local => "l",
    };
    let t: Vec<Value> = p.tracks.iter().map(|t| serde_json::json!([short(t.source), t.id, t.title, t.duration_ms, t.artwork.clone().unwrap_or_default(), t.artists.iter().map(|a| [a.id.clone(), a.name.clone()]).collect::<Vec<_>>()])).collect();
    let mut wire = serde_json::json!({ "v": 2, "n": p.title, "t": t });
    if let Some(d) = &p.description {
        wire["d"] = serde_json::json!(d);
    }
    if let Some(c) = p.icon.as_ref().filter(|c| c.len() <= MAX_ICON) {
        wire["c"] = serde_json::json!(c);
    }
    if p.likes {
        wire["l"] = serde_json::json!(1);
    }
    let raw = wire.to_string();
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    if enc.write_all(raw.as_bytes()).is_ok() {
        if let Ok(z) = enc.finish() {
            return format!("EVP2.{}", b64(&z));
        }
    }
    format!("EVP1.{}", b64(raw.as_bytes()))
}

pub fn decode_code(code: &str) -> Result<Shared, String> {
    let first = extract_codes(code).into_iter().next().ok_or("Это не код плейлиста")?;
    let (kind, body) = first.split_at(4);
    let mut bytes = unb64(&body[1..]).ok_or("Код повреждён")?;
    if kind == "EVP2" {
        let mut out = Vec::new();
        flate2::read::DeflateDecoder::new(&bytes[..]).read_to_end(&mut out).map_err(|_| "Код повреждён")?;
        bytes = out;
    }
    let wire: Value = serde_json::from_slice(&bytes).map_err(|_| "Код повреждён")?;
    let title = wire.get("n").and_then(Value::as_str).ok_or("Код повреждён")?.to_string();
    let rows = wire.get("t").and_then(Value::as_array).ok_or("Код повреждён")?;
    let tracks = rows
        .iter()
        .filter_map(|r| {
            let r = r.as_array()?;
            let source = match r.first()?.as_str()? {
                "s" => Source::Soundcloud,
                "y" => Source::Youtube,
                "l" => Source::Local,
                _ => return None,
            };
            let artists = r.get(5).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| Some(Artist { id: x.get(0)?.as_str()?.into(), name: x.get(1)?.as_str()?.into() })).collect()).unwrap_or_default();
            Some(Track {
                id: r.get(1)?.as_str()?.into(),
                source,
                title: r.get(2)?.as_str()?.into(),
                duration_ms: r.get(3).and_then(Value::as_f64).unwrap_or(0.0),
                artwork: r.get(4).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
                artists,
                ..Default::default()
            })
        })
        .collect();
    Ok(Shared {
        title,
        description: wire.get("d").and_then(Value::as_str).map(str::to_string),
        icon: wire.get("c").and_then(Value::as_str).map(str::to_string),
        tracks,
        likes: wire.get("l").and_then(Value::as_i64) == Some(1),
    })
}

// ---- a pasted list of songs -----------------------------------------------------------------

fn is_duration(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    (2..=3).contains(&parts.len()) && parts.iter().all(|p| !p.is_empty() && p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit())) && parts.last().is_some_and(|l| l.len() == 2)
}

fn duration_of(s: &str) -> Option<f64> {
    is_duration(s).then(|| s.split(':').fold(0.0, |t, p| t * 60.0 + p.parse::<f64>().unwrap_or(0.0)) * 1000.0)
}

fn clean_line(line: &str) -> String {
    let lead = regex::Regex::new(r"^\s*(?:\d{1,4}[.)]\s*|[•·*▶►-]\s+)").unwrap();
    let tail = regex::Regex::new(r"\s+(?:(?:\d{1,2}:)?\d{1,2}:\d{2}|E|🅴)\s*$").unwrap();
    let mut s = lead.replace(line, "").trim().to_string();
    for _ in 0..2 {
        s = tail.replace(&s, "").trim().to_string();
    }
    s
}

fn artists_of(text: &str) -> Vec<String> {
    regex::Regex::new(r"(?i)\s*(?:,|&|\bfeat\.?|\bft\.?|\bx\b)\s*").unwrap().split(text).map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect()
}

/// "Artist — Title" lines, or rows copied from a VK playlist page (title, artist, length)
pub fn parse_track_list(text: &str) -> Vec<Named> {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let sep = regex::Regex::new(r"\s+[-–—]\s+").unwrap();
    let tail_len = regex::Regex::new(r"((?:\d{1,2}:)?\d{1,2}:\d{2})\s*$").unwrap();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if is_duration(line) {
            i += 1;
            continue;
        }
        let cleaned = clean_line(line);
        let split: Vec<&str> = sep.splitn(&cleaned, 2).collect();
        if split.len() == 2 && !split[0].is_empty() && !split[1].is_empty() {
            let length = tail_len.captures(line).and_then(|m| duration_of(&m[1])).or_else(|| lines.get(i + 1).and_then(|n| duration_of(n)));
            out.push(Named { artists: artists_of(split[0]), title: split[1].to_string(), duration_ms: length });
            i += 1;
            continue;
        }
        if let (Some(artist), Some(len)) = (lines.get(i + 1), lines.get(i + 2)) {
            if is_duration(len) && !is_duration(artist) {
                out.push(Named { title: cleaned, artists: artists_of(&clean_line(artist)), duration_ms: duration_of(len) });
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    out.retain(|t| !t.title.is_empty() && t.title.chars().count() < 200);
    out
}

pub fn looks_like_track_list(text: &str) -> bool {
    let t = text.trim_start();
    !t.starts_with("http://") && !t.starts_with("https://") && !t.starts_with("EVP") && !parse_track_list(text).is_empty()
}

// ---- links ---------------------------------------------------------------------------------------

fn yandex_host(host: &str) -> bool {
    regex::Regex::new(r"^(music\.yandex\.(ru|com|by|kz|uz)|next\.music\.yandex\.ru)$").unwrap().is_match(host)
}

pub fn link_kind(text: &str) -> Option<Kind> {
    let v = text.trim();
    if !extract_codes(v).is_empty() {
        return Some(Kind::Code);
    }
    let url = url::Url::parse(v).ok()?;
    let host = url.host_str()?.trim_start_matches("www.").trim_start_matches("m.").to_string();
    Some(match host.as_str() {
        "soundcloud.com" | "on.soundcloud.com" => Kind::Soundcloud,
        "youtube.com" | "music.youtube.com" | "youtu.be" => Kind::Youtube,
        "open.spotify.com" | "spotify.link" => Kind::Spotify,
        "deezer.com" | "link.deezer.com" => Kind::Deezer,
        "music.apple.com" => Kind::Apple,
        h if yandex_host(h) => Kind::Yandex,
        _ => return None,
    })
}

fn youtube_link(url: &url::Url) -> Result<Fetched, String> {
    if let Some(list) = url.query_pairs().find(|(k, _)| k == "list").map(|(_, v)| v.to_string()) {
        let id = if list.starts_with("VL") { list } else { format!("VL{list}") };
        let c = youtube::collection(&id, 500)?;
        return Ok(Fetched { title: c.title, tracks: c.tracks, artwork: c.artwork, description: Some("Импортировано из YouTube".into()), ..Default::default() });
    }
    let id = if url.host_str() == Some("youtu.be") { url.path().trim_start_matches('/').to_string() } else { url.query_pairs().find(|(k, _)| k == "v").map(|(_, v)| v.to_string()).ok_or("В ссылке нет видео или плейлиста")? };
    let mut o = url::Url::parse("https://www.youtube.com/oembed?format=json").unwrap();
    o.query_pairs_mut().append_pair("url", &format!("https://www.youtube.com/watch?v={id}"));
    let info = get_json(o.as_str(), &[]).unwrap_or(Value::Null);
    let author = info.get("author_name").and_then(Value::as_str).map(str::to_string);
    let t = Track {
        id: format!("yt:{id}"),
        source: Source::Youtube,
        title: info.get("title").and_then(Value::as_str).unwrap_or("YouTube").to_string(),
        artists: author.map(|a| vec![Artist { id: format!("ytn:{a}"), name: a }]).unwrap_or_default(),
        artwork: info.get("thumbnail_url").and_then(Value::as_str).map(str::to_string),
        permalink: Some(format!("https://music.youtube.com/watch?v={id}")),
        ..Default::default()
    };
    Ok(Fetched { title: t.title.clone(), tracks: vec![t], ..Default::default() })
}

/// Spotify, best source first: your account or own app (the official API, any length), then
/// the guest access of Spotify's web player (a whole playlist, no sign-in), then the embed page
/// (100 songs)
fn spotify(url: &url::Url, cancel: Option<&AtomicBool>, step: &dyn Fn(&str)) -> Result<Fetched, String> {
    let m = regex::Regex::new(r"/(playlist|album|track)/([A-Za-z0-9]+)").unwrap();
    let c = m.captures(url.path()).ok_or("Ссылка Spotify должна вести на плейлист, альбом или трек")?;
    let (kind, id) = (c[1].to_string(), c[2].to_string());
    let read = |n: usize, total: usize| step(&format!("Spotify: прочитано {n} из {total}"));
    if crate::services::spotify::has_access() {
        let (title, named, artwork) = crate::services::spotify::collection(&kind, &id, cancel, &read)?;
        return Ok(Fetched { title, named, artwork, description: Some("Импортировано из Spotify".into()), ..Default::default() });
    }
    let embed = spotify_embed(&kind, &id)?;
    if kind == "playlist" {
        // the web player changed or Spotify refused: the embed page still works
        if let Ok((named, total)) = crate::services::spotify::guest_playlist(&id, cancel, &read) {
            if !named.is_empty() && named.len() >= embed.named.len() {
                let truncated = total.is_some_and(|t| named.len() < t);
                return Ok(Fetched { named, truncated, ..embed });
            }
        }
    }
    Ok(embed)
}

/// Spotify's embed page (the one other sites use for the Spotify player): at most 100 songs
fn spotify_embed(kind: &str, id: &str) -> Result<Fetched, String> {
    let (kind, id) = (kind.to_string(), id.to_string());
    let html = get_text(&format!("https://open.spotify.com/embed/{kind}/{id}"))?;
    let json = regex::Regex::new(r#"(?s)<script id="__NEXT_DATA__" type="application/json">(.+?)</script>"#).unwrap().captures(&html).map(|m| m[1].to_string()).ok_or("Spotify не показал список треков")?;
    let v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let e = v.pointer("/props/pageProps/state/data/entity").ok_or("Spotify не показал список треков")?;
    let name = e.get("name").or_else(|| e.get("title")).and_then(Value::as_str).unwrap_or("Spotify").to_string();
    let list: Vec<Named> = match e.get("trackList").and_then(Value::as_array).filter(|l| !l.is_empty()) {
        Some(l) => l.iter().filter_map(|t| Some(Named { title: t.get("title")?.as_str()?.to_string(), artists: t.get("subtitle").and_then(Value::as_str).unwrap_or("").split(", ").map(str::to_string).filter(|s| !s.is_empty()).collect(), duration_ms: t.get("duration").and_then(Value::as_f64) })).collect(),
        None => vec![Named { title: name.clone(), artists: e.get("subtitle").and_then(Value::as_str).unwrap_or("").split(", ").map(str::to_string).collect(), duration_ms: e.get("duration").and_then(Value::as_f64) }],
    };
    let artwork = e.pointer("/coverArt/sources").and_then(Value::as_array).and_then(|s| s.iter().max_by_key(|x| x.get("width").and_then(Value::as_u64).unwrap_or(0))).and_then(|x| x.get("url")).and_then(Value::as_str).map(str::to_string);
    let truncated = kind == "playlist" && list.len() >= 100;
    Ok(Fetched { title: name, named: list, artwork, description: Some("Импортировано из Spotify".into()), truncated, ..Default::default() })
}

fn deezer(url: &url::Url) -> Result<Fetched, String> {
    let c = regex::Regex::new(r"/(playlist|album|track)/(\d+)").unwrap().captures(url.path()).ok_or("Ссылка Deezer должна вести на плейлист, альбом или трек")?;
    let body = get_json(&format!("https://api.deezer.com/{}/{}", &c[1], &c[2]), &[])?;
    if let Some(e) = body.get("error") {
        return Err(e.get("message").and_then(Value::as_str).unwrap_or("Deezer не нашёл ссылку").to_string());
    }
    let row = |t: &Value| Named { title: t.get("title").and_then(Value::as_str).unwrap_or("").to_string(), artists: t.pointer("/artist/name").and_then(Value::as_str).map(|a| vec![a.to_string()]).unwrap_or_default(), duration_ms: t.get("duration").and_then(Value::as_f64).map(|d| d * 1000.0) };
    let named = if &c[1] == "track" { vec![row(&body)] } else { body.pointer("/tracks/data").and_then(Value::as_array).map(|l| l.iter().map(row).collect()).unwrap_or_default() };
    Ok(Fetched {
        title: body.get("title").and_then(Value::as_str).unwrap_or("Deezer").to_string(),
        artwork: body.get("picture_xl").or_else(|| body.get("cover_xl")).and_then(Value::as_str).map(str::to_string),
        description: Some("Импортировано из Deezer".into()),
        named,
        ..Default::default()
    })
}

pub fn yandex_get(path: &str) -> Result<Value, String> {
    let url = format!("https://api.music.yandex.net{path}");
    let res = crate::services::http::get(&url, &[])?;
    let status = res.status().as_u16();
    let body: Value = res.json().unwrap_or(Value::Null);
    let err = body.pointer("/error/name").and_then(Value::as_str).unwrap_or("");
    if status == 451 || err == "Unavailable For Legal Reasons" {
        return Err("Яндекс Музыка недоступна в вашей стране — нужен доступ из региона, где она работает".into());
    }
    if status == 404 || err == "not-found" {
        return Err("Яндекс Музыка не нашла это — проверьте ссылку".into());
    }
    if status == 401 || status == 403 {
        return Err("Это закрыто: сделайте плейлист или профиль публичным в Яндекс Музыке".into());
    }
    body.get("result").cloned().filter(|_| (200..300).contains(&status)).ok_or_else(|| format!("Яндекс Музыка ответила {status}"))
}

pub fn yandex_named(t: &Value) -> Option<Named> {
    let title = t.get("title")?.as_str()?;
    let title = match t.get("version").and_then(Value::as_str) {
        Some(v) if !v.is_empty() => format!("{title} ({v})"),
        _ => title.to_string(),
    };
    Some(Named { title, artists: t.get("artists").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.get("name")?.as_str().map(str::to_string)).collect()).unwrap_or_default(), duration_ms: t.get("durationMs").and_then(Value::as_f64) })
}

fn yandex(url: &url::Url) -> Result<Fetched, String> {
    let path = url.path().trim_end_matches('/');
    let re = |p: &str| regex::Regex::new(p).unwrap();
    let from_playlist = |r: Value| Fetched { title: r.get("title").and_then(Value::as_str).unwrap_or("Яндекс Музыка").to_string(), named: r.get("tracks").and_then(Value::as_array).map(|l| l.iter().filter_map(|t| t.get("track").and_then(yandex_named)).collect()).unwrap_or_default(), description: Some("Импортировано из Яндекс Музыки".into()), ..Default::default() };
    if let Some(m) = re(r"/users/([^/]+)/playlists/(\d+)").captures(path) {
        let owner = percent_encoding::percent_decode_str(&m[1]).decode_utf8_lossy().to_string();
        return Ok(from_playlist(yandex_get(&format!("/users/{}/playlists/{}", urlencoding(&owner), &m[2]))?));
    }
    if let Some(m) = re(r"(?i)/playlists/((?:lk\.)?[0-9a-f-]{20,})").captures(path) {
        return Ok(from_playlist(yandex_get(&format!("/playlist/{}", &m[1]))?));
    }
    if let Some(m) = re(r"/track/(\d+)").captures(path) {
        let l = yandex_get(&format!("/tracks/{}", &m[1]))?;
        let n = l.get(0).and_then(yandex_named);
        return Ok(Fetched { title: n.as_ref().map(|n| n.title.clone()).unwrap_or_else(|| "Яндекс Музыка".into()), named: n.into_iter().collect(), ..Default::default() });
    }
    if let Some(m) = re(r"/album/(\d+)").captures(path) {
        let a = yandex_get(&format!("/albums/{}/with-tracks", &m[1]))?;
        let named = a.get("volumes").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_array).flatten().filter_map(yandex_named).collect()).unwrap_or_default();
        return Ok(Fetched { title: a.get("title").and_then(Value::as_str).unwrap_or("Яндекс Музыка").to_string(), named, description: Some("Импортировано из Яндекс Музыки".into()), ..Default::default() });
    }
    Err("Ссылка Яндекс Музыки должна вести на плейлист, альбом или трек".into())
}

fn urlencoding(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

pub fn fetch(text: &str) -> Result<Fetched, String> {
    fetch_with(text, None, &|_| {})
}

/// read a link, a code or a list of songs; `step` hears what is being read
pub fn fetch_with(text: &str, cancel: Option<&AtomicBool>, step: &dyn Fn(&str)) -> Result<Fetched, String> {
    let kind = link_kind(text);
    if kind.is_none() && looks_like_track_list(text) {
        return Ok(Fetched { title: "Импорт списком".into(), named: parse_track_list(text), description: Some("Импортировано списком песен".into()), ..Default::default() });
    }
    if kind == Some(Kind::Code) {
        let s = decode_code(text.trim())?;
        return Ok(Fetched { title: s.title, description: s.description, tracks: s.tracks, icon: s.icon, likes: s.likes, ..Default::default() });
    }
    let url = url::Url::parse(text.trim()).map_err(|_| "Эту ссылку не получилось распознать")?;
    match kind {
        Some(Kind::Soundcloud) => {
            let (title, tracks, artwork) = soundcloud::resolve_url(url.as_str())?;
            Ok(Fetched { title, tracks, artwork, description: Some("Импортировано из SoundCloud".into()), ..Default::default() })
        }
        Some(Kind::Youtube) => youtube_link(&url),
        Some(Kind::Spotify) => spotify(&url, cancel, step),
        Some(Kind::Deezer) => deezer(&url),
        Some(Kind::Yandex) => yandex(&url),
        Some(Kind::Apple) => Err("Apple Music не даёт читать плейлисты без своего аккаунта разработчика. Экспортируйте плейлист в Spotify или вставьте названия вручную.".into()),
        _ => Err("Эту ссылку не получилось распознать".into()),
    }
}

/// one named song on YouTube Music, then SoundCloud (the artist must match; previews are skipped)
pub fn match_one(w: &Named) -> Option<Track> {
    let query = format!("{} {}", w.artists.first().map(String::as_str).unwrap_or(""), w.title).trim().to_string();
    let target = Track { id: format!("name:{}", w.title), source: Source::Youtube, title: w.title.clone(), artists: w.artists.iter().map(|a| Artist { id: a.clone(), name: a.clone() }).collect(), duration_ms: w.duration_ms.unwrap_or(0.0), ..Default::default() };
    let yt = youtube::search(&query).unwrap_or_default();
    if let Some(t) = best_match(&target, &yt[..yt.len().min(8)], 0.65, true, true) {
        return Some(t.clone());
    }
    let sc = soundcloud::search(&query).map(|r| r.0).unwrap_or_default();
    best_match(&target, &sc[..sc.len().min(10)], 0.65, true, true).cloned()
}

/// all the named songs, four at a time; `progress(done, total, found)`
pub fn match_all(wanted: &[Named], cancel: &AtomicBool, progress: &(dyn Fn(usize, usize, usize) + Sync)) -> Vec<Track> {
    let results: Mutex<Vec<Option<Track>>> = Mutex::new(vec![None; wanted.len()]);
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let found = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..4.min(wanted.len()) {
            s.spawn(|| loop {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(w) = wanted.get(i) else { return };
                let r = match_one(w);
                if r.is_some() {
                    found.fetch_add(1, Ordering::Relaxed);
                }
                results.lock().unwrap()[i] = r;
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                progress(d, wanted.len(), found.load(Ordering::Relaxed));
            });
        }
    });
    let mut seen = std::collections::HashSet::new();
    results.into_inner().unwrap().into_iter().flatten().filter(|t| seen.insert(t.id.clone())).collect()
}

/// what the import panel shows while it works
#[derive(Default, Clone)]
pub struct Progress {
    pub text: String,
    pub done: bool,
    pub ok: bool,
    /// the playlist made (to open it), or "likes"
    pub opened: Option<String>,
}

/// read the link or code, find the songs, save them; runs on its own thread
pub fn run(text: String, to_likes: Option<bool>, cancel: Arc<AtomicBool>, progress: Arc<Mutex<Progress>>) {
    let set = |t: &str| {
        progress.lock().unwrap().text = crate::i18n::t(t);
        crate::events::repaint();
    };
    let st = crate::state::get();
    // several codes at once (a text file of playlists): each its own playlist
    let codes = extract_codes(&text);
    if codes.len() > 1 {
        let (mut lists, mut songs, mut broken) = (0, 0, 0);
        for (n, code) in codes.iter().enumerate() {
            set(&format!("Читаю коды: {} из {}", n + 1, codes.len()));
            match decode_code(code) {
                Ok(s) if !s.tracks.is_empty() => {
                    if to_likes.unwrap_or(s.likes) {
                        songs += st.update_library(|l| l.add_likes(&s.tracks));
                    } else {
                        st.update_library(|l| {
                            let id = l.create_playlist(&s.title, s.description.as_deref(), None);
                            if let Some(p) = l.playlist_mut(&id) {
                                p.icon = s.icon.clone();
                            }
                            l.add_to_playlist(&id, &s.tracks);
                        });
                        songs += s.tracks.len();
                        lists += 1;
                    }
                }
                Ok(_) => {}
                Err(_) => broken += 1,
            }
        }
        let mut p = progress.lock().unwrap();
        p.ok = lists + songs > 0;
        p.text = format!("{} · {} {lists}, {} {songs}{}", crate::i18n::t("Готово"), crate::i18n::t("плейлистов:"), crate::i18n::t("песен:"), if broken > 0 { format!(", {} {broken}", crate::i18n::t("не прочиталось кодов:")) } else { String::new() });
        p.done = true;
        p.opened = Some("library".into());
        crate::events::repaint();
        return;
    }
    set(match link_kind(&text) {
        Some(Kind::Code) => "Читаю код…",
        None => "Читаю список…",
        _ => "Читаю ссылку…",
    });
    let result = (|| -> Result<(String, Option<String>), String> {
        let f = fetch_with(&text, Some(&cancel), &set)?;
        let mut tracks = f.tracks.clone();
        if !f.named.is_empty() {
            let total = f.named.len();
            set(&format!("Ищу песни: 0 из {total}"));
            tracks = match_all(&f.named, &cancel, &|d, t, found| set(&format!("Ищу песни: {d} из {t} (найдено {found})")));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Загрузка отменена".into());
        }
        if tracks.is_empty() {
            return Err("В ссылке не нашлось треков, которые можно слушать".into());
        }
        let missed = f.named.len().saturating_sub(tracks.len());
        let missed_note = if !f.named.is_empty() && missed > 0 { format!(", {missed} не нашлось") } else { String::new() };
        if to_likes.unwrap_or(f.likes) {
            let added = st.update_library(|l| l.add_likes(&tracks));
            let already = tracks.len() - added;
            Ok((format!("В «Понравившиеся» добавлено {}{}{missed_note}", crate::i18n::tracks(added), if already > 0 { format!(" ({already} уже были)") } else { String::new() }), Some("likes".into())))
        } else {
            let id = st.update_library(|l| {
                let id = l.create_playlist(&f.title, f.description.as_deref(), None);
                let icon = f.icon.clone().or_else(|| f.artwork.clone().filter(|a| a.starts_with("https:")));
                if let Some(p) = l.playlist_mut(&id) {
                    p.icon = icon;
                }
                l.add_to_playlist(&id, &tracks);
                id
            });
            let mut s = format!("Плейлист «{}»: {}{missed_note}", f.title, crate::i18n::tracks(tracks.len()));
            if f.truncated {
                s.push_str(". Без входа Spotify показывает по ссылке только первые 100 треков");
            }
            Ok((s, Some(id)))
        }
    })();
    let mut p = progress.lock().unwrap();
    match result {
        Ok((summary, opened)) => {
            p.ok = true;
            p.text = format!("{} · {summary}", crate::i18n::t("Готово"));
            p.opened = opened;
        }
        Err(e) => {
            p.ok = false;
            p.text = format!("{}: {e}", crate::i18n::t("Не загрузилось"));
        }
    }
    p.done = true;
    crate::events::repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_go_there_and_back_and_read_the_page_s() {
        let t = Track { id: "sc:1".into(), source: Source::Soundcloud, title: "Песня".into(), artists: vec![Artist { id: "scu:2".into(), name: "Кто-то".into() }], duration_ms: 1000.0, ..Default::default() };
        let code = encode_code(&Shared { title: "Утро".into(), description: None, icon: None, tracks: vec![t.clone()], likes: true });
        assert!(code.starts_with("EVP2."));
        let back = decode_code(&format!("\u{FEFF}  {code}\n")).unwrap();
        assert_eq!(back.title, "Утро");
        assert!(back.likes);
        assert_eq!(back.tracks[0], t);
        // EVP1: plain base64url JSON, as the page makes it without CompressionStream
        let plain = format!("EVP1.{}", b64(br#"{"v":2,"n":"X","t":[["y","yt:abcdefghijk","T",2000,"",[["yta:1","A"]]]]}"#));
        let p = decode_code(&plain).unwrap();
        assert_eq!(p.tracks[0].source, Source::Youtube);
        assert_eq!(p.tracks[0].artists[0].name, "A");
    }

    #[test]
    fn finds_several_codes_and_a_wrapped_one() {
        assert_eq!(extract_codes("one: EVP2.abc\ntwo: EVP1.def").len(), 2);
        assert_eq!(extract_codes("EVP2.abc\ndef")[0], "EVP2.abcdef");
    }

    #[test]
    fn reads_pasted_lists() {
        let l = parse_track_list("1. Daft Punk — One More Time 5:20\nRadiohead - Creep\n3:58");
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].artists, vec!["Daft Punk"]);
        assert_eq!(l[0].title, "One More Time");
        assert_eq!(l[0].duration_ms, Some(320_000.0));
        assert_eq!(l[1].duration_ms, Some(238_000.0));
        let vk = parse_track_list("Кукла колдуна\nКороль и Шут\n3:24\nЛесник\nКороль и Шут\n3:10");
        assert_eq!(vk.len(), 2);
        assert_eq!(vk[1].artists, vec!["Король и Шут"]);
    }

    #[test]
    fn knows_the_links() {
        assert_eq!(link_kind("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M"), Some(Kind::Spotify));
        assert_eq!(link_kind("https://music.yandex.ru/album/123"), Some(Kind::Yandex));
        assert_eq!(link_kind("https://www.deezer.com/ru/playlist/1"), Some(Kind::Deezer));
        assert_eq!(link_kind("https://soundcloud.com/a/sets/b"), Some(Kind::Soundcloud));
        assert_eq!(link_kind("EVP2.xyz"), Some(Kind::Code));
        assert_eq!(link_kind("hello"), None);
    }
}
