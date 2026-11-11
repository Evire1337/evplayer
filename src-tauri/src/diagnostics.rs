//! What the app's processes take, for the diagnostics page: memory and processor time. On
//! Windows the app is this process plus the WebView2 processes it started (the page, the
//! graphics, the network…), found as children of this one; on Linux likewise the WebKitGTK
//! processes it started. On a phone only this process is read: the page runs in Android's own
//! WebView processes, which an app cannot look into.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    /// the process's own memory, as Task Manager's "Memory" column shows it: on Windows the
    /// private working set, elsewhere the resident memory that is not file-backed. The working
    /// set alone would count the browser engine's shared libraries once in every process.
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
                let memory = own_memory(handle);
                let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
                let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
                let cpu_ms = if GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) != 0 { ticks(kernel) + ticks(user) } else { 0 };
                CloseHandle(handle);
                Some(ProcessInfo { pid: *pid, name: name.clone(), memory, cpu_ms, kind: None })
            }
        })
        .collect()
}

/// The private working set: memory only this process uses, which is what Task Manager shows.
/// Windows before 10 21H2 does not know the longer structure; there the plain working set is
/// the best there is.
#[cfg(windows)]
unsafe fn own_memory(handle: windows_sys::Win32::Foundation::HANDLE) -> u64 {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2};
    let mut ex2: PROCESS_MEMORY_COUNTERS_EX2 = std::mem::zeroed();
    ex2.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
    if K32GetProcessMemoryInfo(handle, &mut ex2 as *mut PROCESS_MEMORY_COUNTERS_EX2 as *mut PROCESS_MEMORY_COUNTERS, ex2.cb) != 0 && ex2.PrivateWorkingSetSize > 0 {
        return ex2.PrivateWorkingSetSize as u64;
    }
    let mut basic: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
    basic.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    if K32GetProcessMemoryInfo(handle, &mut basic, basic.cb) != 0 {
        basic.WorkingSetSize as u64
    } else {
        0
    }
}

/// A "RssAnon:   1234 kB" line of /proc/<pid>/status, in bytes.
#[cfg(not(windows))]
fn status_kb(status: &str, key: &str) -> Option<u64> {
    status.lines().find_map(|l| l.strip_prefix(key)).and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok()).map(|kb| kb * 1024)
}

/// The parent and processor time of a process, from /proc/<pid>/stat: the fields after the
/// name in brackets, where the parent is the 2nd and utime and stime the 12th and 13th.
#[cfg(not(windows))]
fn parent_and_cpu(stat: &str) -> Option<(u32, u64)> {
    let after = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = after.split_whitespace().collect();
    let parent = fields.get(1)?.parse::<u32>().ok()?;
    let tick = |i: usize| fields.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    // 100 ticks a second is what Linux and Android use
    Some((parent, (tick(11) + tick(12)) * 10))
}

/// What a WebKitGTK helper does, by its program name.
#[cfg(not(windows))]
fn webkit_kind(name: &str) -> Option<String> {
    let kind = if name.starts_with("WebKitWebProcess") {
        "renderer"
    } else if name.starts_with("WebKitNetworkProcess") {
        "network"
    } else if name.starts_with("WebKitGPUProcess") {
        "gpu"
    } else {
        return None;
    };
    Some(kind.to_string())
}

/// Linux: this process and everything started from it, read from /proc. WebKitGTK runs the page
/// and the network in processes of its own (sometimes inside its bubblewrap sandbox), all
/// started from this one. On Android the page runs in the system WebView's own processes, which
/// belong to another app and cannot be read: only this process shows.
///
/// Memory is the resident memory that is not file-backed (RssAnon): the libraries every process
/// maps, and the shared buffers WebKit passes between its processes, are left out, so nothing
/// is counted twice. That is close to what a system monitor shows as a process's memory.
#[cfg(not(windows))]
pub fn processes() -> Vec<ProcessInfo> {
    use std::collections::HashMap;

    let me = std::process::id();
    let read = |pid: u32, file: &str| std::fs::read_to_string(format!("/proc/{pid}/{file}")).ok();
    // every process with its parent (only those this user may read)
    let mut parents: HashMap<u32, u32> = HashMap::new();
    if let Ok(dir) = std::fs::read_dir("/proc") {
        for entry in dir.flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
            if let Some((parent, _)) = read(pid, "stat").as_deref().and_then(parent_and_cpu) {
                parents.insert(pid, parent);
            }
        }
    }
    let mut ours = vec![me];
    let mut at = 0;
    while at < ours.len() {
        let pid = ours[at];
        let mut kids: Vec<u32> = parents.iter().filter(|(child, parent)| **parent == pid && !ours.contains(*child)).map(|(child, _)| *child).collect();
        kids.sort_unstable();
        ours.extend(kids);
        at += 1;
    }

    ours.into_iter()
        .filter_map(|pid| {
            let status = read(pid, "status")?;
            let cpu_ms = read(pid, "stat").as_deref().and_then(parent_and_cpu).map(|(_, cpu)| cpu).unwrap_or(0);
            let memory = status_kb(&status, "RssAnon:").or_else(|| status_kb(&status, "VmRSS:")).unwrap_or(0);
            // the program's own name: "comm" is cut at 15 letters (WebKitWebProces)
            let program = read(pid, "cmdline")
                .and_then(|c| c.split('\0').next().map(|arg0| arg0.rsplit('/').next().unwrap_or(arg0).to_string()))
                .filter(|n| !n.is_empty())
                .or_else(|| read(pid, "comm").map(|c| c.trim().to_string()))
                .unwrap_or_default();
            let name = if pid == me { "evplayer".to_string() } else { program };
            let kind = webkit_kind(&name);
            Some(ProcessInfo { pid, name, memory, cpu_ms, kind })
        })
        .collect()
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

    #[cfg(not(windows))]
    #[test]
    fn reads_proc_stat() {
        // a name with spaces and brackets must not shift the fields
        let stat = "4242 (Web Content (x)) S 100 4242 4242 0 -1 4194560 10 0 0 0 250 50 0 0 20 0 1 0 1 1 1";
        assert_eq!(super::parent_and_cpu(stat), Some((100, 3000)));
        assert_eq!(super::status_kb("Name:\tx\nRssAnon:\t  2048 kB\n", "RssAnon:"), Some(2048 * 1024));
        assert_eq!(super::webkit_kind("WebKitWebProcess").as_deref(), Some("renderer"));
        assert_eq!(super::webkit_kind("WebKitNetworkProcess").as_deref(), Some("network"));
        assert_eq!(super::webkit_kind("bwrap"), None);
    }
}
