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

/// this process's processor time so far (all threads), in seconds
#[cfg(windows)]
fn cpu_seconds() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let z = || FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut a, mut b, mut k, mut u) = (z(), z(), z(), z());
    // SAFETY: four FILETIMEs filled by the call
    unsafe {
        if GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u) == 0 {
            return 0.0;
        }
    }
    let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 / 1e7;
    t(k) + t(u)
}

#[cfg(not(windows))]
fn cpu_seconds() -> f64 {
    0.0
}

/// threads of this process
#[cfg(windows)]
fn threads() -> usize {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    let me = std::process::id();
    let mut n = 0;
    // SAFETY: a snapshot walked and closed
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snap.is_null() || snap == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return 0;
        }
        let mut e: THREADENTRY32 = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        if Thread32First(snap, &mut e) != 0 {
            loop {
                if e.th32OwnerProcessID == me {
                    n += 1;
                }
                if Thread32Next(snap, &mut e) == 0 {
                    break;
                }
            }
        }
        windows_sys::Win32::Foundation::CloseHandle(snap);
    }
    n
}

#[cfg(not(windows))]
fn threads() -> usize {
    0
}

/// what was measured over the last interval
#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub private_mb: f64,
    pub working_mb: f64,
    /// share of the whole processor, as Task Manager shows it (%)
    pub cpu: f64,
    pub threads: usize,
    /// frames drawn per second
    pub fps: f64,
    /// share of one processor core spent making sound (%)
    pub sound: f64,
}

/// Measures while Настройки → Диагностика is open: the counters are read every couple of
/// seconds and compared with the last reading.
pub struct Probe {
    at: std::time::Instant,
    cpu: f64,
    frames: u64,
    render_ns: u64,
    pub last: Option<Sample>,
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

impl Probe {
    pub fn new() -> Self {
        Self { at: std::time::Instant::now(), cpu: cpu_seconds(), frames: crate::ui::window::FRAMES.load(std::sync::atomic::Ordering::Relaxed), render_ns: crate::audio::output::RENDER_NS.load(std::sync::atomic::Ordering::Relaxed), last: None }
    }

    /// a new reading when `every` has passed since the last
    pub fn tick(&mut self, every: std::time::Duration) {
        let wall = self.at.elapsed();
        if self.last.is_some() && wall < every {
            return;
        }
        let secs = wall.as_secs_f64().max(0.001);
        let cpu = cpu_seconds();
        let frames = crate::ui::window::FRAMES.load(std::sync::atomic::Ordering::Relaxed);
        let render_ns = crate::audio::output::RENDER_NS.load(std::sync::atomic::Ordering::Relaxed);
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
        let m = memory();
        self.last = Some(Sample {
            private_mb: m.private_mb,
            working_mb: m.working_mb,
            cpu: ((cpu - self.cpu) / secs / cores * 100.0).max(0.0),
            threads: threads(),
            fps: (frames - self.frames) as f64 / secs,
            sound: (render_ns - self.render_ns) as f64 / 1e9 / secs * 100.0,
        });
        self.at = std::time::Instant::now();
        self.cpu = cpu;
        self.frames = frames;
        self.render_ns = render_ns;
    }
}

/// A piece of advice (src/diagnostics/advice.ts): what costs what, and the setting that helps.
pub struct Advice {
    pub id: &'static str,
    /// 0 important, 1 worth it, 2 for information
    pub level: u8,
    pub title: &'static str,
    pub detail: String,
}

pub struct Knobs {
    pub memory_saver: bool,
    pub overlay_enabled: bool,
    pub sound_processing: bool,
}

/// "12 МБ", "640 КБ"
pub fn mb(bytes: f64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if bytes < MB {
        format!("{} КБ", (bytes / 1024.0).round().max(1.0))
    } else {
        format!("{} МБ", (bytes / MB).round())
    }
}

pub fn advise(s: &Sample, k: &Knobs, stored: &[(String, usize)]) -> Vec<Advice> {
    let mut out = Vec::new();
    if !k.memory_saver && s.private_mb > 250.0 {
        out.push(Advice {
            id: "memory",
            level: if s.private_mb > 400.0 { 0 } else { 1 },
            title: "Много памяти",
            detail: format!("Приложение занимает {:.0} МБ. Экономия памяти держит меньше картинок и раньше отдаёт видеокарту и страницы, когда окно скрыто.", s.private_mb),
        });
    }
    if s.cpu > 8.0 && s.fps > 30.0 {
        out.push(Advice { id: "busy", level: 1, title: "Окно часто перерисовывается", detail: format!("{:.0} кадров в секунду и {:.0}% процессора. Окно рисуется только когда что-то меняется: обычно это анимация или наведённая мышь.", s.fps, s.cpu) });
    }
    if k.overlay_enabled {
        out.push(Advice { id: "overlay", level: 2, title: "Мини-плеер", detail: "Пока мини-плеер спрятан, он ничего не стоит; открытый держит около мегабайта и рисуется процессором. Если им не пользуетесь, его можно выключить.".into() });
    }
    let total: usize = stored.iter().map(|x| x.1).sum();
    if total > 3 * 1024 * 1024 {
        let top: Vec<String> = stored.iter().take(3).map(|(k, b)| format!("{k} ({})", mb(*b as f64))).collect();
        out.push(Advice { id: "storage", level: 2, title: "Сохранённые данные", detail: format!("Хранится {}; больше всего — {}. Лишнее можно не хранить: Настройки → Данные и копия.", mb(total as f64), top.join(", ")) });
    }
    if k.sound_processing && s.sound > 15.0 {
        out.push(Advice { id: "sound", level: 2, title: "Обработка звука", detail: "Эквалайзер, выравнивание громкости, реверберация, смена тона и кроссфейд считаются на процессоре. Если что-то из этого не нужно, выключите его в Настройках → Звук.".into() });
    }
    out.sort_by_key(|a| a.level);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advice_comes_most_important_first() {
        let s = Sample { private_mb: 500.0, cpu: 1.0, fps: 0.0, sound: 20.0, ..Default::default() };
        let a = advise(&s, &Knobs { memory_saver: false, overlay_enabled: true, sound_processing: true }, &[("evplayer.stats.v1".into(), 4 * 1024 * 1024)]);
        let ids: Vec<&str> = a.iter().map(|a| a.id).collect();
        assert_eq!(ids[0], "memory");
        assert!(ids.contains(&"sound") && ids.contains(&"storage") && ids.contains(&"overlay"));
        let calm = advise(&Sample { private_mb: 40.0, ..Default::default() }, &Knobs { memory_saver: true, overlay_enabled: false, sound_processing: false }, &[]);
        assert!(calm.is_empty());
        assert_eq!(mb(5.0 * 1024.0 * 1024.0), "5 МБ");
    }
}
