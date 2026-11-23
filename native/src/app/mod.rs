//! The player's interface and what holds it together.

pub mod actions;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use winit::window::Window;

use actions::Action;

static RENDERER: OnceLock<&'static str> = OnceLock::new();

/// what draws the window ("Direct3D 11" or "процессор"), for Настройки → Диагностика
pub fn set_renderer_name(name: &'static str) {
    let _ = RENDERER.set(name);
}

pub fn renderer_name() -> &'static str {
    RENDERER.get().copied().unwrap_or("—")
}

pub struct App {
    pub data_dir: PathBuf,
    started: Instant,
    first_frame: Option<std::time::Duration>,
    hide: bool,
    show: bool,
}

impl App {
    pub fn new(_ctx: &egui::Context, data_dir: PathBuf, started: Instant) -> Self {
        Self { data_dir, started, first_frame: None, hide: false, show: false }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if self.first_frame.is_none() {
            self.first_frame = Some(self.started.elapsed());
        }
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("EvPlayer");
            ui.label(format!("Отрисовка: {}", renderer_name()));
            if let Some(t) = self.first_frame {
                ui.label(format!("Первый кадр через {} мс", t.as_millis()));
            }
        });
    }

    /// for --snapshot: open a page by its name
    pub fn open_page_by_name(&mut self, _name: &str) {}

    pub fn attach_window(&mut self, _window: &Window) {}
    pub fn starts_hidden(&self) -> bool {
        false
    }
    pub fn window_shown(&mut self, _shown: bool) {}
    pub fn window_minimized(&mut self) {}
    pub fn close_to_tray(&self) -> bool {
        false
    }
    pub fn quit(&mut self) {}
    pub fn dropped_file(&mut self, _path: PathBuf) {}
    pub fn action(&mut self, _a: Action) {}
    pub fn open_link(&mut self, _link: &str) {}
    pub fn wants_hide(&mut self) -> bool {
        std::mem::take(&mut self.hide)
    }
    pub fn wants_show(&mut self) -> bool {
        std::mem::take(&mut self.show)
    }
}
