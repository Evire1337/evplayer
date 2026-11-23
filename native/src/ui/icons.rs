//! The Tauri build's icons and logo on screen. Each is drawn by ui/vector.rs once for the exact
//! pixel size it is shown at (so a 21 px icon at 150 % is a sharp 32 px picture, not a scaled
//! one), kept as a tiny white texture and tinted when painted. The pictures live with the
//! interface's context, so a window that let its context go lets them go too.

use std::collections::HashMap;

use egui::{Color32, Rect, TextureHandle};

use super::icon_data::{ICONS, LOGO, LOGO_VIEW};

/// the window's own buttons (WindowControls.tsx): 10×10, 1 px lines
const WINDOW: &[(&str, &str)] = &[
    ("winMin", r#"<path d="M0 5h10" stroke="currentColor" />"#),
    ("winMax", r#"<rect x=".5" y=".5" width="9" height="9" fill="none" stroke="currentColor" />"#),
    ("winRestore", r#"<path d="M2.5 .5h7v7M.5 2.5h7v7h-7z" fill="none" stroke="currentColor" />"#),
    ("winClose", r#"<path d="M0 0l10 10M10 0L0 10" stroke="currentColor" />"#),
];

#[derive(Clone, Default)]
struct Cache(HashMap<(String, u32, u32), TextureHandle>);

fn draw(name: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    if name == "logo" {
        let shapes = super::vector::shapes(LOGO, false, 0.0);
        return Some(super::vector::rasterize(&shapes, LOGO_VIEW, w as usize, h as usize));
    }
    if let Some((_, m)) = WINDOW.iter().find(|(n, _)| *n == name) {
        return Some(super::vector::rasterize(&super::vector::shapes(m, true, 1.0), (10.0, 10.0), w as usize, h as usize));
    }
    let (_, m) = ICONS.iter().find(|(n, _)| *n == name)?;
    Some(super::vector::rasterize(&super::vector::shapes(m, true, 1.9), (24.0, 24.0), w as usize, h as usize))
}

/// `rect` on the pixel grid, so the picture is not resampled
fn snap(rect: Rect, ppp: f32) -> (Rect, u32, u32) {
    let x = (rect.min.x * ppp).round();
    let y = (rect.min.y * ppp).round();
    let w = (rect.width() * ppp).round().max(1.0);
    let h = (rect.height() * ppp).round().max(1.0);
    (Rect::from_min_size(egui::pos2(x / ppp, y / ppp), egui::vec2(w / ppp, h / ppp)), w as u32, h as u32)
}

/// paint the icon `name` (an Icons.tsx name like "heartFill", "logo", or a window button)
/// filling `rect`, in `color`
pub fn paint(painter: &egui::Painter, name: &str, rect: Rect, color: Color32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let (rect, w, h) = snap(rect, ppp);
    let key = (name.to_string(), w, h);
    let id = egui::Id::unique("evplayer-icons");
    let found = ctx.data(|d| d.get_temp::<Cache>(id).and_then(|c| c.0.get(&key).map(|t| t.id())));
    let tex = match found {
        Some(t) => t,
        None => {
            let Some(px) = draw(name, w, h) else { return };
            let img = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &px);
            let handle = ctx.load_texture(format!("icon-{name}-{w}x{h}"), img, egui::TextureOptions::NEAREST);
            let t = handle.id();
            ctx.data_mut(|d| {
                d.get_temp_mut_or_default::<Cache>(id).0.insert(key, handle);
            });
            t
        }
    };
    painter.image(tex, rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), color);
}

/// does an icon of that name exist
pub fn exists(name: &str) -> bool {
    name == "logo" || WINDOW.iter().any(|(n, _)| *n == name) || ICONS.iter().any(|(n, _)| *n == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_of_the_page_draws() {
        for (name, _) in ICONS.iter().chain(WINDOW) {
            let px = draw(name, 32, 32).unwrap();
            let ink = px.chunks(4).filter(|p| p[3] > 0).count();
            assert!(ink > 8, "{name} drew nothing");
            assert!(ink < 32 * 32, "{name} filled everything");
        }
        let logo = draw("logo", 64, 48).unwrap();
        let ink = logo.chunks(4).filter(|p| p[3] > 128).count();
        assert!(ink > 300 && ink < 64 * 48 / 2, "logo: {ink}");
        assert!(exists("heartFill") && !exists("nope"));
    }

    /// `EVPLAYER_ICON_SHEET=out.png cargo test icon_sheet -- --ignored`: every icon on one picture
    #[test]
    #[ignore]
    fn icon_sheet() {
        let Some(out) = std::env::var_os("EVPLAYER_ICON_SHEET") else { return };
        let names: Vec<&str> = ICONS.iter().map(|i| i.0).chain(WINDOW.iter().map(|i| i.0)).collect();
        let (cell, cols) = (56u32, 11u32);
        let rows = (names.len() as u32).div_ceil(cols) + 1;
        let mut img = image::RgbaImage::from_pixel(cols * cell, rows * cell, image::Rgba([12, 40, 36, 255]));
        let mut put = |px: &[u8], w: u32, h: u32, x0: u32, y0: u32| {
            for y in 0..h {
                for x in 0..w {
                    let a = px[((y * w + x) * 4 + 3) as usize] as u32;
                    let p = img.get_pixel_mut(x0 + x, y0 + y);
                    for c in 0..3 {
                        p.0[c] = ((p.0[c] as u32 * (255 - a) + 238 * a) / 255) as u8;
                    }
                }
            }
        };
        for (i, n) in names.iter().enumerate() {
            let (x, y) = ((i as u32 % cols) * cell + 12, (i as u32 / cols) * cell + 12);
            let s = if n.starts_with("win") { 10 } else { 32 };
            put(&draw(n, s, s).unwrap(), s, s, x, y);
        }
        let logo = draw("logo", 96, 48).unwrap();
        put(&logo, 96, 48, 8, (rows - 1) * cell + 4);
        img.save(out).unwrap();
    }
}
