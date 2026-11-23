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

pub fn run(mut app: App, ctx: egui::Context, o: Options) -> Result<(), String> {
    if let Some(page) = &o.page {
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
        let out = ctx.run_ui(input, |ui| app.ui(ui));
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
