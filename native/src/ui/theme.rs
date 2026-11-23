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

/// An icon where a glyph of `size` would stand: one of the page's pictures (a box 1.4 times the
/// glyph's size, as the page's 21 px icons stand for 15 px glyphs) or a Segoe glyph.
pub fn paint_icon(p: &egui::Painter, pos: egui::Pos2, align: egui::Align2, glyph: &str, size: f32, color: Color32) {
    match glyph.strip_prefix('@') {
        Some(name) => {
            let b = if name.starts_with("win") { size } else { size * 1.4 };
            let rect = align.anchor_size(pos, egui::vec2(b, b));
            super::icons::paint(p, name, rect, color);
        }
        None => {
            p.text(pos, align, glyph, icon(size), color);
        }
    }
}

fn map_font(name: &str) -> Option<Arc<FontData>> {
    let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
    let file = std::fs::File::open(dir.join(name)).ok()?;
    // SAFETY: a system font file, not written while the program runs
    let map = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    Some(Arc::new(FontData::from_blob(Arc::new(map), 0)))
}

/// the fonts: Windows' own, mapped from its font folder
/// Onest, the font of the Tauri build (@fontsource/onest), in the weights the page uses: 400
/// for text, 500 for menus, 600 for names and buttons, 800 for headings. Each weight comes in
/// four parts (Latin, Latin extended, Cyrillic, Cyrillic extended), turned from the package's
/// WOFF files by fonts/woff2ttf.py. They are part of the program file: no private memory.
macro_rules! onest {
    ($w:literal) => {
        [
            include_bytes!(concat!("../../fonts/onest-latin-", $w, "-normal.ttf")).as_slice(),
            include_bytes!(concat!("../../fonts/onest-latin-ext-", $w, "-normal.ttf")).as_slice(),
            include_bytes!(concat!("../../fonts/onest-cyrillic-", $w, "-normal.ttf")).as_slice(),
            include_bytes!(concat!("../../fonts/onest-cyrillic-ext-", $w, "-normal.ttf")).as_slice(),
        ]
    };
}

pub fn medium500() -> FontFamily {
    FontFamily::Name("w500".into())
}

pub fn fonts() -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    // Onest first, each weight its own family
    let weights: [(&str, [&'static [u8]; 4]); 4] = [("400", onest!("400")), ("500", onest!("500")), ("600", onest!("600")), ("800", onest!("800"))];
    let mut onest_keys: std::collections::HashMap<&str, Vec<String>> = Default::default();
    for (w, parts) in weights {
        for (i, bytes) in parts.into_iter().enumerate() {
            let key = format!("onest-{w}-{i}");
            defs.font_data.insert(key.clone(), Arc::new(FontData::from_static(bytes)));
            onest_keys.entry(w).or_default().push(key);
        }
    }
    let mut add = |key: &str, file: &str| {
        if let Some(d) = map_font(file) {
            defs.font_data.insert(key.to_string(), d);
            true
        } else {
            false
        }
    };
    // Windows' own fonts behind it: letters Onest has not (Greek, Arabic, ...), signs, CJK
    let regular = add("segoe", "segoeui.ttf");
    // Windows 11 has the newer icon font; Windows 10 the MDL2 one (the same codes)
    let icons = add("icons", "SegoeIcons.ttf") || add("icons", "segmdl2.ttf");
    // signs in titles, then Chinese, Japanese and Korean titles: mapped, they cost no memory
    // until a title needs them
    let symbols = add("symbols", "seguisym.ttf");
    let cjk = [("zh", "msyh.ttc"), ("ja", "YuGothM.ttc"), ("ko", "malgun.ttf"), ("emoji", "seguiemj.ttf")].into_iter().filter(|(k, f)| add(k, f)).map(|(k, _)| k.to_string()).collect::<Vec<_>>();
    let base: Vec<String> = [regular.then_some("segoe"), symbols.then_some("symbols")].into_iter().flatten().map(str::to_string).chain(cjk).collect();
    let with = |w: &str| -> Vec<String> { onest_keys.get(w).cloned().unwrap_or_default().into_iter().chain(base.iter().cloned()).collect() };
    defs.families.insert(FontFamily::Proportional, with("400"));
    defs.families.insert(FontFamily::Monospace, with("400"));
    defs.families.insert(medium500(), with("500"));
    defs.families.insert(semibold(), with("600"));
    defs.families.insert(bold(), with("800"));
    let mut ic: Vec<String> = icons.then(|| "icons".to_string()).into_iter().collect();
    ic.extend(with("400"));
    defs.families.insert(icons_family(), ic);
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

/// The icons: "@name" is one of the Tauri build's own (src/ui/components/Icons.tsx, drawn by
/// ui/icons.rs); the rest are glyphs of Segoe MDL2 Assets for the few the page has no picture of.
pub mod ic {
    pub const SLIDERS: &str = "@sliders";
    pub const OFFLINE: &str = "@offline";
    pub const PLAY_CIRCLE: &str = "@playCircle";
    pub const PLUG: &str = "@plug";
    pub const DATABASE: &str = "@database";
    pub const REFRESH: &str = "@refresh";
    pub const USER: &str = "@user";
    pub const COPY: &str = "@copy";
    pub const PLAY: &str = "@play";
    pub const PAUSE: &str = "@pause";
    pub const NEXT: &str = "@next";
    pub const PREV: &str = "@prev";
    pub const SHUFFLE: &str = "@shuffle";
    pub const REPEAT: &str = "@repeat";
    pub const REPEAT_ONE: &str = "\u{E8ED}";
    pub const VOLUME: &str = "@volume";
    pub const MUTE: &str = "@mute";
    pub const HEART: &str = "@heart";
    pub const HEART_FILL: &str = "@heartFill";
    pub const HOME: &str = "@home";
    pub const SEARCH: &str = "@search";
    pub const LIBRARY: &str = "@library";
    pub const FOLDER: &str = "@folder";
    pub const SETTINGS: &str = "@settings";
    pub const PEOPLE: &str = "@jam";
    pub const EQ: &str = "@eq";
    pub const BACK: &str = "@back";
    pub const CLOSE: &str = "@close";
    pub const MINIMIZE: &str = "@winMin";
    pub const MAXIMIZE: &str = "@winMax";
    pub const RESTORE: &str = "@winRestore";
    pub const MORE: &str = "@more";
    pub const ADD: &str = "@plus";
    pub const DOWNLOAD: &str = "@download";
    pub const DELETE: &str = "@trash";
    pub const NOTE: &str = "@music";
    pub const DISC: &str = "@disc";
    pub const FULLSCREEN: &str = "@expand";
    pub const LIST: &str = "@listPlus";
    pub const GRID: &str = "@gridView";
    pub const LINK: &str = "@link";
    pub const RADIO: &str = "@radio";
    pub const LYRICS: &str = "@lyrics";
    pub const EDIT: &str = "@pencil";
    pub const CHECK: &str = "@check";
    pub const CANCEL: &str = "@close";
    pub const CLOCK: &str = "\u{E823}";
    pub const SORT: &str = "@sort";
    pub const QUEUE: &str = "@queue";
    pub const SYNC: &str = "@plug";
    pub const INFO: &str = "@info";
    pub const CHART: &str = "@chart";
    pub const OPEN: &str = "@folder";
    pub const MINI: &str = "@pip";
    pub const KEYBOARD: &str = "@keyboard";
    pub const DISCORD: &str = "@chat";
    pub const STATS: &str = "@chart";
    pub const SPARK: &str = "@palette";
    pub const UP: &str = "\u{E70E}";
    pub const DOWN: &str = "\u{E70D}";
    pub const WARNING: &str = "\u{E7BA}";
}
