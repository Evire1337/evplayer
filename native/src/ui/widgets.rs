//! The pieces the pages are made of, drawn the way the page's CSS draws them: round icon
//! buttons, pill tabs, switches, the seek bar, covers with rounded corners, track rows, cards.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui};

use super::images::Images;
use super::theme::{self, ic, ACCENT, INK};

/// a round button with an icon; `on`: lit (shuffle on, a page open)
pub fn icon_button(ui: &mut Ui, icon: &str, size: f32, on: bool, tip: &str) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    paint_icon_button(ui, rect, &r, icon, on);
    if !tip.is_empty() {
        r.on_hover_text(crate::i18n::t(tip))
    } else {
        r
    }
}

pub fn paint_icon_button(ui: &Ui, rect: Rect, r: &Response, icon: &str, on: bool) {
    let p = ui.painter();
    let bg = if r.is_pointer_button_down_on() {
        theme::active()
    } else if r.hovered() {
        theme::hover()
    } else if on {
        Color32::from_rgba_unmultiplied(238, 247, 244, 18)
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        p.circle_filled(rect.center(), rect.width() / 2.0, bg);
    }
    let color = if on { ACCENT } else if r.hovered() { INK } else { theme::ink2() };
    p.text(rect.center(), Align2::CENTER_CENTER, icon, theme::icon(rect.width() * 0.42), color);
}

/// the big round play button
pub fn play_button(ui: &mut Ui, playing: bool, size: f32) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let p = ui.painter();
    let fill = if r.hovered() { Color32::from_rgba_unmultiplied(238, 247, 244, 70) } else { Color32::from_rgba_unmultiplied(238, 247, 244, 46) };
    p.circle_filled(rect.center(), size / 2.0, fill);
    p.text(rect.center(), Align2::CENTER_CENTER, if playing { ic::PAUSE } else { ic::PLAY }, theme::icon(size * 0.36), INK);
    r.on_hover_text(crate::i18n::t(if playing { "Пауза" } else { "Играть" }))
}

/// a pill-shaped button: white for the main action of a page, glass for the others
pub fn pill_button(ui: &mut Ui, icon: Option<&str>, text: &str, primary: bool) -> Response {
    let text = crate::i18n::t(text);
    let font = theme::medium(13.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), INK);
    let icon_w = if icon.is_some() { 20.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + 30.0, 34.0);
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let (fill, fg) = if primary {
        (if r.hovered() { Color32::WHITE } else { INK }, Color32::from_rgb(0x08, 0x26, 0x22))
    } else {
        (if r.hovered() { theme::active() } else { Color32::from_rgba_unmultiplied(238, 247, 244, 26) }, INK)
    };
    p.rect_filled(rect, CornerRadius::same(17), fill);
    let mut x = rect.left() + 15.0;
    if let Some(i) = icon {
        p.text(pos2(x + 7.0, rect.center().y), Align2::CENTER_CENTER, i, theme::icon(13.0), fg);
        x += icon_w;
    }
    p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font, fg);
    r
}

/// a tab in a row of tabs ("Везде", "SoundCloud", ...)
pub fn tab(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let text = crate::i18n::t(text);
    let font = theme::medium(12.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), INK);
    let (rect, r) = ui.allocate_exact_size(vec2(galley.size().x + 22.0, 28.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(14), INK);
    } else if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(14), theme::hover());
    }
    p.text(rect.center(), Align2::CENTER_CENTER, text, font, if selected { Color32::from_rgb(0x08, 0x26, 0x22) } else { theme::ink2() });
    r
}

/// a group of tabs on a glass background; returns the index pressed
pub fn tabs(ui: &mut Ui, items: &[&str], selected: usize) -> Option<usize> {
    let mut pressed = None;
    egui::Frame::new().fill(theme::glass()).corner_radius(CornerRadius::same(17)).inner_margin(3).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, t) in items.iter().enumerate() {
                if tab(ui, t, i == selected).clicked() {
                    pressed = Some(i);
                }
            }
        });
    });
    pressed
}

/// a switch; true when it was flipped
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let p = ui.painter();
    let track = if *on { Color32::from_rgba_unmultiplied(111, 240, 196, 200) } else { Color32::from_rgba_unmultiplied(238, 247, 244, 40) };
    p.rect_filled(rect, CornerRadius::same(10), track);
    let x = if *on { rect.right() - 10.0 } else { rect.left() + 10.0 };
    p.circle_filled(pos2(x, rect.center().y), 7.5, if *on { Color32::from_rgb(0x06, 0x20, 0x1d) } else { INK });
    r
}

/// The seek bar: the part heard, the part fetched, and a knob while the pointer is over it.
/// Returns where it was pressed or dragged to, 0..1.
pub fn seek_bar(ui: &mut Ui, width: f32, frac: f32, buffered: f32) -> Option<f32> {
    let (rect, r) = ui.allocate_exact_size(vec2(width, 14.0), Sense::click_and_drag());
    let line = Rect::from_center_size(rect.center(), vec2(rect.width(), if r.hovered() || r.dragged() { 5.0 } else { 4.0 }));
    let p = ui.painter();
    p.rect_filled(line, CornerRadius::same(3), Color32::from_rgba_unmultiplied(238, 247, 244, 36));
    if buffered > 0.0 && buffered < 1.0 {
        let b = Rect::from_min_max(line.min, pos2(line.left() + line.width() * buffered.clamp(0.0, 1.0), line.max.y));
        p.rect_filled(b, CornerRadius::same(3), Color32::from_rgba_unmultiplied(238, 247, 244, 36));
    }
    let mut f = frac.clamp(0.0, 1.0);
    let at = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()).map(|p| ((p.x - line.left()) / line.width()).clamp(0.0, 1.0));
    if let Some(a) = at {
        f = a;
    }
    let done = Rect::from_min_max(line.min, pos2(line.left() + line.width() * f, line.max.y));
    p.rect_filled(done, CornerRadius::same(3), INK);
    if r.hovered() || r.dragged() {
        p.circle_filled(pos2(done.right(), line.center().y), 6.0, INK);
    }
    // the seek happens when the button is let go (a drag shows where it will land)
    if r.drag_stopped() || r.clicked() {
        return at.or(Some(f));
    }
    None
}

/// a thin slider for the volume and other levels; true when changed
pub fn level(ui: &mut Ui, width: f32, value: &mut f32) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(width, 16.0), Sense::click_and_drag());
    if let Some(p) = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()) {
        let v = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            r.mark_changed();
        }
    }
    let line = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
    let painter = ui.painter();
    painter.rect_filled(line, CornerRadius::same(2), Color32::from_rgba_unmultiplied(238, 247, 244, 36));
    let done = Rect::from_min_max(line.min, pos2(line.left() + line.width() * *value, line.max.y));
    painter.rect_filled(done, CornerRadius::same(2), INK);
    painter.circle_filled(pos2(done.right(), line.center().y), if r.hovered() || r.dragged() { 6.5 } else { 5.0 }, INK);
    r
}

/// a cover with rounded corners, or a soft placeholder with a note while there is none
pub fn cover(ui: &mut Ui, images: &mut Images, src: Option<&str>, size: f32, radius: u8) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    paint_cover(ui, images, src, rect, radius);
    r
}

pub fn paint_cover(ui: &Ui, images: &mut Images, src: Option<&str>, rect: Rect, radius: u8) {
    let ppp = ui.ctx().pixels_per_point();
    let tex = src.and_then(|s| images.get(s, rect.width(), ppp));
    let p = ui.painter();
    match tex {
        Some(id) => {
            egui::Image::new(egui::load::SizedTexture::new(id, rect.size())).corner_radius(CornerRadius::same(radius)).paint_at(ui, rect);
        }
        None => {
            p.rect_filled(rect, CornerRadius::same(radius), Color32::from_rgba_unmultiplied(238, 247, 244, 22));
            p.text(rect.center(), Align2::CENTER_CENTER, ic::NOTE, theme::icon((rect.width() * 0.3).min(40.0)), theme::ink3());
        }
    }
}

/// "3:07", "1:02:03"
pub fn time(ms: f64) -> String {
    if !ms.is_finite() || ms < 0.0 {
        return "0:00".into();
    }
    let s = (ms / 1000.0).floor() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// text cut to fit `width` with an ellipsis
pub fn clipped(ui: &Ui, text: &str, font: egui::FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    ui.painter().layout_job(job)
}

pub struct RowOut {
    pub clicked: bool,
    pub double: bool,
    pub like: bool,
    pub menu: bool,
    pub response: Response,
}

/// a song in a list: cover, title and artist, extra text (the date added), like, length
pub fn track_row(ui: &mut Ui, images: &mut Images, t: &crate::model::Track, playing: bool, liked: bool, extra: Option<&str>, number: Option<usize>) -> RowOut {
    let width = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(vec2(width, 52.0), Sense::click());
    let hovered = r.hovered();
    if hovered || playing {
        ui.painter().rect_filled(rect, CornerRadius::same(12), if hovered { theme::hover() } else { Color32::from_rgba_unmultiplied(238, 247, 244, 12) });
    }
    let mut x = rect.left() + 10.0;
    if let Some(n) = number {
        ui.painter().text(pos2(x + 10.0, rect.center().y), Align2::CENTER_CENTER, n.to_string(), theme::body(12.5), theme::ink3());
        x += 28.0;
    }
    let art = Rect::from_min_size(pos2(x, rect.center().y - 19.0), vec2(38.0, 38.0));
    let src = super::images::artwork_of(t);
    paint_cover(ui, images, src.as_deref(), art, 8);
    x += 50.0;
    let right_w = 150.0 + if extra.is_some() { 130.0 } else { 0.0 };
    let text_w = (rect.right() - x - right_w).max(60.0);
    let title_color = if playing { ACCENT } else { INK };
    let g = clipped(ui, &t.title, theme::medium(13.5), title_color, text_w);
    ui.painter().galley(pos2(x, rect.center().y - 17.0), g, title_color);
    let mut sub = t.artist_line();
    if t.explicit {
        sub = format!("🅴 {sub}");
    }
    let g = clipped(ui, &sub, theme::body(12.5), theme::ink2(), text_w);
    ui.painter().galley(pos2(x, rect.center().y + 1.0), g, theme::ink2());
    if let Some(e) = extra {
        ui.painter().text(pos2(rect.right() - 270.0, rect.center().y), Align2::LEFT_CENTER, e, theme::body(12.5), theme::ink2());
    }
    // the like, then the length
    let heart = Rect::from_center_size(pos2(rect.right() - 92.0, rect.center().y), vec2(28.0, 28.0));
    let hr = ui.interact(heart, r.id.with("like"), Sense::click());
    if liked || hovered || hr.hovered() {
        ui.painter().text(heart.center(), Align2::CENTER_CENTER, if liked { ic::HEART_FILL } else { ic::HEART }, theme::icon(13.0), if liked { ACCENT } else { theme::ink2() });
    }
    let more = Rect::from_center_size(pos2(rect.right() - 20.0, rect.center().y), vec2(28.0, 28.0));
    let mr = ui.interact(more, r.id.with("more"), Sense::click());
    if hovered || mr.hovered() {
        ui.painter().text(more.center(), Align2::CENTER_CENTER, ic::MORE, theme::icon(13.0), theme::ink2());
    } else {
        ui.painter().text(pos2(rect.right() - 20.0, rect.center().y), Align2::CENTER_CENTER, time(t.duration_ms), theme::body(12.5), theme::ink2());
    }
    RowOut { clicked: r.clicked(), double: r.double_clicked(), like: hr.clicked(), menu: mr.clicked() || r.secondary_clicked(), response: r }
}

/// a card: a big cover, a title, a line under it
pub fn card(ui: &mut Ui, images: &mut Images, src: Option<&str>, title: &str, subtitle: &str, width: f32, round: bool) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(width, width + 46.0), Sense::click());
    let art = Rect::from_min_size(rect.min, vec2(width, width));
    if round {
        paint_cover(ui, images, src, art, (width / 2.0) as u8);
    } else {
        paint_cover(ui, images, src, art, theme::R_CARD);
    }
    if r.hovered() {
        ui.painter().rect_filled(art, CornerRadius::same(if round { (width / 2.0) as u8 } else { theme::R_CARD }), Color32::from_black_alpha(50));
        let b = Rect::from_center_size(art.right_bottom() - vec2(28.0, 28.0), vec2(40.0, 40.0));
        ui.painter().circle_filled(b.center(), 20.0, ACCENT);
        ui.painter().text(b.center(), Align2::CENTER_CENTER, ic::PLAY, theme::icon(15.0), Color32::from_rgb(0x06, 0x20, 0x1d));
    }
    let g = clipped(ui, title, theme::medium(13.0), INK, width);
    ui.painter().galley(pos2(rect.left(), art.bottom() + 8.0), g, INK);
    let g = clipped(ui, subtitle, theme::body(12.0), theme::ink2(), width);
    ui.painter().galley(pos2(rect.left(), art.bottom() + 26.0), g, theme::ink2());
    r
}

/// a heading of a section on a page
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new(crate::i18n::t(text)).font(theme::medium(18.0)).color(INK));
    ui.add_space(6.0);
}

/// a glass panel (settings cards, the queue)
pub fn glass_frame() -> egui::Frame {
    egui::Frame::new().fill(theme::glass()).stroke(Stroke::new(1.0, theme::edge())).corner_radius(CornerRadius::same(theme::R_PANEL)).inner_margin(egui::Margin::same(14))
}

/// a text field with a search icon in a glass box
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let mut out = None;
    egui::Frame::new().fill(theme::glass_strong()).stroke(Stroke::new(1.0, theme::edge())).corner_radius(CornerRadius::same(18)).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(ic::SEARCH).font(theme::icon(14.0)).color(theme::ink2()));
            out = Some(ui.add(egui::TextEdit::singleline(text).hint_text(crate::i18n::t(hint)).frame(egui::Frame::NONE).desired_width(width - 60.0).font(theme::body(15.0))));
        });
    });
    out.unwrap()
}

/// the rounded outline used around focused things
pub fn outline(ui: &Ui, rect: Rect, radius: u8) {
    ui.painter().rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.0, theme::edge()), StrokeKind::Inside);
}
