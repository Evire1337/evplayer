//! The tray icon and the system-wide keys, on one small thread of their own with a hidden
//! message window (Win32 directly: no extra libraries, nothing running between messages).
//!
//! The tray: a click shows the window, the menu has play/pause, next, previous, show and quit,
//! and the tooltip names the song. The keys: whatever Настройки → Клавиши registers as
//! "везде" works from any program, the window hidden or not.

use std::sync::{Mutex, OnceLock};

use crate::app::actions::Action;
use crate::events::{self, Event};

#[cfg(windows)]
mod win {
    pub use windows_sys::Win32::Foundation::*;
    pub use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    pub use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    pub use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    pub use windows_sys::Win32::UI::Shell::*;
    pub use windows_sys::Win32::UI::WindowsAndMessaging::*;
}

/// what the thread is told from outside
enum Order {
    Tooltip(String),
    Hotkeys(Vec<(String, Action)>),
    Playing(bool),
    Quit,
}

static ORDERS: Mutex<Vec<Order>> = Mutex::new(Vec::new());
static THREAD: OnceLock<u32> = OnceLock::new();
static HWND_: OnceLock<isize> = OnceLock::new();
/// the menu's words and the hotkeys' actions, read on the thread
static HOTKEY_ACTIONS: Mutex<Vec<Action>> = Mutex::new(Vec::new());
static PLAYING: Mutex<bool> = Mutex::new(false);

const WM_TRAY: u32 = 0x0400 + 1; // WM_USER + 1
const WM_ORDERS: u32 = 0x8000 + 7; // WM_APP + 7

fn post_orders() {
    #[cfg(windows)]
    if let Some(&h) = HWND_.get() {
        // SAFETY: posting to our own message window
        unsafe { win::PostMessageW(h as _, WM_ORDERS, 0, 0) };
    }
}

fn order(o: Order) {
    ORDERS.lock().unwrap().push(o);
    post_orders();
}

pub fn set_tooltip(text: &str) {
    order(Order::Tooltip(text.to_string()));
}

pub fn set_playing(on: bool) {
    order(Order::Playing(on));
}

/// register these accelerators ("Ctrl+Alt+Space") as system-wide keys; the ones Windows
/// refused (taken by another program) are returned
pub fn set_hotkeys(map: Vec<(String, Action)>) {
    order(Order::Hotkeys(map));
}

pub fn quit() {
    order(Order::Quit);
}

/// "Ctrl+Alt+Right" → (modifiers, virtual key)
#[cfg(windows)]
pub fn parse_accelerator(acc: &str) -> Option<(u32, u32)> {
    use win::*;
    let mut mods = MOD_NOREPEAT;
    let mut vk = 0u32;
    for part in acc.split('+') {
        match part {
            "Ctrl" | "Control" | "CommandOrControl" => mods |= MOD_CONTROL,
            "Alt" => mods |= MOD_ALT,
            "Shift" => mods |= MOD_SHIFT,
            "Super" | "Win" | "Meta" => mods |= MOD_WIN,
            key => {
                vk = match key {
                    "Space" => VK_SPACE as u32,
                    "Right" => VK_RIGHT as u32,
                    "Left" => VK_LEFT as u32,
                    "Up" => VK_UP as u32,
                    "Down" => VK_DOWN as u32,
                    "Enter" => VK_RETURN as u32,
                    "Escape" => VK_ESCAPE as u32,
                    "Tab" => VK_TAB as u32,
                    "Home" => VK_HOME as u32,
                    "End" => VK_END as u32,
                    "PageUp" => VK_PRIOR as u32,
                    "PageDown" => VK_NEXT as u32,
                    "Insert" => VK_INSERT as u32,
                    "Delete" => VK_DELETE as u32,
                    "," => VK_OEM_COMMA as u32,
                    "." => VK_OEM_PERIOD as u32,
                    "MediaPlayPause" => VK_MEDIA_PLAY_PAUSE as u32,
                    "MediaNextTrack" => VK_MEDIA_NEXT_TRACK as u32,
                    "MediaPreviousTrack" => VK_MEDIA_PREV_TRACK as u32,
                    "MediaStop" => VK_MEDIA_STOP as u32,
                    k if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => k.chars().next()?.to_ascii_uppercase() as u32,
                    k if k.starts_with('F') && k[1..].parse::<u32>().is_ok_and(|n| (1..=24).contains(&n)) => VK_F1 as u32 + k[1..].parse::<u32>().ok()? - 1,
                    k if k.starts_with("num") && k[3..].parse::<u32>().is_ok_and(|n| n <= 9) => VK_NUMPAD0 as u32 + k[3..].parse::<u32>().ok()?,
                    _ => return None,
                };
            }
        }
    }
    (vk != 0).then_some((mods, vk))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn copy_tip(dst: &mut [u16; 128], text: &str) {
    let w: Vec<u16> = text.encode_utf16().take(127).collect();
    dst[..w.len()].copy_from_slice(&w);
    dst[w.len()] = 0;
}

#[cfg(windows)]
unsafe extern "system" fn wndproc(hwnd: win::HWND, msg: u32, wparam: win::WPARAM, lparam: win::LPARAM) -> win::LRESULT {
    use win::*;
    match msg {
        WM_TRAY => {
            let event = (lparam & 0xffff) as u32;
            if event == WM_LBUTTONUP {
                events::send(Event::ShowWindow);
            } else if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                tray_menu(hwnd);
            }
            0
        }
        WM_HOTKEY => {
            if let Some(a) = HOTKEY_ACTIONS.lock().unwrap().get(wparam).copied() {
                events::send(Event::Action(a));
            }
            0
        }
        WM_ORDERS => {
            apply_orders(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(windows)]
fn tr(ru: &str) -> Vec<u16> {
    wide(&crate::i18n::t(ru))
}

#[cfg(windows)]
unsafe fn tray_menu(hwnd: win::HWND) {
    use win::*;
    let menu = CreatePopupMenu();
    let playing = *PLAYING.lock().unwrap();
    let items: [(usize, &str); 4] = [(1, "Показать EvPlayer"), (2, if playing { "Пауза" } else { "Играть" }), (3, "Следующий трек"), (4, "Предыдущий трек")];
    for (id, text) in items {
        let w = tr(text);
        AppendMenuW(menu, MF_STRING, id, w.as_ptr());
    }
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    let q = tr("Выход");
    AppendMenuW(menu, MF_STRING, 9, q.as_ptr());
    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    // the menu closes when clicked elsewhere only if its window is in front
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());
    DestroyMenu(menu);
    match cmd {
        1 => events::send(Event::ShowWindow),
        2 => events::send(Event::Action(Action::PlayPause)),
        3 => events::send(Event::Action(Action::Next)),
        4 => events::send(Event::Action(Action::Prev)),
        9 => events::send(Event::Quit),
        _ => {}
    }
}

#[cfg(windows)]
unsafe fn icon_data(hwnd: win::HWND) -> win::NOTIFYICONDATAW {
    use win::*;
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid
}

#[cfg(windows)]
unsafe fn apply_orders(hwnd: win::HWND) {
    use win::*;
    let orders: Vec<Order> = std::mem::take(&mut *ORDERS.lock().unwrap());
    for o in orders {
        match o {
            Order::Tooltip(text) => {
                let mut nid = icon_data(hwnd);
                nid.uFlags = NIF_TIP;
                copy_tip(&mut nid.szTip, &text);
                Shell_NotifyIconW(NIM_MODIFY, &nid);
            }
            Order::Playing(on) => *PLAYING.lock().unwrap() = on,
            Order::Hotkeys(map) => {
                let mut actions = HOTKEY_ACTIONS.lock().unwrap();
                for id in 0..actions.len() {
                    UnregisterHotKey(hwnd, id as i32);
                }
                actions.clear();
                for (acc, action) in map {
                    let Some((mods, vk)) = parse_accelerator(&acc) else { continue };
                    let id = actions.len();
                    if RegisterHotKey(hwnd, id as i32, mods, vk) != 0 {
                        actions.push(action);
                    } else {
                        eprintln!("[keys] {acc} is taken by another program");
                        // keep the ids in step with the list
                        actions.push(Action::Noop);
                    }
                }
            }
            Order::Quit => {
                let nid = icon_data(hwnd);
                Shell_NotifyIconW(NIM_DELETE, &nid);
                PostQuitMessage(0);
            }
        }
    }
}

/// a PNG as a Windows icon (Windows takes PNG data as an icon resource since Vista)
#[cfg(windows)]
unsafe fn png_icon(png: &[u8], size: i32) -> win::HICON {
    use win::*;
    CreateIconFromResourceEx(png.as_ptr(), png.len() as u32, 1, 0x0003_0000, size, size, LR_DEFAULTCOLOR)
}

/// start the thread: the tray icon appears, keys can be registered
#[cfg(windows)]
pub fn start() {
    if THREAD.get().is_some() {
        return;
    }
    std::thread::Builder::new()
        .name("tray".into())
        .spawn(|| unsafe {
            use win::*;
            let _ = THREAD.set(GetCurrentThreadId());
            let class = wide("EvPlayerTray");
            let hinst = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: hinst, lpszClassName: class.as_ptr(), ..std::mem::zeroed() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), hinst, std::ptr::null());
            if hwnd.is_null() {
                return;
            }
            let _ = HWND_.set(hwnd as isize);
            let mut nid = icon_data(hwnd);
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY;
            nid.hIcon = png_icon(include_bytes!("../../src-tauri/icons/32x32.png"), 16);
            copy_tip(&mut nid.szTip, "EvPlayer");
            Shell_NotifyIconW(NIM_ADD, &nid);
            // orders sent before the window existed
            apply_orders(hwnd);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .ok();
}

#[cfg(not(windows))]
pub fn start() {}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn reads_accelerators() {
        use super::parse_accelerator;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        let (m, k) = parse_accelerator("Ctrl+Alt+Space").unwrap();
        assert_eq!(k, VK_SPACE as u32);
        assert!(m & MOD_CONTROL != 0 && m & MOD_ALT != 0);
        assert_eq!(parse_accelerator("Ctrl+Alt+L").unwrap().1, 'L' as u32);
        assert_eq!(parse_accelerator("F7").unwrap().1, VK_F7 as u32);
        assert_eq!(parse_accelerator("Ctrl+num5").unwrap().1, VK_NUMPAD5 as u32);
        assert_eq!(parse_accelerator("MediaPlayPause").unwrap().1, VK_MEDIA_PLAY_PAUSE as u32);
        assert!(parse_accelerator("Ctrl+Nonsense").is_none());
    }
}
