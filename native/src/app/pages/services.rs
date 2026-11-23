//! Настройки → Сервисы и импорт (src/pages/settings/ServicesTab.tsx): where EvPlayer takes
//! music from and how your playlists come over from other services, in the page's cards
//! (.svc-card in a grid of two). The slow steps (connecting a profile, signing in, carrying
//! over a library) run on threads; their result comes as a toast, a long carry-over in the
//! import panel at the bottom.

use std::sync::atomic::AtomicBool;

use egui::{pos2, vec2, Align2, Color32, Rect, Stroke, Ui};

use super::kit::{self, Tone};
use crate::app::App;
use crate::import::{match_all, Named};
use crate::state;
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;

/// what the services tab remembers between frames
#[derive(Default)]
pub struct ServicesState {
    pub link: String,
    pub sc_profile: String,
    pub ym_login: String,
    pub sc_more: bool,
    pub sp_more: bool,
    /// the two letters being typed after the countries' chips
    pub country: String,
    /// the proxy being typed (None: not touched yet, the saved one is shown)
    pub proxy: Option<String>,
    /// a slow step running: its name (buttons wait for it)
    pub busy: std::sync::Arc<std::sync::Mutex<Option<&'static str>>>,
}

/// .svc-badge: a 42 px square in the service's colour, darker to the corner, with its letter
fn badge(ui: &mut Ui, letter: &str, color: Color32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(42.0, 42.0), egui::Sense::hover());
    let dark = Color32::from_rgb((color.r() as f32 * 0.45) as u8, (color.g() as f32 * 0.45) as u8, (color.b() as f32 * 0.45) as u8);
    let shadow = egui::Shadow { offset: [0, 8], blur: 22, spread: 0, color: color.gamma_multiply(0.35) };
    ui.painter().add(shadow.as_shape(rect, egui::CornerRadius::same(13)));
    kit::gradient_rect(ui, rect, 13.0, color, dark, 145.0);
    if !letter.is_empty() {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, letter, theme::heavy(18.0), Color32::WHITE);
    }
    rect
}

/// .svc-head: badge, name and what the service does, and its state (or a button) on the right
fn head(ui: &mut Ui, letter: &str, color: Color32, name: &str, what: &str, right: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        badge(ui, letter, color);
        let text_w = (w - 42.0 - 12.0 - 12.0 - 150.0).max(140.0);
        ui.vertical(|ui| {
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(egui::RichText::new(crate::i18n::t(name)).font(theme::heavy(15.0)).color(INK));
            let g = kit::wrapped(ui, &crate::i18n::t(what), theme::body(12.5), theme::ink3(), text_w, 18.0);
            let (r, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
            ui.painter().galley(r.min, g, theme::ink3());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
}

/// the line between two heads in one card (.svc-head + .svc-head)
fn divider(ui: &mut Ui) {
    let y = ui.cursor().top();
    ui.painter().hline(ui.available_rect_before_wrap().x_range(), y, Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 15)));
    ui.add_space(12.0);
}

/// .svc-card: glass, a faint edge, rounded 22, padding 18, its parts 12 apart
fn svc_card(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(theme::glass()).stroke(Stroke::new(1.0, theme::edge())).corner_radius(22).inner_margin(18).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 12.0;
        body(ui);
    });
}

/// .svc-account: the account's picture, name and a line, with a button on the right; returns
/// (name pressed, button pressed)
fn account(app: &mut App, ui: &mut Ui, avatar: Option<&str>, letter: Option<&str>, name: &str, line: &str, button: &str) -> (bool, bool) {
    let mut out = (false, false);
    egui::Frame::new().fill(Color32::from_rgba_unmultiplied(238, 247, 244, 13)).corner_radius(16).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let (r, _) = ui.allocate_exact_size(vec2(48.0, 48.0), egui::Sense::hover());
            match (avatar, letter) {
                (Some(a), _) => widgets::paint_cover(ui, &mut app.images, Some(a), r, 24),
                (None, Some(l)) => {
                    // .ym-avatar: a yellow disc with the first letter
                    let mesh_rect = r;
                    kit::gradient_rect(ui, mesh_rect, 24.0, Color32::from_rgb(0xff, 0xcc, 0x00), Color32::from_rgb(0xff, 0x8a, 0x00), 145.0);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, l, theme::heavy(20.0), Color32::from_rgb(0x1a, 0x12, 0x00));
                }
                _ => {
                    ui.painter().circle_filled(r.center(), 24.0, Color32::from_rgba_unmultiplied(238, 247, 244, 26));
                    crate::ui::icons::paint(ui.painter(), "user", Rect::from_center_size(r.center(), vec2(22.0, 22.0)), theme::ink3());
                }
            }
            let bw = ui.painter().layout_no_wrap(crate::i18n::t(button), theme::medium(14.0), Color32::PLACEHOLDER).size().x + 32.0;
            let tw = (ui.available_width() - bw - 12.0).max(80.0);
            ui.vertical(|ui| {
                ui.set_width(tw);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add_space(4.0);
                let g = widgets::clipped(ui, name, theme::medium(14.0), INK, tw);
                let (nr, resp) = ui.allocate_exact_size(g.size(), egui::Sense::click());
                ui.painter().galley(nr.min, g, INK);
                if resp.hovered() {
                    ui.painter().hline(nr.x_range(), nr.bottom(), Stroke::new(1.0, INK));
                }
                out.0 = resp.clicked();
                let g = kit::wrapped(ui, line, theme::body(12.5), theme::ink3(), tw, 17.0);
                let (lr, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
                ui.painter().galley(lr.min, g, theme::ink3());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                out.1 = widgets::btn(ui, None, button, false).clicked();
            });
        });
    });
    out
}

/// .svc-connect: a box and a primary button; true when it is sent (the button or Enter)
fn connect_box(ui: &mut Ui, text: &mut String, hint: &str, button: &str, enabled: bool) -> bool {
    let mut sent = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let bw = ui.painter().layout_no_wrap(crate::i18n::t(button), theme::medium(14.0), Color32::PLACEHOLDER).size().x + 32.0 + 24.0;
        let w = ui.available_width() - bw - 8.0;
        let r = kit::input_styled(ui, text, hint, w, 40.0, 13, theme::medium500(), false);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ok = enabled && !text.trim().is_empty();
        if (kit::btn_if(ui, ok, Some(ic::PLUG), button, true).clicked() || enter) && ok {
            sent = true;
        }
    });
    sent
}

/// .svc-actions: buttons side by side
fn actions(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        add(ui);
    });
}

/// .svc-more: a faint bold link that opens the advanced part
fn more(ui: &mut Ui, open: &mut bool, closed: &str, opened: &str) {
    let text = crate::i18n::t(if *open { opened } else { closed });
    let g = ui.painter().layout_no_wrap(text, theme::medium(12.5), Color32::PLACEHOLDER);
    let (r, resp) = ui.allocate_exact_size(g.size(), egui::Sense::click());
    ui.painter().galley(r.min, g, if resp.hovered() { INK } else { theme::ink3() });
    if resp.clicked() {
        *open = !*open;
    }
}

/// a numbered step (.steps li), with a link inside that opens `url`
fn step(ui: &mut Ui, n: usize, text: &str, url: Option<&str>) {
    let w = ui.available_width() - 20.0;
    let g = kit::wrapped(ui, &format!("{n}. {}", crate::i18n::t(text)), theme::body(13.0), theme::ink2(), w, 18.85);
    let (r, resp) = ui.allocate_exact_size(vec2(w + 20.0, g.size().y), if url.is_some() { egui::Sense::click() } else { egui::Sense::hover() });
    ui.painter().galley(r.min + vec2(4.0, 0.0), g, if url.is_some() && resp.hovered() { INK } else { theme::ink2() });
    if let (Some(u), true) = (url, resp.clicked()) {
        crate::dialogs::open_path(std::path::Path::new(u));
    }
}

/// a label over a box (.field)
fn field(ui: &mut Ui, label: &str, value: &str, hint: &str, password: bool, note: Option<&str>) -> Option<String> {
    let mut text = value.to_string();
    let changed = kit::field(ui, label, &mut text, hint, password).changed();
    if let Some(n) = note {
        let w = ui.available_width();
        let g = kit::wrapped(ui, &crate::i18n::t(n), theme::body(12.0), theme::ink3(), w, 16.2);
        let (r, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
        ui.painter().galley(r.min, g, theme::ink3());
    }
    changed.then_some(text)
}

impl App {
    pub fn settings_services(&mut self, ui: &mut Ui) {
        kit::tab_header(ui, ic::PLUG, 25.0, "Сервисы и импорт", "Откуда EvPlayer берёт музыку и как перенести ваши плейлисты из других сервисов.");
        kit::gap(ui);
        kit::card(ui, "Импорт по ссылке", "Плейлист, альбом или трек станет вашим плейлистом. Песни из Spotify и Deezer находятся автоматически", None, |ui| self.import_box(ui));
        // .svc-grid: columns of at least 360, as many as fit (two on a usual window)
        let w = ui.available_width();
        let two = w >= 360.0 * 2.0 + 16.0;
        let col = if two { (w - 16.0) / 2.0 } else { w };
        type Card = fn(&mut App, &mut Ui);
        let cards: [Card; 5] = [App::soundcloud_card, App::youtube_card, App::spotify_card, App::yandex_card, App::other_card];
        let rows: Vec<&[Card]> = if two { cards.chunks(2).collect() } else { cards.chunks(1).collect() };
        for row in rows {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for card in row {
                    ui.vertical(|ui| {
                        ui.set_width(col);
                        card(self, ui);
                    });
                }
            });
            ui.add_space(16.0);
        }
    }

    /// ImportBox: the link, what it is, and "Импортировать"
    fn import_box(&mut self, ui: &mut Ui) {
        let kind = crate::import::link_kind(&self.pages.services.link);
        let busy = self.importing.as_ref().is_some_and(|i| !i.progress.lock().unwrap().done);
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 52.0), egui::Sense::hover());
        let focused_before = ui.memory(|m| m.focused().is_some_and(|f| f == ui.scope_id().with("import-link")));
        ui.painter().rect(rect, 18, Color32::from_rgba_unmultiplied(238, 247, 244, 18), Stroke::new(1.0, if focused_before { Color32::from_rgba_unmultiplied(238, 247, 244, 77) } else { theme::edge() }), egui::StrokeKind::Inside);
        crate::ui::icons::paint(ui.painter(), "link", Rect::from_center_size(pos2(rect.left() + 14.0 + 9.0, rect.center().y), vec2(18.0, 18.0)), theme::ink3());
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(rect.left() + 14.0 + 18.0 + 10.0, rect.top() + 6.0), pos2(rect.right() - 6.0, rect.bottom() - 6.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
        inner.spacing_mut().item_spacing.x = 10.0;
        let ok = !self.pages.services.link.trim().is_empty() && !busy;
        let pressed = kit::btn_if(&mut inner, ok, None, "Импортировать", true).clicked();
        if let Some(k) = &kind {
            let name = match k {
                crate::import::Kind::Soundcloud => "SoundCloud",
                crate::import::Kind::Youtube => "YouTube",
                crate::import::Kind::Spotify => "Spotify",
                crate::import::Kind::Deezer => "Deezer",
                crate::import::Kind::Yandex => "Яндекс Музыка",
                crate::import::Kind::Code => "код EvPlayer",
                crate::import::Kind::Apple => "Apple Music",
            };
            kit::pill(&mut inner, Tone::Info, name);
        }
        let tw = inner.available_width();
        let r = inner.add(
            egui::TextEdit::singleline(&mut self.pages.services.link)
                .id(ui.scope_id().with("import-link"))
                .hint_text(egui::RichText::new(crate::i18n::t("Ссылка из Spotify, Яндекс Музыки, YouTube, SoundCloud, Deezer или код EVP…")).color(theme::ink3()))
                .frame(egui::Frame::NONE)
                .font(egui::FontId::new(14.0, theme::medium500()))
                .desired_width(tw)
                .align(egui::Align2::LEFT_CENTER),
        );
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (pressed || enter) && ok {
            if kind.is_none() {
                self.toast("Это не похоже на ссылку на плейлист, альбом или трек");
            } else {
                let text = std::mem::take(&mut self.pages.services.link).trim().to_string();
                self.start_import(text, None);
            }
        }
    }

    fn soundcloud_card(&mut self, ui: &mut Ui) {
        use crate::services::scaccount;
        let account_now = scaccount::account();
        let busy = self.busy();
        svc_card(ui, |ui| {
            let connected = account_now.is_some();
            head(ui, "S", Color32::from_rgb(0xff, 0x7a, 0x1a), "SoundCloud", "Поиск, чарты, исполнители и полноценное воспроизведение", |ui| {
                kit::pill(ui, if connected { Tone::Ok } else { Tone::Info }, if connected { "аккаунт подключён" } else { "работает без входа" });
            });
            if let Some(a) = &account_now {
                let mut parts = Vec::new();
                if let Some(f) = a.followers {
                    parts.push(format!("{} {}", super::search::fmt_count(f), crate::i18n::t("подписчиков")));
                }
                if let Some(l) = a.likes {
                    parts.push(format!("{} {}", super::search::fmt_count(l), crate::i18n::t("лайков")));
                }
                if let Some(t) = &a.last_sync {
                    parts.push(format!("{} {}", crate::i18n::t("синхронизировано"), super::library::ago(t)));
                }
                let (open, off) = account(self, ui, a.avatar.as_deref(), None, &a.name, &parts.join(" · "), "Отключить");
                if open {
                    self.go(crate::app::Page::Artist(format!("scu:{}", a.id.trim_start_matches("scu:").trim_start_matches("sc:"))));
                }
                if off {
                    scaccount::disconnect();
                }
                actions(ui, |ui| {
                    if kit::btn_if(ui, busy.is_none(), Some(ic::HEART), if busy == Some("likes") { "Забираю лайки…" } else { "Забрать лайки" }, false).clicked() {
                        self.slow("likes", || scaccount::import_likes().map(|n| if n > 0 { format!("В понравившиеся добавлено {n}") } else { "Новых лайков нет".into() }));
                    }
                    if kit::btn_if(ui, busy.is_none(), Some("@listPlus"), if busy == Some("playlists") { "Забираю плейлисты…" } else { "Забрать плейлисты" }, false).clicked() {
                        self.slow("playlists", || scaccount::import_playlists(&|_, _| {}).map(|(c, u)| if c + u > 0 { format!("Плейлистов: новых {c}, обновлено {u}") } else { "Публичных плейлистов не нашлось".into() }));
                    }
                });
                if let Some(v) = refresh_switch(ui, "Синхронизировать лайки", "При запуске новые лайки с SoundCloud сами появятся в «Понравившихся»", a.sync_likes) {
                    let mut a = a.clone();
                    a.sync_likes = v;
                    scaccount::save_account(Some(&a));
                }
            } else {
                let mut text = std::mem::take(&mut self.pages.services.sc_profile);
                if connect_box(ui, &mut text, "Ссылка на профиль или имя: soundcloud.com/ваше-имя", if busy == Some("connect") { "Подключаю…" } else { "Подключить" }, busy.is_none()) {
                    let input = text.trim().to_string();
                    self.slow("connect", move || {
                        scaccount::connect(&input)?;
                        let n = scaccount::import_likes().unwrap_or(0);
                        Ok(if n > 0 { format!("Аккаунт SoundCloud подключён · в понравившиеся добавлено {n}") } else { "Аккаунт SoundCloud подключён".into() })
                    });
                }
                self.pages.services.sc_profile = text;
            }
            kit::note(ui, "Подтягиваются ваши публичные лайки и плейлисты. Пароль не нужен и никуда не передаётся.");
            self.soundcloud_sync(ui);
            self.region_card(ui);
            self.proxy_card(ui);
            let mut open = self.pages.services.sc_more;
            more(ui, &mut open, "Дополнительно", "Скрыть дополнительно");
            self.pages.services.sc_more = open;
            if open {
                let id = state::get().settings.read().unwrap().soundcloud_client_id.clone();
                if let Some(v) = field(ui, "client_id своего приложения SoundCloud", &id, "Пусто — определяется автоматически", false, Some("Если у вас есть приложение на soundcloud.com/you/apps, EvPlayer будет ходить в SoundCloud от его имени.")) {
                    state::get().set_settings(|s| s.soundcloud_client_id = v.trim().to_string());
                }
            }
        });
    }

    /// SoundCloudSync: "Вход и полная синхронизация"
    fn soundcloud_sync(&mut self, ui: &mut Ui) {
        use crate::services::scaccount;
        let busy = self.busy();
        // .svc-sync: a line above, 12 below it
        let y = ui.cursor().top() - 2.0;
        ui.painter().hline(ui.available_rect_before_wrap().x_range(), y, Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 20)));
        ui.add_space(10.0);
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.label(egui::RichText::new(crate::i18n::t("Вход и полная синхронизация")).font(theme::heavy(15.0)).color(INK));
        if let Some(l) = scaccount::login() {
            let st = scaccount::status();
            let line = if st.running { crate::i18n::t("синхронизирую…") } else if let Some(t) = &st.last_run { format!("{} {}", crate::i18n::t("последняя синхронизация"), super::library::ago(t)) } else { crate::i18n::t("вход выполнен") };
            let (_, out) = account(self, ui, l.avatar.as_deref(), None, &l.name, &line, "Выйти");
            if out {
                scaccount::sign_out();
            }
            let full = scaccount::full_sync();
            if let Some(v) = refresh_switch(ui, "Включить полную синхронизацию", "Лайки и плейлисты из плеера появляются в вашем аккаунте SoundCloud, а удалённые здесь удаляются и там", full) {
                if v {
                    self.dialog = Some(crate::app::Dialog::FullSync);
                } else {
                    state::get().set_extra("soundcloudFullSync", serde_json::Value::Bool(false));
                }
            }
            if full {
                actions(ui, |ui| {
                    if kit::btn_if(ui, !st.running, Some("@refresh"), if st.running { "Синхронизирую…" } else { "Синхронизировать сейчас" }, false).clicked() {
                        std::thread::spawn(scaccount::sync_now);
                    }
                });
            }
            if let Some(e) = &st.error {
                kit::error(ui, e);
            }
        } else {
            let id_set = !state::get().settings.read().unwrap().soundcloud_client_id.trim().is_empty();
            let secret = state::get().extra("soundcloudClientSecret").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
            if let Some(v) = field(ui, "client_secret своего приложения", &secret, "Вместе с client_id из «Дополнительно»", true, None) {
                state::get().set_extra("soundcloudClientSecret", serde_json::Value::String(v.trim().to_string()));
            }
            actions(ui, |ui| {
                if kit::btn_if(ui, busy.is_none() && id_set && !secret.trim().is_empty(), Some(ic::PLUG), if busy == Some("sc-login") { "Жду вход в браузере…" } else { "Войти через OAuth" }, true).clicked() {
                    self.slow("sc-login", || scaccount::sign_in().map(|_| "Вход в SoundCloud выполнен".into()));
                }
            });
            kit::note(ui, "Нужно своё приложение на soundcloud.com/you/apps: его client_id впишите в «Дополнительно», а в Redirect URI укажите http://127.0.0.1:43821/callback. Вход проходит на сайте SoundCloud, пароль в EvPlayer не попадает.");
        }
    }

    /// RegionGuardCard: which countries SoundCloud is not asked from
    fn region_card(&mut self, ui: &mut Ui) {
        let (guard, countries) = {
            let s = state::get().settings.read().unwrap();
            (s.region_guard, s.region_countries.clone())
        };
        let busy = self.busy();
        svc_card(ui, |ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Страна подключения")).font(theme::heavy(14.0)).color(INK));
            kit::note(ui, "Если вы подключаетесь из страны, где SoundCloud ограничивает доступ, EvPlayer может вообще не заходить на SoundCloud и играть песни с YouTube Music. Так ваш аккаунт SoundCloud не «светится» с такого адреса. Если вас это не касается, ничего включать не нужно.");
            if let Some(v) = kit::switch_row(ui, "Не заходить на SoundCloud из выбранных стран", "Работает только пока вы в одной из отмеченных стран: SoundCloud не открывается, песни играют с YouTube Music. Включите VPN другой страны, и всё вернётся. В остальных странах ничего не меняется.", guard, false) {
                state::get().set_settings(|s| s.region_guard = v);
            }
            if !guard {
                return;
            }
            const COMMON: [&str; 4] = ["RU", "UA", "BY", "KZ"];
            let toggle = |code: &str| {
                let code = code.to_string();
                state::get().set_settings(|s| {
                    if let Some(i) = s.region_countries.iter().position(|c| *c == code) {
                        s.region_countries.remove(i);
                    } else {
                        s.region_countries.push(code);
                    }
                });
            };
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for code in COMMON {
                    if kit::chip_sized(ui, code, countries.iter().any(|c| c == code), 30.0, None).clicked() {
                        toggle(code);
                    }
                }
                for code in countries.iter().filter(|c| !COMMON.contains(&c.as_str())) {
                    if kit::chip_sized(ui, code, true, 30.0, None).on_hover_text(crate::i18n::t("Убрать")).clicked() {
                        toggle(code);
                    }
                }
                // .chip-input: two letters, added on Enter or leaving the box
                let (r, _) = ui.allocate_exact_size(vec2(64.0, 30.0), egui::Sense::hover());
                kit::dashed_capsule(ui, r.shrink(0.5), Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 64)));
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
                let typed = &mut self.pages.services.country;
                let resp = c.add(egui::TextEdit::singleline(typed).hint_text(egui::RichText::new(crate::i18n::t("+ код")).color(theme::ink3())).frame(egui::Frame::NONE).char_limit(2).font(theme::medium(12.5)).desired_width(44.0).align(egui::Align2::LEFT_CENTER));
                if resp.changed() {
                    *typed = typed.chars().filter(|c| c.is_ascii_alphabetic()).collect::<String>().to_uppercase();
                }
                if resp.lost_focus() {
                    if let Some(code) = crate::services::region::normalize_country(typed) {
                        if !countries.contains(&code) {
                            state::get().set_settings(|s| s.region_countries.push(code));
                        }
                    }
                    typed.clear();
                }
            });
            let country = crate::services::region::known_country();
            let ok = crate::services::region::allowed(country.as_deref(), guard, &countries);
            actions(ui, |ui| {
                let (tone, text) = match &country {
                    Some(c) => (if ok { Tone::Ok } else { Tone::Warn }, format!("{} {c}", crate::i18n::t("Сейчас:"))),
                    None => (Tone::Off, crate::i18n::t("Страна не определена")),
                };
                kit::pill(ui, tone, &text);
                let line = match (&country, ok) {
                    (Some(_), true) => "SoundCloud доступен",
                    (Some(_), false) => "SoundCloud закрыт: включите VPN другой страны",
                    (None, _) => "Проверка не ответила: ничего не ограничиваю",
                };
                ui.label(egui::RichText::new(crate::i18n::t(line)).font(theme::body(12.5)).color(theme::ink3()));
                if kit::btn_if(ui, busy != Some("region"), Some("@refresh"), "Проверить", false).clicked() {
                    self.slow("region", || {
                        crate::services::region::lookup_country(true);
                        Ok(String::new())
                    });
                }
            });
        });
    }

    /// ProxyCard: the person's own proxy for every connection of the program
    fn proxy_card(&mut self, ui: &mut Ui) {
        let saved = crate::userproxy::saved(&self.data_dir);
        svc_card(ui, |ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Свой прокси")).font(theme::heavy(14.0)).color(INK));
            kit::note(ui, "Если сервисы у вас не открываются без VPN, можно пустить через прокси все соединения EvPlayer: и поиск, и сами песни, и скачивание. Подойдёт любой HTTP-прокси, например порт «HTTP» или «mixed» в Clash, v2rayN или Hiddify, которые уже работают у вас на компьютере. Остальные программы это не затрагивает.");
            actions(ui, |ui| {
                let typed = self.pages.services.proxy.get_or_insert_with(|| saved.clone());
                let w = (ui.available_width() - 120.0 - 190.0).clamp(160.0, 320.0);
                let r = kit::input_styled(ui, typed, "Например 127.0.0.1:7890. Пусто: без прокси", w, 40.0, 13, theme::medium500(), false);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let changed = typed.trim() != saved;
                let value = typed.trim().to_string();
                if (kit::btn_if(ui, changed, None, "Сохранить", false).clicked() || (enter && changed)) && changed {
                    match crate::userproxy::save(&self.data_dir, &value) {
                        Ok(()) => {
                            let now = crate::userproxy::saved(&self.data_dir);
                            self.pages.services.proxy = Some(now.clone());
                            self.toast(if now.is_empty() { "Прокси убран. Это заработает после перезапуска EvPlayer" } else { "Прокси сохранён. Он заработает после перезапуска EvPlayer" });
                        }
                        Err(e) => self.toast(&e),
                    }
                }
                kit::pill(ui, if saved.is_empty() { Tone::Off } else { Tone::Ok }, if saved.is_empty() { "Не задан" } else { "Включён после перезапуска" });
            });
        });
    }

    fn youtube_card(&mut self, ui: &mut Ui) {
        svc_card(ui, |ui| {
            head(ui, "Y", Color32::from_rgb(0xff, 0x3b, 0x3b), "YouTube Music", "Поиск песен, альбомов, плейлистов и страницы исполнителей", |ui| {
                kit::pill(ui, Tone::Info, "работает без входа");
            });
            kit::note(ui, "Песни с YouTube играют из их копии на SoundCloud. Если копии нет, трек пропускается.");
            kit::note(ui, "Плейлисты YouTube и YouTube Music можно перенести по ссылке — поле выше.");
        });
    }

    fn spotify_card(&mut self, ui: &mut Ui) {
        use crate::services::spotify;
        let account_now = spotify::account();
        let (id, secret) = {
            let s = state::get().settings.read().unwrap();
            (s.spotify_client_id.clone(), s.spotify_client_secret.clone())
        };
        let app_only = account_now.is_none() && !id.trim().is_empty() && !secret.trim().is_empty();
        let busy = self.busy();
        let importing = self.importing.as_ref().is_some_and(|i| !i.progress.lock().unwrap().done);
        svc_card(ui, |ui| {
            let (tone, label) = if account_now.is_some() {
                (Tone::Ok, "аккаунт подключён")
            } else if app_only {
                (Tone::Ok, "приложение подключено")
            } else {
                (Tone::Info, "плейлисты по ссылке")
            };
            head(ui, "S", Color32::from_rgb(0x1e, 0xd7, 0x60), "Spotify", "Перенос «Любимых треков» и плейлистов любой длины — песни находятся на YouTube Music и SoundCloud", |ui| {
                kit::pill(ui, tone, label);
            });
            if let Some(a) = &account_now {
                let line = format!("{} · {}", if a.product.as_deref() == Some("premium") { "Spotify Premium" } else { "Spotify" }, crate::i18n::t("плейлисты по ссылке читаются целиком"));
                let (_, out) = account(self, ui, a.avatar.as_deref(), None, &a.name, &line, "Выйти");
                if out {
                    spotify::sign_out();
                }
                actions(ui, |ui| {
                    if kit::btn_if(ui, !importing, Some(ic::HEART), "Забрать «Любимые треки»", false).clicked() {
                        self.start_job("Spotify: читаю «Любимые треки»…", |cancel, step| {
                            let named = spotify::liked_songs(Some(cancel), &|n, t| step(&format!("Spotify: прочитано {n} из {t}")))?;
                            if named.is_empty() {
                                return Err("В «Любимых треках» Spotify пусто".into());
                            }
                            let tracks = match_all(&named, cancel, &|d, t, f| step(&format!("Любимые Spotify: {d} из {t}, найдено {f}")));
                            let added = state::get().update_library(|l| l.add_likes(&tracks));
                            Ok((format!("В понравившиеся добавлено {added} из {}", named.len()), Some("likes".into())))
                        });
                    }
                    if kit::btn_if(ui, !importing, Some("@listPlus"), "Забрать плейлисты", false).clicked() {
                        self.start_job("Spotify: читаю плейлисты…", |cancel, step| {
                            let lists = spotify::my_playlists(Some(cancel))?;
                            let mut created = 0;
                            for (i, (pid, name, _, art)) in lists.iter().enumerate() {
                                let read = |n: usize, t: usize| step(&format!("{}/{} «{name}»: читаю {n} из {t}", i + 1, lists.len()));
                                let named = spotify::collection("playlist", pid, Some(cancel), &read).map(|c| c.1).unwrap_or_default();
                                let tracks = match_all(&named, cancel, &|d, t, f| step(&format!("{}/{} «{name}»: {d} из {t}, найдено {f}", i + 1, lists.len())));
                                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                    break;
                                }
                                if !tracks.is_empty() && save_list(&format!("spotify:{pid}"), name, "Из вашего Spotify", art.clone(), &tracks) {
                                    created += 1;
                                }
                            }
                            Ok((if lists.is_empty() { "Плейлистов не нашлось".into() } else { format!("Плейлистов из Spotify: новых {created}") }, Some("library".into())))
                        });
                    }
                });
            } else {
                kit::note(ui, "Без Premium проще всего архив данных Spotify — он переносит все плейлисты целиком. Вход через Spotify тоже забирает плейлисты любой длины, но Spotify открывает свой API только приложениям, владелец которых с Premium (войти в такое приложение можно и с бесплатным аккаунтом).");
                let can = !spotify::client_id().is_empty();
                actions(ui, |ui| {
                    if kit::btn_if(ui, busy.is_none() && can, Some(ic::PLUG), if busy == Some("sp-login") { "Жду вход в браузере…" } else { "Войти через Spotify" }, true).clicked() {
                        self.slow("sp-login", || spotify::sign_in().map(|_| "Spotify подключён".into()));
                    }
                    if widgets::btn(ui, None, "Импорт архива данных", false).clicked() {
                        self.dialog = Some(crate::app::Dialog::SpotifyExport { lists: vec![], chosen: vec![], error: String::new() });
                    }
                });
                if !can {
                    kit::note(ui, "Для входа нужен Client ID приложения Spotify — откройте «Своё приложение Spotify» ниже.");
                }
            }
            let mut open = self.pages.services.sp_more;
            more(ui, &mut open, "Своё приложение Spotify", "Скрыть");
            self.pages.services.sp_more = open;
            if open {
                step(ui, 1, "Нужен аккаунт с Spotify Premium (ваш или друга): только им Spotify разрешает Web API. Откройте developer.spotify.com/dashboard.", Some("https://developer.spotify.com/dashboard"));
                step(ui, 2, "«Create app»: любое имя, Redirect URI — http://127.0.0.1:43821/callback, отметьте «Web API».", None);
                step(ui, 3, "Скопируйте Client ID сюда. Войти в приложение могут только аккаунты, которые владелец добавил в «User Management» (до 25), — они могут быть и без Premium.", None);
                let label = if spotify::builtin_client_id() { format!("Client ID {}", crate::i18n::t("(пусто — встроенный EvPlayer)")) } else { "Client ID".to_string() };
                if let Some(v) = field(ui, &label, &id, "", false, None) {
                    state::get().set_settings(|s| s.spotify_client_id = v.trim().to_string());
                }
                if let Some(v) = field(ui, "Client Secret — необязательно", &secret, "", true, Some("Только если не хотите входить: тогда по ссылке читаются публичные плейлисты, но без «Любимых треков». Хранится только на этом компьютере.")) {
                    state::get().set_settings(|s| s.spotify_client_secret = v.trim().to_string());
                }
            }
        });
    }

    fn yandex_card(&mut self, ui: &mut Ui) {
        use crate::services::yandex;
        let account_now = yandex::account();
        let busy = self.busy();
        let importing = self.importing.as_ref().is_some_and(|i| !i.progress.lock().unwrap().done);
        svc_card(ui, |ui| {
            let connected = account_now.is_some();
            head(ui, "Я", Color32::from_rgb(0xff, 0xcc, 0x00), "Яндекс Музыка", "Перенос «Мне нравится», плейлистов и альбомов — песни находятся на YouTube Music и SoundCloud", |ui| {
                kit::pill(ui, if connected { Tone::Ok } else { Tone::Info }, if connected { "профиль подключён" } else { "перенос" });
            });
            if let Some(a) = &account_now {
                let line = format!("@{} · {} {}", a.login, crate::i18n::t("публичных плейлистов:"), a.playlists);
                let letter: String = a.name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                let (_, off) = account(self, ui, None, Some(&letter), &a.name, &line, "Отключить");
                if off {
                    yandex::disconnect();
                }
                let login = a.login.clone();
                actions(ui, |ui| {
                    if kit::btn_if(ui, !importing, Some(ic::HEART), "Забрать «Мне нравится»", false).clicked() {
                        let login = login.clone();
                        self.start_job("Яндекс Музыка: читаю «Мне нравится»…", move |cancel, step| {
                            let named = yandex::likes(&login)?;
                            if named.is_empty() {
                                return Err("«Мне нравится» пусто или закрыто — откройте его в настройках приватности Яндекс Музыки".into());
                            }
                            let tracks = match_all(&named, cancel, &|d, t, f| step(&format!("Яндекс «Мне нравится»: {d} из {t}, найдено {f}")));
                            let added = state::get().update_library(|l| l.add_likes(&tracks));
                            Ok((format!("В понравившиеся добавлено {added} из {}", named.len()), Some("likes".into())))
                        });
                    }
                    if kit::btn_if(ui, !importing, Some("@listPlus"), "Забрать плейлисты", false).clicked() {
                        let login = login.clone();
                        self.start_job("Яндекс Музыка: читаю плейлисты…", move |cancel, step| {
                            let profile = yandex::profile(&login)?;
                            let mut created = 0;
                            let n = profile.playlists.len();
                            for (i, (pid, title, cover)) in profile.playlists.iter().enumerate() {
                                let named = yandex::playlist(&profile.login, pid).unwrap_or_default();
                                let tracks = match_all(&named, cancel, &|d, t, f| step(&format!("{}/{n} «{title}»: {d} из {t}, найдено {f}", i + 1)));
                                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                    break;
                                }
                                if !tracks.is_empty() && save_list(&format!("ym:{}/{pid}", profile.login), title, "Из вашей Яндекс Музыки", cover.clone(), &tracks) {
                                    created += 1;
                                }
                            }
                            Ok((if n > 0 { format!("Плейлистов из Яндекс Музыки: новых {created}") } else { "Публичных плейлистов не нашлось".into() }, Some("library".into())))
                        });
                    }
                });
            } else {
                let mut text = std::mem::take(&mut self.pages.services.ym_login);
                if connect_box(ui, &mut text, "Логин или ссылка: music.yandex.ru/users/ваш-логин", if busy == Some("ym") { "Подключаю…" } else { "Подключить" }, busy.is_none()) {
                    let input = text.trim().to_string();
                    self.slow("ym", move || yandex::connect(&input).map(|_| "Профиль Яндекс Музыки подключён".into()));
                }
                self.pages.services.ym_login = text;
            }
            kit::note(ui, "Читаются публичные плейлисты и «Мне нравится» (в Яндекс Музыке: Настройки → Приватность → открыть). Пароль не нужен. Отдельный плейлист или альбом можно перенести по ссылке — поле вверху.");
        });
    }

    fn other_card(&mut self, ui: &mut Ui) {
        let files = self.library.list().len();
        svc_card(ui, |ui| {
            let line = if files > 0 { format!("{} {}", crate::i18n::tracks(files), crate::i18n::t("с компьютера")) } else { crate::i18n::t("MP3, FLAC, M4A, OGG, WAV — с папками, которые обновляются сами") };
            let mut open = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let b = badge(ui, "", Color32::from_rgb(0x8a, 0xb4, 0xff));
                crate::ui::icons::paint(ui.painter(), "folder", Rect::from_center_size(b.center(), vec2(18.0, 18.0)), Color32::WHITE);
                let text_w = ui.available_width() - 110.0;
                ui.vertical(|ui| {
                    ui.set_width(text_w);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(egui::RichText::new(crate::i18n::t("Свои файлы")).font(theme::heavy(15.0)).color(INK));
                    ui.label(egui::RichText::new(line).font(theme::body(12.5)).color(theme::ink3()));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    open = widgets::btn(ui, None, "Открыть", false).clicked();
                });
            });
            if open {
                self.go(crate::app::Page::Files);
            }
            divider(ui);
            head(ui, "D", Color32::from_rgb(0xa2, 0x38, 0xff), "Deezer", "Плейлисты и альбомы переносятся по ссылке", |ui| {
                kit::pill(ui, Tone::Info, "перенос");
            });
            divider(ui);
            head(ui, "A", Color32::from_rgb(0xfa, 0x2d, 0x48), "Apple Music", "Apple не открывает плейлисты сторонним программам. Перенесите их в Spotify (например, через Soundiiz) и импортируйте оттуда", |ui| {
                kit::pill(ui, Tone::Off, "недоступно");
            });
        });
    }
}

/// a switch row with the refresh icon in front (the sync settings)
fn refresh_switch(ui: &mut Ui, title: &str, hint: &str, on: bool) -> Option<bool> {
    let icon = |ui: &mut Ui, r: Rect| {
        ui.painter().rect_filled(r, 10, Color32::from_rgba_unmultiplied(238, 247, 244, 18));
        crate::ui::icons::paint(ui.painter(), "refresh", Rect::from_center_size(r.center(), vec2(16.0, 16.0)), theme::ink2());
    };
    let mut v = on;
    let mut flipped = false;
    let r = kit::row_full(ui, Some(&icon), title, hint, true, false, |ui| {
        if kit::switch(ui, &mut v).changed() {
            flipped = true;
        }
    });
    if r.clicked() && !flipped {
        v = !on;
        flipped = true;
    }
    flipped.then_some(v)
}

impl App {
    /// run a slow step on a thread; `done` says what to show when it ends
    fn slow(&mut self, name: &'static str, job: impl FnOnce() -> Result<String, String> + Send + 'static) {
        let busy = self.pages.services.busy.clone();
        if busy.lock().unwrap().is_some() {
            return;
        }
        *busy.lock().unwrap() = Some(name);
        std::thread::spawn(move || {
            let r = job();
            *busy.lock().unwrap() = None;
            match r {
                Ok(text) if !text.is_empty() => crate::app::toast_later(crate::i18n::t(&text)),
                Ok(_) => crate::events::repaint(),
                Err(e) => crate::app::toast_later(crate::i18n::t(&e)),
            }
        });
    }

    fn busy(&self) -> Option<&'static str> {
        *self.pages.services.busy.lock().unwrap()
    }

    /// a long carry-over in the import panel: `job` reads the names, finds the songs, saves them
    pub fn start_job(&mut self, first: &str, job: impl FnOnce(&AtomicBool, &(dyn Fn(&str) + Sync)) -> Result<(String, Option<String>), String> + Send + 'static) {
        if let Some(i) = &self.importing {
            i.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let progress = std::sync::Arc::new(std::sync::Mutex::new(crate::import::Progress { text: crate::i18n::t(first), ..Default::default() }));
        let (c, p) = (cancel.clone(), progress.clone());
        std::thread::spawn(move || {
            let set = |t: &str| {
                p.lock().unwrap().text = crate::i18n::t(t);
                crate::events::repaint();
            };
            let r = job(&c, &set);
            let stopped = c.load(std::sync::atomic::Ordering::Relaxed);
            let mut g = p.lock().unwrap();
            match r {
                Ok((text, opened)) if !stopped => {
                    g.ok = true;
                    g.text = crate::i18n::t(&text);
                    g.opened = opened;
                }
                _ if stopped => g.text = crate::i18n::t("Загрузка отменена"),
                Err(e) => g.text = format!("{} {e}", crate::i18n::t("Не загрузилось:")),
                Ok(_) => {}
            }
            g.done = true;
            drop(g);
            crate::events::repaint();
        });
        self.importing = Some(crate::app::Importing { cancel, progress, finished_at: None });
    }
}

/// a list carried over from a service: new songs into the playlist made last time (by its
/// source), or a new playlist; true when one was made
pub fn save_list(source: &str, title: &str, description: &str, cover: Option<String>, tracks: &[crate::model::Track]) -> bool {
    state::get().update_library(|l| {
        if let Some(existing) = l.playlists.iter().find(|p| p.source.as_deref() == Some(source)).map(|p| p.id.clone()) {
            l.add_to_playlist(&existing, tracks);
            false
        } else {
            let id = l.create_playlist(title, Some(description), Some(source));
            if let Some(p) = l.playlist_mut(&id) {
                p.icon = cover;
            }
            l.add_to_playlist(&id, tracks);
            true
        }
    })
}

/// the chosen lists of a Spotify data archive, found and saved one after another
pub fn import_spotify_export(app: &mut App, lists: Vec<crate::spotify_export::List>) {
    app.start_job("Spotify: начинаю…", move |cancel, step| {
        let mut last: Option<String> = None;
        let mut summary = Vec::new();
        let n = lists.len();
        for (i, p) in lists.iter().enumerate() {
            let prefix = if n > 1 { format!("{}/{n} · ", i + 1) } else { String::new() };
            let named: Vec<Named> = p.tracks.clone();
            let tracks = match_all(&named, cancel, &|d, t, f| step(&format!("{prefix}«{}»: {d} из {t}, найдено {f}", p.title)));
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if tracks.is_empty() {
                continue;
            }
            if p.kind == crate::spotify_export::Kind::Liked {
                state::get().update_library(|l| l.add_likes(&tracks));
                summary.push(format!("{} {} {} {}", crate::i18n::t("в понравившиеся"), tracks.len(), crate::i18n::t("из"), p.tracks.len()));
                last.get_or_insert("likes".into());
            } else {
                let id = state::get().update_library(|l| {
                    let id = l.create_playlist(&p.title, Some(&p.description), None);
                    l.add_to_playlist(&id, &tracks);
                    id
                });
                summary.push(format!("«{}»: {} {} {}", p.title, tracks.len(), crate::i18n::t("из"), p.tracks.len()));
                last = Some(id);
            }
        }
        Ok((format!("{} · {}", crate::i18n::t("Готово"), summary.join(", ")), last))
    });
}
