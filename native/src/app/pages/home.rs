//! Главная: the greeting, "Недавно слушали" (playlists, albums and songs), recommendations
//! from what you play and like, and what is trending on SoundCloud (src/pages/HomePage.tsx).

use egui::Ui;

use super::Loaded;
use crate::app::{App, Page};
use crate::model::Track;
use crate::state;
use crate::ui::theme::{self, ic};
use crate::ui::widgets;

#[derive(Default)]
pub struct HomeState {
    for_you: Loaded<(Vec<Track>, Vec<(Track, Vec<Track>)>)>,
    trending: Loaded<Vec<Track>>,
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
            widgets::section(ui, "В тренде на SoundCloud");
            let trending = self.pages.home.trending.get("trending", || crate::services::soundcloud::charts().unwrap_or_default()).cloned();
            match trending {
                Some(list) if !list.is_empty() => self.card_row(ui, "trending", &list),
                Some(_) => App::muted(ui, "SoundCloud сейчас не отвечает"),
                None => App::loading(ui),
            }
            ui.add_space(24.0);
        });
    }

    fn recent(&mut self, ui: &mut Ui) {
        let (sources, entries) = {
            let h = state::get().history.lock().unwrap();
            (h.sources.clone(), h.entries.clone())
        };
        if sources.is_empty() && entries.is_empty() {
            return;
        }
        widgets::section(ui, "Недавно слушали");
        let w = 172.0;
        let lib = state::get().library.read().unwrap().clone();
        egui::ScrollArea::horizontal().id_salt("recent").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                // playlists and albums first, then songs, newest first
                let mut shown = 0;
                for s in sources.iter().take(6) {
                    let (title, sub, cover, page) = match s.source.page.as_str() {
                        "playlist" => {
                            let id = s.source.id.clone().unwrap_or_default();
                            if id == "likes" {
                                (crate::i18n::t("Любимое"), crate::i18n::tracks(lib.likes.len()), lib.likes.first().and_then(|l| crate::ui::images::artwork_of(&l.track)), Page::Playlist(id))
                            } else if let Some(p) = lib.playlists.iter().find(|p| p.id == id) {
                                let cover = p.icon.clone().or_else(|| p.items.first().and_then(|i| crate::ui::images::artwork_of(&i.track)));
                                (p.title.clone(), format!("{} · {}", crate::i18n::t("Плейлист"), crate::i18n::tracks(p.items.len())), cover, Page::Playlist(id))
                            } else {
                                continue;
                            }
                        }
                        "collection" => {
                            let id = s.source.id.clone().unwrap_or_default();
                            // the kind is kept in the history as the page wrote it ("Плейлист · SoundCloud")
                            (s.source.title.clone().unwrap_or_default(), crate::i18n::t(&s.source.kind.clone().unwrap_or_default()), s.source.cover.clone(), Page::Collection(id))
                        }
                        "files" => (crate::i18n::t("Мои файлы"), String::new(), None, Page::Files),
                        _ => continue,
                    };
                    if widgets::card(ui, &mut self.images, cover.as_deref(), &title, &sub, w, false).clicked() {
                        self.go(page);
                    }
                    shown += 1;
                }
                let tracks: Vec<Track> = entries.iter().map(|e| e.track.clone()).take(12usize.saturating_sub(shown)).collect();
                for (i, t) in tracks.iter().enumerate() {
                    let src = crate::ui::images::artwork_of(t);
                    let r = widgets::card(ui, &mut self.images, src.as_deref(), &t.title, &t.artist_line(), w, false);
                    if r.clicked() {
                        self.play_list(&tracks, i, None);
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
        widgets::section(ui, "Рекомендации для вас");
        match data {
            None => App::loading(ui),
            Some((mix, because)) => {
                if !mix.is_empty() {
                    ui.horizontal(|ui| {
                        App::muted(ui, "По тому, что вы слушаете и лайкаете");
                        ui.add_space(12.0);
                        if widgets::pill_button(ui, Some(ic::RADIO), "Слушать подборку", false).clicked() {
                            self.player.play_queue(mix.clone(), 0, None);
                        }
                    });
                    ui.add_space(6.0);
                    self.card_row(ui, "foryou", &mix);
                }
                for (seed, list) in because.iter().take(2) {
                    if list.is_empty() {
                        continue;
                    }
                    widgets::section(ui, &format!("Похоже на «{}»", seed.title));
                    self.card_row(ui, &format!("because-{}", seed.id), list);
                }
            }
        }
        let _ = theme::INK;
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
                    let (rect, r) = ui.allocate_exact_size(egui::vec2(w, w + 46.0), egui::Sense::click());
                    let art = egui::Rect::from_min_size(rect.min, egui::vec2(w, w));
                    // four covers in a square, tinted with the list's own colour
                    for (i, t) in tracks.iter().take(4).enumerate() {
                        let q = egui::Rect::from_min_size(art.min + egui::vec2((i % 2) as f32 * w / 2.0, (i / 2) as f32 * w / 2.0), egui::vec2(w / 2.0, w / 2.0));
                        let src = crate::ui::images::artwork_of(t);
                        widgets::paint_cover(ui, &mut self.images, src.as_deref(), q, 0);
                    }
                    // a dark wash in the list's own colour (Hsva is linear light: 0.05 is about a quarter on screen), so the star and the covers' mix read as one card
                    let [cr, cg, cb] = egui::ecolor::Hsva::new(hue / 360.0, 0.7, 0.05, 1.0).to_srgb();
                    ui.painter().rect_filled(art, 16, egui::Color32::from_rgba_unmultiplied(cr, cg, cb, 180));
                    crate::ui::theme::paint_icon(&ui.painter(), art.center(), egui::Align2::CENTER_CENTER, "@sparkle", 26.0, crate::ui::theme::INK);
                    let g = widgets::clipped(ui, &crate::i18n::t(title), theme::medium(14.0), crate::ui::theme::INK, w);
                    ui.painter().galley(egui::pos2(rect.left(), art.bottom() + 8.0), g, crate::ui::theme::INK);
                    ui.painter().text(egui::pos2(rect.left(), art.bottom() + 28.0), egui::Align2::LEFT_TOP, crate::i18n::tracks(tracks.len()), theme::body(13.0), theme::ink2());
                    if r.clicked() {
                        self.go(Page::Playlist(format!("smart:{kind}")));
                    }
                }
            });
        });
    }

    /// "Ваш топ-10" with its period
    fn tops(&mut self, ui: &mut Ui) {
        if crate::stats::top_tracks(crate::stats::Range::All, 1).is_empty() {
            return;
        }
        ui.add_space(18.0);
        let mut range = self.pages.home.top_range;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Ваш топ-10")).font(theme::heavy(19.0)));
            ui.add_space(12.0);
            if let Some(i) = widgets::tabs(ui, &["Неделя", "Месяц", "Всё время"], range) {
                range = i;
            }
        });
        self.pages.home.top_range = range;
        ui.add_space(8.0);
        let r = [crate::stats::Range::Week, crate::stats::Range::Month, crate::stats::Range::All][range];
        let top = crate::stats::top_tracks(r, 10);
        if top.is_empty() {
            App::muted(ui, "За этот период ещё ничего не слушали.");
            return;
        }
        let tracks: Vec<Track> = top.iter().map(|x| x.0.clone()).collect();
        let current = self.view.current().map(|t| t.id.clone());
        for (i, (t, n, _)) in top.iter().enumerate() {
            let liked = self.is_liked(&t.id);
            let note = crate::i18n::t(&plays_word(*n));
            let out = widgets::track_row(ui, &mut self.images, t, current.as_deref() == Some(t.id.as_str()), liked, Some(&note), Some(i + 1));
            if out.clicked || out.double {
                self.play_list(&tracks, i, None);
            }
            if out.like {
                self.toggle_like(t);
            }
            if out.menu {
                self.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(out.response.rect.center()), playlist_item: None });
            }
        }
    }
}

/// "1 прослушивание", "3 прослушивания", "5 прослушиваний"
fn plays_word(n: u32) -> String {
    let (m10, m100) = (n % 10, n % 100);
    let w = if m10 == 1 && m100 != 11 { "прослушивание" } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) { "прослушивания" } else { "прослушиваний" };
    format!("{n} {w}")
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
