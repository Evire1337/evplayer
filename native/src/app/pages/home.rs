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
            self.page_title(ui, greeting());
            self.recent(ui);
            self.for_you(ui);
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
                            (s.source.title.clone().unwrap_or_default(), s.source.kind.clone().unwrap_or_default(), s.source.cover.clone(), Page::Collection(id))
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
