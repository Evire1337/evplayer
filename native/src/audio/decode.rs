//! A song turned into sound: a thread per loaded song reads its source, decodes it and hands
//! the samples to the sound card's thread in blocks through a lock-free ring (output.rs).
//!
//! Every block says where in the song it starts and which "jump" it belongs to: after a seek
//! the old blocks still in the ring are simply skipped by the sound card's thread, so a seek
//! needs no waiting on either side, and the position shown is always the position heard.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;

use super::opus::{self, Opus};
use super::source::{self, Opened, Spec};

/// frames in a block
pub const BLOCK: usize = 1024;
/// blocks in a deck's ring: about 1.4 s at 48 kHz, enough to ride over a busy moment
pub const RING_BLOCKS: usize = 64;

#[derive(Clone)]
pub struct Block {
    pub epoch: u32,
    /// the song's frame at the start of the block
    pub pos: u64,
    pub len: u16,
    /// the song ends after this block
    pub end: bool,
    pub data: [f32; BLOCK * 2],
}

impl Block {
    fn new(epoch: u32, pos: u64) -> Self {
        Self { epoch, pos, len: 0, end: false, data: [0.0; BLOCK * 2] }
    }
}

/// what the decoder and the sound card's thread both see of a loaded song
pub struct Track {
    /// the jump the sound belongs to; blocks of an older one are skipped
    pub epoch: AtomicU32,
    /// the frame being heard, in the song's own rate
    pub pos: AtomicU64,
    pub rate: AtomicU32,
    /// frames in the song, when the file says (0: unknown)
    pub frames: AtomicU64,
    /// the decoder got going and the first sound is in the ring
    pub ready: AtomicBool,
    /// the last sound was played
    pub ended: AtomicBool,
    /// the ring ran dry before the end (the network is slow)
    pub starved: AtomicBool,
    pub failed: Mutex<Option<String>>,
    /// what the sound is, for the person to see ("aac 128 кбит/с")
    pub codec: Mutex<String>,
    /// part of the song fetched so far, for the progress bar
    pub remote: Mutex<Option<Arc<source::RemoteHandle>>>,
}

impl Track {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            epoch: AtomicU32::new(0),
            pos: AtomicU64::new(0),
            rate: AtomicU32::new(44_100),
            frames: AtomicU64::new(0),
            ready: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            starved: AtomicBool::new(false),
            failed: Mutex::new(None),
            codec: Mutex::new(String::new()),
            remote: Mutex::new(None),
        })
    }

    pub fn position_ms(&self) -> f64 {
        self.pos.load(Ordering::Relaxed) as f64 * 1000.0 / self.rate.load(Ordering::Relaxed).max(1) as f64
    }

    pub fn duration_ms(&self) -> Option<f64> {
        let f = self.frames.load(Ordering::Relaxed);
        (f > 0).then(|| f as f64 * 1000.0 / self.rate.load(Ordering::Relaxed).max(1) as f64)
    }

    pub fn error(&self) -> Option<String> {
        self.failed.lock().unwrap().clone()
    }
}

pub enum Command {
    /// jump to this many ms, the blocks after it marked with this epoch
    Seek(f64, u32),
    Stop,
}

enum Codec {
    Sym(Box<dyn AudioDecoder>),
    Opus(Opus),
}

struct Decoding {
    format: Box<dyn FormatReader>,
    codec: Codec,
    track_id: u32,
    rate: u32,
    time_base: Option<symphonia::core::units::TimeBase>,
    playlist: Option<source::Playlist>,
    /// seconds of the song before what `format` reads (HLS started at a later piece)
    offset: f64,
}

fn codec_name(id: symphonia::core::codecs::audio::AudioCodecId) -> &'static str {
    use symphonia::core::codecs::audio::well_known::*;
    match id {
        CODEC_ID_MP3 => "mp3",
        CODEC_ID_AAC => "aac",
        CODEC_ID_FLAC => "flac",
        CODEC_ID_VORBIS => "vorbis",
        CODEC_ID_OPUS => "opus",
        CODEC_ID_ALAC => "alac",
        _ => "pcm",
    }
}

fn open_decoding(opened: Opened, ext: Option<&str>) -> Result<(Decoding, String), String> {
    let mut hint = Hint::new();
    if let Some(e) = ext {
        hint.with_extension(e);
    }
    let mss = MediaSourceStream::new(opened.media, MediaSourceStreamOptions::default());
    let format = symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default()).map_err(|e| format!("формат не распознан: {e}"))?;
    let track = format.default_track(TrackType::Audio).ok_or("в файле нет звука")?;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or("в файле нет звука")?.clone();
    let track_id = track.id;
    let time_base = track.time_base;
    let frames = track.num_frames;
    let (codec, rate) = if params.codec == CODEC_ID_OPUS {
        let (ch, pre) = params.extra_data.as_deref().and_then(opus::parse_head).unwrap_or((params.channels.as_ref().map(|c| c.count()).unwrap_or(2), 0));
        (Codec::Opus(Opus::new(ch, pre)?), opus::RATE)
    } else {
        let dec = symphonia::default::get_codecs().make_audio_decoder(&params, &AudioDecoderOptions::default()).map_err(|e| format!("кодек не поддерживается: {e}"))?;
        (Codec::Sym(dec), params.sample_rate.unwrap_or(44_100))
    };
    let _ = frames;
    let name = codec_name(params.codec).to_string();
    Ok((Decoding { format, codec, track_id, rate, time_base, playlist: opened.playlist, offset: opened.starts_at }, name))
}

/// the song's length in frames, from the track
fn total_frames(d: &Decoding) -> u64 {
    let Some(t) = d.format.tracks().iter().find(|t| t.id == d.track_id) else { return 0 };
    if let (Some(n), Some(tb)) = (t.duration.or(t.num_frames.map(symphonia::core::units::Duration::new)), t.time_base) {
        let secs = tb.calc_time_saturating(symphonia::core::units::Timestamp::new(n.get() as i64)).as_secs_f64();
        return (secs * d.rate as f64) as u64;
    }
    t.num_frames.unwrap_or(0)
}

/// interleaved samples of any channel count as stereo
fn to_stereo(src: &[f32], channels: usize, out: &mut Vec<f32>) {
    out.clear();
    match channels {
        0 => {}
        1 => {
            for s in src {
                out.push(*s);
                out.push(*s);
            }
        }
        2 => out.extend_from_slice(src),
        n => {
            // the front pair, with the centre (third channel in WAVE order) folded in
            for f in src.chunks_exact(n) {
                let c = f[2] * 0.707;
                out.push(f[0] + c);
                out.push(f[1] + c);
            }
        }
    }
}

pub struct Decoder {
    pub track: Arc<Track>,
    commands: std::sync::mpsc::Sender<Command>,
}

impl Decoder {
    /// start decoding `spec` from `start_ms` into `ring`
    pub fn start(spec: Spec, start_ms: f64, ring: rtrb::Producer<Block>) -> Self {
        let track = Track::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let t = track.clone();
        let _ = std::thread::Builder::new().name("decoder".into()).spawn(move || run(spec, start_ms, ring, t, rx));
        Self { track, commands: tx }
    }

    /// jump to `ms`; returns the epoch the sound after the jump carries
    pub fn seek(&self, ms: f64) -> u32 {
        let epoch = self.track.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        self.track.ended.store(false, Ordering::Relaxed);
        self.track.pos.store((ms.max(0.0) / 1000.0 * self.track.rate.load(Ordering::Relaxed) as f64) as u64, Ordering::Relaxed);
        let _ = self.commands.send(Command::Seek(ms, epoch));
        epoch
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
    }
}

fn fail(track: &Track, why: String) {
    eprintln!("[decoder] {why}");
    *track.failed.lock().unwrap() = Some(why);
}

fn run(spec: Spec, start_ms: f64, mut ring: rtrb::Producer<Block>, track: Arc<Track>, rx: Receiver<Command>) {
    let ext = spec.extension();
    let opened = match source::open(&spec) {
        Ok(o) => o,
        Err(e) => return fail(&track, e),
    };
    *track.remote.lock().unwrap() = opened.remote.clone();
    let (mut d, name) = match open_decoding(opened, ext.as_deref()) {
        Ok(x) => x,
        Err(e) => return fail(&track, e),
    };
    *track.codec.lock().unwrap() = name;
    track.rate.store(d.rate, Ordering::Relaxed);
    track.frames.store(total_frames(&d), Ordering::Relaxed);

    let mut epoch = track.epoch.load(Ordering::SeqCst);
    let mut block = Block::new(epoch, 0);
    // frames still to drop: the rest of the way to a seek's target (song time), and the
    // run-up of Opus at the very start (not song time at all)
    let mut skip: u64 = 0;
    let mut runup: u64 = match &d.codec {
        Codec::Opus(o) => o.pre_skip as u64,
        _ => 0,
    };
    let mut next_pos: u64 = 0;
    let mut stereo: Vec<f32> = Vec::with_capacity(BLOCK * 4);
    let mut interleaved: Vec<f32> = Vec::new();
    let mut pending_seek = (start_ms > 0.0).then_some((start_ms, epoch));
    let mut finished = false;

    loop {
        // commands first: a jump or the end
        let cmd = if finished {
            match rx.recv_timeout(Duration::from_secs(3600)) {
                Ok(c) => Some(c),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        };
        match cmd {
            Some(Command::Stop) => return,
            Some(Command::Seek(ms, e)) => pending_seek = Some((ms, e)),
            None => {}
        }
        if let Some((ms, e)) = pending_seek.take() {
            epoch = e;
            finished = false;
            match seek(&mut d, ms) {
                Ok(at_frame) => {
                    let want = (ms / 1000.0 * d.rate as f64) as u64;
                    next_pos = at_frame;
                    skip = want.saturating_sub(at_frame);
                    runup = match &d.codec {
                        Codec::Opus(o) if at_frame == 0 => o.pre_skip as u64,
                        _ => 0,
                    };
                }
                Err(e) => {
                    eprintln!("[decoder] seek: {e}");
                }
            }
            block = Block::new(epoch, next_pos + skip);
        }

        // the ring is full: the sound card is well ahead, wait a while
        if ring.slots() == 0 {
            match rx.recv_timeout(Duration::from_millis(120)) {
                Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(Command::Seek(ms, e)) => pending_seek = Some((ms, e)),
                Err(RecvTimeoutError::Timeout) => {}
            }
            continue;
        }

        let packet = match d.format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => {
                flush_end(&mut ring, &mut block, &track);
                finished = true;
                continue;
            }
            Err(SymError::ResetRequired) => {
                if let Codec::Sym(dec) = &mut d.codec {
                    dec.reset();
                }
                continue;
            }
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                flush_end(&mut ring, &mut block, &track);
                finished = true;
                continue;
            }
            Err(e) => {
                if !track.ready.load(Ordering::Relaxed) {
                    return fail(&track, format!("не читается: {e}"));
                }
                // the middle of a song broke off (the network): what was heard stays heard
                eprintln!("[decoder] stream broke: {e}");
                *track.failed.lock().unwrap() = Some("Поток прервался".into());
                flush_end(&mut ring, &mut block, &track);
                finished = true;
                continue;
            }
        };
        if packet.track_id != d.track_id {
            continue;
        }
        let (trim_start, trim_end) = (packet.trim_start.get() as usize, packet.trim_end.get() as usize);
        let channels;
        match &mut d.codec {
            Codec::Sym(dec) => match dec.decode(&packet) {
                Ok(buf) => {
                    channels = buf.spec().channels().count();
                    buf.copy_to_vec_interleaved(&mut interleaved);
                }
                Err(SymError::DecodeError(_)) => continue,
                Err(e) => {
                    eprintln!("[decoder] {e}");
                    continue;
                }
            },
            Codec::Opus(o) => {
                channels = o.channels();
                let s = o.decode(&packet.data);
                interleaved.clear();
                interleaved.extend_from_slice(s);
            }
        }
        if channels == 0 {
            continue;
        }
        let frames = interleaved.len() / channels;
        let keep_from = trim_start.min(frames);
        let keep_to = frames.saturating_sub(trim_end).max(keep_from);
        to_stereo(&interleaved[keep_from * channels..keep_to * channels], channels, &mut stereo);
        let mut frames = stereo.len() / 2;
        let mut from = 0usize;
        if runup > 0 {
            let s = (runup as usize).min(frames);
            runup -= s as u64;
            from = s;
            frames -= s;
        }
        if skip > 0 && frames > 0 {
            let s = (skip as usize).min(frames);
            skip -= s as u64;
            from += s;
            frames -= s;
            next_pos += s as u64;
        }
        let mut i = from;
        while frames > 0 {
            let room = BLOCK - block.len as usize;
            let n = room.min(frames);
            let at = block.len as usize * 2;
            block.data[at..at + n * 2].copy_from_slice(&stereo[i * 2..(i + n) * 2]);
            block.len += n as u16;
            i += n;
            frames -= n;
            next_pos += n as u64;
            if block.len as usize == BLOCK {
                let full = std::mem::replace(&mut block, Block::new(epoch, next_pos));
                if push(&mut ring, full, &rx, &mut pending_seek).is_err() {
                    return;
                }
                if pending_seek.is_some() {
                    break;
                }
                track.ready.store(true, Ordering::Relaxed);
            }
        }
        let _ = d.time_base;
        let _ = d.offset;
    }
}

/// put a block in the ring, waiting while it is full (a command arriving meanwhile is kept)
fn push(ring: &mut rtrb::Producer<Block>, mut b: Block, rx: &Receiver<Command>, pending: &mut Option<(f64, u32)>) -> Result<(), ()> {
    loop {
        match ring.push(b) {
            Ok(()) => return Ok(()),
            Err(rtrb::PushError::Full(back)) => {
                b = back;
                match rx.recv_timeout(Duration::from_millis(60)) {
                    Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return Err(()),
                    Ok(Command::Seek(ms, e)) => {
                        *pending = Some((ms, e));
                        return Ok(());
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
        }
    }
}

fn flush_end(ring: &mut rtrb::Producer<Block>, block: &mut Block, track: &Track) {
    block.end = true;
    let b = std::mem::replace(block, Block::new(block.epoch, block.pos + block.len as u64));
    // the last block always gets in: the sound card's thread keeps taking from the ring
    let mut b = Some(b);
    for _ in 0..200 {
        match ring.push(b.take().unwrap()) {
            Ok(()) => break,
            Err(rtrb::PushError::Full(back)) => {
                b = Some(back);
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    track.ready.store(true, Ordering::Relaxed);
}

/// jump the reader near `ms`; returns the frame it landed on (the rest is skipped by frames)
fn seek(d: &mut Decoding, ms: f64) -> Result<u64, String> {
    let secs = (ms / 1000.0).max(0.0);
    let reset = |d: &mut Decoding| match &mut d.codec {
        Codec::Sym(dec) => dec.reset(),
        Codec::Opus(o) => o.reset(),
    };
    if let Some(list) = d.playlist.clone() {
        // a joined HLS stream cannot seek: it is started again at the piece holding the time
        let (piece, start) = source::piece_at(&list, secs);
        let reader = source::HlsReader::new(&list, piece);
        let opened = Opened { media: Box::new(reader), playlist: Some(list), starts_at: start, remote: None };
        let (nd, _) = open_decoding(opened, None)?;
        *d = nd;
        return Ok((start * d.rate as f64) as u64);
    }
    let time = Time::try_from_secs_f64(secs).ok_or("bad time")?;
    let r = d.format.seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(d.track_id) });
    reset(d);
    match r {
        Ok(seeked) => {
            let tb = d.time_base;
            let frame = match tb {
                Some(tb) => (tb.calc_time_saturating(seeked.actual_ts).as_secs_f64() * d.rate as f64) as u64,
                None => seeked.actual_ts.get().max(0) as u64,
            };
            Ok(frame)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a WAVE file of a known tone, decoded through the whole path: blocks, positions, the end
    #[test]
    fn decodes_a_file_into_blocks() {
        let dir = std::env::temp_dir().join("evplayer-native-decode-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tone.wav");
        let rate = 22_050u32;
        let frames = rate as usize * 2; // two seconds
        let mut wav = Vec::new();
        let data_len = (frames * 2 * 2) as u32;
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let s = ((i as f32 * 0.05).sin() * 8000.0) as i16;
            wav.extend_from_slice(&s.to_le_bytes());
            wav.extend_from_slice(&(-s).to_le_bytes());
        }
        std::fs::write(&path, wav).unwrap();

        let (prod, mut cons) = rtrb::RingBuffer::<Block>::new(8);
        let dec = Decoder::start(Spec::File(path), 0.0, prod);
        let mut got = 0usize;
        let mut first_pos = None;
        let mut ended = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !ended && std::time::Instant::now() < deadline {
            match cons.pop() {
                Ok(b) => {
                    first_pos.get_or_insert(b.pos);
                    assert_eq!(b.pos as usize, got, "blocks follow each other");
                    // the right channel is the left one inverted
                    for f in 0..b.len as usize {
                        assert!((b.data[2 * f] + b.data[2 * f + 1]).abs() < 1e-3);
                    }
                    got += b.len as usize;
                    ended = b.end;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(2)),
            }
        }
        assert!(ended);
        assert_eq!(got, frames);
        assert_eq!(dec.track.rate.load(Ordering::Relaxed), rate);
        assert_eq!(dec.track.frames.load(Ordering::Relaxed) as usize, frames);

        // a jump to one second: the blocks after it carry the new epoch and start there
        let epoch = dec.seek(1000.0);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            assert!(std::time::Instant::now() < deadline, "no blocks after the seek");
            if let Ok(b) = cons.pop() {
                if b.epoch == epoch && b.len > 0 {
                    assert_eq!(b.pos as usize, rate as usize);
                    break;
                }
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
