//! "Звук и эффекты": the panel over the player with the speed, the pitch, the reverb and the
//! equalizer (src/ui/components/SoundPanel.tsx).

use egui::{Rect, Ui};

use crate::app::App;
use crate::state;
use crate::ui::theme::{self, INK};
use crate::ui::widgets;

impl App {
    pub fn sound_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let area = egui::Area::new(egui::Id::unique("sound-panel")).order(egui::Order::Foreground).fixed_pos(rect.min).constrain(true);
        let r = area.show(ui.ctx(), |ui| {
            egui::Frame::new().fill(egui::Color32::from_rgb(0x0d, 0x2e, 0x2a)).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(20).inner_margin(16).show(ui, |ui| {
                ui.set_width(rect.width() - 32.0);
                ui.label(egui::RichText::new(crate::i18n::t("Звук и эффекты")).font(theme::medium(15.0)).color(INK));
                ui.add_space(8.0);
                self.effects_ui(ui);
                ui.add_space(10.0);
                self.equalizer_ui(ui);
            });
        });
        // a press outside closes it (the button that opened it toggles it by itself, below it)
        let pressed_at = ui.ctx().input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
        let outside = pressed_at.is_some_and(|p| !r.response.rect.contains(p) && p.y < rect.bottom() + 8.0);
        if outside || ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pages.sound_open = false;
        }
    }

    fn effects_ui(&mut self, ui: &mut Ui) {
        let fx = state::get().effects.read().unwrap().clone();
        let mut speed = ((fx.speed - 0.5) / 1.5) as f32;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Скорость")).color(theme::ink2()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("×{:.2}", fx.speed)).font(theme::medium(12.5)).color(INK));
                if widgets::level(ui, 170.0, &mut speed).changed() {
                    let s = (0.5 + speed as f64 * 1.5) * 20.0;
                    let s = s.round() / 20.0;
                    state::get().set_effects(|e| e.speed = s);
                    self.player.apply_speed(s);
                }
            });
        });
        ui.horizontal(|ui| {
            let mut keep = fx.keep;
            if widgets::toggle(ui, &mut keep).changed() {
                state::get().set_effects(|e| e.keep = keep);
            }
            ui.label(egui::RichText::new(crate::i18n::t("Оставлять для следующих треков")).font(theme::body(12.5)).color(theme::ink2()));
            if ui.add(egui::Label::new(egui::RichText::new(crate::i18n::t("Сбросить")).font(theme::body(12.5)).color(theme::ink3())).sense(egui::Sense::click())).clicked() {
                state::get().set_effects(|e| *e = crate::store::state::Effects { keep: e.keep, ..Default::default() });
                self.player.apply_speed(1.0);
            }
        });
    }

    fn equalizer_ui(&mut self, ui: &mut Ui) {
        App::muted(ui, "Эквалайзер — скоро здесь.");
    }
}
