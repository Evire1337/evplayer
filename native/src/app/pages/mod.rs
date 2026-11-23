//! The pages, each in its own file, and what several of them share: the menu of a song, the
//! dialogs, rows of cards, lists of songs.

mod artist;
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
}

impl App {
    /// a list of songs; pressing one plays the list from it
    /// `dated`: the notes are the dates songs were added, in their own column
    pub fn track_list(&mut self, ui: &mut Ui, tracks: &[Track], source: Option<PlaySource>, playlist: Option<(&str, &[String])>, extra: &dyn Fn(usize) -> Option<String>, dated: bool) {
        let current = self.view.current().map(|t| t.id.clone());
        // the current row shows pause over its cover while it plays
        let playing = self.view.status == crate::player::Status::Playing;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("evplayer-playing"), playing));
        let double = state::get().settings.read().unwrap().double_click_to_play;
        let lib_likes: std::collections::HashSet<String> = state::get().library.read().unwrap().likes.iter().map(|l| l.track.id.clone()).collect();
        // only the rows on screen are laid out: a playlist of thousands costs as much as a short one
        let row_h = 62.0 + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical().id_salt("tracks").auto_shrink([false, false]).show_rows(ui, row_h - ui.spacing().item_spacing.y, tracks.len(), |ui, range| {
            for i in range {
                let t = &tracks[i];
                let e = extra(i);
                let out = widgets::track_row_full(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), lib_likes.contains(&t.id), e.as_deref(), None, dated);
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
                    crate::ui::theme::paint_icon(&ui.painter(), pos2(rect.left() + 16.0, rect.center().y), egui::Align2::CENTER_CENTER, icon, 13.0, theme::ink2());
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
                if self.downloads.is_saved(&track.id) {
                    if entry(ui, ic::DELETE, "Удалить скачанное") {
                        self.downloads.remove(std::slice::from_ref(&track.id));
                        close = true;
                    }
                } else if track.source != crate::model::Source::Local && entry(ui, ic::DOWNLOAD, "Скачать для офлайн") {
                    self.download(std::slice::from_ref(&track));
                    close = true;
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
        // what is saved, for the offline dialog's counts
        let saved: std::collections::HashSet<String> = if matches!(self.dialog, Some(Dialog::Download { .. })) { self.downloads.saved().into_iter().map(|e| e.id).collect() } else { Default::default() };
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
                Dialog::Import { text, likes } => {
                    ui.label(egui::RichText::new(crate::i18n::t("Импорт по ссылке или коду")).font(theme::heavy(19.0)));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Ссылка на плейлист, альбом или трек из SoundCloud, YouTube, Spotify, Deezer или Яндекс Музыки, код плейлиста EvPlayer — или список песен «Исполнитель — Название» (например, скопированный со страницы VK).")).font(theme::body(12.5)).color(theme::ink2()));
                    ui.add_space(10.0);
                    let r = ui.add(egui::TextEdit::multiline(text).hint_text("https://… или EVP2.…").desired_rows(4).desired_width(f32::INFINITY));
                    r.request_focus();
                    ui.add_space(6.0);
                    if let Some(v) = widgets::toggle_row(ui, "Добавить в «Понравившиеся»", "Иначе — новый плейлист", *likes, true) {
                        *likes = v;
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let ready = !text.trim().is_empty();
                        if widgets::pill_button(ui, Some(ic::DOWNLOAD), "Импортировать", true).clicked() && ready {
                            let (t, l) = (text.clone(), *likes);
                            apply = Some(Box::new(move |app: &mut App| app.start_import(t, if l { Some(true) } else { None })));
                            done = true;
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
