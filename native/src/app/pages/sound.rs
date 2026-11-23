//! The equalizer of Настройки → Звук и эквалайзер, as settings/EqualizerSection.tsx and
//! equalizer/EqualizerGraph.tsx have it: on and off, the mode, clip protection and volume
//! levelling, the presets and your own (save, rename, share as a code, delete, reorder), the
//! graph with its bands to drag, the chosen band's type, frequency, gain and width, and the
//! preamp. The speed, pitch and reverb are in the player's own panel (app::sound_panel).

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui};

use super::kit;
use crate::app::App;
use crate::audio::dsp::eq::{self, BandType, EqState};
use crate::state;
use crate::ui::theme::{self, INK};
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

/// the graph (EqualizerGraph.tsx): 260 high, ±27 dB in view, 14 px padding top and bottom
const HEIGHT: f32 = 260.0;
const PAD_Y: f32 = 14.0;
const RANGE_DB: f64 = 27.0;
const GRID_FREQS: [f64; 10] = [30.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];
const GRID_DB: [f64; 5] = [-24.0, -12.0, 0.0, 12.0, 24.0];
/// points along the curves
const POINTS: usize = 220;

/// the name / code dialog of the presets
#[derive(Clone)]
pub enum PresetDialog {
    Save(String),
    Rename(String, String),
    Import(String, Option<String>),
}

fn db_to_y(top: f32, db: f64) -> f32 {
    top + PAD_Y + ((RANGE_DB - db) / (2.0 * RANGE_DB)) as f32 * (HEIGHT - 2.0 * PAD_Y)
}

fn y_to_db(top: f32, y: f32) -> f64 {
    RANGE_DB - ((y - top - PAD_Y) / (HEIGHT - 2.0 * PAD_Y)) as f64 * 2.0 * RANGE_DB
}

fn format_frequency(f: f64) -> String {
    if f >= 1000.0 {
        let k = f / 1000.0;
        if (k - k.round()).abs() < 0.05 {
            format!("{} кГц", k.round())
        } else {
            format!("{k:.1} кГц")
        }
    } else {
        format!("{} Гц", f.round())
    }
}

type Change = Box<dyn FnOnce(&mut EqState)>;

impl App {
    /// the equalizer (Настройки → Звук и эквалайзер, as in the Tauri build)
    pub fn equalizer_ui(&mut self, ui: &mut Ui) {
        let e = state::get().eq.read().unwrap().clone();
        let mut set: Option<Change> = None;
        let simple = !e.advanced();
        // .settings-group: 14 between the parts
        ui.spacing_mut().item_spacing.y = 14.0;
        let hint = format!("{} {}", e.bands.len(), crate::i18n::t("полос: перетаскивайте точки на графике вверх-вниз (громкость) и влево-вправо (частота)"));
        if let Some(v) = kit::switch_row(ui, "Эквалайзер", &hint, e.enabled, false) {
            set = Some(Box::new(move |s| s.enabled = v));
        }
        kit::row(ui, "Режим", if simple { "Шесть полос, каждую можно поставить на любую частоту" } else { "Десять полос на классических частотах — от 31 Гц до 16 кГц" }, |ui| {
            // right to left: the second chip first
            if kit::chip(ui, "Продвинутый", !simple).clicked() {
                set = Some(Box::new(|s| s.set_mode(true)));
            }
            if kit::chip(ui, "Обычный", simple).clicked() {
                set = Some(Box::new(|s| s.set_mode(false)));
            }
        });
        if let Some(v) = kit::switch_row(ui, "Защита от хрипа", "Приглушает общий уровень на величину усиления и срезает пики. Выключена: бас-буст и любое усиление звучат в полную силу, на громких треках возможен перегруз", e.protect, false) {
            set = Some(Box::new(move |s| s.protect = v));
        }
        if let Some(v) = kit::switch_row(ui, "Выравнивать громкость", "Тихие и громкие треки звучат примерно одинаково — без резких скачков между песнями", e.normalize, false) {
            set = Some(Box::new(move |s| s.normalize = v));
        }
        // the built-in presets, "Свой" while the curve is changed by hand, and "Сбросить"
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (id, name, _) in eq::PRESETS {
                if kit::chip(ui, name, e.preset == id).clicked() {
                    set = Some(Box::new(move |s| s.apply_preset(id)));
                }
            }
            if e.preset == "custom" {
                kit::chip(ui, "Свой", true);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::text_button(ui, "Сбросить").clicked() {
                    set = Some(Box::new(|s| s.reset()));
                }
            });
        });
        self.custom_presets(ui, &e, &mut set);
        let selected = self.pages.eq_band.clone().filter(|id| e.bands.iter().any(|b| &b.id == id)).or_else(|| e.bands.first().map(|b| b.id.clone()));
        self.eq_graph(ui, &e, selected.as_deref(), &mut set);
        kit::note(ui, "Колесо мыши над точкой — ширина полосы, двойной клик — обнулить, Shift — двигать только по высоте.");
        if let Some((index, band)) = selected.as_ref().and_then(|id| e.bands.iter().enumerate().find(|(_, b)| &b.id == id)) {
            band_card(ui, index, band, &mut set);
        }
        let pre = e.preamp;
        kit::row(ui, "Предусиление", "Общий уровень после эквалайзера. Если звук хрипит — опустите его или включите защиту от хрипа", |ui| {
            ui.add_sized(vec2(64.0, 20.0), egui::Label::new(egui::RichText::new(format!("{}{:.1} дБ", if pre > 0.0 { "+" } else { "" }, pre)).font(theme::body(12.5)).color(theme::ink3())));
            if let Some(v) = kit::dial(ui, 160.0, pre, eq::MIN_PREAMP, eq::MAX_PREAMP, 0.5) {
                set = Some(Box::new(move |s| s.preamp = v));
            }
        });
        self.preset_dialog(ui.ctx(), &mut set);
        if let Some(f) = set {
            state::get().set_eq(f);
            self.player.apply_sound();
        }
    }

    /// "Свои пресеты": this mode's own presets with their menus, "+ Сохранить текущие" and
    /// "Вставить код"
    fn custom_presets(&mut self, ui: &mut Ui, e: &EqState, set: &mut Option<Change>) {
        let simple = !e.advanced();
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            kit::eyebrow(ui, &format!("{} · {}", crate::i18n::t("Свои пресеты"), crate::i18n::t(if simple { "обычный режим" } else { "продвинутый режим" })));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                let own: Vec<&eq::CustomPreset> = e.custom_presets.iter().filter(|p| (p.bands.len() > 6) == !simple).collect();
                for (i, p) in own.iter().enumerate() {
                    let on = e.preset == p.id;
                    // a chip with the name, and "…" for its menu
                    let g = ui.painter().layout_no_wrap(p.name.clone(), theme::medium(12.5), Color32::PLACEHOLDER);
                    let (rect, r) = ui.allocate_exact_size(vec2(g.size().x + 13.0 + 6.0 + 14.0 + 7.0, 30.0), Sense::click_and_drag());
                    let more = Rect::from_min_max(pos2(rect.right() - 7.0 - 14.0 - 4.0, rect.top()), rect.max);
                    let over_more = r.hover_pos().is_some_and(|p| more.contains(p));
                    let (fill, fg) = if on {
                        (INK, theme::GROUND)
                    } else if r.hovered() {
                        (Color32::from_rgba_unmultiplied(238, 247, 244, 36), INK)
                    } else {
                        (Color32::from_rgba_unmultiplied(238, 247, 244, 20), theme::ink2())
                    };
                    ui.painter().rect_filled(rect, 15, fill);
                    ui.painter().galley(pos2(rect.left() + 13.0, rect.center().y - g.size().y / 2.0), g, fg);
                    crate::ui::icons::paint(ui.painter(), "more", Rect::from_center_size(pos2(rect.right() - 7.0 - 7.0, rect.center().y), vec2(14.0, 14.0)), fg.gamma_multiply(if over_more { 1.0 } else { 0.7 }));
                    if r.clicked() {
                        if over_more {
                            self.pages.eq_menu = Some((p.id.clone(), pos2(rect.left(), rect.bottom() + 4.0)));
                        } else {
                            let id = p.id.clone();
                            *set = Some(Box::new(move |s| s.apply_custom(&id)));
                        }
                    }
                    // dragged left or right past a neighbour: the order changes
                    if r.dragged() {
                        if let Some(at) = r.interact_pointer_pos() {
                            let to = if at.x > rect.right() + 10.0 && i + 1 < own.len() { Some(i + 1) } else if at.x < rect.left() - 10.0 && i > 0 { Some(i - 1) } else { None };
                            if let Some(to) = to {
                                *set = Some(Box::new(move |s| s.move_preset(i, to)));
                            }
                        }
                    }
                }
                if kit::chip_add(ui, "+ Сохранить текущие").clicked() {
                    self.pages.eq_dialog = Some(PresetDialog::Save(String::new()));
                }
                if kit::chip_add(ui, "Вставить код").clicked() {
                    self.pages.eq_dialog = Some(PresetDialog::Import(String::new(), None));
                }
            });
        });
        // the menu of a preset
        if let Some((id, at)) = self.pages.eq_menu.clone() {
            let Some(p) = e.custom_presets.iter().find(|p| p.id == id).cloned() else {
                self.pages.eq_menu = None;
                return;
            };
            let mut open = true;
            let entries = [
                widgets::MenuEntry { icon: None, label: crate::i18n::t("Сохранить текущие настройки сюда"), danger: false },
                widgets::MenuEntry { icon: None, label: crate::i18n::t("Переименовать"), danger: false },
                widgets::MenuEntry { icon: None, label: crate::i18n::t("Скопировать код пресета"), danger: false },
                widgets::MenuEntry { icon: None, label: crate::i18n::t("Удалить"), danger: true },
            ];
            let anchor = Rect::from_min_size(at - vec2(0.0, 6.0), vec2(1.0, 1.0));
            match widgets::drop_menu(ui, "eq-preset-menu", anchor, &mut open, &entries) {
                Some(0) => {
                    let id = p.id.clone();
                    *set = Some(Box::new(move |s| s.overwrite_preset(&id)));
                    self.toast(&format!("«{}» {}", p.name, crate::i18n::t("обновлён")));
                }
                Some(1) => self.pages.eq_dialog = Some(PresetDialog::Rename(p.id.clone(), p.name.clone())),
                Some(2) => {
                    ui.ctx().copy_text(eq::encode_preset_code(&p.name, &p.bands, p.preamp));
                    self.toast("Код скопирован — отправьте его другу");
                }
                Some(3) => {
                    let id = p.id.clone();
                    *set = Some(Box::new(move |s| s.delete_preset(&id)));
                }
                _ => {}
            }
            if !open {
                self.pages.eq_menu = None;
            }
        }
    }

    /// NameDialog: a name for a new or renamed preset, or a code to paste
    fn preset_dialog(&mut self, ctx: &egui::Context, set: &mut Option<Change>) {
        let Some(d) = self.pages.eq_dialog.clone() else { return };
        let (title, mut value, error) = match &d {
            PresetDialog::Save(v) => ("Новый пресет", v.clone(), None),
            PresetDialog::Rename(_, v) => ("Переименовать пресет", v.clone(), None),
            PresetDialog::Import(v, err) => ("Пресет по коду", v.clone(), err.clone()),
        };
        let multiline = matches!(d, PresetDialog::Import(..));
        let mut close = false;
        let mut submit = false;
        egui::Modal::new(egui::Id::unique("eq-preset-dialog"))
            .backdrop_color(Color32::from_rgba_unmultiplied(2, 8, 9, 140))
            .frame(egui::Frame::new().fill(Color32::from_rgba_unmultiplied(8, 22, 24, 240)).stroke(Stroke::new(1.0, theme::edge())).corner_radius(26).inner_margin(22))
            .show(ctx, |ui| {
                ui.set_width(396.0);
                ui.spacing_mut().item_spacing.y = 14.0;
                ui.label(egui::RichText::new(crate::i18n::t(title)).font(theme::heavy(19.0)).color(INK));
                let r = if multiline {
                    ui.add(egui::TextEdit::multiline(&mut value).hint_text("EVQ1.…").desired_rows(3).desired_width(f32::INFINITY).font(egui::FontId::new(14.0, theme::medium500())))
                } else {
                    let r = kit::input_styled(ui, &mut value, "", 396.0, 40.0, 12, theme::medium500(), false);
                    if value.chars().count() > 32 {
                        value = value.chars().take(32).collect();
                    }
                    r
                };
                if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
                    r.request_focus();
                }
                if !multiline && r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if let Some(err) = &error {
                    kit::error(ui, &crate::i18n::t(err));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if kit::btn_if(ui, !value.trim().is_empty(), None, "Готово", true).clicked() {
                        submit = true;
                    }
                    if widgets::btn(ui, None, "Отмена", false).clicked() {
                        close = true;
                    }
                });
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        let mut next = match d.clone() {
            PresetDialog::Save(_) => PresetDialog::Save(value.clone()),
            PresetDialog::Rename(id, _) => PresetDialog::Rename(id, value.clone()),
            PresetDialog::Import(_, e) => PresetDialog::Import(value.clone(), e),
        };
        if submit && !value.trim().is_empty() {
            match &d {
                PresetDialog::Save(_) => {
                    let name = value.clone();
                    let shown: String = name.trim().chars().take(32).collect();
                    *set = Some(Box::new(move |s| s.save_preset(&name)));
                    self.toast(&format!("{} «{}» {}", crate::i18n::t("Пресет"), shown, crate::i18n::t("сохранён")));
                    close = true;
                }
                PresetDialog::Rename(id, _) => {
                    let (id, name) = (id.clone(), value.clone());
                    *set = Some(Box::new(move |s| s.rename_preset(&id, &name)));
                    close = true;
                }
                PresetDialog::Import(..) => match eq::decode_preset_code(&value) {
                    Some((name, bands, preamp)) => {
                        let shown = name.clone();
                        *set = Some(Box::new(move |s| {
                            s.import_preset(name, bands, preamp);
                        }));
                        self.toast(&format!("{} «{}» {}", crate::i18n::t("Пресет"), shown, crate::i18n::t("добавлен")));
                        close = true;
                    }
                    None => next = PresetDialog::Import(value.clone(), Some("Код повреждён — скопируйте его целиком".into())),
                },
            }
        }
        self.pages.eq_dialog = if close { None } else { Some(next) };
    }

    /// EqualizerGraph: the grid, each band's own curve, the sum in white, and the points
    fn eq_graph(&mut self, ui: &mut Ui, e: &EqState, selected: Option<&str>, set: &mut Option<Change>) {
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, HEIGHT), Sense::hover());
        let alpha = if e.enabled { 1.0 } else { 0.6 };
        let p = ui.painter().with_clip_rect(rect);
        p.rect(rect, CornerRadius::same(18), Color32::from_rgba_unmultiplied(0, 0, 0, 140), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let faint = Color32::from_rgba_unmultiplied(255, 255, 255, (15.0 * alpha) as u8);
        let label = Color32::from_rgba_unmultiplied(255, 255, 255, (89.0 * alpha) as u8);
        let x_of = |f: f64| rect.left() + eq::freq_to_x(f, w as f64) as f32;
        for f in GRID_FREQS {
            let x = x_of(f);
            p.vline(x, rect.y_range(), Stroke::new(1.0, faint));
            let text = if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{f}") };
            p.text(pos2(x + 3.0, rect.bottom() - 4.0), Align2::LEFT_BOTTOM, text, theme::body(9.0), label);
        }
        for db in GRID_DB {
            let y = db_to_y(rect.top(), db);
            let c = if db == 0.0 { Color32::from_rgba_unmultiplied(255, 255, 255, (41.0 * alpha) as u8) } else { faint };
            p.hline(rect.x_range(), y, Stroke::new(1.0, c));
            p.text(pos2(rect.right() - 4.0, y - 3.0), Align2::RIGHT_BOTTOM, if db > 0.0 { format!("+{db}") } else { format!("{db}") }, theme::body(9.0), label);
        }
        let xs: Vec<f32> = (0..=POINTS).map(|i| i as f32 / POINTS as f32 * w).collect();
        let freqs: Vec<f64> = xs.iter().map(|x| eq::x_to_freq(*x as f64, w as f64)).collect();
        let zero = db_to_y(rect.top(), 0.0);
        // each band's own curve, filled down to 0 dB, brighter for the chosen one
        for (i, b) in e.bands.iter().enumerate() {
            if !b.enabled || b.gain == 0.0 {
                continue;
            }
            let color = BAND_COLORS[i % BAND_COLORS.len()];
            let fill_a = if selected == Some(b.id.as_str()) { 0.32 } else { 0.16 };
            let pts: Vec<egui::Pos2> = xs.iter().zip(&freqs).map(|(x, f)| pos2(rect.left() + x, db_to_y(rect.top(), eq::band_response_db(b, *f, 48000.0)))).collect();
            // the fill as thin strips between the curve and the zero line (the curve need not be convex)
            let mut mesh = egui::Mesh::default();
            let fc = color.gamma_multiply(fill_a * alpha);
            for (k, pt) in pts.iter().enumerate() {
                mesh.colored_vertex(*pt, fc);
                mesh.colored_vertex(pos2(pt.x, zero), fc);
                if k > 0 {
                    let a = (k as u32 - 1) * 2;
                    mesh.add_triangle(a, a + 1, a + 2);
                    mesh.add_triangle(a + 1, a + 3, a + 2);
                }
            }
            p.add(egui::Shape::mesh(mesh));
            p.add(egui::Shape::line(pts, Stroke::new(1.0, color.gamma_multiply(0.55 * alpha))));
        }
        let total: Vec<egui::Pos2> = xs.iter().zip(&freqs).map(|(x, f)| pos2(rect.left() + x, db_to_y(rect.top(), eq::total_response_db(&e.bands, *f, 48000.0)))).collect();
        p.add(egui::Shape::line(total, Stroke::new(2.0, Color32::WHITE.gamma_multiply(alpha))));
        // the points: drag to move (Shift keeps the frequency), the wheel changes the width of a
        // peak, a double click puts the band back to 0 dB
        let shift = ui.input(|i| i.modifiers.shift);
        for (i, b) in e.bands.iter().enumerate() {
            let c = pos2(x_of(b.frequency), db_to_y(rect.top(), if b.enabled { b.gain } else { 0.0 }));
            let hit = Rect::from_center_size(c, vec2(32.0, 32.0));
            let resp = ui.interact(hit, ui.scope_id().with(("eq-band", &b.id)), Sense::click_and_drag());
            let color = BAND_COLORS[i % BAND_COLORS.len()];
            let chosen = selected == Some(b.id.as_str());
            if chosen {
                p.circle_stroke(c, 11.0, Stroke::new(2.0, color.gamma_multiply(0.45 * alpha)));
            }
            p.circle(c, 7.0, if b.enabled { color.gamma_multiply(alpha) } else { Color32::from_rgb(0x55, 0x55, 0x55) }, Stroke::new(2.0, Color32::from_rgb(0x0b, 0x0b, 0x0c)));
            p.text(c + vec2(0.0, 0.5), Align2::CENTER_CENTER, (i + 1).to_string(), theme::heavy(8.0), Color32::from_rgb(0x0b, 0x0b, 0x0c));
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
            }
            if resp.drag_started() || resp.clicked() {
                self.pages.eq_band = Some(b.id.clone());
            }
            let id = b.id.clone();
            if resp.double_clicked() {
                *set = Some(Box::new(move |s| {
                    s.enabled = true;
                    s.update_band(&id, |b| b.gain = 0.0);
                }));
                continue;
            }
            if resp.dragged() {
                if let Some(ptr) = resp.interact_pointer_pos() {
                    let x = (ptr.x - rect.left()).clamp(0.0, w);
                    let y = ptr.y.clamp(rect.top(), rect.bottom());
                    let gain = y_to_db(rect.top(), y).clamp(-eq::MAX_GAIN, eq::MAX_GAIN);
                    let freq = eq::x_to_freq(x as f64, w as f64);
                    *set = Some(Box::new(move |s| {
                        s.enabled = true;
                        s.update_band(&id, |b| {
                            b.gain = gain;
                            if !shift {
                                b.frequency = freq;
                            }
                        });
                    }));
                }
                continue;
            }
            if resp.hovered() && b.kind == BandType::Peaking {
                let scroll: f32 = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::MouseWheel { delta, .. } = e { Some(delta.y) } else { None }).sum());
                if scroll != 0.0 {
                    let factor = if scroll > 0.0 { 1.12 } else { 1.0 / 1.12 };
                    let q = (b.q * factor).clamp(eq::MIN_Q, eq::MAX_Q);
                    *set = Some(Box::new(move |s| {
                        s.enabled = true;
                        s.update_band(&id, |b| b.q = q);
                    }));
                }
            }
        }
    }
}

/// .band-card: the chosen band's colour, number and frequency, on and off, its type, and its
/// frequency, gain and width typed in
fn band_card(ui: &mut Ui, index: usize, b: &eq::Band, set: &mut Option<Change>) {
    egui::Frame::new().fill(Color32::from_rgba_unmultiplied(238, 247, 244, 13)).corner_radius(18).inner_margin(14).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 5.0, BAND_COLORS[index % BAND_COLORS.len()]);
            ui.label(egui::RichText::new(format!("{} {} · {}", crate::i18n::t("Полоса"), index + 1, format_frequency(b.frequency))).font(theme::medium(13.5)).color(INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut on = b.enabled;
                if kit::switch(ui, &mut on).changed() {
                    let id = b.id.clone();
                    *set = Some(Box::new(move |s| s.update_band(&id, |b| b.enabled = on)));
                }
                ui.label(egui::RichText::new(crate::i18n::t("Включена")).font(theme::body(12.5)).color(theme::ink3()));
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (kind, label) in [(BandType::Lowshelf, "Низы (полка)"), (BandType::Peaking, "Пик"), (BandType::Highshelf, "Верха (полка)")] {
                if kit::chip(ui, label, b.kind == kind).clicked() {
                    let id = b.id.clone();
                    *set = Some(Box::new(move |s| s.update_band(&id, |b| b.kind = kind)));
                }
            }
        });
        let w = (ui.available_width() - 16.0) / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let fields: [(&str, f64, &str, f64, f64, f64, u8); 3] = [
                ("Частота", b.frequency, "Гц", 1.0, eq::MIN_FREQ, eq::MAX_FREQ, 0),
                ("Усиление", b.gain, "дБ", 0.5, -eq::MAX_GAIN, eq::MAX_GAIN, 1),
                ("Ширина (Q)", b.q, "", 0.1, eq::MIN_Q, eq::MAX_Q, 2),
            ];
            for (label, value, suffix, step, min, max, which) in fields {
                ui.vertical(|ui| {
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label(egui::RichText::new(crate::i18n::t(label)).font(theme::body(11.5)).color(theme::ink3()));
                    if let Some(v) = number_field(ui, &b.id, which, value, suffix, step, min, max, w) {
                        let id = b.id.clone();
                        *set = Some(Box::new(move |s| {
                            s.update_band(&id, |b| match which {
                                0 => b.frequency = v,
                                1 => b.gain = v,
                                _ => b.q = v,
                            })
                        }));
                    }
                });
            }
        });
    });
}

/// NumberField: type freely; a value in range is applied as it is typed, the box is put back in
/// range on Enter or leaving it; the arrows step it (Shift: ten steps)
#[allow(clippy::too_many_arguments)]
fn number_field(ui: &mut Ui, band: &str, which: u8, value: f64, suffix: &str, step: f64, min: f64, max: f64, w: f32) -> Option<f64> {
    let id = ui.scope_id().with(("eq-num", band, which));
    let shown = if step < 1.0 { format!("{}", (value * 100.0).round() / 100.0) } else { format!("{}", value.round()) };
    let mut draft: Option<String> = ui.data(|d| d.get_temp(id));
    let mut text = draft.clone().unwrap_or_else(|| shown.clone());
    let (rect, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
    ui.painter().rect(rect, 10, Color32::from_rgba_unmultiplied(238, 247, 244, 18), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
    let suffix_w = if suffix.is_empty() { 0.0 } else { 28.0 };
    if !suffix.is_empty() {
        ui.painter().text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, crate::i18n::t(suffix), theme::body(12.5), theme::ink3());
    }
    let inner = Rect::from_min_max(pos2(rect.left() + 10.0, rect.top()), pos2(rect.right() - 10.0 - suffix_w, rect.bottom()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let edit_id = id.with("edit");
    let r = child.add(egui::TextEdit::singleline(&mut text).id(edit_id).frame(egui::Frame::NONE).font(theme::medium(13.0)).desired_width(inner.width()).align(egui::Align2::LEFT_CENTER));
    let mut out = None;
    if r.gained_focus() {
        if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), edit_id) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()))));
            st.store(ui.ctx(), edit_id);
        }
    }
    if r.changed() {
        let clean: String = text.chars().filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-')).collect();
        if let Ok(v) = clean.replace(',', ".").parse::<f64>() {
            if v >= min && v <= max {
                out = Some(v);
            }
        }
        draft = Some(clean);
    }
    if r.has_focus() {
        let (up, down, shift) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.modifiers.shift));
        if up || down {
            let base = draft.as_deref().and_then(|d| d.replace(',', ".").parse::<f64>().ok()).unwrap_or(value);
            let k = if shift { 10.0 } else { 1.0 };
            out = Some((base + if up { step * k } else { -step * k }).clamp(min, max));
            draft = None;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            draft = None;
            r.surrender_focus();
        }
    }
    if r.lost_focus() {
        if let Some(d) = draft.take() {
            if let Ok(v) = d.replace(',', ".").parse::<f64>() {
                out = Some(v.clamp(min, max));
            }
        }
    }
    ui.data_mut(|d| match &draft {
        Some(v) => {
            d.insert_temp(id, v.clone());
        }
        None => {
            d.remove::<String>(id);
        }
    });
    out
}
