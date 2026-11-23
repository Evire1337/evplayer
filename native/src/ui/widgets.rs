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
    crate::ui::theme::paint_icon(&p, rect.center(), Align2::CENTER_CENTER, icon, rect.width() * 0.42, color);
}

/// the big round play button
pub fn play_button(ui: &mut Ui, playing: bool, size: f32) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let p = ui.painter();
    let fill = if r.hovered() { Color32::from_rgba_unmultiplied(238, 247, 244, 70) } else { Color32::from_rgba_unmultiplied(238, 247, 244, 46) };
    p.circle_filled(rect.center(), size / 2.0, fill);
    crate::ui::theme::paint_icon(&p, rect.center(), Align2::CENTER_CENTER, if playing { ic::PAUSE } else { ic::PLAY }, size * 0.36, INK);
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
        crate::ui::theme::paint_icon(&p, pos2(x + 7.0, rect.center().y), Align2::CENTER_CENTER, i, 13.0, fg);
        x += icon_w;
    }
    p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font, fg);
    r
}

/// .btn: 40 high, rounded fully, padding 16, a 16 px icon and 8 before the text, 14 px semibold;
/// primary is light with dark text, ghost a faint disc
pub fn btn(ui: &mut Ui, icon: Option<&str>, text: &str, primary: bool) -> Response {
    let text = crate::i18n::t(text);
    let font = theme::medium(14.0);
    // laid out without a colour: it is painted in the button's own
    let galley = ui.painter().layout_no_wrap(text, font, Color32::PLACEHOLDER);
    let icon_w = if icon.is_some() { 16.0 + 8.0 } else { 0.0 };
    let (rect, r) = ui.allocate_exact_size(vec2(galley.size().x + icon_w + 32.0, 40.0), Sense::click());
    let p = ui.painter();
    let (fill, fg) = if primary {
        (if r.hovered() { Color32::WHITE } else { INK }, theme::GROUND)
    } else {
        (Color32::from_rgba_unmultiplied(238, 247, 244, if r.hovered() { 46 } else { 26 }), INK)
    };
    let rect_drawn = if r.is_pointer_button_down_on() { rect.shrink2(rect.size() * 0.015) } else { rect };
    p.rect_filled(rect_drawn, CornerRadius::same(20), fill);
    let mut x = rect.left() + 16.0;
    if let Some(i) = icon {
        crate::ui::icons::paint(p, i.trim_start_matches('@'), Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0)), fg);
        x += icon_w;
    }
    p.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    r
}

/// one entry of a drop-down menu: icon, label, red for danger, a check when chosen
pub struct MenuEntry<'a> {
    pub icon: Option<&'a str>,
    pub label: String,
    pub danger: bool,
}

/// A menu that opens under `anchor` (.menu: glass, rounded 16, entries 13.5 px with icons);
/// closes on a press outside or Escape. Returns the entry chosen.
pub fn drop_menu(ui: &mut Ui, id: &str, anchor: Rect, open: &mut bool, entries: &[MenuEntry]) -> Option<usize> {
    if !*open {
        return None;
    }
    let mut chosen = None;
    let area = egui::Area::new(egui::Id::unique(id)).order(egui::Order::Foreground).fixed_pos(pos2(anchor.left(), anchor.bottom() + 6.0)).constrain(true);
    let r = area.show(ui.ctx(), |ui| {
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(6, 18, 20, 235))
            .stroke(Stroke::new(1.0, theme::edge()))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin::same(6))
            .shadow(egui::Shadow { offset: [0, 18], blur: 50, spread: 0, color: Color32::from_black_alpha(128) })
            .show(ui, |ui| {
                ui.set_min_width(220.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, e) in entries.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(220.0), 38.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, CornerRadius::same(10), theme::hover());
                    }
                    let color = if e.danger { Color32::from_rgb(0xff, 0x9b, 0x9b) } else { INK };
                    let mut x = rect.left() + 10.0;
                    if let Some(icon) = e.icon {
                        crate::ui::icons::paint(ui.painter(), icon.trim_start_matches('@'), Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0)), if e.danger { color } else { theme::ink2() });
                        x += 26.0;
                    }
                    ui.painter().text(pos2(x, rect.center().y), Align2::LEFT_CENTER, &e.label, theme::body(13.5), color);
                    if resp.clicked() {
                        chosen = Some(i);
                    }
                }
            });
    });
    let pressed = ui.ctx().input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
    if chosen.is_some() || pressed.is_some_and(|p| !r.response.rect.contains(p) && !anchor.contains(p)) || ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }
    chosen
}

/// .filter-input: a dark field 38 high, rounded 12, with a faint edge
pub fn filter_input(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let mut out = None;
    egui::Frame::new().fill(Color32::from_rgba_unmultiplied(5, 16, 18, 77)).stroke(Stroke::new(1.0, theme::edge())).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::symmetric(12, 0)).show(ui, |ui| {
        ui.set_height(38.0);
        ui.set_width(width - 24.0);
        ui.centered_and_justified(|ui| {
            out = Some(ui.add(egui::TextEdit::singleline(text).hint_text(crate::i18n::t(hint)).frame(egui::Frame::NONE).font(theme::medium(14.0)).text_color(INK).desired_width(width - 24.0).align(egui::Align2::LEFT_CENTER)));
        });
    });
    out.unwrap()
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

/// a switch like the page's .switch: accent when on, the knob in the ground colour
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    switch_sized(ui, on, 42.0, 24.0)
}

pub fn switch_sized(ui: &mut Ui, on: &mut bool, w: f32, h: f32) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let p = ui.painter();
    let track = if *on { ACCENT } else { Color32::from_rgba_unmultiplied(238, 247, 244, 46) };
    p.rect_filled(rect, CornerRadius::same((h / 2.0) as u8), track);
    let knob = h / 2.0 - 3.0;
    let x = if *on { rect.right() - 3.0 - knob } else { rect.left() + 3.0 + knob };
    p.circle_filled(pos2(x, rect.center().y), knob, if *on { theme::GROUND } else { INK });
    r
}

/// The seek bar (the page's .slider): 5 px (7 px on "Сейчас играет"), the part heard in ink,
/// the rest faint, the knob only while the pointer is over it. Returns where it was let go, 0..1.
pub fn seek_bar(ui: &mut Ui, width: f32, frac: f32, buffered: f32) -> Option<f32> {
    seek_bar_sized(ui, width, frac, buffered, 5.0)
}

pub fn seek_bar_sized(ui: &mut Ui, width: f32, frac: f32, buffered: f32, thick: f32) -> Option<f32> {
    let (rect, r) = ui.allocate_exact_size(vec2(width, 16.0), Sense::click_and_drag());
    let line = Rect::from_center_size(rect.center(), vec2(rect.width(), thick));
    let p = ui.painter();
    let round = CornerRadius::same((thick / 2.0).ceil() as u8);
    p.rect_filled(line, round, Color32::from_rgba_unmultiplied(238, 247, 244, 46));
    if buffered > 0.0 && buffered < 1.0 {
        let b = Rect::from_min_max(line.min, pos2(line.left() + line.width() * buffered.clamp(0.0, 1.0), line.max.y));
        p.rect_filled(b, round, Color32::from_rgba_unmultiplied(238, 247, 244, 30));
    }
    let mut f = frac.clamp(0.0, 1.0);
    let at = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()).map(|p| ((p.x - line.left()) / line.width()).clamp(0.0, 1.0));
    if let Some(a) = at {
        f = a;
    }
    let done = Rect::from_min_max(line.min, pos2(line.left() + line.width() * f, line.max.y));
    p.rect_filled(done, round, INK);
    if r.hovered() || r.dragged() {
        p.circle_filled(pos2(done.right(), line.center().y), 7.0, INK);
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
            crate::ui::theme::paint_icon(&p, rect.center(), Align2::CENTER_CENTER, ic::NOTE, (rect.width() * 0.3).min(40.0), theme::ink3());
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
    track_row_full(ui, images, t, playing, liked, extra, number, false)
}

/// where a song row's columns sit from its right edge (.track-row: padding 8, gap 12; the
/// "…" 34, the time 44, the like 34, the date 130)
pub mod row_cols {
    pub const MORE: f32 = 8.0 + 17.0;
    /// the time is right-aligned to this
    pub const TIME_RIGHT: f32 = 8.0 + 34.0 + 12.0;
    pub const TIME_LEFT: f32 = TIME_RIGHT + 44.0;
    pub const LIKE: f32 = TIME_LEFT + 12.0 + 17.0;
    /// the date column's left edge (it is 130 wide)
    pub const DATE_LEFT: f32 = TIME_LEFT + 12.0 + 34.0 + 12.0 + 130.0;
    /// where the title and artist start from the left: padding, the 48 px cover, a gap
    pub const TEXT: f32 = 8.0 + 48.0 + 12.0;
}

/// A song in a list (TrackRow.tsx): cover (with play or pause over it on hover and while it is
/// the current one), title and artist, the date it was added (`dated`) or a note, the like, the
/// length, the "…". Rows are 62 high, rounded 16.
#[allow(clippy::too_many_arguments)]
pub fn track_row_full(ui: &mut Ui, images: &mut Images, t: &crate::model::Track, playing: bool, liked: bool, extra: Option<&str>, number: Option<usize>, dated: bool) -> RowOut {
    use row_cols::*;
    let width = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(vec2(width, 62.0), Sense::click());
    let hovered = r.hovered();
    if playing {
        ui.painter().rect_filled(rect, CornerRadius::same(16), theme::active());
    } else if hovered {
        ui.painter().rect_filled(rect, CornerRadius::same(16), theme::hover());
    }
    let mut x = rect.left() + 8.0;
    if let Some(n) = number {
        ui.painter().text(pos2(x + 10.0, rect.center().y), Align2::CENTER_CENTER, n.to_string(), theme::heavy(13.0), theme::ink3());
        x += 32.0;
    }
    let art = Rect::from_min_size(pos2(x, rect.center().y - 24.0), vec2(48.0, 48.0));
    let src = super::images::artwork_of(t);
    paint_cover(ui, images, src.as_deref(), art, 13);
    if hovered || playing {
        ui.painter().rect_filled(art, CornerRadius::same(13), Color32::from_black_alpha(115));
        let now_playing = playing && ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::unique("evplayer-playing"))).unwrap_or(false);
        crate::ui::icons::paint(ui.painter(), if now_playing { "pause" } else { "play" }, Rect::from_center_size(art.center(), vec2(16.0, 16.0)), INK);
    }
    x = art.right() + 12.0;
    let right_w = if dated { DATE_LEFT + 12.0 } else if extra.is_some() { LIKE + 17.0 + 12.0 + 140.0 } else { LIKE + 17.0 + 12.0 };
    let text_w = (rect.right() - x - right_w).max(60.0);
    let title_color = if playing { ACCENT } else { INK };
    let g = clipped(ui, &t.title, theme::medium(14.5), title_color, text_w);
    ui.painter().galley(pos2(x, rect.center().y - 19.0), g, title_color);
    let mut sub = t.artist_line();
    if t.explicit {
        sub = format!("🅴 {sub}");
    }
    let g = clipped(ui, &sub, theme::body(13.0), theme::ink2(), text_w);
    ui.painter().galley(pos2(x, rect.center().y + 2.0), g, theme::ink2());
    if let Some(e) = extra {
        if dated {
            let g = clipped(ui, e, theme::body(13.0), theme::ink3(), 130.0);
            ui.painter().galley(pos2(rect.right() - DATE_LEFT, rect.center().y - g.size().y / 2.0), g, theme::ink3());
        } else {
            let g = clipped(ui, e, theme::body(13.0), theme::ink3(), 220.0);
            let w = g.size().x;
            ui.painter().galley(pos2(rect.right() - LIKE - 17.0 - 12.0 - w, rect.center().y - g.size().y / 2.0), g, theme::ink3());
        }
    }
    // the like (shown while hovered, and always when liked)
    let heart = Rect::from_center_size(pos2(rect.right() - LIKE, rect.center().y), vec2(34.0, 34.0));
    let hr = ui.interact(heart, r.id.with("like"), Sense::click());
    if hr.hovered() {
        ui.painter().rect_filled(heart, CornerRadius::same(11), theme::hover());
    }
    if liked || hovered || hr.hovered() {
        let color = if liked { ACCENT } else if hr.hovered() { INK } else { theme::ink3() };
        crate::ui::icons::paint(ui.painter(), if liked { "heartFill" } else { "heart" }, Rect::from_center_size(heart.center(), vec2(17.0, 17.0)), color);
    }
    // the length, right-aligned in its column
    if t.duration_ms > 0.0 {
        ui.painter().text(pos2(rect.right() - TIME_RIGHT, rect.center().y), Align2::RIGHT_CENTER, time(t.duration_ms), theme::body(13.0), theme::ink3());
    }
    // the "…"
    let more = Rect::from_center_size(pos2(rect.right() - MORE, rect.center().y), vec2(34.0, 34.0));
    let mr = ui.interact(more, r.id.with("more"), Sense::click());
    if mr.hovered() {
        ui.painter().rect_filled(more, CornerRadius::same(11), theme::hover());
    }
    if hovered || mr.hovered() {
        crate::ui::icons::paint(ui.painter(), "more", Rect::from_center_size(more.center(), vec2(18.0, 18.0)), if mr.hovered() { INK } else { theme::ink3() });
    }
    RowOut { clicked: r.clicked(), double: r.double_clicked(), like: hr.clicked(), menu: mr.clicked() || r.secondary_clicked(), response: r }
}

/// the head of a song list with dates (.track-head): "Трек", "Добавлен", "Время", each sorting
/// the list when pressed; returns the column pressed (0, 1, 2)
pub fn track_head(ui: &mut Ui, labels: [&str; 3]) -> Option<usize> {
    use row_cols::*;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, theme::edge()));
    let mut pressed = None;
    let spots = [(rect.left() + TEXT, Align2::LEFT_TOP), (rect.right() - DATE_LEFT, Align2::LEFT_TOP), (rect.right() - TIME_RIGHT, Align2::RIGHT_TOP)];
    for (i, ((x, align), label)) in spots.into_iter().zip(labels).enumerate() {
        let g = ui.painter().layout_no_wrap(label.to_string(), theme::body(12.0), theme::ink3());
        let r = align.anchor_size(pos2(x, rect.top()), g.size());
        let resp = ui.interact(r.expand(4.0), ui.scope_id().with(("head", i)), Sense::click());
        ui.painter().galley(r.min, g, if resp.hovered() { INK } else { theme::ink3() });
        if resp.clicked() {
            pressed = Some(i);
        }
    }
    ui.add_space(6.0);
    pressed
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
        crate::ui::theme::paint_icon(&ui.painter(), b.center(), Align2::CENTER_CENTER, ic::PLAY, 15.0, Color32::from_rgb(0x06, 0x20, 0x1d));
    }
    let g = clipped(ui, title, theme::medium(13.0), INK, width);
    ui.painter().galley(pos2(rect.left(), art.bottom() + 8.0), g, INK);
    let g = clipped(ui, subtitle, theme::body(12.0), theme::ink2(), width);
    ui.painter().galley(pos2(rect.left(), art.bottom() + 26.0), g, theme::ink2());
    r
}

/// a heading of a section on a page (.row-title: 19 px, 800)
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(18.0);
    ui.label(egui::RichText::new(crate::i18n::t(text)).font(theme::heavy(19.0)).color(INK));
    ui.add_space(8.0);
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
            let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
            theme::paint_icon(ui.painter(), r.center(), Align2::CENTER_CENTER, ic::SEARCH, 14.0, theme::ink2());
            out = Some(ui.add(egui::TextEdit::singleline(text).hint_text(crate::i18n::t(hint)).frame(egui::Frame::NONE).desired_width(width - 60.0).font(theme::body(15.0))));
        });
    });
    out.unwrap()
}

/// the rounded outline used around focused things
pub fn outline(ui: &Ui, rect: Rect, radius: u8) {
    ui.painter().rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.0, theme::edge()), StrokeKind::Inside);
}


/// the kinds of the page's round buttons
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Round {
    /// no background until hovered (shuffle, repeat)
    Ghost,
    /// a faint disc (previous, next)
    Soft,
    /// the white disc of play / pause
    Main,
}

/// a round button (.round-btn): `size` across, its icon a little under half of it
pub fn round_button(ui: &mut Ui, icon: &str, size: f32, kind: Round, on: bool, tip: &str) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let p = ui.painter();
    let c = rect.center();
    // pressed: the main button sinks a little, as the page's scale(0.94)
    let radius = if kind == Round::Main && r.is_pointer_button_down_on() { size * 0.47 } else { size / 2.0 };
    let (bg, fg) = match kind {
        Round::Ghost => (if r.hovered() { theme::hover() } else { Color32::TRANSPARENT }, if on { ACCENT } else if r.hovered() { INK } else { theme::ink2() }),
        Round::Soft => (Color32::from_rgba_unmultiplied(238, 247, 244, if r.hovered() { 46 } else { 26 }), if on { ACCENT } else { INK }),
        Round::Main => (INK, theme::GROUND),
    };
    if bg != Color32::TRANSPARENT {
        p.circle_filled(c, radius, bg);
    }
    let icon_size = match kind {
        Round::Main => size * 0.36,
        _ => (size * 0.4).min(19.0),
    };
    crate::ui::theme::paint_icon(&p, c, Align2::CENTER_CENTER, icon, icon_size, fg);
    if tip.is_empty() {
        r
    } else {
        r.on_hover_text(crate::i18n::t(tip))
    }
}

/// a tool button of the player's side (.tool-btn): 40 x 40, rounded 14, accent when on
pub fn tool_button(ui: &mut Ui, icon: &str, on: bool, tip: &str) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, CornerRadius::same(14), theme::active());
    } else if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(14), theme::hover());
    }
    let fg = if on { ACCENT } else if r.hovered() { INK } else { theme::ink2() };
    crate::ui::theme::paint_icon(&p, rect.center(), Align2::CENTER_CENTER, icon, 15.0, fg);
    if tip.is_empty() {
        r
    } else {
        r.on_hover_text(crate::i18n::t(tip))
    }
}

/// a row of quick choices on a faint ground (.preset-row): the one chosen in ink
pub fn preset_row(ui: &mut Ui, items: &[String], selected: Option<usize>) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(12), Color32::from_rgba_unmultiplied(238, 247, 244, 15));
    let inner = rect.shrink(3.0);
    let n = items.len().max(1) as f32;
    let cell = (inner.width() - 2.0 * (n - 1.0)) / n;
    let mut pressed = None;
    for (i, label) in items.iter().enumerate() {
        let r = Rect::from_min_size(pos2(inner.left() + i as f32 * (cell + 2.0), inner.top()), vec2(cell, inner.height()));
        let resp = ui.interact(r, ui.scope_id().with(("preset", i)), Sense::click());
        let on = selected == Some(i);
        if on {
            ui.painter().rect_filled(r, CornerRadius::same(9), INK);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(9), theme::hover());
        }
        let fg = if on { theme::GROUND } else if resp.hovered() { INK } else { theme::ink2() };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, label, theme::medium(11.5), fg);
        if resp.clicked() {
            pressed = Some(i);
        }
    }
    pressed
}

/// The page's Stepper: a label, a dial (a strip of ticks, the filled part brighter, an accent
/// notch where the value is) and a row of minus, the value and plus. Returns the new value.
#[allow(clippy::too_many_arguments)]
pub fn stepper(ui: &mut Ui, label: &str, value: f64, display: &str, min: f64, max: f64, step: f64, disabled: bool) -> Option<f64> {
    let mut out = None;
    let w = ui.available_width();
    let label_color = if disabled { theme::ink3() } else { INK };
    ui.label(egui::RichText::new(crate::i18n::t(label)).font(theme::medium(12.5)).color(label_color));
    let (dial, r) = ui.allocate_exact_size(vec2(w, 18.0), if disabled { Sense::hover() } else { Sense::click_and_drag() });
    let p = ui.painter();
    let fill = ((value - min) / (max - min)).clamp(0.0, 1.0) as f32;
    let alpha = if disabled { 0.45 } else { 1.0 };
    let tick = |a: u8| Color32::from_rgba_unmultiplied(238, 247, 244, (a as f32 * alpha) as u8);
    let mut x = dial.left();
    while x + 2.0 <= dial.right() {
        let filled = x < dial.left() + dial.width() * fill;
        let t = Rect::from_min_size(pos2(x, dial.center().y - 6.0), vec2(2.0, 12.0));
        p.rect_filled(t, CornerRadius::ZERO, if filled { tick(140) } else { tick(41) });
        x += 9.0;
    }
    let nx = dial.left() + dial.width() * fill;
    let notch = Rect::from_center_size(pos2(nx.clamp(dial.left() + 2.0, dial.right() - 2.0), dial.center().y), vec2(4.0, 18.0));
    if !disabled {
        p.rect_filled(notch.expand(3.0), CornerRadius::same(4), Color32::from_rgba_unmultiplied(111, 240, 196, 40));
    }
    p.rect_filled(notch, CornerRadius::same(2), if disabled { theme::ink3() } else { ACCENT });
    if !disabled {
        if let Some(pt) = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()) {
            let f = ((pt.x - dial.left()) / dial.width()).clamp(0.0, 1.0) as f64;
            let v = ((min + f * (max - min)) / step).round() * step;
            out = Some(v.clamp(min, max));
        }
        if r.double_clicked() {
            out = Some(if min < 0.0 { 0.0 } else { 1.0 });
        }
    }
    ui.add_space(2.0);
    let (row, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    let minus = Rect::from_min_size(row.min, vec2(30.0, 26.0));
    let plus = Rect::from_min_size(pos2(row.right() - 30.0, row.top()), vec2(30.0, 26.0));
    let mid = Rect::from_min_max(pos2(minus.right() + 6.0, row.top()), pos2(plus.left() - 6.0, row.bottom()));
    let faint = Color32::from_rgba_unmultiplied(238, 247, 244, 20);
    for (i, (b, sign)) in [(minus, "\u{E738}"), (plus, "\u{E710}")].into_iter().enumerate() {
        let br = ui.interact(b, ui.scope_id().with((label, "step", i)), if disabled { Sense::hover() } else { Sense::click() });
        ui.painter().rect_filled(b, CornerRadius::same(9), if br.hovered() && !disabled { Color32::from_rgba_unmultiplied(238, 247, 244, 41) } else { faint });
        crate::ui::theme::paint_icon(&ui.painter(), b.center(), Align2::CENTER_CENTER, sign, 11.0, if disabled { theme::ink3() } else { INK });
        if br.clicked() && !disabled {
            let d = if i == 0 { -step * 5.0 } else { step * 5.0 };
            out = Some((value + d).clamp(min, max));
        }
    }
    ui.painter().rect_filled(mid, CornerRadius::same(9), faint);
    ui.painter().text(mid.center(), Align2::CENTER_CENTER, display, theme::medium(12.5), if disabled { theme::ink3() } else { INK });
    out
}

/// a setting with a switch (.toggle-row): its title and hint on the left, the switch on the right
pub fn toggle_row(ui: &mut Ui, title: &str, hint: &str, on: bool, compact: bool) -> Option<bool> {
    let mut v = on;
    let mut out = None;
    let w = ui.available_width();
    ui.horizontal(|ui| {
        ui.set_width(w);
        let (sw, sh) = if compact { (36.0, 20.0) } else { (42.0, 24.0) };
        ui.vertical(|ui| {
            ui.set_width(w - sw - 12.0);
            ui.label(egui::RichText::new(crate::i18n::t(title)).font(theme::medium(if compact { 12.5 } else { 13.5 })).color(INK));
            if !hint.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::t(hint)).font(theme::body(if compact { 11.0 } else { 12.0 })).color(theme::ink3()));
            }
        });
        if switch_sized(ui, &mut v, sw, sh).changed() {
            out = Some(v);
        }
    });
    out
}

/// A panel that opens above its button (the page's Popover): centred on the button, slid in
/// from the window's edges, closed by Escape or a press elsewhere. `open` is flipped by the
/// button itself; returns the panel's response so the caller can see where it is.
pub fn popover(ui: &mut Ui, id: &str, anchor: Rect, width: f32, open: &mut bool, add: impl FnOnce(&mut Ui)) {
    if !*open {
        return;
    }
    const MARGIN: f32 = 12.0;
    let screen = ui.ctx().content_rect();
    let w = width.min(screen.width() - MARGIN * 2.0);
    let left = (anchor.center().x - w / 2.0).clamp(MARGIN, screen.right() - w - MARGIN);
    let bottom = anchor.top() - 10.0;
    let area = egui::Area::new(egui::Id::unique(id)).order(egui::Order::Foreground).pivot(Align2::LEFT_BOTTOM).fixed_pos(pos2(left, bottom)).constrain(true);
    let r = area.show(ui.ctx(), |ui| {
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(8, 22, 24, 250))
            .corner_radius(CornerRadius::same(20))
            .inner_margin(egui::Margin::same(12))
            .shadow(egui::Shadow { offset: [0, 24], blur: 60, spread: 0, color: Color32::from_black_alpha(115) })
            .show(ui, |ui| {
                ui.set_width(w - 24.0);
                ui.spacing_mut().item_spacing.y = 11.0;
                add(ui);
            });
    });
    let pressed = ui.ctx().input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
    if pressed.is_some_and(|p| !r.response.rect.contains(p) && !anchor.contains(p)) || ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }
}

/// a vertical slider (the volume panel's .vslider): the filled part from the bottom
pub fn vertical_level(ui: &mut Ui, value: &mut f32) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(34.0, 150.0), Sense::click_and_drag());
    if let Some(p) = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()) {
        let v = ((rect.bottom() - p.y) / rect.height()).clamp(0.0, 1.0);
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            r.mark_changed();
        }
    }
    let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
    if r.hovered() && scroll != 0.0 {
        *value = (*value + if scroll > 0.0 { 0.05 } else { -0.05 }).clamp(0.0, 1.0);
        r.mark_changed();
    }
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(14), Color32::from_rgba_unmultiplied(238, 247, 244, 31));
    let fill_h = rect.height() * *value;
    if fill_h > 0.5 {
        let filled = Rect::from_min_max(pos2(rect.left(), rect.bottom() - fill_h), rect.max);
        // rounded at the bottom always, at the top only once it reaches it
        let top_r = if fill_h > rect.height() - 14.0 { 14 } else { 0 };
        p.rect_filled(filled, CornerRadius { nw: top_r, ne: top_r, sw: 14, se: 14 }, Color32::from_rgba_unmultiplied(238, 247, 244, 217));
    }
    r
}
