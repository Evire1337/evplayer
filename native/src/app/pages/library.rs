//! Библиотека: "Понравившиеся", your files and your playlists as cards (src/pages/LibraryPage.tsx),
//! and a playlist of yours with its songs (PlaylistPage.tsx).

use egui::{pos2, vec2, Rect, Ui};

use crate::app::{App, Dialog, Page};
use crate::model::Track;
use crate::state;
use crate::store::state::PlaySource;
use crate::ui::images::artwork_of;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

use super::covers;

/// When a song was added, as the page's formatAddedAt (Intl.RelativeTimeFormat, short):
/// "сейчас", "5 мин. назад", "3 ч назад", "вчера", "3 дн. назад", "3 нед. назад", then the date
pub fn ago(iso: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else { return String::new() };
    let secs = (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds().max(0);
    let en = crate::i18n::is_english();
    let (m, h, d) = (secs / 60, secs / 3600, secs / 86_400);
    match secs {
        0..=59 => (if en { "now" } else { "сейчас" }).into(),
        60..=3599 => if en { format!("{m} min. ago") } else { format!("{m} мин. назад") },
        3600..=86_399 => if en { format!("{h} hr. ago") } else { format!("{h} ч назад") },
        _ if d < 7 => match d {
            1 => (if en { "yesterday" } else { "вчера" }).into(),
            2 if !en => "позавчера".into(),
            _ => if en { format!("{d} days ago") } else { format!("{d} дн. назад") },
        },
        _ if d < 30 => {
            let w = d / 7;
            if w == 1 {
                (if en { "last wk." } else { "на прошлой неделе" }).into()
            } else if en {
                format!("{w} wk. ago")
            } else {
                format!("{w} нед. назад")
            }
        }
        _ => date(iso),
    }
}

/// "27 окт. 2026 г." / "Oct 27, 2026", as Intl's short date
pub fn date(iso: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else { return String::new() };
    let t = t.with_timezone(&chrono::Local);
    use chrono::Datelike;
    const RU: [&str; 12] = ["янв.", "февр.", "мар.", "апр.", "мая", "июн.", "июл.", "авг.", "сент.", "окт.", "нояб.", "дек."];
    const EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let m = t.month0() as usize;
    if crate::i18n::is_english() {
        format!("{} {}, {}", EN[m], t.day(), t.year())
    } else {
        format!("{} {} {} г.", t.day(), RU[m], t.year())
    }
}

// ---- sorting (src/library/sort.ts) ----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Custom,
    Added,
    Title,
    Artist,
    Duration,
    Plays,
}

const SORT_KEYS: [(SortKey, &str, &str); 6] = [
    (SortKey::Custom, "custom", "Свой порядок"),
    (SortKey::Added, "added", "Дата добавления"),
    (SortKey::Title, "title", "Название"),
    (SortKey::Artist, "artist", "Исполнитель"),
    (SortKey::Duration, "duration", "Длительность"),
    (SortKey::Plays, "plays", "Ваши прослушивания"),
];

impl SortKey {
    fn name(self) -> &'static str {
        SORT_KEYS.iter().find(|k| k.0 == self).map(|k| k.1).unwrap_or("custom")
    }
    fn label(self) -> &'static str {
        SORT_KEYS.iter().find(|k| k.0 == self).map(|k| k.2).unwrap_or("")
    }
    /// the direction a key starts with: newest, longest and most played first; names A→Я
    fn first_asc(self) -> bool {
        matches!(self, SortKey::Custom | SortKey::Title | SortKey::Artist)
    }
}

/// the order chosen for song lists, kept in the settings as the Tauri build does ("trackSort")
pub fn track_sort() -> (SortKey, bool) {
    let v = state::get().extra("trackSort").unwrap_or_default();
    let key = SORT_KEYS.iter().find(|k| Some(k.1) == v["key"].as_str()).map(|k| k.0).unwrap_or(SortKey::Custom);
    (key, v["dir"].as_str() != Some("desc"))
}

fn set_track_sort(key: SortKey, asc: bool) {
    state::get().set_extra("trackSort", serde_json::json!({ "key": key.name(), "dir": if asc { "asc" } else { "desc" } }));
}

/// pressing a key: the same key turns the direction, another starts with its own
fn press_sort(key: SortKey) {
    let (cur, asc) = track_sort();
    set_track_sort(key, if cur == key { !asc } else { key.first_asc() });
}

/// rows in the chosen order; ties keep their own order
pub fn sort_rows<T: Clone>(rows: &[(Track, String, T)], key: SortKey, asc: bool) -> Vec<(Track, String, T)> {
    if key == SortKey::Custom {
        let mut v = rows.to_vec();
        if !asc {
            v.reverse();
        }
        return v;
    }
    let plays = |t: &Track| crate::stats::plays_of(&t.id) as f64;
    let lower = |s: &str| s.to_lowercase();
    let mut v: Vec<(usize, &(Track, String, T))> = rows.iter().enumerate().collect();
    v.sort_by(|(ia, a), (ib, b)| {
        let c = match key {
            SortKey::Added => a.1.cmp(&b.1),
            SortKey::Title => lower(&a.0.title).cmp(&lower(&b.0.title)),
            SortKey::Artist => lower(a.0.artists.first().map(|x| x.name.as_str()).unwrap_or("")).cmp(&lower(b.0.artists.first().map(|x| x.name.as_str()).unwrap_or(""))),
            SortKey::Duration => a.0.duration_ms.partial_cmp(&b.0.duration_ms).unwrap_or(std::cmp::Ordering::Equal),
            SortKey::Plays => plays(&a.0).partial_cmp(&plays(&b.0)).unwrap_or(std::cmp::Ordering::Equal),
            SortKey::Custom => std::cmp::Ordering::Equal,
        };
        let c = if asc { c } else { c.reverse() };
        c.then(ia.cmp(ib))
    });
    v.into_iter().map(|(_, r)| r.clone()).collect()
}

impl App {
    /// Библиотека (LibraryPage.tsx): the head with its actions and the view switch, then
    /// "Понравившиеся", "Мои файлы", the smart lists that have songs and your playlists, as big
    /// cards or as a list
    pub fn library_page(&mut self, ui: &mut Ui) {
        let list_view = state::get().settings.read().unwrap().library_view == crate::store::state::LibraryView::List;
        // .page-head.with-actions: the title, the actions at the right, wrapping when narrow
        ui.add_space(18.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Библиотека")).font(theme::heavy(40.0)).color(INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add_space(8.0);
                if let Some(list) = view_toggle(ui, list_view) {
                    state::get().set_settings(|s| s.library_view = if list { crate::store::state::LibraryView::List } else { crate::store::state::LibraryView::Grid });
                }
                if widgets::btn(ui, Some("download"), "Скачать офлайн", false).clicked() {
                    self.dialog = Some(Dialog::Download { picked: vec![], follow: true });
                }
                if widgets::btn(ui, None, "Из Spotify", false).clicked() {
                    self.dialog = Some(Dialog::SpotifyExport { lists: vec![], chosen: vec![], error: String::new() });
                }
                if widgets::btn(ui, Some("code"), "Импорт по ссылке или коду", false).clicked() {
                    self.dialog = Some(Dialog::Import { text: String::new(), likes: false });
                }
                if widgets::btn(ui, Some("plus"), "Новый плейлист", true).clicked() {
                    self.dialog = Some(Dialog::NewPlaylist { title: String::new(), add: vec![] });
                }
            });
        });
        ui.add_space(14.0);
        let lib = state::get().library.read().unwrap().clone();
        let files = self.library.list().len();
        let liked: Vec<Track> = lib.likes.iter().map(|l| l.track.clone()).collect();
        let smart: Vec<(&str, &str, f32, Vec<Track>)> = crate::stats::SMART.iter().map(|(k, title, _, hue)| (*k, *title, *hue, crate::stats::smart(k, &liked, 100))).filter(|s| !s.3.is_empty()).collect();
        let playlist_tracks: Vec<Vec<Track>> = lib.playlists.iter().map(|p| p.items.iter().take(40).map(|i| i.track.clone()).collect()).collect();
        // every card: its picture, name, line and where it leads
        let mut cards: Vec<(covers::Cover, String, String, Page)> = vec![
            (covers::Cover::Liked, crate::i18n::t("Понравившиеся"), crate::i18n::tracks(lib.likes.len()), Page::Playlist("likes".into())),
            (covers::Cover::Files, crate::i18n::t("Мои файлы"), if files > 0 { crate::i18n::tracks(files) } else { crate::i18n::t("MP3, FLAC и другие с компьютера") }, Page::Files),
        ];
        for (kind, title, hue, tracks) in &smart {
            cards.push((covers::Cover::Smart { hue: *hue, tracks }, crate::i18n::t(title), format!("{} · {}", crate::i18n::t("Умный"), crate::i18n::tracks(tracks.len())), Page::Playlist(format!("smart:{kind}"))));
        }
        for (p, tracks) in lib.playlists.iter().zip(&playlist_tracks) {
            cards.push((covers::Cover::Playlist { icon: p.icon.as_deref(), tracks }, p.title.clone(), format!("{} · {} {}", crate::i18n::tracks(p.items.len()), crate::i18n::t("создан"), date(&p.created_at)), Page::Playlist(p.id.clone())));
        }
        let mut go = None;
        egui::ScrollArea::vertical().id_salt("library").auto_shrink([false, false]).show(ui, |ui| {
            let avail = ui.available_width() - 8.0;
            if list_view {
                // .pl-list: rows with 2 between
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.set_width(avail);
                for (cover, title, sub, page) in &cards {
                    if covers::row(ui, &mut self.images, cover, title, sub).clicked() {
                        go = Some(page.clone());
                    }
                }
            } else {
                // .pl-grid: repeat(auto-fill, minmax(170px, 1fr)), 22 between rows, 18 between columns
                let per_row = ((avail + 18.0) / (170.0 + 18.0)).floor().max(1.0) as usize;
                let w = ((avail + 18.0) / per_row as f32 - 18.0).floor();
                for row in cards.chunks(per_row) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 18.0;
                        for (cover, title, sub, page) in row {
                            if covers::card(ui, &mut self.images, cover, w, title, sub).clicked() {
                                go = Some(page.clone());
                            }
                        }
                    });
                    ui.add_space(22.0 - ui.spacing().item_spacing.y);
                }
            }
            if lib.playlists.is_empty() {
                ui.add_space(8.0);
                App::muted(ui, "Создайте свой первый плейлист: в него можно класть треки из SoundCloud, YouTube и ваших файлов вперемешку. Через правый клик по любому треку → «Добавить в плейлист».");
            }
        });
        if let Some(p) = go {
            self.go(p);
        }
    }

    /// A smart list (SmartPage.tsx): its picture, "Умный плейлист · обновляется сам", the name,
    /// what it gathers and how many songs; play, shuffle, "Сохранить как плейлист"; the songs
    /// with their service marked. No filter or order: the list orders itself.
    fn smart_page(&mut self, ui: &mut Ui, kind: &str) {
        let liked: Vec<Track> = state::get().library.read().unwrap().likes.iter().map(|l| l.track.clone()).collect();
        let (title, desc, hue) = crate::stats::SMART.iter().find(|s| s.0 == kind).map(|s| (s.1, s.2, s.3)).unwrap_or(("", "", 160.0));
        let tracks = crate::stats::smart(kind, &liked, 100);
        let source = PlaySource { page: "smart".into(), id: Some(kind.to_string()), title: None, cover: None, kind: None };
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let art = 210.0;
            let (rect, _) = ui.allocate_exact_size(vec2(art, art), egui::Sense::hover());
            covers::paint(ui, &mut self.images, &covers::Cover::Smart { hue, tracks: &tracks }, rect, false);
            ui.add_space(28.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.add_space((art - (4.0 * 26.0 + 56.0)).max(0.0));
                ui.label(egui::RichText::new(crate::i18n::t("Умный плейлист · обновляется сам").to_uppercase()).font(theme::medium(12.0)).color(theme::ink3()).extra_letter_spacing(1.0));
                ui.label(egui::RichText::new(crate::i18n::t(title)).font(theme::heavy(46.0)).color(INK));
                ui.label(egui::RichText::new(format!("{} · {}", crate::i18n::t(desc), crate::i18n::tracks(tracks.len()))).font(theme::body(13.0)).color(theme::ink2()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::round_button(ui, ic::PLAY, 52.0, widgets::Round::Main, false, "Слушать").clicked() && !tracks.is_empty() {
                        self.player.play_queue(tracks.clone(), 0, Some(source.clone()));
                    }
                    // shuffle on, then from a random song
                    if widgets::round_button(ui, ic::SHUFFLE, 42.0, widgets::Round::Soft, false, "Перемешать").clicked() && !tracks.is_empty() {
                        if !self.view.shuffle {
                            self.player.toggle_shuffle();
                        }
                        let start = rand::random::<usize>() % tracks.len();
                        self.player.play_queue(tracks.clone(), start, Some(source.clone()));
                    }
                    if widgets::btn(ui, None, "Сохранить как плейлист", false).clicked() && !tracks.is_empty() {
                        self.dialog = Some(Dialog::NewPlaylist { title: String::new(), add: tracks.clone() });
                    }
                });
            });
        });
        ui.add_space(30.0);
        if tracks.is_empty() {
            ui.vertical_centered(|ui| App::muted(ui, "Пока пусто — слушайте музыку, и плейлист наполнится сам."));
            return;
        }
        self.track_list_badged(ui, &tracks, Some(source));
    }

    /// A list of songs of yours (PlaylistPage.tsx): the cover, what it is, its name and
    /// description, how many songs and when it was made, "Слушать", "Вперемешку", downloading
    /// and "…"; then the filter, the order, and the songs with the date each was added.
    pub fn playlist(&mut self, ui: &mut Ui, id: &str) {
        if let Some(kind) = id.strip_prefix("smart:") {
            let kind = kind.to_string();
            self.smart_page(ui, &kind);
            return;
        }
        let lib = state::get().library.read().unwrap().clone();
        let smart = id.strip_prefix("smart:");
        let editable = id != "likes" && id != "offline" && smart.is_none();
        // (song, added at, its item id)
        let (title, eyebrow, description, cover, rows): (String, &str, Option<String>, Option<String>, Vec<(Track, String, String)>) = if let Some(kind) = smart {
            let liked: Vec<Track> = lib.likes.iter().map(|l| l.track.clone()).collect();
            let (t, desc) = crate::stats::SMART.iter().find(|s| s.0 == kind).map(|s| (s.1, s.2)).unwrap_or(("", ""));
            let tracks = crate::stats::smart(kind, &liked, 100);
            (crate::i18n::t(t), "Собрано для вас", Some(crate::i18n::t(desc)), tracks.first().and_then(artwork_of), tracks.into_iter().map(|t| { let id = t.id.clone(); (t, String::new(), id) }).collect())
        } else if id == "offline" {
            let tracks = self.saved_tracks();
            (crate::i18n::t("Скачанное"), "Плейлист", None, None, tracks.into_iter().map(|t| { let id = t.id.clone(); (t, String::new(), id) }).collect())
        } else if id == "likes" {
            (crate::i18n::t("Понравившиеся"), "Ваши лайки", None, None, lib.likes.iter().map(|l| (l.track.clone(), l.added_at.clone(), l.track.id.clone())).collect())
        } else if let Some(p) = lib.playlists.iter().find(|p| p.id == id) {
            let cover = p.icon.clone().filter(|i| !i.is_empty()).or_else(|| p.items.first().and_then(|i| artwork_of(&i.track)));
            (p.title.clone(), "Плейлист", p.description.clone().filter(|d| !d.is_empty()), cover, p.items.iter().map(|i| (i.track.clone(), i.added_at.clone(), i.id.clone())).collect())
        } else {
            App::muted(ui, "Плейлист не найден.");
            return;
        };
        let dated = rows.iter().any(|r| !r.1.is_empty());
        let created = lib.playlists.iter().find(|p| p.id == id).map(|p| p.created_at.clone());
        // how much of this list is saved for offline
        let saved: std::collections::HashSet<String> = if smart.is_none() && id != "offline" { self.downloads.saved().into_iter().map(|e| e.id).collect() } else { Default::default() };
        let (off_total, off_kept) = if smart.is_none() && id != "offline" { crate::keep::status(id, &saved) } else { (0, 0) };

        // the order and the filter
        let (key, asc) = track_sort();
        let filter = self.pages.playlist_filter.clone();
        let f = crate::player::matching::simplify(&filter);
        let filtered: Vec<(Track, String, String)> = if f.is_empty() { rows.clone() } else { rows.iter().filter(|(t, _, _)| crate::player::matching::simplify(&format!("{} {}", t.title, t.artist_line())).contains(&f)).cloned().collect() };
        let shown = sort_rows(&filtered, key, asc);
        let tracks: Vec<Track> = shown.iter().map(|r| r.0.clone()).collect();
        // drag to reorder: your own lists in their own order, nothing filtered out
        let can_reorder = (editable || id == "likes") && f.is_empty() && key == SortKey::Custom && asc;
        let source = match smart {
            Some(kind) => PlaySource { page: "smart".into(), id: Some(kind.to_string()), title: None, cover: None, kind: None },
            None => PlaySource { page: "playlist".into(), id: Some(id.to_string()), title: None, cover: None, kind: None },
        };

        // .pl-hero: the cover (210), 28 to the text, all at the bottom
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let art = 210.0;
            // PlaylistCover at 210: the likes' gradient, your own picture or the first four covers
            let (rect, _) = ui.allocate_exact_size(vec2(art, art), egui::Sense::hover());
            let all: Vec<Track> = rows.iter().take(40).map(|r| r.0.clone()).collect();
            let icon = lib.playlists.iter().find(|p| p.id == id).and_then(|p| p.icon.clone());
            let pic = if id == "likes" { covers::Cover::Liked } else { covers::Cover::Playlist { icon: icon.as_deref(), tracks: &all } };
            covers::paint(ui, &mut self.images, &pic, rect, false);
            let _ = &cover;
            ui.add_space(28.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                // the text sits at the bottom, by the cover's lower edge
                let lines = 4.0 + if description.is_some() { 1.0 } else { 0.0 };
                ui.add_space((art - (lines * 26.0 + 56.0)).max(0.0));
                ui.label(egui::RichText::new(crate::i18n::t(eyebrow).to_uppercase()).font(theme::medium(12.0)).color(theme::ink3()).extra_letter_spacing(1.0));
                ui.label(egui::RichText::new(&title).font(theme::heavy(46.0)).color(INK));
                if let Some(d) = &description {
                    App::muted(ui, d);
                }
                let mut meta = crate::i18n::tracks(rows.len());
                if let Some(c) = &created {
                    meta.push_str(&format!(" · {} {}", crate::i18n::t("создан"), date(c)));
                }
                if off_kept > 0 {
                    meta.push_str(&format!(" · {} {off_kept} {} {off_total}", crate::i18n::t("скачано"), crate::i18n::t("из")));
                }
                ui.label(egui::RichText::new(meta).font(theme::body(13.0)).color(theme::ink2()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::btn(ui, Some("play"), "Слушать", true).clicked() && !tracks.is_empty() {
                        self.player.play_queue(tracks.clone(), 0, Some(source.clone()));
                    }
                    if widgets::btn(ui, Some("shuffle"), "Вперемешку", false).clicked() && !tracks.is_empty() {
                        let shuffled = crate::player::queue::shuffled(&tracks, None);
                        self.player.play_queue(shuffled, 0, Some(source.clone()));
                    }
                    if off_total > 0 {
                        let all = off_kept == off_total;
                        let r = widgets::round_button(ui, if all { "@offline" } else { ic::DOWNLOAD }, 42.0, widgets::Round::Soft, all, if all { "Всё скачано · настроить" } else { "Скачать для офлайн" });
                        if r.clicked() {
                            self.dialog = Some(Dialog::Download { picked: vec![id.to_string()], follow: true });
                        }
                    }
                    if smart.is_none() && id != "offline" {
                        let r = widgets::round_button(ui, ic::MORE, 42.0, widgets::Round::Soft, false, "");
                        if r.clicked() {
                            self.pages.playlist_menu = !self.pages.playlist_menu;
                        }
                        self.playlist_more(ui, id, r.rect, &title, &rows, off_kept > 0 || crate::keep::get().lists.iter().any(|l| l == id), off_total > off_kept);
                    }
                });
            });
        });
        ui.add_space(30.0);

        // .pl-tools: the filter, the order, "Вернуть свой порядок"
        let mut filter_now = std::mem::take(&mut self.pages.playlist_filter);
        let mut sort_rect = Rect::NOTHING;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            widgets::filter_input(ui, &mut filter_now, "Найти в плейлисте", 260.0);
            let label = format!("{}{}", crate::i18n::t(key.label()), if key == SortKey::Custom && asc { String::new() } else if asc { " ↑".into() } else { " ↓".into() });
            let r = widgets::btn(ui, Some("sort"), &label, false);
            sort_rect = r.rect;
            if r.clicked() {
                self.pages.sort_menu = !self.pages.sort_menu;
            }
            if can_reorder && rows.len() > 1 {
                ui.label(egui::RichText::new(crate::i18n::t("Перетаскивайте треки, чтобы поменять порядок")).font(theme::body(12.5)).color(theme::ink3()));
            }
            if key != SortKey::Custom {
                let reset = ui.add(egui::Label::new(egui::RichText::new(crate::i18n::t("Вернуть свой порядок")).font(theme::body(12.5)).color(theme::ink3())).sense(egui::Sense::click()));
                if reset.clicked() {
                    set_track_sort(SortKey::Custom, true);
                }
            }
        });
        self.pages.playlist_filter = filter_now;
        let entries: Vec<widgets::MenuEntry> = SORT_KEYS
            .iter()
            .map(|(k, _, label)| {
                let arrow = if *k == key { if asc { " ↑" } else { " ↓" } } else { "" };
                widgets::MenuEntry { icon: (*k == key).then_some("check"), label: format!("{}{arrow}", crate::i18n::t(label)), danger: false }
            })
            .collect();
        let mut open = self.pages.sort_menu;
        if let Some(i) = widgets::drop_menu(ui, "sort-menu", sort_rect, &mut open, &entries) {
            press_sort(SORT_KEYS[i].0);
        }
        self.pages.sort_menu = open;
        ui.add_space(16.0);

        if rows.is_empty() {
            App::muted(ui, if id == "likes" { "Нажмите ♥ у любого трека, и он появится здесь." } else { "Пока пусто. Правый клик по треку → «Добавить в плейлист»." });
            return;
        }
        // .track-head: "Трек", "Добавлен", "Время" (pressing one sorts by it)
        if dated {
            let arrow = |k: SortKey| if key == k { if asc { " ↑" } else { " ↓" } } else { "" };
            let labels = [format!("{}{}", crate::i18n::t("Трек"), arrow(SortKey::Title)), format!("{}{}", crate::i18n::t("Добавлен"), arrow(SortKey::Added)), format!("{}{}", crate::i18n::t("Время"), arrow(SortKey::Duration))];
            if let Some(i) = widgets::track_head(ui, [labels[0].as_str(), labels[1].as_str(), labels[2].as_str()]) {
                press_sort([SortKey::Title, SortKey::Added, SortKey::Duration][i]);
            }
        }
        let shown_items: Vec<String> = shown.iter().map(|r| r.2.clone()).collect();
        let added: Vec<String> = shown.iter().map(|r| ago(&r.1)).collect();
        let extra = move |i: usize| added.get(i).cloned().filter(|s| !s.is_empty());
        let menu_items = if editable { Some((id, shown_items.as_slice())) } else { None };
        if can_reorder {
            if let Some((from, to)) = self.track_list_reorder(ui, &tracks, Some(source), menu_items, &extra, dated) {
                // the rows are the list's own order here: no filter, the custom order up
                let (track_id, item_id) = (shown[from].0.id.clone(), shown[from].2.clone());
                let pid = id.to_string();
                state::get().update_library(|l| if pid == "likes" { l.move_like(&track_id, to) } else { l.move_in_playlist(&pid, &item_id, to) });
            }
        } else {
            self.track_list(ui, &tracks, Some(source), menu_items, &extra, dated);
        }
    }

    /// the playlist's "…" (as the page's menu): share the code, offline, cover, rename, delete
    #[allow(clippy::too_many_arguments)]
    fn playlist_more(&mut self, ui: &mut Ui, id: &str, anchor: Rect, title: &str, rows: &[(Track, String, String)], has_offline: bool, can_download: bool) {
        let is_likes = id == "likes";
        let lib = state::get().library.read().unwrap().clone();
        let playlist = lib.playlists.iter().find(|p| p.id == id).cloned();
        let mut entries: Vec<(widgets::MenuEntry, &str)> = Vec::new();
        let e = |icon: &'static str, label: &str, danger: bool| widgets::MenuEntry { icon: Some(icon), label: crate::i18n::t(label), danger };
        if is_likes {
            if !rows.is_empty() {
                entries.push((e("share", "Скопировать код", false), "share"));
            }
            entries.push((e("plus", "Вставить код или ссылку", false), "paste"));
        } else {
            entries.push((e("share", "Поделиться кодом", false), "share"));
        }
        if can_download {
            entries.push((e("download", "Скачать для офлайн", false), "download"));
        }
        if has_offline {
            entries.push((e("trash", "Удалить скачанное", true), "unkeep"));
        }
        if let Some(p) = &playlist {
            entries.push((e("image", "Сменить обложку", false), "cover"));
            if p.icon.as_deref().is_some_and(|i| !i.is_empty()) {
                entries.push((widgets::MenuEntry { icon: None, label: crate::i18n::t("Убрать обложку"), danger: false }, "uncover"));
            }
            entries.push((e("pencil", "Переименовать", false), "rename"));
            entries.push((e("trash", "Удалить плейлист", true), "delete"));
        }
        let list: Vec<widgets::MenuEntry> = entries.iter().map(|(m, _)| widgets::MenuEntry { icon: m.icon, label: m.label.clone(), danger: m.danger }).collect();
        let mut open = self.pages.playlist_menu;
        let chosen = widgets::drop_menu(ui, "playlist-menu", anchor, &mut open, &list).map(|i| entries[i].1);
        self.pages.playlist_menu = open;
        let tracks: Vec<Track> = rows.iter().map(|r| r.0.clone()).collect();
        match chosen {
            Some("share") => {
                let code = crate::import::encode_code(&crate::import::Shared {
                    title: if is_likes { "Понравившиеся".into() } else { title.to_string() },
                    description: playlist.as_ref().and_then(|p| p.description.clone()),
                    icon: playlist.as_ref().and_then(|p| p.icon.clone()),
                    tracks,
                    likes: is_likes,
                });
                ui.ctx().copy_text(code);
                self.toast(if is_likes { "Код «Понравившихся» скопирован — вставьте его на другом устройстве или отправьте другу" } else { "Код плейлиста скопирован — отправьте его другу" });
            }
            Some("paste") => self.dialog = Some(Dialog::Import { text: String::new(), likes: true }),
            Some("download") => {
                crate::keep::add(&[id.to_string()]);
                let n = self.downloads.add(&crate::keep::scope_tracks(id));
                if n > 0 {
                    self.toast(&format!("{} {n}", crate::i18n::t("Скачиваю:")));
                }
            }
            Some("unkeep") => {
                crate::keep::remove(&[id.to_string()]);
                let saved: std::collections::HashSet<String> = self.downloads.saved().into_iter().map(|e| e.id).collect();
                // a song that another list still keeps offline stays
                let others: std::collections::HashSet<String> = crate::keep::tracks_of(&crate::keep::get().lists).into_iter().map(|t| t.id).collect();
                let ids: Vec<String> = crate::keep::scope_tracks(id).into_iter().filter(|t| saved.contains(&t.id) && !others.contains(&t.id)).map(|t| t.id).collect();
                self.downloads.remove(&ids);
                self.toast(&if ids.is_empty() { crate::i18n::t("Эти песни нужны другим спискам, оставил") } else { format!("{} {}", crate::i18n::t("Удалено копий:"), ids.len()) });
            }
            Some("cover") => {
                if let Some(path) = crate::dialogs::open_file("Картинки", "png;*.jpg;*.jpeg;*.webp;*.bmp;*.gif") {
                    match square_data_url(&path, 384) {
                        Some(url) => {
                            let i = id.to_string();
                            state::get().update_library(|l| {
                                if let Some(p) = l.playlist_mut(&i) {
                                    p.icon = Some(url);
                                }
                            });
                        }
                        None => self.toast("Не получилось открыть картинку"),
                    }
                }
            }
            Some("uncover") => {
                let i = id.to_string();
                state::get().update_library(|l| {
                    if let Some(p) = l.playlist_mut(&i) {
                        p.icon = Some(String::new());
                    }
                });
            }
            Some("rename") => {
                let d = playlist.as_ref().and_then(|p| p.description.clone()).unwrap_or_default();
                self.dialog = Some(Dialog::Rename { id: id.to_string(), title: title.to_string(), description: d });
            }
            Some("delete") => self.dialog = Some(Dialog::ConfirmDelete { id: id.to_string(), title: title.to_string() }),
            _ => {}
        }
    }
}

/// .view-toggle: "Крупные значки" and "Списком" in a pill; Some(list) when one was pressed
fn view_toggle(ui: &mut Ui, list: bool) -> Option<bool> {
    let (tray, _) = ui.allocate_exact_size(vec2(3.0 + 34.0 + 2.0 + 34.0 + 3.0, 40.0), egui::Sense::hover());
    ui.painter().rect_filled(tray, 20, covers::rgba(0xeef7f4, 0.08));
    let mut out = None;
    for (i, (icon, tip)) in [("gridView", "Крупные значки"), ("listView", "Списком")].into_iter().enumerate() {
        let b = Rect::from_min_size(pos2(tray.left() + 3.0 + i as f32 * 36.0, tray.top() + 3.0), vec2(34.0, 34.0));
        let r = ui.interact(b, ui.scope_id().with(("view", i)), egui::Sense::click()).on_hover_text(crate::i18n::t(tip));
        let on = (i == 1) == list;
        if on {
            ui.painter().circle_filled(b.center(), 17.0, INK);
        }
        let color = if on { theme::GROUND } else if r.hovered() { INK } else { theme::ink2() };
        crate::ui::icons::paint(ui.painter(), icon, Rect::from_center_size(b.center(), vec2(17.0, 17.0)), color);
        if r.clicked() && !on {
            out = Some(i == 1);
        }
    }
    out
}

/// a picture as the page makes a cover (fileToSquareDataUrl): the middle square, `size` px
/// (384 for a playlist, 600 for a file's own), JPEG 86 %
pub fn square_data_url(path: &std::path::Path, size: u32) -> Option<String> {
    let img = image::open(path).ok()?;
    let side = img.width().min(img.height());
    let cut = img.crop_imm((img.width() - side) / 2, (img.height() - side) / 2, side, side).resize_exact(size, size, image::imageops::FilterType::Lanczos3).into_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 86).encode_image(&cut).ok()?;
    use base64::Engine;
    Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
}
