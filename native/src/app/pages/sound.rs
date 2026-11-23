//! "Звук и эффекты": the panel over the player (src/ui/components/SoundPanel.tsx and the
//! equalizer of settings/EqualizerSection.tsx): the speed, the pitch, the reverb, and the
//! equalizer with its curve, its points to drag, its presets and volume levelling.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui};

use crate::app::App;
use crate::audio::dsp::eq::{self, EqState};
use crate::state;
use crate::ui::theme::{self, ACCENT, INK};
use crate::ui::widgets;

const BAND_COLORS: [Color32; 10] = [
    Color32::from_rgb(0xf5, 0xc2, 0x42),
    Color32::from_rgb(0xff, 0x8a, 0x4c),
    Color32::from_rgb(0x38, 0xbd, 0xf8),
    Color32::from_rgb(0xa7, 0x8b, 0xfa),
    Color32::from_rgb(0xf4, 0x72, 0xb6),
    Color32::from_rgb(0x34, 0xd3, 0x99),
    Color32::from_rgb(0xfa, 0xcc, 0x15),
    Color32::from_rgb(0x60, 0xa5, 0xfa),
    Color32::from_rgb(0xfb, 0x71, 0x85),
    Color32::from_rgb(0x2d, 0xd4, 0xbf),
];

fn slider_row(ui: &mut Ui, label: &str, value_text: String, frac: &mut f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t(label)).color(theme::ink2()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_sized(vec2(52.0, 18.0), egui::Label::new(egui::RichText::new(value_text).font(theme::medium(12.5)).color(INK)));
            changed = widgets::level(ui, 170.0, frac).changed();
        });
    });
    changed
}

fn switch_row(ui: &mut Ui, label: &str, on: bool) -> Option<bool> {
    let mut v = on;
    let mut out = None;
    ui.horizontal(|ui| {
        if widgets::toggle(ui, &mut v).changed() {
            out = Some(v);
        }
        ui.label(egui::RichText::new(crate::i18n::t(label)).font(theme::body(13.0)).color(INK));
    });
    out
}

impl App {
    pub fn sound_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let height = 560.0;
        let at = pos2(rect.left(), (rect.bottom() - height).max(8.0));
        let area = egui::Area::new(egui::Id::unique("sound-panel")).order(egui::Order::Foreground).fixed_pos(at).constrain(true);
        let r = area.show(ui.ctx(), |ui| {
            egui::Frame::new().fill(Color32::from_rgb(0x0d, 0x2e, 0x2a)).stroke(Stroke::new(1.0, theme::edge())).corner_radius(20).inner_margin(16).show(ui, |ui| {
                ui.set_width(rect.width() + 40.0);
                ui.label(egui::RichText::new(crate::i18n::t("Звук и эффекты")).font(theme::medium(15.0)).color(INK));
                ui.add_space(8.0);
                self.effects_ui(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                self.equalizer_ui(ui);
            });
        });
        // a press outside closes it (the button that opened it toggles it by itself)
        let pressed_at = ui.ctx().input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
        let outside = pressed_at.is_some_and(|p| !r.response.rect.contains(p) && p.y < rect.bottom() + 8.0);
        if outside || ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pages.sound_open = false;
        }
    }

    fn effects_ui(&mut self, ui: &mut Ui) {
        let fx = state::get().effects.read().unwrap().clone();
        let mut changed = false;
        let mut speed = ((fx.speed - 0.5) / 1.5) as f32;
        if slider_row(ui, "Скорость", format!("×{:.2}", fx.speed), &mut speed) {
            let s = ((0.5 + speed as f64 * 1.5) * 20.0).round() / 20.0;
            state::get().set_effects(|e| e.speed = s);
            changed = true;
        }
        ui.horizontal(|ui| {
            for s in [0.8, 0.9, 1.0, 1.15, 1.25, 1.35] {
                if widgets::tab(ui, &format!("×{s}"), (fx.speed - s).abs() < 1e-6).clicked() {
                    state::get().set_effects(|e| e.speed = s);
                    changed = true;
                }
            }
        });
        if let Some(v) = switch_row(ui, "Тон вместе со скоростью (как пластинка)", fx.linked) {
            state::get().set_effects(|e| e.linked = v);
            changed = true;
        }
        if !fx.linked {
            let mut pitch = ((fx.pitch + 12.0) / 24.0) as f32;
            if slider_row(ui, "Тон", format!("{:+.1} пт", fx.pitch), &mut pitch) {
                let p = ((pitch as f64 * 24.0 - 12.0) * 2.0).round() / 2.0;
                state::get().set_effects(|e| e.pitch = p);
                changed = true;
            }
        }
        if let Some(v) = switch_row(ui, "Реверберация", fx.reverb) {
            state::get().set_effects(|e| e.reverb = v);
            changed = true;
        }
        if fx.reverb {
            let mut amount = fx.reverb_amount as f32;
            if slider_row(ui, "Сила", format!("{:.0}%", fx.reverb_amount * 100.0), &mut amount) {
                state::get().set_effects(|e| e.reverb_amount = (amount as f64 * 20.0).round() / 20.0);
                changed = true;
            }
        }
        ui.horizontal(|ui| {
            if let Some(v) = switch_row(ui, "Оставлять для следующих треков", fx.keep) {
                state::get().set_effects(|e| e.keep = v);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Label::new(egui::RichText::new(crate::i18n::t("Сбросить")).font(theme::body(12.5)).color(theme::ink3())).sense(Sense::click())).clicked() {
                    state::get().set_effects(|e| *e = crate::store::state::Effects { keep: e.keep, ..Default::default() });
                    changed = true;
                }
            });
        });
        if changed {
            self.player.apply_sound();
        }
    }

    fn equalizer_ui(&mut self, ui: &mut Ui) {
        let e = state::get().eq.read().unwrap().clone();
        let mut set: Option<Box<dyn FnOnce(&mut EqState)>> = None;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Эквалайзер")).font(theme::medium(14.0)).color(INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut on = e.enabled;
                if widgets::toggle(ui, &mut on).changed() {
                    set = Some(Box::new(move |s| s.enabled = on));
                }
            });
        });
        ui.add_space(4.0);
        // presets: the built-in ones, then the person's own of this mode
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (id, name, _) in eq::PRESETS {
                if widgets::tab(ui, name, e.enabled && e.preset == id).clicked() {
                    let id = id.to_string();
                    set = Some(Box::new(move |s| s.apply_preset(&id)));
                }
            }
            for p in e.custom_presets.iter().filter(|p| (p.bands.len() > 6) == e.advanced()) {
                if widgets::tab(ui, &p.name, e.enabled && e.preset == p.id).clicked() {
                    let id = p.id.clone();
                    set = Some(Box::new(move |s| s.apply_custom(&id)));
                }
            }
        });
        ui.add_space(6.0);
        // the curve
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 150.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(12), Color32::from_rgba_unmultiplied(0, 0, 0, 60));
        let y_of = |db: f64| rect.center().y - (db / eq::MAX_GAIN) as f32 * (rect.height() / 2.0 - 10.0);
        for db in [-12.0, 0.0, 12.0] {
            p.hline(rect.x_range(), y_of(db), Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, if db == 0.0 { 40 } else { 18 })));
        }
        for f in [100.0, 1000.0, 10000.0] {
            let x = rect.left() + eq::freq_to_x(f, rect.width() as f64) as f32;
            p.vline(x, rect.y_range(), Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 14)));
            p.text(pos2(x + 3.0, rect.bottom() - 4.0), Align2::LEFT_BOTTOM, if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{f}") }, theme::body(10.0), theme::ink3());
        }
        let curve: Vec<egui::Pos2> = (0..=96)
            .map(|i| {
                let x = rect.width() as f64 * i as f64 / 96.0;
                let f = eq::x_to_freq(x, rect.width() as f64);
                let db = if e.enabled { eq::total_response_db(&e.bands, f, 48000.0) } else { 0.0 };
                pos2(rect.left() + x as f32, y_of(db.clamp(-eq::MAX_GAIN, eq::MAX_GAIN)))
            })
            .collect();
        p.add(egui::Shape::line(curve, Stroke::new(2.0, if e.enabled { ACCENT } else { theme::ink3() })));
        // the points: up-down is the level, left-right the frequency (six-band mode)
        for (i, b) in e.bands.iter().enumerate() {
            let x = rect.left() + eq::freq_to_x(b.frequency, rect.width() as f64) as f32;
            let c = pos2(x, y_of(b.gain));
            let hit = Rect::from_center_size(c, vec2(20.0, 20.0));
            let resp = ui.interact(hit, ui.scope_id().with(("band", i)), Sense::drag());
            let color = BAND_COLORS[i % BAND_COLORS.len()];
            ui.painter().circle_filled(c, if resp.hovered() || resp.dragged() { 7.5 } else { 6.0 }, if e.enabled { color } else { theme::ink3() });
            if resp.dragged() {
                if let Some(ptr) = resp.interact_pointer_pos() {
                    let gain = (((rect.center().y - ptr.y) / (rect.height() / 2.0 - 10.0)) as f64 * eq::MAX_GAIN).clamp(-eq::MAX_GAIN, eq::MAX_GAIN);
                    let gain = (gain * 2.0).round() / 2.0;
                    let freq = if e.advanced() { b.frequency } else { eq::x_to_freq((ptr.x - rect.left()) as f64, rect.width() as f64).round() };
                    let id = b.id.clone();
                    set = Some(Box::new(move |s| {
                        if let Some(band) = s.bands.iter_mut().find(|x| x.id == id) {
                            band.gain = gain;
                            band.frequency = freq;
                        }
                        s.preset = "custom".into();
                        s.enabled = true;
                    }));
                }
            }
            if resp.hovered() || resp.dragged() {
                let f = if b.frequency >= 1000.0 { format!("{:.1} кГц", b.frequency / 1000.0) } else { format!("{:.0} Гц", b.frequency) };
                ui.painter().text(c + vec2(0.0, -14.0), Align2::CENTER_BOTTOM, format!("{f} · {:+.1} дБ", b.gain), theme::medium(11.0), INK);
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if let Some(i) = widgets::tabs(ui, &["6 полос", "10 полос"], if e.advanced() { 1 } else { 0 }) {
                set = Some(Box::new(move |s| s.set_mode(i == 1)));
            }
        });
        let mut pre = ((e.preamp - eq::MIN_PREAMP) / (eq::MAX_PREAMP - eq::MIN_PREAMP)) as f32;
        if slider_row(ui, "Предусиление", format!("{:+.1} дБ", e.preamp), &mut pre) {
            let v = ((eq::MIN_PREAMP + pre as f64 * (eq::MAX_PREAMP - eq::MIN_PREAMP)) * 2.0).round() / 2.0;
            set = Some(Box::new(move |s| s.preamp = v));
        }
        if let Some(v) = switch_row(ui, "Выравнивать громкость треков", e.normalize) {
            set = Some(Box::new(move |s| s.normalize = v));
        }
        if let Some(v) = switch_row(ui, "Защита от перегрузки", e.protect) {
            set = Some(Box::new(move |s| s.protect = v));
        }
        if let Some(f) = set {
            state::get().set_eq(f);
            self.player.apply_sound();
        }
    }
}
