//! YouTube Music: the public search and pages the music.youtube.com site uses (as
//! src/services/youtube/music.ts), and the song's sound straight from YouTube (direct.ts):
//! YouTube hands plain audio addresses to its own apps, so the player asks as two of them.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::http::{find_all, first, post_json};
use crate::model::{Artist, CollectionKind, CollectionSummary, Source, Track};

const ENDPOINT: &str = "https://music.youtube.com/youtubei/v1";
// "Songs" filter; not every country has it, then the "Videos" filter is used
const SONGS: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D";
const VIDEOS: &str = "EgWKAQIQAWoKEAQQBRAVEAoQEQ%3D%3D";
const ALBUMS: &str = "EgWKAQIYAWoKEAkQAxAEEAkQBQ%3D%3D";
const PLAYLISTS: &str = "EgeKAQQoAEABagoQBBAFEBUQChAR";

/// YouTube words its labels in the language the page asks for
fn client() -> Value {
    let hl = if crate::i18n::is_english() { "en" } else { "ru" };
    json!({ "clientName": "WEB_REMIX", "clientVersion": "1.20241023.01.00", "hl": hl, "gl": "US" })
}

fn call(path: &str, mut body: Value) -> Result<Value, String> {
    body["context"] = json!({ "client": client() });
    post_json(&format!("{ENDPOINT}/{path}?prettyPrint=false"), &[], &body).map_err(|e| e.replace("music.youtube.com ответил", "YouTube Music ответил"))
}

fn runs(v: Option<&Value>) -> Vec<&Value> {
    v.and_then(|t| t.get("runs")).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default()
}

fn run_text(r: &Value) -> &str {
    r.get("text").and_then(Value::as_str).unwrap_or("")
}

fn browse_id(r: &Value) -> Option<&str> {
    r.pointer("/navigationEndpoint/browseEndpoint/browseId").and_then(Value::as_str)
}

fn page_type(r: &Value) -> Option<&str> {
    r.pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType").and_then(Value::as_str)
}

pub fn parse_duration(text: &str) -> f64 {
    let t = text.trim();
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 || !parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) || parts[1..].iter().any(|p| p.len() > 2) {
        return 0.0;
    }
    parts.iter().fold(0.0, |acc, p| acc * 60.0 + p.parse::<f64>().unwrap_or(0.0)) * 1000.0
}

/// "58 млн просмотров", "1.2M views", "385 тыс." → a number
pub fn parse_count(text: &str) -> Option<f64> {
    let t = text.replace('\u{a0}', " ");
    let re = regex::Regex::new(r"(?i)([\d.,\s]+)\s*(млрд|млн|тыс|B|M|K|bn|mln)?").ok()?;
    let m = re.captures(&t)?;
    let digits = m.get(1)?.as_str();
    if !digits.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let value: f64 = digits.chars().filter(|c| !c.is_whitespace()).collect::<String>().replacen(',', ".", 1).parse().ok()?;
    let unit = m.get(2).map(|u| u.as_str().to_lowercase()).unwrap_or_default();
    let mult = match unit.as_str() {
        "млрд" | "b" | "bn" => 1e9,
        "млн" | "m" | "mln" => 1e6,
        "тыс" | "k" => 1e3,
        _ => 1.0,
    };
    Some((value * mult).round())
}

fn thumbs_url(node: Option<&Value>) -> Option<String> {
    let list = first(node?, "thumbnails")?.as_array()?;
    list.last()?.get("url")?.as_str().map(str::to_string)
}

fn best_thumb(item: &Value) -> Option<String> {
    let url = thumbs_url(item.get("thumbnail"))?;
    // album covers come as lh3 urls with a size suffix: ask for a bigger one
    Some(regex::Regex::new(r"=w\d+-h\d+").ok()?.replace(&url, "=w544-h544").to_string())
}

fn is_views(t: &str) -> bool {
    let l = t.to_lowercase();
    t.chars().any(|c| c.is_ascii_digit()) && ["просм", "views", "прослуш", "plays", "воспр"].iter().any(|w| l.contains(w))
}

/// the "E" mark YouTube Music puts on the uncensored version of a song: in "badges" on list
/// rows and the queue, in "subtitleBadges" on the cards of artist pages
pub fn is_explicit(item: &Value) -> bool {
    ["badges", "subtitleBadges"].iter().any(|k| item.get(*k).and_then(Value::as_array).is_some_and(|b| b.iter().any(|x| x.pointer("/musicInlineBadgeRenderer/icon/iconType").and_then(Value::as_str) == Some("MUSIC_EXPLICIT_BADGE"))))
}

pub fn parse_list_item(item: &Value) -> Option<Track> {
    let columns = item.get("flexColumns").and_then(Value::as_array);
    let runs_of = |i: usize| runs(columns.and_then(|c| c.get(i)).and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text")));
    let title_runs = runs_of(0);
    let video_id = title_runs
        .iter()
        .find_map(|r| r.pointer("/navigationEndpoint/watchEndpoint/videoId").and_then(Value::as_str))
        .or_else(|| item.pointer("/playlistItemData/videoId").and_then(Value::as_str))?
        .to_string();
    if title_runs.is_empty() {
        return None;
    }
    let meta: Vec<&Value> = runs_of(1).into_iter().chain(runs_of(2)).collect();
    let mut artists: Vec<Artist> = meta
        .iter()
        .filter(|r| matches!(page_type(r), Some("MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL")))
        .filter_map(|r| Some(Artist { id: format!("yta:{}", browse_id(r)?), name: run_text(r).to_string() }))
        .collect();
    let texts: Vec<&str> = meta.iter().map(|r| run_text(r).trim()).filter(|t| !t.is_empty() && *t != "•").collect();
    if artists.is_empty() {
        if let Some(t) = texts.first().filter(|t| !t.starts_with(|c: char| c.is_ascii_digit())) {
            artists.push(Artist { id: format!("ytn:{t}"), name: t.to_string() });
        }
    }
    let mut duration = 0.0;
    let mut plays = None;
    for t in &texts {
        if duration == 0.0 && parse_duration(t) > 0.0 {
            duration = parse_duration(t);
        } else if plays.is_none() && is_views(t) {
            plays = parse_count(t);
        }
    }
    if duration == 0.0 {
        if let Some(t) = item.pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text/runs/0/text").and_then(Value::as_str) {
            duration = parse_duration(t);
        }
    }
    Some(Track {
        id: format!("yt:{video_id}"),
        source: Source::Youtube,
        title: title_runs.iter().map(|r| run_text(r)).collect(),
        artists,
        duration_ms: duration,
        artwork: best_thumb(item),
        plays,
        permalink: Some(format!("https://music.youtube.com/watch?v={video_id}")),
        preview: false,
        explicit: is_explicit(item),
    })
}

fn twin_key(t: &Track) -> String {
    use crate::player::matching::simplify;
    format!("{}|{}", simplify(&t.title), simplify(t.artists.first().map(|a| a.name.as_str()).unwrap_or("")))
}

/// the uncensored version before its censored twin where both are listed; the rest keeps its order
pub fn explicit_first(tracks: Vec<Track>) -> Vec<Track> {
    let mut out: Vec<Track> = Vec::with_capacity(tracks.len());
    for t in tracks {
        let at = if t.explicit { out.iter().position(|o| !o.explicit && twin_key(o) == twin_key(&t)) } else { None };
        match at {
            Some(i) => out.insert(i, t),
            None => out.push(t),
        }
    }
    out
}

pub fn parse_search(response: &Value) -> Vec<Track> {
    let mut items = Vec::new();
    find_all(response, "musicResponsiveListItemRenderer", &mut items);
    let mut out: Vec<Track> = Vec::new();
    for item in items {
        if let Some(t) = parse_list_item(item) {
            if !out.iter().any(|x| x.id == t.id) {
                out.push(t);
            }
        }
    }
    out
}

pub fn search(query: &str) -> Result<Vec<Track>, String> {
    let songs = parse_search(&call("search", json!({ "query": query, "params": SONGS }))?);
    if !songs.is_empty() {
        return Ok(explicit_first(songs));
    }
    Ok(explicit_first(parse_search(&call("search", json!({ "query": query, "params": VIDEOS }))?)))
}

pub fn parse_collection_item(item: &Value) -> Option<CollectionSummary> {
    let nav = item.get("navigationEndpoint")?;
    let browse = nav.get("browseEndpoint")?;
    let id = browse.get("browseId")?.as_str()?;
    let page = browse.pointer("/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType").and_then(Value::as_str)?;
    if page != "MUSIC_PAGE_TYPE_ALBUM" && page != "MUSIC_PAGE_TYPE_PLAYLIST" {
        return None;
    }
    let columns = item.get("flexColumns").and_then(Value::as_array);
    let runs_of = |i: usize| runs(columns.and_then(|c| c.get(i)).and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text")));
    let title: String = runs_of(0).iter().map(|r| run_text(r)).collect();
    if title.is_empty() {
        return None;
    }
    let meta = runs_of(1);
    let texts: Vec<&str> = meta.iter().map(|r| run_text(r).trim()).filter(|t| !t.is_empty() && *t != "•").collect();
    let artist = meta.iter().find_map(|r| Some(Artist { id: format!("yta:{}", browse_id(r)?), name: run_text(r).to_string() }));
    let year = texts.iter().filter_map(|t| t.parse::<i32>().ok()).find(|y| *y > 1900 && *y < 2100);
    Some(CollectionSummary {
        id: format!("ytb:{id}"),
        kind: if page == "MUSIC_PAGE_TYPE_ALBUM" { CollectionKind::Album } else { CollectionKind::Playlist },
        title,
        subtitle: texts.join(" · "),
        artwork: best_thumb(item),
        year,
        track_count: None,
        artist,
    })
}

pub fn parse_collections(response: &Value) -> Vec<CollectionSummary> {
    let mut items = Vec::new();
    find_all(response, "musicResponsiveListItemRenderer", &mut items);
    let mut out: Vec<CollectionSummary> = Vec::new();
    for item in items {
        if let Some(c) = parse_collection_item(item) {
            if !out.iter().any(|x| x.id == c.id) {
                out.push(c);
            }
        }
    }
    out
}

pub fn search_collections(query: &str, kind: CollectionKind) -> Result<Vec<CollectionSummary>, String> {
    Ok(parse_collections(&call("search", json!({ "query": query, "params": if kind == CollectionKind::Album { ALBUMS } else { PLAYLISTS } }))?))
}

pub struct Collection {
    pub title: String,
    pub subtitle: String,
    pub artwork: Option<String>,
    pub artist: Option<Artist>,
    pub tracks: Vec<Track>,
}

/// an album (MPREb…) or playlist (VL…) page with all its songs
pub fn collection(id: &str, limit: usize) -> Result<Collection, String> {
    let id = id.trim_start_matches("ytb:");
    let id = if id.starts_with("MPRE") || id.starts_with("VL") || id.starts_with("OLAK") { id.to_string() } else { format!("VL{id}") };
    let mut response = call("browse", json!({ "browseId": id }))?;
    let header = first(&response, "musicResponsiveHeaderRenderer").or_else(|| first(&response, "musicDetailHeaderRenderer")).cloned().unwrap_or(Value::Null);
    let title: String = runs(header.get("title")).iter().map(|r| run_text(r)).collect();
    let title = if title.is_empty() { "YouTube".to_string() } else { title };
    let artist = runs(header.get("straplineTextOne")).iter().find_map(|r| Some(Artist { id: format!("yta:{}", browse_id(r)?), name: run_text(r).to_string() }));
    let subtitle: String = runs(header.get("subtitle")).iter().map(|r| run_text(r)).collect();
    let artwork = thumbs_url(Some(&header)).map(|u| regex::Regex::new(r"=w\d+-h\d+").map(|r| r.replace(&u, "=w544-h544").to_string()).unwrap_or(u));
    let fill = |list: Vec<Track>| -> Vec<Track> {
        list.into_iter()
            .map(|mut t| {
                if t.artwork.is_none() {
                    t.artwork = artwork.clone();
                }
                if t.artists.is_empty() {
                    t.artists = artist.clone().into_iter().collect();
                }
                t
            })
            .collect()
    };
    let mut tracks = fill(parse_search(&response));
    for _ in 0..20 {
        if tracks.len() >= limit {
            break;
        }
        let token = first(&response, "continuationCommand").and_then(|c| c.get("token")).and_then(Value::as_str).or_else(|| first(&response, "nextContinuationData").and_then(|c| c.get("continuation")).and_then(Value::as_str)).map(str::to_string);
        let Some(token) = token else { break };
        response = call("browse", json!({ "continuation": token }))?;
        let more: Vec<Track> = fill(parse_search(&response)).into_iter().filter(|t| !tracks.iter().any(|x| x.id == t.id)).collect();
        if more.is_empty() {
            break;
        }
        tracks.extend(more);
    }
    tracks.truncate(limit);
    Ok(Collection { title, subtitle, artwork, artist, tracks })
}

/// a "card" (carousels on artist pages): a video or song with its views
pub fn parse_two_row_item(item: &Value) -> Option<Track> {
    let video_id = item.pointer("/navigationEndpoint/watchEndpoint/videoId")?.as_str()?;
    let title: String = runs(item.get("title")).iter().map(|r| run_text(r)).collect();
    if title.is_empty() {
        return None;
    }
    let sub = runs(item.get("subtitle"));
    let artists = sub.iter().filter_map(|r| Some(Artist { id: format!("yta:{}", browse_id(r)?), name: run_text(r).to_string() })).collect();
    let views = sub.iter().map(|r| run_text(r)).find(|t| is_views(t));
    Some(Track {
        id: format!("yt:{video_id}"),
        source: Source::Youtube,
        title,
        artists,
        duration_ms: 0.0,
        artwork: thumbs_url(item.get("thumbnailRenderer")),
        plays: views.and_then(parse_count),
        permalink: Some(format!("https://music.youtube.com/watch?v={video_id}")),
        preview: false,
        explicit: is_explicit(item),
    })
}

pub struct YouTubeArtist {
    pub name: String,
    pub avatar: Option<String>,
    pub monthly_listeners: Option<f64>,
    pub subscribers: Option<f64>,
    pub tracks: Vec<Track>,
}

pub fn parse_artist(response: &Value) -> YouTubeArtist {
    let header = first(response, "musicImmersiveHeaderRenderer").or_else(|| first(response, "musicVisualHeaderRenderer")).cloned().unwrap_or(Value::Null);
    let name = runs(header.get("title")).first().map(|r| run_text(r).to_string()).unwrap_or_else(|| "YouTube".into());
    let monthly = runs(header.get("monthlyListenerCount")).first().map(|r| run_text(r).to_string());
    let subs = first(&header, "subscriberCountText").and_then(|s| s.pointer("/runs/0/text")).and_then(Value::as_str).map(str::to_string);
    let mut tracks = parse_search(response);
    let mut cards = Vec::new();
    find_all(response, "musicTwoRowItemRenderer", &mut cards);
    for c in cards {
        if let Some(t) = parse_two_row_item(c) {
            if !tracks.iter().any(|x| x.id == t.id) {
                tracks.push(t);
            }
        }
    }
    YouTubeArtist {
        name,
        avatar: thumbs_url(header.get("thumbnail")),
        monthly_listeners: monthly.as_deref().and_then(parse_count),
        subscribers: subs.as_deref().and_then(parse_count),
        tracks,
    }
}

pub fn artist(channel_id: &str) -> Result<YouTubeArtist, String> {
    Ok(parse_artist(&call("browse", json!({ "browseId": channel_id.trim_start_matches("yta:") }))?))
}

pub fn parse_panel_item(item: &Value) -> Option<Track> {
    let video_id = item.get("videoId")?.as_str()?;
    let title: String = runs(item.get("title")).iter().map(|r| run_text(r)).collect();
    if title.is_empty() {
        return None;
    }
    let byline = runs(item.get("longBylineText"));
    let mut artists: Vec<Artist> = byline
        .iter()
        .filter(|r| page_type(r).is_some_and(|p| p.contains("ARTIST") || p.contains("USER_CHANNEL")))
        .filter_map(|r| Some(Artist { id: format!("yta:{}", browse_id(r)?), name: run_text(r).to_string() }))
        .collect();
    if artists.is_empty() {
        if let Some(t) = byline.first().map(|r| run_text(r)).filter(|t| !t.is_empty()) {
            artists.push(Artist { id: format!("ytn:{t}"), name: t.to_string() });
        }
    }
    let views = byline.iter().map(|r| run_text(r)).find(|t| is_views(t));
    let length = item.pointer("/lengthText/runs/0/text").and_then(Value::as_str).unwrap_or("");
    Some(Track {
        id: format!("yt:{video_id}"),
        source: Source::Youtube,
        title,
        artists,
        duration_ms: parse_duration(length),
        artwork: best_thumb(item),
        plays: views.and_then(parse_count),
        permalink: Some(format!("https://music.youtube.com/watch?v={video_id}")),
        preview: false,
        explicit: is_explicit(item),
    })
}

pub fn parse_up_next(response: &Value) -> Vec<Track> {
    let mut items = Vec::new();
    find_all(response, "playlistPanelVideoRenderer", &mut items);
    let mut out: Vec<Track> = Vec::new();
    for i in items {
        if let Some(t) = parse_panel_item(i) {
            if !out.iter().any(|x| x.id == t.id) {
                out.push(t);
            }
        }
    }
    out
}

/// YouTube Music's own radio for a song: what plays after it on music.youtube.com
pub fn radio(video_id: &str) -> Result<Vec<Track>, String> {
    let id = video_id.trim_start_matches("yt:");
    let r = call("next", json!({ "videoId": id, "playlistId": format!("RDAMVM{id}"), "isAudioOnly": true }))?;
    Ok(parse_up_next(&r).into_iter().filter(|t| t.id != format!("yt:{id}")).collect())
}

pub fn video_id_of(track_id: &str) -> Option<&str> {
    let id = track_id.trim_start_matches("yt:");
    (id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')).then_some(id)
}

// ---- the sound, straight from YouTube (direct.ts) ---------------------------------------------

struct Profile {
    client_name: &'static str,
    client_version: &'static str,
    client_id: &'static str,
    user_agent: &'static str,
    extra: fn() -> Value,
}

const VISIONOS: Profile = Profile {
    client_name: "VISIONOS",
    client_version: "1.02",
    client_id: "101",
    user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
    extra: || json!({ "osName": "visionOS", "osVersion": "26.5.23O471", "deviceMake": "Apple", "deviceModel": "RealityDevice17,1" }),
};

const ANDROID_VR: Profile = Profile {
    client_name: "ANDROID_VR",
    client_version: "1.65.10",
    client_id: "28",
    user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
    extra: || json!({ "osName": "Android", "osVersion": "12L", "deviceMake": "Oculus", "deviceModel": "Quest 3", "androidSdkVersion": 32 }),
};

const PROFILES: [&Profile; 2] = [&VISIONOS, &ANDROID_VR];

/// Formats best first, as the Tauri build takes them (direct.ts BEST_ITAGS): Opus ~160 kbit/s
/// (libopus; WebM seeks by its cues, checked by net_youtube_seeks_in_both_formats) sounds
/// better than AAC 128 kbit/s, which comes next.
const BEST_ITAGS: [u32; 5] = [251, 140, 250, 249, 139];

#[derive(Clone, Debug)]
pub struct DirectStream {
    pub url: String,
    pub codec: &'static str,
    pub bitrate: f64,
    pub itag: u32,
    pub user_agent: &'static str,
    /// the address stops working then
    pub expires: Option<std::time::SystemTime>,
}

impl DirectStream {
    pub fn quality(&self) -> String {
        format!("{} {} кбит/с", self.codec, (self.bitrate / 1000.0).round())
    }
}

static VISITOR: Mutex<Option<(String, Instant)>> = Mutex::new(None);

pub fn parse_visitor_data(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"Cg[s-v][A-Za-z0-9_-]{20,}(?:%3D|=){0,2}").ok()?;
    let body = text.strip_prefix(")]}'").map(|b| b.split_once('\n').map(|x| x.1).unwrap_or(b)).unwrap_or(text);
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(s) = v.pointer("/0/2/0/0/13").and_then(Value::as_str) {
            if re.is_match(s) {
                return Some(s.to_string());
            }
        }
    }
    re.find(text).map(|m| m.as_str().to_string())
}

fn visitor_data() -> String {
    {
        let v = VISITOR.lock().unwrap();
        if let Some((value, until)) = &*v {
            if Instant::now() < *until {
                return value.clone();
            }
        }
    }
    let fresh = super::http::get_text("https://www.youtube.com/sw.js_data").ok().and_then(|t| parse_visitor_data(&t));
    let mut v = VISITOR.lock().unwrap();
    if let Some(value) = fresh {
        *v = Some((value.clone(), Instant::now() + Duration::from_secs(5 * 3600 + 1800)));
        return value;
    }
    v.as_ref().map(|x| x.0.clone()).unwrap_or_default()
}

fn player(p: &Profile, video_id: &str, visitor: &str) -> Result<Value, String> {
    let mut client = json!({ "clientName": p.client_name, "clientVersion": p.client_version, "hl": "en", "gl": "US", "userAgent": p.user_agent });
    if !visitor.is_empty() {
        client["visitorData"] = json!(visitor);
    }
    if let (Some(c), Value::Object(extra)) = (client.as_object_mut(), (p.extra)()) {
        c.extend(extra);
    }
    let body = json!({ "videoId": video_id, "contentCheckOk": true, "racyCheckOk": true, "context": { "client": client, "user": { "lockedSafetyMode": false } } });
    let mut headers: Vec<(&str, &str)> = vec![("accept", "application/json"), ("x-youtube-client-name", p.client_id), ("x-youtube-client-version", p.client_version), ("x-origin", "https://www.youtube.com"), ("user-agent", p.user_agent)];
    if !visitor.is_empty() {
        headers.push(("x-goog-visitor-id", visitor));
    }
    post_json("https://www.youtube.com/youtubei/v1/player?prettyPrint=false", &headers, &body).map_err(|e| e.replace("www.youtube.com ответил", "YouTube ответил"))
}

pub fn playability_problem(r: &Value) -> Option<String> {
    let st = r.get("playabilityStatus")?;
    let status = st.get("status").and_then(Value::as_str).unwrap_or("");
    if status == "OK" {
        return None;
    }
    Some(st.get("reason").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| format!("YouTube: {status}")))
}

pub fn best_format(r: &Value) -> Option<(u32, String, String, f64)> {
    best_format_of(r, &BEST_ITAGS)
}

pub fn best_format_of(r: &Value, order: &[u32]) -> Option<(u32, String, String, f64)> {
    let list = r.pointer("/streamingData/adaptiveFormats")?.as_array()?;
    let formats: Vec<(u32, String, String, f64)> = list
        .iter()
        .filter_map(|f| {
            let itag = f.get("itag")?.as_u64()? as u32;
            let url = f.get("url")?.as_str().filter(|u| !u.is_empty())?.to_string();
            let mime = f.get("mimeType")?.as_str()?.to_string();
            if !mime.starts_with("audio/") {
                return None;
            }
            let bitrate = f.get("bitrate").and_then(Value::as_f64).or_else(|| f.get("averageBitrate").and_then(Value::as_f64)).unwrap_or(0.0);
            Some((itag, url, mime, bitrate))
        })
        .collect();
    for &itag in order {
        if let Some(f) = formats.iter().find(|f| f.0 == itag) {
            return Some(f.clone());
        }
    }
    formats.into_iter().max_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal))
}

static RESOLVED: Mutex<Option<HashMap<String, DirectStream>>> = Mutex::new(None);
static LAST_GOOD: Mutex<usize> = Mutex::new(0);

/// the audio of a YouTube video, straight from YouTube
pub fn resolve_audio(video_id: &str) -> Result<DirectStream, String> {
    resolve_audio_in(video_id, &BEST_ITAGS)
}

/// the same, with the formats wanted in this order
pub fn resolve_audio_in(video_id: &str, order: &[u32]) -> Result<DirectStream, String> {
    if let Some(known) = RESOLVED.lock().unwrap().as_ref().and_then(|m| m.get(video_id)).cloned().filter(|k| order.first() == Some(&k.itag)) {
        let fresh = known.expires.map(|e| e.duration_since(std::time::SystemTime::now()).map(|d| d > Duration::from_secs(600)).unwrap_or(false)).unwrap_or(true);
        if fresh {
            return Ok(known);
        }
    }
    let first_try = *LAST_GOOD.lock().unwrap();
    let profiles = [first_try, 1 - first_try];
    let mut failures = Vec::new();
    for &i in &profiles {
        let p = PROFILES[i];
        let attempt = (|| {
            let mut r = player(p, video_id, &visitor_data())?;
            if r.pointer("/playabilityStatus/status").and_then(Value::as_str) == Some("LOGIN_REQUIRED") {
                // a fresh visitor id often clears the "confirm you're not a bot" answer
                *VISITOR.lock().unwrap() = None;
                r = player(p, video_id, &visitor_data())?;
            }
            if let Some(problem) = playability_problem(&r) {
                return Err(problem);
            }
            let (itag, url, mime, bitrate) = best_format_of(&r, order).ok_or("нет прямых аудиоформатов")?;
            let expires = url::Url::parse(&url).ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "expire").and_then(|(_, v)| v.parse::<u64>().ok())).map(|s| std::time::UNIX_EPOCH + Duration::from_secs(s));
            let codec = if mime.contains("opus") || mime.contains("webm") { "opus" } else { "aac" };
            Ok(DirectStream { url, codec, bitrate: if bitrate > 0.0 { bitrate } else { 160_000.0 }, itag, user_agent: p.user_agent, expires })
        })();
        match attempt {
            Ok(s) => {
                *LAST_GOOD.lock().unwrap() = i;
                RESOLVED.lock().unwrap().get_or_insert_with(HashMap::new).insert(video_id.to_string(), s.clone());
                return Ok(s);
            }
            Err(e) => failures.push(format!("{}: {e}", p.client_name)),
        }
    }
    Err(format!("YouTube не отдал звук ({})", failures.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/services/youtube").join(name);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn reads_durations_and_counts() {
        assert_eq!(parse_duration("4:08"), 248000.0);
        assert_eq!(parse_duration("1:02:03"), 3723000.0);
        assert_eq!(parse_duration("abc"), 0.0);
        assert_eq!(parse_count("58 млн просмотров"), Some(58e6));
        assert_eq!(parse_count("385,4 тыс."), Some(385400.0));
        assert_eq!(parse_count("1.2M views"), Some(1.2e6));
    }

    #[test]
    fn turns_search_results_into_tracks() {
        let tracks = parse_search(&fixture("fixture.test.json"));
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].id, "yt:CCHdMIEGaaM");
        assert_eq!(tracks[0].duration_ms, 248000.0);
        assert_eq!(tracks[0].plays, Some(58e6));
        assert_eq!(tracks[0].artists[0].name, "convar HUN");
        assert!(tracks[0].artwork.as_deref().unwrap().starts_with("https:"));
    }

    #[test]
    fn reads_an_artist() {
        let a = parse_artist(&fixture("artist.test.json"));
        assert_eq!(a.name, "Daft Punk");
        assert_eq!(a.monthly_listeners, Some(67.8e6));
        assert_eq!(a.subscribers, Some(7.2e6));
        assert_eq!(a.tracks.len(), 2);
        assert_eq!(a.tracks[0].id, "yt:FGBhQbmPwH8");
        assert_eq!(a.tracks[0].title, "One More Time");
        assert_eq!(a.tracks[0].plays, Some(622e6));
    }

    #[test]
    fn reads_playlist_rows() {
        let list = parse_collections(&fixture("playlists.test.json"));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "ytb:VLPLRVVwMYsRVWw");
        assert_eq!(list[0].kind, CollectionKind::Playlist);
        assert_eq!(list[0].title, "DAFT PUNK DISCOVERY");
        assert!(list[0].subtitle.contains("Dowita 2610"));
    }

    #[test]
    fn reads_the_up_next_list() {
        let list = parse_up_next(&fixture("next.test.json"));
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].id, "yt:pjSMPn8dEN0");
        assert_eq!(list[1].title, "Snoh Aalegra - I Love You");
        assert_eq!(list[1].duration_ms, 207000.0);
        assert_eq!(list[1].plays, Some(327000.0));
        assert_eq!(list[1].artists[0].name, "Snoh Aalegra");
    }

    #[test]
    fn uncensored_first() {
        let song = |id: &str, title: &str, explicit: bool| Track { id: id.into(), source: Source::Youtube, title: title.into(), artists: vec![Artist { id: "a".into(), name: "Kendrick Lamar".into() }], duration_ms: 1000.0, explicit, ..Default::default() };
        assert!(is_explicit(&json!({ "badges": [{ "musicInlineBadgeRenderer": { "icon": { "iconType": "MUSIC_EXPLICIT_BADGE" } } }] })));
        assert!(!is_explicit(&json!({})));
        let list = vec![song("yt:clean00000", "Money Trees", false), song("yt:other00000", "Alright", true), song("yt:expl000000", "Money Trees", true)];
        let ids: Vec<String> = explicit_first(list).into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["yt:expl000000", "yt:clean00000", "yt:other00000"]);
    }

    #[test]
    fn picks_opus_first_and_reads_the_visitor() {
        let r = json!({ "streamingData": { "adaptiveFormats": [
            { "itag": 251, "url": "https://x.googlevideo.com/a", "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 150000 },
            { "itag": 140, "url": "https://x.googlevideo.com/b", "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 130000 },
            { "itag": 137, "url": "https://x.googlevideo.com/v", "mimeType": "video/mp4", "bitrate": 900000 }
        ] } });
        assert_eq!(best_format(&r).unwrap().0, 251);
        assert_eq!(parse_visitor_data("xx CgtabcdefghijklmnopqrstuvW%3D%3D yy").as_deref(), Some("CgtabcdefghijklmnopqrstuvW%3D%3D"));
        assert_eq!(video_id_of("yt:dQw4w9WgXcQ"), Some("dQw4w9WgXcQ"));
        assert_eq!(video_id_of("yt:short"), None);
    }
}
