//! Мои файлы: songs from this computer (src/pages/FilesPage.tsx). The index, tags and covers
//! are the library of src-tauri/src/library.rs, the same files the Tauri build keeps, so the
//! songs added there are here too. Folders are watched: new files show up by themselves.

use egui::Ui;

use crate::app::App;
use crate::model::{Artist, Source, Track};
use crate::store::state::PlaySource;
use crate::ui::theme::ic;
use crate::ui::widgets;

/// a file of the library as a song
pub fn track_of(r: &crate::library::LocalTrackRecord) -> Track {
    Track {
        id: r.id.clone(),
        source: Source::Local,
        title: r.title.clone(),
        artists: r.artists.iter().map(|a| Artist { id: format!("local-artist:{a}"), name: a.clone() }).collect(),
        duration_ms: r.duration_ms as f64,
        artwork: None,
        plays: None,
        permalink: None,
        preview: false,
        explicit: false,
    }
}

impl App {
    pub fn files(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Мои файлы")).font(crate::ui::theme::heavy(40.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if widgets::pill_button(ui, Some(ic::FOLDER), "Добавить папку", true).clicked() {
                    if let Some(dir) = crate::dialogs::pick_folder() {
                        let lib = self.library.clone();
                        std::thread::spawn(move || {
                            let mut files = Vec::new();
                            crate::library::Library::collect_audio_files(&[dir], 6, &mut files);
                            let _ = lib.add(&files);
                            crate::events::repaint();
                        });
                    }
                }
                if widgets::pill_button(ui, Some(ic::ADD), "Добавить файлы", false).clicked() {
                    let files = crate::dialogs::pick_files();
                    if !files.is_empty() {
                        let lib = self.library.clone();
                        std::thread::spawn(move || {
                            let _ = lib.add(&files);
                            crate::events::repaint();
                        });
                    }
                }
            });
        });
        App::muted(ui, "MP3, FLAC, M4A/AAC, OGG/Opus, WAV. Файлы остаются на месте — EvPlayer только читает их.");
        ui.add_space(10.0);
        let tracks: Vec<Track> = self.library.list().iter().map(track_of).collect();
        if tracks.is_empty() {
            App::muted(ui, "Пока пусто");
            return;
        }
        let source = PlaySource { page: "files".into(), id: None, title: None, cover: None, kind: None };
        self.track_list(ui, &tracks, Some(source), None, &|_| None, false);
    }
}
