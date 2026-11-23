//! `evplayer-native --snapshot out.png [--size 1280x800] [--scale 1.25] [--page name]`:
//! lays the interface out and draws it into a picture with no window at all (ui/soft.rs does
//! the drawing, the same pixels the video card makes up to rounding). For checking how a page
//! looks without opening anything on the screen, and for the layout tests.

use std::path::Path;

use super::soft::SoftRenderer;
use crate::app::App;

pub struct Options {
    pub out: String,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub page: Option<String>,
}

pub fn parse(args: &[String]) -> Option<Options> {
    let i = args.iter().position(|a| a == "--snapshot")?;
    let out = args.get(i + 1)?.clone();
    let mut o = Options { out, width: 1280, height: 800, scale: 1.0, page: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--size" => {
                if let Some((w, h)) = it.next().and_then(|s| s.split_once('x')) {
                    o.width = w.parse().unwrap_or(o.width);
                    o.height = h.parse().unwrap_or(o.height);
                }
            }
            "--scale" => o.scale = it.next().and_then(|s| s.parse().ok()).unwrap_or(1.0),
            "--page" => o.page = it.next().cloned(),
            _ => {}
        }
    }
    Some(o)
}

/// `--glyphs out.png E8B7 E838 …`: the icon font's glyphs with their codes, to pick icons by sight
pub fn glyph_sheet(out: &str, codes: &[String]) -> Result<(), String> {
    let ctx = egui::Context::default();
    super::theme::apply(&ctx);
    let (w, h) = (900u32, 60 + 70 * (codes.len() as u32).div_ceil(8));
    let mut renderer = SoftRenderer::new(0x0b2a27);
    let mut px = vec![0u32; (w * h) as usize];
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32))), ..Default::default() };
        let outp = ctx.run_ui(input, |ui| {
            for (i, c) in codes.iter().enumerate() {
                let ch = u32::from_str_radix(c, 16).ok().and_then(char::from_u32).unwrap_or('?');
                let pos = egui::pos2(20.0 + (i % 8) as f32 * 110.0, 20.0 + (i / 8) as f32 * 70.0);
                ui.painter().text(pos, egui::Align2::LEFT_TOP, ch.to_string(), super::theme::icon(28.0), egui::Color32::WHITE);
                ui.painter().text(pos + egui::vec2(0.0, 36.0), egui::Align2::LEFT_TOP, c, super::theme::body(12.0), egui::Color32::LIGHT_GRAY);
            }
        });
        let prims = ctx.tessellate(outp.shapes, 1.0);
        renderer.set_textures(&outp.textures_delta);
        renderer.render(&prims, 1.0, &mut px, w as usize, h as usize, true);
        let mut d = outp.textures_delta;
        d.clear();
    }
    let rgb: Vec<u8> = px.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    image::save_buffer(Path::new(out), &rgb, w, h, image::ColorType::Rgb8).map_err(|e| e.to_string())
}

pub fn run(mut app: App, ctx: egui::Context, mut o: Options) -> Result<(), String> {
    // `--english`: the picture in English, whatever the settings say
    if std::env::args().any(|a| a == "--english") {
        crate::i18n::set_english(true);
    }
    // the mini player's bar alone, at its own size
    let mini = o.page.as_deref() == Some("mini");
    let mut cover = None;
    if mini {
        o.width = (super::mini::WIDTH as f32 * o.scale).round() as u32;
        o.height = (super::mini::HEIGHT as f32 * o.scale).round() as u32;
    }
    if let Some(page) = o.page.as_ref().filter(|_| !mini) {
        app.open_page_by_name(page);
    }
    let points = egui::vec2(o.width as f32 / o.scale, o.height as f32 / o.scale);
    let mut renderer = SoftRenderer::new(((super::window::GROUND[0] as u32) << 16) | ((super::window::GROUND[1] as u32) << 8) | super::window::GROUND[2] as u32);
    let mut px = vec![0u32; (o.width * o.height) as usize];
    // a few passes: egui settles sizes over the first frames, pictures arrive
    for pass in 0..6 {
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, points)), ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(o.scale);
        input.time = Some(pass as f64 * 0.1);
        let out = if mini { ctx.run_ui(input, |ui| super::mini::snapshot_bar(ui, &app, &mut cover)) } else { ctx.run_ui(input, |ui| app.ui(ui)) };
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        renderer.set_textures(&out.textures_delta);
        renderer.render(&prims, out.pixels_per_point, &mut px, o.width as usize, o.height as usize, true);
        renderer.free_textures(&out.textures_delta);
        let mut d = out.textures_delta;
        d.clear();
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    let rgb: Vec<u8> = px.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    image::save_buffer(Path::new(&o.out), &rgb, o.width, o.height, image::ColorType::Rgb8).map_err(|e| e.to_string())
}
