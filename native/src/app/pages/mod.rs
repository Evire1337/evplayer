//! The pages, each in its own file, and what several of them share: the menu of a song, the
//! dialogs, rows of cards, lists of songs.

mod artist;
pub mod covers;
mod files;
mod home;
pub mod jam;
mod library;
pub mod now;
mod search;
pub mod services;
mod settings;
pub mod kit;
pub mod sound;

use egui::{pos2, vec2, Rect, Ui};

use super::task::Load;
use super::{App, Dialog, Menu, Page};
use crate::model::{CollectionSummary, Track};
use crate::state;
use crate::store::state::PlaySource;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

pub use search::SearchState;

/// what the pages keep while the window is open (dropped when it sleeps)
#[derive(Default)]
pub struct Pages {
    pub search: SearchState,
    pub home: home::HomeState,
    pub artist: artist::ArtistState,
    pub collection: artist::CollectionState,
    pub playlist_filter: String,
    /// Мои файлы: "Поиск по файлам"
    pub files_filter: String,
    pub settings_tab: usize,
    pub sound_open: bool,
    pub volume_open: bool,
    pub show_queue: Option<bool>,
    /// Настройки → Данные: when the sizes were looked at, and library / statistics / cache bytes
    pub data_usage: Option<(std::time::Instant, [u64; 3])>,
    /// the offline menu of a playlist is open
    pub offline_menu: bool,
    /// clicks on the version in О программе (seven turn the diagnostics on)
    pub version_clicks: Vec<std::time::Instant>,
    /// the diagnostics' measurements while their tab is open
    pub probe: Option<crate::diagnostics::Probe>,
    /// Настройки → Сервисы и импорт
    pub services: services::ServicesState,
    /// «Слушать вместе»
    pub jam: jam::JamUi,
    /// a playlist's "…" and its order menu are open
    pub playlist_menu: bool,
    pub sort_menu: bool,
    /// Настройки → Клавиши: the scope shown, the filter, and the shortcut waiting for a key
    pub keys_scope: usize,
    pub keys_query: String,
    pub keys_listening: Option<&'static str>,
    /// Настройки → Внешний вид: what each "Не сохранять" item takes, looked at every few seconds
    pub dont_keep: Option<(std::time::Instant, [u64; 5])>,
    /// Настройки → Discord: the setup steps are open
    pub discord_setup: Option<bool>,
    /// Настройки → Звук: the band chosen on the graph, the preset whose menu is open, and the
    /// name / code dialog (save, rename, paste)
    pub eq_band: Option<String>,
    pub eq_menu: Option<(String, egui::Pos2)>,
    pub eq_dialog: Option<sound::PresetDialog>,
    /// «Сейчас играет»: when the lyrics were last scrolled by hand, and where they are scrolled
    pub lyrics_hand: Option<std::time::Instant>,
    pub lyrics_y: f32,
    /// a row being dragged to a new place (useReorder)
    pub drag: Option<Drag>,
    /// «Далее» was last opened at this place and song (it opens at the current song again when another plays)
    pub queue_at: Option<(usize, String)>,
}

/// a drag in a list: which list, the row taken, the row under the pointer and which half
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub list: &'static str,
    pub from: usize,
    pub over: usize,
    pub before: bool,
}

/// where the dragged item ends up in the list after the move (finalIndex)
pub fn final_index(from: usize, over: usize, before: bool) -> usize {
    let to = over + if before { 0 } else { 1 };
    if from < to { to - 1 } else { to }
}

impl Pages {
    /// One row of a list that can be reordered, after it was laid out: a drag starts on it,
    /// the pointer over it marks where the row would land, and the accent line shows it
    /// (2 px, 8 in from the sides, as .drop-before / .drop-after).
    pub fn reorder_row(&mut self, ui: &Ui, list: &'static str, i: usize, r: &egui::Response) {
        if r.drag_started() {
            self.drag = Some(Drag { list, from: i, over: i, before: true });
        }
        let Some(d) = self.drag.as_mut().filter(|d| d.list == list) else { return };
        if let Some(p) = ui.ctx().pointer_latest_pos() {
            if p.y >= r.rect.top() && p.y < r.rect.bottom() {
                d.over = i;
                d.before = p.y < r.rect.center().y;
            }
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if d.over == i && d.from != i {
            let y = if d.before { r.rect.top() } else { r.rect.bottom() };
            ui.painter().rect_filled(Rect::from_min_max(pos2(r.rect.left() + 8.0, y - 1.0), pos2(r.rect.right() - 8.0, y + 1.0)), 2, theme::ACCENT);
        }
    }

    /// After the list: the move to make when the pointer was let go over another place, or
    /// nothing; Escape gives the drag up
    pub fn reorder_end(&mut self, ctx: &egui::Context, list: &'static str) -> Option<(usize, usize)> {
        let d = self.drag.filter(|d| d.list == list)?;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.drag = None;
            return None;
        }
        if ctx.input(|i| i.pointer.any_down()) {
            return None;
        }
        self.drag = None;
        let to = final_index(d.from, d.over, d.before);
        (d.over != d.from && to != d.from).then_some((d.from, to))
    }
}

impl App {
    /// a list of songs; pressing one plays the list from it
    /// `dated`: the notes are the dates songs were added, in their own column
    pub fn track_list(&mut self, ui: &mut Ui, tracks: &[Track], source: Option<PlaySource>, playlist: Option<(&str, &[String])>, extra: &dyn Fn(usize) -> Option<String>, dated: bool) {
        self.track_list_full(ui, tracks, source, playlist, extra, dated, false, false);
    }

    /// a list of your own whose songs can be dragged to a new place: the move, when one was made
    #[allow(clippy::too_many_arguments)]
    pub fn track_list_reorder(&mut self, ui: &mut Ui, tracks: &[Track], source: Option<PlaySource>, playlist: Option<(&str, &[String])>, extra: &dyn Fn(usize) -> Option<String>, dated: bool) -> Option<(usize, usize)> {
        self.track_list_full(ui, tracks, source, playlist, extra, dated, false, true)
    }

    /// a list whose songs show their service (SC, YT, Файл) before the artist (showSource)
    pub fn track_list_badged(&mut self, ui: &mut Ui, tracks: &[Track], source: Option<PlaySource>) {
        self.track_list_full(ui, tracks, source, None, &|_| None, false, true, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn track_list_full(&mut self, ui: &mut Ui, tracks: &[Track], source: Option<PlaySource>, playlist: Option<(&str, &[String])>, extra: &dyn Fn(usize) -> Option<String>, dated: bool, badge: bool, reorder: bool) -> Option<(usize, usize)> {
        let current = self.view.current().map(|t| t.id.clone());
        // the current row shows pause over its cover while it plays
        let playing = self.view.status == crate::player::Status::Playing;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("evplayer-playing"), playing));
        let double = state::get().settings.read().unwrap().double_click_to_play;
        let lib_likes: std::collections::HashSet<String> = state::get().library.read().unwrap().likes.iter().map(|l| l.track.id.clone()).collect();
        self.mark_kept();
        // only the rows on screen are laid out: a playlist of thousands costs as much as a short one
        let row_h = 62.0 + ui.spacing().item_spacing.y;
        widgets::set_row_drag(reorder);
        egui::ScrollArea::vertical().id_salt("tracks").auto_shrink([false, false]).show_rows(ui, row_h - ui.spacing().item_spacing.y, tracks.len(), |ui, range| {
            for i in range {
                let t = &tracks[i];
                let e = extra(i);
                let out = widgets::track_row_full(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), lib_likes.contains(&t.id), e.as_deref(), None, dated, badge);
                if reorder {
                    self.pages.reorder_row(ui, "tracks", i, &out.response);
                }
                if (out.clicked && !double) || out.double {
                    self.play_list(tracks, i, source.clone());
                }
                if out.like {
                    self.toggle_like(t);
                }
                if out.menu {
                    let at = ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center());
                    let item = playlist.and_then(|(pid, items)| items.get(i).map(|item| (pid.to_string(), item.clone())));
                    self.menu = Some(Menu { track: t.clone(), at, playlist_item: item, queue_index: None, sub: false, files_page: false });
                }
            }
        });
        widgets::set_row_drag(false);
        if reorder { self.pages.reorder_end(ui.ctx(), "tracks") } else { None }
    }

    /// the offline marks of the song rows drawn next
    pub fn mark_kept(&self) {
        let saved = self.downloads.saved().into_iter().map(|e| e.id).collect();
        let s = self.downloads.status();
        widgets::set_kept(saved, s.running.into_keys().collect());
    }

    /// a row of cards that scrolls sideways
    pub fn card_row(&mut self, ui: &mut Ui, id: &str, tracks: &[Track]) {
        let w = 172.0;
        egui::ScrollArea::horizontal().id_salt(id).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for (i, t) in tracks.iter().enumerate() {
                    let src = crate::ui::images::artwork_of(t);
                    let r = widgets::card(ui, &mut self.images, src.as_deref(), &t.title, &t.artist_line(), w, false);
                    if r.clicked() {
                        self.play_list(tracks, i, None);
                    }
                    if r.secondary_clicked() {
                        self.menu = Some(Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(r.rect.center()), playlist_item: None, queue_index: None, sub: false, files_page: false });
                    }
                }
            });
        });
    }

    /// a row of albums or playlists that scrolls sideways
    pub fn collection_row(&mut self, ui: &mut Ui, id: &str, list: &[CollectionSummary]) {
        let w = 172.0;
        egui::ScrollArea::horizontal().id_salt(id).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for c in list {
                    if widgets::card(ui, &mut self.images, c.artwork.as_deref(), &c.title, &c.subtitle, w, false).clicked() {
                        self.go(Page::Collection(c.id.clone()));
                    }
                }
            });
        });
    }

    pub fn page_title(&mut self, ui: &mut Ui, text: &str) {
        ui.add_space(18.0);
        ui.label(egui::RichText::new(crate::i18n::t(text)).font(theme::heavy(40.0)).color(INK));
        ui.add_space(8.0);
    }

    /// The menu of a song (trackMenu.tsx and ContextMenu.tsx): the same entries in the same
    /// order, "Добавить в плейлист" opening its list at the side, kept on screen, scrolling
    /// when longer than 70% of the window. Closes on a press outside, Escape or leaving the window.
    pub fn track_menu(&mut self, ctx: &egui::Context) {
        let Some(m) = &self.menu else { return };
        let track = m.track.clone();
        let at = m.at;
        let item = m.playlist_item.clone();
        let queue_index = m.queue_index;
        let mut sub_open = m.sub;
        let files_page = m.files_page;
        #[derive(Clone, Copy, PartialEq)]
        enum A {
            Next,
            Queue,
            Radio,
            Like,
            AddTo,
            Stats,
            Artist,
            Download,
            Forget,
            Copy,
            Open,
            Reveal,
            Unqueue,
            Unlist,
            Tags,
            Unfile,
        }
        let together = crate::jam::engine::get().is_some_and(|j| j.view().status == crate::jam::engine::JamStatus::Live);
        let liked = self.is_liked(&track.id);
        let mut items: Vec<(&str, String, bool, A)> = vec![
            ("next", crate::i18n::t("Играть следующим"), false, A::Next),
            (if together { "jam" } else { "queue" }, crate::i18n::t(if together { "В общую очередь" } else { "Добавить в очередь" }), false, A::Queue),
            ("radio", crate::i18n::t("Радио по треку"), false, A::Radio),
            (if liked { "heartFill" } else { "heart" }, crate::i18n::t(if liked { "Убрать из понравившихся" } else { "Нравится" }), false, A::Like),
            ("listPlus", crate::i18n::t("Добавить в плейлист"), false, A::AddTo),
            ("chart", crate::i18n::t("Статистика прослушиваний"), false, A::Stats),
        ];
        // the first artist, on a service or by name (artistTarget)
        let artist = track.artists.first().cloned();
        if let Some(a) = &artist {
            items.push(("user", format!("{}: {}", crate::i18n::t("Исполнитель"), a.name), false, A::Artist));
        }
        if track.source != crate::model::Source::Local {
            if self.downloads.is_saved(&track.id) {
                items.push(("trash", crate::i18n::t("Удалить скачанное"), false, A::Forget));
            } else {
                items.push(("download", crate::i18n::t("Скачать для офлайн"), false, A::Download));
            }
        }
        if track.permalink.is_some() {
            items.push(("link", crate::i18n::t("Скопировать ссылку"), false, A::Copy));
            items.push(("disc", crate::i18n::t(if track.source == crate::model::Source::Youtube { "Открыть на YouTube" } else { "Открыть на SoundCloud" }), false, A::Open));
        }
        if track.source == crate::model::Source::Local {
            items.push(("pencil", crate::i18n::t("Изменить теги и обложку"), false, A::Tags));
            items.push(("folder", crate::i18n::t("Показать в папке"), false, A::Reveal));
        }
        if queue_index.is_some() {
            items.push(("trash", crate::i18n::t("Убрать из очереди"), true, A::Unqueue));
        }
        if item.is_some() {
            items.push(("trash", crate::i18n::t("Убрать из этого плейлиста"), true, A::Unlist));
        }
        if files_page && track.source == crate::model::Source::Local {
            items.push(("trash", crate::i18n::t("Убрать из EvPlayer (файл останется)"), true, A::Unfile));
        }
        let lists: Vec<(String, String)> = state::get().library.read().unwrap().playlists.iter().map(|p| (p.id.clone(), p.title.clone())).collect();

        // .menu: 220 to 300 wide, padding 6, rounded 16, entries 13.5 px with 16 px icons
        let screen = ctx.content_rect();
        let width_of = |labels: &mut dyn Iterator<Item = String>, icons: bool, more: bool| -> f32 {
            let widest = ctx.fonts_mut(|f| labels.map(|l| f.layout_no_wrap(l, egui::FontId::new(13.5, theme::medium500()), INK).size().x).fold(0.0, f32::max));
            (widest + 20.0 + if icons { 26.0 } else { 0.0 } + if more { 20.0 } else { 0.0 } + 12.0).clamp(220.0, 300.0)
        };
        let main_w = width_of(&mut items.iter().map(|i| i.1.clone()), true, true);
        let max_h = screen.height() * 0.7;
        let row_h = 36.0;
        let menu_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(6, 18, 20, 235))
            .stroke(egui::Stroke::new(1.0, theme::edge()))
            .corner_radius(16)
            .inner_margin(6)
            .shadow(egui::Shadow { offset: [0, 18], blur: 50, spread: 0, color: egui::Color32::from_black_alpha(128) });
        // one entry; true when pressed; the row's rect comes back for the side list
        let entry = |ui: &mut Ui, icon: Option<&str>, text: &str, danger: bool, open: bool, more: bool, w: f32| -> (bool, bool, Rect) {
            let (rect, r) = ui.allocate_exact_size(vec2(w, row_h), egui::Sense::click());
            if r.hovered() || open {
                ui.painter().rect_filled(rect, 10, theme::hover());
            }
            let color = if danger { theme::DANGER } else { INK };
            let mut x = rect.left() + 10.0;
            if let Some(i) = icon {
                crate::ui::icons::paint(ui.painter(), i, Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0)), if danger { theme::DANGER } else { theme::ink2() });
                x += 26.0;
            }
            let tw = rect.right() - 10.0 - x - if more { 16.0 } else { 0.0 };
            let g = widgets::clipped(ui, text, egui::FontId::new(13.5, theme::medium500()), color, tw);
            ui.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, color);
            if more {
                ui.painter().text(pos2(rect.right() - 10.0, rect.center().y), egui::Align2::RIGHT_CENTER, "›", theme::body(15.0), theme::ink3());
            }
            (r.clicked(), r.hovered(), rect)
        };
        let mut chosen: Option<A> = None;
        let mut add_row: Option<Rect> = None;
        let main = egui::Area::new(egui::Id::unique("track-menu")).order(egui::Order::Foreground).fixed_pos(at).constrain_to(screen.shrink(10.0)).show(ctx, |ui| {
            menu_frame.show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                egui::ScrollArea::vertical().id_salt("track-menu-scroll").max_height(max_h).show(ui, |ui| {
                    for (icon, label, danger, a) in &items {
                        let more = *a == A::AddTo;
                        let (pressed, hovered, rect) = entry(ui, Some(icon), label, *danger, more && sub_open, more, main_w - 12.0);
                        if more {
                            add_row = Some(rect);
                            if hovered || pressed {
                                sub_open = true;
                            }
                        } else if hovered {
                            sub_open = false;
                        }
                        if pressed && !more {
                            chosen = Some(*a);
                        }
                    }
                });
            });
        });
        // the playlists at the side of "Добавить в плейлист": right of it, or left when there is no room
        let mut sub_rect = None;
        let mut add_to: Option<Option<(String, String)>> = None;
        if let (true, Some(row)) = (sub_open, add_row) {
            let sub_w = width_of(&mut std::iter::once(crate::i18n::t("Новый плейлист…")).chain(lists.iter().map(|l| l.1.clone())), true, false);
            let x = if row.right() + 4.0 + sub_w <= screen.right() - 10.0 { row.right() + 4.0 } else { (row.left() - 4.0 - sub_w).max(10.0) };
            let r = egui::Area::new(egui::Id::unique("track-menu-sub")).order(egui::Order::Foreground).fixed_pos(pos2(x, row.top() - 6.0)).constrain_to(screen.shrink(10.0)).show(ctx, |ui| {
                menu_frame.show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    egui::ScrollArea::vertical().id_salt("track-menu-sub-scroll").max_height(max_h).show(ui, |ui| {
                        if entry(ui, Some("plus"), &crate::i18n::t("Новый плейлист…"), false, false, false, sub_w - 12.0).0 {
                            add_to = Some(None);
                        }
                        for (id, title) in &lists {
                            if entry(ui, None, title, false, false, false, sub_w - 12.0).0 {
                                add_to = Some(Some((id.clone(), title.clone())));
                            }
                        }
                    });
                });
            });
            sub_rect = Some(r.response.rect);
        }
        let mut close = chosen.is_some() || add_to.is_some();
        match add_to {
            Some(None) => self.dialog = Some(Dialog::NewPlaylist { title: String::new(), description: String::new(), add: vec![track.clone()] }),
            Some(Some((id, title))) => {
                let n = state::get().update_library(|l| l.add_to_playlist(&id, std::slice::from_ref(&track)));
                self.toast(&if n > 0 { format!("{} «{title}»", crate::i18n::t("Добавлено в")) } else { format!("{} «{title}»", crate::i18n::t("Уже есть в")) });
            }
            None => {}
        }
        match chosen {
            Some(A::Next) => {
                self.player.add(track.clone(), true);
                self.toast("Будет следующим");
            }
            Some(A::Queue) => {
                self.player.add(track.clone(), false);
                self.toast("Добавлено в очередь");
            }
            Some(A::Radio) => self.player.start_radio(track.clone()),
            Some(A::Like) => self.toggle_like(&track),
            Some(A::Stats) => self.dialog = Some(Dialog::TrackStats { track: track.clone(), range: 1 }),
            Some(A::Artist) => {
                if let Some(a) = &artist {
                    self.go(Page::Artist(artist::artist_page_id(a)));
                }
            }
            Some(A::Download) => {
                self.download(std::slice::from_ref(&track));
                self.toast("Скачиваю для офлайн");
            }
            Some(A::Forget) => {
                self.downloads.remove(std::slice::from_ref(&track.id));
                self.toast("Копия для офлайн удалена");
            }
            Some(A::Copy) => {
                if let Some(link) = &track.permalink {
                    ctx.copy_text(link.clone());
                    self.toast("Ссылка скопирована");
                }
            }
            Some(A::Open) => {
                if let Some(link) = &track.permalink {
                    crate::dialogs::open_path(std::path::Path::new(link));
                }
            }
            Some(A::Reveal) => {
                if let Some(rec) = self.library.get(&track.id) {
                    crate::dialogs::reveal(std::path::Path::new(&rec.path));
                }
            }
            Some(A::Unqueue) => {
                if let Some(i) = queue_index {
                    self.player.remove(i);
                }
            }
            Some(A::Unlist) => {
                if let Some((pid, item_id)) = &item {
                    if pid == "likes" {
                        self.toggle_like(&track);
                    } else {
                        state::get().update_library(|l| l.remove_from_playlist(pid, item_id));
                    }
                }
            }
            Some(A::Tags) => self.open_tag_editor(&track),
            Some(A::Unfile) => self.library.remove(std::slice::from_ref(&track.id)),
            Some(A::AddTo) | None => {}
        }
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let inside = pointer.is_some_and(|p| main.response.rect.contains(p) || sub_rect.is_some_and(|r| r.contains(p)));
        let pressed_away = ctx.input(|i| i.pointer.any_pressed()) && !inside;
        let left_window = ctx.input(|i| !i.focused);
        if pressed_away || left_window || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if close {
            self.menu = None;
        } else if let Some(m) = self.menu.as_mut() {
            m.sub = sub_open;
        }
    }

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        // what is saved, for the offline dialog's counts
        let saved: std::collections::HashSet<String> = if matches!(self.dialog, Some(Dialog::Download { .. })) { self.downloads.saved().into_iter().map(|e| e.id).collect() } else { Default::default() };
        let Some(d) = &mut self.dialog else { return };
        let mut done = false;
        let mut apply: Option<Box<dyn FnOnce(&mut App)>> = None;
        let frame = egui::Frame::new().fill(egui::Color32::from_rgb(0x0c, 0x2c, 0x28)).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(20).inner_margin(22);
        let r = egui::Modal::new(egui::Id::unique("dialog")).frame(frame).backdrop_color(egui::Color32::from_black_alpha(140)).show(ctx, |ui| {
            ui.set_width(380.0);
            match d {
                Dialog::NewPlaylist { title, description, add } => {
                    ui.label(egui::RichText::new(crate::i18n::t("Новый плейлист")).font(theme::heavy(22.0)));
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Название")).font(theme::body(12.5)).color(theme::ink2()));
                    let r = ui.add(egui::TextEdit::singleline(title).char_limit(60).hint_text(crate::i18n::t("Например, «Ночная поездка»")).desired_width(f32::INFINITY));
                    if ui.memory(|m| m.focused().is_none()) {
                        r.request_focus();
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Описание (необязательно)")).font(theme::body(12.5)).color(theme::ink2()));
                    ui.add(egui::TextEdit::multiline(description).char_limit(300).desired_rows(2).desired_width(f32::INFINITY));
                    ui.add_space(14.0);
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::pill_button(ui, None, "Создать", true).clicked() || enter {
                            let (t, d, a) = (title.trim().to_string(), description.trim().to_string(), std::mem::take(add));
                            apply = Some(Box::new(move |app: &mut App| {
                                let t = if t.is_empty() { crate::i18n::t("Новый плейлист") } else { t };
                                let with_songs = !a.is_empty();
                                let id = state::get().update_library(|l| {
                                    let id = l.create_playlist(&t, (!d.is_empty()).then_some(d.as_str()), None);
                                    l.add_to_playlist(&id, &a);
                                    id
                                });
                                let name = state::get().library.read().unwrap().playlists.iter().find(|p| p.id == id).map(|p| p.title.clone()).unwrap_or_default();
                                app.toast(&format!("Плейлист «{name}» создан"));
                                // an empty one is opened, to fill it
                                if !with_songs {
                                    app.go(Page::Playlist(id));
                                }
                            }));
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::Rename { id, title, description } => {
                    ui.label(egui::RichText::new(crate::i18n::t("Изменить плейлист")).font(theme::heavy(22.0)));
                    ui.add_space(10.0);
                    ui.add(egui::TextEdit::singleline(title).hint_text(crate::i18n::t("Название")).desired_width(f32::INFINITY));
                    ui.add_space(6.0);
                    ui.add(egui::TextEdit::multiline(description).hint_text(crate::i18n::t("Описание")).desired_rows(3).desired_width(f32::INFINITY));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::pill_button(ui, None, "Сохранить", true).clicked() {
                            let (i, t, d) = (id.clone(), title.clone(), description.clone());
                            state::get().update_library(|l| l.rename_playlist(&i, &t, Some(&d)));
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::Import { text, likes, error } => {
                    ui.set_width(440.0);
                    ui.label(egui::RichText::new(crate::i18n::t(if *likes { "Добавить в понравившиеся" } else { "Импорт плейлиста" })).font(theme::heavy(22.0)));
                    ui.add_space(8.0);
                    let about = if *likes {
                        "Вставьте код EvPlayer (строка с EVP) — например, скопированный из чьих-то «Понравившихся», — или ссылку на плейлист. Треки добавятся к вашим лайкам, повторы пропускаются."
                    } else {
                        "Вставьте ссылку на плейлист, альбом или трек из Spotify, Яндекс Музыки, YouTube / YouTube Music, SoundCloud или Deezer, код EvPlayer (строка с EVP) — или просто список песен, по одной на строку: «Исполнитель — Название». Из ВКонтакте: откройте плейлист или «Мою музыку» на vk.com, выделите треки мышью, скопируйте и вставьте сюда."
                    };
                    ui.label(egui::RichText::new(crate::i18n::t(about)).font(theme::body(12.5)).color(theme::ink3()));
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Ссылка, код или список песен")).font(theme::body(12.5)).color(theme::ink2()));
                    let r = ui.add(egui::TextEdit::multiline(text).hint_text("https://open.spotify.com/playlist/…, EVP2.… или\nИсполнитель — Название\nИсполнитель — Название").desired_rows(6).desired_width(f32::INFINITY));
                    if ui.memory(|m| m.focused().is_none()) {
                        r.request_focus();
                    }
                    if r.changed() {
                        error.clear();
                    }
                    let value = text.trim().to_string();
                    let kind = crate::import::link_kind(&value);
                    if kind.is_none() && crate::import::looks_like_track_list(text) {
                        ui.add_space(4.0);
                        let n = crate::import::parse_track_list(text).len();
                        ui.label(egui::RichText::new(format!("{} {n}. {}", crate::i18n::t("В списке песен:"), crate::i18n::t("Их найдём на YouTube Music и SoundCloud — сколько нашлось, будет видно в окошке загрузки."))).font(theme::body(12.5)).color(theme::ink3()));
                    }
                    if !error.is_empty() {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(crate::i18n::t(error)).font(theme::body(12.5)).color(theme::DANGER));
                    }
                    ui.add_space(14.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::pill_button(ui, None, if *likes { "Добавить" } else { "Импортировать" }, true).clicked() && !value.is_empty() {
                            if kind.is_none() && !crate::import::looks_like_track_list(&value) {
                                *error = "Не похоже ни на ссылку, ни на код EvPlayer (он начинается с EVP), ни на список песен «Исполнитель — Название».".into();
                            } else if kind == Some(crate::import::Kind::Code) && !crate::import::extract_codes(&value).iter().any(|c| crate::import::decode_code(c).is_ok()) {
                                // a broken code is told here, before the dialog closes
                                *error = "Код повреждён или неполный. Скопируйте его целиком, начиная с EVP.".into();
                            } else {
                                let l = *likes;
                                apply = Some(Box::new(move |app: &mut App| app.start_import(value, if l { Some(true) } else { None })));
                                done = true;
                            }
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::Download { picked, follow } => {
                    ui.label(egui::RichText::new(crate::i18n::t("Скачать для офлайн")).font(theme::heavy(22.0)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Отметьте, что сохранить на этом компьютере: лайки, плейлисты или всё сразу. Скачанное играет без интернета.")).font(theme::body(12.5)).color(theme::ink2()));
                    ui.add_space(8.0);
                    let kept_lists = crate::keep::get().lists;
                    let rows: Vec<(String, usize, usize)> = crate::keep::all_scopes().into_iter().map(|s| {
                        let (total, kept) = crate::keep::status(&s, &saved);
                        (s, total, kept)
                    }).filter(|r| r.1 > 0).collect();
                    if rows.is_empty() {
                        ui.label(egui::RichText::new(crate::i18n::t("Пока нечего скачивать: добавьте треки из SoundCloud или YouTube в лайки или плейлист.")).color(theme::ink2()));
                    } else {
                        let all_on = rows.iter().all(|r| picked.contains(&r.0));
                        let mut all = all_on;
                        if ui.checkbox(&mut all, egui::RichText::new(crate::i18n::t("Всё сразу")).font(theme::medium(13.5))).changed() {
                            *picked = if all { rows.iter().map(|r| r.0.clone()).collect() } else { vec![] };
                        }
                        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for (scope, total, kept) in &rows {
                                let mut on = picked.contains(scope);
                                let mut note = String::new();
                                if kept == total {
                                    note.push_str(&crate::i18n::t("всё скачано · "));
                                } else if *kept > 0 {
                                    note.push_str(&format!("{} {} {} ", crate::i18n::t("скачано"), kept, crate::i18n::t("из")));
                                }
                                note.push_str(&crate::i18n::tracks(*total));
                                if kept_lists.contains(scope) {
                                    note.push_str(&crate::i18n::t(" · следит"));
                                }
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut on, egui::RichText::new(crate::keep::title_of(scope)).font(theme::medium(13.5))).changed() {
                                        if on {
                                            picked.push(scope.clone());
                                        } else {
                                            picked.retain(|p| p != scope);
                                        }
                                    }
                                    ui.label(egui::RichText::new(note).font(theme::body(12.0)).color(theme::ink3()));
                                });
                            }
                        });
                    }
                    ui.add_space(6.0);
                    if let Some(v) = widgets::toggle_row(ui, "Докачивать новые треки", "Что добавите в эти списки потом, скачается само", *follow, true) {
                        *follow = v;
                    }
                    ui.add_space(10.0);
                    let missing = crate::keep::tracks_of(picked).iter().filter(|t| !saved.contains(&t.id)).count();
                    ui.horizontal(|ui| {
                        let label = if picked.is_empty() { crate::i18n::t("Скачать") } else if missing > 0 { format!("{} {}", crate::i18n::t("Скачать"), crate::i18n::tracks(missing)) } else { crate::i18n::t("Сохранить выбор") };
                        if widgets::pill_button(ui, Some(ic::DOWNLOAD), &label, true).clicked() && !picked.is_empty() {
                            let (p, f) = (picked.clone(), *follow);
                            apply = Some(Box::new(move |app: &mut App| {
                                if f {
                                    crate::keep::add(&p);
                                }
                                let n = app.downloads.add(&crate::keep::tracks_of(&p));
                                app.toast(&if n > 0 { format!("{} {}", crate::i18n::t("Скачиваю для офлайн:"), crate::i18n::tracks(n)) } else { crate::i18n::t("Всё выбранное уже сохранено") });
                            }));
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::Restore(b) => {
                    ui.label(egui::RichText::new(crate::i18n::t("Восстановить из копии?")).font(theme::heavy(22.0)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(format!("{} {}.", crate::i18n::t("Всё, что сейчас есть в EvPlayer на этом компьютере, заменится содержимым копии от"), crate::backup::when(b))).color(theme::ink2()));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(crate::i18n::t("EvPlayer перезапустится.")).font(theme::body(12.5)).color(theme::ink3()));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::pill_button(ui, None, "Восстановить", true).clicked() {
                            crate::backup::restore(&state::get().kv, b);
                            crate::system::restart();
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::FullSync => {
                    ui.label(egui::RichText::new(crate::i18n::t("Включить полную синхронизацию?")).font(theme::heavy(22.0)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Все песни SoundCloud из «Понравившихся» будут отмечены лайком в вашем аккаунте, а плейлисты, созданные в EvPlayer, появятся там как приватные. Если убрать лайк или удалить такой плейлист в плеере, он исчезнет и на SoundCloud. Плейлисты, которые вы забрали с SoundCloud, и всё, что создано не в EvPlayer, не трогаются.")).color(theme::ink2()));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::pill_button(ui, None, "Включить", true).clicked() {
                            state::get().set_extra("soundcloudFullSync", serde_json::Value::Bool(true));
                            crate::services::scaccount::schedule(500);
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::TrackStats { track, range } => {
                    // TrackStatsDialog.tsx: the song, three counts, a chart and some facts
                    ui.set_width(596.0);
                    ui.spacing_mut().item_spacing.y = 16.0;
                    ui.label(egui::RichText::new(crate::i18n::t("Статистика трека")).font(theme::heavy(19.0)).color(INK));
                    let s = crate::stats::track_summary(&track.id);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), egui::Sense::hover());
                        let src = crate::ui::images::artwork_of(track);
                        widgets::paint_cover(ui, &mut self.images, src.as_deref(), r, 14);
                        ui.vertical(|ui| {
                            ui.add_space(8.0);
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.label(widgets::clipped(ui, &track.title, theme::medium(14.0), INK, 500.0));
                            ui.label(widgets::clipped(ui, &track.artist_line(), theme::body(13.0), theme::ink2(), 500.0));
                        });
                    });
                    let tile_w = (596.0 - 16.0) / 3.0;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        for (label, row) in [("За неделю", s.week), ("За месяц", s.month), ("За всё время", s.all)] {
                            let (r, _) = ui.allocate_exact_size(vec2(tile_w, 76.0), egui::Sense::hover());
                            ui.painter().rect_filled(r, 16, egui::Color32::from_rgba_unmultiplied(238, 247, 244, 15));
                            ui.painter().text(r.min + vec2(14.0, 12.0), egui::Align2::LEFT_TOP, crate::i18n::t(label), theme::body(11.5), theme::ink3());
                            ui.painter().text(r.min + vec2(14.0, 27.0), egui::Align2::LEFT_TOP, row.n.to_string(), theme::heavy(24.0), INK);
                            ui.painter().text(r.min + vec2(14.0, 56.0), egui::Align2::LEFT_TOP, crate::i18n::t(&crate::stats::format_time(row.ms)), theme::body(11.5), theme::ink2());
                        }
                    });
                    if let Some(i) = kit::segmented(ui, &["Неделя", "30 дней", "Год"], *range) {
                        *range = i;
                    }
                    if s.all.n == 0 {
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Этот трек ещё не слушали — статистика появится после первых 5 секунд.")).font(theme::body(14.0)).color(theme::ink3()));
                        });
                    } else {
                        stats_bars(ui, &s, *range);
                    }
                    // .stats-facts: two columns of name and value
                    let mut facts = vec![
                        ("Первое прослушивание".to_string(), long_date(s.first.as_deref())),
                        ("Последнее".to_string(), long_date(s.last.as_deref())),
                        ("В среднем в день (месяц)".to_string(), format!("{:.1}", s.month.n as f64 / 30.0)),
                        ("Среднее прослушивание".to_string(), if s.all.n > 0 { crate::i18n::t(&crate::stats::format_time(s.all.ms / s.all.n as f64)) } else { "—".into() }),
                    ];
                    if let Some(p) = track.plays {
                        let on = if track.source == crate::model::Source::Youtube { "YouTube" } else { "SoundCloud" };
                        facts.push((format!("{} {on}", crate::i18n::t("Прослушиваний на")), widgets::count(p)));
                    }
                    let col = (596.0 - 20.0) / 2.0;
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        for pair in facts.chunks(2) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 20.0;
                                for (k, v) in pair {
                                    let (r, _) = ui.allocate_exact_size(vec2(col, 18.0), egui::Sense::hover());
                                    ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, crate::i18n::t(k), theme::body(12.5), theme::ink3());
                                    ui.painter().text(r.right_center(), egui::Align2::RIGHT_CENTER, v, theme::medium(12.5), INK);
                                }
                            });
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::btn(ui, None, "Закрыть", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::SpotifyExport { lists, chosen, error } => {
                    ui.label(egui::RichText::new(crate::i18n::t("Импорт из Spotify")).font(theme::heavy(22.0)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(crate::i18n::t("На spotify.com → «Конфиденциальность» → «Скачать свои данные» запросите архив (приходит на почту за несколько дней). Выберите здесь весь zip или JSON-файлы из него: YourLibrary.json, Playlist*.json, StreamingHistory*.json. EvPlayer найдёт эти песни на YouTube Music и SoundCloud.")).font(theme::body(12.5)).color(theme::ink2()));
                    ui.add_space(8.0);
                    if widgets::pill_button(ui, Some(ic::OPEN), "Выбрать zip или JSON", false).clicked() {
                        let files = crate::dialogs::pick_files_of("Архив данных Spotify", "*.zip;*.json");
                        if !files.is_empty() {
                            let found = crate::spotify_export::parse(&crate::spotify_export::read_files(&files));
                            if found.is_empty() {
                                *error = crate::i18n::t("В файлах не нашлось плейлистов или понравившихся. Нужен архив «Данные аккаунта» от Spotify.");
                                lists.clear();
                            } else {
                                error.clear();
                                *chosen = vec![true; found.len()];
                                *lists = found;
                            }
                        }
                    }
                    if !error.is_empty() {
                        ui.label(egui::RichText::new(error.as_str()).color(egui::Color32::from_rgb(0xff, 0x6b, 0x6b)));
                    }
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for (i, l) in lists.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut chosen[i], egui::RichText::new(&l.title).font(theme::medium(13.5)));
                                ui.label(egui::RichText::new(crate::i18n::tracks(l.tracks.len())).font(theme::body(12.0)).color(theme::ink3()));
                            });
                        }
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let any = chosen.iter().any(|c| *c);
                        if widgets::pill_button(ui, Some(ic::DOWNLOAD), "Импортировать", true).clicked() && any {
                            let picked: Vec<crate::spotify_export::List> = lists.iter().zip(chosen.iter()).filter(|(_, c)| **c).map(|(l, _)| l.clone()).collect();
                            apply = Some(Box::new(move |app: &mut App| services::import_spotify_export(app, picked)));
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::ResetStats => {
                    ui.label(egui::RichText::new(crate::i18n::t("Сбросить статистику?")).font(theme::heavy(22.0)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Все прослушивания, топы и умные плейлисты обнулятся. Это нельзя отменить.")).color(theme::ink2()));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::pill_button(ui, None, "Сбросить", true).clicked() {
                            crate::stats::clear();
                            apply = Some(Box::new(|app: &mut App| {
                                app.pages.data_usage = None;
                                app.toast("Статистика сброшена");
                            }));
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::Tags { id, title, artists, album, cover } => {
                    // tagEditor.tsx: the cover with its two buttons, then the three fields
                    ui.label(egui::RichText::new(crate::i18n::t("Теги и обложка")).font(theme::heavy(22.0)));
                    ui.add_space(12.0);
                    let current = id.strip_prefix("local:").map(|s| format!("localcover:{s}"));
                    let preview: Option<String> = match cover {
                        None => current.filter(|_| self.library.get(id).is_some_and(|r| r.has_cover)),
                        Some(c) => c.clone(),
                    };
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        let (r, _) = ui.allocate_exact_size(vec2(96.0, 96.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, 14, egui::Color32::from_rgba_unmultiplied(238, 247, 244, 15));
                        if preview.is_some() {
                            widgets::paint_cover(ui, &mut self.images, preview.as_deref(), r, 14);
                        }
                        ui.vertical(|ui| {
                            ui.add_space(14.0);
                            if widgets::btn(ui, None, "Выбрать обложку", false).clicked() {
                                if let Some(path) = crate::dialogs::open_file("Картинки", "png;*.jpg;*.jpeg;*.webp;*.bmp;*.gif") {
                                    match library::square_data_url(&path, 600) {
                                        Some(url) => *cover = Some(Some(url)),
                                        None => apply = Some(Box::new(|app: &mut App| app.toast("Не получилось открыть картинку"))),
                                    }
                                }
                            }
                            if preview.is_some() && widgets::btn(ui, None, "Убрать", false).clicked() {
                                *cover = Some(None);
                            }
                        });
                    });
                    ui.add_space(12.0);
                    let field = |ui: &mut Ui, label: &str, text: &mut String| {
                        ui.label(egui::RichText::new(crate::i18n::t(label)).font(theme::body(12.5)).color(theme::ink2()));
                        ui.add(egui::TextEdit::singleline(text).char_limit(300).desired_width(f32::INFINITY));
                        ui.add_space(8.0);
                    };
                    field(ui, "Название", title);
                    field(ui, "Исполнители (через запятую)", artists);
                    field(ui, "Альбом", album);
                    ui.add_space(6.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::pill_button(ui, None, "Сохранить", true).clicked() {
                            let patch = crate::library::LocalTrackEdit {
                                title: Some(title.clone()),
                                artists: Some(artists.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect()),
                                album: Some(Some(album.trim().to_string()).filter(|a| !a.is_empty())),
                                cover: cover.clone(),
                            };
                            let i = id.clone();
                            apply = Some(Box::new(move |app: &mut App| app.save_tags(&i, patch)));
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
                Dialog::ConfirmDelete { id, title } => {
                    ui.label(egui::RichText::new(crate::i18n::t("Удалить плейлист?")).font(theme::heavy(22.0)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(format!("«{title}»")).color(theme::ink2()));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::pill_button(ui, Some(ic::DELETE), "Удалить", true).clicked() {
                            let i = id.clone();
                            state::get().update_library(|l| l.delete_playlist(&i));
                            apply = Some(Box::new(|app: &mut App| app.go(Page::Library)));
                            done = true;
                        }
                        if widgets::pill_button(ui, None, "Отмена", false).clicked() {
                            done = true;
                        }
                    });
                }
            }
        });
        if r.should_close() {
            done = true;
        }
        if done {
            self.dialog = None;
        }
        if let Some(f) = apply {
            f(self);
        }
    }

    /// "loading" in the middle of an empty part of a page
    pub fn loading(ui: &mut Ui) {
        ui.add_space(30.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            // no spinner: an animation would keep the window drawing all the time it waits
            ui.label(egui::RichText::new(crate::i18n::t("Загрузка…")).color(theme::ink2()));
        });
    }

    pub fn muted(ui: &mut Ui, text: &str) {
        ui.label(egui::RichText::new(crate::i18n::t(text)).font(theme::body(13.0)).color(theme::ink2()));
    }
}

/// what a page loads from a service, kept per page
pub struct Loaded<T> {
    pub key: String,
    pub load: Load<T>,
}

impl<T> Default for Loaded<T> {
    fn default() -> Self {
        Self { key: String::new(), load: Load::Idle }
    }
}

impl<T: Send + 'static> Loaded<T> {
    /// load for `key` (again when the key changed); the answer when it is in
    pub fn get(&mut self, key: &str, f: impl FnOnce() -> T + Send + 'static) -> Option<&T> {
        if self.key != key {
            self.key = key.to_string();
            self.load = Load::start(f);
        }
        self.load.poll();
        self.load.ready()
    }
}

#[allow(dead_code)]
fn _unused(_: Rect) {}

/// "27 октября 2026 г." / "October 27, 2026" from a saved ISO time
fn long_date(iso: Option<&str>) -> String {
    use chrono::Datelike;
    let Some(d) = iso.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&chrono::Local).date_naive()) else { return "—".into() };
    const RU: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
    const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let m = d.month0() as usize;
    if crate::i18n::is_english() {
        format!("{} {}, {}", EN[m], d.day(), d.year())
    } else {
        format!("{} {} {} г.", d.day(), RU[m], d.year())
    }
}

/// Bars (TrackStatsDialog.tsx): listens per day (a week, 30 days) or per month (a year); a
/// bar under the pointer shows its count
fn stats_bars(ui: &mut Ui, s: &crate::stats::TrackSummary, range: usize) {
    use chrono::Datelike;
    let english = crate::i18n::is_english();
    let (values, labels): (Vec<u32>, Vec<String>) = match range {
        2 => {
            const RU: [&str; 12] = ["янв", "февр", "мар", "апр", "мая", "июн", "июл", "авг", "сент", "окт", "нояб", "дек"];
            const EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            s.monthly.iter().map(|(d, r)| (r.n, (if english { EN } else { RU })[d.month0() as usize].to_string())).unzip()
        }
        0 => {
            const RU: [&str; 7] = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];
            const EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
            s.daily[s.daily.len().saturating_sub(7)..].iter().map(|(d, r)| (r.n, (if english { EN } else { RU })[d.weekday().num_days_from_monday() as usize].to_string())).unzip()
        }
        _ => {
            let n = s.daily.len();
            s.daily.iter().enumerate().map(|(i, (d, r))| (r.n, if i % 5 == 4 || i == n - 1 { d.day().to_string() } else { String::new() })).unzip()
        }
    };
    let w = ui.available_width();
    let h = 150.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h + 18.0), egui::Sense::hover());
    let p = ui.painter();
    for f in [0.25, 0.5, 0.75, 1.0] {
        p.hline(rect.x_range(), rect.top() + h - h * f, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(238, 247, 244, 20)));
    }
    let max = values.iter().copied().max().unwrap_or(1).max(1) as f32;
    let gap = if values.len() > 14 { 3.0 } else { 8.0 };
    let bar = (w - gap * (values.len() as f32 - 1.0)) / values.len() as f32;
    let hover = resp.hover_pos().map(|p| ((p.x - rect.left()) / (bar + gap)) as usize).filter(|i| *i < values.len());
    for (i, v) in values.iter().enumerate() {
        let x = rect.left() + i as f32 * (bar + gap);
        let bh = if *v == 0 { 2.0 } else { ((*v as f32 / max) * (h - 8.0)).max(4.0) };
        let r = Rect::from_min_size(pos2(x, rect.top() + h - bh), vec2(bar, bh));
        let fill = if *v == 0 { egui::Color32::from_rgba_unmultiplied(238, 247, 244, 26) } else if hover == Some(i) { theme::ACCENT } else { egui::Color32::from_rgba_unmultiplied(111, 240, 196, 178) };
        p.rect_filled(r, (bar / 2.0).min(4.0) as u8, fill);
        if !labels[i].is_empty() {
            p.text(pos2(x + bar / 2.0, rect.top() + h + 14.0), egui::Align2::CENTER_BOTTOM, &labels[i], egui::FontId::new(10.0, theme::medium500()), theme::ink3());
        }
    }
    if let Some(i) = hover {
        let x = rect.left() + i as f32 * (bar + gap) + bar / 2.0;
        let g = p.layout_no_wrap(values[i].to_string(), theme::heavy(11.5), theme::GROUND);
        let tip = Rect::from_center_size(pos2(x, rect.top() - 8.0), g.size() + vec2(16.0, 6.0));
        p.rect_filled(tip, 8, INK);
        p.galley(tip.center() - g.size() / 2.0, g, theme::GROUND);
    }
}

#[cfg(test)]
mod tests {
    use super::final_index;

    #[test]
    fn places_the_item_before_or_after_the_row_it_was_dropped_on() {
        assert_eq!(final_index(0, 3, true), 2);
        assert_eq!(final_index(0, 3, false), 3);
        assert_eq!(final_index(5, 1, true), 1);
        assert_eq!(final_index(5, 1, false), 2);
        assert_eq!(final_index(2, 2, false), 2);
    }
}
