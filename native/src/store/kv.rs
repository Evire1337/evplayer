//! A small key-value store on disk, in the shape of the page's localStorage: each key
//! (`evplayer.library.v1`, ...) is a file holding the same JSON text the Tauri build kept
//! under that key. Writes are gathered and made a moment later on a thread of their own, each
//! through a temporary file and a rename, so a crash never leaves half a file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// changes are written this long after the last one (a drag in a list makes many)
const SAVE_AFTER: Duration = Duration::from_millis(700);

pub struct Kv {
    dir: PathBuf,
    values: Mutex<BTreeMap<String, String>>,
    dirty: Mutex<BTreeSet<String>>,
    wake: Condvar,
}

fn file_of(dir: &Path, key: &str) -> PathBuf {
    // keys are ours ("evplayer.x.v1"); anything else in them is replaced
    let safe: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect();
    dir.join(format!("{safe}.json"))
}

impl Kv {
    /// the store in `dir`, with what is saved there; `seed` gives the values for an empty store
    pub fn open(dir: PathBuf, seed: impl FnOnce() -> Option<BTreeMap<String, String>>) -> Arc<Self> {
        let _ = std::fs::create_dir_all(&dir);
        let mut values = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Some(key) = name.strip_suffix(".json") else { continue };
                if let Ok(text) = std::fs::read_to_string(e.path()) {
                    values.insert(key.to_string(), text);
                }
            }
        }
        let fresh = values.is_empty();
        let me = Arc::new(Self { dir, values: Mutex::new(values), dirty: Mutex::new(BTreeSet::new()), wake: Condvar::new() });
        if fresh {
            if let Some(seeded) = seed() {
                for (k, v) in seeded {
                    me.set(&k, v);
                }
                me.flush();
            }
        }
        let saver = me.clone();
        let _ = std::thread::Builder::new().name("store".into()).spawn(move || saver.saver());
        me
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.values.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: &str, value: String) {
        {
            let mut v = self.values.lock().unwrap();
            if v.get(key) == Some(&value) {
                return;
            }
            v.insert(key.to_string(), value);
        }
        self.dirty.lock().unwrap().insert(key.to_string());
        self.wake.notify_all();
    }

    pub fn remove(&self, key: &str) {
        self.values.lock().unwrap().remove(key);
        let _ = std::fs::remove_file(file_of(&self.dir, key));
    }

    /// how many bytes a key takes (Настройки → память)
    pub fn size(&self, key: &str) -> usize {
        self.values.lock().unwrap().get(key).map(|v| v.len()).unwrap_or(0)
    }

    /// write everything changed now (on quit)
    pub fn flush(&self) {
        let keys: Vec<String> = std::mem::take(&mut *self.dirty.lock().unwrap()).into_iter().collect();
        for key in keys {
            let Some(value) = self.get(&key) else { continue };
            let path = file_of(&self.dir, &key);
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, value.as_bytes()).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    fn saver(self: Arc<Self>) {
        loop {
            {
                let mut dirty = self.dirty.lock().unwrap();
                while dirty.is_empty() {
                    dirty = self.wake.wait(dirty).unwrap();
                }
            }
            // more changes may follow right away: wait for a quiet moment
            std::thread::sleep(SAVE_AFTER);
            self.flush();
        }
    }
}

/// a zustand `persist` value: `{"state": …, "version": N}`
pub fn read_state<T: serde::de::DeserializeOwned>(kv: &Kv, key: &str) -> Option<T> {
    let text = kv.get(key)?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let state = v.get("state").cloned().unwrap_or(v);
    serde_json::from_value(state).map_err(|e| eprintln!("[store] {key}: {e}")).ok()
}

pub fn write_state<T: serde::Serialize>(kv: &Kv, key: &str, state: &T, version: u32) {
    if let Ok(state) = serde_json::to_value(state) {
        let v = serde_json::json!({ "state": state, "version": version });
        kv.set(key, v.to_string());
    }
}

/// a plain JSON value (not wrapped by zustand)
pub fn read_plain<T: serde::de::DeserializeOwned>(kv: &Kv, key: &str) -> Option<T> {
    serde_json::from_str(&kv.get(key)?).ok()
}

pub fn write_plain<T: serde::Serialize>(kv: &Kv, key: &str, value: &T) {
    if let Ok(s) = serde_json::to_string(value) {
        kv.set(key, s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_values_between_runs_and_seeds_an_empty_store() {
        let dir = std::env::temp_dir().join(format!("evplayer-kv-test-{}", rand::random::<u32>()));
        {
            let kv = Kv::open(dir.clone(), || Some([("evplayer.a.v1".to_string(), r#"{"state":{"x":1},"version":0}"#.to_string())].into()));
            #[derive(serde::Deserialize, serde::Serialize)]
            struct S {
                x: i32,
            }
            let s: S = read_state(&kv, "evplayer.a.v1").unwrap();
            assert_eq!(s.x, 1);
            write_state(&kv, "evplayer.a.v1", &S { x: 2 }, 0);
            kv.flush();
        }
        // a second run reads the files, and the seed is not used again
        let kv = Kv::open(dir.clone(), || panic!("seeded twice"));
        assert!(kv.get("evplayer.a.v1").unwrap().contains("\"x\":2"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
