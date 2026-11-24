//! Поиск (src/pages/SearchPage.tsx): SoundCloud and YouTube Music at once, or one of them;
//! artists, albums, playlists and songs, each song marked with where it is from. The recent
//! queries are kept as the page keeps them (evplayer.searchHistory.v1): a query goes in when
//! Enter is pressed or when its results stayed on screen a moment.

use egui::{pos2, vec2, Align2, Color32, Rect, Stroke, Ui};

use super::kit;
use super::Loaded;
use crate::app::{App, Page};
use crate::model::{ArtistSummary, CollectionKind, CollectionSummary, Track};
use crate::services::{soundcloud, youtube};
use crate::state;
use crate::store::kv;
use crate::store::state::SearchSource;
use crate::ui::theme::{self, INK};
use crate::ui::widgets;

const HISTORY_KEY: &str = "evplayer.searchHistory.v1";
/// a query is asked this long after the last key press
const TYPE_PAUSE_MS: u64 = 350;
/// a query that stayed on screen with results this long counts as a search for the history
const HISTORY_AFTER_MS: u64 = 2500;
/// the page's gap between its blocks (.page: gap 30) and the filters' place under the field
const PAGE_GAP: f32 = 30.0;

#[derive(Default, Clone)]
pub struct Results {
    pub tracks: Vec<Track>,
    pub artists: Vec<ArtistSummary>,
    pub albums: Vec<CollectionSummary>,
    pub playlists: Vec<CollectionSummary>,
    /// the services that did not answer
    pub failed: Vec<&'static str>,
}

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub focus: bool,
    /// "Везде", SoundCloud, YouTube Music; unset: the default from Настройки
    source: Option<usize>,
    kind: usize,
    typed_at: Option<std::time::Instant>,
    asked: String,
    results: Loaded<Results>,
    /// the last results, shown dimmed while new ones load
    last: Option<Results>,
    /// when the shown results came in (for the history)
    shown_at: Option<(String, std::time::Instant)>,
}

/// "Очистить историю" in Настройки → Данные: the recent queries go too
pub fn clear_history() {
    kv::write_state(&state::get().kv, HISTORY_KEY, &History::default(), 0);
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct History {
    queries: Vec<String>,
}

fn history() -> History {
    kv::read_state(&state::get().kv, HISTORY_KEY).unwrap_or_default()
}

fn remember(q: &str) {
    let q = q.trim();
    if q.is_empty() {
        return;
    }
    let mut h = history();
    h.queries.retain(|x| !x.eq_ignore_ascii_case(q));
    h.queries.insert(0, q.to_string());
    h.queries.truncate(12);
    kv::write_state(&state::get().kv, HISTORY_KEY, &h, 0);
}

fn forget(q: &str) {
    let mut h = history();
    h.queries.retain(|x| x != q);
    kv::write_state(&state::get().kv, HISTORY_KEY, &h, 0);
}

/// one list from two: take turns so neither buries the other
fn mix<T: Clone>(a: &[T], b: &[T]) -> Vec<T> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    for i in 0..a.len().max(b.len()) {
        if let Some(x) = a.get(i) {
            out.push(x.clone());
        }
        if let Some(x) = b.get(i) {
            out.push(x.clone());
        }
    }
    out
}

fn collections(q: &str, sc: bool, yt: bool, kind: CollectionKind) -> Vec<CollectionSummary> {
    std::thread::scope(|s| {
        let a = s.spawn(|| if sc { soundcloud::search_collections(q, kind).unwrap_or_default() } else { vec![] });
        let b = s.spawn(|| if yt { youtube::search_collections(q, kind).unwrap_or_default() } else { vec![] });
        let scl = a.join().unwrap_or_default();
        let ytl = b.join().unwrap_or_default();
        mix(&ytl, &scl)
    })
}

/// runSearch: kinds 0 all, 1 songs, 2 artists, 3 albums, 4 playlists
fn search(q: String, source: usize, kind: usize) -> Results {
    let (sc, yt) = (source != 2, source != 1);
    let mut r = Results::default();
    if kind == 3 || kind == 4 {
        let list = collections(&q, sc, yt, if kind == 3 { CollectionKind::Album } else { CollectionKind::Playlist });
        if kind == 3 {
            r.albums = list;
        } else {
            r.playlists = list;
        }
        return r;
    }
    std::thread::scope(|s| {
        let a = s.spawn(|| if sc { Some(soundcloud::search(&q)) } else { None });
        let b = s.spawn(|| if yt { Some(youtube::search(&q)) } else { None });
        let albums = s.spawn(|| if kind == 0 { collections(&q, sc, yt, CollectionKind::Album) } else { vec![] });
        let (sct, sca) = match a.join().ok().flatten() {
            Some(Ok(x)) => x,
            Some(Err(_)) => {
                r.failed.push("SoundCloud");
                Default::default()
            }
            None => Default::default(),
        };
        let ytt = match b.join().ok().flatten() {
            Some(Ok(x)) => x,
            Some(Err(_)) => {
                r.failed.push("YouTube Music");
                vec![]
            }
            None => vec![],
        };
        r.tracks = mix(&sct, &ytt);
        r.artists = sca;
        r.albums = albums.join().unwrap_or_default();
    });
    r
}

impl App {
    pub fn search(&mut self, ui: &mut Ui) {
        let default_source = match state::get().settings.read().unwrap().default_search_source {
            SearchSource::All => 0,
            SearchSource::Soundcloud => 1,
            SearchSource::Youtube => 2,
        };
        let source = self.pages.search.source.unwrap_or(default_source);
        ui.add_space(18.0);
        let resp = self.search_box(ui);
        if std::mem::take(&mut self.pages.search.focus) || self.pages.search.query.is_empty() && !resp.has_focus() && self.pages.search.typed_at.is_none() {
            resp.request_focus();
        }
        if resp.changed() {
            self.pages.search.typed_at = Some(std::time::Instant::now());
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(TYPE_PAUSE_MS + 20));
        }
        let q = self.pages.search.query.trim().to_string();
        let link = crate::import::link_kind(&q).is_some() || (q.contains('\n') && crate::import::looks_like_track_list(&q));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter {
            if link {
                self.start_import(q.clone(), None);
            } else {
                remember(&q);
            }
        }
        ui.add_space(16.0);
        // .search-filters: where to look, and what to show
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            if let Some(i) = kit::segmented(ui, &["Везде", "SoundCloud", "YouTube Music"], source) {
                self.pages.search.source = Some(i);
            }
            if let Some(i) = kit::segmented(ui, &["Всё", "Треки", "Исполнители", "Альбомы", "Плейлисты"], self.pages.search.kind) {
                self.pages.search.kind = i;
            }
        });
        ui.add_space(PAGE_GAP);
        let kind = self.pages.search.kind;
        // a link, a code or a list of songs pasted into the search: it can become a playlist
        if link {
            self.link_hint(ui, &q);
            return;
        }
        if q.chars().count() < 2 {
            if q.is_empty() && !history().queries.is_empty() {
                self.search_history(ui);
            } else {
                centered_note(ui, "Ищите на SoundCloud и YouTube Music сразу — или вставьте ссылку на плейлист из Spotify, YouTube, SoundCloud или Deezer.");
            }
            return;
        }
        let st = &mut self.pages.search;
        let settled = st.typed_at.is_none_or(|t| t.elapsed().as_millis() as u64 >= TYPE_PAUSE_MS) || enter;
        if settled && st.asked != q {
            st.asked = q.clone();
        }
        let asked = st.asked.clone();
        if asked.is_empty() {
            return;
        }
        let key = format!("{source}|{kind}|{asked}");
        let fresh = st.results.get(&key, move || search(asked, source, kind)).cloned();
        let loading = fresh.is_none() || st.asked != q;
        if let Some(r) = &fresh {
            st.last = Some(r.clone());
        }
        // results that stayed a moment: the query goes into the history
        if let Some(r) = &fresh {
            let found = r.tracks.len() + r.artists.len() + r.albums.len() + r.playlists.len() > 0;
            match &st.shown_at {
                Some((k, at)) if *k == key => {
                    if found && at.elapsed().as_millis() as u64 >= HISTORY_AFTER_MS {
                        remember(&st.asked);
                        st.shown_at = Some((key.clone(), std::time::Instant::now() + std::time::Duration::from_secs(3600)));
                    } else if found && at.elapsed().as_millis() as u64 <= HISTORY_AFTER_MS {
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(HISTORY_AFTER_MS + 50) - at.elapsed().min(std::time::Duration::from_millis(HISTORY_AFTER_MS)));
                    }
                }
                _ => {
                    st.shown_at = Some((key.clone(), std::time::Instant::now()));
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(HISTORY_AFTER_MS + 50));
                }
            }
        }
        let Some(res) = fresh.or_else(|| st.last.clone()) else {
            App::loading(ui);
            return;
        };
        let nothing = res.tracks.len() + res.artists.len() + res.albums.len() + res.playlists.len() == 0;
        egui::ScrollArea::vertical().id_salt("search").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            if !loading {
                if nothing && !res.failed.is_empty() {
                    centered_note(ui, "Не получилось найти. Проверьте интернет и попробуйте ещё раз.");
                    return;
                }
                if !res.failed.is_empty() {
                    kit::note(ui, &format!("{} {}", res.failed.join(&format!(" {} ", crate::i18n::t("и"))), crate::i18n::t("сейчас не отвечает — показаны остальные результаты.")));
                    ui.add_space(PAGE_GAP);
                }
                if nothing {
                    centered_note(ui, "Ничего не нашлось.");
                    return;
                }
            }
            if (kind == 0 || kind == 2) && !res.artists.is_empty() {
                row_title(ui, "Исполнители");
                self.artist_row(ui, &res.artists);
                ui.add_space(PAGE_GAP);
            }
            if (kind == 0 || kind == 3) && !res.albums.is_empty() {
                row_title(ui, "Альбомы");
                if kind == 3 {
                    self.card_grid(ui, &res.albums);
                } else {
                    self.collection_row(ui, "albums", &res.albums);
                }
                ui.add_space(PAGE_GAP);
            }
            if kind == 4 && !res.playlists.is_empty() {
                row_title(ui, "Плейлисты");
                self.card_grid(ui, &res.playlists);
                ui.add_space(PAGE_GAP);
            }
            if (kind == 0 || kind == 1) && !res.tracks.is_empty() {
                row_title(ui, "Треки");
                let current = self.view.current().map(|t| t.id.clone());
                let playing = self.view.status == crate::player::Status::Playing;
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("evplayer-playing"), playing));
                // .track-list.is-stale: the old results stay, dimmed, while new ones load
                if loading {
                    ui.set_opacity(0.5);
                }
                for (i, t) in res.tracks.iter().enumerate() {
                    let liked = self.is_liked(&t.id);
                    let out = widgets::track_row_full(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), liked, None, None, false, true);
                    ui.add_space(2.0);
                    if out.clicked || out.double {
                        self.play_list(&res.tracks, i, None);
                    }
                    if out.like {
                        self.toggle_like(t);
                    }
                    if out.menu {
                        self.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center()), playlist_item: None, queue_index: None, sub: false });
                    }
                }
            }
            ui.add_space(20.0);
        });
    }

    /// .search-field: 56 high, rounded 22, dark glass with a shadow; the icon, the field and a
    /// cross that empties it
    fn search_box(&mut self, ui: &mut Ui) -> egui::Response {
        let w = ui.available_width() - 8.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, 56.0), egui::Sense::hover());
        let p = ui.painter();
        p.add(egui::Shadow { offset: [0, 10], blur: 24, spread: 0, color: Color32::from_black_alpha(77) }.as_shape(rect, egui::CornerRadius::same(22)));
        p.rect_filled(rect, 22, Color32::from_rgba_unmultiplied(8, 22, 24, 230));
        crate::ui::icons::paint(p, "search", Rect::from_center_size(pos2(rect.left() + 16.0 + 9.5, rect.center().y), vec2(19.0, 19.0)), theme::ink2());
        let has_text = !self.pages.search.query.is_empty();
        let right = if has_text { 16.0 + 30.0 + 10.0 } else { 16.0 };
        let inner = Rect::from_min_max(pos2(rect.left() + 16.0 + 19.0 + 10.0, rect.top()), pos2(rect.right() - right, rect.bottom()));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let r = c.add(
            egui::TextEdit::singleline(&mut self.pages.search.query)
                .hint_text(egui::RichText::new(crate::i18n::t("Трек, исполнитель или ссылка на плейлист")).color(theme::ink3()))
                .frame(egui::Frame::NONE)
                .font(theme::medium(16.0))
                .text_color(INK)
                .desired_width(inner.width())
                .align(Align2::LEFT_CENTER),
        );
        if has_text {
            let x = Rect::from_center_size(pos2(rect.right() - 16.0 - 15.0, rect.center().y), vec2(30.0, 30.0));
            let xr = ui.interact(x, ui.scope_id().with("search-clear"), egui::Sense::click());
            if xr.hovered() {
                ui.painter().circle_filled(x.center(), 15.0, theme::hover());
            }
            crate::ui::icons::paint(ui.painter(), "close", Rect::from_center_size(x.center(), vec2(16.0, 16.0)), if xr.hovered() { INK } else { theme::ink2() });
            if xr.on_hover_text(crate::i18n::t("Очистить")).clicked() {
                self.pages.search.query.clear();
                self.pages.search.typed_at = Some(std::time::Instant::now());
                self.pages.search.focus = true;
            }
        }
        r
    }

    /// .link-hint: a link in the field can become a playlist
    fn link_hint(&mut self, ui: &mut Ui, q: &str) {
        let w = ui.available_width() - 8.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, 64.0), egui::Sense::hover());
        ui.painter().rect(rect, 18, theme::glass(), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        ui.painter().text(pos2(rect.left() + 18.0, rect.center().y), Align2::LEFT_CENTER, crate::i18n::t("Это ссылка. Добавить её содержимое в библиотеку как плейлист?"), theme::body(14.0), INK);
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(12.0)).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if widgets::btn(&mut c, None, "Импортировать", true).clicked() {
            self.start_import(q.to_string(), None);
        }
    }

    /// .search-history: "Недавние запросы" with "Очистить", each query with a cross
    fn search_history(&mut self, ui: &mut Ui) {
        let h = history();
        let w = ui.available_width().min(640.0);
        ui.horizontal(|ui| {
            ui.set_width(w);
            ui.label(egui::RichText::new(crate::i18n::t("Недавние запросы")).font(theme::heavy(19.0)).color(INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::text_button(ui, "Очистить").clicked() {
                    clear_history();
                }
            });
        });
        ui.add_space(6.0);
        for q in &h.queries {
            let (rect, r) = ui.allocate_exact_size(vec2(w, 41.0), egui::Sense::click());
            let x = Rect::from_center_size(pos2(rect.right() - 4.0 - 15.0, rect.center().y), vec2(30.0, 30.0));
            let xr = ui.interact(x, ui.scope_id().with(("forget", q)), egui::Sense::click());
            if r.hovered() || xr.hovered() {
                ui.painter().rect_filled(rect, 14, theme::hover());
            }
            crate::ui::icons::paint(ui.painter(), "search", Rect::from_center_size(pos2(rect.left() + 12.0 + 7.5, rect.center().y), vec2(15.0, 15.0)), theme::ink2());
            let g = widgets::clipped(ui, q, theme::body(14.0), INK, w - 12.0 - 15.0 - 12.0 - 40.0);
            ui.painter().galley(pos2(rect.left() + 12.0 + 15.0 + 12.0, rect.center().y - g.size().y / 2.0), g, INK);
            if xr.hovered() {
                ui.painter().circle_filled(x.center(), 15.0, theme::hover());
            }
            crate::ui::icons::paint(ui.painter(), "close", Rect::from_center_size(x.center(), vec2(14.0, 14.0)), if xr.hovered() { INK } else { theme::ink2().gamma_multiply(0.6) });
            if xr.on_hover_text(format!("{} «{q}» {}", crate::i18n::t("Убрать"), crate::i18n::t("из истории"))).clicked() {
                forget(q);
            } else if r.clicked() {
                self.pages.search.query = q.clone();
                self.pages.search.typed_at = None;
            }
            ui.add_space(2.0);
        }
    }

    /// .artist-chip: a round picture 112 across, the name and the followers, 132 wide
    fn artist_row(&mut self, ui: &mut Ui, artists: &[ArtistSummary]) {
        egui::ScrollArea::horizontal().id_salt("artists").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for a in artists {
                    let (rect, r) = ui.allocate_exact_size(vec2(132.0, 8.0 + 112.0 + 6.0 + 18.0 + 4.0 + 17.0 + 8.0), egui::Sense::click());
                    if r.hovered() {
                        ui.painter().rect_filled(rect, 18, theme::hover());
                    }
                    let pic = Rect::from_center_size(pos2(rect.center().x, rect.top() + 8.0 + 56.0), vec2(112.0, 112.0));
                    ui.painter().circle_filled(pic.center(), 56.0, Color32::from_rgba_unmultiplied(255, 255, 255, 15));
                    if a.avatar.is_some() {
                        widgets::paint_cover(ui, &mut self.images, a.avatar.as_deref(), pic, 56);
                    }
                    let g = widgets::clipped(ui, &a.name, theme::medium(14.0), INK, 124.0);
                    ui.painter().galley(pos2(rect.center().x - g.size().x / 2.0, pic.bottom() + 6.0), g, INK);
                    if let Some(f) = a.followers {
                        let g = widgets::clipped(ui, &format!("{} {}", widgets::count(f), crate::i18n::t("подписчиков")), theme::body(12.5), theme::ink3(), 124.0);
                        ui.painter().galley(pos2(rect.center().x - g.size().x / 2.0, pic.bottom() + 6.0 + 22.0), g, theme::ink3());
                    }
                    if r.clicked() {
                        self.go(Page::Artist(a.id.clone()));
                    }
                }
            });
        });
    }

    /// .card-grid: as many 170+ columns as fit, 18 apart, rows 22 apart
    fn card_grid(&mut self, ui: &mut Ui, list: &[CollectionSummary]) {
        let w = ui.available_width();
        let cols = (((w + 18.0) / (170.0 + 18.0)).floor() as usize).max(1);
        let cw = (w - 18.0 * (cols as f32 - 1.0)) / cols as f32;
        for chunk in list.chunks(cols) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                for c in chunk {
                    if widgets::card(ui, &mut self.images, c.artwork.as_deref(), &c.title, &c.subtitle, cw, false).clicked() {
                        self.go(Page::Collection(c.id.clone()));
                    }
                }
            });
            ui.add_space(22.0);
        }
    }
}

/// .row-title: 19 px, 800, 14 above the row's content
fn row_title(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(crate::i18n::t(text)).font(theme::heavy(19.0)).color(INK));
    ui.add_space(14.0);
}

/// .muted.center: a faint line in the middle, 40 below the filters
fn centered_note(ui: &mut Ui, text: &str) {
    ui.add_space(40.0 - PAGE_GAP.min(40.0) + 10.0);
    let w = ui.available_width() - 8.0;
    let g = kit::wrapped(ui, &crate::i18n::t(text), theme::body(14.0), theme::ink3(), w, 21.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), egui::Sense::hover());
    // each line centred
    let job_w = g.size().x;
    ui.painter().galley(pos2(r.center().x - job_w / 2.0, r.top()), g, theme::ink3());
}

pub fn fmt_count(n: f64) -> String {
    widgets::count(n)
}
