//! Главная (src/pages/HomePage.tsx), in the page's order: the greeting with "Сегодня
//! прослушано", "Недавно слушали" (playlists, albums and songs by when they played),
//! recommendations from what you play and like, "Собрано для вас", "Ваш топ-10" with "Ваши
//! топ-исполнители" beside it, "Новое у ваших исполнителей" and what is trending on SoundCloud.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use egui::{pos2, vec2, Color32, CornerRadius, Rect, Ui};

use super::covers::{self, hsl};
use super::Loaded;
use crate::app::{App, Page};
use crate::model::Track;
use crate::state;
use crate::ui::theme::{self, INK};
use crate::ui::widgets;

#[derive(Default)]
pub struct HomeState {
    for_you: Loaded<(Vec<Track>, Vec<(Track, Vec<Track>)>)>,
    trending: Loaded<Result<Vec<Track>, String>>,
    new_from: Loaded<Vec<Track>>,
    /// "Ваш топ-10": week, month, all time
    pub top_range: usize,
}

fn greeting() -> &'static str {
    use chrono::Timelike;
    let h = chrono::Local::now().hour();
    match h {
        0..=4 => "Доброй ночи",
        5..=11 => "Доброе утро",
        12..=17 => "Добрый день",
        _ => "Добрый вечер",
    }
}

/// SoundCloud's charts change slowly: kept for the run instead of asked every visit (chartCache)
static CHART: Mutex<Option<Vec<Track>>> = Mutex::new(None);

impl App {
    pub fn home(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().id_salt("home").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| self.page_title(ui, greeting()));
                // "Сегодня прослушано", from a minute on
                let today = crate::stats::listened_today();
                if today >= 60_000.0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            ui.add_space(26.0);
                            ui.label(egui::RichText::new(crate::i18n::t("СЕГОДНЯ ПРОСЛУШАНО")).font(theme::medium(12.0)).color(theme::ink3()));
                            ui.label(egui::RichText::new(crate::i18n::t(&crate::stats::format_time(today))).font(theme::heavy(24.0)));
                        });
                    });
                }
            });
            self.recent(ui);
            self.for_you(ui);
            self.smart_row(ui);
            self.tops(ui);
            self.new_from_artists(ui);
            self.trending(ui);
            ui.add_space(24.0);
        });
    }

    /// "В тренде на SoundCloud": cards, or why there are none (the region guard, no answer)
    fn trending(&mut self, ui: &mut Ui) {
        let cached = CHART.lock().unwrap().clone();
        let data = match cached {
            Some(list) => Some(Ok(list)),
            None => self.pages.home.trending.get("trending", || crate::services::soundcloud::charts()).cloned(),
        };
        if let Some(Ok(list)) = &data {
            if list.is_empty() {
                return;
            }
            CHART.lock().unwrap().get_or_insert_with(|| list.clone());
        }
        widgets::section(ui, "В тренде на SoundCloud");
        match data {
            Some(Ok(list)) => self.card_row(ui, "trending", &list),
            Some(Err(e)) if e.starts_with("SoundCloud не трогаю") => App::muted(ui, "SoundCloud не трогаю с адреса вашей страны. Включите VPN другой страны или отключите защиту: Настройки → Сервисы."),
            Some(Err(_)) => App::muted(ui, "SoundCloud сейчас не отвечает. Попробуйте позже."),
            None => App::loading(ui),
        }
    }

    /// "Недавно слушали": playlists and albums stand for the songs played from them, other songs
    /// show on their own; the newest first, twenty at most
    fn recent(&mut self, ui: &mut Ui) {
        let (sources, entries) = {
            let h = state::get().history.lock().unwrap();
            (h.sources.clone(), h.entries.clone())
        };
        let shown: std::collections::HashSet<String> = sources.iter().map(|s| s.source.key()).collect();
        enum Item<'a> {
            Source(&'a crate::store::state::PlaySource),
            Song(&'a Track),
        }
        let mut items: Vec<(&str, Item)> = sources.iter().map(|s| (s.played_at.as_str(), Item::Source(&s.source))).collect();
        items.extend(entries.iter().filter(|e| e.from.as_ref().is_none_or(|f| !shown.contains(f))).map(|e| (e.played_at.as_str(), Item::Song(&e.track))));
        items.sort_by(|a, b| b.0.cmp(a.0));
        items.truncate(20);
        if items.is_empty() {
            return;
        }
        let songs: Vec<Track> = items.iter().filter_map(|i| if let Item::Song(t) = i.1 { Some((*t).clone()) } else { None }).collect();
        widgets::section(ui, "Недавно слушали");
        let w = 172.0;
        let lib = state::get().library.read().unwrap().clone();
        let liked: Vec<Track> = lib.likes.iter().map(|l| l.track.clone()).collect();
        egui::ScrollArea::horizontal().id_salt("recent").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let mut song_i = 0;
                for (_, item) in &items {
                    match item {
                        Item::Song(t) => {
                            let src = crate::ui::images::artwork_of(t);
                            let r = widgets::card(ui, &mut self.images, src.as_deref(), &t.title, &t.artist_line(), w, false);
                            if r.clicked() {
                                self.play_list(&songs, song_i, None);
                            }
                            if r.secondary_clicked() {
                                self.menu = Some(crate::app::Menu { track: (*t).clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(r.rect.center()), playlist_item: None, queue_index: None, sub: false, files_page: false });
                            }
                            song_i += 1;
                        }
                        Item::Source(s) => {
                            let id = s.id.clone().unwrap_or_default();
                            let r = match s.page.as_str() {
                                "playlist" if id == "likes" => (covers::card(ui, &mut self.images, &covers::Cover::Liked, w, &crate::i18n::t("Понравившиеся"), &crate::i18n::tracks(lib.likes.len())), Page::Playlist(id)),
                                "playlist" => {
                                    let Some(p) = lib.playlists.iter().find(|p| p.id == id) else { continue };
                                    let tracks: Vec<Track> = p.items.iter().take(40).map(|i| i.track.clone()).collect();
                                    let sub = format!("{} · {}", crate::i18n::t("Плейлист"), crate::i18n::tracks(p.items.len()));
                                    (covers::card(ui, &mut self.images, &covers::Cover::Playlist { icon: p.icon.as_deref(), tracks: &tracks }, w, &p.title, &sub), Page::Playlist(id))
                                }
                                "smart" => {
                                    let Some((_, title, _, hue)) = crate::stats::SMART.iter().find(|x| x.0 == id) else { continue };
                                    let tracks = crate::stats::smart(&id, &liked, 100);
                                    (covers::card(ui, &mut self.images, &covers::Cover::Smart { hue: *hue, tracks: &tracks }, w, &crate::i18n::t(title), &crate::i18n::t("Собрано для вас")), Page::Playlist(format!("smart:{id}")))
                                }
                                "files" => (covers::card(ui, &mut self.images, &covers::Cover::Files, w, &crate::i18n::t("Мои файлы"), &crate::i18n::t("С компьютера")), Page::Files),
                                // an album or a playlist on a service; the kind as the page wrote it ("Плейлист · SoundCloud")
                                _ => (
                                    covers::card(ui, &mut self.images, &covers::Cover::Playlist { icon: s.cover.as_deref(), tracks: &[] }, w, s.title.as_deref().unwrap_or(&crate::i18n::t("Плейлист")), &crate::i18n::t(s.kind.as_deref().unwrap_or("Плейлист"))),
                                    Page::Collection(id),
                                ),
                            };
                            if r.0.clicked() {
                                self.go(r.1);
                            }
                        }
                    }
                }
            });
        });
    }

    fn for_you(&mut self, ui: &mut Ui) {
        // seeds: the songs heard last and the newest likes
        let seeds: Vec<Track> = {
            let h = state::get().history.lock().unwrap();
            let l = state::get().library.read().unwrap();
            let mut s: Vec<Track> = h.entries.iter().take(2).map(|e| e.track.clone()).collect();
            for like in l.likes.iter().take(2) {
                if !s.iter().any(|x| x.id == like.track.id) {
                    s.push(like.track.clone());
                }
            }
            s
        };
        if seeds.is_empty() {
            return;
        }
        let key = seeds.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join("|");
        let resolver = self.resolver.clone();
        let data = self.pages.home.for_you.get(&key, move || recommendations(&resolver, &seeds)).cloned();
        // nothing found: the row is not shown at all, as on the page
        if matches!(&data, Some((mix, _)) if mix.is_empty()) {
            return;
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Рекомендации для вас")).font(theme::heavy(19.0)).color(INK));
            if let Some((mix, _)) = &data {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if widgets::btn(ui, Some("radio"), "Слушать подборку", false).clicked() {
                        self.player.play_queue(mix.clone(), 0, None);
                    }
                });
            }
        });
        App::muted(ui, "По тому, что вы слушаете и лайкаете");
        ui.add_space(6.0);
        match data {
            None => App::loading(ui),
            Some((mix, because)) => {
                self.card_row(ui, "foryou", &mix);
                for (seed, list) in because.iter() {
                    if list.is_empty() {
                        continue;
                    }
                    widgets::section(ui, &format!("Похоже на «{}»", seed.title));
                    self.card_row(ui, &format!("because-{}", seed.id), list);
                }
            }
        }
    }

    /// "Новое у ваших исполнителей": the artists met most in your likes and their newest songs
    /// you have not liked yet (newFromArtists.ts)
    fn new_from_artists(&mut self, ui: &mut Ui) {
        let likes: Vec<Track> = state::get().library.read().unwrap().likes.iter().map(|l| l.track.clone()).collect();
        if likes.is_empty() {
            return;
        }
        let key = likes.len().to_string();
        let Some(tracks) = self.pages.home.new_from.get(&key, move || new_from_artists(&likes)).cloned() else { return };
        if tracks.is_empty() {
            return;
        }
        widgets::section(ui, "Новое у ваших исполнителей");
        App::muted(ui, "Свежие треки тех, кого вы чаще всего лайкаете");
        ui.add_space(6.0);
        self.card_row(ui, "new-from", &tracks);
    }
}

impl App {
    /// "Собрано для вас": the smart playlists that have songs
    fn smart_row(&mut self, ui: &mut Ui) {
        let liked: Vec<Track> = state::get().library.read().unwrap().likes.iter().map(|l| l.track.clone()).collect();
        let lists: Vec<(&str, &str, f32, Vec<Track>)> = crate::stats::SMART.iter().map(|(k, title, _, hue)| (*k, *title, *hue, crate::stats::smart(k, &liked, 100))).filter(|l| !l.3.is_empty()).collect();
        if lists.is_empty() {
            return;
        }
        widgets::section(ui, "Собрано для вас");
        let w = 172.0;
        egui::ScrollArea::horizontal().id_salt("smart").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for (kind, title, hue, tracks) in &lists {
                    if self.smart_card(ui, w, *hue, &crate::i18n::t(title), &crate::i18n::tracks(tracks.len()), tracks).clicked() {
                        self.go(Page::Playlist(format!("smart:{kind}")));
                    }
                }
            });
        });
    }

    /// .smart-card: four covers in a square rounded 24 on the list's own colour, a glow from
    /// its light shade to its dark one, the sparkle at the bottom left; the name and a line
    pub fn smart_card(&mut self, ui: &mut Ui, w: f32, hue: f32, title: &str, sub: &str, tracks: &[Track]) -> egui::Response {
        let (rect, r) = ui.allocate_exact_size(vec2(w, w + 8.0 + 42.0), egui::Sense::click());
        let art = Rect::from_min_size(rect.min, vec2(w, w));
        let p = ui.painter().clone();
        // hsl(var(--hue) 60% 30%) behind, for the squares with no cover
        p.rect_filled(art, 24, hsl(hue, 0.60, 0.30, 1.0));
        for (i, t) in tracks.iter().take(4).enumerate() {
            let (col, row) = (i % 2, i / 2);
            let q = Rect::from_min_size(art.min + vec2(col as f32 * w / 2.0, row as f32 * w / 2.0), vec2(w / 2.0, w / 2.0));
            // only the square's outer corner follows the card's rounding
            let corner = CornerRadius { nw: if i == 0 { 24 } else { 0 }, ne: if i == 1 { 24 } else { 0 }, sw: if i == 2 { 24 } else { 0 }, se: if i == 3 { 24 } else { 0 } };
            let src = crate::ui::images::artwork_of(t);
            if let Some(id) = src.as_deref().and_then(|s| self.images.get(s, q.width(), ui.ctx().pixels_per_point())) {
                // saturate(0.9): a touch greyer
                egui::Image::new(egui::load::SizedTexture::new(id, q.size())).corner_radius(corner).tint(Color32::from_gray(238)).paint_at(ui, q);
            }
        }
        // .smart-glow: linear-gradient(160deg, hsl(h 80% 55% / .55), hsl(h 70% 12% / .85))
        p.add(covers::linear_gradient(art, 24.0, 160.0, &[(hsl(hue, 0.80, 0.55, 0.55), 0.0), (hsl(hue, 0.70, 0.12, 0.85), 1.0)]));
        crate::ui::icons::paint(&p, "sparkle", Rect::from_min_size(pos2(art.left() + 14.0, art.bottom() - 14.0 - 22.0), vec2(22.0, 22.0)), Color32::WHITE);
        if r.hovered() {
            p.rect_filled(art, 24, Color32::from_white_alpha(10));
        }
        let g = widgets::clipped(ui, title, theme::medium(14.0), INK, w);
        p.galley(pos2(rect.left(), art.bottom() + 8.0), g, INK);
        p.text(pos2(rect.left(), art.bottom() + 28.0), egui::Align2::LEFT_TOP, sub, theme::body(13.0), theme::ink2());
        r
    }

    /// "Ваш топ-10" with its period, and "Ваши топ-исполнители" beside it (under it in a
    /// narrow window, as the page's grid does below 1100 px)
    fn tops(&mut self, ui: &mut Ui) {
        if crate::stats::top_tracks(crate::stats::Range::All, 1).is_empty() {
            return;
        }
        ui.add_space(18.0);
        let mut range = self.pages.home.top_range;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Ваш топ-10")).font(theme::heavy(19.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if let Some(i) = super::kit::segmented(ui, &["Неделя", "Месяц", "Всё время"], range) {
                    range = i;
                }
            });
        });
        self.pages.home.top_range = range;
        ui.add_space(8.0);
        let r = [crate::stats::Range::Week, crate::stats::Range::Month, crate::stats::Range::All][range];
        let top = crate::stats::top_tracks(r, 10);
        if !top.is_empty() && top.len() < 10 {
            let n = top.len();
            let word = if n == 1 { "трек" } else if n < 5 { "трека" } else { "треков" };
            let lead = if range == 2 { "Пока" } else { "За этот период" };
            App::muted(ui, &format!("{} {} {n} {} — {}", crate::i18n::t(lead), crate::i18n::t("прослушано"), crate::i18n::t(word), crate::i18n::t("топ дополнится, когда послушаете больше (трек засчитывается после 5 секунд)")));
            ui.add_space(6.0);
        }
        if top.is_empty() {
            App::muted(ui, "За этот период ещё ничего не слушали.");
            return;
        }
        let artists = crate::stats::top_artists(r, 10);
        let full = ui.available_width();
        let wide = ui.ctx().content_rect().width() > 1100.0;
        // grid-template-columns: minmax(0, 1.6fr) minmax(240px, 1fr); gap 24
        let side = if wide { ((full - 24.0) / 2.6).max(240.0) } else { full };
        let main = if wide { full - 24.0 - side } else { full };
        let tracks: Vec<Track> = top.iter().map(|x| x.0.clone()).collect();
        let draw_list = |app: &mut App, ui: &mut Ui| {
            app.mark_kept();
            ui.set_width(main);
            let current = app.view.current().map(|t| t.id.clone());
            for (i, (t, n, _)) in top.iter().enumerate() {
                let liked = app.is_liked(&t.id);
                let note = crate::i18n::t(&plays_word(*n));
                let out = widgets::track_row(ui, &mut app.images, t, current.as_deref() == Some(t.id.as_str()), liked, Some(&note), Some(i + 1));
                if out.clicked || out.double {
                    app.play_list(&tracks, i, None);
                }
                if out.like {
                    app.toggle_like(t);
                }
                if out.menu {
                    app.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center()), playlist_item: None, queue_index: None, sub: false, files_page: false });
                }
            }
        };
        if wide {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 24.0;
                ui.allocate_ui_with_layout(vec2(main, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| draw_list(self, ui));
                ui.allocate_ui_with_layout(vec2(side, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| self.top_artists(ui, side, &artists));
            });
        } else {
            draw_list(self, ui);
            ui.add_space(24.0);
            self.top_artists(ui, side, &artists);
        }
    }

    /// .top-artists: a glass panel (padding 16, rounded 22) with the eyebrow and the artists:
    /// the place, a round picture, the name, plays and time
    fn top_artists(&mut self, ui: &mut Ui, width: f32, artists: &[crate::stats::TopArtist]) {
        egui::Frame::new().fill(theme::glass()).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(22).inner_margin(16).show(ui, |ui| {
            ui.set_width(width - 32.0 - 2.0);
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(egui::RichText::new(crate::i18n::t("Ваши топ-исполнители").to_uppercase()).font(theme::medium(12.0)).color(theme::ink3()).extra_letter_spacing(1.0));
            ui.add_space(4.0);
            for (i, a) in artists.iter().enumerate() {
                let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), egui::Sense::click());
                if r.hovered() {
                    ui.painter().rect_filled(rect, 14, theme::hover());
                }
                let p = ui.painter();
                p.text(pos2(rect.left() + 6.0 + 9.0, rect.center().y), egui::Align2::CENTER_CENTER, (i + 1).to_string(), theme::heavy(14.0), theme::ink3());
                let pic = Rect::from_min_size(pos2(rect.left() + 6.0 + 18.0 + 12.0, rect.center().y - 20.0), vec2(40.0, 40.0));
                match a.cover.as_deref().and_then(|s| self.images.get(s, 40.0, ui.ctx().pixels_per_point())) {
                    Some(id) => egui::Image::new(egui::load::SizedTexture::new(id, pic.size())).corner_radius(20).paint_at(ui, pic),
                    None => {
                        p.circle_filled(pic.center(), 20.0, Color32::from_rgba_unmultiplied(238, 247, 244, 26));
                        crate::ui::icons::paint(p, "user", Rect::from_center_size(pic.center(), vec2(18.0, 18.0)), theme::ink3());
                    }
                }
                let x = pic.right() + 12.0;
                let tw = rect.right() - 6.0 - x;
                let g = widgets::clipped(ui, &a.name, theme::medium(14.0), INK, tw);
                ui.painter().galley(pos2(x, rect.center().y - 18.0), g, INK);
                let line = format!("{} · {}", crate::i18n::t(&plays_word(a.plays)), crate::i18n::t(&crate::stats::format_time(a.ms)));
                let g = widgets::clipped(ui, &line, theme::body(12.5), theme::ink2(), tw);
                ui.painter().galley(pos2(x, rect.center().y + 2.0), g, theme::ink2());
                // the artist's page, for the artists a service knows
                if r.clicked() && (a.id.starts_with("scu:") || a.id.starts_with("yta:")) {
                    self.go(Page::Artist(a.id.clone()));
                }
            }
        });
    }
}

/// "1 прослушивание", "3 прослушивания", "5 прослушиваний"
fn plays_word(n: u32) -> String {
    let (m10, m100) = (n % 10, n % 100);
    let w = if m10 == 1 && m100 != 11 { "прослушивание" } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) { "прослушивания" } else { "прослушиваний" };
    format!("{n} {w}")
}

/// the artists met most in the likes (only those a service knows), as topLikedArtists
pub fn top_liked_artists(likes: &[Track], limit: usize) -> Vec<(String, String)> {
    let mut counts: Vec<(String, String, usize)> = Vec::new();
    for t in likes {
        for a in &t.artists {
            if !(a.id.starts_with("scu:") || a.id.starts_with("yta:")) {
                continue;
            }
            match counts.iter_mut().find(|c| c.0 == a.id) {
                Some(c) => c.2 += 1,
                None => counts.push((a.id.clone(), a.name.clone(), 1)),
            }
        }
    }
    // a stable sort: ties keep the order they were met in
    counts.sort_by(|a, b| b.2.cmp(&a.2));
    counts.into_iter().take(limit).map(|c| (c.0, c.1)).collect()
}

/// the answer kept half an hour, unless "Ответы сервисов" are not to be kept
static NEW_FROM: Mutex<Option<(String, Instant, Vec<Track>)>> = Mutex::new(None);
const NEW_FROM_TTL: Duration = Duration::from_secs(30 * 60);

fn new_from_artists(likes: &[Track]) -> Vec<Track> {
    const MAX_ARTISTS: usize = 8;
    const PER_ARTIST: usize = 2;
    let artists = top_liked_artists(likes, MAX_ARTISTS);
    let key = artists.iter().map(|a| a.0.as_str()).collect::<Vec<_>>().join(",");
    if let Some((k, at, tracks)) = NEW_FROM.lock().unwrap().as_ref() {
        if *k == key && at.elapsed() < NEW_FROM_TTL {
            return tracks.clone();
        }
    }
    let liked: std::collections::HashSet<&str> = likes.iter().map(|t| t.id.as_str()).collect();
    let groups: Vec<Vec<Track>> = std::thread::scope(|s| {
        let hs: Vec<_> = artists
            .iter()
            .map(|(id, _)| {
                s.spawn(move || {
                    let tracks = if id.starts_with("scu:") { crate::services::soundcloud::user(id).map(|u| u.tracks) } else { crate::services::youtube::artist(id).map(|a| a.tracks) };
                    tracks.unwrap_or_default()
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or_default()).map(|g| g.into_iter().filter(|t| !liked.contains(t.id.as_str())).take(PER_ARTIST).collect()).collect()
    });
    // take turns so one busy artist does not fill the whole row
    let mut out = Vec::new();
    for i in 0..PER_ARTIST {
        for g in &groups {
            if let Some(t) = g.get(i) {
                out.push(t.clone());
            }
        }
    }
    if !state::get().settings.read().unwrap().no_service_cache {
        *NEW_FROM.lock().unwrap() = Some((key, Instant::now(), out.clone()));
    }
    out
}

/// a mix of songs similar to the seeds, and a row per seed
fn recommendations(r: &crate::player::resolve::Resolver, seeds: &[Track]) -> (Vec<Track>, Vec<(Track, Vec<Track>)>) {
    use crate::player::resolve::Resolve;
    let st = state::get();
    let mut exclude: std::collections::HashSet<String> = st.library.read().unwrap().likes.iter().map(|l| l.track.id.clone()).collect();
    exclude.extend(st.history.lock().unwrap().entries.iter().map(|e| e.track.id.clone()));
    let rows: Vec<(Track, Vec<Track>)> = std::thread::scope(|s| {
        let hs: Vec<_> = seeds.iter().map(|seed| {
            let ex = &exclude;
            s.spawn(move || (seed.clone(), r.similar(seed, ex)))
        }).collect();
        hs.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut mix = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for i in 0..25 {
        for (_, list) in &rows {
            if let Some(t) = list.get(i) {
                if seen.insert(crate::player::resolve::song_key(t)) {
                    mix.push(t.clone());
                }
            }
        }
    }
    mix.truncate(30);
    (mix, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Artist;

    fn song(id: &str, artists: &[&str]) -> Track {
        Track { id: id.into(), artists: artists.iter().map(|a| Artist { id: a.to_string(), name: a.to_string() }).collect(), ..Default::default() }
    }

    #[test]
    fn counts_only_the_artists_a_service_knows() {
        let likes = [song("1", &["scu:1", "local-artist:x"]), song("2", &["yta:2"]), song("3", &["yta:2"])];
        assert_eq!(top_liked_artists(&likes, 8).iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), ["yta:2", "scu:1"]);
    }
}
