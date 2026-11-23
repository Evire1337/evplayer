//! Библиотека: "Любимое", your files and your playlists as cards (src/pages/LibraryPage.tsx),
//! and a playlist of yours with its songs (PlaylistPage.tsx).

use egui::{pos2, vec2, Align2, Rect, Ui};

use crate::app::{App, Dialog, Page};
use crate::model::Track;
use crate::state;
use crate::store::state::PlaySource;
use crate::ui::images::artwork_of;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

/// "30 мин назад", "2 ч назад", "вчера", "22 нояб."
pub fn ago(iso: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else { return String::new() };
    let secs = (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds().max(0);
    let en = crate::i18n::is_english();
    match secs {
        0..=59 => (if en { "just now" } else { "только что" }).into(),
        60..=3599 => if en { format!("{} min ago", secs / 60) } else { format!("{} мин назад", secs / 60) },
        3600..=86_399 => if en { format!("{} hr ago", secs / 3600) } else { format!("{} ч назад", secs / 3600) },
        86_400..=172_799 => (if en { "yesterday" } else { "вчера" }).into(),
        _ => date(iso),
    }
}

pub fn date(iso: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else { return String::new() };
    let t = t.with_timezone(&chrono::Local);
    use chrono::Datelike;
    const RU: [&str; 12] = ["янв.", "февр.", "мар.", "апр.", "мая", "июн.", "июл.", "авг.", "сент.", "окт.", "нояб.", "дек."];
    const EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let m = t.month0() as usize;
    if crate::i18n::is_english() {
        format!("{} {} {}", t.day(), EN[m], t.year())
    } else {
        format!("{} {} {}", t.day(), RU[m], t.year())
    }
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
            });
        });
        ui.add_space(12.0);
        let lib = state::get().library.read().unwrap().clone();
        let files = self.library.list();
        let w = 172.0;
        egui::ScrollArea::vertical().id_salt("library").auto_shrink([false, false]).show(ui, |ui| {
            let per_row = (((ui.available_width() - 8.0) + 16.0) / (w + 16.0)).floor().max(1.0) as usize;
            let mut cards: Vec<(Option<String>, String, String, Page, &str)> = vec![
                (None, crate::i18n::t("Любимое"), crate::i18n::tracks(lib.likes.len()), Page::Playlist("likes".into()), ic::HEART_FILL),
                (None, crate::i18n::t("Мои файлы"), crate::i18n::t("MP3, FLAC и другие с этого компьютера"), Page::Files, ic::FOLDER),
            ];
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
                            ui.painter().text(art.center(), Align2::CENTER_CENTER, *icon, theme::icon(34.0), egui::Color32::from_rgb(10, 30, 30));
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

    pub fn playlist(&mut self, ui: &mut Ui, id: &str) {
        let lib = state::get().library.read().unwrap().clone();
        let (title, sub, cover, tracks, items, created): (String, String, Option<String>, Vec<Track>, Vec<String>, String) = if id == "likes" {
            let tracks: Vec<Track> = lib.likes.iter().map(|l| l.track.clone()).collect();
            let items = tracks.iter().map(|t| t.id.clone()).collect();
            (crate::i18n::t("Любимое"), crate::i18n::tracks(tracks.len()), None, tracks, items, String::new())
        } else if let Some(p) = lib.playlists.iter().find(|p| p.id == id) {
            let tracks: Vec<Track> = p.items.iter().map(|i| i.track.clone()).collect();
            let cover = p.icon.clone().or_else(|| p.items.first().and_then(|i| artwork_of(&i.track)));
            (p.title.clone(), format!("{} · {} {}", crate::i18n::tracks(tracks.len()), crate::i18n::t("создан"), date(&p.created_at)), cover, tracks, p.items.iter().map(|i| i.id.clone()).collect(), p.created_at.clone())
        } else {
            App::muted(ui, "Плейлист не найден");
            return;
        };
        let _ = created;
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let art = 200.0;
            if id == "likes" {
                let (rect, _) = ui.allocate_exact_size(vec2(art, art), egui::Sense::hover());
                gradient(ui, rect, egui::Color32::from_rgb(92, 214, 172), egui::Color32::from_rgb(40, 120, 140));
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, ic::HEART_FILL, theme::icon(56.0), egui::Color32::from_rgb(10, 30, 30));
            } else {
                widgets::cover(ui, &mut self.images, cover.as_deref(), art, 18);
            }
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.add_space(28.0);
                ui.label(egui::RichText::new(crate::i18n::t("ПЛЕЙЛИСТ")).font(theme::medium(11.0)).color(theme::ink3()));
                ui.label(egui::RichText::new(&title).font(theme::heavy(40.0)).color(INK));
                App::muted(ui, &sub);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let source = PlaySource { page: "playlist".into(), id: Some(id.to_string()), title: None, cover: None, kind: None };
                    if widgets::pill_button(ui, Some(ic::PLAY), "Играть", true).clicked() && !tracks.is_empty() {
                        self.player.play_queue(tracks.clone(), 0, Some(source.clone()));
                    }
                    if widgets::pill_button(ui, Some(ic::SHUFFLE), "Вперемешку", false).clicked() && !tracks.is_empty() {
                        let shuffled = crate::player::queue::shuffled(&tracks, None);
                        self.player.play_queue(shuffled, 0, Some(source));
                    }
                    if id != "likes" {
                        if widgets::icon_button(ui, ic::EDIT, 34.0, false, "Изменить").clicked() {
                            let d = lib.playlists.iter().find(|p| p.id == id).and_then(|p| p.description.clone()).unwrap_or_default();
                            self.dialog = Some(Dialog::Rename { id: id.to_string(), title: title.clone(), description: d });
                        }
                        if widgets::icon_button(ui, ic::DELETE, 34.0, false, "Удалить плейлист").clicked() {
                            self.dialog = Some(Dialog::ConfirmDelete { id: id.to_string(), title: title.clone() });
                        }
                    }
                });
            });
        });
        ui.add_space(14.0);
        let filter_w = 260.0;
        let mut filter = std::mem::take(&mut self.pages.playlist_filter);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text(crate::i18n::t("Поиск в плейлисте")).desired_width(filter_w));
        });
        self.pages.playlist_filter = filter.clone();
        ui.add_space(6.0);
        if tracks.is_empty() {
            App::muted(ui, "Здесь пока пусто — добавляйте песни из поиска через «…» или ♥");
            return;
        }
        let f = crate::player::matching::simplify(&filter);
        let (shown, shown_items): (Vec<Track>, Vec<String>) = if f.is_empty() {
            (tracks.clone(), items.clone())
        } else {
            tracks.iter().zip(items.iter()).filter(|(t, _)| crate::player::matching::simplify(&format!("{} {}", t.title, t.artist_line())).contains(&f)).map(|(t, i)| (t.clone(), i.clone())).unzip()
        };
        let added: Vec<String> = if id == "likes" {
            lib.likes.iter().map(|l| ago(&l.added_at)).collect()
        } else {
            lib.playlists.iter().find(|p| p.id == id).map(|p| p.items.iter().map(|i| ago(&i.added_at)).collect()).unwrap_or_default()
        };
        let index_of: Vec<usize> = shown_items.iter().map(|s| items.iter().position(|x| x == s).unwrap_or(0)).collect();
        let source = PlaySource { page: "playlist".into(), id: Some(id.to_string()), title: None, cover: None, kind: None };
        let extra = move |i: usize| index_of.get(i).and_then(|j| added.get(*j)).cloned();
        self.track_list(ui, &shown, Some(source), Some((id, &shown_items)), &extra);
    }
}

/// a rounded card filled with a diagonal blend of two colours (the "Любимое" and files cards)
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
