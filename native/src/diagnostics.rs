//! What the program takes from the computer right now (Настройки → О программе), and the
//! measurement mode used for the README: `--measure N` writes the memory after N seconds.

pub struct Memory {
    /// private bytes: what only this program holds
    pub private_mb: f64,
    /// working set: what of it is in RAM now (with shared system files)
    pub working_mb: f64,
}

#[cfg(windows)]
pub fn memory() -> Memory {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: a counters structure of the size the call is told
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut _, c.cb) != 0 {
            return Memory { private_mb: c.PrivateUsage as f64 / 1_048_576.0, working_mb: c.WorkingSetSize as f64 / 1_048_576.0 };
        }
    }
    Memory { private_mb: 0.0, working_mb: 0.0 }
}

#[cfg(not(windows))]
pub fn memory() -> Memory {
    Memory { private_mb: 0.0, working_mb: 0.0 }
}
