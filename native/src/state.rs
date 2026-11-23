//! The program's kept state in one place: settings, library, history, effects, with the store
//! they are saved to. Any thread reads it; a change is saved shortly after (store/kv.rs) and the
//! window is told to draw again.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use crate::store::kv::{self, Kv};
use crate::store::state::{Effects, History, Library, Session, Settings};

pub const SETTINGS: &str = "evplayer.settings.v1";
pub const LIBRARY: &str = "evplayer.library.v1";
pub const HISTORY: &str = "evplayer.history.v1";
pub const SESSION: &str = "evplayer.session.v1";
pub const EFFECTS: &str = "evplayer.effects";
pub const VOLUME: &str = "evplayer.volume";

pub struct State {
    pub data_dir: PathBuf,
    pub kv: Arc<Kv>,
    pub settings: RwLock<Settings>,
    pub library: RwLock<Library>,
    pub history: Mutex<History>,
    pub effects: RwLock<Effects>,
}

static STATE: OnceLock<Arc<State>> = OnceLock::new();

/// the state; `init` must have run (it does, first thing in main)
pub fn get() -> &'static Arc<State> {
    STATE.get().expect("state::init")
}

pub fn init(data_dir: PathBuf) -> &'static Arc<State> {
    STATE.get_or_init(|| {
        let store_dir = data_dir.join("native");
        let scratch = std::env::temp_dir().join("EvPlayer");
        let kv = Kv::open(store_dir, || {
            if std::env::var_os("EVPLAYER_NO_IMPORT").is_some() {
                return None;
            }
            // the first start: what the Tauri build saved comes along
            let dir = crate::store::import::webview_storage_dir()?;
            let found = crate::store::import::read_webview_storage(&dir, &scratch);
            if let Some(f) = &found {
                eprintln!("[state] took {} values from the Tauri build", f.len());
            }
            found
        });
        let mut settings: Settings = kv::read_state(&kv, SETTINGS).unwrap_or_default();
        // an app that was in use before there was a choice stays Russian (as in the page)
        if kv.get(SETTINGS).is_some() && !settings.rest.contains_key("language") && settings.language == crate::store::state::Language::Auto {
            settings.language = crate::store::state::Language::Ru;
        }
        let library: Library = kv::read_state(&kv, LIBRARY).unwrap_or_default();
        let history: History = kv::read_state(&kv, HISTORY).unwrap_or_default();
        let effects: Effects = kv::read_plain(&kv, EFFECTS).unwrap_or_default();
        Arc::new(State { data_dir, kv, settings: RwLock::new(settings), library: RwLock::new(library), history: Mutex::new(history), effects: RwLock::new(effects) })
    })
}

impl State {
    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    /// change the settings and save them
    pub fn set_settings(&self, f: impl FnOnce(&mut Settings)) {
        let mut s = self.settings.write().unwrap();
        f(&mut s);
        kv::write_state(&self.kv, SETTINGS, &*s, 1);
        drop(s);
        crate::events::repaint();
    }

    pub fn update_library<R>(&self, f: impl FnOnce(&mut Library) -> R) -> R {
        let mut l = self.library.write().unwrap();
        let r = f(&mut l);
        kv::write_state(&self.kv, LIBRARY, &*l, 0);
        drop(l);
        crate::events::repaint();
        r
    }

    pub fn update_history(&self, f: impl FnOnce(&mut History)) {
        let mut h = self.history.lock().unwrap();
        f(&mut h);
        kv::write_state(&self.kv, HISTORY, &*h, 0);
    }

    pub fn set_effects(&self, f: impl FnOnce(&mut Effects)) {
        let mut e = self.effects.write().unwrap();
        f(&mut e);
        kv::write_plain(&self.kv, EFFECTS, &*e);
        drop(e);
        crate::events::repaint();
    }

    pub fn session(&self) -> Option<Session> {
        kv::read_plain(&self.kv, SESSION)
    }

    pub fn save_session(&self, s: &Session) {
        kv::write_plain(&self.kv, SESSION, s);
    }

    pub fn volume(&self) -> f32 {
        self.kv.get(VOLUME).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| *v > 0.0 && *v <= 1.0).unwrap_or(0.8)
    }

    pub fn save_volume(&self, v: f32) {
        self.kv.set(VOLUME, format!("{}", (v * 1000.0).round() / 1000.0));
    }
}
