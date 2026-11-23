//! Настройки → Сервисы и импорт (src/pages/settings/ServicesTab.tsx): where EvPlayer takes
//! music from and how your playlists come over from other services. The slow steps (connecting
//! a profile, signing in, carrying over a library) run on threads; their result comes as a
//! toast, a long carry-over in the import panel at the bottom.

use std::sync::atomic::AtomicBool;

use egui::{vec2, Ui};

use crate::app::App;
use crate::import::{match_all, Named};
use crate::state;
use crate::ui::theme::{self, ic, ACCENT, INK};
use crate::ui::widgets;

/// what the services tab remembers between frames
#[derive(Default)]
pub struct ServicesState {
    pub link: String,
    pub sc_profile: String,
    pub ym_login: String,
    pub sc_more: bool,
    pub sp_more: bool,
    /// a slow step running: its name (buttons wait for it)
    pub busy: std::sync::Arc<std::sync::Mutex<Option<&'static str>>>,
}

/// a coloured letter for a service
fn badge(ui: &mut Ui, letter: &str, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(38.0, 38.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 12, color.gamma_multiply(0.22));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, letter, theme::heavy(17.0), color);
}

/// a small rounded label with the service's state (its own height, whatever the row's)
fn pill(ui: &mut Ui, text: &str, on: bool) {
    let color = if on { ACCENT } else { theme::ink2() };
    let g = ui.painter().layout_no_wrap(crate::i18n::t(text), theme::medium(11.5), color);
    let (rect, _) = ui.allocate_exact_size(vec2(g.size().x + 18.0, 22.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 11, color.gamma_multiply(0.14));
    ui.painter().galley(rect.center() - g.size() / 2.0, g, color);
}

/// a service card's head: badge, name, what it does, its state
fn head(ui: &mut Ui, letter: &str, color: egui::Color32, name: &str, what: &str, state_text: &str, on: bool) {
    ui.horizontal(|ui| {
        badge(ui, letter, color);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() - 190.0);
            ui.label(egui::RichText::new(name).font(theme::heavy(15.0)).color(INK));
            ui.label(egui::RichText::new(crate::i18n::t(what)).font(theme::body(12.0)).color(theme::ink2()));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| pill(ui, state_text, on));
    });
}

fn small(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(crate::i18n::t(text)).font(theme::body(12.0)).color(theme::ink3()));
}

fn card(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    widgets::glass_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 8.0;
        body(ui);
    });
    ui.add_space(10.0);
}

/// avatar, name and a line under it, with a button at the right
fn account_row(app: &mut App, ui: &mut Ui, avatar: Option<&str>, name: &str, line: &str, button: &str) -> (bool, bool) {
    let mut name_clicked = false;
    let mut pressed = false;
    ui.horizontal(|ui| {
        widgets::cover(ui, &mut app.images, avatar, 40.0, 20);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            name_clicked = ui.add(egui::Label::new(egui::RichText::new(name).font(theme::medium(14.0)).color(INK)).sense(egui::Sense::click())).clicked();
            ui.label(egui::RichText::new(line).font(theme::body(12.0)).color(theme::ink2()));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            pressed = widgets::pill_button(ui, None, button, false).clicked();
        });
    });
    (name_clicked, pressed)
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

    pub fn settings_services(&mut self, ui: &mut Ui) {
        App::muted(ui, "Откуда EvPlayer берёт музыку и как перенести ваши плейлисты из других сервисов.");
        ui.add_space(8.0);
        self.import_box(ui);
        self.soundcloud_card(ui);
        card(ui, |ui| {
            head(ui, "Y", egui::Color32::from_rgb(0xff, 0x3b, 0x3b), "YouTube Music", "Поиск песен, альбомов, плейлистов и страницы исполнителей", "работает без входа", false);
            small(ui, "Песни с YouTube играют из их копии на SoundCloud. Если копии нет, трек пропускается.");
            small(ui, "Плейлисты YouTube и YouTube Music можно перенести по ссылке — поле выше.");
        });
        self.spotify_card(ui);
        self.yandex_card(ui);
        self.other_card(ui);
    }

    fn import_box(&mut self, ui: &mut Ui) {
        widgets::glass_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(crate::i18n::t("Импорт по ссылке")).font(theme::medium(14.0)).color(INK));
            small(ui, "Плейлист, альбом или трек станет вашим плейлистом. Песни из Spotify и Deezer находятся автоматически");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let s = &mut self.pages.services;
                let kind = crate::import::link_kind(&s.link);
                let w = ui.available_width() - if kind.is_some() { 260.0 } else { 150.0 };
                let r = ui.add(egui::TextEdit::singleline(&mut s.link).hint_text(crate::i18n::t("Ссылка из Spotify, Яндекс Музыки, YouTube, SoundCloud, Deezer или код EVP…")).desired_width(w));
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
                    pill(ui, name, true);
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let busy = self.importing.as_ref().is_some_and(|i| !i.progress.lock().unwrap().done);
                if (widgets::pill_button(ui, Some(ic::DOWNLOAD), "Импортировать", true).clicked() || enter) && !self.pages.services.link.trim().is_empty() && !busy {
                    if kind.is_none() {
                        self.toast("Это не похоже на ссылку на плейлист, альбом или трек");
                    } else {
                        let text = std::mem::take(&mut self.pages.services.link).trim().to_string();
                        self.start_import(text, None);
                    }
                }
            });
        });
        ui.add_space(10.0);
    }

    fn soundcloud_card(&mut self, ui: &mut Ui) {
        use crate::services::scaccount;
        let account = scaccount::account();
        let busy = self.busy();
        card(ui, |ui| {
            head(ui, "S", egui::Color32::from_rgb(0xff, 0x7a, 0x1a), "SoundCloud", "Поиск, чарты, исполнители и полноценное воспроизведение", if account.is_some() { "аккаунт подключён" } else { "работает без входа" }, account.is_some());
            if let Some(a) = &account {
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
                let (open, off) = account_row(self, ui, a.avatar.as_deref(), &a.name, &parts.join(" · "), "Отключить");
                if open {
                    self.go(crate::app::Page::Artist(format!("scu:{}", a.id.trim_start_matches("scu:").trim_start_matches("sc:"))));
                }
                if off {
                    scaccount::disconnect();
                }
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(busy.is_none(), |ui| {
                        if widgets::pill_button(ui, Some(ic::HEART), if busy == Some("likes") { "Забираю лайки…" } else { "Забрать лайки" }, false).clicked() {
                            self.slow("likes", || scaccount::import_likes().map(|n| if n > 0 { format!("В понравившиеся добавлено {n}") } else { "Новых лайков нет".into() }));
                        }
                        if widgets::pill_button(ui, Some(ic::LIST), if busy == Some("playlists") { "Забираю плейлисты…" } else { "Забрать плейлисты" }, false).clicked() {
                            self.slow("playlists", || scaccount::import_playlists(&|_, _| {}).map(|(c, u)| if c + u > 0 { format!("Плейлистов: новых {c}, обновлено {u}") } else { "Публичных плейлистов не нашлось".into() }));
                        }
                    });
                });
                if let Some(v) = widgets::toggle_row(ui, "Синхронизировать лайки", "При запуске новые лайки с SoundCloud сами появятся в «Понравившихся»", a.sync_likes, true) {
                    let mut a = a.clone();
                    a.sync_likes = v;
                    scaccount::save_account(Some(&a));
                }
            } else {
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 150.0;
                    let r = ui.add(egui::TextEdit::singleline(&mut self.pages.services.sc_profile).hint_text(crate::i18n::t("Ссылка на профиль или имя: soundcloud.com/ваше-имя")).desired_width(w));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let input = self.pages.services.sc_profile.trim().to_string();
                    if (widgets::pill_button(ui, Some(ic::LINK), if busy == Some("connect") { "Подключаю…" } else { "Подключить" }, true).clicked() || enter) && !input.is_empty() && busy.is_none() {
                        self.slow("connect", move || {
                            scaccount::connect(&input)?;
                            let n = scaccount::import_likes().unwrap_or(0);
                            Ok(if n > 0 { format!("Аккаунт SoundCloud подключён · в понравившиеся добавлено {n}") } else { "Аккаунт SoundCloud подключён".into() })
                        });
                    }
                });
            }
            small(ui, "Подтягиваются ваши публичные лайки и плейлисты. Пароль не нужен и никуда не передаётся.");
            ui.separator();
            self.soundcloud_sync(ui);
            ui.separator();
            if ui.add(egui::Label::new(egui::RichText::new(crate::i18n::t(if self.pages.services.sc_more { "Скрыть дополнительно" } else { "Дополнительно" })).font(theme::medium(12.5)).color(ACCENT)).sense(egui::Sense::click())).clicked() {
                self.pages.services.sc_more = !self.pages.services.sc_more;
            }
            if self.pages.services.sc_more {
                let mut id = state::get().settings.read().unwrap().soundcloud_client_id.clone();
                ui.label(egui::RichText::new(crate::i18n::t("client_id своего приложения SoundCloud")).font(theme::medium(12.5)).color(INK));
                if ui.add(egui::TextEdit::singleline(&mut id).hint_text(crate::i18n::t("Пусто — определяется автоматически")).desired_width(f32::INFINITY)).changed() {
                    state::get().set_settings(|s| s.soundcloud_client_id = id.trim().to_string());
                }
                small(ui, "Если у вас есть приложение на soundcloud.com/you/apps, EvPlayer будет ходить в SoundCloud от его имени.");
            }
        });
        self.region_and_proxy(ui);
    }

    /// "Вход и полная синхронизация"
    fn soundcloud_sync(&mut self, ui: &mut Ui) {
        use crate::services::scaccount;
        let busy = self.busy();
        ui.label(egui::RichText::new(crate::i18n::t("Вход и полная синхронизация")).font(theme::medium(13.5)).color(INK));
        if let Some(l) = scaccount::login() {
            let st = scaccount::status();
            let line = if st.running { crate::i18n::t("синхронизирую…") } else if let Some(t) = &st.last_run { format!("{} {}", crate::i18n::t("последняя синхронизация"), super::library::ago(t)) } else { crate::i18n::t("вход выполнен") };
            let (_, out) = account_row(self, ui, l.avatar.as_deref(), &l.name, &line, "Выйти");
            if out {
                scaccount::sign_out();
            }
            let full = scaccount::full_sync();
            if let Some(v) = widgets::toggle_row(ui, "Включить полную синхронизацию", "Лайки и плейлисты из плеера появляются в вашем аккаунте SoundCloud, а удалённые здесь удаляются и там", full, true) {
                if v {
                    self.dialog = Some(crate::app::Dialog::FullSync);
                } else {
                    state::get().set_extra("soundcloudFullSync", serde_json::Value::Bool(false));
                }
            }
            if full {
                ui.add_enabled_ui(!st.running, |ui| {
                    if widgets::pill_button(ui, Some(ic::SYNC), if st.running { "Синхронизирую…" } else { "Синхронизировать сейчас" }, false).clicked() {
                        std::thread::spawn(scaccount::sync_now);
                    }
                });
            }
            if let Some(e) = &st.error {
                ui.label(egui::RichText::new(e).font(theme::body(12.0)).color(egui::Color32::from_rgb(0xff, 0x6b, 0x6b)));
            }
        } else {
            let id_set = !state::get().settings.read().unwrap().soundcloud_client_id.trim().is_empty();
            let mut secret = state::get().extra("soundcloudClientSecret").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
            ui.label(egui::RichText::new(crate::i18n::t("client_secret своего приложения")).font(theme::medium(12.5)).color(INK));
            if ui.add(egui::TextEdit::singleline(&mut secret).password(true).hint_text(crate::i18n::t("Вместе с client_id из «Дополнительно»")).desired_width(f32::INFINITY)).changed() {
                state::get().set_extra("soundcloudClientSecret", serde_json::Value::String(secret.trim().to_string()));
            }
            ui.add_enabled_ui(busy.is_none() && id_set && !secret.trim().is_empty(), |ui| {
                if widgets::pill_button(ui, Some(ic::LINK), if busy == Some("sc-login") { "Жду вход в браузере…" } else { "Войти через OAuth" }, true).clicked() {
                    self.slow("sc-login", || scaccount::sign_in().map(|_| "Вход в SoundCloud выполнен".into()));
                }
            });
            small(ui, "Нужно своё приложение на soundcloud.com/you/apps: его client_id впишите в «Дополнительно», а в Redirect URI укажите http://127.0.0.1:43821/callback. Вход проходит на сайте SoundCloud, пароль в EvPlayer не попадает.");
        }
    }

    fn spotify_card(&mut self, ui: &mut Ui) {
        use crate::services::spotify;
        let account = spotify::account();
        let (id, secret) = {
            let s = state::get().settings.read().unwrap();
            (s.spotify_client_id.clone(), s.spotify_client_secret.clone())
        };
        let app_only = account.is_none() && !id.trim().is_empty() && !secret.trim().is_empty();
        let busy = self.busy();
        let importing = self.importing.as_ref().is_some_and(|i| !i.progress.lock().unwrap().done);
        card(ui, |ui| {
            head(ui, "S", egui::Color32::from_rgb(0x1e, 0xd7, 0x60), "Spotify", "Перенос «Любимых треков» и плейлистов любой длины — песни находятся на YouTube Music и SoundCloud", if account.is_some() { "аккаунт подключён" } else if app_only { "приложение подключено" } else { "плейлисты по ссылке" }, account.is_some() || app_only);
            if let Some(a) = &account {
                let line = format!("{} · {}", if a.product.as_deref() == Some("premium") { "Spotify Premium" } else { "Spotify" }, crate::i18n::t("плейлисты по ссылке читаются целиком"));
                let (_, out) = account_row(self, ui, a.avatar.as_deref(), &a.name, &line, "Выйти");
                if out {
                    spotify::sign_out();
                }
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!importing, |ui| {
                        if widgets::pill_button(ui, Some(ic::HEART), "Забрать «Любимые треки»", false).clicked() {
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
                        if widgets::pill_button(ui, Some(ic::LIST), "Забрать плейлисты", false).clicked() {
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
                });
            } else {
                small(ui, "Без Premium проще всего архив данных Spotify — он переносит все плейлисты целиком. Вход через Spotify тоже забирает плейлисты любой длины, но Spotify открывает свой API только приложениям, владелец которых с Premium (войти в такое приложение можно и с бесплатным аккаунтом).");
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(busy.is_none() && !spotify::client_id().is_empty(), |ui| {
                        if widgets::pill_button(ui, Some(ic::LINK), if busy == Some("sp-login") { "Жду вход в браузере…" } else { "Войти через Spotify" }, true).clicked() {
                            self.slow("sp-login", || spotify::sign_in().map(|_| "Spotify подключён".into()));
                        }
                    });
                    if widgets::pill_button(ui, None, "Импорт архива данных", false).clicked() {
                        self.dialog = Some(crate::app::Dialog::SpotifyExport { lists: vec![], chosen: vec![], error: String::new() });
                    }
                });
                if spotify::client_id().is_empty() {
                    small(ui, "Для входа нужен Client ID приложения Spotify — откройте «Своё приложение Spotify» ниже.");
                }
            }
            if ui.add(egui::Label::new(egui::RichText::new(crate::i18n::t(if self.pages.services.sp_more { "Скрыть" } else { "Своё приложение Spotify" })).font(theme::medium(12.5)).color(ACCENT)).sense(egui::Sense::click())).clicked() {
                self.pages.services.sp_more = !self.pages.services.sp_more;
            }
            if self.pages.services.sp_more {
                small(ui, "1. Нужен аккаунт с Spotify Premium (ваш или друга): только им Spotify разрешает Web API. Откройте developer.spotify.com/dashboard.");
                small(ui, "2. «Create app»: любое имя, Redirect URI — http://127.0.0.1:43821/callback, отметьте «Web API».");
                small(ui, "3. Скопируйте Client ID сюда. Войти в приложение могут только аккаунты, которые владелец добавил в «User Management» (до 25), — они могут быть и без Premium.");
                if widgets::pill_button(ui, None, "developer.spotify.com/dashboard", false).clicked() {
                    crate::dialogs::open_path(std::path::Path::new("https://developer.spotify.com/dashboard"));
                }
                let (mut cid, mut sec) = (id.clone(), secret.clone());
                ui.label(egui::RichText::new("Client ID").font(theme::medium(12.5)).color(INK));
                if ui.add(egui::TextEdit::singleline(&mut cid).desired_width(f32::INFINITY)).changed() {
                    state::get().set_settings(|s| s.spotify_client_id = cid.trim().to_string());
                }
                ui.label(egui::RichText::new(crate::i18n::t("Client Secret — необязательно")).font(theme::medium(12.5)).color(INK));
                if ui.add(egui::TextEdit::singleline(&mut sec).password(true).desired_width(f32::INFINITY)).changed() {
                    state::get().set_settings(|s| s.spotify_client_secret = sec.trim().to_string());
                }
                small(ui, "Только если не хотите входить: тогда по ссылке читаются публичные плейлисты, но без «Любимых треков». Хранится только на этом компьютере.");
            }
        });
    }

    fn yandex_card(&mut self, ui: &mut Ui) {
        use crate::services::yandex;
        let account = yandex::account();
        let busy = self.busy();
        let importing = self.importing.as_ref().is_some_and(|i| !i.progress.lock().unwrap().done);
        card(ui, |ui| {
            head(ui, "Я", egui::Color32::from_rgb(0xff, 0xcc, 0x00), "Яндекс Музыка", "Перенос «Мне нравится», плейлистов и альбомов — песни находятся на YouTube Music и SoundCloud", if account.is_some() { "профиль подключён" } else { "перенос" }, account.is_some());
            if let Some(a) = &account {
                let line = format!("@{} · {} {}", a.login, crate::i18n::t("публичных плейлистов:"), a.playlists);
                let (_, off) = account_row(self, ui, None, &a.name, &line, "Отключить");
                if off {
                    yandex::disconnect();
                }
                let login = a.login.clone();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!importing, |ui| {
                        if widgets::pill_button(ui, Some(ic::HEART), "Забрать «Мне нравится»", false).clicked() {
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
                        if widgets::pill_button(ui, Some(ic::LIST), "Забрать плейлисты", false).clicked() {
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
                });
            } else {
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 150.0;
                    let r = ui.add(egui::TextEdit::singleline(&mut self.pages.services.ym_login).hint_text(crate::i18n::t("Логин или ссылка: music.yandex.ru/users/ваш-логин")).desired_width(w));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let input = self.pages.services.ym_login.trim().to_string();
                    if (widgets::pill_button(ui, Some(ic::LINK), if busy == Some("ym") { "Подключаю…" } else { "Подключить" }, true).clicked() || enter) && !input.is_empty() && busy.is_none() {
                        self.slow("ym", move || yandex::connect(&input).map(|_| "Профиль Яндекс Музыки подключён".into()));
                    }
                });
            }
            small(ui, "Читаются публичные плейлисты и «Мне нравится» (в Яндекс Музыке: Настройки → Приватность → открыть). Пароль не нужен. Отдельный плейлист или альбом можно перенести по ссылке — поле вверху.");
        });
    }

    fn other_card(&mut self, ui: &mut Ui) {
        let files = self.library.list().len();
        card(ui, |ui| {
            ui.horizontal(|ui| {
                badge(ui, "♪", egui::Color32::from_rgb(0x8a, 0xb4, 0xff));
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 120.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Свои файлы")).font(theme::heavy(15.0)).color(INK));
                    let line = if files > 0 { format!("{} {}", crate::i18n::tracks(files), crate::i18n::t("с компьютера")) } else { crate::i18n::t("MP3, FLAC, M4A, OGG, WAV — с папками, которые обновляются сами") };
                    ui.label(egui::RichText::new(line).font(theme::body(12.0)).color(theme::ink2()));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::pill_button(ui, None, "Открыть", false).clicked() {
                        self.go(crate::app::Page::Files);
                    }
                });
            });
            head(ui, "D", egui::Color32::from_rgb(0xa2, 0x38, 0xff), "Deezer", "Плейлисты и альбомы переносятся по ссылке", "перенос", false);
            head(ui, "A", egui::Color32::from_rgb(0xfa, 0x2d, 0x48), "Apple Music", "Apple не открывает плейлисты сторонним программам. Перенесите их в Spotify (например, через Soundiiz) и импортируйте оттуда", "недоступно", false);
        });
    }

    /// Защита региона and the proxy (RegionGuardCard, ProxyCard)
    fn region_and_proxy(&mut self, ui: &mut Ui) {
        widgets::glass_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(crate::i18n::t("Защита региона")).font(theme::medium(14.0)).color(INK));
            let mut on = state::get().settings.read().unwrap().region_guard;
            if let Some(v) = widgets::toggle_row(ui, "Не обращаться к SoundCloud из выбранных стран", "Песни тогда играют из копии на YouTube Music", on, true) {
                on = v;
                state::get().set_settings(|s| s.region_guard = on);
            }
            let countries = state::get().settings.read().unwrap().region_countries.join(", ");
            let mut text = countries.clone();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Страны")).color(theme::ink2()));
                if ui.add(egui::TextEdit::singleline(&mut text).desired_width(160.0)).lost_focus() && text != countries {
                    let list: Vec<String> = text.split(',').filter_map(crate::services::region::normalize_country).collect();
                    state::get().set_settings(|s| s.region_countries = list);
                }
                small(ui, "двухбуквенные коды через запятую");
            });
            let known = crate::services::region::known_country();
            small(ui, &format!("{}: {}", crate::i18n::t("Страна подключения"), known.unwrap_or_else(|| "—".into())));
        });
        ui.add_space(10.0);
        widgets::glass_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(crate::i18n::t("Прокси")).font(theme::medium(14.0)).color(INK));
            let saved = crate::userproxy::saved(&self.data_dir);
            let mut text = saved.clone();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Свой HTTP-прокси")).color(theme::ink2()));
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(200.0).hint_text("127.0.0.1:2080"));
                if r.lost_focus() && text != saved {
                    if let Err(e) = crate::userproxy::save(&self.data_dir, &text) {
                        self.toast(&e);
                    }
                }
            });
            small(ui, "host:port — для всех запросов программы, с перезапуска");
        });
        ui.add_space(10.0);
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
