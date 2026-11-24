//! An artist's page and an album's or a service playlist's page (src/pages/ArtistPage.tsx,
//! CollectionPage.tsx), with "save to the library" for albums and playlists.

use egui::{vec2, Ui};

use super::search::fmt_count;
use super::Loaded;
use crate::app::App;
use crate::model::{ArtistSummary, Track};
use crate::services::{soundcloud, youtube};
use crate::state;
use crate::store::state::PlaySource;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

#[derive(Clone, Default)]
pub struct ArtistData {
    pub name: String,
    pub avatar: Option<String>,
    pub line: String,
    pub popular: Vec<Track>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Default)]
pub struct CollectionData {
    pub title: String,
    pub subtitle: String,
    pub artwork: Option<String>,
    pub tracks: Vec<Track>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct ArtistState {
    data: Loaded<Result<ArtistData, String>>,
}

#[derive(Default)]
pub struct CollectionState {
    data: Loaded<CollectionData>,
}

fn load_artist(id: String) -> Result<ArtistData, String> {
    if id.starts_with("yta:") {
        let a = youtube::artist(&id)?;
        let line = [a.monthly_listeners.map(|n| format!("{} {}", fmt_count(n), crate::i18n::t("слушателей в месяц"))), a.subscribers.map(|n| format!("{} {}", fmt_count(n), crate::i18n::t("подписчиков на YouTube")))].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        Ok(ArtistData { name: a.name, avatar: a.avatar, line, popular: vec![], tracks: a.tracks })
    } else {
        let u = soundcloud::user(&id)?;
        let a: ArtistSummary = u.artist.unwrap_or_default();
        let line = a.followers.map(|n| format!("{} {}", fmt_count(n), crate::i18n::t("подписчиков на SoundCloud"))).unwrap_or_default();
        Ok(ArtistData { name: a.name, avatar: a.avatar, line, popular: u.popular, tracks: u.tracks })
    }
}

fn load_collection(id: String) -> CollectionData {
    if id.starts_with("ytb:") {
        match youtube::collection(&id, 500) {
            Ok(c) => CollectionData { title: c.title, subtitle: c.subtitle, artwork: c.artwork, tracks: c.tracks, error: None },
            Err(e) => CollectionData { error: Some(e), ..Default::default() },
        }
    } else {
        match soundcloud::playlist(&id) {
            Ok((summary, tracks)) => {
                let s = summary.unwrap_or_default();
                CollectionData { title: s.title, subtitle: s.subtitle, artwork: s.artwork, tracks, error: None }
            }
            Err(e) => CollectionData { error: Some(e), ..Default::default() },
        }
    }
}

impl App {
    pub fn artist(&mut self, ui: &mut Ui, id: &str) {
        let key = id.to_string();
        let data = self.pages.artist.data.get(id, move || load_artist(key)).cloned();
        let a = match data {
            None => return App::loading(ui),
            Some(Err(e)) => return App::muted(ui, &e),
            Some(Ok(a)) => a,
        };
        egui::ScrollArea::vertical().id_salt("artist").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                widgets::cover(ui, &mut self.images, a.avatar.as_deref(), 180.0, 90);
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new(crate::i18n::t("ИСПОЛНИТЕЛЬ")).font(theme::medium(11.0)).color(theme::ink3()));
                    ui.label(egui::RichText::new(&a.name).font(theme::heavy(40.0)).color(INK));
                    App::muted(ui, &a.line);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let all: Vec<Track> = a.popular.iter().chain(a.tracks.iter()).cloned().collect();
                        if widgets::pill_button(ui, Some(ic::PLAY), "Играть", true).clicked() && !all.is_empty() {
                            self.player.play_queue(all.clone(), 0, None);
                        }
                        if widgets::pill_button(ui, Some(ic::RADIO), "Радио исполнителя", false).clicked() {
                            if let Some(t) = all.first() {
                                self.player.start_radio(t.clone());
                            }
                        }
                    });
                });
            });
            if !a.popular.is_empty() {
                widgets::section(ui, "Популярное");
                self.plain_rows(ui, &a.popular);
            }
            if !a.tracks.is_empty() {
                widgets::section(ui, "Треки");
                self.plain_rows(ui, &a.tracks);
            }
            ui.add_space(20.0);
        });
    }

    /// songs as rows without their own scrolling (inside a page that scrolls)
    pub fn plain_rows(&mut self, ui: &mut Ui, tracks: &[Track]) {
        let current = self.view.current().map(|t| t.id.clone());
        for (i, t) in tracks.iter().enumerate() {
            let liked = self.is_liked(&t.id);
            let plays = t.plays.map(|p| format!("{} {}", fmt_count(p), crate::i18n::t("прослушиваний")));
            let out = widgets::track_row(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), liked, plays.as_deref(), Some(i + 1));
            if out.clicked || out.double {
                self.play_list(tracks, i, None);
            }
            if out.like {
                self.toggle_like(t);
            }
            if out.menu {
                self.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center()), playlist_item: None, queue_index: None, sub: false });
            }
        }
    }

    pub fn collection(&mut self, ui: &mut Ui, id: &str) {
        let key = id.to_string();
        let data = self.pages.collection.data.get(id, move || load_collection(key)).cloned();
        let Some(c) = data else { return App::loading(ui) };
        if let Some(e) = &c.error {
            return App::muted(ui, e);
        }
        let kind = if id.contains("MPRE") || id.contains("OLAK") { "Альбом" } else { "Плейлист" };
        let service = if id.starts_with("ytb:") { "YouTube Music" } else { "SoundCloud" };
        let source = PlaySource { page: "collection".into(), id: Some(id.to_string()), title: Some(c.title.clone()), cover: c.artwork.clone(), kind: Some(format!("{} · {service}", crate::i18n::t(kind))) };
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            widgets::cover(ui, &mut self.images, c.artwork.as_deref(), 200.0, 18);
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.add_space(28.0);
                ui.label(egui::RichText::new(format!("{} · {service}", crate::i18n::t(kind)).to_uppercase()).font(theme::medium(11.0)).color(theme::ink3()));
                ui.label(egui::RichText::new(&c.title).font(theme::heavy(36.0)).color(INK));
                App::muted(ui, &format!("{} · {}", c.subtitle, crate::i18n::tracks(c.tracks.len())));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if widgets::pill_button(ui, Some(ic::PLAY), "Играть", true).clicked() && !c.tracks.is_empty() {
                        self.player.play_queue(c.tracks.clone(), 0, Some(source.clone()));
                    }
                    if widgets::pill_button(ui, Some(ic::SHUFFLE), "Вперемешку", false).clicked() && !c.tracks.is_empty() {
                        self.player.play_queue(crate::player::queue::shuffled(&c.tracks, None), 0, Some(source.clone()));
                    }
                    if widgets::pill_button(ui, Some(ic::ADD), "В библиотеку", false).clicked() {
                        let (title, tracks, from) = (c.title.clone(), c.tracks.clone(), id.to_string());
                        state::get().update_library(|l| {
                            // imported again: the same playlist is updated
                            let existing = l.playlists.iter().find(|p| p.source.as_deref() == Some(from.as_str())).map(|p| p.id.clone());
                            let pid = existing.unwrap_or_else(|| l.create_playlist(&title, None, Some(&from)));
                            l.add_to_playlist(&pid, &tracks);
                        });
                        self.toast(&format!("«{}» сохранён в библиотеку", c.title));
                    }
                });
            });
        });
        ui.add_space(12.0);
        let _ = vec2(0.0, 0.0);
        self.track_list(ui, &c.tracks, Some(source), None, &|_| None, false);
    }
}
