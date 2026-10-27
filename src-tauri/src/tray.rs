//! Keeps EvPlayer in the notification area: closing (or, if chosen, minimizing) the window
//! hides it there while music keeps playing; the tray icon brings it back and has the basic
//! playback controls.

use std::sync::Mutex;

use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub const ID: &str = "main";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayOptions {
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TrayTrack {
    pub title: Option<String>,
    pub artist: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TrayUpdate {
    pub status: Option<String>,
    pub track: Option<TrayTrack>,
}

pub struct Tray {
    pub options: Mutex<TrayOptions>,
    track: Mutex<Option<TrayTrack>>,
    playing: Mutex<bool>,
}

impl Default for Tray {
    fn default() -> Self {
        Self {
            options: Mutex::new(TrayOptions { close_to_tray: true, minimize_to_tray: false, locale: None }),
            track: Mutex::new(None),
            playing: Mutex::new(false),
        }
    }
}

struct Text {
    show: &'static str,
    hide: &'static str,
    play: &'static str,
    pause: &'static str,
    overlay: &'static str,
    next: &'static str,
    prev: &'static str,
    quit: &'static str,
    nothing: &'static str,
}

fn text(locale: &str) -> Text {
    let l = locale.to_lowercase();
    if l.starts_with("ru") || l.starts_with("uk") || l.starts_with("be") || l.is_empty() {
        Text { show: "Показать EvPlayer", hide: "Скрыть в трей", play: "Играть", pause: "Пауза", overlay: "Мини-плеер", next: "Следующий", prev: "Предыдущий", quit: "Выход", nothing: "Ничего не играет" }
    } else {
        Text { show: "Show EvPlayer", hide: "Hide to tray", play: "Play", pause: "Pause", overlay: "Mini player", next: "Next", prev: "Previous", quit: "Quit", nothing: "Nothing playing" }
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let icon = app.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("EvPlayer")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => crate::send_command(app, "togglePlay", None),
            "next" => crate::send_command(app, "next", None),
            "prev" => crate::send_command(app, "prev", None),
            "overlay" => app.state::<crate::overlay::Overlay>().toggle(app),
            "show" => crate::show_main(app),
            "hide" => crate::hide_main(app),
            "quit" => crate::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                if crate::main_visible(app) {
                    crate::hide_main(app)
                } else {
                    crate::show_main(app)
                }
            }
        });
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    rebuild(app);
    Ok(())
}

pub fn update(app: &AppHandle, state: TrayUpdate) {
    let tray = app.state::<Tray>();
    let playing = matches!(state.status.as_deref(), Some("playing" | "loading" | "buffering"));
    {
        let mut p = tray.playing.lock().unwrap();
        let mut t = tray.track.lock().unwrap();
        let same = *p == playing
            && t.as_ref().map(|x| (&x.title, &x.artist)) == state.track.as_ref().map(|x| (&x.title, &x.artist));
        if same {
            return;
        }
        *p = playing;
        *t = state.track;
    }
    rebuild(app);
}

pub fn rebuild(app: &AppHandle) {
    let Some(icon) = app.tray_by_id(ID) else { return };
    let tray = app.state::<Tray>();
    let locale = tray.options.lock().unwrap().locale.clone().unwrap_or_default();
    let t = text(&locale);
    let track = tray.track.lock().unwrap().clone();
    let playing = *tray.playing.lock().unwrap();
    let has = track.as_ref().and_then(|x| x.title.as_ref()).is_some();
    let label = match &track {
        Some(TrayTrack { title: Some(title), artist }) => match artist.as_deref().filter(|a| !a.is_empty()) {
            Some(a) => format!("{title} — {a}"),
            None => title.clone(),
        },
        _ => t.nothing.to_string(),
    };
    let short: String = if label.chars().count() > 60 { format!("{}…", label.chars().take(57).collect::<String>()) } else { label.clone() };
    let tooltip: String = if has { format!("EvPlayer · {label}").chars().take(127).collect() } else { "EvPlayer".into() };
    let _ = icon.set_tooltip(Some(tooltip));
    let build = || -> tauri::Result<Menu<tauri::Wry>> {
        let visible = crate::main_visible(app);
        Menu::with_items(
            app,
            &[
                &MenuItem::with_id(app, "label", short, false, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "toggle", if playing { t.pause } else { t.play }, has, None::<&str>)?,
                &MenuItem::with_id(app, "next", t.next, has, None::<&str>)?,
                &MenuItem::with_id(app, "prev", t.prev, has, None::<&str>)?,
                &MenuItem::with_id(app, "overlay", t.overlay, true, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &if visible { MenuItem::with_id(app, "hide", t.hide, true, None::<&str>)? } else { MenuItem::with_id(app, "show", t.show, true, None::<&str>)? },
                &MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?,
            ],
        )
    };
    if let Ok(menu) = build() {
        let _ = icon.set_menu(Some(menu));
    }
}
