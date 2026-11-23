//! Updates (src-tauri/src/updates.rs): from the GitHub releases of this repository, the same
//! signed latest.json, checked 10 s after the start and every 4 hours (not in a development
//! build). A new version downloads in the background, its signature is checked with the key
//! the Tauri build trusts, and the program offers a restart.
//!
//! The native build reads its own entry of latest.json, `windows-x86_64-native`: the program
//! file itself (not an installer) and its signature. Installing renames the running file aside
//! (Windows allows that), puts the new one in its place and starts it; the old one is removed
//! on the next start. A release without that entry is reported as such, never mixed up with
//! the Tauri installer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

const LATEST: &str = "https://github.com/Evire1337/evplayer/releases/latest/download/latest.json";
/// latest.json's platform entry for this build
const PLATFORM: &str = "windows-x86_64-native";
/// the key from tauri.conf.json (base64 of the minisign public key file)
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDk1MDgzMzcyNTcwRDlFMzkKUldRNW5nMVhjak1JbGRnVjV2OHlFRVU0ZXVVWjFQNVFDMENrRzk1ckxFZFArS2lLL3NGSytLVUkK";
const CHECK_EVERY: Duration = Duration::from_secs(4 * 60 * 60);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);
/// a program file is a few tens of MB; anything far larger is not one
const MAX_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    Downloading { version: String, percent: u32 },
    Ready { version: String },
    Latest,
    Error(String),
}

static STATE: Mutex<Status> = Mutex::new(Status::Idle);
static READY: Mutex<Option<Vec<u8>>> = Mutex::new(None);
static BUSY: AtomicBool = AtomicBool::new(false);

pub fn status() -> Status {
    STATE.lock().unwrap().clone()
}

fn set(s: Status) {
    *STATE.lock().unwrap() = s;
    crate::events::repaint();
}

/// "проверяются сами при запуске и каждые 4 часа", as the page says it
pub fn line(s: &Status) -> String {
    use crate::i18n::t;
    match s {
        Status::Checking => t("Проверяю…"),
        Status::Downloading { version, percent } => format!("{} {version}: {percent}%", t("Скачиваю")),
        Status::Ready { version } => format!("{} {version} {}", t("Версия"), t("готова — установится при перезапуске")),
        Status::Latest => t("У вас последняя версия"),
        Status::Error(m) => format!("{} {m}", t("Не получилось проверить:")),
        Status::Idle => t("Проверяются сами при запуске и каждые 4 часа"),
    }
}

/// a short, readable reason instead of a full HTTP dump
pub fn short_error(message: &str) -> String {
    let m = message.to_lowercase();
    if m.contains("404") {
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

/// only GitHub's own hosts, also after a redirect (release files are served from its CDN)
fn github(u: &reqwest::Url) -> bool {
    u.scheme() == "https" && u.host_str().is_some_and(|h| h == "github.com" || h.ends_with(".github.com") || h.ends_with(".githubusercontent.com"))
}

fn client() -> Result<reqwest::blocking::Client, String> {
    crate::userproxy::apply_blocking(reqwest::blocking::Client::builder())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .user_agent(concat!("EvPlayer/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|a| if a.previous().len() > 8 || !github(a.url()) { a.stop() } else { a.follow() }))
        .build()
        .map_err(|e| e.to_string())
}

/// "0.7.10" > "0.7.9"
pub fn newer(remote: &str, local: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.trim_start_matches('v').split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect() };
    parts(remote) > parts(local)
}

/// the signature in latest.json (base64 of a minisign signature) checked against the key
pub fn verify(bytes: &[u8], signature_b64: &str) -> Result<(), String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let key_text = String::from_utf8(b64.decode(PUBKEY).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let key = minisign_verify::PublicKey::decode(&key_text).map_err(|e| format!("ключ: {e}"))?;
    let sig_text = String::from_utf8(b64.decode(signature_b64.trim()).map_err(|_| "подпись повреждена".to_string())?).map_err(|_| "подпись повреждена".to_string())?;
    let sig = minisign_verify::Signature::decode(&sig_text).map_err(|_| "подпись повреждена".to_string())?;
    key.verify(bytes, &sig, true).map_err(|_| "подпись не сходится — файл не от автора".to_string())
}

/// look for a new version and download it (blocking: from a thread)
pub fn check() {
    if READY.lock().unwrap().is_some() || BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    set(Status::Checking);
    let result = (|| -> Result<(), String> {
        let c = client()?;
        let res = c.get(LATEST).send().map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(res.status().as_u16().to_string());
        }
        let latest: serde_json::Value = res.json().map_err(|e| e.to_string())?;
        let version = latest.get("version").and_then(|v| v.as_str()).unwrap_or_default().trim_start_matches('v').to_string();
        if !newer(&version, env!("CARGO_PKG_VERSION")) {
            set(Status::Latest);
            return Ok(());
        }
        let Some(entry) = latest.get("platforms").and_then(|p| p.get(PLATFORM)) else {
            return Err(format!("вышла версия {version}, но нативной сборки в релизе нет — её можно взять со страницы релизов на GitHub"));
        };
        let url = entry.get("url").and_then(|v| v.as_str()).ok_or("в latest.json нет адреса файла")?;
        let signature = entry.get("signature").and_then(|v| v.as_str()).ok_or("в latest.json нет подписи")?;
        let u = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
        if !github(&u) {
            return Err("файл обновления не на GitHub".into());
        }
        set(Status::Downloading { version: version.clone(), percent: 0 });
        let mut res = c.get(u).send().map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(res.status().as_u16().to_string());
        }
        let total = res.content_length().unwrap_or(0);
        if total > MAX_BYTES {
            return Err("файл обновления слишком большой".into());
        }
        let mut bytes = Vec::with_capacity(total as usize);
        let mut buf = vec![0u8; 64 * 1024];
        let mut last = 0;
        loop {
            use std::io::Read;
            let n = res.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buf[..n]);
            if bytes.len() as u64 > MAX_BYTES {
                return Err("файл обновления слишком большой".into());
            }
            if total > 0 {
                let percent = (bytes.len() as u64 * 100 / total).min(100) as u32;
                if percent != last {
                    last = percent;
                    set(Status::Downloading { version: version.clone(), percent });
                }
            }
        }
        verify(&bytes, signature)?;
        // a Windows program starts with "MZ"
        if !bytes.starts_with(b"MZ") {
            return Err("файл обновления — не программа".into());
        }
        *READY.lock().unwrap() = Some(bytes);
        set(Status::Ready { version });
        Ok(())
    })();
    if let Err(e) = result {
        set(Status::Error(short_error(&e)));
    }
    BUSY.store(false, Ordering::SeqCst);
}

/// checks in the background, as the Tauri build does (a development build does not)
pub fn start() {
    remove_old();
    if cfg!(debug_assertions) {
        return;
    }
    let _ = std::thread::Builder::new().name("updates".into()).spawn(|| {
        std::thread::sleep(FIRST_CHECK_AFTER);
        loop {
            check();
            std::thread::sleep(CHECK_EVERY);
        }
    });
}

/// "Проверить" by hand
pub fn check_now() {
    if cfg!(debug_assertions) {
        set(Status::Latest);
        return;
    }
    let _ = std::thread::Builder::new().name("updates".into()).spawn(check);
}

fn old_path(exe: &std::path::Path) -> std::path::PathBuf {
    exe.with_extension("exe.old")
}

/// the file the last update set aside
fn remove_old() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_path(&exe));
    }
}

/// put the downloaded version in place and start it
pub fn install() -> Result<(), String> {
    let Some(bytes) = READY.lock().unwrap().take() else { return Ok(()) };
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = old_path(&exe);
    let _ = std::fs::remove_file(&old);
    let fresh = exe.with_extension("exe.new");
    std::fs::write(&fresh, &bytes).map_err(|e| format!("не удалось записать: {e}"))?;
    std::fs::rename(&exe, &old).map_err(|e| format!("не удалось заменить: {e}"))?;
    if let Err(e) = std::fs::rename(&fresh, &exe) {
        // put the running one back so the program still starts
        let _ = std::fs::rename(&old, &exe);
        return Err(format!("не удалось заменить: {e}"));
    }
    crate::system::restart();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.7.10", "0.7.9"));
        assert!(newer("v0.8.0", "0.7.6"));
        assert!(!newer("0.7.6", "0.7.6"));
        assert!(!newer("0.7.5", "0.7.6"));
    }

    #[test]
    fn a_bad_signature_is_refused_and_the_key_reads() {
        use base64::Engine;
        let fake = base64::engine::general_purpose::STANDARD.encode("untrusted comment: x\nRWQ5ng1XcjMIlQ==\ntrusted comment: y\nAA==\n");
        assert!(verify(b"MZ...", &fake).is_err());
        assert!(verify(b"MZ...", "not base64!").is_err());
        let key = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(PUBKEY).unwrap()).unwrap();
        assert!(minisign_verify::PublicKey::decode(&key).is_ok());
    }

    #[test]
    fn only_github_is_asked() {
        assert!(github(&reqwest::Url::parse("https://objects.githubusercontent.com/x").unwrap()));
        assert!(github(&reqwest::Url::parse("https://github.com/Evire1337/evplayer").unwrap()));
        assert!(!github(&reqwest::Url::parse("https://evil.example/github.com").unwrap()));
        assert!(!github(&reqwest::Url::parse("http://github.com/").unwrap()));
    }
}
