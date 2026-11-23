//! Moving over from the Tauri build: its page kept everything (library, settings, history,
//! statistics, the equalizer) in WebView2's localStorage, a LevelDB folder under
//! %LOCALAPPDATA%\io.github.evire1337.evplayer. The values there are the same JSON the
//! native build writes (zustand's `{"state": …, "version": N}`), so they are read once, on
//! the first start, and written into the native store as they are. The folder is read from a
//! copy, so a Tauri build that is still running is not disturbed, and nothing is changed in it:
//! both builds can be used side by side.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// the page's origin in the Tauri build on Windows, as Chromium prefixes its keys
const ORIGINS: &[&[u8]] = &[b"_http://tauri.localhost\x00\x01", b"_https://tauri.localhost\x00\x01"];

/// where WebView2 keeps the Tauri build's page data
pub fn webview_storage_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("io.github.evire1337.evplayer").join("EBWebView").join("Default").join("Local Storage").join("leveldb"))
}

fn decode(value: &[u8]) -> Option<String> {
    let (&kind, rest) = value.split_first()?;
    match kind {
        // UTF-16LE
        0 => {
            let units: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            String::from_utf16(&units).ok()
        }
        // Latin-1
        1 => Some(rest.iter().map(|&b| b as char).collect()),
        _ => None,
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "LOCK" {
            continue;
        }
        if entry.file_type()?.is_file() {
            std::fs::copy(entry.path(), to.join(name))?;
        }
    }
    Ok(())
}

/// every `evplayer.*` value of the Tauri build's page, or nothing if there is none
pub fn read_webview_storage(source: &Path, scratch: &Path) -> Option<BTreeMap<String, String>> {
    if !source.is_dir() {
        return None;
    }
    let copy = scratch.join("webview-localstorage");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(source, &copy).ok()?;
    let mut options = rusty_leveldb::Options::default();
    options.create_if_missing = false;
    let mut out = BTreeMap::new();
    {
        let mut db = rusty_leveldb::DB::open(&copy, options).ok()?;
        let mut iter = db.new_iter().ok()?;
        use rusty_leveldb::LdbIterator;
        while let Some((key, value)) = iter.next() {
            let Some(name) = ORIGINS.iter().find_map(|o| key.strip_prefix(*o)) else { continue };
            let (Ok(name), Some(value)) = (String::from_utf8(name.to_vec()), decode(&value)) else { continue };
            if name.starts_with("evplayer.") {
                out.insert(name, value);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&copy);
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// on a computer with the Tauri build: its saved keys are found (names and sizes only)
    #[test]
    #[ignore]
    fn lists_the_real_profile() {
        let Some(dir) = webview_storage_dir() else { return };
        let scratch = std::env::temp_dir().join("evplayer-native-import-test");
        let found = read_webview_storage(&dir, &scratch).expect("storage");
        for (k, v) in &found {
            println!("{k}: {} bytes", v.len());
        }
        assert!(found.contains_key("evplayer.settings.v1"));
    }

    fn shape(v: &serde_json::Value, depth: usize) -> String {
        match v {
            serde_json::Value::Object(m) if depth < 4 => format!("{{{}}}", m.iter().take(40).map(|(k, v)| format!("{}: {}", if k.contains(':') || k.len() > 24 { "<id>" } else { k }, shape(v, depth + 1))).collect::<Vec<_>>().join(", ")),
            serde_json::Value::Object(_) => "{…}".into(),
            serde_json::Value::Array(a) => format!("[{}; {}]", a.first().map(|x| shape(x, depth + 1)).unwrap_or_default(), a.len()),
            serde_json::Value::String(_) => "str".into(),
            serde_json::Value::Number(_) => "num".into(),
            serde_json::Value::Bool(_) => "bool".into(),
            serde_json::Value::Null => "null".into(),
        }
    }

    /// the shapes (names and kinds, no values) of what the Tauri build saved
    #[test]
    #[ignore]
    fn shapes_of_the_real_profile() {
        let Some(dir) = webview_storage_dir() else { return };
        let found = read_webview_storage(&dir, &std::env::temp_dir().join("evplayer-native-import-shape")).unwrap();
        for (k, v) in &found {
            match serde_json::from_str::<serde_json::Value>(v) {
                Ok(j) => {
                    let mut s = shape(&j, 0);
                    s.truncate(1400);
                    println!("{k} => {s}")
                }
                Err(_) => println!("{k} => (not json) {} chars", v.len()),
            }
        }
    }
}
