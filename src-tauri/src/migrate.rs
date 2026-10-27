//! Moving over from the Electron version: its saved state (library, playlists, settings,
//! statistics) lived in Chromium's localStorage, a LevelDB folder in its data folder. The
//! new version reads that folder once, on first start, and the page writes the values into
//! its own storage. Local-file index and covers sit in the same data folder and are used
//! as they are.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// keys of a page loaded from file:// in the Electron version
const PREFIX: &[u8] = b"_file://\x00\x01";

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

/// the localStorage of the Electron version, or nothing if there is none
pub fn read_electron_storage(electron_data: &Path, scratch: &Path) -> Option<BTreeMap<String, String>> {
    let source = electron_data.join("Local Storage").join("leveldb");
    if !source.is_dir() {
        return None;
    }
    // work on a copy: the old app may still be running and holding the folder
    let copy: PathBuf = scratch.join("electron-localstorage");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(&source, &copy).ok()?;
    let mut options = rusty_leveldb::Options::default();
    options.create_if_missing = false;
    let mut db = rusty_leveldb::DB::open(&copy, options).ok()?;
    let mut iter = db.new_iter().ok()?;
    let mut out = BTreeMap::new();
    use rusty_leveldb::LdbIterator;
    while let Some((key, value)) = iter.next() {
        let Some(name) = key.strip_prefix(PREFIX) else { continue };
        let (Ok(name), Some(value)) = (String::from_utf8(name.to_vec()), decode(&value)) else { continue };
        out.insert(name, value);
    }
    drop(iter);
    drop(db);
    let _ = std::fs::remove_dir_all(&copy);
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_both_string_kinds() {
        assert_eq!(decode(b"\x01abc").as_deref(), Some("abc"));
        let utf16: Vec<u8> = std::iter::once(0u8).chain("дом".encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
        assert_eq!(decode(&utf16).as_deref(), Some("дом"));
    }

    /// runs against a real Electron profile when one exists on this machine
    #[test]
    fn reads_a_real_profile_if_present() {
        let Some(home) = std::env::var_os("HOME") else { return };
        let dir = Path::new(&home).join(".config/EvPlayer");
        if !dir.join("Local Storage/leveldb").is_dir() {
            return;
        }
        let tmp = std::env::temp_dir().join("evplayer-migrate-test");
        let found = read_electron_storage(&dir, &tmp).expect("storage");
        assert!(found.keys().any(|k| k.starts_with("evplayer.")), "{:?}", found.keys().collect::<Vec<_>>());
    }
}
