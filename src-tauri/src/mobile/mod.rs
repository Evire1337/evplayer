//! Phones have no tray, no mini player window and no update installer: these stand-ins
//! keep the same names so the commands the page calls exist everywhere and simply do
//! nothing on a phone.

pub mod tray {
    use std::sync::Mutex;

    use serde::Deserialize;
    use tauri::AppHandle;

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TrayOptions {
        pub close_to_tray: bool,
        pub minimize_to_tray: bool,
        pub locale: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct TrayUpdate {}

    pub struct Tray {
        pub options: Mutex<TrayOptions>,
    }

    impl Default for Tray {
        fn default() -> Self {
            Self { options: Mutex::new(TrayOptions { close_to_tray: false, minimize_to_tray: false, locale: None }) }
        }
    }

    pub fn create(_app: &AppHandle) -> tauri::Result<()> {
        Ok(())
    }
    pub fn update(_app: &AppHandle, _state: TrayUpdate) {}
    pub fn rebuild(_app: &AppHandle) {}
}

pub mod overlay {
    use std::sync::Mutex;

    use serde_json::Value;
    use tauri::AppHandle;

    #[derive(Clone, Debug, Default)]
    pub struct Options {
        pub on_minimize: bool,
    }

    #[derive(Default)]
    pub struct Overlay {
        pub options: Mutex<Options>,
    }

    impl Overlay {
        pub fn set_options(&self, _app: &AppHandle, _next: Value) {}
        pub fn toggle(&self, _app: &AppHandle) {}
        pub fn show_for_minimize(&self, _app: &AppHandle) {}
        pub fn hide_for_restore(&self, _app: &AppHandle) {}
        pub fn hide(&self, _app: &AppHandle) {}
        pub fn destroy(&self, _app: &AppHandle) {}
        pub fn send(&self, _app: &AppHandle, _state: Value) {}
        pub fn resend(&self, _app: &AppHandle) {}
        pub fn replace(&self, _app: &AppHandle) {}
    }
}

pub mod updates {
    use serde_json::{json, Value};
    use tauri::AppHandle;

    /// the phone version checks GitHub for a newer APK from the page itself
    #[derive(Default)]
    pub struct Updates;

    pub fn start(_app: &AppHandle) {}

    #[tauri::command]
    pub fn updates_get() -> Value {
        json!({ "status": "idle" })
    }

    #[tauri::command]
    pub async fn updates_check() {}

    #[tauri::command]
    pub fn updates_install() -> Result<(), String> {
        Ok(())
    }
}
