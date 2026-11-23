//! Windows' own dialogs and shell: picking files and folders, opening a folder in Explorer.
//! The old-style dialogs of comdlg32/shell32 take no COM setup and no extra library.

use std::path::{Path, PathBuf};

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// audio files to add (several at once)
#[cfg(windows)]
pub fn pick_files() -> Vec<PathBuf> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let mut buf = vec![0u16; 64 * 1024];
    let filter: Vec<u16> = "Музыка\0*.mp3;*.flac;*.m4a;*.aac;*.ogg;*.opus;*.wav;*.wma;*.alac\0Все файлы\0*.*\0\0".encode_utf16().collect();
    // SAFETY: the structure points at buffers that live through the call
    let ok = unsafe {
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = buf.as_mut_ptr();
        ofn.nMaxFile = buf.len() as u32;
        ofn.Flags = OFN_ALLOWMULTISELECT | OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR;
        GetOpenFileNameW(&mut ofn) != 0
    };
    if !ok {
        return vec![];
    }
    // "folder\0file1\0file2\0\0", or one full path
    let parts: Vec<String> = buf.split(|c| *c == 0).take_while(|p| !p.is_empty()).map(String::from_utf16_lossy).collect();
    match parts.as_slice() {
        [] => vec![],
        [one] => vec![PathBuf::from(one)],
        [dir, files @ ..] => files.iter().map(|f| Path::new(dir).join(f)).collect(),
    }
}

/// a folder to add (with everything in it)
#[cfg(windows)]
pub fn pick_folder() -> Option<PathBuf> {
    use windows_sys::Win32::UI::Shell::*;
    let title = wide(&crate::i18n::t("Папка с музыкой"));
    // SAFETY: plain shell calls; the item list is freed after use
    unsafe {
        let mut bi: BROWSEINFOW = std::mem::zeroed();
        bi.lpszTitle = title.as_ptr();
        bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
        let pidl = SHBrowseForFolderW(&bi);
        if pidl.is_null() {
            return None;
        }
        let mut path = vec![0u16; 1024];
        let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr()) != 0;
        windows_sys::Win32::System::Com::CoTaskMemFree(pidl as _);
        if !ok {
            return None;
        }
        let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
        Some(PathBuf::from(String::from_utf16_lossy(&path[..end])))
    }
}

/// open a folder (or a link) the way Windows opens it
#[cfg(windows)]
pub fn open_path(p: &Path) {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let op = wide("open");
    let file = wide(&p.to_string_lossy());
    // SAFETY: NUL-terminated strings alive through the call
    unsafe {
        ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
    }
}

#[cfg(not(windows))]
pub fn pick_files() -> Vec<PathBuf> {
    vec![]
}
#[cfg(not(windows))]
pub fn pick_folder() -> Option<PathBuf> {
    None
}
#[cfg(not(windows))]
pub fn open_path(_p: &Path) {}
