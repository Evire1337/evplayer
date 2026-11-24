//! An artist's page and an album's or a service playlist's page (src/pages/ArtistPage.tsx,
//! CollectionPage.tsx), with "save to the library" for albums and playlists.

use egui::{vec2, Ui};

use super::search::fmt_count;
use super::home::plays_word;
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
    /// what the service says (followers, monthly listeners), or how many are in your files
    pub facts: Vec<String>,
    pub popular: Vec<Track>,
    pub tracks: Vec<Track>,
    /// "SoundCloud", "YouTube Music" or "поиск"
    pub source: String,
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

/// An artist as the page loads one (loadArtist): from SoundCloud or YouTube Music by id, or, for
/// your files' artists and names only ("name:…"), your files by that artist and what both
/// services find under the name
fn load_artist(id: String, library: std::sync::Arc<crate::library::Library>) -> Result<ArtistData, String> {
    let by_plays = |v: &[Track]| {
        let mut v = v.to_vec();
        v.sort_by(|a, b| b.plays.unwrap_or(0.0).partial_cmp(&a.plays.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
        v.truncate(10);
        v
    };
    if id.starts_with("scu:") {
        let u = soundcloud::user(&id)?;
        let a: ArtistSummary = u.artist.unwrap_or_default();
        let facts = a.followers.map(|n| vec![format!("{} {}", fmt_count(n), crate::i18n::t("подписчиков на SoundCloud"))]).unwrap_or_default();
        let popular = if u.popular.is_empty() { by_plays(&u.tracks) } else { u.popular.into_iter().take(10).collect() };
        return Ok(ArtistData { name: a.name, avatar: a.avatar, facts, popular, tracks: u.tracks, source: "SoundCloud".into() });
    }
    if id.starts_with("yta:") {
        let a = youtube::artist(&id)?;
        let mut tracks = a.tracks;
        // a channel with few songs: what the search finds under its name fills in
        if tracks.len() < 5 {
            for t in youtube::search(&a.name).unwrap_or_default() {
                if !tracks.iter().any(|x| x.id == t.id) {
                    tracks.push(t);
                }
            }
        }
        let facts = [a.monthly_listeners.map(|n| format!("{} {}", fmt_count(n), crate::i18n::t("слушателей в месяц"))), a.subscribers.map(|n| format!("{} {}", fmt_count(n), crate::i18n::t("подписчиков на YouTube")))].into_iter().flatten().collect();
        return Ok(ArtistData { name: a.name, avatar: a.avatar, facts, popular: by_plays(&tracks), tracks, source: "YouTube Music".into() });
    }
    let name = id.strip_prefix("name:").unwrap_or(&id).to_string();
    let lower = name.to_lowercase();
    let local: Vec<Track> = library.list().iter().map(super::files::track_of).filter(|t| t.artists.iter().any(|a| a.name.to_lowercase() == lower)).collect();
    let (sc, yt) = std::thread::scope(|s| {
        let sc = s.spawn(|| soundcloud::search(&name).unwrap_or_default());
        let yt = s.spawn(|| youtube::search(&name).unwrap_or_default());
        (sc.join().unwrap_or_default(), yt.join().unwrap_or_default())
    });
    let found: Vec<Track> = yt.into_iter().chain(sc.0).filter(|t| t.artists.iter().any(|a| a.name.to_lowercase().contains(&lower))).collect();
    let avatar = sc.1.iter().find(|a| a.name.to_lowercase() == lower).and_then(|a| a.avatar.clone());
    let facts = if local.is_empty() { vec![] } else { vec![format!("{} {}", local.len(), crate::i18n::t("в ваших файлах"))] };
    Ok(ArtistData { name, avatar, facts, popular: by_plays(&found), tracks: local.into_iter().chain(found).collect(), source: crate::i18n::t("поиск") })
}

/// where an artist's name leads: its page on a service, else a page found by the name
pub fn artist_page_id(a: &crate::model::Artist) -> String {
    if a.id.starts_with("scu:") || a.id.starts_with("yta:") { a.id.clone() } else { format!("name:{}", a.name) }
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
    /// ArtistPage.tsx: the round photo (200, with its shadow), "Исполнитель · <where from>", the
    /// name, what the service says and how much you played them, "Слушать" and "Радио"; then
    /// "Популярное" (the service's and your plays) and "Все треки" with their services
    pub fn artist(&mut self, ui: &mut Ui, id: &str) {
        let key = id.to_string();
        let library = self.library.clone();
        let data = self.pages.artist.data.get(id, move || load_artist(key, library)).cloned();
        let fallback = id.strip_prefix("name:").unwrap_or("").to_string();
        let (a, loading, failed) = match data {
            None => (ArtistData { name: fallback, ..Default::default() }, true, false),
            Some(Err(_)) => (ArtistData { name: fallback, ..Default::default() }, false, true),
            Some(Ok(a)) => (a, false, false),
        };
        // your own listening of this artist (the statistics on this computer)
        let mut seen = std::collections::HashSet::new();
        let mut mine = 0;
        let mut per_track = std::collections::HashMap::new();
        for t in a.popular.iter().chain(a.tracks.iter()) {
            if seen.insert(t.id.clone()) {
                let n = crate::stats::plays_of(&t.id);
                per_track.insert(t.id.clone(), n);
                mine += n;
            }
        }
        egui::ScrollArea::vertical().id_salt("artist").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let (photo, _) = ui.allocate_exact_size(vec2(200.0, 200.0), egui::Sense::hover());
                // box-shadow: 0 24px 60px rgba(0,0,0,.35)
                ui.painter().add(egui::Shadow { offset: [0, 24], blur: 60, spread: 0, color: egui::Color32::from_black_alpha(89) }.as_shape(photo, 100));
                ui.painter().circle_filled(photo.center(), 100.0, egui::Color32::from_rgba_unmultiplied(238, 247, 244, 20));
                match a.avatar.as_deref().and_then(|s| self.images.get(s, 200.0, ui.ctx().pixels_per_point())) {
                    Some(tex) => egui::Image::new(egui::load::SizedTexture::new(tex, photo.size())).corner_radius(100).paint_at(ui, photo),
                    None => crate::ui::icons::paint(ui.painter(), "user", egui::Rect::from_center_size(photo.center(), vec2(64.0, 64.0)), theme::ink3()),
                }
                ui.add_space(28.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.add_space((200.0f32 - (4.0 * 26.0 + 56.0)).max(0.0));
                    let eyebrow = if loading || failed { crate::i18n::t("Исполнитель") } else { format!("{} · {}", crate::i18n::t("Исполнитель"), a.source) };
                    ui.label(egui::RichText::new(eyebrow.to_uppercase()).font(theme::medium(12.0)).color(theme::ink3()).extra_letter_spacing(1.0));
                    ui.label(egui::RichText::new(&a.name).font(theme::heavy(46.0)).color(INK));
                    let mut facts = a.facts.clone();
                    if mine > 0 {
                        facts.push(format!("{} {}", crate::i18n::t("у вас"), crate::i18n::t(&plays_word(mine))));
                    }
                    let line = if facts.is_empty() && loading { crate::i18n::t("Загружаю…") } else { facts.join(" · ") };
                    ui.label(egui::RichText::new(line).font(theme::body(13.0)).color(theme::ink2()));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let play = if a.popular.is_empty() { a.tracks.clone() } else { a.popular.clone() };
                        if widgets::round_button(ui, ic::PLAY, 52.0, widgets::Round::Main, false, "Слушать").clicked() && !play.is_empty() {
                            self.player.play_queue(play, 0, None);
                        }
                        if let Some(first) = a.popular.first() {
                            if widgets::btn(ui, Some("radio"), "Радио", false).clicked() {
                                self.player.start_radio(first.clone());
                            }
                        }
                    });
                });
            });
            if failed {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| App::muted(ui, "Не получилось загрузить исполнителя."));
            }
            if !a.popular.is_empty() {
                widgets::section(ui, "Популярное");
                let notes: Vec<Option<String>> = a.popular.iter().map(|t| {
                    let parts: Vec<String> = [t.plays.map(widgets::count), per_track.get(&t.id).filter(|n| **n > 0).map(|n| format!("{} {n}", crate::i18n::t("вы:")))].into_iter().flatten().collect();
                    (!parts.is_empty()).then(|| parts.join(" · "))
                }).collect();
                self.plain_rows(ui, &a.popular, &notes, true, false);
            }
            if a.tracks.len() > a.popular.len() {
                widgets::section(ui, "Все треки");
                self.plain_rows(ui, &a.tracks, &[], false, true);
            }
            ui.add_space(20.0);
        });
    }

    /// songs as rows without their own scrolling (inside a page that scrolls): their notes,
    /// numbered or not, with their service marked or not
    pub fn plain_rows(&mut self, ui: &mut Ui, tracks: &[Track], notes: &[Option<String>], numbered: bool, badge: bool) {
        self.mark_kept();
        let current = self.view.current().map(|t| t.id.clone());
        for (i, t) in tracks.iter().enumerate() {
            let liked = self.is_liked(&t.id);
            let note = notes.get(i).cloned().flatten();
            let out = widgets::track_row_full(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), liked, note.as_deref(), numbered.then_some(i + 1), false, badge);
            if out.clicked || out.double {
                self.play_list(tracks, i, None);
            }
            if out.like {
                self.toggle_like(t);
            }
            if out.menu {
                self.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center()), playlist_item: None, queue_index: None, sub: false, files_page: false });
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
