//! Мои файлы: songs from this computer (src/pages/FilesPage.tsx). The index, tags and covers
//! are the library of src-tauri/src/library.rs, the same files the Tauri build keeps, so the
//! songs added there are here too. Folders are watched (App::watcher): new files show up and
//! deleted ones go by themselves. The page as the Tauri build draws it: the hero with "Слушать
//! всё" and the two ways to add, the watched folders, the filter, each file's format, and a
//! file's own menu entry to take it out of EvPlayer.

use egui::{pos2, vec2, Rect, Ui};

use crate::app::App;
use crate::model::{Artist, Source, Track};
use crate::store::state::PlaySource;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

/// a file of the library as a song (recordToTrack): a file with no artist in its tags is
/// "Неизвестный исполнитель", as the page shows it
pub fn track_of(r: &crate::library::LocalTrackRecord) -> Track {
    let names: Vec<String> = if r.artists.is_empty() { vec![crate::i18n::t("Неизвестный исполнитель")] } else { r.artists.clone() };
    Track {
        id: r.id.clone(),
        source: Source::Local,
        title: r.title.clone(),
        artists: names.into_iter().map(|a| Artist { id: format!("local-artist:{}", a.to_lowercase()), name: a }).collect(),
        duration_ms: r.duration_ms as f64,
        artwork: None,
        plays: None,
        permalink: None,
        preview: false,
        explicit: false,
    }
}

/// "FLAC · 24 бит / 96 кГц", "MP3 · 320 кбит/с" (formatLabel)
pub fn format_label(r: &crate::library::LocalTrackRecord) -> String {
    let mut parts = Vec::new();
    if let Some(c) = &r.codec {
        parts.push(if c.eq_ignore_ascii_case("MPEG 1 Layer 3") { "MP3".to_string() } else { c.to_uppercase() });
    }
    match (r.lossless, r.bits_per_sample, r.sample_rate, r.bitrate) {
        (Some(true), Some(bits), Some(rate), _) => parts.push(format!("{bits} {} / {} {}", crate::i18n::t("бит"), (rate as f64 / 100.0).round() / 10.0, crate::i18n::t("кГц"))),
        (_, _, _, Some(b)) => parts.push(format!("{} {}", (b as f64 / 1000.0).round(), crate::i18n::t("кбит/с"))),
        _ => {}
    }
    parts.join(" · ")
}

impl App {
    pub fn files(&mut self, ui: &mut Ui) {
        let records = self.library.list();
        let folders = self.library.folders();
        let all: Vec<Track> = records.iter().map(track_of).collect();
        let source = PlaySource { page: "files".into(), id: None, title: None, cover: None, kind: None };

        // files dragged over the window: the page lights up where they will land (.is-dragging)
        if self.dragging_files {
            let r = ui.max_rect();
            ui.painter().rect(r.shrink(2.0), 20, egui::Color32::from_rgba_unmultiplied(111, 240, 196, 18), egui::Stroke::new(2.0, theme::ACCENT), egui::StrokeKind::Inside);
        }

        // .pl-hero: the cover (210), the eyebrow, the name, what it holds, the buttons
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let art = 210.0;
            let (rect, _) = ui.allocate_exact_size(vec2(art, art), egui::Sense::hover());
            // .files-cover with the music sign, 64
            ui.painter().add(super::covers::linear_gradient(rect, super::covers::RADIUS, 150.0, &[(super::covers::rgba(0x8ab4ff, 1.0), 0.0), (super::covers::rgba(0x23306b, 1.0), 0.75)]));
            crate::ui::icons::paint(ui.painter(), "music", Rect::from_center_size(rect.center(), vec2(64.0, 64.0)), egui::Color32::WHITE);
            ui.add_space(28.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.add_space((art - (4.0 * 26.0 + 56.0)).max(0.0));
                ui.label(egui::RichText::new(crate::i18n::t("Ваши файлы").to_uppercase()).font(theme::medium(12.0)).color(theme::ink3()).extra_letter_spacing(1.0));
                ui.label(egui::RichText::new(crate::i18n::t("Мои файлы")).font(theme::heavy(46.0)).color(INK));
                let count = if records.is_empty() { crate::i18n::t("Пока пусто") } else { crate::i18n::tracks(records.len()) };
                ui.label(egui::RichText::new(format!("{count} · {}", crate::i18n::t("MP3, FLAC, M4A/AAC, OGG/Opus, WAV. Файлы остаются на месте — EvPlayer только читает их."))).font(theme::body(13.0)).color(theme::ink2()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::round_button(ui, ic::PLAY, 52.0, widgets::Round::Main, false, "Слушать всё").clicked() && !all.is_empty() {
                        self.player.play_queue(all.clone(), 0, Some(source.clone()));
                    }
                    if widgets::btn(ui, None, "Добавить файлы", false).clicked() {
                        let files = crate::dialogs::pick_files();
                        self.add_local(files);
                    }
                    if widgets::btn(ui, Some("folder"), "Добавить папку", false).clicked() {
                        // the folder itself, so it is remembered and watched
                        if let Some(dir) = crate::dialogs::pick_folder() {
                            self.add_local(vec![dir]);
                        }
                    }
                });
            });
        });
        ui.add_space(24.0);

        // the folders watched, each with "Перестать следить"
        if !folders.is_empty() {
            widgets::section(ui, "Папки, за которыми следит EvPlayer");
            App::muted(ui, "Новые файлы в этих папках появляются сами, удалённые — исчезают.");
            ui.add_space(8.0);
            let mut unwatch = None;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for f in &folders {
                    if folder_chip(ui, f) {
                        unwatch = Some(f.clone());
                    }
                }
            });
            if let Some(f) = unwatch {
                self.unwatch_folder(&f);
            }
            ui.add_space(14.0);
        }

        if records.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| App::muted(ui, "Перетащите сюда файлы или папку с музыкой."));
            return;
        }

        // .pl-tools: the filter
        let mut filter = std::mem::take(&mut self.pages.files_filter);
        widgets::filter_input(ui, &mut filter, "Поиск по файлам", 260.0);
        self.pages.files_filter = filter;
        ui.add_space(12.0);
        let q = self.pages.files_filter.trim().to_lowercase();
        let shown: Vec<usize> = (0..all.len()).filter(|&i| q.is_empty() || format!("{} {}", all[i].title, all[i].artist_line()).to_lowercase().contains(&q)).collect();
        let tracks: Vec<Track> = shown.iter().map(|&i| all[i].clone()).collect();
        let labels: Vec<String> = shown.iter().map(|&i| format_label(&records[i])).collect();
        let before = self.menu.is_none();
        let extra = move |i: usize| labels.get(i).cloned().filter(|s| !s.is_empty());
        self.track_list(ui, &tracks, Some(source), None, &extra, false);
        // a file's menu opened here offers "Убрать из EvPlayer (файл останется)" too
        if before {
            if let Some(m) = self.menu.as_mut() {
                m.files_page = true;
            }
        }
    }
}

impl App {
    /// "Изменить теги и обложку": the dialog filled from the file's record
    pub fn open_tag_editor(&mut self, t: &Track) {
        let rec = self.library.get(&t.id);
        let title = rec.as_ref().map(|r| r.title.clone()).unwrap_or_else(|| t.title.clone());
        let artists = rec.as_ref().map(|r| r.artists.clone()).unwrap_or_else(|| t.artists.iter().map(|a| a.name.clone()).collect()).join(", ");
        let album = rec.and_then(|r| r.album).unwrap_or_default();
        self.dialog = Some(crate::app::Dialog::Tags { id: t.id.clone(), title, artists, album, cover: None });
    }

    /// Saves the edit in EvPlayer's own index (the file on disk is not touched), and the
    /// copies of the song kept in the likes, the playlists and the queue show it too
    /// (syncEverywhere in tagEditor.tsx).
    pub fn save_tags(&mut self, id: &str, patch: crate::library::LocalTrackEdit) {
        let new_cover = patch.cover.is_some();
        let Some(rec) = self.library.update(id, patch) else { return };
        let fresh = track_of(&rec);
        crate::state::get().update_library(|l| {
            for like in l.likes.iter_mut().filter(|x| x.track.id == fresh.id) {
                like.track = fresh.clone();
            }
            for p in l.playlists.iter_mut() {
                for item in p.items.iter_mut().filter(|x| x.track.id == fresh.id) {
                    item.track = fresh.clone();
                }
            }
        });
        self.player.refresh_track(&fresh);
        // the old cover is held under the same address: drop the pictures so it is read anew
        if new_cover {
            self.images.release_all();
        }
        self.toast("Сохранено");
    }
}

/// .folder-chip: the folder sign, its path (cut to fit, whole on hover), the bin; true when
/// the bin was pressed
fn folder_chip(ui: &mut Ui, path: &str) -> bool {
    let g = widgets::clipped(ui, path, theme::body(13.0), theme::ink2(), 420.0);
    let size = vec2(10.0 + 15.0 + 8.0 + g.size().x + 8.0 + 26.0 + 4.0, 34.0);
    let (rect, r) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 12, egui::Color32::from_rgba_unmultiplied(238, 247, 244, 15));
    crate::ui::icons::paint(ui.painter(), "folder", Rect::from_center_size(pos2(rect.left() + 10.0 + 7.5, rect.center().y), vec2(15.0, 15.0)), theme::ink2());
    ui.painter().galley(pos2(rect.left() + 33.0, rect.center().y - g.size().y / 2.0), g, theme::ink2());
    r.on_hover_text(path);
    let bin = Rect::from_center_size(pos2(rect.right() - 4.0 - 13.0, rect.center().y), vec2(26.0, 26.0));
    let b = ui.interact(bin, ui.scope_id().with(("unwatch", path)), egui::Sense::click());
    if b.hovered() {
        ui.painter().rect_filled(bin, 8, theme::hover());
    }
    crate::ui::icons::paint(ui.painter(), "trash", Rect::from_center_size(bin.center(), vec2(15.0, 15.0)), if b.hovered() { INK } else { theme::ink3() });
    b.on_hover_text(crate::i18n::t("Перестать следить")).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> crate::library::LocalTrackRecord {
        serde_json::from_value(serde_json::json!({ "id": "local:x", "path": "C:\\m\\a.flac", "title": "A", "artists": [], "durationMs": 1000, "size": 1, "addedAt": "2026-01-01T00:00:00Z" })).unwrap()
    }

    #[test]
    fn names_the_format_as_the_page_does() {
        let mut r = record();
        r.codec = Some("FLAC".into());
        r.lossless = Some(true);
        r.bits_per_sample = Some(24);
        r.sample_rate = Some(96_000);
        assert_eq!(format_label(&r), "FLAC · 24 бит / 96 кГц");
        r.codec = Some("MPEG 1 Layer 3".into());
        r.lossless = Some(false);
        r.bitrate = Some(320_000);
        assert_eq!(format_label(&r), "MP3 · 320 кбит/с");
    }

    #[test]
    fn a_file_without_artist_is_unknown() {
        let t = track_of(&record());
        assert_eq!(t.artists[0].name, "Неизвестный исполнитель");
        assert_eq!(t.artists[0].id, "local-artist:неизвестный исполнитель");
    }
}
