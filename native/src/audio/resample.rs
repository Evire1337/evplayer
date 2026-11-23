//! Changing a song's sample rate to the sound card's, and the speed of "Скорость" when it is
//! linked to the pitch (a faster record sounds higher): both are reading the song at another
//! rate. A windowed-sinc interpolator with a variable step does it; when the rates match and
//! the speed is 1 the samples pass through untouched, which costs next to nothing.

/// input frames each output frame is made from
const TAPS: usize = 16;
/// steps between two input frames in the kernel table
const PHASES: usize = 256;

pub struct Resampler {
    /// the last TAPS input frames, twice over so a window is always one slice
    hist: [[f32; 2 * TAPS]; 2],
    head: usize,
    /// how far past the window's centre the next output frame lies, in input frames
    frac: f64,
    /// input frames owed before the next output frame
    owed: f64,
    table: Vec<f32>,
    cutoff: f32,
}

fn kernel_table(cutoff: f32) -> Vec<f32> {
    let half = TAPS as f32 / 2.0;
    let mut t = vec![0f32; (PHASES + 1) * TAPS];
    for p in 0..=PHASES {
        let frac = p as f32 / PHASES as f32;
        let row = &mut t[p * TAPS..(p + 1) * TAPS];
        let mut sum = 0.0;
        for (k, w) in row.iter_mut().enumerate() {
            // distance of input frame k from the output point (which is `frac` past frame half-1)
            let x = k as f32 - (half - 1.0) - frac;
            let sinc = if x.abs() < 1e-6 { 1.0 } else { (std::f32::consts::PI * x * cutoff).sin() / (std::f32::consts::PI * x * cutoff) };
            // Blackman window over the taps
            let n = (x + half) / TAPS as f32;
            let win = if (0.0..=1.0).contains(&n) { 0.42 - 0.5 * (2.0 * std::f32::consts::PI * n).cos() + 0.08 * (4.0 * std::f32::consts::PI * n).cos() } else { 0.0 };
            *w = sinc * win;
            sum += *w;
        }
        // the same level whatever the phase: no buzz from the kernel itself
        if sum.abs() > 1e-6 {
            for w in row.iter_mut() {
                *w /= sum;
            }
        }
    }
    t
}

impl Default for Resampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Resampler {
    pub fn new() -> Self {
        Self { hist: [[0.0; 2 * TAPS]; 2], head: 0, frac: 0.0, owed: TAPS as f64 / 2.0 + 1.0, table: kernel_table(1.0), cutoff: 1.0 }
    }

    /// start over (a new song, a jump): the window is emptied
    pub fn reset(&mut self) {
        self.hist = [[0.0; 2 * TAPS]; 2];
        self.head = 0;
        self.frac = 0.0;
        self.owed = TAPS as f64 / 2.0 + 1.0;
    }

    fn push(&mut self, f: [f32; 2]) {
        for c in 0..2 {
            self.hist[c][self.head] = f[c];
            self.hist[c][self.head + TAPS] = f[c];
        }
        self.head = (self.head + 1) % TAPS;
    }

    /// Fill `out` (interleaved stereo) reading input `ratio` frames per output frame from
    /// `pull`. Returns the frames written; fewer than asked when `pull` ran dry (the state
    /// stays as it was, so the next call goes on seamlessly).
    pub fn process(&mut self, ratio: f64, out: &mut [f32], mut pull: impl FnMut() -> Option<[f32; 2]>) -> usize {
        let frames = out.len() / 2;
        // reading faster than the source rate: the kernel is narrowed so nothing above the
        // new Nyquist folds back as noise
        let cutoff = if ratio > 1.0 { (0.97 / ratio).max(0.1) as f32 } else { 1.0 };
        if (cutoff - self.cutoff).abs() > 0.01 {
            self.cutoff = cutoff;
            self.table = kernel_table(cutoff);
        }
        let passthrough = (ratio - 1.0).abs() < 1e-9 && self.frac == 0.0;
        for i in 0..frames {
            while self.owed >= 1.0 {
                match pull() {
                    Some(f) => {
                        self.push(f);
                        self.owed -= 1.0;
                    }
                    None => return i,
                }
            }
            let (l, r) = if passthrough {
                // the frame at the window's centre, as it is
                let at = (self.head + TAPS / 2 - 1) % TAPS;
                (self.hist[0][at], self.hist[1][at])
            } else {
                let p = self.frac * PHASES as f64;
                let pi = (p as usize).min(PHASES - 1);
                let t = (p - pi as f64) as f32;
                let a = &self.table[pi * TAPS..(pi + 1) * TAPS];
                let b = &self.table[(pi + 1) * TAPS..(pi + 2) * TAPS];
                let hl = &self.hist[0][self.head..self.head + TAPS];
                let hr = &self.hist[1][self.head..self.head + TAPS];
                let (mut l, mut r) = (0.0f32, 0.0f32);
                for k in 0..TAPS {
                    let w = a[k] + (b[k] - a[k]) * t;
                    l += hl[k] * w;
                    r += hr[k] * w;
                }
                (l, r)
            };
            out[2 * i] = l;
            out[2 * i + 1] = r;
            let next = self.frac + ratio;
            let whole = next.floor();
            self.frac = next - whole;
            self.owed += whole;
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ratio: f64, input: &[f32], out_frames: usize) -> Vec<f32> {
        let mut r = Resampler::new();
        let mut i = 0;
        let mut out = vec![0f32; out_frames * 2];
        let n = r.process(ratio, &mut out, || {
            let s = *input.get(i)?;
            i += 1;
            Some([s, s])
        });
        out.truncate(n * 2);
        out
    }

    #[test]
    fn passes_samples_through_at_the_same_rate() {
        let input: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin()).collect();
        let out = run(1.0, &input, 80);
        // the same samples, in step (the window looks ahead instead of lagging)
        for k in 0..60 {
            let want = input[k];
            assert!((out[2 * k] - want).abs() < 1e-6, "{k}: {} vs {want}", out[2 * k]);
        }
    }

    #[test]
    fn keeps_a_tone_at_another_rate() {
        // 1 kHz at 44.1 kHz read out at 48 kHz stays a 1 kHz tone of the same level
        let input: Vec<f32> = (0..44_100).map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44_100.0).sin() * 0.5).collect();
        let out = run(44_100.0 / 48_000.0, &input, 40_000);
        let tail: Vec<f32> = out.iter().step_by(2).skip(1000).copied().collect();
        let peak = tail.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.02, "peak {peak}");
        // zero crossings: 1000 per second, 2 per period
        let crossings = tail.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as f32;
        let seconds = tail.len() as f32 / 48_000.0;
        assert!((crossings / seconds - 1000.0).abs() < 5.0, "{}", crossings / seconds);
    }

    #[test]
    fn stops_when_the_input_runs_dry_and_goes_on_later() {
        let mut r = Resampler::new();
        let mut out = vec![0f32; 200];
        let mut given = 0;
        let n = r.process(1.5, &mut out, || {
            if given < 30 {
                given += 1;
                Some([1.0, 1.0])
            } else {
                None
            }
        });
        assert!(n > 0 && n < 100);
        let m = r.process(1.5, &mut out, || Some([1.0, 1.0]));
        assert_eq!(m, 100);
    }
}
