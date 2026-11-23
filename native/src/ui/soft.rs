//! Draws egui's triangles with the processor, into the window's own pixels (softbuffer).
//!
//! Why not the video card: an OpenGL or DirectX context alone made the video driver load
//! 100-500 MB into the program before it drew a single letter. A player's window is mostly
//! still: it is drawn only when something on it changes, and then only the part that changed
//! (while a song plays that is the time and the progress bar, a few thousand pixels). So the
//! processor does it in well under a millisecond most of the time, and the memory stays small.
//!
//! The rules are the ones a video card follows: a pixel belongs to a triangle when its centre
//! is inside (with the top-left rule on edges, so two triangles that share an edge never both
//! paint it), colours and texture places are interpolated across the triangle, and colours are
//! premultiplied by their alpha (as egui gives them) and blended in the same gamma space egui's
//! own renderers blend in.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use egui::epaint::{ClippedPrimitive, ImageDelta, Primitive, TextureId};
use egui::TexturesDelta;

/// a texture as the drawing needs it: premultiplied RGBA, row by row
struct Texture {
    w: usize,
    h: usize,
    px: Vec<[u8; 4]>,
    /// texels are bigger than pixels (pictures shown larger than they are): blend neighbours
    linear: bool,
}

/// a rectangle of pixels: x0..x1, y0..y1 (ends excluded)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl PxRect {
    pub const EMPTY: PxRect = PxRect { x0: i32::MAX, y0: i32::MAX, x1: i32::MIN, y1: i32::MIN };

    pub fn is_empty(&self) -> bool {
        self.x0 >= self.x1 || self.y0 >= self.y1
    }

    pub fn union(self, o: PxRect) -> PxRect {
        if o.is_empty() {
            return self;
        }
        if self.is_empty() {
            return o;
        }
        PxRect { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }

    pub fn intersect(self, o: PxRect) -> PxRect {
        PxRect { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) }
    }

    pub fn area(&self) -> i64 {
        if self.is_empty() {
            0
        } else {
            (self.x1 - self.x0) as i64 * (self.y1 - self.y0) as i64
        }
    }
}

/// what was drawn last time, piece by piece, to find what changed
#[derive(Default)]
struct Drawn {
    /// (where, a fingerprint of how) per primitive, in drawing order
    pieces: Vec<(PxRect, u64)>,
    size: (usize, usize),
}

pub struct SoftRenderer {
    textures: HashMap<TextureId, Texture>,
    last: Drawn,
    /// a texture changed: everything drawn with it may look different
    textures_changed: Vec<TextureId>,
    /// the colour behind everything
    pub clear: u32,
}

/// (x * a) / 255, rounded, for 0..=255 values
#[inline(always)]
fn mul8(x: u32, a: u32) -> u32 {
    let t = x * a + 128;
    (t + (t >> 8)) >> 8
}

/// premultiplied `src` over the 0RGB pixel `dst`
#[inline(always)]
fn blend(dst: u32, r: u32, g: u32, b: u32, a: u32) -> u32 {
    if a >= 255 {
        return (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
    }
    let inv = 255 - a;
    let dr = (dst >> 16) & 0xff;
    let dg = (dst >> 8) & 0xff;
    let db = dst & 0xff;
    let or = (r + mul8(dr, inv)).min(255);
    let og = (g + mul8(dg, inv)).min(255);
    let ob = (b + mul8(db, inv)).min(255);
    (or << 16) | (og << 8) | ob
}

impl Texture {
    #[inline(always)]
    fn at(&self, x: i32, y: i32) -> [u8; 4] {
        let x = x.clamp(0, self.w as i32 - 1) as usize;
        let y = y.clamp(0, self.h as i32 - 1) as usize;
        self.px[y * self.w + x]
    }

    /// the texel colour at texture place (u, v) in 0..1, as 0..255 floats
    #[inline(always)]
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let fx = u * self.w as f32 - 0.5;
        let fy = v * self.h as f32 - 0.5;
        if !self.linear {
            let t = self.at((fx + 0.5).floor() as i32, (fy + 0.5).floor() as i32);
            return [t[0] as f32, t[1] as f32, t[2] as f32, t[3] as f32];
        }
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let (x0, y0) = (x0 as i32, y0 as i32);
        let a = self.at(x0, y0);
        let b = self.at(x0 + 1, y0);
        let c = self.at(x0, y0 + 1);
        let d = self.at(x0 + 1, y0 + 1);
        let mut out = [0f32; 4];
        for i in 0..4 {
            let top = a[i] as f32 + (b[i] as f32 - a[i] as f32) * tx;
            let bottom = c[i] as f32 + (d[i] as f32 - c[i] as f32) * tx;
            out[i] = top + (bottom - top) * ty;
        }
        out
    }
}

fn fingerprint(p: &ClippedPrimitive, changed: &[TextureId]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let c = p.clip_rect;
    (c.min.x.to_bits(), c.min.y.to_bits(), c.max.x.to_bits(), c.max.y.to_bits()).hash(&mut h);
    if let Primitive::Mesh(m) = &p.primitive {
        m.texture_id.hash(&mut h);
        // a texture that was replaced makes the mesh new even if its triangles are the same
        changed.contains(&m.texture_id).hash(&mut h);
        m.indices.hash(&mut h);
        for v in &m.vertices {
            (v.pos.x.to_bits(), v.pos.y.to_bits(), v.uv.x.to_bits(), v.uv.y.to_bits(), v.color.to_array()).hash(&mut h);
        }
    }
    h.finish()
}

/// a fingerprint of a whole frame: the same picture at the same size gives the same number,
/// and such a frame is not drawn again
pub fn frame_print(prims: &[ClippedPrimitive], w: u32, h: u32, ppp: f32) -> u64 {
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    (w, h, ppp.to_bits(), prims.len()).hash(&mut hs);
    for p in prims {
        fingerprint(p, &[]).hash(&mut hs);
    }
    hs.finish()
}

/// the pixels a primitive can touch: its triangles' bounds within its clip rect
fn bounds(p: &ClippedPrimitive, ppp: f32, w: usize, h: usize) -> PxRect {
    let screen = PxRect { x0: 0, y0: 0, x1: w as i32, y1: h as i32 };
    let clip = PxRect {
        x0: (p.clip_rect.min.x * ppp).floor() as i32,
        y0: (p.clip_rect.min.y * ppp).floor() as i32,
        x1: (p.clip_rect.max.x * ppp).ceil() as i32,
        y1: (p.clip_rect.max.y * ppp).ceil() as i32,
    }
    .intersect(screen);
    let Primitive::Mesh(m) = &p.primitive else { return PxRect::EMPTY };
    if m.vertices.is_empty() {
        return PxRect::EMPTY;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for v in &m.vertices {
        x0 = x0.min(v.pos.x);
        y0 = y0.min(v.pos.y);
        x1 = x1.max(v.pos.x);
        y1 = y1.max(v.pos.y);
    }
    PxRect { x0: (x0 * ppp).floor() as i32 - 1, y0: (y0 * ppp).floor() as i32 - 1, x1: (x1 * ppp).ceil() as i32 + 1, y1: (y1 * ppp).ceil() as i32 + 1 }.intersect(clip)
}

impl SoftRenderer {
    pub fn new(clear: u32) -> Self {
        Self { textures: HashMap::new(), last: Drawn::default(), textures_changed: Vec::new(), clear }
    }

    /// the textures egui added or replaced, before drawing
    pub fn set_textures(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for d in deltas {
                self.apply(*id, d);
            }
            self.textures_changed.push(*id);
        }
    }

    /// the textures egui let go of, after drawing
    pub fn free_textures(&mut self, delta: &TexturesDelta) {
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    fn apply(&mut self, id: TextureId, d: &ImageDelta) {
        let egui::ImageData::Color(img) = &d.image;
        let [w, h] = img.size;
        let linear = d.options.magnification == egui::TextureFilter::Linear;
        let src = img.pixels.iter().map(|c| c.to_array());
        match d.pos {
            None => {
                self.textures.insert(id, Texture { w, h, px: src.collect(), linear });
            }
            Some([x, y]) => {
                let Some(t) = self.textures.get_mut(&id) else { return };
                for (i, p) in src.enumerate() {
                    let (px, py) = (x + i % w, y + i / w);
                    if px < t.w && py < t.h {
                        t.px[py * t.w + px] = p;
                    }
                }
            }
        }
    }

    /// The part of the window that has to be drawn again: everything after a resize or when
    /// the picture changed a lot, else the places where pieces appeared, went or changed.
    fn damage(&mut self, prims: &[ClippedPrimitive], ppp: f32, w: usize, h: usize) -> (PxRect, Vec<(PxRect, u64)>) {
        let full = PxRect { x0: 0, y0: 0, x1: w as i32, y1: h as i32 };
        let changed = std::mem::take(&mut self.textures_changed);
        let now: Vec<(PxRect, u64)> = prims.iter().map(|p| (bounds(p, ppp, w, h), fingerprint(p, &[]))).collect();
        if self.last.size != (w, h) {
            return (full, now);
        }
        let old = &self.last.pieces;
        let mut dmg = PxRect::EMPTY;
        // a texture that was replaced changes everything drawn with it
        for (p, (r, _)) in prims.iter().zip(&now) {
            if let Primitive::Mesh(m) = &p.primitive {
                if changed.contains(&m.texture_id) {
                    dmg = dmg.union(*r);
                }
            }
        }
        // the same pieces in the same order, most of the time: compare them pairwise from both
        // ends, so a piece added or dropped in the middle does not mark everything after it
        let mut a = 0;
        while a < old.len() && a < now.len() && old[a] == now[a] {
            a += 1;
        }
        let (mut eo, mut en) = (old.len(), now.len());
        while eo > a && en > a && old[eo - 1] == now[en - 1] {
            eo -= 1;
            en -= 1;
        }
        for (r, _) in &old[a..eo] {
            dmg = dmg.union(*r);
        }
        for (r, _) in &now[a..en] {
            dmg = dmg.union(*r);
        }
        let dmg = dmg.intersect(full);
        // most of the window changed: drawing all of it costs the same and is simpler
        if dmg.area() * 10 > full.area() * 7 {
            return (full, now);
        }
        (dmg, now)
    }

    /// Draw the frame into `target` (0RGB, `w` x `h`). Returns the rectangle that changed, to
    /// be shown; an empty one when nothing did. `force` draws everything (the window's pixels
    /// were lost, as after it was hidden).
    pub fn render(&mut self, prims: &[ClippedPrimitive], ppp: f32, target: &mut [u32], w: usize, h: usize, force: bool) -> PxRect {
        let (mut dmg, pieces) = self.damage(prims, ppp, w, h);
        if force {
            dmg = PxRect { x0: 0, y0: 0, x1: w as i32, y1: h as i32 };
        }
        self.last = Drawn { pieces: pieces.clone(), size: (w, h) };
        if dmg.is_empty() || target.len() < w * h {
            return PxRect::EMPTY;
        }
        // the changed rows are split into bands drawn side by side on several cores, but only
        // when there is enough to draw for that to pay off
        let rows = (dmg.y1 - dmg.y0) as usize;
        let threads = if dmg.area() > 300_000 { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 4) } else { 1 };
        let clear = self.clear;
        let textures = &self.textures;
        let draw_band = |band: PxRect, slice: &mut [u32], offset_row: i32| {
            for y in band.y0..band.y1 {
                let row = &mut slice[((y - offset_row) as usize) * w..][..w];
                row[band.x0 as usize..band.x1 as usize].fill(clear);
            }
            for (p, (r, _)) in prims.iter().zip(&pieces) {
                let area = r.intersect(band);
                if area.is_empty() {
                    continue;
                }
                if let Primitive::Mesh(m) = &p.primitive {
                    let Some(tex) = textures.get(&m.texture_id) else { continue };
                    for tri in m.indices.chunks_exact(3) {
                        let (Some(a), Some(b), Some(c)) = (m.vertices.get(tri[0] as usize), m.vertices.get(tri[1] as usize), m.vertices.get(tri[2] as usize)) else { continue };
                        triangle(slice, w, offset_row, area, tex, ppp, a, b, c);
                    }
                }
            }
        };
        if threads <= 1 {
            draw_band(dmg, target, 0);
        } else {
            let per = rows.div_ceil(threads);
            let mut rest = &mut target[(dmg.y0 as usize) * w..(dmg.y1 as usize) * w];
            std::thread::scope(|scope| {
                let mut y = dmg.y0;
                while y < dmg.y1 {
                    let n = per.min((dmg.y1 - y) as usize);
                    let (mine, tail) = rest.split_at_mut(n * w);
                    rest = tail;
                    let band = PxRect { x0: dmg.x0, y0: y, x1: dmg.x1, y1: y + n as i32 };
                    let draw = &draw_band;
                    scope.spawn(move || draw(band, mine, band.y0));
                    y += n as i32;
                }
            });
        }
        dmg
    }
}

/// one triangle into `slice` (rows from `offset_row` on), only within `area`
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn triangle(slice: &mut [u32], w: usize, offset_row: i32, area: PxRect, tex: &Texture, ppp: f32, a: &egui::epaint::Vertex, b: &egui::epaint::Vertex, c: &egui::epaint::Vertex) {
    let (ax, ay) = (a.pos.x * ppp, a.pos.y * ppp);
    let (mut bx, mut by) = (b.pos.x * ppp, b.pos.y * ppp);
    let (mut cx, mut cy) = (c.pos.x * ppp, c.pos.y * ppp);
    let (mut vb, mut vc) = (b, c);
    // the signed doubled area; the corners are put in one turning direction
    let mut area2 = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
    if area2 == 0.0 || !area2.is_finite() {
        return;
    }
    if area2 < 0.0 {
        std::mem::swap(&mut bx, &mut cx);
        std::mem::swap(&mut by, &mut cy);
        std::mem::swap(&mut vb, &mut vc);
        area2 = -area2;
    }
    let va = a;
    let x0 = (ax.min(bx).min(cx).floor() as i32).max(area.x0);
    let x1 = (ax.max(bx).max(cx).ceil() as i32).min(area.x1);
    let y0 = (ay.min(by).min(cy).floor() as i32).max(area.y0);
    let y1 = (ay.max(by).max(cy).ceil() as i32).min(area.y1);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    // edge functions: e(x, y) = A*x + B*y + C, positive inside; edge i is opposite corner i
    let edge = |px: f32, py: f32, qx: f32, qy: f32| {
        let a = py - qy;
        let b = qx - px;
        let c = px * qy - py * qx;
        (a, b, c)
    };
    let e = [edge(bx, by, cx, cy), edge(cx, cy, ax, ay), edge(ax, ay, bx, by)];
    // the top-left rule: a pixel centre exactly on an edge belongs to the triangle only for a
    // left edge (A > 0) or a top edge (A == 0, B < 0 in this orientation)
    let inclusive = e.map(|(a, b, _)| a > 0.0 || (a == 0.0 && b < 0.0));

    let col = |v: &egui::epaint::Vertex| {
        let c = v.color.to_array();
        [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]
    };
    let (ca, cb, cc) = (col(va), col(vb), col(vc));
    let flat_color = ca == cb && cb == cc;
    let flat_uv = va.uv == vb.uv && vb.uv == vc.uv;
    let inv = 1.0 / area2;

    // a constant colour: worked out once for the whole triangle
    let solid = if flat_uv && flat_color {
        let t = tex.sample(va.uv.x, va.uv.y);
        let k = |i: usize| ((t[i] * ca[i]) / 255.0 + 0.5) as u32;
        Some((k(0), k(1), k(2), k(3)))
    } else {
        None
    };
    if let Some((_, _, _, 0)) = solid {
        return;
    }

    for y in y0..y1 {
        let py = y as f32 + 0.5;
        // the run of pixel centres inside on this row, from each edge's own bound
        let mut lo = x0 as f32;
        let mut hi = x1 as f32; // exclusive
        let mut empty = false;
        for (i, &(ea, eb, ec)) in e.iter().enumerate() {
            let at_row = eb * py + ec; // e = ea * px + at_row
            if ea == 0.0 {
                if at_row < 0.0 || (at_row == 0.0 && !inclusive[i]) {
                    empty = true;
                    break;
                }
                continue;
            }
            // px where the edge crosses this row; centres px = x + 0.5
            let cross = -at_row / ea;
            if ea > 0.0 {
                // inside for px >= cross (or > for a non-inclusive edge)
                let first = if inclusive[i] { (cross - 0.5).ceil() } else { (cross - 0.5).floor() + 1.0 };
                lo = lo.max(first);
            } else {
                // inside for px <= cross (or <)
                let last = if inclusive[i] { (cross - 0.5).floor() + 1.0 } else { (cross - 0.5).ceil() };
                hi = hi.min(last);
            }
        }
        if empty || lo >= hi {
            continue;
        }
        let (xs, xe) = (lo as i32, hi as i32);
        let row = &mut slice[((y - offset_row) as usize) * w..][..w];
        if let Some((r, g, b, al)) = solid {
            for d in &mut row[xs as usize..xe as usize] {
                *d = blend(*d, r, g, b, al);
            }
            continue;
        }
        // barycentric weights at the first pixel, and how they change per pixel
        let px0 = xs as f32 + 0.5;
        let w0 = (e[0].0 * px0 + e[0].1 * py + e[0].2) * inv;
        let w1 = (e[1].0 * px0 + e[1].1 * py + e[1].2) * inv;
        let (dw0, dw1) = (e[0].0 * inv, e[1].0 * inv);
        let (mut w0, mut w1) = (w0, w1);
        for d in &mut row[xs as usize..xe as usize] {
            let w2 = 1.0 - w0 - w1;
            let vcol = if flat_color { ca } else { [ca[0] * w0 + cb[0] * w1 + cc[0] * w2, ca[1] * w0 + cb[1] * w1 + cc[1] * w2, ca[2] * w0 + cb[2] * w1 + cc[2] * w2, ca[3] * w0 + cb[3] * w1 + cc[3] * w2] };
            let t = if flat_uv {
                tex.sample(va.uv.x, va.uv.y)
            } else {
                let u = va.uv.x * w0 + vb.uv.x * w1 + vc.uv.x * w2;
                let v = va.uv.y * w0 + vb.uv.y * w1 + vc.uv.y * w2;
                tex.sample(u, v)
            };
            let k = |i: usize| ((t[i] * vcol[i]) / 255.0 + 0.5).clamp(0.0, 255.0) as u32;
            let al = k(3);
            if al > 0 {
                *d = blend(*d, k(0), k(1), k(2), al);
            }
            w0 += dw0;
            w1 += dw1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::{Color32, Mesh, Pos2, Rect, TextureId, Vertex, WHITE_UV};

    fn white(r: &mut SoftRenderer) {
        let img = egui::ColorImage::new([1, 1], vec![Color32::WHITE]);
        let mut d = TexturesDelta::default();
        d.set.insert(TextureId::default(), vec![ImageDelta::full(img, egui::TextureOptions::NEAREST)].into());
        r.set_textures(&d);
        d.clear();
    }

    fn rect_mesh(x0: f32, y0: f32, x1: f32, y1: f32, color: Color32) -> ClippedPrimitive {
        let mut m = Mesh::default();
        let v = |x, y| Vertex { pos: Pos2::new(x, y), uv: WHITE_UV, color };
        m.vertices = vec![v(x0, y0), v(x1, y0), v(x0, y1), v(x1, y1)];
        // the two triangles share the diagonal: its pixels must be painted once
        m.indices = vec![0, 1, 2, 2, 1, 3];
        ClippedPrimitive { clip_rect: Rect::EVERYTHING, primitive: Primitive::Mesh(m) }
    }

    #[test]
    fn fills_a_rectangle_exactly_and_once() {
        let mut r = SoftRenderer::new(0);
        white(&mut r);
        let (w, h) = (16, 16);
        let mut px = vec![0u32; w * h];
        // half-transparent white: a pixel painted twice would come out brighter
        let half = Color32::from_white_alpha(128);
        r.render(&[rect_mesh(2.0, 3.0, 10.0, 9.0, half)], 1.0, &mut px, w, h, true);
        for y in 0..h {
            for x in 0..w {
                let inside = (2..10).contains(&x) && (3..9).contains(&y);
                let v = px[y * w + x] & 0xff;
                assert_eq!(v, if inside { 128 } else { 0 }, "pixel {x},{y}");
            }
        }
    }

    #[test]
    fn redraws_only_what_changed() {
        let mut r = SoftRenderer::new(0);
        white(&mut r);
        let (w, h) = (100, 100);
        let mut px = vec![0u32; w * h];
        let a = rect_mesh(0.0, 0.0, 50.0, 50.0, Color32::RED);
        let b = rect_mesh(60.0, 60.0, 70.0, 70.0, Color32::GREEN);
        let full = r.render(&[a.clone(), b.clone()], 1.0, &mut px, w, h, false);
        assert_eq!(full, PxRect { x0: 0, y0: 0, x1: 100, y1: 100 });
        // nothing changed: nothing is drawn
        assert!(r.render(&[a.clone(), b.clone()], 1.0, &mut px, w, h, false).is_empty());
        // the small square moved: only around it
        let b2 = rect_mesh(62.0, 60.0, 72.0, 70.0, Color32::GREEN);
        let d = r.render(&[a, b2], 1.0, &mut px, w, h, false);
        assert!(d.x0 >= 58 && d.y0 >= 58 && d.x1 <= 74 && d.y1 <= 72, "{d:?}");
        assert_eq!(px[65 * w + 71] & 0xff00, 0xff00);
        assert_eq!(px[65 * w + 61] & 0xff00, 0);
    }

    #[test]
    fn blends_premultiplied() {
        assert_eq!(blend(0x000000, 255, 0, 0, 255), 0xff0000);
        assert_eq!(blend(0xffffff, 0, 0, 0, 0), 0xffffff);
        assert_eq!(blend(0x0000ff, 64, 0, 0, 128) & 0xff, 127);
    }
}
