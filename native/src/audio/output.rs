//! The sound card's thread: two decks mixed, the effects, the volume, out to WASAPI.
//!
//! Nothing here waits, locks or allocates while sound plays: the decks' samples come through
//! lock-free rings (decode.rs), settings through atomics, and commands through one more ring.
//! A song let go is handed back to the main side to be freed there.
//!
//! The decks are what the browser version had as two <audio> elements: one plays, the other
//! gets the next song ready. When the playing one ends and the next is armed, the switch is
//! made here, between two samples, so songs follow each other without a gap; a crossfade is
//! two gain ramps on this same clock.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use super::decode::{Block, Track};
use super::dsp::Chain;
use super::resample::Resampler;

/// a loaded song as the sound card's thread holds it
pub struct DeckIn {
    pub ring: rtrb::Consumer<Block>,
    pub track: Arc<Track>,
}

pub enum Cmd {
    Load(usize, DeckIn),
    Unload(usize),
    /// the deck is heard (or stops being heard) at this gain
    Play(usize, bool),
    /// ramp the deck's gain to `to` over `secs`; a deck faded to 0 stops
    Fade(usize, f32, f32),
    SetGain(usize, f32),
    /// when the other deck's song ends, this one starts at once
    Arm(usize, bool),
}

/// what the main side and the sound card's thread share
pub struct Shared {
    /// 0..1, as f32 bits
    pub volume: AtomicU32,
    /// the speed of "Скорость", as f32 bits (1 = as recorded)
    pub speed: AtomicU32,
    /// everything is paused (the decks keep their place)
    pub paused: AtomicBool,
    /// how many times the armed deck took over by itself
    pub switched: AtomicU64,
    /// the deck that took over last
    pub switched_to: AtomicU32,
    pub device_rate: AtomicU32,
    /// the sound card stopped (unplugged): the stream must be made again
    pub broken: AtomicBool,
    /// peak level of the last moment, as f32 bits (the "Дорожки" meters, a test without sound)
    pub peak: AtomicU32,
    /// how far the sound card plays behind what was just made, in microseconds
    pub latency_us: AtomicU32,
}

impl Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            volume: AtomicU32::new(0.8f32.to_bits()),
            speed: AtomicU32::new(1.0f32.to_bits()),
            paused: AtomicBool::new(true),
            switched: AtomicU64::new(0),
            switched_to: AtomicU32::new(0),
            device_rate: AtomicU32::new(48_000),
            broken: AtomicBool::new(false),
            peak: AtomicU32::new(0),
            latency_us: AtomicU32::new(0),
        })
    }

    pub fn f(a: &AtomicU32) -> f32 {
        f32::from_bits(a.load(Ordering::Relaxed))
    }
}

/// where a deck's frames come from
struct Feed {
    input: Option<DeckIn>,
    cur: Option<Block>,
    offset: usize,
    /// the end block was taken: what is left is the resampler's tail
    draining: bool,
}

struct Deck {
    feed: Feed,
    resampler: Resampler,
    playing: bool,
    gain: f32,
    target: f32,
    step: f32,
    armed: bool,
}

impl Deck {
    fn new() -> Self {
        Self { feed: Feed { input: None, cur: None, offset: 0, draining: false }, resampler: Resampler::new(), playing: false, gain: 1.0, target: 1.0, step: 0.0, armed: false }
    }
}

pub struct Mixer {
    decks: [Deck; 2],
    cmds: rtrb::Consumer<Cmd>,
    trash: rtrb::Producer<DeckIn>,
    shared: Arc<Shared>,
    pub chain: Chain,
    scratch: Vec<f32>,
    mix: Vec<f32>,
    volume: f32,
    /// 0 when paused, 1 when playing, ramped so a pause never clicks
    run: f32,
    rate: u32,
}

/// the next stereo frame of a deck, from its blocks (None: nothing there yet, or the end)
fn next_frame(deck: &mut Feed) -> Option<[f32; 2]> {
    let input = deck.input.as_mut()?;
    let want = input.track.epoch.load(Ordering::Relaxed);
    loop {
        if let Some(b) = &deck.cur {
            if b.epoch == want && deck.offset < b.len as usize {
                let i = deck.offset * 2;
                deck.offset += 1;
                input.track.pos.store(b.pos + deck.offset as u64, Ordering::Relaxed);
                return Some([b.data[i], b.data[i + 1]]);
            }
            if b.epoch == want && b.end {
                deck.draining = true;
                deck.cur = None;
                return None;
            }
        }
        match input.ring.pop() {
            Ok(b) => {
                if b.epoch != want {
                    // sound from before a jump
                    deck.cur = None;
                    continue;
                }
                if deck.draining {
                    deck.draining = false;
                }
                deck.offset = 0;
                deck.cur = Some(b);
            }
            Err(_) => {
                deck.cur = None;
                return None;
            }
        }
    }
}

impl Mixer {
    pub fn new(cmds: rtrb::Consumer<Cmd>, trash: rtrb::Producer<DeckIn>, shared: Arc<Shared>, rate: u32) -> Self {
        Self {
            decks: [Deck::new(), Deck::new()],
            cmds,
            trash,
            chain: Chain::new(rate),
            scratch: vec![0.0; 8192],
            mix: vec![0.0; 8192],
            volume: Shared::f(&shared.volume),
            run: 0.0,
            shared,
            rate,
        }
    }

    /// the sound card changed: its rate
    pub fn set_rate(&mut self, rate: u32) {
        self.rate = rate;
        for d in &mut self.decks {
            d.resampler.reset();
        }
    }

    fn drop_input(&mut self, d: usize) {
        let deck = &mut self.decks[d];
        if let Some(old) = deck.feed.input.take() {
            // freed on the main side, never here
            let _ = self.trash.push(old);
        }
        deck.feed.cur = None;
        deck.feed.offset = 0;
        deck.feed.draining = false;
        deck.playing = false;
        deck.armed = false;
        deck.resampler.reset();
    }

    fn commands(&mut self) {
        while let Ok(cmd) = self.cmds.pop() {
            match cmd {
                Cmd::Load(d, input) => {
                    self.drop_input(d);
                    self.decks[d].feed.input = Some(input);
                }
                Cmd::Unload(d) => self.drop_input(d),
                Cmd::Play(d, on) => self.decks[d].playing = on,
                Cmd::Fade(d, to, secs) => {
                    let deck = &mut self.decks[d];
                    deck.target = to;
                    let frames = (secs.max(0.001) * self.rate as f32).max(1.0);
                    deck.step = (to - deck.gain) / frames;
                }
                Cmd::SetGain(d, g) => {
                    let deck = &mut self.decks[d];
                    deck.gain = g;
                    deck.target = g;
                    deck.step = 0.0;
                }
                Cmd::Arm(d, on) => self.decks[d].armed = on,
            }
        }
    }

    /// fill `out` (interleaved, `channels` per frame) with the next sound
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        self.commands();
        let frames = out.len() / channels.max(1);
        if self.mix.len() < frames * 2 {
            // only when the sound card suddenly asks for more than ever before
            self.mix.resize(frames * 2, 0.0);
            self.scratch.resize(frames * 2, 0.0);
        }
        let mix = &mut self.mix[..frames * 2];
        mix.fill(0.0);
        let paused = self.shared.paused.load(Ordering::Relaxed);
        let speed = Shared::f(&self.shared.speed).clamp(0.25, 4.0) as f64;
        let device = self.rate as f64;
        // a fully paused player: silence, and the decks keep their place
        if !(paused && self.run <= 0.0) {
            for d in 0..2 {
                if !self.decks[d].playing || self.decks[d].feed.input.is_none() {
                    continue;
                }
                let rate = self.decks[d].feed.input.as_ref().map(|i| i.track.rate.load(Ordering::Relaxed)).unwrap_or(44_100) as f64;
                let ratio = rate * speed / device;
                let scratch = &mut self.scratch[..frames * 2];
                let deck = &mut self.decks[d];
                let Deck { resampler, feed, .. } = &mut *deck;
                let made = resampler.process(ratio, scratch, || next_frame(feed));
                for i in 0..made {
                    if deck.step != 0.0 {
                        deck.gain += deck.step;
                        if (deck.step > 0.0 && deck.gain >= deck.target) || (deck.step < 0.0 && deck.gain <= deck.target) {
                            deck.gain = deck.target;
                            deck.step = 0.0;
                        }
                    }
                    mix[2 * i] += scratch[2 * i] * deck.gain;
                    mix[2 * i + 1] += scratch[2 * i + 1] * deck.gain;
                }
                if made < frames {
                    let input = deck.feed.input.as_ref().unwrap();
                    if deck.feed.draining {
                        // the song is over
                        input.track.ended.store(true, Ordering::Relaxed);
                        input.track.starved.store(false, Ordering::Relaxed);
                        deck.playing = false;
                        deck.feed.draining = false;
                        let other = 1 - d;
                        if self.decks[other].armed && self.decks[other].feed.input.is_some() {
                            let o = &mut self.decks[other];
                            o.armed = false;
                            o.playing = true;
                            o.gain = 1.0;
                            o.target = 1.0;
                            o.step = 0.0;
                            self.shared.switched_to.store(other as u32, Ordering::Relaxed);
                            self.shared.switched.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if input.track.ready.load(Ordering::Relaxed) {
                        input.track.starved.store(true, Ordering::Relaxed);
                    }
                } else if let Some(input) = deck.feed.input.as_ref() {
                    input.track.starved.store(false, Ordering::Relaxed);
                }
                // a deck faded out stops
                let deck = &mut self.decks[d];
                if deck.target <= 0.0 && deck.gain <= 0.0 && deck.step == 0.0 {
                    deck.playing = false;
                }
            }
        }
        // the effects, then the volume and the pause ramp
        self.chain.process(mix, self.rate);
        let vol_target = Shared::f(&self.shared.volume).clamp(0.0, 1.0);
        let run_target = if paused { 0.0 } else { 1.0 };
        // about 5 ms for a pause, 20 ms for the volume knob
        let run_step = 1.0 / (0.005 * self.rate as f32);
        let vol_step = 1.0 / (0.02 * self.rate as f32);
        let mut peak = 0f32;
        for i in 0..frames {
            if self.run != run_target {
                self.run = if run_target > self.run { (self.run + run_step).min(1.0) } else { (self.run - run_step).max(0.0) };
            }
            if self.volume != vol_target {
                let d = vol_target - self.volume;
                self.volume = if d.abs() <= vol_step { vol_target } else { self.volume + vol_step * d.signum() };
            }
            // the knob is felt as loudness: a curve, not a straight line
            let g = self.volume * self.volume * self.run;
            let (l, r) = (mix[2 * i] * g, mix[2 * i + 1] * g);
            peak = peak.max(l.abs()).max(r.abs());
            let o = &mut out[i * channels..(i + 1) * channels];
            if channels == 1 {
                o[0] = (l + r) * 0.5;
            } else {
                o[0] = l;
                o[1] = r;
                for x in &mut o[2..] {
                    *x = 0.0;
                }
            }
        }
        self.shared.peak.store(peak.to_bits(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::decode::BLOCK;

    fn block(epoch: u32, pos: u64, value: f32, end: bool) -> Block {
        let mut b = Block { epoch, pos, len: BLOCK as u16, end, data: [0.0; BLOCK * 2] };
        for s in b.data.iter_mut() {
            *s = value;
        }
        b
    }

    fn setup() -> (Mixer, rtrb::Producer<Cmd>, Arc<Shared>) {
        let (ctx, crx) = rtrb::RingBuffer::new(16);
        let (ttx, _trx) = rtrb::RingBuffer::new(16);
        let shared = Shared::new();
        shared.volume.store(1.0f32.to_bits(), Ordering::Relaxed);
        shared.paused.store(false, Ordering::Relaxed);
        let mut m = Mixer::new(crx, ttx, shared.clone(), 48_000);
        m.run = 1.0;
        (m, ctx, shared)
    }

    fn deck(value: f32, blocks: usize) -> (DeckIn, Arc<Track>) {
        let (mut p, c) = rtrb::RingBuffer::new(blocks + 1);
        let track = Track::new();
        track.rate.store(48_000, Ordering::Relaxed);
        for i in 0..blocks {
            let _ = p.push(block(0, (i * BLOCK) as u64, value, i + 1 == blocks));
        }
        track.ready.store(true, Ordering::Relaxed);
        (DeckIn { ring: c, track: track.clone() }, track)
    }

    #[test]
    fn plays_a_deck_and_reports_its_end() {
        let (mut m, mut cmds, _) = setup();
        let (d, track) = deck(0.5, 2);
        let _ = cmds.push(Cmd::Load(0, d));
        let _ = cmds.push(Cmd::Play(0, true));
        let mut out = vec![0f32; 512 * 2];
        m.render(&mut out, 2);
        // a quarter: the volume knob is a curve (1.0² here) times the deck's 0.5
        assert!((out[600] - 0.5).abs() < 1e-3, "{}", out[600]);
        assert!(track.pos.load(Ordering::Relaxed) > 0);
        for _ in 0..10 {
            m.render(&mut out, 2);
        }
        assert!(track.ended.load(Ordering::Relaxed));
    }

    #[test]
    fn the_armed_deck_takes_over_without_a_gap() {
        let (mut m, mut cmds, shared) = setup();
        let (a, _) = deck(0.25, 1);
        let (b, tb) = deck(0.75, 4);
        let _ = cmds.push(Cmd::Load(0, a));
        let _ = cmds.push(Cmd::Load(1, b));
        let _ = cmds.push(Cmd::Play(0, true));
        let _ = cmds.push(Cmd::Arm(1, true));
        let mut out = vec![0f32; 2048 * 2];
        m.render(&mut out, 2);
        m.render(&mut out, 2);
        assert_eq!(shared.switched.load(Ordering::Relaxed), 1);
        assert!(tb.pos.load(Ordering::Relaxed) > 0, "the next song started on its own");
    }

    #[test]
    fn old_sound_after_a_jump_is_skipped() {
        let (mut m, mut cmds, _) = setup();
        let (mut p, c) = rtrb::RingBuffer::new(8);
        let track = Track::new();
        track.rate.store(48_000, Ordering::Relaxed);
        track.ready.store(true, Ordering::Relaxed);
        let _ = p.push(block(0, 0, 0.9, false));
        let _ = p.push(block(1, 48_000, 0.1, false));
        track.epoch.store(1, Ordering::Relaxed);
        let _ = cmds.push(Cmd::Load(0, DeckIn { ring: c, track: track.clone() }));
        let _ = cmds.push(Cmd::Play(0, true));
        let mut out = vec![0f32; 256 * 2];
        m.render(&mut out, 2);
        assert!((out[300] - 0.1).abs() < 1e-3, "{}", out[300]);
        assert!(track.pos.load(Ordering::Relaxed) >= 48_000);
    }

    #[test]
    fn a_pause_is_silent_and_keeps_the_place() {
        let (mut m, mut cmds, shared) = setup();
        let (d, track) = deck(0.5, 8);
        let _ = cmds.push(Cmd::Load(0, d));
        let _ = cmds.push(Cmd::Play(0, true));
        let mut out = vec![0f32; 512 * 2];
        m.render(&mut out, 2);
        shared.paused.store(true, Ordering::Relaxed);
        m.render(&mut out, 2);
        let at = track.pos.load(Ordering::Relaxed);
        m.render(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(track.pos.load(Ordering::Relaxed), at);
    }
}
