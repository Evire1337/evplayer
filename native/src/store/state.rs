//! What the player keeps: the same fields, names and JSON as the page's stores (settings,
//! library, history, session, effects), so the two builds read each other's data. A field the
//! native build does not use (the Tauri build's own, or one added later) is kept as it is
//! and written back untouched.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::model::Track;

fn yes() -> bool {
    true
}
fn one() -> f64 {
    1.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchSource {
    #[default]
    All,
    Soundcloud,
    Youtube,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayCorner {
    TopLeft,
    #[default]
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StatusShows {
    App,
    Artist,
    #[default]
    Song,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Auto,
    Ru,
    En,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LibraryView {
    #[default]
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LyricsSize {
    S,
    #[default]
    M,
    L,
}

/// Настройки: every field with the page's name and default
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub autoplay: bool,
    /// seconds the next song fades in over the end of the current one; 0 = straight cut, no gap
    pub crossfade: f64,
    pub default_search_source: SearchSource,
    pub resume_on_start: bool,
    pub pause_on_device_change: bool,
    pub double_click_to_play: bool,
    pub animated_backdrop: bool,
    pub cover_glow: bool,
    pub lyrics_blur: bool,
    pub lyrics_size: LyricsSize,
    pub memory_saver: bool,
    pub no_cache: bool,
    pub no_service_cache: bool,
    pub no_loudness_memory: bool,
    pub no_stats: bool,
    pub no_history: bool,
    pub show_queue: bool,
    pub library_view: LibraryView,
    pub genius_fallback: bool,
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    pub global_shortcuts: bool,
    /// changed keys: shortcut id → accelerator ("Ctrl+Alt+Space") or null for none
    pub shortcuts: Map<String, Value>,
    pub overlay_enabled: bool,
    pub overlay_on_top: bool,
    pub overlay_on_minimize: bool,
    pub overlay_corner: OverlayCorner,
    pub overlay_show_lyrics: bool,
    pub soundcloud_client_id: String,
    pub language: Language,
    pub region_guard: bool,
    pub region_countries: Vec<String>,
    pub soundcloud_client_secret: String,
    pub soundcloud_full_sync: bool,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
    pub discord_enabled: bool,
    pub discord_app_id: String,
    pub discord_show_artwork: bool,
    pub discord_show_paused: bool,
    pub discord_show_timeline: bool,
    pub discord_show_button: bool,
    pub discord_join: bool,
    pub dev_tools: bool,
    pub discord_status_shows: StatusShows,
    /// the rest (accounts, the Tauri build's own settings): kept as they are
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            autoplay: true,
            crossfade: 0.0,
            default_search_source: SearchSource::All,
            resume_on_start: true,
            pause_on_device_change: true,
            double_click_to_play: false,
            animated_backdrop: true,
            cover_glow: true,
            lyrics_blur: true,
            lyrics_size: LyricsSize::M,
            memory_saver: false,
            no_cache: false,
            no_service_cache: false,
            no_loudness_memory: false,
            no_stats: false,
            no_history: false,
            show_queue: true,
            library_view: LibraryView::Grid,
            genius_fallback: true,
            close_to_tray: true,
            minimize_to_tray: false,
            global_shortcuts: true,
            shortcuts: Map::new(),
            overlay_enabled: true,
            overlay_on_top: true,
            overlay_on_minimize: true,
            overlay_corner: OverlayCorner::TopCenter,
            overlay_show_lyrics: true,
            soundcloud_client_id: String::new(),
            language: Language::Auto,
            region_guard: false,
            region_countries: vec!["RU".into()],
            soundcloud_client_secret: String::new(),
            soundcloud_full_sync: false,
            spotify_client_id: String::new(),
            spotify_client_secret: String::new(),
            discord_enabled: true,
            discord_app_id: String::new(),
            discord_show_artwork: true,
            discord_show_paused: true,
            discord_show_timeline: true,
            discord_show_button: true,
            discord_join: true,
            dev_tools: false,
            discord_status_shows: StatusShows::Song,
            rest: Map::new(),
        }
    }
}

/// EvPlayer's own application on discord.com/developers (see src/settings/store.ts)
pub const DEFAULT_DISCORD_APP_ID: &str = "1564485226267279360";

impl Settings {
    pub fn discord_app_id(&self) -> String {
        let id = self.discord_app_id.trim();
        if id.is_empty() {
            DEFAULT_DISCORD_APP_ID.to_string()
        } else {
            id.to_string()
        }
    }
}

// ---- library ---------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LikedTrack {
    pub track: Track,
    pub added_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    pub id: String,
    pub track: Track,
    pub added_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// your own picture, a small data: url
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub items: Vec<PlaylistItem>,
    /// where it was imported from ("scp:123", a link): importing again updates it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    pub likes: Vec<LikedTrack>,
    pub playlists: Vec<Playlist>,
    /// when likes or playlists last changed (ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<f64>,
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

/// "pl_lx2k3abcde": the page's ids (time in base 36 and a random tail)
pub fn new_id(prefix: &str) -> String {
    let mut t = now_ms() as u64;
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut time = Vec::new();
    while t > 0 {
        time.push(digits[(t % 36) as usize]);
        t /= 36;
    }
    time.reverse();
    let tail: String = (0..5).map(|_| digits[rand::random::<usize>() % 36] as char).collect();
    format!("{prefix}_{}{tail}", String::from_utf8_lossy(&time))
}

pub fn move_item<T>(list: &mut Vec<T>, from: usize, to: usize) {
    if from >= list.len() || from == to {
        return;
    }
    let item = list.remove(from);
    let to = to.min(list.len());
    list.insert(to, item);
}

impl Library {
    fn touch(&mut self) {
        self.saved_at = Some(now_ms());
    }

    pub fn is_liked(&self, id: &str) -> bool {
        self.likes.iter().any(|l| l.track.id == id)
    }

    pub fn toggle_like(&mut self, track: &Track) -> bool {
        self.touch();
        if let Some(i) = self.likes.iter().position(|l| l.track.id == track.id) {
            self.likes.remove(i);
            false
        } else {
            self.likes.insert(0, LikedTrack { track: track.clone(), added_at: now_iso() });
            true
        }
    }

    /// like many at once (an import); how many were new
    pub fn add_likes(&mut self, tracks: &[Track]) -> usize {
        let mut have: std::collections::HashSet<String> = self.likes.iter().map(|l| l.track.id.clone()).collect();
        let stamp = now_iso();
        let fresh: Vec<LikedTrack> = tracks.iter().filter(|t| have.insert(t.id.clone())).map(|t| LikedTrack { track: t.clone(), added_at: stamp.clone() }).collect();
        let n = fresh.len();
        if n > 0 {
            self.touch();
            self.likes.splice(0..0, fresh);
        }
        n
    }

    pub fn move_like(&mut self, id: &str, to: usize) {
        if let Some(from) = self.likes.iter().position(|l| l.track.id == id) {
            move_item(&mut self.likes, from, to);
            self.touch();
        }
    }

    pub fn create_playlist(&mut self, title: &str, description: Option<&str>, source: Option<&str>) -> String {
        let stamp = now_iso();
        let title: String = title.trim().chars().take(60).collect();
        let p = Playlist {
            id: new_id("pl"),
            title: if title.is_empty() { "Новый плейлист".into() } else { title },
            description: description.map(|d| d.trim().chars().take(300).collect::<String>()).filter(|d| !d.is_empty()),
            icon: None,
            created_at: stamp.clone(),
            updated_at: stamp,
            items: Vec::new(),
            source: source.map(str::to_string),
        };
        let id = p.id.clone();
        self.playlists.insert(0, p);
        self.touch();
        id
    }

    pub fn playlist_mut(&mut self, id: &str) -> Option<&mut Playlist> {
        self.playlists.iter_mut().find(|p| p.id == id)
    }

    pub fn rename_playlist(&mut self, id: &str, title: &str, description: Option<&str>) {
        if let Some(p) = self.playlist_mut(id) {
            let t: String = title.trim().chars().take(60).collect();
            if !t.is_empty() {
                p.title = t;
            }
            if let Some(d) = description {
                let d: String = d.trim().chars().take(300).collect();
                p.description = (!d.is_empty()).then_some(d);
            }
            p.updated_at = now_iso();
        }
        self.touch();
    }

    pub fn delete_playlist(&mut self, id: &str) {
        self.playlists.retain(|p| p.id != id);
        self.touch();
    }

    pub fn add_to_playlist(&mut self, id: &str, tracks: &[Track]) -> usize {
        let Some(p) = self.playlists.iter_mut().find(|p| p.id == id) else { return 0 };
        // new to the playlist, and once each (an import can name a song twice)
        let mut have: std::collections::HashSet<String> = p.items.iter().map(|i| i.track.id.clone()).collect();
        let fresh: Vec<Track> = tracks.iter().filter(|t| have.insert(t.id.clone())).cloned().collect();
        if fresh.is_empty() {
            return 0;
        }
        let stamp = now_iso();
        let n = fresh.len();
        p.items.extend(fresh.into_iter().map(|track| PlaylistItem { id: new_id("pi"), track, added_at: stamp.clone() }));
        p.updated_at = stamp;
        self.touch();
        n
    }

    pub fn remove_from_playlist(&mut self, id: &str, item: &str) {
        if let Some(p) = self.playlist_mut(id) {
            p.items.retain(|i| i.id != item);
            p.updated_at = now_iso();
        }
        self.touch();
    }

    pub fn move_in_playlist(&mut self, id: &str, item: &str, to: usize) {
        if let Some(p) = self.playlist_mut(id) {
            if let Some(from) = p.items.iter().position(|i| i.id == item) {
                move_item(&mut p.items, from, to);
                p.updated_at = now_iso();
            }
        }
        self.touch();
    }
}

// ---- history, session, effects --------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaySource {
    /// "playlist" | "smart" | "collection" | "files"
    pub page: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl PlaySource {
    pub fn key(&self) -> String {
        format!("{}:{}", self.page, self.id.as_deref().unwrap_or(""))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub track: Track,
    pub played_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEntry {
    pub source: PlaySource,
    pub played_at: String,
}

/// "Недавно слушали": a song lands here after it played this long
pub const HISTORY_AFTER_MS: f64 = 5000.0;
const HISTORY_LIMIT: usize = 60;
const SOURCE_LIMIT: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
    pub sources: Vec<SourceEntry>,
}

impl History {
    pub fn push(&mut self, track: &Track, from: Option<&PlaySource>) {
        let at = now_iso();
        self.entries.retain(|e| e.track.id != track.id);
        self.entries.insert(0, HistoryEntry { track: track.clone(), played_at: at.clone(), from: from.map(|f| f.key()) });
        self.entries.truncate(HISTORY_LIMIT);
        if let Some(f) = from {
            let key = f.key();
            self.sources.retain(|s| s.source.key() != key);
            self.sources.insert(0, SourceEntry { source: f.clone(), played_at: at });
            self.sources.truncate(SOURCE_LIMIT);
        }
    }
}

/// the queue and position to pick up from (`evplayer.session.v1`, plain JSON)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    pub queue: Vec<Track>,
    pub index: i64,
    pub position_ms: f64,
}

/// "Скорость", "Тон", "Реверберация" (`evplayer.effects`, plain JSON)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Effects {
    /// keep them for the next song (otherwise they reset when a song changes)
    pub keep: bool,
    /// the speed also moves the pitch, the way a faster record sounds higher
    pub linked: bool,
    /// semitones, while not linked
    pub pitch: f64,
    pub reverb: bool,
    #[serde(rename = "reverbAmount")]
    pub reverb_amount: f64,
    #[serde(default = "one")]
    pub speed: f64,
}

impl Default for Effects {
    fn default() -> Self {
        Self { keep: false, linked: true, pitch: 0.0, reverb: false, reverb_amount: 0.5, speed: 1.0 }
    }
}

impl Effects {
    pub fn is_neutral(&self) -> bool {
        (self.speed - 1.0).abs() < 1e-6 && self.pitch.abs() < 1e-6 && !self.reverb
    }
}

#[allow(dead_code)]
fn _defaults_used() -> (bool, f64) {
    (yes(), one())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_keep_what_they_do_not_know() {
        let json = r#"{"autoplay":false,"crossfade":4,"soundcloudAccount":{"id":"scu:1","name":"x"},"futureThing":[1,2]}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(!s.autoplay);
        assert_eq!(s.crossfade, 4.0);
        assert!(s.close_to_tray, "a missing field takes the page's default");
        let back = serde_json::to_value(&s).unwrap();
        assert_eq!(back["futureThing"], serde_json::json!([1, 2]));
        assert_eq!(back["soundcloudAccount"]["name"], "x");
        assert_eq!(back["overlayCorner"], "top-center");
    }

    #[test]
    fn likes_and_playlists() {
        let t = |id: &str| Track { id: id.into(), title: id.into(), ..Default::default() };
        let mut lib = Library::default();
        assert!(lib.toggle_like(&t("sc:1")));
        assert_eq!(lib.add_likes(&[t("sc:1"), t("sc:2"), t("sc:2")]), 1);
        assert_eq!(lib.likes[0].track.id, "sc:2");
        let id = lib.create_playlist("  Утро  ", None, None);
        assert_eq!(lib.playlists[0].title, "Утро");
        assert_eq!(lib.add_to_playlist(&id, &[t("a"), t("b"), t("a")]), 2);
        assert_eq!(lib.add_to_playlist(&id, &[t("a")]), 0);
        let first = lib.playlists[0].items[0].id.clone();
        lib.move_in_playlist(&id, &first, 1);
        assert_eq!(lib.playlists[0].items[1].track.id, "a");
        assert!(new_id("pl").starts_with("pl_"));
    }

    #[test]
    fn history_keeps_one_entry_per_song_and_brings_the_playlist_up() {
        let t = Track { id: "sc:1".into(), ..Default::default() };
        let mut h = History::default();
        let src = PlaySource { page: "playlist".into(), id: Some("pl_x".into()), title: None, cover: None, kind: None };
        h.push(&t, Some(&src));
        h.push(&t, None);
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.sources.len(), 1);
        assert_eq!(h.sources[0].source.key(), "playlist:pl_x");
    }
}
