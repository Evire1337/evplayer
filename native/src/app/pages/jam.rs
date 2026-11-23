//! "Совместное прослушивание" on screen (src/jam/JamDialog.tsx, JamBanner.tsx, JamFloat.tsx,
//! JamAsks.tsx and jam.css): the dialog to start or join a session and to run it, the banner on
//! "Сейчас играет", reactions and messages floating up in the corner, and the people asking to
//! come in. The session itself is jam/engine.rs.

use std::time::{Duration, Instant};

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Ui};

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
    qr: Option<(String, egui::TextureHandle)>,
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

fn card(ui: &mut Ui, room: bool, body: impl FnOnce(&mut Ui)) {
    let (fill, stroke) = if room { (Color32::from_rgba_unmultiplied(31, 138, 115, 40), Color32::from_rgba_unmultiplied(111, 240, 196, 64)) } else { (theme::glass(), theme::edge()) };
    egui::Frame::new().fill(fill).stroke(egui::Stroke::new(1.0, stroke)).corner_radius(18).inner_margin(14).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui);
    });
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

    fn jam_dialog(&mut self, ctx: &egui::Context, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        let frame = egui::Frame::new().fill(Color32::from_rgb(0x0c, 0x2c, 0x28)).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(24).inner_margin(22);
        let r = egui::Modal::new(egui::Id::unique("jam-dialog")).frame(frame).backdrop_color(Color32::from_black_alpha(140)).show(ctx, |ui| {
            ui.set_width(600.0);
            ui.spacing_mut().item_spacing.y = 12.0;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Совместное прослушивание")).font(theme::heavy(22.0)).color(INK));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, ic::CLOSE, 30.0, false, "Закрыть").clicked() {
                        jam.set_open(false);
                    }
                });
            });
            egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 160.0).show(ui, |ui| {
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
            card(ui, false, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().color(ACCENT));
                    ui.label(format!("{} {}…", crate::i18n::t("Организатор решает, пустить ли вас в сессию"), v.code));
                });
            });
            if widgets::pill_button(ui, None, "Не ждать", false).clicked() {
                let j = jam.clone();
                std::thread::spawn(move || j.leave());
            }
            return;
        }
        App::muted(ui, "Слушайте вместе с друзьями: одна очередь на всех, музыка играет одновременно, а в чат можно писать и слать реакции.");
        card(ui, false, |ui| {
            label(ui, "Как вас увидят");
            ui.horizontal_wrapped(|ui| {
                let mut name = v.name.clone();
                if ui.add(egui::TextEdit::singleline(&mut name).hint_text(crate::i18n::t("Ваше имя")).char_limit(24).desired_width(200.0)).changed() {
                    jam.set_profile(Some(name), None);
                }
                ui.add_space(8.0);
                for c in COLORS {
                    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                    let grow = if resp.hovered() { 1.15 } else { 1.0 };
                    ui.painter().circle_filled(r.center(), 12.0 * grow, color_of(c));
                    if v.color == c {
                        ui.painter().circle_stroke(r.center(), 12.0 * grow, egui::Stroke::new(2.0, Color32::WHITE));
                        crate::ui::icons::paint(ui.painter(), "check", Rect::from_center_size(r.center(), vec2(12.0, 12.0)), theme::GROUND);
                    }
                    if resp.clicked() {
                        jam.set_profile(None, Some(c.to_string()));
                    }
                }
            });
        });
        if let Some(i) = widgets::tabs(ui, &["Создать сессию", "Присоединиться"], if self.pages.jam.join_tab { 1 } else { 0 }) {
            self.pages.jam.join_tab = i == 1;
            jam.browse(i == 1 && v.browse);
        }
        if !self.pages.jam.join_tab {
            self.jam_settings(ui, jam, v);
            ui.add_enabled_ui(!busy, |ui| {
                if widgets::pill_button(ui, Some(ic::PEOPLE), if v.status == JamStatus::Starting { "Создаю…" } else { "Создать сессию" }, true).clicked() {
                    jam.start();
                }
            });
        } else {
            ui.label(egui::RichText::new(crate::i18n::t("Код или ссылка приглашения")).font(theme::medium(13.0)).color(INK));
            let r = ui.add(egui::TextEdit::singleline(&mut self.pages.jam.code).hint_text("JAM-7K9P-2QXZ").char_limit(60).font(theme::heavy(16.0)).align(egui::Align2::CENTER_CENTER).desired_width(f32::INFINITY));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_enabled_ui(!busy && !self.pages.jam.code.trim().is_empty(), |ui| {
                if widgets::pill_button(ui, None, if v.status == JamStatus::Joining { "Вхожу…" } else { "Присоединиться" }, true).clicked() || enter {
                    jam.join(&self.pages.jam.code.clone());
                }
            });
            // the public list
            if let Some(on) = widgets::toggle_row(ui, "Открытые сессии", "Сессии, которые их организаторы показали всем", v.browse, true) {
                jam.set_browse(on);
            }
            if v.browse {
                if v.lobby.is_empty() {
                    App::muted(ui, "Пока никого: открытые сессии появятся здесь сами");
                }
                for l in &v.lobby {
                    card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_max_width(ui.available_width() - 160.0);
                                ui.label(egui::RichText::new(&l.host).font(theme::medium(14.0)).color(INK));
                                let mut line = listeners(l.people);
                                if let Some(s) = &l.song {
                                    line.push_str(&format!(" · {s}"));
                                }
                                ui.label(egui::RichText::new(line).font(theme::body(12.0)).color(theme::ink2()));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::pill_button(ui, None, "Войти", true).clicked() && !busy {
                                    jam.join(&l.code);
                                }
                                if l.access == Access::Ask {
                                    ui.label(egui::RichText::new(crate::i18n::t("по запросу")).font(theme::medium(10.5)).color(theme::ink2()));
                                }
                            });
                        });
                    });
                }
            }
        }
    }

    /// how the session is set up: before it starts (remembered) and, for the host, while it is on
    fn jam_settings(&mut self, ui: &mut Ui, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        let live = v.status == JamStatus::Live;
        let control = if live { v.control } else { v.setup.control };
        let access = if live { v.access } else { v.setup.access };
        let listed = if live { v.listed } else { v.setup.listed };
        card(ui, false, |ui| {
            let hint = if control == Control::Anyone { "Друзья включают треки, переключают их и ставят паузу" } else { "Управляете только вы; друзья добавляют песни в очередь, а своё у них не играет" };
            if let Some(on) = widgets::toggle_row(ui, "Все участники управляют музыкой", hint, control == Control::Anyone, true) {
                jam.set_control(if on { Control::Anyone } else { Control::Host });
            }
            ui.label(egui::RichText::new(crate::i18n::t("Вход")).font(theme::medium(13.5)).color(INK));
            let sel = match access {
                Access::Open => 0,
                Access::Ask => 1,
                Access::Closed => 2,
            };
            if let Some(i) = widgets::tabs(ui, &["Открыт", "По запросу", "Закрыт"], sel) {
                jam.set_access([Access::Open, Access::Ask, Access::Closed][i]);
            }
            App::muted(ui, match access {
                Access::Open => "Заходит любой, у кого есть код, ссылка или QR-код",
                Access::Ask => "Каждый, кто пришёл по коду, ждёт, пока вы его пустите",
                Access::Closed => "Новые люди не войдут ни по коду, ни по ссылке",
            });
            let hint = if access == Access::Closed { "Закрытая сессия в списке не видна" } else { "Сессию увидят все, кто открыл «Присоединиться», вместе с числом участников" };
            if let Some(on) = widgets::toggle_row(ui, "Показывать в общем списке", hint, listed, true) {
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
            ui.vertical_centered(|ui| {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), vec2(180.0, 180.0))).corner_radius(12));
                App::muted(ui, "Наведите камеру телефона с EvPlayer");
            });
        }
    }

    fn jam_session(&mut self, ui: &mut Ui, jam: &std::sync::Arc<crate::jam::engine::Jam>, v: &View) {
        let host_name = v.people.iter().find(|p| p.host).map(|p| p.name.clone()).unwrap_or_default();
        card(ui, true, |ui| {
            ui.horizontal(|ui| {
                // the live dot
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                let t = ui.ctx().input(|i| i.time) as f32;
                let saver = crate::state::get().settings.read().unwrap().memory_saver;
                let pulse = if saver { 1.0 } else { 1.0 + 0.35 * (0.5 - 0.5 * (t * std::f32::consts::TAU / 1.8).cos()) };
                ui.painter().circle_filled(r.center(), 5.0 * pulse, ACCENT.gamma_multiply(if saver { 1.0 } else { 1.0 - (pulse - 1.0) }));
                if !saver {
                    ui.ctx().request_repaint_after(Duration::from_millis(66));
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Сессия идёт")).font(theme::medium(14.0)).color(ACCENT));
                    let who = if v.host { crate::i18n::t("Вы организатор") } else { format!("{} {host_name}", crate::i18n::t("Организатор:")) };
                    ui.label(egui::RichText::new(who).font(theme::body(12.0)).color(theme::ink2()));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if v.host && widgets::icon_button(ui, "@refresh", 30.0, false, "Новый код, ссылка и QR: старые перестанут работать").clicked() {
                        let j = jam.clone();
                        std::thread::spawn(move || j.change_code());
                    }
                    if widgets::icon_button(ui, "@qr", 30.0, self.pages.jam.show_qr, "QR-код").clicked() {
                        self.pages.jam.show_qr = !self.pages.jam.show_qr;
                    }
                    if widgets::icon_button(ui, ic::LINK, 30.0, false, "Копировать ссылку").clicked() {
                        ui.ctx().copy_text(protocol::invite_link(&v.code));
                        self.toast("Ссылка-приглашение скопирована");
                    }
                    if widgets::icon_button(ui, "@copy", 30.0, false, "Копировать код").clicked() {
                        ui.ctx().copy_text(v.code.clone());
                        self.toast("Код скопирован");
                    }
                    ui.label(egui::RichText::new(&v.code).font(egui::FontId::monospace(16.0)).strong().color(INK));
                });
            });
            if self.pages.jam.show_qr {
                self.jam_qr(ui, &v.code);
            }
        });
        // the reactions
        ui.horizontal(|ui| {
            for e in REACTIONS {
                let (r, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
                let fill = if resp.is_pointer_button_down_on() { Color32::from_rgba_unmultiplied(111, 240, 196, 51) } else if resp.hovered() { Color32::from_rgba_unmultiplied(238, 247, 244, 36) } else { Color32::from_rgba_unmultiplied(238, 247, 244, 15) };
                ui.painter().circle(r.center(), 19.0, fill, egui::Stroke::new(1.0, theme::edge()));
                let s = if resp.hovered() { 26.0 } else { 24.0 };
                self.emoji(ui, e, Rect::from_center_size(r.center(), vec2(s, s)));
                if resp.clicked() {
                    jam.react(e);
                }
            }
        });
        let me = v.people.iter().find(|p| p.name == v.name).map(|p| p.id.clone()).unwrap_or_default();
        ui.columns(2, |cols| {
            label(&mut cols[0], &format!("{} · {}", crate::i18n::t("Участники"), v.people.len()));
            egui::ScrollArea::vertical().id_salt("jam-people").max_height(240.0).show(&mut cols[0], |ui| {
                for p in &v.people {
                    let can_kick = v.host && !p.host;
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), if can_kick { Sense::click() } else { Sense::hover() });
                    let hot = can_kick && resp.hovered();
                    ui.painter().rect(r, 14, Color32::from_rgba_unmultiplied(238, 247, 244, 10), egui::Stroke::new(1.0, if hot { Color32::from_rgb(0xff, 0x9b, 0x9b) } else { theme::edge() }), egui::StrokeKind::Inside);
                    avatar(ui.painter(), pos2(r.left() + 24.0, r.center().y), 28.0, &p.name, &p.color);
                    ui.painter().text(pos2(r.left() + 46.0, r.top() + 9.0), Align2::LEFT_TOP, &p.name, theme::medium(13.5), INK);
                    let mut x = r.left() + 46.0;
                    for (badge, on) in [("Организатор", p.host), ("Вы", p.id == me)] {
                        if !on {
                            continue;
                        }
                        let g = ui.painter().layout_no_wrap(crate::i18n::t(badge), theme::heavy(10.5), if badge == "Вы" { theme::ink2() } else { ACCENT });
                        let br = Rect::from_min_size(pos2(x, r.top() + 27.0), g.size() + vec2(12.0, 2.0));
                        ui.painter().rect_filled(br, 6, if badge == "Вы" { Color32::from_rgba_unmultiplied(238, 247, 244, 31) } else { Color32::from_rgba_unmultiplied(111, 240, 196, 38) });
                        ui.painter().galley(br.min + vec2(6.0, 1.0), g, INK);
                        x = br.right() + 4.0;
                    }
                    if hot {
                        ui.painter().rect_filled(r, 14, Color32::from_rgba_unmultiplied(196, 43, 28, 209));
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, crate::i18n::t("Исключить"), theme::heavy(13.0), Color32::WHITE);
                    }
                    if can_kick && resp.clicked() {
                        jam.kick(&p.id);
                    }
                }
            });
            label(&mut cols[1], "Чат");
            let ui = &mut cols[1];
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(5, 16, 18, 115)).stroke(egui::Stroke::new(1.0, theme::edge())).corner_radius(18).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().id_salt("jam-chat").max_height(190.0).min_scrolled_height(190.0).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                    if v.chat.is_empty() {
                        App::muted(ui, "Напишите что-нибудь или пришлите реакцию");
                    }
                    for m in &v.chat {
                        let mine = m.from == v.name;
                        let layout = if mine { egui::Layout::right_to_left(egui::Align::Min) } else { egui::Layout::left_to_right(egui::Align::Min) };
                        ui.with_layout(layout, |ui| {
                            egui::Frame::new().fill(if mine { Color32::from_rgba_unmultiplied(111, 240, 196, 20) } else { Color32::from_rgba_unmultiplied(238, 247, 244, 10) }).stroke(egui::Stroke::new(1.0, if mine { Color32::from_rgba_unmultiplied(111, 240, 196, 56) } else { theme::edge() })).corner_radius(12).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                                ui.set_max_width(ui.available_width() * 0.9);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&m.from).font(theme::heavy(11.5)).color(if mine { theme::ink2() } else { ACCENT }));
                                    ui.label(egui::RichText::new(&m.text).font(theme::body(13.0)).color(INK));
                                });
                            });
                        });
                    }
                });
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.pages.jam.chat).hint_text(crate::i18n::t("Сообщение…")).char_limit(200).desired_width(ui.available_width() - 110.0));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (widgets::pill_button(ui, None, "Отправить", true).clicked() || enter) && !self.pages.jam.chat.trim().is_empty() {
                        jam.send_chat(&std::mem::take(&mut self.pages.jam.chat));
                        r.request_focus();
                    }
                });
            });
        });
        if v.host {
            if !v.knocks.is_empty() {
                label(ui, "Хотят войти");
                for k in &v.knocks {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                        avatar(ui.painter(), r.center(), 28.0, &k.name, &k.color);
                        ui.label(egui::RichText::new(&k.name).font(theme::medium(13.5)).color(INK));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::pill_button(ui, None, "Нет", false).clicked() {
                                jam.turn_away(&k.id);
                            }
                            if widgets::pill_button(ui, None, "Пустить", true).clicked() {
                                jam.admit(&k.id);
                            }
                        });
                    });
                }
            }
            self.jam_settings(ui, jam, v);
        }
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let leave = ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(if v.host { "Завершить сессию" } else { "Выйти из сессии" })).font(theme::medium(13.5)).color(Color32::WHITE)).fill(Color32::from_rgb(0xc4, 0x2b, 0x1c)).corner_radius(18).min_size(vec2(0.0, 36.0)));
                if leave.clicked() {
                    let j = jam.clone();
                    std::thread::spawn(move || j.leave());
                }
                if widgets::pill_button(ui, None, "Свернуть", false).clicked() {
                    jam.set_open(false);
                }
            });
        });
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
