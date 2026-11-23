//! A backup is every EvPlayer setting and list kept on this computer (likes, playlists,
//! statistics, history, equalizer presets, settings) in one JSON file, the same file the Tauri
//! build makes (src/settings/backup.ts): a copy made in either opens in the other. Your own
//! audio files are not inside, only the list of them stays with the app.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::store::kv::Kv;

const PREFIX: &str = "evplayer.";

/// caches that rebuild themselves are left out
fn skipped(key: &str) -> bool {
    key.starts_with("evplayer.loudness")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub app: String,
    pub version: u32,
    pub created_at: String,
    pub data: BTreeMap<String, String>,
}

pub fn make(kv: &Kv) -> BackupFile {
    let data = kv.snapshot().into_iter().filter(|(k, _)| k.starts_with(PREFIX) && !skipped(k)).collect();
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    BackupFile { app: "EvPlayer".into(), version: 1, created_at, data }
}

/// "evplayer-backup-2026-11-23.json"
pub fn file_name(b: &BackupFile) -> String {
    format!("evplayer-backup-{}.json", b.created_at.get(..10).unwrap_or("copy"))
}

pub fn read(text: &str) -> Result<BackupFile, String> {
    let not_ours = || "Это не файл резервной копии EvPlayer".to_string();
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| not_ours())?;
    if v.get("app").and_then(|a| a.as_str()) != Some("EvPlayer") || v.get("version").and_then(|a| a.as_u64()) != Some(1) {
        return Err(not_ours());
    }
    let created_at = v.get("createdAt").and_then(|a| a.as_str()).unwrap_or_default().to_string();
    let data = v.get("data").and_then(|d| d.as_object()).ok_or_else(not_ours)?;
    // values that are not text are skipped, like the page does
    let data = data.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
    Ok(BackupFile { app: "EvPlayer".into(), version: 1, created_at, data })
}

/// what is on this computer is replaced with the backup; the program starts again after this
pub fn restore(kv: &Kv, b: &BackupFile) -> usize {
    let data: BTreeMap<String, String> = b.data.iter().filter(|(k, _)| k.starts_with(PREFIX)).map(|(k, v)| (k.clone(), v.clone())).collect();
    kv.replace_all(PREFIX, &data);
    data.len()
}

/// "23.11.2026, 14:05" for the question before putting a backup back
pub fn when(b: &BackupFile) -> String {
    chrono::DateTime::parse_from_rfc3339(&b.created_at).map(|t| t.with_timezone(&chrono::Local).format("%d.%m.%Y, %H:%M").to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_evplayer_keys_only_and_reads_the_page_s_files() {
        let dir = std::env::temp_dir().join(format!("evplayer-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let kv = Kv::open(dir.clone(), || None);
        kv.set("evplayer.library.v1", "{\"a\":1}".into());
        kv.set("evplayer.loudness.v1", "x".into());
        kv.set("other", "y".into());
        let text = serde_json::to_string(&make(&kv)).unwrap();
        let b = read(&text).unwrap();
        assert_eq!(b.data.keys().collect::<Vec<_>>(), vec!["evplayer.library.v1"]);
        kv.set("evplayer.library.v1", "changed".into());
        kv.set("evplayer.extra", "z".into());
        assert_eq!(restore(&kv, &b), 1);
        assert_eq!(kv.get("evplayer.library.v1").as_deref(), Some("{\"a\":1}"));
        assert_eq!(kv.get("evplayer.extra"), None);
        assert_eq!(kv.get("other").as_deref(), Some("y"));
        // it is on disk at once, and nothing is written after it
        assert_eq!(std::fs::read_to_string(dir.join("evplayer.library.v1.json")).unwrap(), "{\"a\":1}");
        kv.set("evplayer.library.v1", "late".into());
        assert_eq!(kv.get("evplayer.library.v1").as_deref(), Some("{\"a\":1}"));
        // a file made by the Tauri build
        let page = r#"{"app":"EvPlayer","version":1,"createdAt":"2026-11-20T10:00:00.000Z","data":{"evplayer.settings.v1":"{\"state\":{},\"version\":1}"}}"#;
        assert_eq!(read(page).unwrap().data.len(), 1);
        assert_eq!(file_name(&read(page).unwrap()), "evplayer-backup-2026-11-20.json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_other_files() {
        assert!(read("{}").is_err());
        assert!(read("not json").is_err());
    }
}
