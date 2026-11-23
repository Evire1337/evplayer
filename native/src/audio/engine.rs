//! The sound as the player sees it: load a song on a deck, play, pause, seek, fade. The work
//! is done by the decoder threads (decode.rs) and the sound card's thread (output.rs).
//!
//! The output to the sound card is opened on the first play and paused when the player has
//! been quiet a while (the player decides when), so an idle EvPlayer keeps no audio thread
//! busy and the system can let the sound card sleep.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::decode::{Block, Decoder, Track, RING_BLOCKS};
use super::output::{Cmd, DeckIn, Mixer, Shared};
use super::source::Spec;

pub struct Engine {
    pub shared: Arc<Shared>,
    cmds: rtrb::Producer<Cmd>,
    fx: rtrb::Producer<super::dsp::Params>,
    trash: rtrb::Consumer<DeckIn>,
    mixer: Arc<Mutex<Mixer>>,
    stream: Option<cpal::Stream>,
    running: bool,
    decks: [Option<Decoder>; 2],
    /// what the sound card is ("Динамики (Realtek) · 48 кГц"), for the diagnostics
    pub device: String,
}

impl Engine {
    pub fn new() -> Self {
        let shared = Shared::new();
        let (cmds, crx) = rtrb::RingBuffer::new(64);
        let (ttx, trash) = rtrb::RingBuffer::new(16);
        let (fx, fx_rx) = rtrb::RingBuffer::new(32);
        let mixer = Arc::new(Mutex::new(Mixer::with_fx(crx, ttx, shared.clone(), 48_000, Some(fx_rx))));
        Self { shared, cmds, fx, trash, mixer, stream: None, running: false, decks: [None, None], device: String::new() }
    }

    fn build(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("нет устройства вывода звука")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = config.sample_rate();
        let channels = config.channels() as usize;
        self.device = format!("{} · {} кГц", device.description().map(|d| d.to_string()).unwrap_or_else(|_| "звуковая карта".into()), rate / 1000);
        self.shared.device_rate.store(rate, Ordering::Relaxed);
        if let Ok(mut m) = self.mixer.lock() {
            m.set_rate(rate);
        }
        let mixer = self.mixer.clone();
        let shared = self.shared.clone();
        let err_shared = self.shared.clone();
        let on_error = move |e: cpal::Error| {
            eprintln!("[audio] {e}");
            err_shared.broken.store(true, Ordering::Relaxed);
        };
        let stream_config: cpal::StreamConfig = config.config();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream::<f32, _, _>(
                stream_config,
                move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    note_latency(&shared, info);
                    match mixer.try_lock() {
                        Ok(mut m) => m.render(out, channels),
                        Err(_) => out.fill(0.0),
                    }
                },
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => {
                let mut buf = Vec::<f32>::new();
                device.build_output_stream::<i16, _, _>(
                    stream_config,
                    move |out: &mut [i16], info: &cpal::OutputCallbackInfo| {
                        note_latency(&shared, info);
                        if buf.len() < out.len() {
                            buf.resize(out.len(), 0.0);
                        }
                        let b = &mut buf[..out.len()];
                        match mixer.try_lock() {
                            Ok(mut m) => m.render(b, channels),
                            Err(_) => b.fill(0.0),
                        }
                        for (o, s) in out.iter_mut().zip(b.iter()) {
                            *o = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                        }
                    },
                    on_error,
                    None,
                )
            }
            other => return Err(format!("формат звуковой карты {other:?} не поддерживается")),
        }
        .map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        self.shared.broken.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// the sound card is open and running
    pub fn ensure_running(&mut self) -> Result<(), String> {
        if self.shared.broken.load(Ordering::Relaxed) {
            // unplugged headphones, a changed default device: open the new one
            self.stream = None;
            self.running = false;
        }
        if self.stream.is_none() {
            self.build()?;
        }
        if !self.running {
            if let Some(s) = &self.stream {
                s.play().map_err(|e| e.to_string())?;
            }
            self.running = true;
        }
        Ok(())
    }

    /// let the sound card go quiet (after a long pause): nothing runs until the next play
    pub fn suspend(&mut self) {
        if self.running {
            if let Some(s) = &self.stream {
                let _ = s.pause();
            }
            self.running = false;
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// the sound card's thread let go of songs: freed here
    pub fn collect(&mut self) {
        while self.trash.pop().is_ok() {}
    }

    fn send(&mut self, cmd: Cmd) {
        self.collect();
        if let Err(rtrb::PushError::Full(_)) = self.cmds.push(cmd) {
            eprintln!("[audio] command ring full");
        }
    }

    /// decode `spec` on `deck` from `start_ms`; the deck stays silent until `play`
    pub fn load(&mut self, deck: usize, spec: Spec, start_ms: f64) -> Arc<Track> {
        let (prod, cons) = rtrb::RingBuffer::<Block>::new(RING_BLOCKS);
        let dec = Decoder::start(spec, start_ms, prod);
        let track = dec.track.clone();
        if start_ms > 0.0 {
            track.pos.store((start_ms / 1000.0 * 44_100.0) as u64, Ordering::Relaxed);
        }
        self.send(Cmd::Load(deck, DeckIn { ring: cons, track: track.clone() }));
        self.decks[deck] = Some(dec);
        track
    }

    pub fn unload(&mut self, deck: usize) {
        self.send(Cmd::Unload(deck));
        self.decks[deck] = None;
    }

    pub fn track(&self, deck: usize) -> Option<Arc<Track>> {
        self.decks[deck].as_ref().map(|d| d.track.clone())
    }

    pub fn play(&mut self, deck: usize, on: bool) {
        self.send(Cmd::Play(deck, on));
    }

    pub fn set_gain(&mut self, deck: usize, gain: f32) {
        self.send(Cmd::SetGain(deck, gain));
    }

    pub fn fade(&mut self, deck: usize, to: f32, secs: f32) {
        self.send(Cmd::Fade(deck, to, secs));
    }

    /// `deck` starts by itself the moment the other one's song ends
    pub fn arm(&mut self, deck: usize, on: bool) {
        self.send(Cmd::Arm(deck, on));
    }

    pub fn seek(&mut self, deck: usize, ms: f64) {
        if let Some(d) = &self.decks[deck] {
            d.seek(ms);
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }

    pub fn set_volume(&mut self, v: f32) {
        self.shared.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// the effects and the equalizer, whole
    pub fn set_fx(&mut self, p: super::dsp::Params) {
        let _ = self.fx.push(p);
    }

    pub fn set_speed(&mut self, s: f32) {
        self.shared.speed.store(s.clamp(0.25, 4.0).to_bits(), Ordering::Relaxed);
    }

    /// the sound you hear is this far behind the position (the sound card's buffer)
    pub fn latency_ms(&self) -> f64 {
        self.shared.latency_us.load(Ordering::Relaxed) as f64 / 1000.0
    }

    pub fn mixer(&self) -> &Arc<Mutex<Mixer>> {
        &self.mixer
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

fn note_latency(shared: &Shared, info: &cpal::OutputCallbackInfo) {
    let ts = info.timestamp();
    let d = ts.playback.duration_since(ts.callback);
    shared.latency_us.store(d.as_micros().min(u32::MAX as u128) as u32, Ordering::Relaxed);
}
