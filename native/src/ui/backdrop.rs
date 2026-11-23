//! The live background (src/ui/backdrop/*): the cover of the song playing, warped slowly by a
//! shader on the video card (shaders/backdrop.hlsl), as the Tauri build draws it with WebGL.
//!
//! It costs as little as the page's: drawn at half the window's size, 30 frames a second while
//! a song plays and 10 while paused (it drifts four times slower then, and is dimmer), and
//! between the interface's own frames the interface is not laid out again: the video card
//! draws the background and puts the last frame's triangles over it from the buffers it still
//! holds. With the animation off (or the memory saver on) it draws once, lets a new cover fade
//! in, and holds still. A hidden window draws nothing.
//!
//! The interface shows it as a picture with the id `TEXTURE`; the painter fills that picture.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// the picture the interface draws where the background goes
pub const TEXTURE: egui::TextureId = egui::TextureId::User(0xBAC0_D0);
/// the cover's size on the video card: it is blurred anyway
pub const TEX_SIZE: u32 = 64;
const FADE_MS: f32 = 1400.0;
/// 30 frames a second look the same as 60 for a slow drift and cost half
pub const FRAME: Duration = Duration::from_millis(1000 / 30 - 2);
/// paused, it drifts four times slower: 10 frames a second are just as smooth
pub const PAUSED_FRAME: Duration = Duration::from_millis(1000 / 10 - 2);

struct Wanted {
    src: Option<String>,
    playing: bool,
    animated: bool,
    /// a cover decoded and waiting to go to the video card: (its address, 64×64 RGBA)
    ready: Option<(Option<String>, Vec<u8>)>,
    loading: Option<String>,
    shown: Option<Option<String>>,
}

static WANTED: Mutex<Wanted> = Mutex::new(Wanted { src: None, playing: false, animated: true, ready: None, loading: None, shown: None });

/// the background for this frame: the cover, whether a song plays, whether it moves. A new
/// cover is decoded on a thread with `loader`.
pub fn want(src: Option<String>, playing: bool, animated: bool, loader: impl FnOnce() -> crate::ui::images::CoverLoader) {
    let mut w = WANTED.lock().unwrap();
    w.playing = playing;
    w.animated = animated;
    if w.src == src && (w.shown.as_ref() == Some(&src) || w.loading == src || w.ready.as_ref().is_some_and(|r| r.0 == src)) {
        return;
    }
    w.src = src.clone();
    match src {
        None => w.ready = Some((None, fallback())),
        Some(s) => {
            w.loading = Some(s.clone());
            let loader = loader();
            std::thread::spawn(move || {
                let px = loader.load(&s, 256).map(|img| shrink(&img));
                let mut w = WANTED.lock().unwrap();
                if w.src.as_deref() == Some(s.as_str()) {
                    w.loading = None;
                    w.ready = Some((Some(s), px.unwrap_or_else(fallback)));
                }
                drop(w);
                crate::events::repaint();
            });
        }
    }
}

/// a cover decoded since the last frame: it fades in now
pub fn take_ready() -> Option<Vec<u8>> {
    let mut w = WANTED.lock().unwrap();
    let r = w.ready.take()?;
    w.shown = Some(r.0);
    Some(r.1)
}

/// (playing, animated)
pub fn mode() -> (bool, bool) {
    let w = WANTED.lock().unwrap();
    (w.playing, w.animated)
}

/// the painter let go of the video card: the cover goes up again when it comes back
pub fn forget_shown() {
    let mut w = WANTED.lock().unwrap();
    w.shown = None;
    w.src = None;
    w.loading = None;
}

/// the default look before anything plays: deep sea green with a cool blue glow
pub fn fallback() -> Vec<u8> {
    let n = TEX_SIZE as usize;
    let stops = [(0.0f32, [0x38, 0xd9, 0xa9]), (0.5, [0x1f, 0x6a, 0x8a]), (1.0, [0x08, 0x2b, 0x28])];
    let mut out = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            // a diagonal gradient from the top-left corner to the bottom-right
            let t = (x + y) as f32 / (2 * (n - 1)) as f32;
            let (a, b) = if t <= 0.5 { (stops[0], stops[1]) } else { (stops[1], stops[2]) };
            let k = (t - a.0) / (b.0 - a.0);
            for c in 0..3 {
                out.push((a.1[c] as f32 + (b.1[c] as f32 - a.1[c] as f32) * k).round() as u8);
            }
            out.push(255);
        }
    }
    out
}

/// the middle 70 % of a cover (many covers have a frame whose edge would be stretched along
/// the window's sides), 64×64, blurred by 2 px as the page's canvas filter does
pub fn shrink(img: &egui::ColorImage) -> Vec<u8> {
    let [w, h] = img.size;
    let side = (w.min(h) as f32 * 0.7).max(1.0) as u32;
    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    let Some(full) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) else { return fallback() };
    let x = (w as u32 - side) / 2;
    let y = (h as u32 - side) / 2;
    let cut = image::imageops::crop_imm(&full, x, y, side, side).to_image();
    let small = image::imageops::resize(&cut, TEX_SIZE, TEX_SIZE, image::imageops::FilterType::Triangle);
    image::imageops::blur(&small, 2.0).into_raw()
}

/// the fade between two covers and the clock of the drift
pub struct Clock {
    pub time: f32,
    pub mix: f32,
    mix_start: Option<Instant>,
    last: Option<Instant>,
}

impl Default for Clock {
    fn default() -> Self {
        Self { time: 0.0, mix: 1.0, mix_start: None, last: None }
    }
}

impl Clock {
    pub fn fade(&mut self) {
        self.mix = 0.0;
        self.mix_start = Some(Instant::now());
    }

    /// move on to now; (eased mix, time, dim)
    pub fn tick(&mut self, playing: bool, animated: bool) -> (f32, f32, f32) {
        let now = Instant::now();
        let dt = self.last.map(|l| now.duration_since(l).as_secs_f32()).unwrap_or(0.0).min(0.1);
        self.last = Some(now);
        if animated {
            self.time += dt * if playing { 1.0 } else { 0.25 };
        }
        if let Some(s) = self.mix_start {
            self.mix = (now.duration_since(s).as_millis() as f32 / FADE_MS).min(1.0);
            if self.mix >= 1.0 {
                self.mix_start = None;
            }
        }
        let m = self.mix;
        (m * m * (3.0 - 2.0 * m), self.time, if playing { 1.0 } else { 0.8 })
    }

    /// a new cover is still fading in
    pub fn fading(&self) -> bool {
        self.mix < 1.0
    }

    /// the clock stood still (a hidden window): the next frame does not jump
    pub fn pause(&mut self) {
        self.last = None;
    }
}

/// when the next background frame is due, if the background moves now
pub fn next_frame(clock_fading: bool) -> Option<Duration> {
    let (playing, animated) = mode();
    if animated {
        Some(if playing || clock_fading { FRAME } else { PAUSED_FRAME })
    } else if clock_fading {
        Some(FRAME)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_look_runs_from_sea_green_to_the_ground() {
        let f = fallback();
        assert_eq!(f.len(), (TEX_SIZE * TEX_SIZE * 4) as usize);
        assert_eq!(&f[..4], &[0x38, 0xd9, 0xa9, 255]);
        assert_eq!(&f[f.len() - 4..], &[0x08, 0x2b, 0x28, 255]);
    }

    #[test]
    fn a_new_cover_fades_in_and_the_drift_slows_when_paused() {
        let mut c = Clock::default();
        c.fade();
        assert!(c.fading());
        let (m, _, dim) = c.tick(false, true);
        assert!(m < 0.1 && (dim - 0.8).abs() < 1e-6);
        std::thread::sleep(Duration::from_millis(30));
        let (_, t, _) = c.tick(false, true);
        assert!(t > 0.0 && t < 0.03, "{t}");
    }
}
