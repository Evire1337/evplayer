//! Поиск: SoundCloud and YouTube Music at once, or one of them; songs, artists, albums,
//! playlists (src/pages/SearchPage.tsx). The last queries are kept as the page kept them
//! (evplayer.searchHistory.v1).

use egui::Ui;

use super::Loaded;
use crate::app::{App, Page};
use crate::model::{ArtistSummary, CollectionKind, CollectionSummary, Track};
use crate::services::{soundcloud, youtube};
use crate::state;
use crate::store::kv;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

const HISTORY_KEY: &str = "evplayer.searchHistory.v1";
/// a query is asked this long after the last key press
const TYPE_PAUSE_MS: u64 = 350;

#[derive(Default, Clone)]
pub struct Results {
    pub tracks: Vec<Track>,
    pub artists: Vec<ArtistSummary>,
    pub albums: Vec<CollectionSummary>,
    pub playlists: Vec<CollectionSummary>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub focus: bool,
    source: usize,
    kind: usize,
    typed_at: Option<std::time::Instant>,
    asked: String,
    results: Loaded<Results>,
}

/// "Очистить историю" in Настройки → Данные: the recent queries go too
pub fn clear_history() {
    kv::write_state(&state::get().kv, HISTORY_KEY, &History::default(), 0);
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct History {
    queries: Vec<String>,
}

fn remember(q: &str) {
    let kv_ = &state::get().kv;
    let mut h: History = kv::read_state(kv_, HISTORY_KEY).unwrap_or_default();
    h.queries.retain(|x| x != q);
    h.queries.insert(0, q.to_string());
    h.queries.truncate(20);
    kv::write_state(kv_, HISTORY_KEY, &h, 0);
}

fn search(q: String, source: usize, kind: usize) -> Results {
    let (sc, yt) = (source != 2, source != 1);
    let mut r = Results::default();
    std::thread::scope(|s| {
        let a = s.spawn(|| if sc { soundcloud::search(&q).ok() } else { None });
        let b = s.spawn(|| if yt { youtube::search(&q).ok() } else { None });
        let albums = s.spawn(|| {
            if kind == 0 || kind == 3 {
                let mut v = Vec::new();
                if yt {
                    v.extend(youtube::search_collections(&q, CollectionKind::Album).unwrap_or_default());
                }
                if sc {
                    v.extend(soundcloud::search_collections(&q, CollectionKind::Album).unwrap_or_default());
                }
                v
            } else {
                vec![]
            }
        });
        let lists = s.spawn(|| {
            if kind == 0 || kind == 4 {
                let mut v = Vec::new();
                if yt {
                    v.extend(youtube::search_collections(&q, CollectionKind::Playlist).unwrap_or_default());
                }
                if sc {
                    v.extend(soundcloud::search_collections(&q, CollectionKind::Playlist).unwrap_or_default());
                }
                v
            } else {
                vec![]
            }
        });
        let scr = a.join().ok().flatten();
        let ytr = b.join().ok().flatten();
        // both services' songs, alternating, the same song once
        let (sct, sca) = scr.map(|(t, a)| (t, a)).unwrap_or_default();
        let ytt = ytr.unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        let (mut i, mut j) = (0, 0);
        while i < ytt.len() || j < sct.len() {
            if let Some(t) = ytt.get(i) {
                if seen.insert(crate::player::resolve::song_key(t)) {
                    r.tracks.push(t.clone());
                }
                i += 1;
            }
            if let Some(t) = sct.get(j) {
                if seen.insert(crate::player::resolve::song_key(t)) || !yt {
                    r.tracks.push(t.clone());
                }
                j += 1;
            }
        }
        r.artists = sca;
        r.albums = albums.join().unwrap_or_default();
        r.playlists = lists.join().unwrap_or_default();
        if r.tracks.is_empty() && r.albums.is_empty() && r.playlists.is_empty() {
            r.error = Some("Ничего не нашлось".into());
        }
    });
    r
}

impl App {
    pub fn search(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        let width = ui.available_width() - 8.0;
        let resp = widgets::search_field(ui, &mut self.pages.search.query, "Трек, исполнитель или ссылка на плейлист", width);
        if std::mem::take(&mut self.pages.search.focus) || self.pages.search.query.is_empty() && !resp.has_focus() && self.pages.search.typed_at.is_none() {
            resp.request_focus();
        }
        if resp.changed() {
            self.pages.search.typed_at = Some(std::time::Instant::now());
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(TYPE_PAUSE_MS + 20));
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if let Some(i) = widgets::tabs(ui, &["Везде", "SoundCloud", "YouTube Music"], self.pages.search.source) {
                self.pages.search.source = i;
            }
            ui.add_space(6.0);
            if let Some(i) = widgets::tabs(ui, &["Всё", "Песни", "Исполнители", "Альбомы", "Плейлисты"], self.pages.search.kind) {
                self.pages.search.kind = i;
            }
        });
        ui.add_space(8.0);
        let q = self.pages.search.query.trim().to_string();
        let st = &mut self.pages.search;
        let settled = st.typed_at.is_none_or(|t| t.elapsed().as_millis() as u64 >= TYPE_PAUSE_MS) || resp.lost_focus();
        // a link, a code or a list of songs pasted into the search: it can become a playlist
        if crate::import::link_kind(&q).is_some() || (q.contains('\n') && crate::import::looks_like_track_list(&q)) {
            ui.add_space(20.0);
            App::muted(ui, "Это ссылка или код плейлиста — его можно сохранить в библиотеку.");
            ui.add_space(8.0);
            if widgets::pill_button(ui, Some(ic::DOWNLOAD), "Импортировать", true).clicked() {
                self.start_import(q.clone(), None);
            }
            return;
        }
        if q.is_empty() {
            self.search_history(ui);
            return;
        }
        if settled && st.asked != q {
            st.asked = q.clone();
            remember(&q);
        }
        let asked = st.asked.clone();
        if asked.is_empty() {
            return;
        }
        let (source, kind) = (st.source, st.kind);
        let key = format!("{source}|{kind}|{asked}");
        let res = st.results.get(&key, move || search(asked, source, kind)).cloned();
        let Some(res) = res else {
            App::loading(ui);
            return;
        };
        egui::ScrollArea::vertical().id_salt("search").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);
            if let Some(e) = &res.error {
                App::muted(ui, e);
            }
            if (kind == 0 || kind == 2) && !res.artists.is_empty() {
                widgets::section(ui, "Исполнители");
                egui::ScrollArea::horizontal().id_salt("artists").auto_shrink([false, true]).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        for a in &res.artists {
                            let sub = a.followers.map(|f| format!("{} {}", fmt_count(f), crate::i18n::t("подписчиков"))).unwrap_or_default();
                            if widgets::card(ui, &mut self.images, a.avatar.as_deref(), &a.name, &sub, 140.0, true).clicked() {
                                self.go(Page::Artist(a.id.clone()));
                            }
                        }
                    });
                });
            }
            if (kind == 0 || kind == 1) && !res.tracks.is_empty() {
                widgets::section(ui, "Песни");
                let show: Vec<Track> = if kind == 0 { res.tracks.iter().take(12).cloned().collect() } else { res.tracks.clone() };
                let current = self.view.current().map(|t| t.id.clone());
                for (i, t) in show.iter().enumerate() {
                    let liked = self.is_liked(&t.id);
                    let out = widgets::track_row(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), liked, Some(if t.id.starts_with("sc:") { "SoundCloud" } else { "YouTube" }), None);
                    if out.clicked || out.double {
                        self.play_list(&show, i, None);
                    }
                    if out.like {
                        self.toggle_like(t);
                    }
                    if out.menu {
                        self.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center()), playlist_item: None });
                    }
                }
            }
            if (kind == 0 || kind == 3) && !res.albums.is_empty() {
                widgets::section(ui, "Альбомы");
                self.collection_row(ui, "albums", &res.albums);
            }
            if (kind == 0 || kind == 4) && !res.playlists.is_empty() {
                widgets::section(ui, "Плейлисты");
                self.collection_row(ui, "playlists", &res.playlists);
            }
            ui.add_space(20.0);
        });
    }

    fn search_history(&mut self, ui: &mut Ui) {
        let h: History = kv::read_state(&state::get().kv, HISTORY_KEY).unwrap_or_default();
        if h.queries.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| App::muted(ui, "Ищите сразу на SoundCloud и YouTube Music — или вставьте ссылку на плейлист из Spotify, YouTube, SoundCloud или Deezer."));
            return;
        }
        widgets::section(ui, "Вы искали");
        for q in h.queries.iter().take(12) {
            let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(520.0), 34.0), egui::Sense::click());
            if r.hovered() {
                ui.painter().rect_filled(rect, 10, theme::hover());
            }
            crate::ui::theme::paint_icon(&ui.painter(), rect.left_center() + egui::vec2(14.0, 0.0), egui::Align2::CENTER_CENTER, ic::CLOCK, 12.0, theme::ink3());
            ui.painter().text(rect.left_center() + egui::vec2(32.0, 0.0), egui::Align2::LEFT_CENTER, q, theme::body(14.0), INK);
            if r.clicked() {
                self.pages.search.query = q.clone();
                self.pages.search.typed_at = None;
            }
        }
    }
}

pub fn fmt_count(n: f64) -> String {
    if n >= 1e9 {
        format!("{:.1} млрд", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.1} млн", n / 1e6)
    } else if n >= 1e3 {
        format!("{:.0} тыс.", n / 1e3)
    } else {
        format!("{n:.0}")
    }
}
