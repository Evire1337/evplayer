//! What happens to the sound after the decks meet, in the order of src/audio/graph.ts:
//!   pitch shifter -> dry + reverb -> levelling -> preamp -> equalizer bands -> limiter
//! (the volume comes after, in output.rs). Each stage is skipped while it changes nothing,
//! so with everything off the chain costs a few comparisons per block.
//!
//! Settings come from the main side as a whole `Params` through a lock-free ring; changes
//! glide (the equalizer's levels over a few hundred milliseconds, like the page's
//! setTargetAtTime), so dragging a point or switching a preset never clicks.

pub mod eq;
pub mod loudness;
mod pitch;
mod reverb;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use eq::{coefficients, BandType, Coefficients, MAX_BANDS};

use super::output::Shared;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandParams {
    pub kind: BandType,
    pub frequency: f32,
    /// dB; 0 for a band that is off
    pub gain: f32,
    pub q: f32,
}

impl Default for BandParams {
    fn default() -> Self {
        Self { kind: BandType::Peaking, frequency: 1000.0, gain: 0.0, q: 1.0 }
    }
}

/// everything the chain needs to know, sent whole on every change
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub bands: [BandParams; MAX_BANDS],
    /// the level before the bands, dB (the preamp and the clip-protection headroom)
    pub preamp_db: f32,
    pub limiter: bool,
    /// volume levelling on: the level gain comes from `Shared::level_db`
    pub normalize: bool,
    /// the pitch shifter's ratio (1: off)
    pub pitch: f32,
    /// 0: no reverb
    pub reverb: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self { bands: [BandParams::default(); MAX_BANDS], preamp_db: 0.0, limiter: false, normalize: false, pitch: 1.0, reverb: 0.0 }
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    c: Coefficients,
    /// direct form II transposed state, per channel
    z: [[f64; 2]; 2],
    /// the band as it is now (gliding toward the target)
    now: BandParams,
}

impl Biquad {
    #[inline(always)]
    fn run(&mut self, ch: usize, x: f64) -> f64 {
        let c = &self.c;
        let z = &mut self.z[ch];
        let y = c.b0 * x + z[0];
        z[0] = c.b1 * x - c.a1 * y + z[1];
        z[1] = c.b2 * x - c.a2 * y;
        y
    }
}

/// a peak limiter: -1 dBFS, fast attack, 120 ms release (the page's DynamicsCompressor)
struct Limiter {
    env: f32,
    attack: f32,
    release: f32,
}

impl Limiter {
    fn new(rate: u32) -> Self {
        let k = |ms: f32| (-1.0 / (ms * 0.001 * rate as f32)).exp();
        Self { env: 1.0, attack: k(3.0), release: k(120.0) }
    }

    #[inline(always)]
    fn run(&mut self, l: f32, r: f32) -> f32 {
        const THRESHOLD: f32 = 0.891; // -1 dBFS
        let peak = l.abs().max(r.abs());
        let want = if peak > THRESHOLD { THRESHOLD / peak } else { 1.0 };
        let k = if want < self.env { self.attack } else { self.release };
        self.env = want + (self.env - want) * k;
        // a peak over the line is caught at once; the gain comes back slowly
        self.env.min(want)
    }
}

pub struct Chain {
    rate: u32,
    params: Params,
    incoming: Option<rtrb::Consumer<Params>>,
    shared: Option<Arc<Shared>>,
    bands: [Biquad; MAX_BANDS],
    /// the preamp as it is now, linear
    preamp: f32,
    level: f32,
    limiter: Limiter,
    pitch: pitch::Shifter,
    reverb: reverb::Reverb,
    /// the reverb's level as it is now (it fades out before it is switched off)
    wet: f32,
}

impl Chain {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            params: Params::default(),
            incoming: None,
            shared: None,
            bands: [Biquad::default(); MAX_BANDS],
            preamp: 1.0,
            level: 1.0,
            limiter: Limiter::new(rate),
            pitch: pitch::Shifter::new(),
            reverb: reverb::Reverb::new(rate),
            wet: 0.0,
        }
    }

    /// where the settings come from, and where the measured level goes
    pub fn connect(&mut self, incoming: rtrb::Consumer<Params>, shared: Arc<Shared>) {
        self.incoming = Some(incoming);
        self.shared = Some(shared);
    }

    fn take_params(&mut self) {
        if let Some(rx) = &mut self.incoming {
            while let Ok(p) = rx.pop() {
                self.params = p;
            }
        }
    }

    /// the bands glide toward their settings: about a quarter of the way per block
    fn glide_bands(&mut self) {
        let rate = self.rate as f64;
        for (b, want) in self.bands.iter_mut().zip(self.params.bands.iter()) {
            if b.now == *want {
                continue;
            }
            let near = |a: f32, b: f32, eps: f32| (a - b).abs() <= eps;
            if b.now.kind != want.kind {
                b.now.kind = want.kind;
            }
            let step = |a: f32, t: f32| a + (t - a) * 0.35;
            b.now.gain = if near(b.now.gain, want.gain, 0.01) { want.gain } else { step(b.now.gain, want.gain) };
            b.now.frequency = if near(b.now.frequency, want.frequency, 0.5) { want.frequency } else { step(b.now.frequency, want.frequency) };
            b.now.q = if near(b.now.q, want.q, 0.001) { want.q } else { step(b.now.q, want.q) };
            b.c = coefficients(b.now.kind, b.now.frequency as f64, b.now.gain as f64, b.now.q as f64, rate);
        }
    }

    /// `buf`: interleaved stereo at the sound card's rate
    pub fn process(&mut self, buf: &mut [f32], rate: u32) {
        if rate != self.rate {
            self.rate = rate;
            self.limiter = Limiter::new(rate);
            self.reverb = reverb::Reverb::new(rate);
            for b in &mut self.bands {
                b.now = BandParams::default();
                b.now.gain = f32::NAN; // recomputed on the next glide
            }
        }
        self.take_params();
        let p = self.params;
        let frames = buf.len() / 2;

        // the level the player measures for volume levelling (before anything is changed)
        if let Some(sh) = &self.shared {
            if p.normalize && frames > 0 {
                let mut sum = 0.0f64;
                for s in buf.iter() {
                    sum += (*s as f64) * (*s as f64);
                }
                let ms = (sum / buf.len() as f64) as f32;
                sh.level_power.store(ms.to_bits(), Ordering::Relaxed);
            }
        }

        // pitch
        if (p.pitch - 1.0).abs() > 1e-4 || self.pitch.busy() {
            self.pitch.process(buf, p.pitch);
        }

        // reverb: the tail fades out before the reverb is let go
        let wet_target = p.reverb.clamp(0.0, 1.0);
        if wet_target > 0.0 || self.wet > 0.0001 {
            self.reverb.process(buf, &mut self.wet, wet_target, frames);
        }

        // levelling, preamp, bands, limiter
        let level_db = match &self.shared {
            Some(sh) if p.normalize => f32::from_bits(sh.level_db.load(Ordering::Relaxed)),
            _ => 0.0,
        };
        let level_target = 10f32.powf(level_db / 20.0);
        let preamp_target = 10f32.powf(p.preamp_db / 20.0);
        self.glide_bands();
        let any_band = self.bands.iter().any(|b| b.now.gain != 0.0 && !b.now.gain.is_nan());
        let level_k = 1.0 - (-1.0 / (1.2 * self.rate as f32)).exp(); // tau 1.2 s
        let pre_k = 1.0 - (-1.0 / (0.02 * self.rate as f32)).exp();
        let steady = (self.level - level_target).abs() < 1e-4 && (self.preamp - preamp_target).abs() < 1e-4;
        if !any_band && !p.limiter && steady && (level_target - 1.0).abs() < 1e-4 && (preamp_target - 1.0).abs() < 1e-4 {
            self.level = level_target;
            self.preamp = preamp_target;
            return;
        }
        for i in 0..frames {
            self.level += (level_target - self.level) * level_k;
            self.preamp += (preamp_target - self.preamp) * pre_k;
            let g = self.level * self.preamp;
            let mut l = (buf[2 * i] * g) as f64;
            let mut r = (buf[2 * i + 1] * g) as f64;
            if any_band {
                for b in self.bands.iter_mut() {
                    if b.now.gain != 0.0 && !b.now.gain.is_nan() {
                        l = b.run(0, l);
                        r = b.run(1, r);
                    }
                }
            }
            let (mut l, mut r) = (l as f32, r as f32);
            if p.limiter {
                let k = self.limiter.run(l, r);
                l *= k;
                r *= k;
            }
            buf[2 * i] = l;
            buf[2 * i + 1] = r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, rate: u32, frames: usize, amp: f32) -> Vec<f32> {
        (0..frames).flat_map(|i| {
            let s = (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin() * amp;
            [s, s]
        }).collect()
    }

    fn rms(buf: &[f32]) -> f32 {
        (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
    }

    #[test]
    fn with_everything_off_the_sound_is_untouched() {
        let mut c = Chain::new(48_000);
        let mut buf = tone(440.0, 48_000, 4800, 0.5);
        let orig = buf.clone();
        c.process(&mut buf, 48_000);
        assert_eq!(buf, orig);
    }

    #[test]
    fn a_band_boosts_its_frequency_by_its_gain() {
        let mut c = Chain::new(48_000);
        let (mut tx, rx) = rtrb::RingBuffer::new(4);
        c.connect(rx, Shared::new());
        let mut p = Params::default();
        p.bands[0] = BandParams { kind: BandType::Peaking, frequency: 1000.0, gain: 6.0, q: 1.0 };
        tx.push(p).unwrap();
        // let the glide settle, then measure
        for _ in 0..40 {
            let mut b = tone(1000.0, 48_000, 480, 0.1);
            c.process(&mut b, 48_000);
        }
        let mut b = tone(1000.0, 48_000, 4800, 0.1);
        let before = rms(&b);
        c.process(&mut b, 48_000);
        let db = 20.0 * (rms(&b[2000..]) / before).log10();
        assert!((db - 6.0).abs() < 0.3, "{db}");
    }

    #[test]
    fn the_limiter_keeps_peaks_under_minus_one_db() {
        let mut c = Chain::new(48_000);
        let (mut tx, rx) = rtrb::RingBuffer::new(4);
        c.connect(rx, Shared::new());
        tx.push(Params { limiter: true, preamp_db: 12.0, ..Params::default() }).unwrap();
        let mut peak = 0f32;
        for _ in 0..50 {
            let mut b = tone(200.0, 48_000, 480, 0.8);
            c.process(&mut b, 48_000);
            peak = b.iter().fold(0.0, |m: f32, s| m.max(s.abs()));
        }
        assert!(peak < 0.95, "{peak}");
    }
}
