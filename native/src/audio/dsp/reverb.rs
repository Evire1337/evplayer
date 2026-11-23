//! Reverberation: a hall of the Freeverb kind (eight combs and four all-passes per side) in
//! place of the page's 2.6-second convolution — the same kind of sound for a small fraction of
//! the processor time. As in src/audio/graph.ts, the low end is kept out of the hall (a
//! high-pass at 160 Hz before it), the wet part is 0.9 × amount and the dry part 1 − 0.35 ×
//! amount; switched off, the tail fades out before the reverb stops.

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;
/// a long hall (the page's impulse lasted 2.6 s)
const FEEDBACK: f32 = 0.86;
const DAMP: f32 = 0.35;
const INPUT_GAIN: f32 = 0.015;

struct Comb {
    buf: Vec<f32>,
    i: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], i: 0, store: 0.0 }
    }
    #[inline(always)]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.buf[self.i];
        self.store = y * (1.0 - DAMP) + self.store * DAMP;
        self.buf[self.i] = x + self.store * FEEDBACK;
        self.i = (self.i + 1) % self.buf.len();
        y
    }
}

struct AllPass {
    buf: Vec<f32>,
    i: usize,
}

impl AllPass {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], i: 0 }
    }
    #[inline(always)]
    fn run(&mut self, x: f32) -> f32 {
        let b = self.buf[self.i];
        self.buf[self.i] = x + b * 0.5;
        self.i = (self.i + 1) % self.buf.len();
        b - x
    }
}

pub struct Reverb {
    combs: [Vec<Comb>; 2],
    alls: [Vec<AllPass>; 2],
    /// the 160 Hz high-pass before the hall (one pole), per side
    hp: [(f32, f32); 2],
    hp_k: f32,
    rate: u32,
    made: bool,
}

impl Reverb {
    pub fn new(rate: u32) -> Self {
        let hp_k = (-2.0 * std::f32::consts::PI * 160.0 / rate as f32).exp();
        Self { combs: [Vec::new(), Vec::new()], alls: [Vec::new(), Vec::new()], hp: [(0.0, 0.0); 2], hp_k, rate, made: false }
    }

    /// the delay lines are made the first time the reverb is used (about 100 KB)
    fn make(&mut self) {
        let scale = self.rate as f32 / 44_100.0;
        for side in 0..2 {
            let extra = side * SPREAD;
            self.combs[side] = COMBS.iter().map(|l| Comb::new(((l + extra) as f32 * scale) as usize)).collect();
            self.alls[side] = ALLPASSES.iter().map(|l| AllPass::new(((l + extra) as f32 * scale) as usize)).collect();
        }
        self.made = true;
    }

    /// mixes the hall into `buf`; `wet` glides toward `target` (0..1)
    pub fn process(&mut self, buf: &mut [f32], wet: &mut f32, target: f32, frames: usize) {
        if !self.made {
            self.make();
        }
        let k = 1.0 - (-1.0 / (0.05 * self.rate as f32)).exp();
        for f in 0..frames {
            *wet += (target - *wet) * k;
            let amount = *wet;
            for side in 0..2 {
                let x = buf[2 * f + side];
                // high-pass: the input minus its low part
                let (ref mut lp, _) = self.hp[side];
                *lp = x + (*lp - x) * self.hp_k;
                let hi = x - *lp;
                let input = hi * INPUT_GAIN;
                let mut out = 0.0;
                for c in self.combs[side].iter_mut() {
                    out += c.run(input);
                }
                for a in self.alls[side].iter_mut() {
                    out = a.run(out);
                }
                buf[2 * f + side] = x * (1.0 - amount * 0.35) + out * amount * 0.9;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_leaves_a_fading_tail() {
        let mut r = Reverb::new(48_000);
        let mut wet = 1.0;
        let mut buf = vec![0f32; 48_000 * 2];
        buf[0] = 1.0;
        buf[1] = 1.0;
        r.process(&mut buf, &mut wet, 1.0, 48_000);
        let energy = |a: usize, b: usize| buf[a * 2..b * 2].iter().map(|s| s * s).sum::<f32>();
        let early = energy(2_000, 12_000);
        let late = energy(36_000, 46_000);
        assert!(early > 0.0 && late > 0.0, "a tail");
        assert!(late < early, "it fades: {early} → {late}");
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() < 2.0));
    }
}
