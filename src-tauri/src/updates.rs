//! Updates come from the GitHub releases of this repository (latest.json, signed). A new
//! version downloads in the background; the app offers a restart.

use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const CHECK_EVERY: Duration = Duration::from_secs(4 * 60 * 60);

#[derive(Default)]
pub struct Updates {
    state: Mutex<Value>,
    ready: Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>,
    busy: Mutex<bool>,
}

fn send(app: &AppHandle, state: Value) {
    *app.state::<Updates>().state.lock().unwrap() = state.clone();
    let _ = app.emit_to(crate::MAIN, "updates:state", state);
}

/// a short, readable reason instead of a full HTTP dump
pub fn short_error(message: &str) -> String {
    let m = message.to_lowercase();
    if m.contains("404") || m.contains("could not fetch a valid release json") {
        return "в последнем релизе на GitHub нет файла обновления (latest.json)".into();
    }
    if m.contains("dns") || m.contains("connect") || m.contains("timed out") {
        return "нет связи с GitHub".into();
    }
    if m.contains("403") || m.contains("rate limit") {
        return "GitHub временно ограничил запросы, попробуйте позже".into();
    }
    let first = message.lines().next().unwrap_or(message);
    if first.chars().count() > 140 {
        format!("{}…", first.chars().take(137).collect::<String>())
    } else {
        first.to_string()
    }
}

async fn check(app: AppHandle) {
    {
        let updates = app.state::<Updates>();
        let mut busy = updates.busy.lock().unwrap();
        if *busy || updates.ready.lock().unwrap().is_some() {
            return;
        }
        *busy = true;
    }
    send(&app, json!({ "status": "checking" }));
    let result = async {
        let updater = app.updater().map_err(|e| e.to_string())?;
        let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
            send(&app, json!({ "status": "latest" }));
            return Ok::<(), String>(());
        };
        let version = update.version.clone();
        send(&app, json!({ "status": "downloading", "version": version, "percent": 0 }));
        let mut got: usize = 0;
        let mut last = 0u64;
        let app2 = app.clone();
        let v2 = version.clone();
        let bytes = update
            .download(
                move |chunk, total| {
                    got += chunk;
                    if let Some(total) = total.filter(|t| *t > 0) {
                        let percent = (got as u64 * 100 / total).min(100);
                        if percent != last {
                            last = percent;
                            send(&app2, json!({ "status": "downloading", "version": v2, "percent": percent }));
                        }
                    }
                },
                || {},
            )
            .await
            .map_err(|e| e.to_string())?;
        *app.state::<Updates>().ready.lock().unwrap() = Some((update, bytes));
        send(&app, json!({ "status": "ready", "version": version }));
        Ok(())
    }
    .await;
    if let Err(e) = result {
        send(&app, json!({ "status": "error", "message": short_error(&e) }));
    }
    *app.state::<Updates>().busy.lock().unwrap() = false;
}

pub fn start(app: &AppHandle) {
    *app.state::<Updates>().state.lock().unwrap() = json!({ "status": "idle" });
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            check(app.clone()).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

#[tauri::command]
pub fn updates_get(app: AppHandle) -> Value {
    app.state::<Updates>().state.lock().unwrap().clone()
}

#[tauri::command]
pub async fn updates_check(app: AppHandle) {
    if cfg!(debug_assertions) {
        send(&app, json!({ "status": "latest" }));
        return;
    }
    check(app).await;
}

#[tauri::command]
pub fn updates_install(app: AppHandle) -> Result<(), String> {
    let ready = app.state::<Updates>().ready.lock().unwrap().take();
    let Some((update, bytes)) = ready else { return Ok(()) };
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}
