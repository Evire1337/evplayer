//! Opus (YouTube's best sound, SoundCloud's HLS Opus) through libopus, the reference decoder.
//! Symphonia reads the Ogg and WebM containers; their packets come here.

use opusic_sys as sys;

/// Opus always decodes at this rate
pub const RATE: u32 = 48_000;
/// the longest packet Opus has (120 ms at 48 kHz)
const MAX_FRAMES: usize = 5760;

pub struct Opus {
    dec: *mut sys::OpusDecoder,
    channels: usize,
    /// samples at the very start that are the encoder's run-up, not the song (OpusHead)
    pub pre_skip: usize,
    buf: Vec<f32>,
}

// SAFETY: the decoder state is only used by the thread that owns this value
unsafe impl Send for Opus {}

/// channels and pre-skip from an "OpusHead" header (the codec's extra data in Ogg and WebM)
pub fn parse_head(head: &[u8]) -> Option<(usize, usize)> {
    if head.len() < 19 || &head[..8] != b"OpusHead" {
        return None;
    }
    let channels = head[9] as usize;
    let pre_skip = u16::from_le_bytes([head[10], head[11]]) as usize;
    Some((channels, pre_skip))
}

impl Opus {
    /// `channels` 1 or 2 (more channels need the multistream decoder: folded to stereo then)
    pub fn new(channels: usize, pre_skip: usize) -> Result<Self, String> {
        let ch = channels.clamp(1, 2);
        let mut err = 0;
        // SAFETY: plain FFI call; the result is checked
        let dec = unsafe { sys::opus_decoder_create(RATE as i32, ch as i32, &mut err) };
        if dec.is_null() || err != sys::OPUS_OK as i32 {
            return Err(format!("libopus: {err}"));
        }
        Ok(Self { dec, channels: ch, pre_skip, buf: vec![0.0; MAX_FRAMES * 2] })
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// one packet into interleaved samples; empty on a damaged packet (the song goes on)
    pub fn decode(&mut self, packet: &[u8]) -> &[f32] {
        // SAFETY: the buffer holds MAX_FRAMES frames of `channels` samples
        let n = unsafe { sys::opus_decode_float(self.dec, packet.as_ptr(), packet.len() as i32, self.buf.as_mut_ptr(), MAX_FRAMES as i32, 0) };
        if n <= 0 {
            return &[];
        }
        &self.buf[..n as usize * self.channels]
    }

    /// forget the state after a jump in the song
    pub fn reset(&mut self) {
        // SAFETY: a valid decoder and a request without arguments
        unsafe {
            sys::opus_decoder_ctl(self.dec, sys::OPUS_RESET_STATE);
        }
    }
}

impl Drop for Opus {
    fn drop(&mut self) {
        // SAFETY: created by opus_decoder_create, destroyed once
        unsafe { sys::opus_decoder_destroy(self.dec) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_opus_head() {
        let mut head = b"OpusHead".to_vec();
        head.extend_from_slice(&[1, 2, 0x38, 0x01, 0x80, 0xbb, 0, 0, 0, 0, 0]);
        assert_eq!(parse_head(&head), Some((2, 312)));
        assert_eq!(parse_head(b"nope"), None);
    }

    #[test]
    fn makes_a_decoder_and_survives_rubbish() {
        let mut d = Opus::new(2, 0).unwrap();
        // a damaged packet gives nothing instead of breaking the song
        let _ = d.decode(&[0xff, 0x00, 0x13]);
        d.reset();
    }
}
