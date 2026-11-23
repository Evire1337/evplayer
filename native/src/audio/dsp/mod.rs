//! What happens to the sound after the decks meet (the same order as src/audio/graph.ts):
//! the mixer of voice, bass and width; the pitch shifter; the reverb; the equalizer with
//! volume levelling and the limiter. Each stage costs nothing while it is off.

pub struct Chain {
    rate: u32,
}

impl Chain {
    pub fn new(rate: u32) -> Self {
        Self { rate }
    }

    /// `buf`: interleaved stereo at the sound card's rate
    pub fn process(&mut self, buf: &mut [f32], rate: u32) {
        if rate != self.rate {
            self.rate = rate;
        }
        let _ = buf;
    }
}
