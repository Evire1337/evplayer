//! What the app's processes take, for the diagnostics page: memory and processor time. On
//! Windows the app is this process plus the WebView2 processes it started (the page, the
//! graphics, the network…), found as children of this one; elsewhere only this process is read
//! (on a phone the page runs in Android's own WebView processes, which an app cannot look into).

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    /// working set: what Task Manager shows as the process's memory
    pub memory: u64,
    /// processor time used since the process started
    pub cpu_ms: u64,
    /// what the process does for WebView2 ("renderer", "gpu", "browser", "utility"), when known
    pub kind: Option<String>,
}

#[cfg(windows)]
pub fn processes() -> Vec<ProcessInfo> {
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // every process with its parent and name
    let mut all: Vec<(u32, u32, String)> = Vec::new();
    // SAFETY: a snapshot handle checked before use and closed after; the entry is sized for the API
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            all.push((entry.th32ProcessID, entry.th32ParentProcessID, String::from_utf16_lossy(&entry.szExeFile[..len])));
            ok = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
    }

    // this process and everything started from it (WebView2 starts its own helpers)
    let mut children: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, (_, parent, _)) in all.iter().enumerate() {
        children.entry(*parent).or_default().push(i);
    }
    // SAFETY: no arguments, always succeeds
    let me = unsafe { GetCurrentProcessId() };
    let mut ours: Vec<usize> = all.iter().position(|(pid, _, _)| *pid == me).into_iter().collect();
    let mut at = 0;
    while at < ours.len() {
        let pid = all[ours[at]].0;
        for &c in children.get(&pid).map(Vec::as_slice).unwrap_or(&[]) {
            // a pid can be reused by an unrelated process whose parent died: not ours then
            if all[c].0 != pid && !ours.contains(&c) {
                ours.push(c);
            }
        }
        at += 1;
    }

    let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64) / 10_000;
    ours.into_iter()
        .filter_map(|i| {
            let (pid, _, name) = &all[i];
            // SAFETY: the handle is checked and closed; the out structures are sized for the API
            unsafe {
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, *pid);
                if handle.is_null() {
                    return None;
                }
                let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
                counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
                let memory = if K32GetProcessMemoryInfo(handle, &mut counters, counters.cb) != 0 { counters.WorkingSetSize as u64 } else { 0 };
                let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
                let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
                let cpu_ms = if GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) != 0 { ticks(kernel) + ticks(user) } else { 0 };
                CloseHandle(handle);
                Some(ProcessInfo { pid: *pid, name: name.clone(), memory, cpu_ms, kind: None })
            }
        })
        .collect()
}

#[cfg(not(windows))]
pub fn processes() -> Vec<ProcessInfo> {
    // /proc/self: resident memory from status, processor time from stat (in clock ticks)
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let memory = status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .unwrap_or(0);
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // fields after the name in brackets: utime and stime are the 12th and 13th of them
    let after = stat.rsplit_once(')').map(|(_, rest)| rest).unwrap_or("");
    let fields: Vec<&str> = after.split_whitespace().collect();
    let tick = |i: usize| fields.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    // 100 ticks a second is what Linux and Android use
    let cpu_ms = (tick(11) + tick(12)) * 10;
    vec![ProcessInfo { pid: std::process::id(), name: "evplayer".into(), memory, cpu_ms, kind: None }]
}

/// What each WebView2 process is for, as WebView2 itself tells: a page, the graphics, the
/// manager or a helper (network, audio, storage). Asked on the window's own thread.
#[cfg(windows)]
pub async fn webview_kinds(app: &tauri::AppHandle) -> std::collections::HashMap<u32, String> {
    use tauri::Manager;
    let Some(win) = app.get_webview_window("main") else { return Default::default() };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let asked = win.with_webview(move |webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2Environment8, ICoreWebView2_2, COREWEBVIEW2_PROCESS_KIND_BROWSER, COREWEBVIEW2_PROCESS_KIND_GPU,
            COREWEBVIEW2_PROCESS_KIND_RENDERER,
        };
        use windows::core::Interface;
        let read = || -> windows::core::Result<std::collections::HashMap<u32, String>> {
            let mut out = std::collections::HashMap::new();
            // SAFETY: COM calls on the window's thread, on objects WebView2 hands out
            unsafe {
                let core = webview.controller().CoreWebView2()?;
                let environment = core.cast::<ICoreWebView2_2>()?.Environment()?.cast::<ICoreWebView2Environment8>()?;
                let infos = environment.GetProcessInfos()?;
                let mut count = 0u32;
                infos.Count(&mut count)?;
                for i in 0..count {
                    let info = infos.GetValueAtIndex(i)?;
                    let mut kind = COREWEBVIEW2_PROCESS_KIND_BROWSER;
                    info.Kind(&mut kind)?;
                    let name = if kind == COREWEBVIEW2_PROCESS_KIND_RENDERER {
                        "renderer"
                    } else if kind == COREWEBVIEW2_PROCESS_KIND_GPU {
                        "gpu"
                    } else if kind == COREWEBVIEW2_PROCESS_KIND_BROWSER {
                        "browser"
                    } else {
                        "utility"
                    };
                    let mut pid = 0i32;
                    info.ProcessId(&mut pid)?;
                    out.insert(pid as u32, name.to_string());
                }
            }
            Ok(out)
        };
        let _ = tx.send(read().unwrap_or_default());
    });
    if asked.is_err() {
        return Default::default();
    }
    tokio::time::timeout(std::time::Duration::from_secs(2), rx).await.ok().and_then(Result::ok).unwrap_or_default()
}

#[cfg(not(windows))]
pub async fn webview_kinds(_app: &tauri::AppHandle) -> std::collections::HashMap<u32, String> {
    Default::default()
}
#[cfg(test)]
mod tests {
    #[test]
    fn finds_this_process() {
        let list = super::processes();
        let me = list.iter().find(|p| p.pid == std::process::id()).expect("this process");
        assert!(me.memory > 0);
    }
}
