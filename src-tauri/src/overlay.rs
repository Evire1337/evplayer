//! The mini player: a small always-on-top window with the cover, the title and the basic
//! buttons. It is a separate small page (overlay.html) that only shows what the main window
//! tells it, and sends its button presses back.
//!
//! Games put themselves on top too and Windows lets the newest one win; "show desktop" and
//! resolution changes also hide or move small tool windows. While the mini player should be
//! visible, a watchdog checks it every 1.5 s and brings it back: shown, on a screen, on top.

use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalRect, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "overlay";
/// logical size: a slim bar that covers nothing important in a game
pub const WIDTH: f64 = 330.0;
pub const HEIGHT: f64 = 52.0;
const MARGIN: f64 = 24.0;
const KEEP_ON_TOP: Duration = Duration::from_millis(1500);
/// a hidden mini player is closed after a while so it costs no memory
const CLOSE_HIDDEN_AFTER: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub enabled: bool,
    pub on_top: bool,
    pub on_minimize: bool,
    pub corner: String,
}

impl Default for Options {
    fn default() -> Self {
        Self { enabled: true, on_top: true, on_minimize: true, corner: "top-center".into() }
    }
}

#[derive(Default)]
pub struct Overlay {
    pub options: Mutex<Options>,
    last_state: Mutex<Option<Value>>,
    /// where the user dragged it; None = use the corner
    moved: Mutex<Option<(i32, i32)>>,
    /// the user wants it on screen
    wanted: Mutex<bool>,
    /// shown because the main window was minimized: restoring the window hides it again
    auto_shown: Mutex<bool>,
    /// bumped on every show/hide so old timers know they are stale
    generation: Mutex<u64>,
}

pub fn corner_position(corner: &str, area: PhysicalRect<i32, u32>, scale: f64) -> (i32, i32) {
    let w = (WIDTH * scale).round() as i32;
    let h = (HEIGHT * scale).round() as i32;
    let m = (MARGIN * scale).round() as i32;
    let (ax, ay) = (area.position.x, area.position.y);
    let (aw, ah) = (area.size.width as i32, area.size.height as i32);
    let x = if corner.ends_with("center") {
        ax + (aw - w) / 2
    } else if corner.ends_with("left") {
        ax + m
    } else {
        ax + aw - w - m
    };
    let y = if corner.starts_with("top") { ay + m } else { ay + ah - h - m };
    (x, y)
}

fn on_some_screen(app: &AppHandle, (x, y): (i32, i32)) -> bool {
    app.available_monitors().unwrap_or_default().iter().any(|m| {
        let a = m.work_area();
        let w = (WIDTH * m.scale_factor()) as i32;
        let h = (HEIGHT * m.scale_factor()) as i32;
        x >= a.position.x - w / 2 && y >= a.position.y && x + w / 2 <= a.position.x + a.size.width as i32 && y + h <= a.position.y + a.size.height as i32
    })
}

impl Overlay {
    fn window(app: &AppHandle) -> Option<WebviewWindow> {
        app.get_webview_window(LABEL)
    }

    pub fn is_visible(app: &AppHandle) -> bool {
        Self::window(app).and_then(|w| w.is_visible().ok()).unwrap_or(false)
    }

    pub fn set_options(&self, app: &AppHandle, next: Value) {
        let corner_changed;
        {
            let mut o = self.options.lock().unwrap();
            let mut merged = serde_json::to_value(OptionsOut::from(&*o)).unwrap();
            if let (Some(m), Some(n)) = (merged.as_object_mut(), next.as_object()) {
                for (k, v) in n {
                    m.insert(k.clone(), v.clone());
                }
            }
            let new: Options = serde_json::from_value(merged).unwrap_or_default();
            corner_changed = new.corner != o.corner;
            *o = new;
        }
        let o = self.options.lock().unwrap().clone();
        if !o.enabled {
            self.hide(app);
        }
        if let Some(w) = Self::window(app) {
            let _ = w.set_always_on_top(o.on_top);
            if corner_changed {
                *self.moved.lock().unwrap() = None;
                self.place(app, &w);
            }
        }
    }

    pub fn toggle(&self, app: &AppHandle) {
        if Self::is_visible(app) {
            self.hide(app)
        } else {
            self.show(app)
        }
    }

    pub fn show_for_minimize(&self, app: &AppHandle) {
        if Self::is_visible(app) {
            return;
        }
        self.show(app);
        *self.auto_shown.lock().unwrap() = Self::is_visible(app) || *self.wanted.lock().unwrap();
    }

    pub fn hide_for_restore(&self, app: &AppHandle) {
        if *self.auto_shown.lock().unwrap() {
            self.hide(app);
        }
    }

    pub fn show(&self, app: &AppHandle) {
        if !self.options.lock().unwrap().enabled {
            return;
        }
        *self.auto_shown.lock().unwrap() = false;
        *self.wanted.lock().unwrap() = true;
        let generation = {
            let mut g = self.generation.lock().unwrap();
            *g += 1;
            *g
        };
        let win = match Self::window(app) {
            Some(w) => w,
            None => match self.create(app) {
                Some(w) => w,
                None => return,
            },
        };
        self.place(app, &win);
        show_inactive(&win);
        // some window systems only take the size once the window is on screen
        let _ = win.set_size(tauri::LogicalSize::new(WIDTH, HEIGHT));
        self.apply_top(&win);
        let _ = app.emit_to("main", "overlay:visible", true);
        // the watchdog
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(KEEP_ON_TOP).await;
                let overlay = app.state::<Overlay>();
                if *overlay.generation.lock().unwrap() != generation || !*overlay.wanted.lock().unwrap() {
                    break;
                }
                let Some(win) = Self::window(&app) else {
                    overlay.show(&app);
                    break;
                };
                if win.is_minimized().unwrap_or(false) {
                    let _ = win.unminimize();
                }
                if !win.is_visible().unwrap_or(true) {
                    show_inactive(&win);
                }
                overlay.apply_top(&win);
            }
        });
    }

    pub fn hide(&self, app: &AppHandle) {
        *self.wanted.lock().unwrap() = false;
        *self.auto_shown.lock().unwrap() = false;
        let generation = {
            let mut g = self.generation.lock().unwrap();
            *g += 1;
            *g
        };
        let Some(win) = Self::window(app) else { return };
        let _ = win.hide();
        let _ = app.emit_to("main", "overlay:visible", false);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(CLOSE_HIDDEN_AFTER).await;
            let overlay = app.state::<Overlay>();
            if *overlay.generation.lock().unwrap() == generation {
                if let Some(w) = Self::window(&app) {
                    let _ = w.destroy();
                }
            }
        });
    }

    pub fn destroy(&self, app: &AppHandle) {
        *self.wanted.lock().unwrap() = false;
        if let Some(w) = Self::window(app) {
            let _ = w.destroy();
        }
    }

    /// player state from the main window
    pub fn send(&self, app: &AppHandle, state: Value) {
        if Self::window(app).is_some() {
            let _ = app.emit_to(LABEL, "overlay:state", state.clone());
        }
        *self.last_state.lock().unwrap() = Some(state);
    }

    pub fn resend(&self, app: &AppHandle) {
        if let Some(s) = self.last_state.lock().unwrap().clone() {
            let _ = app.emit_to(LABEL, "overlay:state", s);
        }
    }

    pub fn remember_position(&self, x: i32, y: i32) {
        *self.moved.lock().unwrap() = Some((x, y));
    }

    pub fn replace(&self, app: &AppHandle) {
        if let Some(w) = Self::window(app) {
            self.place(app, &w);
        }
    }

    fn apply_top(&self, win: &WebviewWindow) {
        if self.options.lock().unwrap().on_top {
            force_topmost(win);
        }
    }

    fn place(&self, app: &AppHandle, win: &WebviewWindow) {
        let moved = *self.moved.lock().unwrap();
        if let Some(p) = moved.filter(|p| on_some_screen(app, *p)) {
            let _ = win.set_position(PhysicalPosition::new(p.0, p.1));
            return;
        }
        *self.moved.lock().unwrap() = None;
        let Some(monitor) = app.primary_monitor().ok().flatten() else { return };
        let corner = self.options.lock().unwrap().corner.clone();
        let (x, y) = corner_position(&corner, *monitor.work_area(), monitor.scale_factor());
        let _ = win.set_size(tauri::LogicalSize::new(WIDTH, HEIGHT));
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    fn create(&self, app: &AppHandle) -> Option<WebviewWindow> {
        let on_top = self.options.lock().unwrap().on_top;
        let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
            .title("EvPlayer — мини-плеер")
            .inner_size(WIDTH, HEIGHT)
            // some systems give new windows a minimum height; the bar must stay a bar
            .min_inner_size(WIDTH, HEIGHT)
            .max_inner_size(WIDTH, HEIGHT)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .shadow(false)
            .focused(false)
            .always_on_top(on_top)
            .visible_on_all_workspaces(true)
            .additional_browser_args(&crate::browser_args(app))
            .build()
            .ok()?;
        let app2 = app.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Moved(p) = event {
                app2.state::<Overlay>().remember_position(p.x, p.y);
            }
        });
        Some(win)
    }
}

/// show without taking the keyboard away from the game
fn show_inactive(win: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
        unsafe { ShowWindow(hwnd.0 as _, SW_SHOWNOACTIVATE) };
        return;
    }
    let _ = win.show();
}

/// put it above everything right now, also above windows that made themselves topmost later
fn force_topmost(win: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOOWNERZORDER};
        unsafe { SetWindowPos(hwnd.0 as _, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER) };
        return;
    }
    let _ = win.set_always_on_top(true);
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OptionsOut {
    enabled: bool,
    on_top: bool,
    on_minimize: bool,
    corner: String,
}

impl From<&Options> for OptionsOut {
    fn from(o: &Options) -> Self {
        Self { enabled: o.enabled, on_top: o.on_top, on_minimize: o.on_minimize, corner: o.corner.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::PhysicalSize;

    #[test]
    fn corners() {
        let area = PhysicalRect { position: PhysicalPosition::new(0, 0), size: PhysicalSize::new(1920, 1040) };
        assert_eq!(corner_position("bottom-right", area, 1.0), (1920 - 330 - 24, 1040 - 52 - 24));
        assert_eq!(corner_position("top-left", area, 1.0), (24, 24));
        assert_eq!(corner_position("top-center", area, 1.0), ((1920 - 330) / 2, 24));
        assert_eq!(corner_position("bottom-center", area, 1.5), ((1920 - 495) / 2, 1040 - 78 - 36));
    }
}
