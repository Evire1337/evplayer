//! Splitting a song into vocals, drums, bass and the rest ("Дорожки"), on this computer, with
//! Spleeter (Deezer's open model) run by ONNX Runtime. Computers only: a phone has neither the
//! memory nor the time for it, and the phone build gets stand-ins at the bottom.
//!
//! The switch in Settings decides everything: while it is off, nothing here is loaded (no
//! model, no session, no thread); switching it on loads the four small networks, switching it
//! off drops them again. The page sends a song's sound once (16-bit stereo at 44.1 kHz), the
//! four tracks are written next to the app's data (a folder per song, named by a key the page
//! makes from the song), and the page plays them from there like any file.
//!
//! The method is the one the model was trained for: periodic Hann window, frame 4096, hop 1024;
//! the networks see the loudness of the lower 1024 of 2049 frequency bins and say how much of
//! each belongs to each part; the four answers are turned into masks that add up to exactly one
//! and are applied to the original spectrum, so the four tracks always add up to the song.
//! Everything is done a piece of about twelve seconds at a time and written out as it goes, so a
//! long song costs time, not memory.

#[cfg(desktop)]
mod real {
    use std::fs::{self, File};
    use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use ort::inputs;
    use ort::session::builder::GraphOptimizationLevel;
    use ort::session::Session;
    use ort::value::TensorRef;
    use realfft::num_complex::Complex32;
    use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};

    pub const SAMPLE_RATE: u32 = 44_100;
    const N_FFT: usize = 4096;
    const HOP: usize = 1024;
    /// frames the networks take at once (about twelve seconds)
    const T: usize = 512;
    /// frequency bins the networks see; the spectrum has BINS
    const F: usize = 1024;
    const BINS: usize = N_FFT / 2 + 1;
    /// zeros in front, so that the very start of the song is covered by enough frames
    const PAD: usize = N_FFT - HOP;
    /// the parts, in the order of the networks
    pub const PARTS: [&str; 4] = ["vocals", "drums", "bass", "other"];
    /// how many values of loudness per part the page gets to draw its wave
    pub const PEAKS: usize = 900;
    /// a mask's tiny share so that silence does not divide by zero
    const EPS: f32 = 1e-10;
    /// how many of the network's top bins give the share carried up to the bins it does not see
    const TAIL_BINS: usize = 128;

    const MODEL_BASE: &str = "https://huggingface.co/Best-Practice/spleeter-4stems-onnx/resolve/main";
    /// the four models, with what a good copy of each looks like (sha-256)
    const MODELS: [(&str, &str); 4] = [
        ("vocals", "db47148ab1c52709ce694893f532c91abfe3edc4d46238939570e036a22878ca"),
        ("drums", "7ae4002e5633634674f74dc3356d5875b0da894d59ce0f60e844bb8f9cb8aa92"),
        ("bass", "ba4c4949a27222492cca49859901a873b4b71461dc48c7c5a51f93d31eb11f55"),
        ("other", "3cc59116cb7195946ab9596d8ca25984d09c0f8a70db8cf85d063132f97bc61d"),
    ];

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StemsMeta {
        pub key: String,
        pub seconds: f64,
        /// per part (PARTS order): PEAKS loudness values, 0 to 255, over the whole song
        pub peaks: Vec<Vec<u8>>,
        pub bytes: u64,
    }

    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StemsState {
        pub supported: bool,
        pub model_ready: bool,
        pub loaded: bool,
        pub cache_bytes: u64,
    }

    pub struct Stems {
        dir: PathBuf,
        sessions: Mutex<Option<Vec<Session>>>,
        cancel: AtomicBool,
        working: AtomicBool,
        /// the song being split now (its folder is served while it is written, and is not cleared)
        active: Mutex<String>,
    }

    fn e(err: impl std::fmt::Display) -> String {
        err.to_string()
    }

    /// a key the page made: letters and digits only, so it can name a folder
    pub fn valid_key(key: &str) -> bool {
        !key.is_empty() && key.len() <= 64 && key.bytes().all(|b| b.is_ascii_alphanumeric())
    }

    impl Stems {
        pub fn new(data_dir: &Path) -> Arc<Self> {
            Arc::new(Self { dir: data_dir.join("stems"), sessions: Mutex::new(None), cancel: AtomicBool::new(false), working: AtomicBool::new(false), active: Mutex::new(String::new()) })
        }

        fn model_dir(&self) -> PathBuf {
            self.dir.join("models")
        }

        fn cache_dir(&self) -> PathBuf {
            self.dir.join("cache")
        }

        fn model_path(&self, part: &str) -> PathBuf {
            self.model_dir().join(format!("{part}.fp16.onnx"))
        }

        pub fn models_ready(&self) -> bool {
            MODELS.iter().all(|(part, _)| fs::metadata(self.model_path(part)).map(|m| m.len() > 1_000_000).unwrap_or(false))
        }

        pub fn is_loaded(&self) -> bool {
            self.sessions.lock().map(|s| s.is_some()).unwrap_or(false)
        }

        pub fn state(&self) -> StemsState {
            StemsState { supported: true, model_ready: self.models_ready(), loaded: self.is_loaded(), cache_bytes: self.cache_bytes() }
        }

        /// downloads the four networks (about 80 MB in all) once; `report(done, total)` counts bytes
        pub fn download_models(&self, report: &dyn Fn(u64, u64)) -> Result<(), String> {
            fs::create_dir_all(self.model_dir()).map_err(e)?;
            // how much is still to fetch (the sizes are known only once asked, so count what is there)
            let total_guess: u64 = 4 * 19_714_141;
            let mut done_before: u64 = 0;
            for (part, hash) in MODELS {
                let target = self.model_path(part);
                if file_hash(&target).map(|h| h == hash).unwrap_or(false) {
                    done_before += fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                    report(done_before, total_guess);
                    continue;
                }
                let url = format!("{MODEL_BASE}/{part}.fp16.onnx");
                let mut res = crate::media_server::blocking_client().get(&url).send().map_err(e)?;
                if !res.status().is_success() {
                    return Err(format!("модель не скачалась ({})", res.status()));
                }
                let tmp = target.with_extension("part");
                let mut out = File::create(&tmp).map_err(e)?;
                let mut hasher = Sha256::new();
                let mut buf = vec![0u8; 256 * 1024];
                let mut got: u64 = 0;
                loop {
                    let n = res.read(&mut buf).map_err(e)?;
                    if n == 0 {
                        break;
                    }
                    out.write_all(&buf[..n]).map_err(e)?;
                    hasher.update(&buf[..n]);
                    got += n as u64;
                    report(done_before + got, total_guess);
                }
                out.flush().map_err(e)?;
                drop(out);
                let hex = hex(&hasher.finalize());
                if hex != hash {
                    let _ = fs::remove_file(&tmp);
                    return Err("скачанная модель не совпала с ожидаемой".into());
                }
                fs::rename(&tmp, &target).map_err(e)?;
                done_before += got;
            }
            Ok(())
        }

        /// opens the four networks (a few seconds); they stay in memory until `unload`
        pub fn load(&self) -> Result<(), String> {
            if self.is_loaded() {
                return Ok(());
            }
            if !self.models_ready() {
                return Err("модель ещё не скачана".into());
            }
            // a share of the processor, not all of it: the music has to go on playing
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
            let threads = (threads * 3 / 4).clamp(2, 8);
            let _low = priority::lower();
            let mut sessions = Vec::with_capacity(PARTS.len());
            for part in PARTS {
                let session = Session::builder()
                    .map_err(e)?
                    .with_optimization_level(GraphOptimizationLevel::Level3)
                    .map_err(e)?
                    .with_intra_threads(threads)
                    .map_err(e)?
                    .commit_from_file(self.model_path(part))
                    .map_err(e)?;
                sessions.push(session);
            }
            *self.sessions.lock().map_err(e)? = Some(sessions);
            Ok(())
        }

        /// drops the networks and stops what is running: the memory goes back
        pub fn unload(&self) {
            self.cancel.store(true, Ordering::SeqCst);
            // waits for a running piece to notice (it checks between pieces), then lets go
            if let Ok(mut s) = self.sessions.lock() {
                *s = None;
            }
        }

        pub fn cancel(&self) {
            self.cancel.store(true, Ordering::SeqCst);
        }

        pub fn meta(&self, key: &str) -> Option<StemsMeta> {
            if !valid_key(key) {
                return None;
            }
            let text = fs::read_to_string(self.cache_dir().join(key).join("meta.json")).ok()?;
            serde_json::from_str(&text).ok()
        }

        /// where a part of a song is, for the media server; only what this module wrote
        pub fn path_of(&self, key: &str, part: &str) -> Option<PathBuf> {
            if !valid_key(key) || !PARTS.contains(&part) {
                return None;
            }
            let path = self.cache_dir().join(key).join(format!("{part}.wav"));
            path.is_file().then_some(path)
        }

        pub fn cache_bytes(&self) -> u64 {
            dir_bytes(&self.cache_dir())
        }

        /// removes the finished songs (what is being written right now is left alone)
        pub fn clear_cache(&self) {
            let Ok(rd) = fs::read_dir(self.cache_dir()) else { return };
            let active = self.active.lock().map(|a| a.clone()).unwrap_or_default();
            for d in rd.flatten() {
                let name = d.file_name().to_string_lossy().to_string();
                if d.path().is_dir() && !name.ends_with(".tmp") && name != active {
                    let _ = fs::remove_dir_all(d.path());
                }
            }
        }

        /// the oldest songs go until the folder fits `limit` bytes (never the one just made)
        pub fn trim_cache(&self, limit: u64, keep: &str) {
            let Ok(rd) = fs::read_dir(self.cache_dir()) else { return };
            let mut songs: Vec<(std::time::SystemTime, PathBuf, u64)> = rd
                .flatten()
                .filter(|d| d.path().is_dir() && d.file_name().to_string_lossy() != keep)
                .map(|d| {
                    let at = d.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                    (at, d.path(), dir_bytes(&d.path()))
                })
                .collect();
            let mut total = self.cache_bytes();
            songs.sort_by_key(|s| s.0);
            for (_, path, size) in songs {
                if total <= limit {
                    break;
                }
                if fs::remove_dir_all(&path).is_ok() {
                    total = total.saturating_sub(size);
                }
            }
        }

        /// splits one song: `pcm` is 16-bit little-endian stereo, 44.1 kHz; `report` gets 0 to 1
        pub fn separate(&self, key: &str, pcm: &[u8], report: &dyn Fn(f32)) -> Result<StemsMeta, String> {
            if !valid_key(key) {
                return Err("неверный ключ песни".into());
            }
            if pcm.len() < 4 * HOP {
                return Err("песня слишком короткая".into());
            }
            if self.working.swap(true, Ordering::SeqCst) {
                return Err("уже идёт другое разделение".into());
            }
            self.cancel.store(false, Ordering::SeqCst);
            if let Ok(mut a) = self.active.lock() {
                *a = key.to_string();
            }
            let result = self.separate_inner(key, pcm, report);
            if let Ok(mut a) = self.active.lock() {
                a.clear();
            }
            // a song that was not finished leaves nothing behind (only a finished one has its meta.json)
            if result.is_err() {
                let _ = fs::remove_dir_all(self.cache_dir().join(key));
            }
            self.working.store(false, Ordering::SeqCst);
            result
        }

        fn separate_inner(&self, key: &str, pcm: &[u8], report: &dyn Fn(f32)) -> Result<StemsMeta, String> {
            let mut guard = self.sessions.lock().map_err(e)?;
            let sessions = guard.as_mut().ok_or("модель не загружена")?;
            // the music keeps its share of the processor: the splitter works at a lower priority
            let _low = priority::lower();

            let n = pcm.len() / 4;
            let frames = (PAD + n).div_ceil(HOP);
            let splits = frames.div_ceil(T);
            let window = hann();
            let mut planner = RealFftPlanner::<f32>::new();
            let fwd = planner.plan_fft_forward(N_FFT);
            let inv = planner.plan_fft_inverse(N_FFT);

            // the parts are written where they are served from, so the player can start on the first
            // seconds while the rest is still being split; the song is whole once meta.json is there
            let work = self.cache_dir().join(key);
            let _ = fs::remove_dir_all(&work);
            fs::create_dir_all(&work).map_err(e)?;
            let mut writers: Vec<WavWriter> = Vec::new();
            for part in PARTS {
                writers.push(WavWriter::create(&work.join(format!("{part}.wav")))?);
            }
            let mut peaks = vec![vec![0u8; PEAKS]; PARTS.len()];

            // overlap-add of the four parts (two channels each) and of the window's weight
            let mut ola: Vec<[Ola; 2]> = (0..PARTS.len()).map(|_| [Ola::new(), Ola::new()]).collect();
            let mut weight = Ola::new();
            let mut spec = vec![Complex32::new(0.0, 0.0); 2 * T * BINS];
            let mut mags = vec![0f32; 2 * T * F];
            let mut est: Vec<Vec<f32>> = vec![Vec::new(); PARTS.len()];
            let mut time = vec![0f32; N_FFT];
            let mut frame_spec = vec![Complex32::new(0.0, 0.0); BINS];
            let mut frame_out = vec![0f32; N_FFT];
            let mut written = 0usize;
            let mut masks = vec![vec![0f32; F]; PARTS.len()];

            for s in 0..splits {
                if self.cancel.load(Ordering::SeqCst) {
                    return Err("отменено".into());
                }
                let f0 = s * T;
                let f1 = ((s + 1) * T).min(frames);
                // the spectrum of these frames, and what the networks look at
                spec.fill(Complex32::new(0.0, 0.0));
                mags.fill(0.0);
                for c in 0..2 {
                    for t in 0..(f1 - f0) {
                        let at = (f0 + t) * HOP;
                        for i in 0..N_FFT {
                            time[i] = sample(pcm, n, c, at + i) * window[i];
                        }
                        let out = &mut spec[(c * T + t) * BINS..(c * T + t + 1) * BINS];
                        fwd.process(&mut time, out).map_err(e)?;
                        for f in 0..F {
                            mags[(c * T + t) * F + f] = out[f].norm();
                        }
                    }
                }
                // each network says how much of every bin is its own
                for (p, session) in sessions.iter_mut().enumerate() {
                    let input = TensorRef::from_array_view(([2usize, 1, T, F], mags.as_slice())).map_err(e)?;
                    let outputs = session.run(inputs!["x" => input]).map_err(e)?;
                    let (_, data) = outputs["y"].try_extract_tensor::<f32>().map_err(e)?;
                    if data.len() != 2 * T * F {
                        return Err("модель ответила неожиданным размером".into());
                    }
                    est[p].clear();
                    est[p].extend_from_slice(data);
                }
                // the masks, the parts, and what is finished goes to the files
                for t in 0..(f1 - f0) {
                    let at = (f0 + t) * HOP;
                    // the window's weight, once for everything
                    for i in 0..N_FFT {
                        frame_out[i] = window[i] * window[i];
                    }
                    weight.add(at, &frame_out);
                    for c in 0..2 {
                        let base = (c * T + t) * F;
                        // the masks of the four parts for every bin: Spleeter's own (the share of
                        // each network's squared estimate). They add up to one, so the parts add
                        // up to the song. Sharper masks were tried, and they leave holes in the
                        // sound when a part is taken out (the bubbling "shakal"), so they are not
                        for f in 0..F {
                            let denom: f32 = (0..PARTS.len()).map(|p| est[p][base + f] * est[p][base + f]).sum::<f32>() + EPS;
                            for p in 0..PARTS.len() {
                                masks[p][f] = (est[p][base + f] * est[p][base + f] + EPS / PARTS.len() as f32) / denom;
                            }
                        }
                        for p in 0..PARTS.len() {
                            let src = &spec[(c * T + t) * BINS..(c * T + t + 1) * BINS];
                            for f in 0..F {
                                frame_spec[f] = src[f] * masks[p][f];
                            }
                            // above what the network sees, the share of its top bins is carried up
                            // (the hiss of a voice and the cymbals live there)
                            let tail = masks[p][F - TAIL_BINS..].iter().sum::<f32>() / TAIL_BINS as f32;
                            for f in F..BINS {
                                frame_spec[f] = src[f] * tail;
                            }
                            // a real signal has nothing imaginary at the two ends
                            frame_spec[0].im = 0.0;
                            frame_spec[BINS - 1].im = 0.0;
                            inv.process(&mut frame_spec, &mut frame_out).map_err(e)?;
                            for i in 0..N_FFT {
                                frame_out[i] = frame_out[i] / N_FFT as f32 * window[i];
                            }
                            ola[p][c].add(at, &frame_out);
                        }
                    }
                    // everything before the next frame's start is complete
                    let upto = (f0 + t + 1) * HOP;
                    flush(&mut ola, &mut weight, upto, n, &mut written, &mut writers, &mut peaks)?;
                }
                // what is finished is on disk before it is said to be: the player reads it from there
                for w in writers.iter_mut() {
                    w.sync()?;
                }
                report((s + 1) as f32 / splits as f32);
            }
            flush(&mut ola, &mut weight, usize::MAX, n, &mut written, &mut writers, &mut peaks)?;

            let mut bytes = 0u64;
            for w in writers {
                bytes += w.finish()?;
            }
            let meta = StemsMeta { key: key.to_string(), seconds: n as f64 / SAMPLE_RATE as f64, peaks, bytes };
            fs::write(work.join("meta.json"), serde_json::to_string(&meta).map_err(e)?).map_err(e)?;
            Ok(meta)
        }
    }

    // ---- the pieces ------------------------------------------------------------------

    /// the program works at a lower priority while the splitter runs (the sound of the page is
    /// made in other processes and keeps its priority), and gets its own back afterwards
    mod priority {
        #[cfg(windows)]
        pub struct Lowered(u32);

        #[cfg(windows)]
        pub fn lower() -> Lowered {
            use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetPriorityClass, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
            // SAFETY: the pseudo-handle of this process is always valid
            unsafe {
                let me = GetCurrentProcess();
                let before = GetPriorityClass(me);
                SetPriorityClass(me, BELOW_NORMAL_PRIORITY_CLASS);
                Lowered(before)
            }
        }

        #[cfg(windows)]
        impl Drop for Lowered {
            fn drop(&mut self) {
                use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass};
                if self.0 != 0 {
                    // SAFETY: as above
                    unsafe {
                        SetPriorityClass(GetCurrentProcess(), self.0);
                    }
                }
            }
        }

        #[cfg(not(windows))]
        pub struct Lowered;

        #[cfg(not(windows))]
        pub fn lower() -> Lowered {
            Lowered
        }
    }

    /// the periodic Hann window the networks were trained with
    pub fn hann() -> Vec<f32> {
        (0..N_FFT).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N_FFT as f32).cos()).collect()
    }

    /// one sample of the song at a place in the padded timeline (zeros before and after it)
    fn sample(pcm: &[u8], n: usize, channel: usize, at: usize) -> f32 {
        if at < PAD || at - PAD >= n {
            return 0.0;
        }
        let i = ((at - PAD) * 2 + channel) * 2;
        i16::from_le_bytes([pcm[i], pcm[i + 1]]) as f32 / 32768.0
    }

    /// overlap-add of frames that arrive in order; what is behind the next frame is handed out
    struct Ola {
        buf: Vec<f32>,
        base: usize,
    }

    impl Ola {
        fn new() -> Self {
            Self { buf: Vec::new(), base: 0 }
        }

        fn add(&mut self, at: usize, frame: &[f32]) {
            let from = at - self.base;
            if self.buf.len() < from + frame.len() {
                self.buf.resize(from + frame.len(), 0.0);
            }
            for (i, v) in frame.iter().enumerate() {
                self.buf[from + i] += v;
            }
        }

        /// takes what is complete, up to (not including) `upto`
        fn take(&mut self, upto: usize) -> (usize, Vec<f32>) {
            let count = upto.saturating_sub(self.base).min(self.buf.len());
            let start = self.base;
            let out: Vec<f32> = self.buf.drain(..count).collect();
            self.base += count;
            (start, out)
        }
    }

    fn flush(ola: &mut [[Ola; 2]], weight: &mut Ola, upto: usize, n: usize, written: &mut usize, writers: &mut [WavWriter], peaks: &mut [Vec<u8>]) -> Result<(), String> {
        let (start, w) = weight.take(upto);
        let mut taken: Vec<[Vec<f32>; 2]> = Vec::with_capacity(ola.len());
        for part in ola.iter_mut() {
            taken.push([part[0].take(upto).1, part[1].take(upto).1]);
        }
        // the samples of the song itself: from PAD on, up to its length
        let mut chunk = vec![0u8; 0];
        for (p, part) in taken.iter().enumerate() {
            chunk.clear();
            let mut count = 0usize;
            for i in 0..w.len() {
                let coord = start + i;
                if coord < PAD || coord - PAD >= n {
                    continue;
                }
                let at = coord - PAD;
                for c in 0..2 {
                    let v = if w[i] > 1e-6 { part[c][i] / w[i] } else { 0.0 };
                    let q = (v * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
                    chunk.extend_from_slice(&q.to_le_bytes());
                    let bucket = (at * PEAKS / n.max(1)).min(PEAKS - 1);
                    let level = (v.abs().min(1.0) * 255.0) as u8;
                    if level > peaks[p][bucket] {
                        peaks[p][bucket] = level;
                    }
                }
                count += 1;
            }
            writers[p].write(&chunk, count)?;
            if p == 0 {
                *written += count;
            }
        }
        Ok(())
    }

    /// a 16-bit stereo WAV that is written as it goes; the sizes are filled in at the end
    struct WavWriter {
        file: BufWriter<File>,
        frames: u64,
    }

    impl WavWriter {
        fn create(path: &Path) -> Result<Self, String> {
            let mut file = BufWriter::new(File::create(path).map_err(e)?);
            file.write_all(&wav_header(0)).map_err(e)?;
            Ok(Self { file, frames: 0 })
        }

        fn write(&mut self, bytes: &[u8], frames: usize) -> Result<(), String> {
            self.file.write_all(bytes).map_err(e)?;
            self.frames += frames as u64;
            Ok(())
        }

        /// what has been written so far can be read by others
        fn sync(&mut self) -> Result<(), String> {
            self.file.flush().map_err(e)
        }

        /// closes the file; returns its size
        fn finish(mut self) -> Result<u64, String> {
            self.file.flush().map_err(e)?;
            let data = self.frames * 4;
            let file = self.file.get_mut();
            file.seek(SeekFrom::Start(0)).map_err(e)?;
            file.write_all(&wav_header(data as u32)).map_err(e)?;
            file.flush().map_err(e)?;
            Ok(44 + data)
        }
    }

    /// the header of a 16-bit stereo 44.1 kHz WAV holding `data` bytes of sound
    pub fn wav_header(data: u32) -> [u8; 44] {
        let mut h = [0u8; 44];
        h[0..4].copy_from_slice(b"RIFF");
        h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
        h[8..16].copy_from_slice(b"WAVEfmt ");
        h[16..20].copy_from_slice(&16u32.to_le_bytes());
        h[20..22].copy_from_slice(&1u16.to_le_bytes());
        h[22..24].copy_from_slice(&2u16.to_le_bytes());
        h[24..28].copy_from_slice(&SAMPLE_RATE.to_le_bytes());
        h[28..32].copy_from_slice(&(SAMPLE_RATE * 4).to_le_bytes());
        h[32..34].copy_from_slice(&4u16.to_le_bytes());
        h[34..36].copy_from_slice(&16u16.to_le_bytes());
        h[36..40].copy_from_slice(b"data");
        h[40..44].copy_from_slice(&data.to_le_bytes());
        h
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn file_hash(path: &Path) -> Option<String> {
        let mut file = File::open(path).ok()?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = file.read(&mut buf).ok()?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Some(hex(&hasher.finalize()))
    }

    fn dir_bytes(path: &Path) -> u64 {
        fs::read_dir(path)
            .map(|rd| {
                rd.flatten()
                    .map(|d| match d.file_type() {
                        Ok(t) if t.is_dir() => dir_bytes(&d.path()),
                        _ => d.metadata().map(|m| m.len()).unwrap_or(0),
                    })
                    .sum()
            })
            .unwrap_or(0)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// frames of a signal, put back together the way `separate` does it (without the networks)
        fn round_trip(signal: &[f32]) -> Vec<f32> {
            let n = signal.len();
            let pcm: Vec<u8> = signal.iter().flat_map(|v| [(v * 32767.0).round() as i16; 2]).flat_map(|q| q.to_le_bytes()).collect();
            let frames = (PAD + n).div_ceil(HOP);
            let window = hann();
            let mut planner = RealFftPlanner::<f32>::new();
            let fwd = planner.plan_fft_forward(N_FFT);
            let inv = planner.plan_fft_inverse(N_FFT);
            let mut ola = Ola::new();
            let mut weight = Ola::new();
            let mut time = vec![0f32; N_FFT];
            let mut spec = vec![Complex32::new(0.0, 0.0); BINS];
            let mut out = vec![0f32; N_FFT];
            let mut got = Vec::new();
            for k in 0..frames {
                let at = k * HOP;
                for i in 0..N_FFT {
                    time[i] = sample(&pcm, n, 0, at + i) * window[i];
                }
                fwd.process(&mut time, &mut spec).unwrap();
                spec[0].im = 0.0;
                spec[BINS - 1].im = 0.0;
                inv.process(&mut spec, &mut out).unwrap();
                let w: Vec<f32> = window.iter().map(|x| x * x).collect();
                weight.add(at, &w);
                for i in 0..N_FFT {
                    out[i] = out[i] / N_FFT as f32 * window[i];
                }
                ola.add(at, &out);
                let (start, done) = ola.take((k + 1) * HOP);
                let (_, ws) = weight.take((k + 1) * HOP);
                for (i, v) in done.iter().enumerate() {
                    let coord = start + i;
                    if coord >= PAD && coord - PAD < n {
                        got.push(if ws[i] > 1e-6 { v / ws[i] } else { 0.0 });
                    }
                }
            }
            let (start, done) = ola.take(usize::MAX);
            let (_, ws) = weight.take(usize::MAX);
            for (i, v) in done.iter().enumerate() {
                let coord = start + i;
                if coord >= PAD && coord - PAD < n {
                    got.push(if ws[i] > 1e-6 { v / ws[i] } else { 0.0 });
                }
            }
            got
        }

        #[test]
        fn the_window_is_periodic_hann() {
            let w = hann();
            assert_eq!(w.len(), N_FFT);
            assert!(w[0].abs() < 1e-7);
            assert!((w[N_FFT / 2] - 1.0).abs() < 1e-6);
            // periodic: the far end is not zero again, the next sample would be
            assert!(w[N_FFT - 1] > 0.0);
        }

        #[test]
        fn frames_put_back_together_give_the_song() {
            let signal: Vec<f32> = (0..20_000).map(|i| 0.4 * (i as f32 * 0.05).sin() + 0.2 * (i as f32 * 0.31).sin()).collect();
            let back = round_trip(&signal);
            assert_eq!(back.len(), signal.len());
            let worst = signal.iter().zip(&back).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
            // the song went through 16-bit and 32-bit float: a few ten-thousandths at most
            assert!(worst < 2e-3, "worst difference {worst}");
        }

        #[test]
        fn the_wav_header_says_how_much_sound_follows() {
            let h = wav_header(1000);
            assert_eq!(&h[0..4], b"RIFF");
            assert_eq!(u32::from_le_bytes(h[4..8].try_into().unwrap()), 1036);
            assert_eq!(u32::from_le_bytes(h[24..28].try_into().unwrap()), 44_100);
            assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 1000);
        }

        #[test]
        fn keys_name_folders_safely() {
            assert!(valid_key("a1B2c3"));
            assert!(!valid_key(""));
            assert!(!valid_key("../x"));
            assert!(!valid_key("a/b"));
        }

        #[test]
        fn samples_outside_the_song_are_silence() {
            let pcm: Vec<u8> = [1000i16, -1000, 2000, -2000].iter().flat_map(|q| q.to_le_bytes()).collect();
            assert_eq!(sample(&pcm, 2, 0, 0), 0.0);
            assert_eq!(sample(&pcm, 2, 0, PAD + 2), 0.0);
            assert!((sample(&pcm, 2, 0, PAD) - 1000.0 / 32768.0).abs() < 1e-9);
            assert!((sample(&pcm, 2, 1, PAD + 1) + 2000.0 / 32768.0).abs() < 1e-9);
        }

        /// the real networks on a made-up song (skipped where they have not been downloaded): the
        /// parts can be read while they are written, the song is whole when meta.json is there, and
        /// the four parts add up to the song
        #[test]
        fn a_song_is_split_and_its_first_seconds_can_be_read_meanwhile() {
            use std::cell::RefCell;
            let models = std::env::var_os("EVPLAYER_STEMS_MODELS")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("EvPlayer").join("stems").join("models")));
            let Some(models) = models.filter(|m| m.join("vocals.fp16.onnx").is_file()) else {
                eprintln!("no networks here: skipped");
                return;
            };
            let root = std::env::temp_dir().join(format!("evplayer-stems-test-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("stems").join("models")).unwrap();
            let stems = Stems::new(&root);
            for part in PARTS {
                fs::copy(models.join(format!("{part}.fp16.onnx")), stems.model_path(part)).unwrap();
            }
            stems.load().expect("the networks open");

            // 30 s: a low note, a higher one that comes and goes, and a click twice a second
            let n = 30 * SAMPLE_RATE as usize;
            let mut song = vec![[0i16; 2]; n];
            for (i, frame) in song.iter_mut().enumerate() {
                let t = i as f32 / SAMPLE_RATE as f32;
                let note = 0.25 * (std::f32::consts::TAU * 110.0 * t).sin();
                let high = if t % 4.0 < 2.0 { 0.15 * (std::f32::consts::TAU * 440.0 * t).sin() } else { 0.0 };
                let click = if i % 22_050 < 200 { 0.3 } else { 0.0 };
                let q = |v: f32| (v * 32767.0).round() as i16;
                *frame = [q(note + high + click), q(note * 0.8 + high * 1.2 - click)];
            }
            let pcm: Vec<u8> = song.iter().flat_map(|f| f.iter().flat_map(|q| q.to_le_bytes())).collect();

            let seen = RefCell::new(Vec::<(f32, u64, bool)>::new());
            let meta = stems
                .separate("testsong", &pcm, &|share| {
                    // what is said to be done is on disk, and the song is not "whole" yet
                    let len = fs::metadata(stems.path_of("testsong", "vocals").expect("served while written")).unwrap().len();
                    seen.borrow_mut().push((share, len, stems.meta("testsong").is_some()));
                })
                .expect("the song is split");

            let seen = seen.into_inner();
            assert!(seen.len() >= 2, "progress was reported {} times", seen.len());
            assert!(seen.iter().all(|s| !s.2), "meta.json must come last");
            assert!(seen.windows(2).all(|w| w[1].1 >= w[0].1), "the files only grow");
            assert!(seen[0].1 > 44 + 4 * SAMPLE_RATE as u64 * 5, "after the first piece several seconds are readable: {}", seen[0].1);
            assert!(stems.meta("testsong").is_some());
            assert!((meta.seconds - 30.0).abs() < 0.01);
            assert_eq!(meta.peaks.len(), PARTS.len());

            // every part is a whole WAV of the song's length, and together they are the song
            let mut sum = vec![[0f64; 2]; n];
            for part in PARTS {
                let bytes = fs::read(stems.path_of("testsong", part).unwrap()).unwrap();
                assert_eq!(bytes.len(), 44 + n * 4, "{part}: length");
                assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize, n * 4, "{part}: header");
                for (i, s) in sum.iter_mut().enumerate() {
                    for c in 0..2 {
                        let at = 44 + (i * 2 + c) * 2;
                        s[c] += i16::from_le_bytes([bytes[at], bytes[at + 1]]) as f64;
                    }
                }
            }
            let (mut err, mut power) = (0f64, 0f64);
            for (s, f) in sum.iter().zip(&song) {
                for c in 0..2 {
                    err += (s[c] - f[c] as f64).powi(2);
                    power += (f[c] as f64).powi(2);
                }
            }
            let ratio = (err / power).sqrt();
            assert!(ratio < 0.05, "the parts add up to the song within {ratio}");

            // a cleared cache keeps the finished song's folder only until asked, and never the one being written
            stems.clear_cache();
            assert!(stems.meta("testsong").is_none());
            stems.unload();
            let _ = fs::remove_dir_all(&root);
        }
    }
}

#[cfg(desktop)]
pub use real::*;

/// phones: the same names, and "not supported"
#[cfg(mobile)]
mod stand_in {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StemsMeta {
        pub key: String,
        pub seconds: f64,
        pub peaks: Vec<Vec<u8>>,
        pub bytes: u64,
    }

    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StemsState {
        pub supported: bool,
        pub model_ready: bool,
        pub loaded: bool,
        pub cache_bytes: u64,
    }

    pub struct Stems;

    pub fn valid_key(_key: &str) -> bool {
        false
    }

    const NO: &str = "разделение на дорожки есть только на компьютере";

    impl Stems {
        pub fn new(_data_dir: &Path) -> Arc<Self> {
            Arc::new(Self)
        }
        pub fn state(&self) -> StemsState {
            StemsState { supported: false, model_ready: false, loaded: false, cache_bytes: 0 }
        }
        pub fn download_models(&self, _report: &dyn Fn(u64, u64)) -> Result<(), String> {
            Err(NO.into())
        }
        pub fn load(&self) -> Result<(), String> {
            Err(NO.into())
        }
        pub fn unload(&self) {}
        pub fn cancel(&self) {}
        pub fn meta(&self, _key: &str) -> Option<StemsMeta> {
            None
        }
        pub fn path_of(&self, _key: &str, _part: &str) -> Option<PathBuf> {
            None
        }
        pub fn clear_cache(&self) {}
        pub fn trim_cache(&self, _limit: u64, _keep: &str) {}
        pub fn separate(&self, _key: &str, _pcm: &[u8], _report: &dyn Fn(f32)) -> Result<StemsMeta, String> {
            Err(NO.into())
        }
    }
}

#[cfg(mobile)]
pub use stand_in::*;
