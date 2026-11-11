//! Who the app says it is to Windows. The media flyout (volume and media keys, the lock
//! screen) names the app that plays by its AppUserModelID; an app that has none registered is
//! shown as "Unknown app". The id is set for this process and registered under the user's
//! own settings (name and picture), so no installer step is needed. Windows only.

#[cfg(windows)]
pub const APP_ID: &str = "io.github.evire1337.evplayer";

#[cfg(windows)]
pub fn register(data: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let wide: Vec<u16> = std::ffi::OsStr::new(APP_ID).encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: a NUL-terminated wide string that stays alive through the call
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(wide.as_ptr()) };

    // the name and picture for the id; off the main path, it starts a small program a few times
    let data = data.to_path_buf();
    std::thread::spawn(move || {
        use std::os::windows::process::CommandExt;
        let icon = data.join("identity.png");
        // without the picture the name still shows
        let _ = std::fs::write(&icon, include_bytes!("../icons/128x128.png"));
        let key = format!(r"HKCU\Software\Classes\AppUserModelId\{APP_ID}");
        for (name, value) in [("DisplayName", "EvPlayer".to_string()), ("IconUri", icon.to_string_lossy().to_string())] {
            // CREATE_NO_WINDOW: no console flashes up
            let _ = std::process::Command::new("reg").args(["add", &key, "/v", name, "/t", "REG_SZ", "/d", &value, "/f"]).creation_flags(0x0800_0000).output();
        }
    });
}

#[cfg(not(windows))]
pub fn register(_data: &std::path::Path) {}
