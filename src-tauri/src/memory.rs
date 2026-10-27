//! While the window is hidden (tray, minimized) nobody looks at the page, so the browser
//! engine is told it may keep as little memory as it can: WebView2 then trims caches and
//! hands memory back to the system. Playback is not affected. Windows only; elsewhere this
//! does nothing.

use tauri::WebviewWindow;

pub fn set_low(win: &WebviewWindow, low: bool) {
    #[cfg(windows)]
    {
        let _ = win.with_webview(move |webview| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
            };
            use windows::core::Interface;
            let Ok(core) = webview.controller().CoreWebView2() else { return };
            if let Ok(v19) = core.cast::<ICoreWebView2_19>() {
                let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
                let _ = v19.SetMemoryUsageTargetLevel(level);
            }
        });
    }
    #[cfg(not(windows))]
    {
        let _ = (win, low);
    }
}
