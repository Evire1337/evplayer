//! "Совместное прослушивание" on screen (src/jam/JamDialog.tsx, JamBanner.tsx, JamFloat.tsx,
//! JamAsks.tsx and jam.css): the dialog to start or join a session and to run it, the banner on
//! "Сейчас играет", reactions and messages floating up in the corner, and the people asking to
//! come in. The session itself is jam/engine.rs.

use std::time::{Duration, Instant};

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Ui};

use super::kit;
use crate::app::App;
use crate::jam::engine::{Access, Control, JamStatus, View};
use crate::jam::protocol::{self, COLORS, REACTIONS};
use crate::ui::theme::{self, ic, ACCENT, INK};
use crate::ui::widgets;

const REACTION_SHOWN: Duration = Duration::from_millis(4000);
const MESSAGE_SHOWN: Duration = Duration::from_millis(6000);
/// Microsoft's Fluent Emoji (MIT), the same pictures the page shows
const EMOJI_BASE: &str = "https://cdn.jsdelivr.net/gh/microsoft/fluentui-emoji@main/assets/";

/// what the session's screens remember between frames
#[derive(Default)]
pub struct JamUi {
    pub join_tab: bool,
    pub code: String,
    pub chat: String,
    pub show_qr: bool,
    pub qr: Option<(String, egui::TextureHandle)>,
}

fn color_of(hex: &str) -> Color32 {
    let h = hex.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("6f"), 16).unwrap_or(0x6f);
    Color32::from_rgb(p(0), p(2), p(4))
}

/// "1 слушатель", "2 слушателя", "5 слушателей"
pub fn listeners(n: usize) -> String {
    let (tens, ones) = (n % 100, n % 10);
    let word = if (11..=14).contains(&tens) { "слушателей" } else if ones == 1 { "слушатель" } else if (2..=4).contains(&ones) { "слушателя" } else { "слушателей" };
    crate::i18n::t(&format!("{n} {word}"))
}

fn emoji_url(e: &str) -> Option<String> {
    let file = match e {
        "🔥" => "Fire/3D/fire_3d.png",
        "❤️" => "Red heart/3D/red_heart_3d.png",
        "👏" => "Clapping hands/Default/3D/clapping_hands_3d_default.png",
        "🎉" => "Party popper/3D/party_popper_3d.png",
        "🚀" => "Rocket/3D/rocket_3d.png",
        "😭" => "Loudly crying face/3D/loudly_crying_face_3d.png",
        "⚡" => "High voltage/3D/high_voltage_3d.png",
        "🎧" => "Headphone/3D/headphone_3d.png",
        _ => return None,
    };
    let path: Vec<String> = file.split('/').map(|s| percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()).collect();
    Some(format!("{EMOJI_BASE}{}", path.join("/")))
}

/// a round letter in a person's colour
fn avatar(p: &egui::Painter, center: egui::Pos2, size: f32, name: &str, color: &str) {
    p.circle_filled(center, size / 2.0, color_of(color));
    let letter: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    p.text(center, Align2::CENTER_CENTER, letter, theme::heavy(size * 0.45), theme::GROUND);
}

fn label(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(crate::i18n::t(text).to_uppercase()).font(theme::heavy(11.5)).color(theme::ink3()));
}

/// .set-card in the dialog: glass, a faint edge, rounded 22
fn set_card(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(theme::glass()).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(22).inner_margin(egui::Margin { left: 18, right: 18, top: 16, bottom: 16 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui);
    });
}

/// .dialog-actions: buttons at the right, 8 apart (added right to left)
fn dialog_actions(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(vec2(ui.available_width(), 40.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        add(ui);
    });
}

/// p.muted: faint text, wrapped
fn muted_text(ui: &mut Ui, text: &str, size: f32) {
    let w = ui.available_width();
    let g = kit::wrapped(ui, &crate::i18n::t(text), theme::body(size), theme::ink3(), w, size * 1.5);
    let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, theme::ink3());
}

/// a faint line in the middle
fn centered(ui: &mut Ui, text: &str) {
    let w = ui.available_width();
    let g = kit::wrapped(ui, &crate::i18n::t(text), theme::body(12.5), theme::ink3(), w, 18.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y + 24.0), Sense::hover());
    ui.painter().galley(pos2(r.center().x - g.size().x / 2.0, r.top() + 12.0), g, theme::ink3());
}

/// Toggle (.toggle-row): the title and hint on the left, the switch on the right, the whole
/// line toggles
fn toggle(ui: &mut Ui, title: &str, hint: &str, on: bool) -> Option<bool> {
    let w = ui.available_width();
    let tw = w - 42.0 - 14.0;
    let tg = kit::wrapped(ui, &crate::i18n::t(title), theme::medium(13.5), INK, tw, 18.0);
    let hg = kit::wrapped(ui, &crate::i18n::t(hint), theme::body(12.0), theme::ink3(), tw, 16.2);
    let h = (tg.size().y + 2.0 + hg.size().y).max(24.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let th = tg.size().y;
    ui.painter().galley(r.min, tg, if resp.hovered() { Color32::WHITE } else { INK });
    ui.painter().galley(r.min + vec2(0.0, th + 2.0), hg, theme::ink3());
    let sw = Rect::from_min_size(pos2(r.right() - 42.0, r.center().y - 12.0), vec2(42.0, 24.0));
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(sw));
    let mut v = on;
    let flipped = kit::switch(&mut c, &mut v).changed();
    if flipped {
        return Some(v);
    }
    resp.clicked().then_some(!on)
}

/// .jam-public-item: a line in a faint box; `body` lays it out and says if its button was pressed
fn item(ui: &mut Ui, accent: bool, body: impl FnOnce(&mut Ui) -> bool) -> bool {
    let (fill, edge) = if accent { (Color32::from_rgba_unmultiplied(111, 240, 196, 20), Color32::from_rgba_unmultiplied(111, 240, 196, 64)) } else { (Color32::from_rgba_unmultiplied(238, 247, 244, 10), theme::edge()) };
    egui::Frame::new().fill(fill).stroke(egui::Stroke::new(1.0, edge)).corner_radius(14).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| body(ui)).inner
    })
    .inner
}

/// the small buttons of the lists (.jam-public-item .btn: 32 high, 13 px)
fn small_btn(ui: &mut Ui, text: &str, primary: bool, enabled: bool) -> bool {
    let g = ui.painter().layout_no_wrap(crate::i18n::t(text), theme::medium(13.0), Color32::PLACEHOLDER);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 24.0, 32.0), if enabled { Sense::click() } else { Sense::hover() });
    let (fill, fg) = if primary {
        (if resp.hovered() && enabled { Color32::WHITE } else { INK }, theme::GROUND)
    } else {
        (Color32::from_rgba_unmultiplied(238, 247, 244, if resp.hovered() { 46 } else { 26 }), INK)
    };
    let dim = if enabled { 1.0 } else { 0.5 };
    ui.painter().rect_filled(r, 16, fill.gamma_multiply(dim));
    ui.painter().galley(r.center() - g.size() / 2.0, g, fg.gamma_multiply(dim));
    enabled && resp.clicked()
}

/// .jam-badge laid out in a row
fn badge(ui: &mut Ui, text: &str, host: bool) {
    let g = ui.painter().layout_no_wrap(crate::i18n::t(text), theme::heavy(10.5), Color32::PLACEHOLDER);
    let (r, _) = ui.allocate_exact_size(g.size() + vec2(12.0, 2.0), Sense::hover());
    paint_badge(ui, r, g, host);
}

/// .jam-badge at a place; returns its width
fn badge_at(ui: &Ui, at: egui::Pos2, text: &str, host: bool) -> f32 {
    let g = ui.painter().layout_no_wrap(crate::i18n::t(text), theme::heavy(10.5), Color32::PLACEHOLDER);
    let r = Rect::from_min_size(at, g.size() + vec2(12.0, 2.0));
    paint_badge(ui, r, g, host);
    r.width()
}

fn paint_badge(ui: &Ui, r: Rect, g: std::sync::Arc<egui::Galley>, host: bool) {
    let (bg, fg) = if host { (Color32::from_rgba_unmultiplied(111, 240, 196, 38), ACCENT) } else { (Color32::from_rgba_unmultiplied(238, 247, 244, 31), theme::ink2()) };
    ui.painter().rect_filled(r, 6, bg);
    ui.painter().galley(r.min + vec2(6.0, 1.0), g, fg);
}

impl App {
    fn emoji(&mut self, ui: &Ui, e: &str, rect: Rect) {
        let tex = emoji_url(e).and_then(|u| self.images.get(&u, rect.width(), ui.ctx().pixels_per_point()));
        match tex {
            Some(id) => {
                ui.painter().image(id, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, e, egui::FontId::proportional(rect.height() * 0.8), INK);
            }
        }
    }

    /// the session's dialog, the floating reactions and the asks; every frame
    pub fn jam_ui(&mut self, ctx: &egui::Context) {
        let Some(jam) = crate::jam::engine::get().cloned() else { return };
        let v = jam.view();
        if v.open {
            self.jam_dialog(ctx, &jam, &v);
        }
        self.jam_float(ctx, &v);
        self.jam_asks(ctx, &jam, &v);
    }

    /// JamDialog: the page's wide dialog (640 across, rounded 26), its body scrolling when the
    /// window is too low for it; Escape or a click outside folds it away
    fn jam_dialog(&mut self, ctx: &egui::Context, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        let frame = egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(8, 22, 24, 240))
            .stroke(egui::Stroke::new(1.0, theme::edge()))
            .corner_radius(26)
            .inner_margin(22)
            .shadow(egui::Shadow { offset: [0, 30], blur: 80, spread: 0, color: Color32::from_black_alpha(128) });
        let max_h = (ctx.content_rect().height() - 60.0 - 44.0 - 22.0 * 2.0).max(160.0);
        let r = egui::Modal::new(egui::Id::unique("jam-dialog")).frame(frame).backdrop_color(Color32::from_rgba_unmultiplied(2, 8, 9, 140)).show(ctx, |ui| {
            ui.set_width(596.0);
            ui.spacing_mut().item_spacing.y = 14.0;
            ui.label(egui::RichText::new(crate::i18n::t("Совместное прослушивание")).font(theme::heavy(19.0)).color(INK));
            egui::ScrollArea::vertical().id_salt("jam-dialog-body").max_height(max_h).min_scrolled_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(596.0);
                ui.spacing_mut().item_spacing.y = 14.0;
                if v.status == JamStatus::Live {
                    self.jam_session(ui, jam, v);
                } else {
                    self.jam_start(ui, jam, v);
                }
            });
        });
        if r.should_close() {
            jam.set_open(false);
        }
    }

    fn jam_start(&mut self, ui: &mut Ui, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        let busy = matches!(v.status, JamStatus::Starting | JamStatus::Joining);
        if v.waiting {
            set_card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    ui.add(egui::Spinner::new().size(16.0).color(ACCENT));
                    ui.label(egui::RichText::new(format!("{} {}…", crate::i18n::t("Организатор решает, пустить ли вас в сессию"), v.code)).font(theme::body(14.0)).color(INK));
                });
            });
            dialog_actions(ui, |ui| {
                if widgets::btn(ui, None, "Не ждать", false).clicked() {
                    let j = jam.clone();
                    std::thread::spawn(move || j.leave());
                }
            });
            return;
        }
        muted_text(ui, "Слушайте вместе с друзьями: одна очередь на всех, музыка играет одновременно, а в чат можно писать и слать реакции.", 14.0);
        set_card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            label(ui, "Как вас увидят");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
                let mut name = v.name.clone();
                let w = (ui.available_width() - 12.0 - (COLORS.len() as f32) * 30.0).max(140.0);
                if widgets::filter_input(ui, &mut name, "Ваше имя", w).changed() {
                    let name: String = name.chars().take(24).collect();
                    jam.set_profile(Some(name), None);
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for c in COLORS {
                        let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                        let grow = if resp.hovered() { 1.15 } else { 1.0 };
                        ui.painter().circle_filled(r.center(), 12.0 * grow, color_of(c));
                        if v.color == c {
                            ui.painter().circle_stroke(r.center(), 12.0 * grow - 1.0, egui::Stroke::new(2.0, Color32::WHITE));
                            crate::ui::icons::paint(ui.painter(), "check", Rect::from_center_size(r.center(), vec2(12.0, 12.0)), theme::GROUND);
                        }
                        if resp.on_hover_text(format!("{} {c}", crate::i18n::t("Цвет"))).clicked() {
                            jam.set_profile(None, Some(c.to_string()));
                        }
                    }
                });
            });
        });
        if let Some(i) = kit::segmented(ui, &["Создать сессию", "Присоединиться"], if self.pages.jam.join_tab { 1 } else { 0 }) {
            self.pages.jam.join_tab = i == 1;
            jam.browse(i == 1 && v.browse);
        }
        if !self.pages.jam.join_tab {
            self.jam_settings(ui, jam, v);
            dialog_actions(ui, |ui| {
                if kit::btn_if(ui, !busy, Some(ic::PEOPLE), if v.status == JamStatus::Starting { "Создаю…" } else { "Создать сессию" }, true).clicked() {
                    jam.start();
                }
            });
        } else {
            // .field with .jam-code-input: centred capitals, spaced
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.label(egui::RichText::new(crate::i18n::t("Код или ссылка приглашения")).font(theme::body(12.5)).color(theme::ink2()));
                let w = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(vec2(w, 38.0), Sense::hover());
                ui.painter().rect(rect, 12, Color32::from_rgba_unmultiplied(5, 16, 18, 77), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(12.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
                let r = c.add(
                    egui::TextEdit::singleline(&mut self.pages.jam.code)
                        .hint_text(egui::RichText::new("JAM-7K9P-2QXZ").color(theme::ink3()))
                        .char_limit(60)
                        .frame(egui::Frame::NONE)
                        .font(theme::heavy(14.0))
                        .align(Align2::CENTER_CENTER)
                        .desired_width(rect.width() - 24.0),
                );
                if r.changed() {
                    self.pages.jam.code = self.pages.jam.code.to_uppercase();
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.pages.jam.code.trim().is_empty() && !busy {
                    jam.join(&self.pages.jam.code.clone());
                }
            });
            dialog_actions(ui, |ui| {
                if kit::btn_if(ui, !busy && !self.pages.jam.code.trim().is_empty(), None, if v.status == JamStatus::Joining { "Вхожу…" } else { "Присоединиться" }, true).clicked() {
                    jam.join(&self.pages.jam.code.clone());
                }
            });
            // PublicList: the sessions their hosts showed to everyone
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                if let Some(on) = toggle(ui, "Открытые сессии", "Сессии, которые их организаторы показали всем", v.browse) {
                    jam.set_browse(on);
                }
                if v.browse {
                    if v.lobby.is_empty() {
                        centered(ui, "Пока никого: открытые сессии появятся здесь сами");
                    }
                    for l in &v.lobby {
                        let mut line = listeners(l.people);
                        if let Some(s) = &l.song {
                            line.push_str(&format!(" · {s}"));
                        }
                        let ask = l.access == Access::Ask;
                        let enter = item(ui, false, |ui| {
                            let w = ui.available_width() - 80.0 - if ask { 90.0 } else { 0.0 };
                            ui.vertical(|ui| {
                                ui.set_width(w);
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.label(widgets::clipped(ui, &l.host, theme::medium(14.0), INK, w));
                                ui.label(widgets::clipped(ui, &line, theme::body(12.5), theme::ink3(), w));
                            });
                            let mut pressed = false;
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                pressed = small_btn(ui, "Войти", true, !busy);
                                if ask {
                                    badge(ui, "по запросу", false);
                                }
                            });
                            pressed
                        });
                        if enter {
                            jam.join(&l.code);
                        }
                    }
                }
            });
        }
    }

    /// SessionSettings: who controls the music, who gets in, the public list
    fn jam_settings(&mut self, ui: &mut Ui, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        let live = v.status == JamStatus::Live;
        let control = if live { v.control } else { v.setup.control };
        let access = if live { v.access } else { v.setup.access };
        let listed = if live { v.listed } else { v.setup.listed };
        set_card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 14.0;
            let hint = if control == Control::Anyone { "Друзья включают треки, переключают их и ставят паузу" } else { "Управляете только вы; друзья добавляют песни в очередь, а своё у них не играет" };
            if let Some(on) = toggle(ui, "Все участники управляют музыкой", hint, control == Control::Anyone) {
                jam.set_control(if on { Control::Anyone } else { Control::Host });
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.label(egui::RichText::new(crate::i18n::t("Вход")).font(theme::medium(14.0)).color(INK));
                let sel = match access {
                    Access::Open => 0,
                    Access::Ask => 1,
                    Access::Closed => 2,
                };
                if let Some(i) = kit::segmented(ui, &["Открыт", "По запросу", "Закрыт"], sel) {
                    jam.set_access([Access::Open, Access::Ask, Access::Closed][i]);
                }
                ui.label(egui::RichText::new(crate::i18n::t(match access {
                    Access::Open => "Заходит любой, у кого есть код, ссылка или QR-код",
                    Access::Ask => "Каждый, кто пришёл по коду, ждёт, пока вы его пустите",
                    Access::Closed => "Новые люди не войдут ни по коду, ни по ссылке",
                })).font(theme::body(12.5)).color(theme::ink3()));
            });
            let hint = if access == Access::Closed { "Закрытая сессия в списке не видна" } else { "Сессию увидят все, кто открыл «Присоединиться», вместе с числом участников" };
            if let Some(on) = toggle(ui, "Показывать в общем списке", hint, listed) {
                jam.set_listed(on);
            }
        });
    }

    fn jam_qr(&mut self, ui: &mut Ui, code: &str) {
        let link = protocol::invite_link(code);
        if self.pages.jam.qr.as_ref().is_none_or(|q| q.0 != link) {
            let Ok(qr) = qrcode::QrCode::new(link.as_bytes()) else { return };
            let n = qr.width();
            let margin = 1;
            let size = n + margin * 2;
            let (dark, light) = ([0x06u8, 0x20, 0x1d, 255], [0xeeu8, 0xf7, 0xf4, 255]);
            let mut px = Vec::with_capacity(size * size * 4);
            for y in 0..size {
                for x in 0..size {
                    let inside = x >= margin && y >= margin && x < n + margin && y < n + margin;
                    let on = inside && qr[(x - margin, y - margin)] == qrcode::Color::Dark;
                    px.extend_from_slice(if on { &dark } else { &light });
                }
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([size, size], &px);
            self.pages.jam.qr = Some((link.clone(), ui.ctx().load_texture("jam-qr", img, egui::TextureOptions::NEAREST)));
        }
        if let Some((_, tex)) = &self.pages.jam.qr {
            // .jam-qr: a dark box with the code 180 across and a line under it
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(5, 16, 18, 128)).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(16).inner_margin(14).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), vec2(180.0, 180.0))).corner_radius(12));
                    ui.label(egui::RichText::new(crate::i18n::t("Наведите камеру телефона с EvPlayer")).font(theme::body(12.5)).color(theme::ink3()));
                });
            });
        }
    }

    fn jam_session(&mut self, ui: &mut Ui, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        let host_name = v.people.iter().find(|p| p.host).map(|p| p.name.clone()).unwrap_or_default();
        // .jam-room: an accent-tinted card with the live dot, who leads, and the code
        let w = ui.available_width();
        let room = egui::Frame::new().stroke(egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(111, 240, 196, 64))).corner_radius(22).inner_margin(egui::Margin { left: 18, right: 18, top: 16, bottom: 16 });
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = room.show(ui, |ui| {
            ui.set_width(w - 36.0);
            ui.spacing_mut().item_spacing.y = 12.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                // the live dot
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                let t = ui.ctx().input(|i| i.time) as f32;
                let saver = crate::state::get().settings.read().unwrap().memory_saver;
                let k = if saver { 0.0 } else { 0.5 - 0.5 * (t * std::f32::consts::TAU / 1.8).cos() };
                ui.painter().circle_filled(r.center(), 9.0 * (1.0 + 0.35 * k), ACCENT.gamma_multiply(0.18 * (1.0 - 0.45 * k)));
                ui.painter().circle_filled(r.center(), 5.0 * (1.0 + 0.35 * k), ACCENT.gamma_multiply(1.0 - 0.45 * k));
                if !saver {
                    ui.ctx().request_repaint_after(Duration::from_millis(66));
                }
                let who = if v.host { crate::i18n::t("Вы организатор") } else { format!("{} {host_name}", crate::i18n::t("Организатор:")) };
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(egui::RichText::new(crate::i18n::t("Сессия идёт")).font(theme::heavy(14.0)).color(ACCENT));
                    ui.label(egui::RichText::new(who).font(theme::body(12.0)).color(theme::ink2()));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // .jam-code: the code and its buttons in a dark capsule
                    let code_g = {
                        let job = egui::text::LayoutJob::single_section(v.code.clone(), egui::TextFormat { font_id: egui::FontId::monospace(16.0), color: INK, extra_letter_spacing: 2.0, ..Default::default() });
                        ui.painter().layout_job(job)
                    };
                    let n_btn = if v.host { 4.0 } else { 3.0 };
                    let cw = 12.0 + code_g.size().x + 4.0 + 4.0 + n_btn * 34.0 + 6.0;
                    let (cap, _) = ui.allocate_exact_size(vec2(cw, 42.0), Sense::hover());
                    ui.painter().rect(cap, 14, Color32::from_rgba_unmultiplied(5, 16, 18, 128), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
                    ui.painter().galley(pos2(cap.left() + 12.0, cap.center().y - code_g.size().y / 2.0), code_g.clone(), INK);
                    let mut x = cap.left() + 12.0 + code_g.size().x + 8.0;
                    let mut buttons: Vec<(&str, &str, u8)> = vec![("copy", "Копировать код", 0), ("link", "Копировать ссылку", 1), ("qr", "QR-код", 2)];
                    if v.host {
                        buttons.push(("refresh", "Новый код, ссылка и QR: старые перестанут работать", 3));
                    }
                    for (icon, tip, what) in buttons {
                        let b = Rect::from_center_size(pos2(x + 15.0, cap.center().y), vec2(30.0, 30.0));
                        x += 34.0;
                        let r = ui.interact(b, ui.scope_id().with(("jam-code", what)), Sense::click());
                        let on = what == 2 && self.pages.jam.show_qr;
                        if r.hovered() || on {
                            ui.painter().circle_filled(b.center(), 15.0, theme::hover());
                        }
                        crate::ui::icons::paint(ui.painter(), icon, Rect::from_center_size(b.center(), vec2(16.0, 16.0)), if r.hovered() || on { INK } else { theme::ink2() });
                        if r.on_hover_text(crate::i18n::t(tip)).clicked() {
                            match what {
                                0 => {
                                    ui.ctx().copy_text(v.code.clone());
                                    self.toast("Код скопирован");
                                }
                                1 => {
                                    ui.ctx().copy_text(protocol::invite_link(&v.code));
                                    self.toast("Ссылка-приглашение скопирована");
                                }
                                2 => self.pages.jam.show_qr = !self.pages.jam.show_qr,
                                _ => {
                                    let j = jam.clone();
                                    std::thread::spawn(move || j.change_code());
                                }
                            }
                        }
                    }
                });
            });
            if self.pages.jam.show_qr {
                self.jam_qr(ui, &v.code);
            }
        });
        // linear-gradient(135deg, rgba(31,138,115,.2), rgba(5,16,18,.45)) under it
        let rr = inner.response.rect;
        ui.painter().set(bg, kit::gradient_shape(ui.ctx(), rr, 22.0, Color32::from_rgba_unmultiplied(31, 138, 115, 51), Color32::from_rgba_unmultiplied(5, 16, 18, 115), 135.0));
        // .jam-reactions
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for e in REACTIONS {
                let (r, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
                let fill = if resp.is_pointer_button_down_on() { Color32::from_rgba_unmultiplied(111, 240, 196, 51) } else if resp.hovered() { Color32::from_rgba_unmultiplied(238, 247, 244, 36) } else { Color32::from_rgba_unmultiplied(238, 247, 244, 15) };
                let grow = if resp.is_pointer_button_down_on() { 0.92 } else if resp.hovered() { 1.15 } else { 1.0 };
                ui.painter().circle(r.center(), 19.0 * grow, fill, egui::Stroke::new(1.0, theme::edge()));
                self.emoji(ui, e, Rect::from_center_size(r.center(), vec2(24.0 * grow, 24.0 * grow)));
                if resp.on_hover_text(format!("{} {e}", crate::i18n::t("Отправить"))).clicked() {
                    jam.react(e);
                }
            }
        });
        // .jam-columns: the people (1) and the chat (1.35), 12 apart
        let me = v.people.iter().find(|p| p.name == v.name).map(|p| p.id.clone()).unwrap_or_default();
        let total = ui.available_width();
        let left = (total - 12.0) / 2.35;
        let right = total - 12.0 - left;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.vertical(|ui| {
                ui.set_width(left);
                ui.spacing_mut().item_spacing.y = 8.0;
                label(ui, &format!("{} · {}", crate::i18n::t("Участники"), v.people.len()));
                egui::ScrollArea::vertical().id_salt("jam-people").max_height(240.0).min_scrolled_height(240.0).auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    for p in &v.people {
                        let can_kick = v.host && !p.host;
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), if can_kick { Sense::click() } else { Sense::hover() });
                        let hot = can_kick && resp.hovered();
                        ui.painter().rect(r, 14, Color32::from_rgba_unmultiplied(238, 247, 244, 10), egui::Stroke::new(1.0, if hot { theme::DANGER } else { theme::edge() }), egui::StrokeKind::Inside);
                        avatar(ui.painter(), pos2(r.left() + 10.0 + 14.0, r.center().y), 28.0, &p.name, &p.color);
                        let tx = r.left() + 10.0 + 28.0 + 10.0;
                        let g = widgets::clipped(ui, &p.name, theme::medium(13.5), INK, r.right() - 10.0 - tx);
                        ui.painter().galley(pos2(tx, r.top() + 7.0), g, INK);
                        let mut x = tx;
                        for (text, host_badge, on) in [("Организатор", true, p.host), ("Вы", false, p.id == me)] {
                            if !on {
                                continue;
                            }
                            x += badge_at(ui, pos2(x, r.top() + 27.0), text, host_badge) + 4.0;
                        }
                        if hot {
                            ui.painter().rect_filled(r, 14, Color32::from_rgba_unmultiplied(196, 43, 28, 209));
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, crate::i18n::t("Исключить"), theme::heavy(13.0), Color32::WHITE);
                        }
                        if can_kick && resp.on_hover_text(format!("{} {}", crate::i18n::t("Исключить"), p.name)).clicked() {
                            jam.kick(&p.id);
                        }
                    }
                });
            });
            ui.vertical(|ui| {
                ui.set_width(right);
                ui.spacing_mut().item_spacing.y = 8.0;
                label(ui, "Чат");
                self.jam_chat(ui, jam, v, right);
            });
        });
        if v.host {
            if !v.knocks.is_empty() {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    label(ui, "Хотят войти");
                    for k in &v.knocks {
                        // .jam-knock: an accent-tinted line with the name and two buttons
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
                        ui.painter().rect(r, 14, Color32::from_rgba_unmultiplied(111, 240, 196, 20), egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(111, 240, 196, 64)), egui::StrokeKind::Inside);
                        avatar(ui.painter(), pos2(r.left() + 10.0 + 14.0, r.center().y), 28.0, &k.name, &k.color);
                        ui.painter().text(pos2(r.left() + 48.0, r.center().y), Align2::LEFT_CENTER, &k.name, theme::medium(13.5), INK);
                        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 8.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
                        c.spacing_mut().item_spacing.x = 8.0;
                        if small_btn(&mut c, "Нет", false, true) {
                            jam.turn_away(&k.id);
                        }
                        if small_btn(&mut c, "Пустить", true, true) {
                            jam.admit(&k.id);
                        }
                    }
                });
            }
            self.jam_settings(ui, jam, v);
        }
        dialog_actions(ui, |ui| {
            if kit::danger_btn(ui, if v.host { "Завершить сессию" } else { "Выйти из сессии" }).clicked() {
                let j = jam.clone();
                std::thread::spawn(move || j.leave());
            }
            if widgets::btn(ui, None, "Свернуть", false).clicked() {
                jam.set_open(false);
            }
        });
    }

    /// .jam-chat: 240 high, the messages scrolling to the newest, the line to write at the bottom
    fn jam_chat(&mut self, ui: &mut Ui, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View, width: f32) {
        let (rect, _) = ui.allocate_exact_size(vec2(width, 240.0), Sense::hover());
        ui.painter().rect(rect, 18, Color32::from_rgba_unmultiplied(5, 16, 18, 115), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let form_h = 52.0;
        let list = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - form_h));
        let form = Rect::from_min_max(pos2(rect.left(), rect.bottom() - form_h), rect.max);
        ui.painter().rect_filled(form.shrink(0.5), egui::CornerRadius { nw: 0, ne: 0, sw: 18, se: 18 }, Color32::from_rgba_unmultiplied(5, 16, 18, 166));
        ui.painter().hline(form.x_range(), form.top(), egui::Stroke::new(1.0, theme::edge()));
        let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(list.shrink2(vec2(12.0, 10.0))).layout(egui::Layout::top_down(egui::Align::Min)));
        lui.set_clip_rect(list.shrink(1.0));
        if v.chat.is_empty() {
            // .jam-chat-empty: in the middle of the box
            let g = kit::wrapped(&lui, &crate::i18n::t("Напишите что-нибудь или пришлите реакцию"), theme::body(12.5), theme::ink3(), list.width() - 40.0, 18.0);
            let at = list.center() - g.size() / 2.0;
            lui.painter().galley(at, g, theme::ink3());
        } else {
            egui::ScrollArea::vertical().id_salt("jam-chat").auto_shrink([false, false]).stick_to_bottom(true).show(&mut lui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let w = ui.available_width();
                for m in &v.chat {
                    let mine = m.from == v.name;
                    let max = w * 0.9;
                    let text = kit::wrapped(ui, &m.text, theme::body(13.0), INK, max - 20.0, 18.2);
                    let time = (chrono::Local::now() - chrono::Duration::from_std(m.at.elapsed()).unwrap_or_default()).format("%H:%M").to_string();
                    let tg = ui.painter().layout_no_wrap(time, theme::body(10.0), theme::ink3());
                    let from_w = (max - 20.0 - tg.size().x - 8.0).max(30.0);
                    let fg = widgets::clipped(ui, &m.from, theme::heavy(11.5), if mine { theme::ink2() } else { ACCENT }, from_w);
                    let bw = (text.size().x.max(fg.size().x + 8.0 + tg.size().x) + 20.0).min(max);
                    let bh = fg.size().y + 3.0 + text.size().y + 12.0;
                    let (row, _) = ui.allocate_exact_size(vec2(w, bh), Sense::hover());
                    let b = if mine { Rect::from_min_size(pos2(row.right() - bw, row.top()), vec2(bw, bh)) } else { Rect::from_min_size(row.min, vec2(bw, bh)) };
                    let (fill, edge) = if mine { (Color32::from_rgba_unmultiplied(111, 240, 196, 20), Color32::from_rgba_unmultiplied(111, 240, 196, 56)) } else { (Color32::from_rgba_unmultiplied(238, 247, 244, 10), theme::edge()) };
                    ui.painter().rect(b, 12, fill, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
                    let fh = fg.size().y;
                    ui.painter().galley(pos2(b.left() + 10.0, b.top() + 6.0), fg, ACCENT);
                    ui.painter().galley(pos2(b.right() - 10.0 - tg.size().x, b.top() + 6.0 + (fh - tg.size().y) / 2.0), tg, theme::ink3());
                    ui.painter().galley(pos2(b.left() + 10.0, b.top() + 6.0 + fh + 3.0), text, INK);
                }
            });
        }
        // .jam-chat-form: the field and "Отправить", 36 high
        let mut fui = ui.new_child(egui::UiBuilder::new().max_rect(form.shrink(8.0)).layout(egui::Layout::right_to_left(egui::Align::Center)));
        fui.spacing_mut().item_spacing.x = 6.0;
        let can = !self.pages.jam.chat.trim().is_empty();
        let g = fui.painter().layout_no_wrap(crate::i18n::t("Отправить"), theme::medium(13.0), Color32::PLACEHOLDER);
        let (br, bresp) = fui.allocate_exact_size(vec2(g.size().x + 28.0, 36.0), if can { Sense::click() } else { Sense::hover() });
        fui.painter().rect_filled(br, 18, if can && bresp.hovered() { Color32::WHITE } else { INK }.gamma_multiply(if can { 1.0 } else { 0.5 }));
        fui.painter().galley(br.center() - g.size() / 2.0, g, theme::GROUND);
        let iw = fui.available_width();
        let (ir, _) = fui.allocate_exact_size(vec2(iw, 36.0), Sense::hover());
        fui.painter().rect(ir, 12, Color32::from_rgba_unmultiplied(5, 16, 18, 77), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let mut tui = fui.new_child(egui::UiBuilder::new().max_rect(ir.shrink2(vec2(12.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let r = tui.add(
            egui::TextEdit::singleline(&mut self.pages.jam.chat)
                .hint_text(egui::RichText::new(crate::i18n::t("Сообщение…")).color(theme::ink3()))
                .char_limit(200)
                .frame(egui::Frame::NONE)
                .font(egui::FontId::new(13.0, theme::medium500()))
                .desired_width(ir.width() - 24.0)
                .align(Align2::LEFT_CENTER),
        );
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (bresp.clicked() || enter) && can {
            jam.send_chat(&std::mem::take(&mut self.pages.jam.chat));
            r.request_focus();
        }
    }

    /// reactions and messages float up in the corner for a few seconds (messages only while
    /// the dialog is closed: in it, they are right there)
    fn jam_float(&mut self, ctx: &egui::Context, v: &View) {
        let now = Instant::now();
        let reactions: Vec<_> = v.reactions.iter().filter(|r| now.duration_since(r.at) < REACTION_SHOWN).cloned().collect();
        let chat: Vec<_> = if v.open {
            vec![]
        } else {
            let all: Vec<_> = v.chat.iter().filter(|m| now.duration_since(m.at) < MESSAGE_SHOWN).cloned().collect();
            all[all.len().saturating_sub(3)..].to_vec()
        };
        if reactions.is_empty() && chat.is_empty() {
            return;
        }
        let screen = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("jam-float")));
        let saver = crate::state::get().settings.read().unwrap().memory_saver;
        // .jam-float: right 28, bottom 110
        let base = pos2(screen.right() - 28.0, screen.bottom() - 110.0);
        let mut y = base.y;
        for m in chat.iter().rev() {
            let age = now.duration_since(m.at).as_secs_f32() / MESSAGE_SHOWN.as_secs_f32();
            let alpha = if age > 0.85 { (1.0 - age) / 0.15 } else { (age / 0.05).min(1.0) };
            let text = format!("{}: {}", m.from, m.text);
            let g = painter.layout(text, theme::body(13.0), INK, 256.0);
            let r = Rect::from_min_size(pos2(base.x - g.size().x - 24.0, y - g.size().y - 16.0), g.size() + vec2(24.0, 16.0));
            painter.rect(r, 14, Color32::from_rgba_unmultiplied(8, 22, 24, (235.0 * alpha) as u8), egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(111, 240, 196, (71.0 * alpha) as u8)), egui::StrokeKind::Inside);
            painter.galley(r.min + vec2(12.0, 8.0), g, INK.gamma_multiply(alpha));
            y = r.top() - 10.0;
        }
        for (i, r) in reactions.iter().enumerate() {
            let t = now.duration_since(r.at).as_secs_f32() / REACTION_SHOWN.as_secs_f32();
            let rise = if saver { 0.0 } else { 150.0 * (1.0 - (1.0 - t).powi(2)) };
            let alpha = if saver { 1.0 } else if t > 0.7 { (1.0 - t) / 0.3 } else { 1.0 };
            let x = base.x - 140.0 + ((i * 26) % 100) as f32 + 22.0;
            let c = pos2(x, y - 30.0 - rise);
            let tex = emoji_url(&r.text).and_then(|u| self.images.get(&u, 44.0, ctx.pixels_per_point()));
            let er = Rect::from_center_size(c, vec2(44.0, 44.0));
            match tex {
                Some(id) => {
                    painter.image(id, er, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE.gamma_multiply(alpha));
                }
                None => {
                    painter.text(c, Align2::CENTER_CENTER, &r.text, egui::FontId::proportional(36.0), INK.gamma_multiply(alpha));
                }
            }
            let g = painter.layout_no_wrap(r.from.clone(), theme::heavy(10.5), INK.gamma_multiply(alpha));
            let fr = Rect::from_center_size(pos2(c.x, c.y + 30.0), g.size() + vec2(12.0, 2.0));
            painter.rect(fr, 6, Color32::from_rgba_unmultiplied(5, 16, 18, (217.0 * alpha) as u8), egui::Stroke::new(1.0, theme::edge().gamma_multiply(alpha)), egui::StrokeKind::Inside);
            painter.galley(fr.min + vec2(6.0, 1.0), g, INK);
        }
        // the fading and rising is drawn while there is something to show
        ctx.request_repaint_after(Duration::from_millis(if saver { 500 } else { 33 }));
    }

    /// people asking to come in, for the host while the dialog is closed (.jam-asks)
    fn jam_asks(&mut self, ctx: &egui::Context, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        if v.open || v.knocks.is_empty() {
            return;
        }
        let screen = ctx.content_rect();
        let w = 460.0_f32.min(screen.width() - 32.0);
        egui::Area::new(egui::Id::unique("jam-asks")).order(egui::Order::Foreground).fixed_pos(pos2(screen.center().x - w / 2.0, screen.top() + 52.0)).show(ctx, |ui| {
            for k in &v.knocks {
                egui::Frame::new().fill(theme::glass_strong()).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(18).inner_margin(egui::Margin::symmetric(12, 10)).shadow(egui::Shadow { offset: [0, 12], blur: 30, spread: 0, color: Color32::from_black_alpha(102) }).show(ui, |ui| {
                    ui.set_width(w - 24.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                        avatar(ui.painter(), r.center(), 28.0, &k.name, &k.color);
                        ui.label(egui::RichText::new(format!("{} {}", k.name, crate::i18n::t("просится в сессию"))).color(INK));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::pill_button(ui, None, "Нет", false).clicked() {
                                jam.turn_away(&k.id);
                            }
                            if widgets::pill_button(ui, None, "Пустить", true).clicked() {
                                jam.admit(&k.id);
                            }
                        });
                    });
                });
                ui.add_space(8.0);
            }
        });
    }

    /// on "Сейчас играет" while a session is on: who is in it; a click opens the session
    pub fn jam_banner(&mut self, ui: &mut Ui) {
        let Some(jam) = crate::jam::engine::get().cloned() else { return };
        let v = jam.view();
        if v.status != JamStatus::Live {
            return;
        }
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
        let fill = if resp.hovered() { Color32::from_rgba_unmultiplied(31, 138, 115, 66) } else { Color32::from_rgba_unmultiplied(31, 138, 115, 41) };
        ui.painter().rect(r, 16, fill, egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(111, 240, 196, 64)), egui::StrokeKind::Inside);
        let mut x = r.left() + 14.0 + 11.0;
        for p in v.people.iter().take(4) {
            ui.painter().circle_filled(pos2(x, r.center().y), 12.5, theme::GROUND);
            avatar(ui.painter(), pos2(x, r.center().y), 22.0, &p.name, &p.color);
            x += 16.0;
        }
        let host_name = v.people.iter().find(|p| p.host).map(|p| p.name.clone()).unwrap_or_default();
        let tx = x + 8.0;
        ui.painter().text(pos2(tx, r.top() + 8.0), Align2::LEFT_TOP, format!("{} {}", crate::i18n::t("Вместе:"), listeners(v.people.len())), theme::medium(12.5), INK);
        let who = if v.host { crate::i18n::t("Вы организатор") } else { format!("{} {host_name}", crate::i18n::t("Организатор:")) };
        ui.painter().text(pos2(tx, r.top() + 25.0), Align2::LEFT_TOP, who, theme::body(11.0), ACCENT);
        let icon_r = Rect::from_center_size(pos2(r.right() - 22.0, r.center().y), vec2(16.0, 16.0));
        crate::ui::icons::paint(ui.painter(), "jam", icon_r, ACCENT);
        ui.painter().text(pos2(icon_r.left() - 6.0, r.center().y), Align2::RIGHT_CENTER, &v.code, theme::heavy(11.0), ACCENT);
        if resp.clicked() {
            jam.set_open(true);
        }
        let _ = CornerRadius::ZERO;
    }
}
