//! Библиотека: "Понравившиеся", your files and your playlists as cards (src/pages/LibraryPage.tsx),
//! and a playlist of yours with its songs (PlaylistPage.tsx).

use egui::{pos2, vec2, Align2, Rect, Ui};

use crate::app::{App, Dialog, Page};
use crate::model::Track;
use crate::state;
use crate::store::state::PlaySource;
use crate::ui::images::artwork_of;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

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
    pub fn library_page(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(0.0);
            ui.label(egui::RichText::new(crate::i18n::t("Библиотека")).font(theme::heavy(40.0)).color(INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if widgets::pill_button(ui, Some(ic::ADD), "Новый плейлист", true).clicked() {
                    self.dialog = Some(Dialog::NewPlaylist { title: String::new(), add: vec![] });
                }
                if widgets::pill_button(ui, Some(ic::LINK), "Импорт по ссылке или коду", false).clicked() {
                    self.dialog = Some(Dialog::Import { text: String::new(), likes: false });
                }
            });
        });
        ui.add_space(12.0);
        let lib = state::get().library.read().unwrap().clone();
        let files = self.library.list();
        let w = 172.0;
        egui::ScrollArea::vertical().id_salt("library").auto_shrink([false, false]).show(ui, |ui| {
            let per_row = (((ui.available_width() - 8.0) + 16.0) / (w + 16.0)).floor().max(1.0) as usize;
            let mut cards: Vec<(Option<String>, String, String, Page, &str)> = vec![
                (None, crate::i18n::t("Понравившиеся"), crate::i18n::tracks(lib.likes.len()), Page::Playlist("likes".into()), ic::HEART_FILL),
                (None, crate::i18n::t("Мои файлы"), crate::i18n::t("MP3, FLAC и другие с этого компьютера"), Page::Files, ic::FOLDER),
            ];
            let saved = self.downloads.saved().len();
            if saved > 0 {
                cards.push((None, crate::i18n::t("Скачанное"), crate::i18n::tracks(saved), Page::Playlist("offline".into()), ic::DOWNLOAD));
            }
            let _ = files.len();
            for p in &lib.playlists {
                let cover = p.icon.clone().or_else(|| p.items.first().and_then(|i| artwork_of(&i.track)));
                cards.push((cover, p.title.clone(), format!("{} · {} {}", crate::i18n::tracks(p.items.len()), crate::i18n::t("создан"), date(&p.created_at)), Page::Playlist(p.id.clone()), ic::NOTE));
            }
            for row in cards.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    for (cover, title, sub, page, icon) in row {
                        let r = if cover.is_some() {
                            widgets::card(ui, &mut self.images, cover.as_deref(), title, sub, w, false)
                        } else {
                            // likes and files: a coloured card with a sign
                            let (rect, r) = ui.allocate_exact_size(vec2(w, w + 46.0), egui::Sense::click());
                            let art = Rect::from_min_size(rect.min, vec2(w, w));
                            let (a, b) = if *icon == ic::HEART_FILL { (egui::Color32::from_rgb(92, 214, 172), egui::Color32::from_rgb(40, 120, 140)) } else { (egui::Color32::from_rgb(120, 140, 230), egui::Color32::from_rgb(70, 60, 150)) };
                            gradient(ui, art, a, b);
                            crate::ui::theme::paint_icon(&ui.painter(), art.center(), Align2::CENTER_CENTER, *icon, 34.0, egui::Color32::from_rgb(10, 30, 30));
                            let g = widgets::clipped(ui, title, theme::medium(13.0), INK, w);
                            ui.painter().galley(pos2(rect.left(), art.bottom() + 8.0), g, INK);
                            let g = widgets::clipped(ui, sub, theme::body(12.0), theme::ink2(), w);
                            ui.painter().galley(pos2(rect.left(), art.bottom() + 26.0), g, theme::ink2());
                            r
                        };
                        if r.clicked() {
                            self.go(page.clone());
                        }
                    }
                });
                ui.add_space(14.0);
            }
        });
    }

    /// A list of songs of yours (PlaylistPage.tsx): the cover, what it is, its name and
    /// description, how many songs and when it was made, "Слушать", "Вперемешку", downloading
    /// and "…"; then the filter, the order, and the songs with the date each was added.
    pub fn playlist(&mut self, ui: &mut Ui, id: &str) {
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
        let source = PlaySource { page: "playlist".into(), id: Some(id.to_string()), title: None, cover: None, kind: None };

        // .pl-hero: the cover (210), 28 to the text, all at the bottom
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let art = 210.0;
            if id == "likes" {
                let (rect, _) = ui.allocate_exact_size(vec2(art, art), egui::Sense::hover());
                gradient(ui, rect, egui::Color32::from_rgb(92, 214, 172), egui::Color32::from_rgb(40, 120, 140));
                crate::ui::icons::paint(ui.painter(), "heartFill", Rect::from_center_size(rect.center(), vec2(72.0, 72.0)), egui::Color32::from_rgb(10, 30, 30));
            } else {
                widgets::cover(ui, &mut self.images, cover.as_deref(), art, 18);
            }
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
        self.track_list(ui, &tracks, Some(source), menu_items, &extra, dated);
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

/// a rounded card filled with a diagonal blend of two colours (the "Понравившиеся" and files cards)
pub fn gradient(ui: &Ui, rect: Rect, a: egui::Color32, b: egui::Color32) {
    let r = (theme::R_CARD as f32).min(rect.width() / 2.0);
    let color_at = |p: egui::Pos2| {
        let t = (((p.x - rect.left()) / rect.width() + (p.y - rect.top()) / rect.height()) / 2.0).clamp(0.0, 1.0);
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
        egui::Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.center(), color_at(rect.center()));
    // the outline: four quarter circles
    let corners = [(rect.right_top() + vec2(-r, r), -90.0f32), (rect.right_bottom() + vec2(-r, -r), 0.0), (rect.left_bottom() + vec2(r, -r), 90.0), (rect.left_top() + vec2(r, r), 180.0)];
    for (c, start) in corners {
        for k in 0..=8 {
            let ang = (start + 90.0 * k as f32 / 8.0).to_radians();
            let p = c + vec2(r * ang.cos(), r * ang.sin());
            mesh.colored_vertex(p, color_at(p));
        }
    }
    let n = mesh.vertices.len() as u32;
    for i in 1..n {
        let next = if i + 1 < n { i + 1 } else { 1 };
        mesh.add_triangle(0, i, next);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

