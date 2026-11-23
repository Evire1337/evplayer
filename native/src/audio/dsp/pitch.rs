//! The pitch shifter of public/worklets/pitch-shifter.js: two read heads move through a short
//! ring buffer at `ratio` times the write speed; each head restarts when it drifts a window
//! away, and the two are cross-faded with a raised-cosine envelope so the restarts are not
//! heard. ratio > 1 raises the pitch, < 1 lowers it; the tempo is untouched.

/// ~43 ms at 48 kHz: short enough to stay tight, long enough for bass
const WINDOW: f32 = 2048.0;
const BUFFER: usize = 8192;

pub struct Shifter {
    buf: Vec<[f32; 2]>,
    write: usize,
    phase: f32,
    /// it was shifting a moment ago: run until the heads catch up (no click when it stops)
    tail: usize,
}

impl Shifter {
    pub fn new() -> Self {
        Self { buf: Vec::new(), write: 0, phase: 0.0, tail: 0 }
    }

    pub fn busy(&self) -> bool {
        self.tail > 0
    }

    #[inline(always)]
    fn read(&self, pos: f32, c: usize) -> f32 {
        let i = pos.floor();
        let frac = pos - i;
        let i = i as isize;
        let n = BUFFER as isize;
        let a = self.buf[(i.rem_euclid(n)) as usize][c];
        let b = self.buf[((i + 1).rem_euclid(n)) as usize][c];
        a + (b - a) * frac
    }

    pub fn process(&mut self, buf: &mut [f32], ratio: f32) {
        if self.buf.is_empty() {
            // made once, the first time the pitch is changed
            self.buf = vec![[0.0; 2]; BUFFER];
        }
        let ratio = ratio.clamp(0.25, 4.0);
        let shifting = (ratio - 1.0).abs() > 1e-4;
        self.tail = if shifting { BUFFER } else { self.tail.saturating_sub(buf.len() / 2) };
        let step = (1.0 - ratio) / WINDOW;
        for f in buf.chunks_exact_mut(2) {
            let w = self.write % BUFFER;
            self.buf[w] = [f[0], f[1]];
            if shifting {
                let phase_b = (self.phase + 0.5) % 1.0;
                let pos_a = self.write as f32 - self.phase * WINDOW - 1.0;
                let pos_b = self.write as f32 - phase_b * WINDOW - 1.0;
                let ga = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * self.phase).cos();
                let gb = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * phase_b).cos();
                for c in 0..2 {
                    f[c] = self.read(pos_a, c) * ga + self.read(pos_b, c) * gb;
                }
                self.phase += step;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                if self.phase < 0.0 {
                    self.phase += 1.0;
                }
            }
            self.write = (self.write + 1) % (BUFFER * 1024);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// zero crossings per second of the left channel
    fn freq(buf: &[f32], rate: f32) -> f32 {
        let l: Vec<f32> = buf.iter().step_by(2).copied().collect();
        let n = l.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        n as f32 / (l.len() as f32 / rate)
    }

    #[test]
    fn an_octave_up_doubles_the_frequency() {
        let rate = 48_000.0;
        let mut s = Shifter::new();
        let mut out = Vec::new();
        for block in 0..100 {
            let mut b: Vec<f32> = (0..480).flat_map(|i| {
                let t = (block * 480 + i) as f32 / rate;
                let v = (2.0 * std::f32::consts::PI * 300.0 * t).sin() * 0.5;
                [v, v]
            }).collect();
            s.process(&mut b, 2.0);
            if block > 20 {
                out.extend(b);
            }
        }
        let f = freq(&out, rate);
        assert!((f - 600.0).abs() < 30.0, "{f}");
    }
}
