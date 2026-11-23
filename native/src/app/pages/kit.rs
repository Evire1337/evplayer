//! The building blocks of the settings page, drawn as src/pages/settings/parts.tsx and the
//! page's CSS have them (.set-head, .set-card, .set-row, .switch, .segmented, .chip, .dial,
//! .pill, .field, .check-row): the same sizes, colours and spacing, so each tab can be written
//! line for line like its TSX.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, Ui};

use crate::ui::theme::{self, ACCENT, GROUND, INK};

/// rows in a card are split by a faint line (.set-card-body > .set-row + .set-row)
fn last_row_id() -> egui::Id {
    egui::Id::unique("kit-last-row")
}

fn set_last_row(ui: &Ui, v: bool) {
    ui.data_mut(|d| d.insert_temp(last_row_id(), v));
}

fn t(s: &str) -> String {
    crate::i18n::t(s)
}

/// a wrapped text block with the page's line height
pub fn wrapped(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32, line: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat { font_id: font, color, line_height: Some(line), ..Default::default() });
    job.wrap.max_width = width.max(10.0);
    ui.painter().layout_job(job)
}

/// hsl → colour, as the page's `hsl(var(--hue) 75% 58%)`
pub fn hsl(h: f32, s: f32, l: f32, a: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(to(r), to(g), to(b), (a * 255.0).round() as u8)
}

/// A rounded rectangle filled with a linear gradient (`linear-gradient(145deg, from, to)`):
/// egui's own antialiased shape, its vertices recoloured along the gradient's direction.
pub fn gradient_rect(ui: &Ui, rect: Rect, radius: f32, from: Color32, to: Color32, deg: f32) {
    let shape = egui::Shape::Rect(egui::epaint::RectShape::filled(rect, CornerRadius::same(radius as u8), Color32::WHITE));
    let ppp = ui.ctx().pixels_per_point();
    let mut tess = egui::epaint::Tessellator::new(ppp, ui.ctx().tessellation_options(|o| *o), [1, 1], vec![]);
    let mut mesh = egui::Mesh::default();
    tess.tessellate_shape(shape, &mut mesh);
    // CSS angles: 0deg points up, 90deg right
    let a = deg.to_radians();
    let dir = vec2(a.sin(), -a.cos());
    let half = (rect.width() * dir.x.abs() + rect.height() * dir.y.abs()) / 2.0;
    let c = rect.center();
    let (f, tcol) = (egui::Rgba::from(from), egui::Rgba::from(to));
    for v in &mut mesh.vertices {
        let k = (((v.pos - c).dot(dir) / half.max(1.0)) * 0.5 + 0.5).clamp(0.0, 1.0);
        let col = f * (1.0 - k) + tcol * k;
        let cov = v.color.a() as f32 / 255.0;
        v.color = Color32::from(col * cov);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// .set-head: a 52 px gradient square with the tab's icon, the title and what the tab is for
pub fn tab_header(ui: &mut Ui, icon: &str, hue: f32, title: &str, desc: &str) {
    let w = ui.available_width();
    let text_w = w - 52.0 - 16.0 - 4.0;
    let tg = ui.painter().layout_no_wrap(t(title), theme::heavy(24.0), INK);
    let dg = wrapped(ui, &t(desc), theme::body(13.5), theme::ink2(), text_w, 19.0);
    let text_h = tg.size().y + 3.0 + dg.size().y;
    let h = text_h.max(52.0) + 10.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let sq = Rect::from_min_size(pos2(rect.left() + 2.0, rect.center().y - 26.0), vec2(52.0, 52.0));
    let shadow = egui::Shadow { offset: [0, 12], blur: 30, spread: 0, color: hsl(hue, 0.7, 0.2, 0.5) };
    ui.painter().add(shadow.as_shape(sq, CornerRadius::same(17)));
    gradient_rect(ui, sq, 17.0, hsl(hue, 0.75, 0.58, 1.0), hsl(hue, 0.65, 0.28, 1.0), 145.0);
    theme::paint_icon(ui.painter(), sq.center(), Align2::CENTER_CENTER, icon, 22.0 / 1.4, Color32::WHITE);
    let x = sq.right() + 16.0;
    let y = rect.center().y - text_h / 2.0;
    ui.painter().galley(pos2(x, y), tg.clone(), INK);
    ui.painter().galley(pos2(x, y + tg.size().y + 3.0), dg, theme::ink2());
}

/// .set-card: glass, a faint edge, rounded 22, padding 16 by 18; a title and hint on top with
/// the card's buttons on the right
pub fn card(ui: &mut Ui, title: &str, hint: &str, actions: Option<&mut dyn FnMut(&mut Ui)>, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::glass())
        .stroke(Stroke::new(1.0, theme::edge()))
        .corner_radius(CornerRadius::same(22))
        .inner_margin(egui::Margin { left: 18, right: 18, top: 16, bottom: 16 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            if !title.is_empty() || actions.is_some() {
                let w = ui.available_width();
                ui.horizontal_top(|ui| {
                    let actions_w = if actions.is_some() { 180.0 } else { 0.0 };
                    ui.vertical(|ui| {
                        ui.set_width(w - actions_w);
                        if !title.is_empty() {
                            ui.label(egui::RichText::new(t(title)).font(theme::heavy(14.0)).color(INK));
                        }
                        if !hint.is_empty() {
                            let g = wrapped(ui, &t(hint), theme::body(12.5), theme::ink3(), w - actions_w, 17.5);
                            let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                            ui.painter().galley(r.min, g, theme::ink3());
                        }
                    });
                    if let Some(a) = actions {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| a(ui));
                    }
                });
                ui.add_space(2.0);
            }
            set_last_row(ui, false);
            body(ui);
            set_last_row(ui, false);
        });
    ui.add_space(16.0);
}

/// the panel's gap between blocks that are not cards
pub fn gap(ui: &mut Ui) {
    ui.add_space(16.0);
}

/// .set-row: the title and hint on the left, the control on the right, centred; the control's
/// size is remembered from the frame before, so the text knows how much room it has
pub fn row_full(ui: &mut Ui, icon: Option<&dyn Fn(&mut Ui, Rect)>, title: &str, hint: &str, clickable: bool, disabled: bool, control: impl FnOnce(&mut Ui)) -> Response {
    let id = ui.next_auto_id().with("kit-row");
    let prev = ui.data(|d| d.get_temp::<bool>(last_row_id())).unwrap_or(false);
    let cs: egui::Vec2 = ui.data(|d| d.get_temp(id)).unwrap_or(vec2(0.0, 0.0));
    let w = ui.available_width();
    let icon_w = if icon.is_some() { 32.0 + 14.0 } else { 0.0 };
    let text_w = (w - icon_w - if cs.x > 0.0 { cs.x + 14.0 } else { 0.0 }).max(120.0);
    let alpha = if disabled { 0.45 } else { 1.0 };
    let tg = (!title.is_empty()).then(|| wrapped(ui, &t(title), theme::medium(13.5), INK.gamma_multiply(alpha), text_w, 18.0));
    let hg = (!hint.is_empty()).then(|| wrapped(ui, &t(hint), theme::body(12.0), theme::ink3().gamma_multiply(alpha), text_w, 16.8));
    let text_h = tg.as_ref().map_or(0.0, |g| g.size().y) + hg.as_ref().map_or(0.0, |g| g.size().y) + if tg.is_some() && hg.is_some() { 2.0 } else { 0.0 };
    let h = text_h.max(cs.y).max(icon.map_or(0.0, |_| 32.0)) + 20.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if clickable && !disabled { Sense::click() } else { Sense::hover() });
    if prev {
        ui.painter().hline(rect.x_range(), rect.top() - ui.spacing().item_spacing.y / 2.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 15)));
    }
    let mut x = rect.left();
    if let Some(paint) = icon {
        let r = Rect::from_min_size(pos2(x, rect.center().y - 16.0), vec2(32.0, 32.0));
        paint(ui, r);
        x += icon_w;
    }
    let mut y = rect.center().y - text_h / 2.0;
    let hot = clickable && !disabled && resp.hovered();
    if let Some(g) = tg {
        let h = g.size().y;
        ui.painter().galley(pos2(x, y), g, if hot { Color32::WHITE } else { INK.gamma_multiply(alpha) });
        y += h + 2.0;
    }
    if let Some(g) = hg {
        ui.painter().galley(pos2(x, y), g, theme::ink3().gamma_multiply(alpha));
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(0.0, 10.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
    child.spacing_mut().item_spacing.x = 8.0;
    if disabled {
        child.disable();
    }
    control(&mut child);
    let used = child.min_rect();
    let size = if used.width() > 0.5 { used.size() } else { vec2(0.0, 0.0) };
    if (size - cs).length() > 0.5 {
        ui.data_mut(|d| d.insert_temp(id, size));
        ui.ctx().request_repaint();
    }
    set_last_row(ui, true);
    resp
}

pub fn row(ui: &mut Ui, title: &str, hint: &str, control: impl FnOnce(&mut Ui)) {
    row_full(ui, None, title, hint, false, false, control);
}

/// a row with a value on the right (the diagnostics' numbers)
pub fn value_row(ui: &mut Ui, title: &str, hint: &str, value: &str) {
    let v = value.to_string();
    row(ui, title, hint, |ui| {
        ui.label(egui::RichText::new(v).font(theme::body(14.0)).color(INK));
    });
}

/// SwitchRow: the whole row toggles the switch; returns the new value when it changed
pub fn switch_row(ui: &mut Ui, title: &str, hint: &str, on: bool, disabled: bool) -> Option<bool> {
    let mut v = on;
    let mut flipped = false;
    let r = row_full(ui, None, title, hint, true, disabled, |ui| {
        if switch(ui, &mut v).changed() {
            flipped = true;
        }
    });
    if r.clicked() && !flipped {
        v = !on;
        flipped = true;
    }
    flipped.then_some(v)
}

/// a switch row bound to a setting
pub fn setting(ui: &mut Ui, title: &str, hint: &str, get: impl Fn(&crate::store::state::Settings) -> bool, set: impl Fn(&mut crate::store::state::Settings, bool)) {
    setting_if(ui, title, hint, true, get, set);
}

pub fn setting_if(ui: &mut Ui, title: &str, hint: &str, enabled: bool, get: impl Fn(&crate::store::state::Settings) -> bool, set: impl Fn(&mut crate::store::state::Settings, bool)) {
    let on = get(&crate::state::get().settings.read().unwrap());
    if let Some(v) = switch_row(ui, title, hint, on, !enabled) {
        crate::state::get().set_settings(|s| set(s, v));
    }
}

/// .switch: 42 by 24, faint when off; accent with the knob in the ground colour when on
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(42.0, 24.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let p = ui.painter();
    let dim = if ui.is_enabled() { 1.0 } else { 0.45 };
    let track = if *on { ACCENT } else { Color32::from_rgba_unmultiplied(238, 247, 244, 46) };
    p.rect_filled(rect, CornerRadius::same(12), track.gamma_multiply(dim));
    let x = if *on { rect.left() + 3.0 + 18.0 + 9.0 } else { rect.left() + 3.0 + 9.0 };
    p.circle_filled(pos2(x, rect.center().y), 9.0, if *on { GROUND } else { INK }.gamma_multiply(dim));
    r
}

/// .segmented: a faint tray with buttons 30 high; the chosen one in ink
pub fn segmented(ui: &mut Ui, items: &[&str], selected: usize) -> Option<usize> {
    let mut pressed = None;
    // measured first and placed as one block, so the order stays left to right whichever way
    // the row around it lays things out
    let gs: Vec<_> = items.iter().map(|l| ui.painter().layout_no_wrap(t(l), theme::medium(12.5), Color32::PLACEHOLDER)).collect();
    let total: f32 = gs.iter().map(|g| g.size().x + 26.0).sum::<f32>() + 3.0 * (gs.len().saturating_sub(1)) as f32 + 6.0;
    let (tray, _) = ui.allocate_exact_size(vec2(total, 36.0), Sense::hover());
    ui.painter().rect_filled(tray, CornerRadius::same(13), Color32::from_rgba_unmultiplied(238, 247, 244, 18));
    let mut x = tray.left() + 3.0;
    for (i, g) in gs.into_iter().enumerate() {
        let rect = Rect::from_min_size(pos2(x, tray.top() + 3.0), vec2(g.size().x + 26.0, 30.0));
        x = rect.right() + 3.0;
        let r = ui.interact(rect, ui.scope_id().with(("seg", i, items.len())), Sense::click());
        let on = i == selected;
        let (fill, fg) = if on {
            (INK, GROUND)
        } else if r.hovered() {
            (theme::hover(), INK)
        } else {
            (Color32::TRANSPARENT, theme::ink2())
        };
        ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
        ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
        if r.clicked() {
            pressed = Some(i);
        }
    }
    pressed
}

/// .chip: 30 high, fully round; ink when chosen
pub fn chip(ui: &mut Ui, label: &str, on: bool) -> Response {
    chip_sized(ui, &t(label), on, 30.0, None)
}

pub fn chip_sized(ui: &mut Ui, text: &str, on: bool, height: f32, lead: Option<&dyn Fn(&Ui, Rect)>) -> Response {
    let g = ui.painter().layout_no_wrap(text.to_string(), theme::medium(12.5), Color32::PLACEHOLDER);
    let lead_w = if lead.is_some() { 18.0 + 8.0 } else { 0.0 };
    let pad = if height > 32.0 { 14.0 } else { 13.0 };
    let (rect, r) = ui.allocate_exact_size(vec2(g.size().x + lead_w + pad * 2.0, height), Sense::click());
    let (fill, fg) = if on {
        (INK, GROUND)
    } else if r.hovered() {
        (Color32::from_rgba_unmultiplied(238, 247, 244, 36), INK)
    } else {
        (Color32::from_rgba_unmultiplied(238, 247, 244, 20), theme::ink2())
    };
    ui.painter().rect_filled(rect, CornerRadius::same((height / 2.0) as u8), fill);
    let mut x = rect.left() + pad;
    if let Some(paint) = lead {
        paint(ui, Rect::from_min_size(pos2(x, rect.center().y - 9.0), vec2(18.0, 18.0)));
        x += lead_w;
    }
    ui.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
    r
}

/// .chip.is-add: a dashed outline, nothing inside
pub fn chip_add(ui: &mut Ui, label: &str) -> Response {
    let g = ui.painter().layout_no_wrap(t(label), theme::medium(12.5), Color32::PLACEHOLDER);
    let (rect, r) = ui.allocate_exact_size(vec2(g.size().x + 26.0, 30.0), Sense::click());
    dashed_capsule(ui, rect.shrink(0.5), Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 64)));
    if r.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(15), Color32::from_rgba_unmultiplied(238, 247, 244, 20));
    }
    ui.painter().galley(rect.center() - g.size() / 2.0, g, if r.hovered() { INK } else { theme::ink2() });
    r
}

/// a dashed outline around a capsule
pub fn dashed_capsule(ui: &Ui, rect: Rect, stroke: Stroke) {
    let rad = rect.height() / 2.0;
    let mut pts = Vec::new();
    let steps = 10;
    // top edge, right cap, bottom edge, left cap
    pts.push(pos2(rect.left() + rad, rect.top()));
    pts.push(pos2(rect.right() - rad, rect.top()));
    for i in 1..=steps {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / steps as f32;
        pts.push(pos2(rect.right() - rad + rad * a.cos(), rect.center().y + rad * a.sin()));
    }
    pts.push(pos2(rect.left() + rad, rect.bottom()));
    for i in 1..=steps {
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / steps as f32;
        pts.push(pos2(rect.left() + rad + rad * a.cos(), rect.center().y + rad * a.sin()));
    }
    ui.painter().extend(egui::Shape::dashed_line(&pts, stroke, 4.0, 3.0));
}

/// .reset-btn: plain faint text that lights up
pub fn text_button(ui: &mut Ui, label: &str) -> Response {
    let g = ui.painter().layout_no_wrap(t(label), theme::body(12.5), Color32::PLACEHOLDER);
    let (rect, r) = ui.allocate_exact_size(g.size() + vec2(0.0, 8.0), Sense::click());
    ui.painter().galley(pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, if r.hovered() { INK } else { theme::ink3() });
    r
}

/// .svc-more / .link-btn.strong: a bold faint link
pub fn link(ui: &mut Ui, label: &str, strong: bool) -> Response {
    let font = if strong { theme::medium(12.5) } else { theme::body(12.5) };
    let g = ui.painter().layout_no_wrap(t(label), font, Color32::PLACEHOLDER);
    let (rect, r) = ui.allocate_exact_size(g.size() + vec2(0.0, 4.0), Sense::click());
    let color = if r.hovered() { INK } else if strong { INK } else { theme::ink3() };
    ui.painter().galley(pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, color);
    if r.hovered() {
        ui.painter().hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(1.0, color));
    }
    r
}

/// .dial: ticks every 9 px, the part up to the value brighter, an accent notch with a glow
pub fn dial(ui: &mut Ui, width: f32, value: f64, min: f64, max: f64, step: f64) -> Option<f64> {
    let (rect, r) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click_and_drag());
    let enabled = ui.is_enabled();
    let frac = ((value - min) / (max - min)).clamp(0.0, 1.0) as f32;
    let p = ui.painter();
    let dim = if enabled { 1.0 } else { 0.45 };
    let fill_x = rect.left() + rect.width() * frac;
    let mut x = rect.left();
    while x + 2.0 <= rect.right() {
        let a = if x < fill_x { 140 } else { 41 };
        p.rect_filled(Rect::from_min_size(pos2(x, rect.center().y - 6.0), vec2(2.0, 12.0)), CornerRadius::ZERO, Color32::from_rgba_unmultiplied(238, 247, 244, a).gamma_multiply(dim));
        x += 9.0;
    }
    let tx = (fill_x - 2.0).clamp(rect.left(), rect.right() - 4.0);
    let notch = Rect::from_min_size(pos2(tx, rect.top()), vec2(4.0, 26.0));
    for (grow, a) in [(6.0, 18), (3.0, 40)] {
        p.rect_filled(notch.expand(grow), CornerRadius::same(4), Color32::from_rgba_unmultiplied(111, 240, 196, a).gamma_multiply(dim));
    }
    p.rect_filled(notch, CornerRadius::same(2), ACCENT.gamma_multiply(dim));
    let at = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked() || r.is_pointer_button_down_on())?;
    let f = ((at.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
    let v = ((min + f * (max - min)) / step).round() * step;
    let v = v.clamp(min, max);
    ((v - value).abs() > step / 4.0).then_some(v)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Ok,
    Warn,
    Off,
    Info,
}

/// .pill: a small round label with a dot in its colour
pub fn pill(ui: &mut Ui, tone: Tone, text: &str) -> Response {
    let (bg, fg) = match tone {
        Tone::Ok => (Color32::from_rgba_unmultiplied(111, 240, 196, 36), ACCENT),
        Tone::Warn => (Color32::from_rgba_unmultiplied(255, 196, 92, 36), Color32::from_rgb(0xff, 0xc4, 0x5c)),
        Tone::Off => (Color32::from_rgba_unmultiplied(238, 247, 244, 20), theme::ink3()),
        Tone::Info => (Color32::from_rgba_unmultiplied(138, 180, 255, 36), Color32::from_rgb(0xa9, 0xc5, 0xff)),
    };
    let g = ui.painter().layout_no_wrap(t(text), theme::heavy(11.5), Color32::PLACEHOLDER);
    let (rect, r) = ui.allocate_exact_size(vec2(g.size().x + 20.0 + 12.0, 24.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(12), bg);
    ui.painter().circle_filled(pos2(rect.left() + 13.0, rect.center().y), 3.0, fg);
    ui.painter().galley(pos2(rect.left() + 22.0, rect.center().y - g.size().y / 2.0), g, fg);
    r
}

/// .eyebrow: small capitals with wide spacing
pub fn eyebrow(ui: &mut Ui, text: &str) {
    let job = egui::text::LayoutJob::single_section(t(text).to_uppercase(), egui::TextFormat { font_id: theme::medium(12.0), color: theme::ink3(), extra_letter_spacing: 0.96, ..Default::default() });
    let g = ui.painter().layout_job(job);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, theme::ink3());
    set_last_row(ui, false);
}

/// .muted.small: a faint note, wrapped
pub fn note(ui: &mut Ui, text: &str) {
    let w = ui.available_width();
    let g = wrapped(ui, &t(text), theme::body(12.5), theme::ink3(), w, 18.75);
    let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, theme::ink3());
    set_last_row(ui, false);
}

/// .field-error
pub fn error(ui: &mut Ui, text: &str) {
    let w = ui.available_width();
    let g = wrapped(ui, text, theme::body(12.5), theme::DANGER, w, 17.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, theme::DANGER);
}

/// The page's text box (.field input, .svc-connect input): faint fill, a faint edge that
/// brightens while typing, rounded 12 or 13, 40 high.
pub fn input(ui: &mut Ui, text: &mut String, hint: &str, width: f32, password: bool) -> Response {
    input_styled(ui, text, hint, width, 40.0, 13, theme::medium500(), password)
}

pub fn input_styled(ui: &mut Ui, text: &mut String, hint: &str, width: f32, height: f32, radius: u8, family: egui::FontFamily, password: bool) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let id = ui.next_auto_id().with("kit-input");
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect(rect, CornerRadius::same(radius), Color32::from_rgba_unmultiplied(238, 247, 244, 18), Stroke::new(1.0, if focused { Color32::from_rgba_unmultiplied(238, 247, 244, 89) } else { theme::edge() }), egui::StrokeKind::Inside);
    let inner = rect.shrink2(vec2(13.0, 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .hint_text(egui::RichText::new(t(hint)).color(theme::ink3()))
            .frame(egui::Frame::NONE)
            .font(FontId::new(13.5, family))
            .text_color(INK)
            .password(password)
            .desired_width(inner.width())
            .align(egui::Align2::LEFT_CENTER),
    )
}

/// .field: a label over a box
pub fn field(ui: &mut Ui, label: &str, text: &mut String, hint: &str, password: bool) -> Response {
    ui.label(egui::RichText::new(t(label)).font(theme::body(12.5)).color(theme::ink2()));
    let w = ui.available_width();
    let r = input_styled(ui, text, hint, w, 40.0, 12, theme::medium500(), password);
    set_last_row(ui, false);
    r
}

/// .check-row: a box that ticks, a title and hint, and a size on the right
pub fn check_row(ui: &mut Ui, title: &str, hint: &str, on: bool, right: &str) -> Option<bool> {
    let w = ui.available_width();
    let size_g = ui.painter().layout_no_wrap(right.to_string(), theme::medium(13.0), Color32::PLACEHOLDER);
    let text_w = w - 12.0 - 18.0 - 12.0 - size_g.size().x - 12.0 - 12.0;
    let tg = wrapped(ui, &t(title), theme::medium(14.0), INK, text_w, 19.0);
    let hg = wrapped(ui, &t(hint), theme::body(12.5), theme::ink3(), text_w, 18.75);
    let h = tg.size().y + 2.0 + hg.size().y + 16.0;
    let (rect, r) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let p = ui.painter();
    if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(12), theme::hover());
    }
    let b = Rect::from_min_size(pos2(rect.left() + 6.0, rect.top() + 10.0), vec2(18.0, 18.0));
    if on {
        p.rect_filled(b, CornerRadius::same(6), ACCENT);
        let c = b.center();
        p.line_segment([c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.0)], Stroke::new(2.0, GROUND));
        p.line_segment([c + vec2(-1.0, 3.0), c + vec2(4.5, -2.5)], Stroke::new(2.0, GROUND));
    } else {
        p.rect_stroke(b, CornerRadius::same(6), Stroke::new(1.5, theme::ink3()), egui::StrokeKind::Inside);
    }
    let x = b.right() + 12.0;
    let th = tg.size().y;
    p.galley(pos2(x, rect.top() + 8.0), tg, INK);
    p.galley(pos2(x, rect.top() + 8.0 + th + 2.0), hg, theme::ink3());
    p.galley(pos2(rect.right() - 6.0 - size_g.size().x, rect.top() + 9.0), size_g, theme::ink2());
    set_last_row(ui, false);
    r.clicked().then_some(!on)
}

/// kbd: a key of a combination
pub fn kbd(ui: &mut Ui, key: &str) {
    let g = ui.painter().layout_no_wrap(key.to_string(), theme::medium(12.0), Color32::PLACEHOLDER);
    let (rect, _) = ui.allocate_exact_size(g.size() + vec2(16.0, 6.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(7), Color32::from_rgba_unmultiplied(238, 247, 244, 26), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, INK);
}

/// .btn.danger
pub fn danger_btn(ui: &mut Ui, text: &str) -> Response {
    let g = ui.painter().layout_no_wrap(t(text), theme::medium(14.0), Color32::PLACEHOLDER);
    let (rect, r) = ui.allocate_exact_size(vec2(g.size().x + 32.0, 40.0), Sense::click());
    ui.painter().rect_filled(rect, CornerRadius::same(20), Color32::from_rgb(0xff, 0x6b, 0x6b));
    ui.painter().galley(rect.center() - g.size() / 2.0, g, Color32::from_rgb(0x1a, 0x06, 0x06));
    r
}

/// a button that is shown but cannot be pressed (`disabled`): dimmed
pub fn btn_if(ui: &mut Ui, enabled: bool, icon: Option<&str>, text: &str, primary: bool) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        if !enabled {
            ui.set_opacity(0.5);
        }
        crate::ui::widgets::btn(ui, icon, text, primary)
    })
    .inner
}
