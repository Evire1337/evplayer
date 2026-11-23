//! The pages, each in its own file, and what several of them share: the menu of a song, the
//! dialogs, rows of cards, lists of songs.

mod artist;
mod files;
mod home;
mod library;
mod now;
mod search;
mod settings;
mod sound;

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
    pub settings_tab: usize,
    pub sound_open: bool,
    pub show_queue: Option<bool>,
}

impl App {
    /// a list of songs; pressing one plays the list from it
    pub fn track_list(&mut self, ui: &mut Ui, tracks: &[Track], source: Option<PlaySource>, playlist: Option<(&str, &[String])>, extra: &dyn Fn(usize) -> Option<String>) {
        let current = self.view.current().map(|t| t.id.clone());
        let double = state::get().settings.read().unwrap().double_click_to_play;
        let lib_likes: std::collections::HashSet<String> = state::get().library.read().unwrap().likes.iter().map(|l| l.track.id.clone()).collect();
        // only the rows on screen are laid out: a playlist of thousands costs as much as a short one
        let row_h = 52.0 + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical().id_salt("tracks").auto_shrink([false, false]).show_rows(ui, row_h - ui.spacing().item_spacing.y, tracks.len(), |ui, range| {
            for i in range {
                let t = &tracks[i];
                let e = extra(i);
                let out = widgets::track_row(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), lib_likes.contains(&t.id), e.as_deref(), None);
                if (out.clicked && !double) || out.double {
                    self.play_list(tracks, i, source.clone());
                }
                if out.like {
                    self.toggle_like(t);
                }
                if out.menu {
                    let at = ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center());
                    let item = playlist.and_then(|(pid, items)| items.get(i).map(|item| (pid.to_string(), item.clone())));
                    self.menu = Some(Menu { track: t.clone(), at, playlist_item: item });
                }
            }
        });
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
                        self.menu = Some(Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(r.rect.center()), playlist_item: None });
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

    /// the menu of a song: play next, add to the queue, to a playlist, like, radio, the artist
    pub fn track_menu(&mut self, ctx: &egui::Context) {
        let Some(m) = &self.menu else { return };
        let track = m.track.clone();
        let at = m.at;
        let item = m.playlist_item.clone();
        let mut close = false;
        let res = egui::Area::new(egui::Id::unique("track-menu")).order(egui::Order::Foreground).fixed_pos(at).constrain(true).show(ctx, |ui| {
            egui::Frame::new().fill(egui::Color32::from_rgb(0x0f, 0x31, 0x2d)).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(14).inner_margin(6).show(ui, |ui| {
                ui.set_min_width(230.0);
                let entry = |ui: &mut Ui, icon: &str, text: &str| -> bool {
                    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width().max(230.0), 32.0), egui::Sense::click());
                    if r.hovered() {
                        ui.painter().rect_filled(rect, 9, theme::hover());
                    }
                    ui.painter().text(pos2(rect.left() + 16.0, rect.center().y), egui::Align2::CENTER_CENTER, icon, theme::icon(13.0), theme::ink2());
                    ui.painter().text(pos2(rect.left() + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, crate::i18n::t(text), theme::body(13.5), INK);
                    r.clicked()
                };
                if entry(ui, ic::PLAY, "Играть") {
                    self.player.play_queue(vec![track.clone()], 0, None);
                    close = true;
                }
                if entry(ui, ic::NEXT, "Играть следующим") {
                    self.player.add(track.clone(), true);
                    self.toast("Будет следующим");
                    close = true;
                }
                if entry(ui, ic::QUEUE, "В очередь") {
                    self.player.add(track.clone(), false);
                    self.toast("Добавлено в очередь");
                    close = true;
                }
                if entry(ui, ic::RADIO, "Радио по треку") {
                    self.player.start_radio(track.clone());
                    close = true;
                }
                let liked = self.is_liked(&track.id);
                if entry(ui, if liked { ic::HEART_FILL } else { ic::HEART }, if liked { "Убрать из «Любимого»" } else { "Нравится" }) {
                    self.toggle_like(&track);
                    close = true;
                }
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("Добавить в плейлист")).font(theme::body(12.0)).color(theme::ink3()));
                let lists: Vec<(String, String)> = state::get().library.read().unwrap().playlists.iter().map(|p| (p.id.clone(), p.title.clone())).collect();
                for (id, title) in lists.iter().take(8) {
                    if entry(ui, ic::ADD, title) {
                        let n = state::get().update_library(|l| l.add_to_playlist(id, std::slice::from_ref(&track)));
                        self.toast(&if n > 0 { format!("Добавлено в «{title}»") } else { format!("Уже есть в «{title}»") });
                        close = true;
                    }
                }
                if entry(ui, ic::ADD, "Новый плейлист…") {
                    self.dialog = Some(Dialog::NewPlaylist { title: String::new(), add: vec![track.clone()] });
                    close = true;
                }
                if let Some((pid, item_id)) = &item {
                    ui.separator();
                    if entry(ui, ic::DELETE, "Убрать из плейлиста") {
                        if pid == "likes" {
                            self.toggle_like(&track);
                        } else {
                            state::get().update_library(|l| l.remove_from_playlist(pid, item_id));
                        }
                        close = true;
                    }
                }
                if let Some(a) = track.artists.first().filter(|a| a.id.starts_with("scu:") || a.id.starts_with("yta:")) {
                    ui.separator();
                    if entry(ui, ic::PEOPLE, &format!("Исполнитель · {}", a.name)) {
                        self.go(Page::Artist(a.id.clone()));
                        close = true;
                    }
                }
                if let Some(link) = &track.permalink {
                    if entry(ui, ic::LINK, "Копировать ссылку") {
                        ui.ctx().copy_text(link.clone());
                        self.toast("Ссылка скопирована");
                        close = true;
                    }
                }
            });
        });
        let clicked_away = ctx.input(|i| i.pointer.any_pressed()) && !res.response.contains_pointer();
        if close || clicked_away || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.menu = None;
        }
    }

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.dialog else { return };
        let mut done = false;
        let mut apply: Option<Box<dyn FnOnce(&mut App)>> = None;
        let frame = egui::Frame::new().fill(egui::Color32::from_rgb(0x0c, 0x2c, 0x28)).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(20).inner_margin(22);
        let r = egui::Modal::new(egui::Id::unique("dialog")).frame(frame).backdrop_color(egui::Color32::from_black_alpha(140)).show(ctx, |ui| {
            ui.set_width(380.0);
            match d {
                Dialog::NewPlaylist { title, add } => {
                    ui.label(egui::RichText::new(crate::i18n::t("Новый плейлист")).font(theme::heavy(22.0)));
                    ui.add_space(10.0);
                    let r = ui.add(egui::TextEdit::singleline(title).hint_text(crate::i18n::t("Название")).desired_width(f32::INFINITY));
                    r.request_focus();
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if widgets::pill_button(ui, None, "Создать", true).clicked() || enter {
                            let (t, a) = (title.clone(), std::mem::take(add));
                            apply = Some(Box::new(move |app: &mut App| {
                                let id = state::get().update_library(|l| {
                                    let id = l.create_playlist(&t, None, None);
                                    l.add_to_playlist(&id, &a);
                                    id
                                });
                                let name = state::get().library.read().unwrap().playlists.iter().find(|p| p.id == id).map(|p| p.title.clone()).unwrap_or_default();
                                app.toast(&format!("Плейлист «{name}» создан"));
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
