//! Keyboard shortcuts, with the ids, defaults and accelerator texts of
//! src/settings/shortcuts.ts, so the bindings saved by the Tauri build work here as they are.

use serde_json::{Map, Value};

use super::actions::Action;
use super::App;
use crate::store::state::Settings;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    App,
    Global,
    /// while the mini player has the keyboard
    Overlay,
}

pub struct Shortcut {
    pub id: &'static str,
    pub action: Action,
    pub scope: Scope,
    pub title: &'static str,
    pub hint: &'static str,
    pub default: Option<&'static str>,
}

const fn s(id: &'static str, action: Action, scope: Scope, title: &'static str, hint: &'static str, default: Option<&'static str>) -> Shortcut {
    Shortcut { id, action, scope, title, hint, default }
}

pub const SHORTCUTS: &[Shortcut] = &[
    s("app.playPause", Action::PlayPause, Scope::App, "Играть / пауза", "Остановить или продолжить текущий трек", Some("Space")),
    s("app.next", Action::Next, Scope::App, "Следующий трек", "Перейти к следующему в очереди", Some("Ctrl+Right")),
    s("app.prev", Action::Prev, Scope::App, "Предыдущий трек", "В начале трека — к предыдущему, иначе с начала", Some("Ctrl+Left")),
    s("app.seekForward", Action::SeekForward, Scope::App, "Вперёд на 5 секунд", "Перемотать вперёд", Some("Right")),
    s("app.seekBackward", Action::SeekBack, Scope::App, "Назад на 5 секунд", "Перемотать назад", Some("Left")),
    s("app.volumeUp", Action::VolumeUp, Scope::App, "Громче", "Прибавить громкость на 5%", Some("Up")),
    s("app.volumeDown", Action::VolumeDown, Scope::App, "Тише", "Убавить громкость на 5%", Some("Down")),
    s("app.mute", Action::Mute, Scope::App, "Без звука", "Выключить и включить звук", Some("M")),
    s("app.like", Action::Like, Scope::App, "Нравится", "Добавить текущий трек в понравившиеся или убрать", Some("L")),
    s("app.shuffle", Action::Shuffle, Scope::App, "Перемешать", "Включить или выключить перемешивание", Some("S")),
    s("app.repeat", Action::Repeat, Scope::App, "Повтор", "Выключен → вся очередь → один трек", Some("R")),
    s("app.search", Action::Search, Scope::App, "Поиск", "Открыть поиск", Some("Ctrl+F")),
    s("app.settings", Action::Settings, Scope::App, "Настройки", "Открыть настройки", Some("Ctrl+,")),
    s("app.nowPlaying", Action::NowPlaying, Scope::App, "Сейчас играет", "Открыть экран с текстом и очередью", Some("Ctrl+N")),
    s("app.overlay", Action::ToggleOverlay, Scope::App, "Мини-плеер", "Показать или спрятать мини-плеер поверх окон", Some("Ctrl+M")),
    s("global.playPause", Action::PlayPause, Scope::Global, "Играть / пауза", "Из любой программы", Some("Ctrl+Alt+Space")),
    s("global.next", Action::Next, Scope::Global, "Следующий трек", "Из любой программы", Some("Ctrl+Alt+Right")),
    s("global.prev", Action::Prev, Scope::Global, "Предыдущий трек", "Из любой программы", Some("Ctrl+Alt+Left")),
    s("global.like", Action::Like, Scope::Global, "Нравится", "Лайкнуть то, что играет, не открывая окно", Some("Ctrl+Alt+L")),
    s("global.volumeUp", Action::VolumeUp, Scope::Global, "Громче", "Громкость EvPlayer, не системы", None),
    s("global.volumeDown", Action::VolumeDown, Scope::Global, "Тише", "Громкость EvPlayer, не системы", None),
    s("global.overlay", Action::ToggleOverlay, Scope::Global, "Мини-плеер", "Показать или спрятать мини-плеер поверх окон", Some("Ctrl+Alt+M")),
    s("global.show", Action::ShowWindow, Scope::Global, "Показать EvPlayer", "Вывести окно на передний план", Some("Ctrl+Alt+E")),
    s("overlay.playPause", Action::PlayPause, Scope::Overlay, "Играть / пауза", "Когда мини-плеер в фокусе (кликните по нему)", Some("Space")),
    s("overlay.next", Action::Next, Scope::Overlay, "Следующий трек", "Когда мини-плеер в фокусе", Some("Right")),
    s("overlay.prev", Action::Prev, Scope::Overlay, "Предыдущий трек", "Когда мини-плеер в фокусе", Some("Left")),
    s("overlay.seekForward", Action::SeekForward, Scope::Overlay, "Вперёд на 5 секунд", "Когда мини-плеер в фокусе", Some("Shift+Right")),
    s("overlay.seekBackward", Action::SeekBack, Scope::Overlay, "Назад на 5 секунд", "Когда мини-плеер в фокусе", Some("Shift+Left")),
    s("overlay.volumeUp", Action::VolumeUp, Scope::Overlay, "Громче", "Когда мини-плеер в фокусе", Some("Up")),
    s("overlay.volumeDown", Action::VolumeDown, Scope::Overlay, "Тише", "Когда мини-плеер в фокусе", Some("Down")),
    s("overlay.mute", Action::Mute, Scope::Overlay, "Без звука", "Когда мини-плеер в фокусе", Some("M")),
    s("overlay.like", Action::Like, Scope::Overlay, "Нравится", "Когда мини-плеер в фокусе", Some("L")),
    s("overlay.show", Action::ShowWindow, Scope::Overlay, "Открыть EvPlayer", "Развернуть основное окно", Some("Enter")),
    s("overlay.hide", Action::HideOverlay, Scope::Overlay, "Спрятать мини-плеер", "Убрать мини-плеер с экрана", Some("Escape")),
];

/// the key for a shortcut: the person's own, or the default
pub fn binding<'a>(overrides: &'a Map<String, Value>, def: &'a Shortcut) -> Option<&'a str> {
    match overrides.get(def.id) {
        Some(Value::String(s)) => Some(s.as_str()),
        Some(Value::Null) => None,
        _ => def.default,
    }
}

/// "Ctrl+Alt+Space" as it is shown ("Ctrl+Alt+Пробел")
pub fn pretty(acc: &str) -> String {
    acc.split('+')
        .map(|k| match k {
            "Space" => crate::i18n::t("Пробел"),
            "Right" => "→".into(),
            "Left" => "←".into(),
            "Up" => "↑".into(),
            "Down" => "↓".into(),
            "Escape" => "Esc".into(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// an egui key as the page names it ("Right", "L", "Space", ",")
fn key_name(k: egui::Key) -> Option<String> {
    use egui::Key::*;
    Some(match k {
        Space => "Space".into(),
        ArrowRight => "Right".into(),
        ArrowLeft => "Left".into(),
        ArrowUp => "Up".into(),
        ArrowDown => "Down".into(),
        Enter => "Enter".into(),
        Escape => "Escape".into(),
        Comma => ",".into(),
        Period => ".".into(),
        other => {
            let n = other.name();
            if n.len() == 1 || (n.starts_with('F') && n.len() <= 3) {
                n.to_string()
            } else {
                return None;
            }
        }
    })
}

/// a new key is being recorded in Настройки → Клавиши: the window's keys stay quiet
pub static RECORDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// the shortcut of the same scope that already has this key
pub fn conflict_for(overrides: &Map<String, Value>, id: &str, acc: &str) -> Option<&'static Shortcut> {
    let scope = SHORTCUTS.iter().find(|d| d.id == id)?.scope;
    SHORTCUTS.iter().find(|d| d.id != id && d.scope == scope && binding(overrides, d) == Some(acc))
}

pub struct Keys;

impl Keys {
    pub fn new() -> Self {
        Self
    }

    /// register the "везде" shortcuts (or none, when switched off)
    pub fn apply_global(&self, s: &Settings) {
        let map: Vec<(String, Action)> = if s.global_shortcuts {
            SHORTCUTS.iter().filter(|d| d.scope == Scope::Global).filter_map(|d| binding(&s.shortcuts, d).map(|a| (a.to_string(), d.action))).collect()
        } else {
            vec![]
        };
        crate::system::set_hotkeys(map);
    }

    /// keys pressed in the window (not while typing in a field)
    pub fn window_keys(ctx: &egui::Context, app: &mut App) {
        if ctx.egui_wants_keyboard_input() || app.dialog.is_some() || RECORDING.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let pressed: Vec<(egui::Key, egui::Modifiers)> = ctx.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Key { key, pressed: true, modifiers, repeat: _, .. } = e { Some((*key, *modifiers)) } else { None }).collect());
        if pressed.is_empty() {
            return;
        }
        let overrides = crate::state::get().settings.read().unwrap().shortcuts.clone();
        for (key, m) in pressed {
            if key == egui::Key::Escape && app.menu.is_some() {
                continue;
            }
            if let Some(a) = action_for(&overrides, Scope::App, key, m) {
                app.action(a);
            }
        }
    }
}

/// "Ctrl+Shift+Right" for a pressed key, in the page's accelerator words
pub fn accelerator(key: egui::Key, m: egui::Modifiers) -> Option<String> {
    let name = key_name(key)?;
    let mut parts = Vec::new();
    if m.ctrl || m.command {
        parts.push("Ctrl".to_string());
    }
    if m.alt {
        parts.push("Alt".to_string());
    }
    if m.shift {
        parts.push("Shift".to_string());
    }
    parts.push(name);
    Some(parts.join("+"))
}

/// what a key pressed in `scope` does, by the person's bindings
pub fn action_for(overrides: &Map<String, Value>, scope: Scope, key: egui::Key, m: egui::Modifiers) -> Option<Action> {
    let acc = accelerator(key, m)?;
    SHORTCUTS.iter().find(|d| d.scope == scope && binding(overrides, d) == Some(acc.as_str())).map(|d| d.action)
}

impl Default for Keys {
    fn default() -> Self {
        Self::new()
    }
}
