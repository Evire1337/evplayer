//! How the player looks: the colours of src/styles/base.css, Windows' own fonts (Segoe UI for
//! text, Segoe MDL2 Assets for the icons), sizes and roundings like the page's.
//!
//! The fonts are not copied into the program: the system's font files are mapped into memory,
//! so they cost the program no private memory at all (the system already holds them).

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};

pub const INK: Color32 = Color32::from_rgb(0xee, 0xf7, 0xf4);
pub const ACCENT: Color32 = Color32::from_rgb(0x6f, 0xf0, 0xc4);
pub const GROUND: Color32 = Color32::from_rgb(0x06, 0x20, 0x1d);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x9b, 0x9b);

pub fn ink2() -> Color32 {
    Color32::from_rgba_unmultiplied(238, 247, 244, 174)
}
pub fn ink3() -> Color32 {
    Color32::from_rgba_unmultiplied(238, 247, 244, 112)
}
pub fn glass() -> Color32 {
    Color32::from_rgba_unmultiplied(5, 16, 18, 92)
}
pub fn glass_strong() -> Color32 {
    Color32::from_rgba_unmultiplied(5, 16, 18, 158)
}
pub fn edge() -> Color32 {
    Color32::from_rgba_unmultiplied(238, 247, 244, 31)
}
pub fn hover() -> Color32 {
    Color32::from_rgba_unmultiplied(238, 247, 244, 20)
}
pub fn active() -> Color32 {
    Color32::from_rgba_unmultiplied(238, 247, 244, 41)
}

pub const R_CARD: u8 = 16;
pub const R_PANEL: u8 = 18;

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn icons_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, semibold())
}
pub fn heavy(size: f32) -> FontId {
    FontId::new(size, bold())
}
pub fn icon(size: f32) -> FontId {
    FontId::new(size, icons_family())
}

fn map_font(name: &str) -> Option<Arc<FontData>> {
    let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
    let file = std::fs::File::open(dir.join(name)).ok()?;
    // SAFETY: a system font file, not written while the program runs
    let map = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    Some(Arc::new(FontData::from_blob(Arc::new(map), 0)))
}

/// the fonts: Windows' own, mapped from its font folder
pub fn fonts() -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    let mut add = |key: &str, file: &str| {
        if let Some(d) = map_font(file) {
            defs.font_data.insert(key.to_string(), d);
            true
        } else {
            false
        }
    };
    let regular = add("segoe", "segoeui.ttf");
    let semi = add("segoe-semibold", "seguisb.ttf");
    let heavy = add("segoe-bold", "segoeuib.ttf");
    // Windows 11 has the newer icon font; Windows 10 the MDL2 one (the same codes)
    let icons = add("icons", "SegoeIcons.ttf") || add("icons", "segmdl2.ttf");
    // ♥ ★ ⏯ and other signs in titles, then Chinese, Japanese and Korean titles: mapped, they
    // cost no memory until a title needs them
    let symbols = add("symbols", "seguisym.ttf");
    let cjk = [("zh", "msyh.ttc"), ("ja", "YuGothM.ttc"), ("ko", "malgun.ttf"), ("emoji", "seguiemj.ttf")].into_iter().filter(|(k, f)| add(k, f)).map(|(k, _)| k.to_string()).collect::<Vec<_>>();
    let base: Vec<String> = [regular.then_some("segoe"), symbols.then_some("symbols")].into_iter().flatten().map(str::to_string).chain(cjk).collect();
    let with = |first: Option<&str>| -> Vec<String> { first.map(str::to_string).into_iter().chain(base.iter().cloned()).collect() };
    defs.families.insert(FontFamily::Proportional, with(None));
    defs.families.insert(FontFamily::Monospace, with(None));
    defs.families.insert(semibold(), with(semi.then_some("segoe-semibold")));
    defs.families.insert(bold(), with(heavy.then_some("segoe-bold")));
    defs.families.insert(icons_family(), with(icons.then_some("icons")));
    defs
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    let mut v = Visuals::dark();
    v.override_text_color = Some(INK);
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = Color32::from_rgb(0x0b, 0x2a, 0x27);
    v.window_stroke = Stroke::new(1.0, edge());
    v.window_corner_radius = CornerRadius::same(R_PANEL);
    v.menu_corner_radius = CornerRadius::same(14);
    v.extreme_bg_color = glass_strong();
    v.faint_bg_color = glass();
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(111, 240, 196, 70);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(12);
        w.fg_stroke = Stroke::new(1.0, INK);
    }
    v.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.bg_fill = hover();
    v.widgets.inactive.weak_bg_fill = hover();
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = active();
    v.widgets.hovered.weak_bg_fill = active();
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.bg_fill = active();
    v.widgets.active.weak_bg_fill = active();
    // glyphs on whole pixels and hinted: sub-pixel binning makes text visibly blurrier
    v.text_options.subpixel_binning = false;
    v.text_options.font_hinting = true;
    v.popup_shadow = egui::Shadow::NONE;
    v.window_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(90) };
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.text_styles.insert(egui::TextStyle::Body, body(14.0));
        s.text_styles.insert(egui::TextStyle::Button, medium(13.0));
        s.text_styles.insert(egui::TextStyle::Small, body(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, heavy(40.0));
        s.text_styles.insert(egui::TextStyle::Monospace, body(13.0));
        // no animation that would keep the window drawing on its own
        s.animation_time = 0.08;
        // tooltips waiting for the pointer to stand still redraw the window every frame
        // while it moves: they show after a short delay instead
        s.interaction.show_tooltips_only_when_still = false;
        s.interaction.tooltip_delay = 0.5;
    });
}

/// icon codes of Segoe MDL2 Assets / Segoe Fluent Icons
pub mod ic {
    pub const PLAY: &str = "\u{E768}";
    pub const PAUSE: &str = "\u{E769}";
    pub const NEXT: &str = "\u{E893}";
    pub const PREV: &str = "\u{E892}";
    pub const SHUFFLE: &str = "\u{E8B1}";
    pub const REPEAT: &str = "\u{E8EE}";
    pub const REPEAT_ONE: &str = "\u{E8ED}";
    pub const VOLUME: &str = "\u{E767}";
    pub const MUTE: &str = "\u{E74F}";
    pub const HEART: &str = "\u{EB51}";
    pub const HEART_FILL: &str = "\u{EB52}";
    pub const HOME: &str = "\u{E80F}";
    pub const SEARCH: &str = "\u{E721}";
    pub const LIBRARY: &str = "\u{E8F1}";
    pub const FOLDER: &str = "\u{ED25}";
    pub const SETTINGS: &str = "\u{E713}";
    pub const PEOPLE: &str = "\u{E716}";
    pub const EQ: &str = "\u{E9E9}";
    pub const BACK: &str = "\u{E72B}";
    pub const CLOSE: &str = "\u{E8BB}";
    pub const MINIMIZE: &str = "\u{E921}";
    pub const MAXIMIZE: &str = "\u{E922}";
    pub const RESTORE: &str = "\u{E923}";
    pub const MORE: &str = "\u{E712}";
    pub const ADD: &str = "\u{E710}";
    pub const DOWNLOAD: &str = "\u{E896}";
    pub const DELETE: &str = "\u{E74D}";
    pub const NOTE: &str = "\u{E8D6}";
    pub const DISC: &str = "\u{E958}";
    pub const FULLSCREEN: &str = "\u{E740}";
    pub const LIST: &str = "\u{E8FD}";
    pub const GRID: &str = "\u{F0E2}";
    pub const LINK: &str = "\u{E71B}";
    pub const RADIO: &str = "\u{E93E}";
    pub const LYRICS: &str = "\u{E8D2}";
    pub const EDIT: &str = "\u{E70F}";
    pub const CHECK: &str = "\u{E73E}";
    pub const CANCEL: &str = "\u{E711}";
    pub const CLOCK: &str = "\u{E823}";
    pub const SORT: &str = "\u{E8CB}";
    pub const QUEUE: &str = "\u{E90B}";
    pub const SYNC: &str = "\u{E895}";
    pub const INFO: &str = "\u{E946}";
    pub const OPEN: &str = "\u{E8A7}";
    pub const MINI: &str = "\u{E944}";
    pub const KEYBOARD: &str = "\u{E765}";
    pub const DISCORD: &str = "\u{E8BD}";
    pub const STATS: &str = "\u{E9D2}";
    pub const SPARK: &str = "\u{E945}";
    pub const UP: &str = "\u{E70E}";
    pub const DOWN: &str = "\u{E70D}";
    pub const WARNING: &str = "\u{E7BA}";
}
