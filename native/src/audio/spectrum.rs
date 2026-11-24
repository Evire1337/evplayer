//! The live spectrum behind the equalizer's graph (EqualizerGraph.tsx draws it from a Web
//! Audio AnalyserNode: fftSize 4096, smoothing 0.82, −100…−30 dB). The sound card's thread
//! copies the finished sound into a small ring only while the graph is on screen (`want`); the
//! interface turns it into the same bytes `getByteFrequencyData` gives. Nothing runs, and
//! nothing is copied, while the graph is not shown.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// the analyser's window: 4096 samples, 2048 bins
pub const SIZE: usize = 4096;
const SMOOTHING: f32 = 0.82;
const MIN_DB: f32 = -100.0;
const MAX_DB: f32 = -30.0;

struct Tap {
    ring: Vec<AtomicU32>,
    pos: AtomicUsize,
    /// ms since `epoch` until which the graph wants the sound
    wanted_until: AtomicU64,
    epoch: Instant,
    /// the sound card's rate (for where each bin's frequency is)
    rate: AtomicU32,
}

fn tap() -> &'static Tap {
    static TAP: OnceLock<Tap> = OnceLock::new();
    TAP.get_or_init(|| Tap { ring: (0..SIZE).map(|_| AtomicU32::new(0)).collect(), pos: AtomicUsize::new(0), wanted_until: AtomicU64::new(0), epoch: Instant::now(), rate: AtomicU32::new(48_000) })
}

/// the graph is on screen: keep copying the sound for the next half second
pub fn want() {
    let t = tap();
    t.wanted_until.store(t.epoch.elapsed().as_millis() as u64 + 500, Ordering::Relaxed);
}

/// the sound card's thread: the finished stereo block, copied as mono only while wanted
pub fn feed(stereo: &[f32], rate: u32) {
    let t = tap();
    if t.wanted_until.load(Ordering::Relaxed) <= t.epoch.elapsed().as_millis() as u64 {
        return;
    }
    t.rate.store(rate, Ordering::Relaxed);
    let mut pos = t.pos.load(Ordering::Relaxed);
    for f in stereo.chunks_exact(2) {
        t.ring[pos % SIZE].store(((f[0] + f[1]) * 0.5).to_bits(), Ordering::Relaxed);
        pos = pos.wrapping_add(1);
    }
    t.pos.store(pos, Ordering::Relaxed);
}

/// the rate of the sound the spectrum is made from
pub fn rate() -> u32 {
    tap().rate.load(Ordering::Relaxed).max(1)
}

/// the last `SIZE` samples, oldest first
fn samples() -> Vec<f32> {
    let t = tap();
    let end = t.pos.load(Ordering::Relaxed);
    (0..SIZE).map(|i| f32::from_bits(t.ring[end.wrapping_add(i) % SIZE].load(Ordering::Relaxed))).collect()
}

/// the smoothed magnitudes kept between looks, as the analyser keeps them
static SMOOTHED: Mutex<Vec<f32>> = Mutex::new(Vec::new());

/// getByteFrequencyData: 2048 levels, 0..255
pub fn bytes() -> Vec<u8> {
    let mags = magnitudes(&samples());
    let mut s = SMOOTHED.lock().unwrap();
    if s.len() != mags.len() {
        *s = vec![0.0; mags.len()];
    }
    s.iter_mut()
        .zip(mags)
        .map(|(prev, m)| {
            *prev = SMOOTHING * *prev + (1.0 - SMOOTHING) * m;
            let db = 20.0 * prev.max(1e-12).log10();
            (((db - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0) * 255.0) as u8
        })
        .collect()
}

/// |X[k]| / N of the Blackman-windowed block, for the first N/2 bins
pub fn magnitudes(block: &[f32]) -> Vec<f32> {
    let n = block.len();
    let mut re: Vec<f32> = block.iter().enumerate().map(|(i, x)| x * blackman(i, n)).collect();
    let mut im = vec![0.0f32; n];
    fft(&mut re, &mut im);
    (0..n / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt() / n as f32).collect()
}

/// the window Web Audio's analyser uses (alpha 0.16)
fn blackman(i: usize, n: usize) -> f32 {
    let a = 0.16f32;
    let x = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
    (1.0 - a) / 2.0 - 0.5 * x.cos() + a / 2.0 * (2.0 * x).cos()
}

/// an in-place radix-2 FFT (the length a power of two)
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos() as f32, ang.sin() as f32);
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let next = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = next;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_shows_at_its_own_frequency() {
        // 1 kHz at 48 kHz: bin 1000 / (48000 / 4096) ≈ 85
        let block: Vec<f32> = (0..SIZE).map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        let m = magnitudes(&block);
        let peak = m.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        assert!((84..=86).contains(&peak), "peak at {peak}");
        // far from the tone there is next to nothing
        assert!(m[600] < m[peak] * 1e-3);
    }

    #[test]
    fn nothing_is_copied_unless_the_graph_wants_it() {
        let t = tap();
        let before = t.pos.load(Ordering::Relaxed);
        feed(&[0.5; 64], 48_000);
        assert_eq!(t.pos.load(Ordering::Relaxed), before);
        want();
        feed(&[0.5; 64], 48_000);
        assert_eq!(t.pos.load(Ordering::Relaxed), before + 32);
    }
}
