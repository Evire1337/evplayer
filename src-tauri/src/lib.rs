//! EvPlayer's desktop side: windows, tray, keys, the mini player, local files, the network
//! proxy, Discord and updates. The page talks to it through the commands below; the
//! JavaScript side (src/platform/tauri.ts) offers them to the app as `window.evplayer`,
//! the same interface the Electron version had.

mod discord;
mod library;
mod media_server;
mod memory;
mod migrate;
mod overlay;
mod proxy;
mod services;
mod tray;
mod updates;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

use library::{AddResult, Library, LocalTrackEdit, LocalTrackRecord, Watcher};
use overlay::Overlay;
use tray::Tray;

pub const MAIN: &str = "main";

pub struct AppState {
    pub data_dir: PathBuf,
    pub library: Arc<Library>,
    pub watcher: Mutex<Option<Arc<Watcher>>>,
    pub discord: discord::Discord,
    /// was the main window minimized at the last resize (to notice minimize and restore)
    minimized: Mutex<bool>,
    quitting: Mutex<bool>,
}

/// the data folder: the same one the Electron version used, so local files and caches carry over
fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().config_dir().map(|d| d.join("EvPlayer")).unwrap_or_else(|_| PathBuf::from("."))
}

pub fn send_command(app: &AppHandle, action: &str, value: Option<f64>) {
    let _ = app.emit_to(MAIN, "player:command", serde_json::json!({ "action": action, "value": value }));
}

pub fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN)
        .map(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
        .unwrap_or(false)
}

fn set_visible(app: &AppHandle, visible: bool) {
    let _ = app.emit_to(MAIN, "window:visible", visible);
    if let Some(w) = app.get_webview_window(MAIN) {
        memory::set_low(&w, !visible);
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    *app.state::<AppState>().minimized.lock().unwrap() = false;
    set_visible(app, true);
    app.state::<Overlay>().hide_for_restore(app);
    tray::rebuild(app);
}

pub fn hide_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.hide();
    }
    set_visible(app, false);
    tray::rebuild(app);
}

pub fn quit(app: &AppHandle) {
    *app.state::<AppState>().quitting.lock().unwrap() = true;
    app.state::<AppState>().discord.quit();
    app.state::<Overlay>().destroy(app);
    app.exit(0);
}

// ---- commands -------------------------------------------------------------------------

#[tauri::command]
async fn soundcloud_client_id(state: tauri::State<'_, AppState>, force: bool) -> Result<String, String> {
    services::soundcloud_client_id(state.data_dir.clone(), force).await
}

fn dir_size(path: &std::path::Path) -> u64 {
    std::fs::read_dir(path)
        .map(|rd| {
            rd.flatten()
                .map(|e| match e.file_type() {
                    Ok(t) if t.is_dir() => dir_size(&e.path()),
                    _ => e.metadata().map(|m| m.len()).unwrap_or(0),
                })
                .sum()
        })
        .unwrap_or(0)
}

/// the "don't keep a cache" setting, as a file so it is known before any window starts
fn no_cache_flag(app: &AppHandle) -> PathBuf {
    data_dir(app).join("no-cache")
}

/// options for the browser engine; every window must get the same ones (WebView2 refuses a
/// second window with different options)
pub fn browser_args(app: &AppHandle) -> String {
    // the first two are what Tauri passes by default; autoplay lets media keys and global
    // hotkeys start the music while the window is hidden, before any click in it
    let mut args = String::from("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required");
    if no_cache_flag(app).exists() {
        args.push_str(" --disk-cache-size=1");
    }
    args
}

fn clear_cache_dirs(app: &AppHandle) {
    // files the browser engine holds open stay; everything else goes
    for d in cache_dirs(app) {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            }
        }
    }
}

#[tauri::command]
fn set_no_cache(app: AppHandle, on: bool) {
    let flag = no_cache_flag(&app);
    let _ = std::fs::create_dir_all(data_dir(&app));
    let _ = if on { std::fs::write(&flag, b"1") } else { std::fs::remove_file(&flag) };
    if on {
        clear_cache_dirs(&app);
    }
}

fn cache_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = app.path().app_local_data_dir() {
        for d in ["Cache", "Code Cache", "GPUCache"] {
            out.push(local.join("EBWebView").join("Default").join(d));
        }
    }
    if let Ok(cache) = app.path().app_cache_dir() {
        out.push(cache);
    }
    out
}

#[tauri::command]
fn cache_size(app: AppHandle) -> Value {
    let total: u64 = cache_dirs(&app).iter().map(|d| dir_size(d)).sum();
    serde_json::json!({ "cache": total })
}

#[tauri::command]
fn clear_cache(app: AppHandle) {
    clear_cache_dirs(&app);
}

#[tauri::command]
fn open_data_folder(app: AppHandle, state: tauri::State<'_, AppState>) {
    let _ = app.opener().open_path(state.data_dir.to_string_lossy(), None::<&str>);
}

#[tauri::command]
fn show_window(app: AppHandle) {
    show_main(&app);
}

#[tauri::command]
fn local_list(state: tauri::State<'_, AppState>) -> Vec<LocalTrackRecord> {
    state.library.list()
}

fn add_paths(app: &AppHandle, paths: Vec<PathBuf>) -> AddResult {
    let state = app.state::<AppState>();
    let (result, folders) = state.library.add(&paths);
    if let Some(w) = state.watcher.lock().unwrap().as_ref() {
        for f in &folders {
            w.watch(f);
        }
    }
    result
}

#[tauri::command]
async fn local_pick(app: AppHandle, kind: String) -> Result<AddResult, String> {
    let dialog = app.dialog().file();
    let picked: Vec<PathBuf> = if kind == "folder" {
        dialog.blocking_pick_folder().and_then(|p| p.into_path().ok()).into_iter().collect()
    } else {
        dialog
            .add_filter("Audio", library::AUDIO_EXTENSIONS)
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .collect()
    };
    if picked.is_empty() {
        return Ok(AddResult { added: vec![], skipped: 0 });
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || add_paths(&app2, picked)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn local_add_paths(app: AppHandle, paths: Vec<String>) -> Result<AddResult, String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || add_paths(&app, paths)).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn local_remove(state: tauri::State<'_, AppState>, ids: Vec<String>) {
    state.library.remove(&ids);
}

#[tauri::command]
fn local_reveal(app: AppHandle, state: tauri::State<'_, AppState>, id: String) {
    if let Some(r) = state.library.get(&id) {
        let _ = app.opener().reveal_item_in_dir(r.path);
    }
}

#[tauri::command]
fn local_update(state: tauri::State<'_, AppState>, id: String, patch: LocalTrackEdit) -> Option<LocalTrackRecord> {
    state.library.update(&id, patch)
}

#[tauri::command]
fn local_watched_folders(state: tauri::State<'_, AppState>) -> Vec<String> {
    state.library.folders()
}

#[tauri::command]
fn local_unwatch_folder(state: tauri::State<'_, AppState>, folder: String) {
    state.library.unwatch(&folder);
    if let Some(w) = state.watcher.lock().unwrap().as_ref() {
        w.unwatch(&folder);
    }
}

#[tauri::command]
async fn save_text(app: AppHandle, name: String, text: String) -> Result<bool, String> {
    let dir = app.path().document_dir().ok();
    let mut dialog = app.dialog().file().set_file_name(&name).add_filter("JSON", &["json"]);
    if let Some(d) = dir {
        dialog = dialog.set_directory(d);
    }
    let Some(path) = dialog.blocking_save_file().and_then(|p| p.into_path().ok()) else { return Ok(false) };
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
async fn open_text(app: AppHandle, extensions: Vec<String>) -> Result<Vec<Value>, String> {
    let ext: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let files = app.dialog().file().add_filter("Data", &ext).blocking_pick_files().unwrap_or_default();
    let mut out = Vec::new();
    for f in files {
        let Ok(path) = f.into_path() else { continue };
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        out.push(serde_json::json!({ "name": name, "text": text }));
    }
    Ok(out)
}

#[tauri::command]
fn tray_set_options(app: AppHandle, options: Value) {
    let tray = app.state::<Tray>();
    {
        let mut o = tray.options.lock().unwrap();
        if let Some(v) = options.get("closeToTray").and_then(Value::as_bool) {
            o.close_to_tray = v;
        }
        if let Some(v) = options.get("minimizeToTray").and_then(Value::as_bool) {
            o.minimize_to_tray = v;
        }
        if let Some(v) = options.get("locale").and_then(Value::as_str) {
            o.locale = Some(v.to_string());
        }
    }
    tray::rebuild(&app);
}

#[tauri::command]
fn tray_update(app: AppHandle, state: tray::TrayUpdate) {
    tray::update(&app, state);
}

/// Electron-style key names from the settings to the names the system hotkey library knows
pub fn accelerator(text: &str) -> String {
    text.split('+')
        .map(|part| match part {
            "," => "Comma",
            "." => "Period",
            "/" => "Slash",
            "\\" => "Backslash",
            ";" => "Semicolon",
            "'" => "Quote",
            "[" => "BracketLeft",
            "]" => "BracketRight",
            "-" => "Minus",
            "=" => "Equal",
            "`" => "Backquote",
            "MediaNextTrack" => "MediaTrackNext",
            "MediaPreviousTrack" => "MediaTrackPrevious",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[tauri::command]
fn set_global_shortcuts(app: AppHandle, map: BTreeMap<String, String>) -> Vec<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut failed = Vec::new();
    for (acc, action) in map {
        let ok = accelerator(&acc)
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .ok()
            .map(|shortcut| {
                let action = action.clone();
                gs.on_shortcut(shortcut, move |app, _sc, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    match action.as_str() {
                        "showWindow" => show_main(app),
                        "toggleOverlay" => app.state::<Overlay>().toggle(app),
                        other => send_command(app, other, None),
                    }
                })
                .is_ok()
            })
            .unwrap_or(false);
        if !ok {
            failed.push(acc);
        }
    }
    failed
}

#[tauri::command]
fn overlay_set_options(app: AppHandle, options: Value) {
    app.state::<Overlay>().set_options(&app, options);
}

#[tauri::command]
fn overlay_toggle(app: AppHandle) {
    app.state::<Overlay>().toggle(&app);
}

#[tauri::command]
fn overlay_hide(app: AppHandle) {
    app.state::<Overlay>().hide(&app);
}

#[tauri::command]
fn overlay_state(app: AppHandle, state: Value) {
    app.state::<Overlay>().send(&app, state);
}

#[tauri::command]
fn overlay_ready(app: AppHandle) {
    app.state::<Overlay>().resend(&app);
    let _ = app.emit_to(MAIN, "overlay:ready", ());
}

#[tauri::command]
fn overlay_command(app: AppHandle, action: String, value: Option<f64>) {
    match action.as_str() {
        "showWindow" => show_main(&app),
        _ => send_command(&app, &action, value),
    }
}

#[tauri::command]
async fn spotify_sign_in(app: AppHandle, client_id: String) -> Result<Value, String> {
    let opener = app.clone();
    let tokens = services::spotify_sign_in(client_id, move |url| {
        let _ = opener.opener().open_url(url, None::<&str>);
    })
    .await?;
    serde_json::to_value(tokens).map_err(|e| e.to_string())
}

#[tauri::command]
async fn spotify_refresh(client_id: String, refresh_token: String) -> Result<Value, String> {
    let tokens = services::spotify_refresh(client_id, refresh_token).await?;
    serde_json::to_value(tokens).map_err(|e| e.to_string())
}

#[tauri::command]
fn discord_set(state: tauri::State<'_, AppState>, app_id: Option<String>) {
    state.discord.set_app_id(app_id);
}

#[tauri::command]
fn discord_update(state: tauri::State<'_, AppState>, presence: Option<discord::PresenceState>) {
    state.discord.update(presence);
}

#[tauri::command]
fn discord_status(state: tauri::State<'_, AppState>) -> discord::Status {
    state.discord.status()
}

/// the saved state of the Electron version, read once on the first start of this one
#[tauri::command]
async fn migrate_electron(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<Option<BTreeMap<String, String>>, String> {
    let from = state.data_dir.clone();
    let scratch = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || migrate::read_electron_storage(&from, &scratch)).await.map_err(|e| e.to_string())
}

// ---- start ----------------------------------------------------------------------------

fn create_main(app: &AppHandle, media_base: &str) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        // where the page gets songs from (see media_server.rs)
        .additional_browser_args(&browser_args(app))
        .initialization_script(format!("window.__EVPLAYER_MEDIA__ = {};", serde_json::to_string(media_base).unwrap_or_default()))
        .title("EvPlayer")
        .inner_size(1280.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .decorations(false)
        .background_color(tauri::window::Color(6, 32, 29, 255))
        // music has to keep playing and counting while the window is hidden
        .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled)
        .build()?;
    let app2 = app.clone();
    win.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            let state = app2.state::<AppState>();
            if *state.quitting.lock().unwrap() {
                return;
            }
            api.prevent_close();
            if app2.state::<Tray>().options.lock().unwrap().close_to_tray {
                hide_main(&app2);
            } else {
                quit(&app2);
            }
        }
        WindowEvent::Resized(_) => {
            let Some(w) = app2.get_webview_window(MAIN) else { return };
            let now = w.is_minimized().unwrap_or(false);
            let state = app2.state::<AppState>();
            let was = std::mem::replace(&mut *state.minimized.lock().unwrap(), now);
            if now && !was {
                if app2.state::<Tray>().options.lock().unwrap().minimize_to_tray {
                    hide_main(&app2);
                } else {
                    set_visible(&app2, false);
                }
                if app2.state::<Overlay>().options.lock().unwrap().on_minimize {
                    app2.state::<Overlay>().show_for_minimize(&app2);
                }
                tray::rebuild(&app2);
            } else if !now && was {
                set_visible(&app2, true);
                app2.state::<Overlay>().hide_for_restore(&app2);
                tray::rebuild(&app2);
            }
        }
        WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
            let list: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
            let _ = app2.emit_to(MAIN, "files:drop", list);
        }
        WindowEvent::DragDrop(tauri::DragDropEvent::Enter { .. }) => {
            let _ = app2.emit_to(MAIN, "files:drag", true);
        }
        WindowEvent::DragDrop(tauri::DragDropEvent::Leave) => {
            let _ = app2.emit_to(MAIN, "files:drag", false);
        }
        WindowEvent::ScaleFactorChanged { .. } => app2.state::<Overlay>().replace(&app2),
        _ => {}
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .register_asynchronous_uri_scheme_protocol("evproxy", |_ctx, request, responder| {
            tauri::async_runtime::spawn(async move { responder.respond(proxy::handle(request).await) });
        })
        .register_asynchronous_uri_scheme_protocol("evmedia", |ctx, request, responder| {
            let library = ctx.app_handle().state::<AppState>().library.clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(library.handle(&request)));
        })
        .manage(Overlay::default())
        .manage(Tray::default())
        .manage(updates::Updates::default())
        .invoke_handler(tauri::generate_handler![
            soundcloud_client_id,
            cache_size,
            clear_cache,
            set_no_cache,
            open_data_folder,
            show_window,
            local_list,
            local_pick,
            local_add_paths,
            local_remove,
            local_reveal,
            local_update,
            local_watched_folders,
            local_unwatch_folder,
            save_text,
            open_text,
            tray_set_options,
            tray_update,
            set_global_shortcuts,
            overlay_set_options,
            overlay_toggle,
            overlay_hide,
            overlay_state,
            overlay_ready,
            overlay_command,
            spotify_sign_in,
            spotify_refresh,
            discord_set,
            discord_update,
            discord_status,
            migrate_electron,
            updates::updates_get,
            updates::updates_check,
            updates::updates_install,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = data_dir(&handle);
            let library = Library::new(dir.clone());
            app.manage(AppState {
                data_dir: dir,
                library: library.clone(),
                watcher: Mutex::new(None),
                discord: discord::Discord::new(),
                minimized: Mutex::new(false),
                quitting: Mutex::new(false),
            });
            // "don't keep a cache": what the last run left behind goes before anything starts
            if no_cache_flag(&handle).exists() {
                clear_cache_dirs(&handle);
            }
            let media = media_server::MediaServer::start(library.clone()).map(|m| m.base).unwrap_or_default();
            create_main(&handle, &media)?;
            tray::create(&handle)?;
            let notify = handle.clone();
            let watcher = Watcher::start(library, move || {
                let _ = notify.emit_to(MAIN, "local:changed", ());
            });
            *app.state::<AppState>().watcher.lock().unwrap() = Some(watcher);
            updates::start(&handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building EvPlayer")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = &event {
                // closing the last window is not quitting: the player lives on in the tray
                if code.is_none() && !*app.state::<AppState>().quitting.lock().unwrap() {
                    api.prevent_exit();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn accelerators() {
        assert_eq!(super::accelerator("Ctrl+Shift+L"), "Ctrl+Shift+L");
        assert_eq!(super::accelerator("Ctrl+,"), "Ctrl+Comma");
        assert_eq!(super::accelerator("MediaNextTrack"), "MediaTrackNext");
        for a in ["Ctrl+Shift+L", "Alt+Right", "MediaPlayPause", "Ctrl+num5", "F7", "Ctrl+Alt+Space"] {
            assert!(super::accelerator(a).parse::<tauri_plugin_global_shortcut::Shortcut>().is_ok(), "{a}");
        }
    }
}
