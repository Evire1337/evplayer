//! The pictures of lists, as the page draws them (src/library/Cover.tsx, src/stats/SmartCover.tsx
//! and .pl-cover, .is-liked, .files-cover, .smart-cover, .smart-badge in app.css): rounded 24,
//! "Понравившиеся" and "Мои файлы" on their own gradients, a playlist as its own picture or a
//! mosaic of its first four covers (one cover while it has fewer), a smart list the same with a
//! sparkle badge in its colour. Gradients follow CSS linear-gradient: the angle, the length of
//! the gradient line and the stops' places.

use egui::{pos2, vec2, Color32, CornerRadius, Pos2, Rect, Shape, Ui};

use crate::model::Track;
use crate::ui::images::{artwork_of, Images};

/// .pl-cover's rounding
pub const RADIUS: f32 = 24.0;

pub enum Cover<'a> {
    Liked,
    Files,
    /// a smart list: its hue and its songs
    Smart { hue: f32, tracks: &'a [Track] },
    /// a playlist: its own picture, else its songs' covers
    Playlist { icon: Option<&'a str>, tracks: &'a [Track] },
}

/// the first four different covers of a list (the page dedups them too)
pub fn arts_of(tracks: &[Track]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(4);
    for t in tracks {
        if let Some(a) = artwork_of(t) {
            if !out.contains(&a) {
                out.push(a);
                if out.len() == 4 {
                    break;
                }
            }
        }
    }
    out
}

/// a CSS colour with alpha
pub fn rgba(hex: u32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, (a * 255.0).round() as u8)
}

/// hsl() as CSS reads it
pub fn hsl(h: f32, s: f32, l: f32, a: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(to(r), to(g), to(b), (a * 255.0).round() as u8)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let (a, b) = (a.to_srgba_unmultiplied(), b.to_srgba_unmultiplied());
    let m = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(0), m(1), m(2), m(3))
}

/// the colour of a CSS linear-gradient at `t` along its line, stops as (colour, place 0..1)
fn at(stops: &[(Color32, f32)], t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    if t <= stops[0].1 {
        return stops[0].0;
    }
    for w in stops.windows(2) {
        if t <= w[1].1 {
            return mix(w[0].0, w[1].0, (t - w[0].1) / (w[1].1 - w[0].1).max(1e-6));
        }
    }
    stops[stops.len() - 1].0
}

/// `linear-gradient(<angle>deg, stops…)` filling a rect rounded `r`, as one mesh: rings from
/// the middle out to the rounded edge, so a stop inside the shape bends where it should
pub fn linear_gradient(rect: Rect, r: f32, angle_deg: f32, stops: &[(Color32, f32)]) -> Shape {
    let a = angle_deg.to_radians();
    // CSS: 0deg points up, 90deg right; the line is as long as the box seen along it
    let dir = vec2(a.sin(), -a.cos());
    let len = (rect.width() * a.sin()).abs() + (rect.height() * a.cos()).abs();
    let c = rect.center();
    let color = |p: Pos2| at(stops, (p - c).dot(dir) / len + 0.5);
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
    // the outline: four quarter circles, 9 points each
    let corners = [(rect.right_top() + vec2(-r, r), -90.0f32), (rect.right_bottom() + vec2(-r, -r), 0.0), (rect.left_bottom() + vec2(r, -r), 90.0), (rect.left_top() + vec2(r, r), 180.0)];
    let mut edge: Vec<Pos2> = Vec::with_capacity(36);
    for (o, start) in corners {
        for k in 0..=8 {
            let ang = (start + 90.0 * k as f32 / 8.0).to_radians();
            edge.push(o + vec2(r * ang.cos(), r * ang.sin()));
        }
    }
    const RINGS: usize = 4;
    let n = edge.len();
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color(c));
    for ring in 1..=RINGS {
        let f = ring as f32 / RINGS as f32;
        for e in &edge {
            let p = c + (*e - c) * f;
            mesh.colored_vertex(p, color(p));
        }
    }
    let idx = |ring: usize, i: usize| (1 + (ring - 1) * n + i % n) as u32;
    for i in 0..n {
        mesh.add_triangle(0, idx(1, i), idx(1, i + 1));
    }
    for ring in 1..RINGS {
        for i in 0..n {
            let (a0, a1, b0, b1) = (idx(ring, i), idx(ring, i + 1), idx(ring + 1, i), idx(ring + 1, i + 1));
            mesh.add_triangle(a0, b0, b1);
            mesh.add_triangle(a0, b1, a1);
        }
    }
    Shape::mesh(mesh)
}

/// one picture filling `rect` with its own corners rounded
fn picture(ui: &Ui, images: &mut Images, src: &str, rect: Rect, corners: CornerRadius) {
    if let Some(id) = images.get(src, rect.width(), ui.ctx().pixels_per_point()) {
        egui::Image::new(egui::load::SizedTexture::new(id, rect.size())).corner_radius(corners).paint_at(ui, rect);
    }
}

/// a single picture, or four in a 2×2 grid with 2 px between (.pl-cover.is-grid)
fn mosaic(ui: &Ui, images: &mut Images, arts: &[String], rect: Rect, r: f32) {
    let ru = r.round() as u8;
    ui.painter().rect_filled(rect, ru, rgba(0xeef7f4, 0.08));
    if arts.len() < 4 {
        if let Some(a) = arts.first() {
            picture(ui, images, a, rect, CornerRadius::same(ru));
        }
        return;
    }
    let half = (rect.width() - 2.0) / 2.0;
    for (i, a) in arts.iter().take(4).enumerate() {
        let (col, row) = ((i % 2) as f32, (i / 2) as f32);
        let q = Rect::from_min_size(rect.min + vec2(col * (half + 2.0), row * (half + 2.0)), vec2(half, half));
        let corners = CornerRadius { nw: if i == 0 { ru } else { 0 }, ne: if i == 1 { ru } else { 0 }, sw: if i == 2 { ru } else { 0 }, se: if i == 3 { ru } else { 0 } };
        picture(ui, images, a, q, corners);
    }
}

/// Draws a list's picture in `rect`. `small` is the list view's 52 px cover (rounded 12, its
/// signs 22 px, the badge 18 at the corner).
pub fn paint(ui: &Ui, images: &mut Images, cover: &Cover, rect: Rect, small: bool) {
    let r = if small { 12.0 } else { RADIUS.min(rect.width() / 2.0) };
    let p = ui.painter();
    let sign = |frac: f32, fixed: f32| if small { 22.0 } else if fixed > 0.0 { fixed } else { rect.width() * frac };
    match cover {
        Cover::Liked => {
            // linear-gradient(145deg, #6ff0c4, #1f6a8a 70%), the heart 42% in #06201d
            p.add(linear_gradient(rect, r, 145.0, &[(rgba(0x6ff0c4, 1.0), 0.0), (rgba(0x1f6a8a, 1.0), 0.7)]));
            let s = sign(0.42, 0.0);
            crate::ui::icons::paint(p, "heartFill", Rect::from_center_size(rect.center(), vec2(s, s)), rgba(0x06201d, 1.0));
        }
        Cover::Files => {
            // linear-gradient(150deg, #8ab4ff, #23306b 75%), a white folder 48 (64 on the page)
            p.add(linear_gradient(rect, r, 150.0, &[(rgba(0x8ab4ff, 1.0), 0.0), (rgba(0x23306b, 1.0), 0.75)]));
            let s = sign(0.0, if rect.width() >= 200.0 { 64.0 } else { 48.0 });
            crate::ui::icons::paint(p, "folder", Rect::from_center_size(rect.center(), vec2(s, s)), Color32::WHITE);
        }
        Cover::Smart { hue, tracks } => {
            let arts = arts_of(tracks);
            if arts.is_empty() {
                // .smart-cover: linear-gradient(150deg, hsl(h 80% 62%), hsl(h 70% 18%)), a white sparkle 44
                p.add(linear_gradient(rect, r, 150.0, &[(hsl(*hue, 0.8, 0.62, 1.0), 0.0), (hsl(*hue, 0.7, 0.18, 1.0), 1.0)]));
                let s = sign(0.0, 44.0);
                crate::ui::icons::paint(p, "sparkle", Rect::from_center_size(rect.center(), vec2(s, s)), Color32::WHITE);
                return;
            }
            mosaic(ui, images, &arts, rect, r);
            // .smart-badge: 26 at 8 from the corner (18 at 2 in the list), its own gradient
            let (d, inset, icon) = if small { (18.0, 2.0, 10.0) } else { (26.0, 8.0, 14.0) };
            let badge = Rect::from_min_size(pos2(rect.right() - inset - d, rect.bottom() - inset - d), vec2(d, d));
            p.circle_filled(badge.center() + vec2(0.0, 4.0), d / 2.0 + 2.0, Color32::from_black_alpha(40));
            p.add(linear_gradient(badge, d / 2.0, 145.0, &[(hsl(*hue, 0.8, 0.6, 1.0), 0.0), (hsl(*hue, 0.7, 0.3, 1.0), 1.0)]));
            crate::ui::icons::paint(p, "sparkle", Rect::from_center_size(badge.center(), vec2(icon, icon)), Color32::WHITE);
        }
        Cover::Playlist { icon, tracks } => {
            if let Some(i) = icon.filter(|i| !i.is_empty()) {
                p.rect_filled(rect, r.round() as u8, rgba(0xeef7f4, 0.08));
                picture(ui, images, i, rect, CornerRadius::same(r.round() as u8));
                return;
            }
            let arts = arts_of(tracks);
            if arts.is_empty() {
                // .is-empty: the note, 36%, in the third ink
                p.rect_filled(rect, r.round() as u8, rgba(0xeef7f4, 0.08));
                let s = sign(0.36, 0.0);
                crate::ui::icons::paint(p, "music", Rect::from_center_size(rect.center(), vec2(s, s)), crate::ui::theme::ink3());
                return;
            }
            mosaic(ui, images, &arts, rect, r);
        }
    }
}

/// .pl-card: the picture w×w, 8 + 4 below it the name (600) and a line (12.5 px, third ink);
/// a little brighter under the pointer
pub fn card(ui: &mut Ui, images: &mut Images, cover: &Cover, w: f32, title: &str, sub: &str) -> egui::Response {
    use crate::ui::{theme, widgets};
    let (rect, r) = ui.allocate_exact_size(vec2(w, w + 12.0 + 19.0 + 4.0 + 17.0), egui::Sense::click());
    let art = Rect::from_min_size(rect.min, vec2(w, w));
    paint(ui, images, cover, art, false);
    if r.hovered() {
        ui.painter().rect_filled(art, RADIUS as u8, Color32::from_white_alpha(10));
    }
    let g = widgets::clipped(ui, title, theme::medium(14.0), theme::INK, w);
    ui.painter().galley(pos2(rect.left(), art.bottom() + 12.0), g, theme::INK);
    let g = widgets::clipped(ui, sub, theme::body(12.5), theme::ink3(), w);
    ui.painter().galley(pos2(rect.left(), art.bottom() + 12.0 + 19.0 + 4.0), g, theme::ink3());
    r
}

/// .pl-list .pl-card: a 64 px row (padding 6 10 6 6), the 52 px picture, 14 to the name over
/// its line; rounded 14 under the pointer
pub fn row(ui: &mut Ui, images: &mut Images, cover: &Cover, title: &str, sub: &str) -> egui::Response {
    use crate::ui::{theme, widgets};
    let w = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(vec2(w, 64.0), egui::Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(rect, 14, theme::hover());
    }
    let art = Rect::from_min_size(pos2(rect.left() + 6.0, rect.top() + 6.0), vec2(52.0, 52.0));
    paint(ui, images, cover, art, true);
    let x = art.right() + 14.0;
    let tw = (rect.right() - 10.0 - x).max(20.0);
    let g = widgets::clipped(ui, title, theme::medium(14.0), theme::INK, tw);
    ui.painter().galley(pos2(x, rect.center().y - g.size().y), g, theme::INK);
    let g = widgets::clipped(ui, sub, theme::body(12.5), theme::ink3(), tw);
    ui.painter().galley(pos2(x, rect.center().y + 1.0), g, theme::ink3());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_hold_their_colour_past_their_place() {
        let stops = [(Color32::BLACK, 0.0), (Color32::WHITE, 0.5)];
        assert_eq!(at(&stops, 0.25), Color32::from_gray(128));
        assert_eq!(at(&stops, 0.9), Color32::WHITE);
    }

    #[test]
    fn hsl_as_css() {
        assert_eq!(hsl(0.0, 1.0, 0.5, 1.0), Color32::from_rgb(255, 0, 0));
        assert_eq!(hsl(120.0, 1.0, 0.25, 1.0), Color32::from_rgb(0, 128, 0));
    }
}
