//! Covers and pictures. Fetched by a few worker threads, scaled down at once to the size they
//! are shown at (a 500 px cover in a 48 px row is 48 px of memory, not 500), kept on the
//! video card as textures, and only so many of them: the least recently shown go first, and a
//! window hidden for a while lets them all go. Scaled covers are also kept on disk, so the next
//! start shows them without the network (unless "Не хранить кэш" is on).

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};

use crate::model::{Source, Track};

/// pictures kept on the video card at most (a page of covers and some more)
const MAX_TEXTURES: usize = 160;
const WORKERS: usize = 4;
/// the disk cache is trimmed to this
const DISK_LIMIT_BYTES: u64 = 120 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
struct Key(u64, u32);

enum Slot {
    Loading,
    Ready(TextureHandle, u64),
    Failed,
}

struct Job {
    key: Key,
    src: String,
    px: u32,
}

struct Shared {
    jobs: Mutex<VecDeque<Job>>,
    wake: Condvar,
    done: Mutex<Vec<(Key, Option<ColorImage>)>>,
}

/// the average colour of a picture, a little brighter (the backdrop's glow)
fn average(img: &ColorImage) -> egui::Color32 {
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for p in img.pixels.iter().step_by(7) {
        r += p.r() as u64;
        g += p.g() as u64;
        b += p.b() as u64;
        n += 1;
    }
    let n = n.max(1);
    let lift = |v: u64| ((v / n) as f32 * 1.25).min(255.0) as u8;
    egui::Color32::from_rgb(lift(r), lift(g), lift(b))
}

pub struct Images {
    slots: HashMap<Key, Slot>,
    tints: HashMap<u64, egui::Color32>,
    shared: Arc<Shared>,
    started: bool,
    frame: u64,
    disk: Option<PathBuf>,
    local_covers: PathBuf,
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// where a track's picture comes from: its cover address, or for your own files the cover the
/// library took out of the file
pub fn artwork_of(t: &Track) -> Option<String> {
    if t.source == Source::Local {
        return t.id.strip_prefix("local:").map(|s| format!("localcover:{s}"));
    }
    t.artwork.clone()
}

fn fetch(src: &str, local_covers: &std::path::Path) -> Option<Vec<u8>> {
    if let Some(stem) = src.strip_prefix("localcover:") {
        for ext in ["jpg", "png"] {
            if let Ok(b) = std::fs::read(local_covers.join(format!("{stem}.{ext}"))) {
                return Some(b);
            }
        }
        return None;
    }
    if let Some(rest) = src.strip_prefix("data:") {
        let (_, b64) = rest.split_once(";base64,")?;
        use base64::Engine;
        return base64::engine::general_purpose::STANDARD.decode(b64).ok();
    }
    crate::services::http::get_image(src).ok()
}

/// decoded and scaled to cover a `px` square (the middle of a wide picture, like CSS's cover)
fn decode(bytes: &[u8], px: u32) -> Option<image::RgbaImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    let side = w.min(h);
    let img = if w != h { img.crop_imm((w - side) / 2, (h - side) / 2, side, side) } else { img };
    let target = px.min(side).max(1);
    let img = if side > target { img.resize_exact(target, target, image::imageops::FilterType::Triangle) } else { img };
    Some(img.into_rgba8())
}

/// the size asked for the backdrop's picture (not a real size)
const BACKDROP: u32 = 0;
/// the backdrop's key differs from the cover's
const BACKDROP_SALT: u64 = 0x5eed_bacd_0000_0001;

/// a cover already on disk, at any size (the backdrop needs only a small one)
fn find_cached(dir: &std::path::Path, cover_key: u64) -> Option<Vec<u8>> {
    let prefix = format!("{cover_key:016x}_");
    let rd = std::fs::read_dir(dir).ok()?;
    let file = rd.flatten().find(|e| e.file_name().to_string_lossy().starts_with(&prefix))?;
    std::fs::read(file.path()).ok()
}

/// The backdrop as the page's shader made it, without the movement: the middle 70 % of the
/// cover, tiny and blurred, colours made 28 % richer and dimmed to 78 %.
fn backdrop(bytes: &[u8]) -> Option<image::RgbaImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = (img.width(), img.height());
    let side = (w.min(h) as f32 * 0.7) as u32;
    if side == 0 {
        return None;
    }
    let img = img.crop_imm((w - side) / 2, (h - side) / 2, side, side).resize_exact(32, 32, image::imageops::FilterType::Triangle).into_rgba8();
    let mut img = image::imageops::blur(&img, 2.2);
    for p in img.pixels_mut() {
        let [r, g, b, _] = p.0.map(|c| c as f32);
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        let rich = |c: f32| ((l + (c - l) * 1.28) * 0.6).clamp(0.0, 255.0) as u8;
        p.0 = [rich(r), rich(g), rich(b), 255];
    }
    Some(img)
}

fn worker(shared: Arc<Shared>, disk: Option<PathBuf>, local_covers: PathBuf) {
    loop {
        let job = {
            let mut q = shared.jobs.lock().unwrap();
            loop {
                if let Some(j) = q.pop_front() {
                    break j;
                }
                q = shared.wake.wait(q).unwrap();
            }
        };
        // the backdrop: the cover, small, blurred and richer (px 0 asks for it)
        if job.px == BACKDROP {
            let img = disk.as_ref().and_then(|d| find_cached(d, job.key.0 ^ BACKDROP_SALT)).or_else(|| fetch(&job.src, &local_covers)).and_then(|b| backdrop(&b));
            let image = img.map(|img| {
                let (w, h) = img.dimensions();
                ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw())
            });
            shared.done.lock().unwrap().push((job.key, image));
            crate::events::repaint();
            continue;
        }
        let cached = disk.as_ref().map(|d| d.join(format!("{:016x}_{}.jpg", job.key.0, job.px)));
        let mut rgba = cached.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| image::load_from_memory(&b).ok()).map(|i| i.into_rgba8());
        if rgba.is_none() {
            rgba = fetch(&job.src, &local_covers).and_then(|b| decode(&b, job.px));
            if let (Some(img), Some(path)) = (&rgba, &cached) {
                if !job.src.starts_with("localcover:") && !job.src.starts_with("data:") {
                    let rgb = image::DynamicImage::ImageRgba8(img.clone()).into_rgb8();
                    let _ = rgb.save_with_format(path, image::ImageFormat::Jpeg);
                }
            }
        }
        let image = rgba.map(|img| {
            let (w, h) = img.dimensions();
            ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw())
        });
        shared.done.lock().unwrap().push((job.key, image));
        crate::events::repaint();
    }
}

impl Images {
    pub fn new(cache_dir: Option<PathBuf>, local_covers: PathBuf) -> Self {
        if let Some(d) = &cache_dir {
            let _ = std::fs::create_dir_all(d);
        }
        Self { slots: HashMap::new(), tints: HashMap::new(), shared: Arc::new(Shared { jobs: Mutex::new(VecDeque::new()), wake: Condvar::new(), done: Mutex::new(Vec::new()) }), started: false, frame: 0, disk: cache_dir, local_covers }
    }

    /// a new frame: pictures that arrived become textures, the oldest go past the limit
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        let done: Vec<_> = std::mem::take(&mut *self.shared.done.lock().unwrap());
        for (key, img) in done {
            if let Some(i) = &img {
                self.tints.entry(key.0).or_insert_with(|| average(i));
            }
            let slot = match img {
                Some(img) => Slot::Ready(ctx.load_texture(format!("img{:x}_{}", key.0, key.1), img, TextureOptions::LINEAR), self.frame),
                None => Slot::Failed,
            };
            self.slots.insert(key, slot);
        }
        let ready = self.slots.values().filter(|s| matches!(s, Slot::Ready(..))).count();
        if ready > MAX_TEXTURES {
            let mut ages: Vec<(u64, Key)> = self.slots.iter().filter_map(|(k, s)| if let Slot::Ready(_, used) = s { Some((*used, *k)) } else { None }).collect();
            ages.sort();
            for (_, k) in ages.into_iter().take(ready - MAX_TEXTURES) {
                self.slots.remove(&k);
            }
        }
    }

    /// the picture at `src` for a square of `points` (×`ppp` pixels); None while it loads
    pub fn get(&mut self, src: &str, points: f32, ppp: f32) -> Option<TextureId> {
        if src.is_empty() {
            return None;
        }
        // sizes in steps of 64 px, so a cover shown a little bigger or smaller is one texture
        let px = (((points * ppp).ceil() as u32).div_ceil(64) * 64).clamp(64, 1024);
        let key = Key(hash(src), px);
        let frame = self.frame;
        match self.slots.get_mut(&key) {
            Some(Slot::Ready(t, used)) => {
                *used = frame;
                return Some(t.id());
            }
            Some(_) => return None,
            None => {}
        }
        // while it loads, a bigger or smaller copy already here stands in
        let h = key.0;
        if let Some(t) = self.slots.iter_mut().find_map(|(k, s)| if k.0 == h { if let Slot::Ready(t, used) = s { *used = frame; Some(t.id()) } else { None } } else { None }) {
            if self.slots.keys().any(|k| k.0 == h && k.1 >= px) {
                return Some(t);
            }
            self.request(key, src, px);
            return Some(t);
        }
        self.request(key, src, px);
        None
    }

    fn request(&mut self, key: Key, src: &str, px: u32) {
        if self.slots.contains_key(&key) {
            return;
        }
        self.slots.insert(key, Slot::Loading);
        if !self.started {
            self.started = true;
            for _ in 0..WORKERS {
                let shared = self.shared.clone();
                let disk = self.disk.clone();
                let local = self.local_covers.clone();
                let _ = std::thread::Builder::new().name("images".into()).spawn(move || worker(shared, disk, local));
            }
            if let Some(d) = self.disk.clone() {
                std::thread::spawn(move || trim_disk(&d));
            }
        }
        self.shared.jobs.lock().unwrap().push_back(Job { key, src: src.to_string(), px });
        self.shared.wake.notify_one();
    }

    /// the backdrop made from a cover (None while it is made)
    pub fn backdrop(&mut self, src: &str) -> Option<TextureId> {
        let key = Key(hash(src) ^ BACKDROP_SALT, BACKDROP);
        let frame = self.frame;
        match self.slots.get_mut(&key) {
            Some(Slot::Ready(t, used)) => {
                *used = frame;
                Some(t.id())
            }
            Some(_) => None,
            None => {
                self.request(key, src, BACKDROP);
                None
            }
        }
    }

    /// the colour of a picture already loaded (any size)
    pub fn tint(&self, src: &str) -> Option<egui::Color32> {
        self.tints.get(&hash(src)).copied()
    }

    /// everything goes (the window was hidden a while); it comes back from disk when shown
    pub fn release_all(&mut self) {
        self.slots.retain(|_, s| matches!(s, Slot::Loading));
    }

    pub fn count(&self) -> usize {
        self.slots.values().filter(|s| matches!(s, Slot::Ready(..))).count()
    }
}

/// the oldest cached covers go when the folder grows past its limit
fn trim_disk(dir: &std::path::Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd.flatten().filter_map(|e| {
        let m = e.metadata().ok()?;
        Some((m.modified().ok()?, m.len(), e.path()))
    }).collect();
    let total: u64 = files.iter().map(|f| f.1).sum();
    if total <= DISK_LIMIT_BYTES {
        return;
    }
    files.sort();
    let mut over = total - DISK_LIMIT_BYTES * 8 / 10;
    for (_, len, path) in files {
        if over == 0 {
            break;
        }
        let _ = std::fs::remove_file(path);
        over = over.saturating_sub(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_to_the_middle_square() {
        let mut img = image::RgbaImage::new(400, 200);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if (100..300).contains(&x) { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) };
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let out = decode(&png, 64).unwrap();
        assert_eq!(out.dimensions(), (64, 64));
        // only the red middle is left
        assert!(out.pixels().all(|p| p.0[0] > 200 && p.0[2] < 60));
    }

    #[test]
    fn local_files_use_the_library_s_covers() {
        let t = Track { id: "local:abc".into(), source: Source::Local, artwork: Some("http://127.0.0.1:1/x/cover/abc".into()), ..Default::default() };
        assert_eq!(artwork_of(&t).as_deref(), Some("localcover:abc"));
    }
}
