//! Настройки, with the page's sections (src/pages/SettingsPage.tsx and settings/*): the
//! playback, the sound, the services, the look, the keys, the window, Discord, the data, and
//! "О программе" with what the program takes from the computer right now.

use egui::{vec2, Ui};

use crate::app::App;
use crate::state;
use crate::store::state::{Language, LyricsSize, Settings};
use crate::ui::theme::{self, ic, ACCENT, INK};
use crate::ui::widgets;

/// the diagnostics tab's place: shown once turned on (seven clicks on the version)
const DIAGNOSTICS: usize = 9;

const TABS: [(&str, &str, &str); 11] = [
    ("Музыка", ic::PLAY, "Воспроизведение"),
    ("", ic::EQ, "Звук и эквалайзер"),
    ("", ic::SYNC, "Сервисы и импорт"),
    ("Приложение", ic::SPARK, "Внешний вид"),
    ("", ic::KEYBOARD, "Клавиши"),
    ("", ic::MINI, "Мини-плеер"),
    ("", ic::MINI, "Окно и трей"),
    ("", ic::DISCORD, "Discord"),
    ("Другое", ic::FOLDER, "Данные и копия"),
    ("", ic::CHART, "Диагностика"),
    ("", ic::INFO, "О программе"),
];

/// a setting: its name and hint on the left, its control on the right
fn row(ui: &mut Ui, title: &str, hint: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(42.0);
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() - 200.0);
            if !title.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::t(title)).font(theme::medium(13.5)).color(INK));
            }
            if !hint.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::t(hint)).font(theme::body(12.0)).color(theme::ink2()));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            control(ui);
        });
    });
}

fn switch(ui: &mut Ui, title: &str, hint: &str, get: impl Fn(&Settings) -> bool, set: impl Fn(&mut Settings, bool)) {
    let mut v = get(&state::get().settings.read().unwrap());
    row(ui, title, hint, |ui| {
        if widgets::toggle(ui, &mut v).changed() {
            state::get().set_settings(|s| set(s, v));
        }
    });
}

fn card(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    widgets::glass_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(crate::i18n::t(title)).font(theme::medium(14.0)).color(INK));
        ui.add_space(6.0);
        body(ui);
    });
    ui.add_space(10.0);
}

impl App {
    pub fn settings(&mut self, ui: &mut Ui) {
        self.page_title(ui, "Настройки");
        let avail = ui.available_rect_before_wrap();
        let side = egui::Rect::from_min_size(avail.min, vec2(220.0, avail.height().min(520.0)));
        let body = egui::Rect::from_min_max(egui::pos2(side.right() + 18.0, avail.top()), avail.max);
        ui.painter().rect_filled(side, 18, theme::glass());
        let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink(10.0)).layout(egui::Layout::top_down(egui::Align::Min)));
        let dev_tools = state::get().settings.read().unwrap().dev_tools;
        if self.pages.settings_tab == DIAGNOSTICS && !dev_tools {
            self.pages.settings_tab = 0;
        }
        for (i, (group, icon, name)) in TABS.iter().enumerate() {
            if i == DIAGNOSTICS && !dev_tools {
                continue;
            }
            if !group.is_empty() {
                sui.add_space(if i == 0 { 2.0 } else { 10.0 });
                sui.label(egui::RichText::new(crate::i18n::t(group).to_uppercase()).font(theme::medium(10.5)).color(theme::ink3()));
            }
            let (rect, r) = sui.allocate_exact_size(vec2(side.width() - 20.0, 32.0), egui::Sense::click());
            let on = self.pages.settings_tab == i;
            if on {
                sui.painter().rect_filled(rect, 10, INK);
            } else if r.hovered() {
                sui.painter().rect_filled(rect, 10, theme::hover());
            }
            let fg = if on { egui::Color32::from_rgb(8, 38, 34) } else { theme::ink2() };
            sui.painter().text(rect.left_center() + vec2(14.0, 0.0), egui::Align2::CENTER_CENTER, *icon, theme::icon(12.0), fg);
            sui.painter().text(rect.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, crate::i18n::t(name), theme::medium(13.0), if on { egui::Color32::from_rgb(8, 38, 34) } else { INK });
            if r.clicked() {
                self.pages.settings_tab = i;
            }
        }
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        egui::ScrollArea::vertical().id_salt(("settings", self.pages.settings_tab)).auto_shrink([false, false]).show(&mut bui, |ui| {
            ui.set_width(ui.available_width() - 10.0);
            match self.pages.settings_tab {
                0 => self.settings_playback(ui),
                1 => self.settings_sound(ui),
                2 => self.settings_services(ui),
                3 => self.settings_look(ui),
                4 => self.settings_keys(ui),
                5 => self.settings_overlay(ui),
                6 => self.settings_window(ui),
                7 => self.settings_discord(ui),
                8 => self.settings_data(ui),
                DIAGNOSTICS => self.settings_diagnostics(ui),
                _ => self.settings_about(ui),
            }
        });
    }

    fn settings_playback(&mut self, ui: &mut Ui) {
        card(ui, "Очередь", |ui| {
            switch(ui, "Автовоспроизведение", "Когда очередь закончится, играть похожие треки", |s| s.autoplay, |s, v| s.autoplay = v);
            let mut fade = state::get().settings.read().unwrap().crossfade as f32;
            let hint = if fade < 0.5 { "Выключен: следующий трек готовится заранее и начинается сразу, без паузы".to_string() } else { format!("Следующий трек вступает за {fade:.0} с до конца текущего") };
            row(ui, "Кроссфейд", &hint, |ui| {
                ui.label(egui::RichText::new(if fade < 0.5 { crate::i18n::t("выкл.") } else { format!("{fade:.0} с") }).color(theme::ink2()));
                let mut f = fade / 12.0;
                if widgets::level(ui, 160.0, &mut f).changed() {
                    fade = (f * 12.0).round();
                    state::get().set_settings(|s| s.crossfade = fade as f64);
                }
            });
            switch(ui, "Продолжать с того же места", "При запуске вернуть очередь и позицию — останется нажать «Играть»", |s| s.resume_on_start, |s, v| s.resume_on_start = v);
            switch(ui, "Запуск двойным щелчком", "Одиночный щелчок только выделяет трек — удобно, если часто промахиваетесь", |s| s.double_click_to_play, |s, v| s.double_click_to_play = v);
        });
        card(ui, "Устройства", |ui| {
            switch(ui, "Пауза при отключении наушников", "Если устройство вывода пропадёт, музыка остановится, а не заиграет из колонок", |s| s.pause_on_device_change, |s, v| s.pause_on_device_change = v);
        });
    }

    fn settings_sound(&mut self, ui: &mut Ui) {
        widgets::glass_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            self.equalizer_ui(ui);
        });
        ui.add_space(10.0);
    }

    fn settings_services(&mut self, ui: &mut Ui) {
        card(ui, "SoundCloud", |ui| {
            let mut id = state::get().settings.read().unwrap().soundcloud_client_id.clone();
            row(ui, "Client ID своего приложения", "Пусто — берётся автоматически с сайта SoundCloud", |ui| {
                if ui.add(egui::TextEdit::singleline(&mut id).desired_width(220.0)).changed() {
                    state::get().set_settings(|s| s.soundcloud_client_id = id.trim().to_string());
                }
            });
        });
        card(ui, "Защита региона", |ui| {
            switch(ui, "Не обращаться к SoundCloud из выбранных стран", "Песни тогда играют из копии на YouTube Music", |s| s.region_guard, |s, v| s.region_guard = v);
            let countries = state::get().settings.read().unwrap().region_countries.join(", ");
            let mut text = countries.clone();
            row(ui, "Страны", "Двухбуквенные коды через запятую", |ui| {
                if ui.add(egui::TextEdit::singleline(&mut text).desired_width(160.0)).lost_focus() && text != countries {
                    let list: Vec<String> = text.split(',').filter_map(crate::services::region::normalize_country).collect();
                    state::get().set_settings(|s| s.region_countries = list);
                }
            });
            let known = crate::services::region::known_country();
            App::muted(ui, &format!("{}: {}", crate::i18n::t("Страна подключения"), known.unwrap_or_else(|| "—".into())));
        });
        card(ui, "Прокси", |ui| {
            let saved = crate::userproxy::saved(&self.data_dir);
            let mut text = saved.clone();
            row(ui, "Свой HTTP-прокси", "host:port — для всех запросов программы, с перезапуска", |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(200.0).hint_text("127.0.0.1:2080"));
                if r.lost_focus() && text != saved {
                    if let Err(e) = crate::userproxy::save(&self.data_dir, &text) {
                        self.toasts.push(crate::app::Toast { text: e, until: std::time::Instant::now() + std::time::Duration::from_secs(3) });
                    }
                }
            });
        });
    }

    fn settings_look(&mut self, ui: &mut Ui) {
        card(ui, "Язык", |ui| {
            let lang = state::get().settings.read().unwrap().language;
            let sel = match lang {
                Language::Auto => 0,
                Language::Ru => 1,
                Language::En => 2,
            };
            if let Some(i) = widgets::tabs(ui, &["Как в системе", "Русский", "English"], sel) {
                state::get().set_settings(|s| s.language = [Language::Auto, Language::Ru, Language::En][i]);
                crate::app::apply_language(&state::get().settings());
            }
        });
        card(ui, "Текст песни", |ui| {
            let size = state::get().settings.read().unwrap().lyrics_size;
            let sel = match size {
                LyricsSize::S => 0,
                LyricsSize::M => 1,
                LyricsSize::L => 2,
            };
            row(ui, "Размер текста", "", |ui| {
                if let Some(i) = widgets::tabs(ui, &["Мелкий", "Средний", "Крупный"], sel) {
                    state::get().set_settings(|s| s.lyrics_size = [LyricsSize::S, LyricsSize::M, LyricsSize::L][i]);
                }
            });
        });
        card(ui, "Экономия памяти", |ui| {
            switch(ui, "Режим экономии", "Скрытое окно отпускает страницы и обложки через 2 секунды вместо 5", |s| s.memory_saver, |s, v| s.memory_saver = v);
            switch(ui, "Не хранить кэш обложек", "Обложки каждый раз загружаются из сети (со следующего запуска)", |s| s.no_cache, |s, v| s.no_cache = v);
            switch(ui, "Не вести историю", "«Недавно слушали» перестанет пополняться", |s| s.no_history, |s, v| s.no_history = v);
        });
    }

    fn settings_keys(&mut self, ui: &mut Ui) {
        card(ui, "Глобальные клавиши", |ui| {
            let before = state::get().settings.read().unwrap().global_shortcuts;
            switch(ui, "Работают из любой программы", "Плей/пауза, следующий, предыдущий и другие — даже когда окно скрыто", |s| s.global_shortcuts, |s, v| s.global_shortcuts = v);
            if state::get().settings.read().unwrap().global_shortcuts != before {
                self.keys.apply_global(&state::get().settings());
            }
        });
        card(ui, "Клавиши", |ui| {
            let s = state::get().settings();
            for def in crate::app::keys::SHORTCUTS {
                let acc = crate::app::keys::binding(&s.shortcuts, def);
                let scope = match def.scope {
                    crate::app::keys::Scope::App => "в окне",
                    crate::app::keys::Scope::Global => "везде",
                    crate::app::keys::Scope::Overlay => "в мини-плеере",
                };
                row(ui, def.title, &format!("{} · {}", crate::i18n::t(scope), crate::i18n::t(def.hint)), |ui| {
                    let text = acc.map(crate::app::keys::pretty).unwrap_or_else(|| crate::i18n::t("нет"));
                    egui::Frame::new().fill(theme::hover()).corner_radius(8).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
                        ui.label(egui::RichText::new(text).font(theme::medium(12.5)).color(INK));
                    });
                });
            }
        });
    }

    /// Мини-плеер (src/pages/settings/OverlayTab.tsx)
    fn settings_overlay(&mut self, ui: &mut Ui) {
        let s = state::get().settings();
        let shown = crate::ui::mini::is_shown();
        card(ui, if shown { "Мини-плеер открыт" } else { "Мини-плеер спрятан" }, |ui| {
            App::muted(ui, "Маленькое окно поверх всех программ: обложка, название, строка текста и кнопки. Перетаскивайте его за название трека.");
            ui.add_space(6.0);
            ui.add_enabled_ui(s.overlay_enabled, |ui| {
                if widgets::pill_button(ui, Some(ic::MINI), if shown { "Спрятать" } else { "Показать сейчас" }, true).clicked() {
                    crate::events::send(crate::events::Event::Action(crate::app::actions::Action::ToggleOverlay));
                }
            });
        });
        card(ui, "Поведение", |ui| {
            switch(ui, "Мини-плеер включён", "Если выключить, его нельзя будет открыть ни клавишей, ни из трея", |s| s.overlay_enabled, |s, v| s.overlay_enabled = v);
            ui.add_enabled_ui(s.overlay_enabled, |ui| {
                switch(ui, "Поверх всех окон", "Не прячется за браузером и играми в окне", |s| s.overlay_on_top, |s, v| s.overlay_on_top = v);
                switch(ui, "Показывать при сворачивании", "Свернули EvPlayer — появился мини-плеер; развернули — исчез", |s| s.overlay_on_minimize, |s, v| s.overlay_on_minimize = v);
                switch(ui, "Строка текста песни", "Вместо исполнителя показывать строку, которую сейчас поют", |s| s.overlay_show_lyrics, |s, v| s.overlay_show_lyrics = v);
            });
        });
        card(ui, "Где появляется", |ui| {
            App::muted(ui, "Если вы перетащили мини-плеер, он остаётся там, пока не выберете угол заново");
            ui.add_space(8.0);
            use crate::store::state::OverlayCorner::*;
            let corners = [(TopLeft, "Слева сверху"), (TopCenter, "По центру сверху"), (TopRight, "Справа сверху"), (BottomLeft, "Слева снизу"), (BottomCenter, "По центру снизу"), (BottomRight, "Справа снизу")];
            // a little screen with the six places on it
            let (screen, _) = ui.allocate_exact_size(vec2(200.0, 124.0), egui::Sense::hover());
            let p = ui.painter();
            p.rect(screen, 12, theme::hover(), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
            for (i, (c, label)) in corners.iter().enumerate() {
                let col = (i % 3) as f32;
                let top = i < 3;
                let size = vec2(52.0, 16.0);
                let x = screen.left() + 12.0 + col * (screen.width() - 24.0 - size.x) / 2.0;
                let y = if top { screen.top() + 12.0 } else { screen.bottom() - 12.0 - size.y };
                let rect = egui::Rect::from_min_size(egui::pos2(x, y), size);
                let r = ui.interact(rect, ui.scope_id().with(("corner", i)), egui::Sense::click());
                let on = s.overlay_corner == *c;
                let fill = if on { ACCENT } else if r.hovered() { theme::ink3() } else { egui::Color32::from_rgba_unmultiplied(238, 247, 244, 30) };
                ui.painter().rect_filled(rect, 5, fill);
                if r.clicked() {
                    let c = *c;
                    state::get().set_settings(|s| s.overlay_corner = c);
                }
                r.on_hover_text(crate::i18n::t(label));
            }
        });
        card(ui, "Клавиши", |ui| {
            let key = |id: &str| crate::app::keys::SHORTCUTS.iter().find(|d| d.id == id).and_then(|d| crate::app::keys::binding(&s.shortcuts, d)).map(crate::app::keys::pretty).unwrap_or_else(|| crate::i18n::t("не задано"));
            row(ui, "В окне EvPlayer", &key("app.overlay"), |_| {});
            row(ui, "Из любой программы", &key("global.overlay"), |_| {});
            let inside = format!(
                "{} — {}, {} / {} — {}, {} — {}, {} — {}",
                key("overlay.playPause"),
                crate::i18n::t("пауза"),
                key("overlay.prev"),
                key("overlay.next"),
                crate::i18n::t("треки"),
                key("overlay.like"),
                crate::i18n::t("нравится"),
                key("overlay.hide"),
                crate::i18n::t("спрятать")
            );
            row(ui, "Внутри мини-плеера", &inside, |_| {});
            App::muted(ui, "Ещё мини-плеер открывается из меню значка в трее.");
        });
    }

    fn settings_window(&mut self, ui: &mut Ui) {
        card(ui, "Окно", |ui| {
            switch(ui, "Закрывать в трей", "Крестик прячет окно, музыка продолжает играть", |s| s.close_to_tray, |s, v| s.close_to_tray = v);
            switch(ui, "Сворачивать в трей", "Свёрнутое окно убирается с панели задач", |s| s.minimize_to_tray, |s, v| s.minimize_to_tray = v);
        });
    }

    fn settings_discord(&mut self, ui: &mut Ui) {
        card(ui, "Статус в Discord", |ui| {
            switch(ui, "Показывать, что играет", "Discord должен быть запущен на этом компьютере", |s| s.discord_enabled, |s, v| s.discord_enabled = v);
            switch(ui, "Обложка", "", |s| s.discord_show_artwork, |s, v| s.discord_show_artwork = v);
            switch(ui, "Шкала времени", "", |s| s.discord_show_timeline, |s, v| s.discord_show_timeline = v);
            switch(ui, "Показывать на паузе", "", |s| s.discord_show_paused, |s, v| s.discord_show_paused = v);
            switch(ui, "Кнопка со ссылкой на песню", "", |s| s.discord_show_button, |s, v| s.discord_show_button = v);
            let st = crate::presence::status();
            App::muted(ui, &st);
        });
    }

    /// Данные и копия (DataTab in src/pages/SettingsPage.tsx)
    fn settings_data(&mut self, ui: &mut Ui) {
        App::muted(ui, "Всё хранится только на этом компьютере. Сделайте копию, чтобы перенести библиотеку.");
        ui.add_space(8.0);
        // what takes how much, looked at again every few seconds (not every frame: the cache is a folder walk)
        let stale = self.pages.data_usage.as_ref().is_none_or(|(at, _)| at.elapsed() > std::time::Duration::from_secs(5));
        if stale {
            let kv = &state::get().kv;
            let library = kv.size(crate::state::LIBRARY) as u64;
            let stats = (kv.size(crate::state::HISTORY) + crate::stats::bytes()) as u64;
            let cache = dir_size(&crate::app::cache_dir());
            self.pages.data_usage = Some((std::time::Instant::now(), [library, stats, cache]));
        }
        let [library, stats, cache] = self.pages.data_usage.as_ref().map(|u| u.1).unwrap_or_default();
        ui.horizontal(|ui| {
            for (label, bytes) in [("Библиотека", library), ("Статистика и история", stats), ("Кэш обложек и ответов", cache)] {
                widgets::glass_frame().show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(180.0);
                        ui.label(egui::RichText::new(crate::i18n::t(label)).font(theme::body(12.0)).color(theme::ink2()));
                        ui.label(egui::RichText::new(size_text(bytes)).font(theme::heavy(20.0)).color(INK));
                    });
                });
            }
        });
        ui.add_space(10.0);
        card(ui, "Резервная копия", |ui| {
            App::muted(ui, "Лайки, плейлисты, статистика, история, пресеты и настройки одним файлом");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::pill_button(ui, Some(ic::DOWNLOAD), "Сохранить копию", true).clicked() {
                    state::get().kv.flush();
                    let b = crate::backup::make(&state::get().kv);
                    if let Some(path) = crate::dialogs::save_file(&crate::backup::file_name(&b), "Резервная копия EvPlayer", "json") {
                        match serde_json::to_string(&b).map_err(|e| e.to_string()).and_then(|t| std::fs::write(&path, t).map_err(|e| e.to_string())) {
                            Ok(()) => self.toast("Копия сохранена"),
                            Err(e) => self.toast(&format!("Не удалось сохранить: {e}")),
                        }
                    }
                }
                if widgets::pill_button(ui, Some(ic::OPEN), "Восстановить из копии", false).clicked() {
                    if let Some(path) = crate::dialogs::open_file("Резервная копия EvPlayer", "json") {
                        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| crate::backup::read(&t)) {
                            Ok(b) => self.dialog = Some(crate::app::Dialog::Restore(b)),
                            Err(e) => self.toast(&e),
                        }
                    }
                }
            });
        });
        self.offline_card(ui);
        card(ui, "Очистка", |ui| {
            row(ui, "Кэш", "Обложки и ответы сервисов — скачаются заново, когда понадобятся", |ui| {
                if widgets::pill_button(ui, None, "Очистить", false).clicked() {
                    let _ = std::fs::remove_dir_all(crate::app::cache_dir().join("covers"));
                    self.images.release_all();
                    self.pages.data_usage = None;
                    self.toast("Кэш очищен");
                }
            });
            row(ui, "История прослушиваний", "«Недавно слушали» на главной и недавние запросы в поиске", |ui| {
                if widgets::pill_button(ui, None, "Очистить", false).clicked() {
                    state::get().update_history(|h| *h = Default::default());
                    super::search::clear_history();
                    self.pages.data_usage = None;
                    self.toast("История очищена");
                }
            });
            row(ui, "Статистика", "Топы, графики и умные плейлисты строятся из неё", |ui| {
                if widgets::pill_button(ui, None, "Сбросить", false).clicked() {
                    self.dialog = Some(crate::app::Dialog::ResetStats);
                }
            });
            row(ui, "Папка с данными", "Список ваших файлов и обложки к ним", |ui| {
                if widgets::pill_button(ui, None, "Открыть", false).clicked() {
                    crate::dialogs::open_path(&self.data_dir);
                }
            });
        });
    }

    /// "Скачанное для офлайн" (OfflineCard)
    fn offline_card(&mut self, ui: &mut Ui) {
        let saved = self.downloads.saved();
        let count = saved.len();
        let bytes: u64 = saved.iter().map(|e| e.size).sum();
        let lists = crate::keep::get().lists;
        card(ui, "Скачанное для офлайн", |ui| {
            App::muted(ui, "Песни из SoundCloud и YouTube, сохранённые на этом компьютере: играют без интернета");
            ui.add_space(4.0);
            let (title, hint) = if count > 0 {
                (crate::i18n::tracks(count), format!("{} {}", size_text(bytes), crate::i18n::t("на диске")))
            } else {
                (crate::i18n::t("Пока ничего не скачано"), crate::i18n::t("Скачать можно трек, плейлист или лайки: правый клик по треку или кнопка на странице списка"))
            };
            row(ui, &title, &hint, |ui| {
                if widgets::pill_button(ui, Some(ic::DOWNLOAD), "Скачать…", true).clicked() {
                    self.dialog = Some(crate::app::Dialog::Download { picked: vec![], follow: true });
                }
            });
            if !lists.is_empty() {
                let names: Vec<String> = lists.iter().map(|l| crate::keep::title_of(l)).collect();
                row(ui, "Докачиваются сами", &names.join(", "), |ui| {
                    if widgets::pill_button(ui, None, "Перестать", false).clicked() {
                        crate::keep::remove(&lists);
                    }
                });
            }
            row(ui, "Папка со скачанным", "Файлы лежат там под служебными именами", |ui| {
                if widgets::pill_button(ui, None, "Открыть", false).clicked() {
                    let dir = self.downloads.folder();
                    let _ = std::fs::create_dir_all(&dir);
                    crate::dialogs::open_path(&dir);
                }
            });
            if count > 0 {
                row(ui, "Сохранить в папку", "Копии как обычные файлы «Исполнитель - Название» с тегами и обложкой: для другого плеера или телефона. Скачанное остаётся как было", |ui| {
                    if widgets::pill_button(ui, None, "Выбрать папку…", false).clicked() {
                        if let Some(folder) = crate::dialogs::pick_folder_for("Куда сохранить копии") {
                            let tracks = self.saved_tracks();
                            let d = self.downloads.clone();
                            self.toast("Сохраняю копии…");
                            std::thread::spawn(move || match d.export(&tracks, &folder) {
                                Ok(r) => {
                                    let mut line = format!("{}: {} → {}", crate::i18n::t("Сохранено"), crate::i18n::tracks(r.copied), r.folder);
                                    if r.missing > 0 {
                                        line.push_str(&format!(" ({} {})", r.missing, crate::i18n::t("не нашлось")));
                                    }
                                    crate::app::toast_later(line);
                                }
                                Err(e) => crate::app::toast_later(e),
                            });
                        }
                    }
                });
                row(ui, "Удалить всё скачанное", "Песни останутся в плейлистах и будут играть из интернета", |ui| {
                    if widgets::pill_button(ui, Some(ic::DELETE), "Удалить", false).clicked() {
                        crate::keep::remove(&lists);
                        self.downloads.clear();
                        self.toast("Скачанное удалено");
                    }
                });
            }
        });
    }

    /// О программе (About in src/pages/SettingsPage.tsx)
    fn settings_about(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Some(logo) = &self.logo {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(logo.id(), vec2(64.0, 64.0))).corner_radius(16));
            }
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("EvPlayer").font(theme::heavy(26.0)).color(INK));
                let line = format!("{} {} · {}", crate::i18n::t("Версия"), env!("CARGO_PKG_VERSION"), crate::i18n::t("открытый проект, лицензия MIT"));
                let r = ui.add(egui::Label::new(egui::RichText::new(line).color(theme::ink2())).sense(egui::Sense::click()));
                // seven clicks on the version within three seconds turn the developer's tools on or off
                if r.clicked() {
                    let now = std::time::Instant::now();
                    self.pages.version_clicks.retain(|t| now.duration_since(*t) < std::time::Duration::from_secs(3));
                    self.pages.version_clicks.push(now);
                    if self.pages.version_clicks.len() >= 7 {
                        self.pages.version_clicks.clear();
                        let on = !state::get().settings.read().unwrap().dev_tools;
                        state::get().set_settings(|s| s.dev_tools = on);
                        self.toast(if on { "Диагностика включена: Настройки → Диагностика" } else { "Диагностика выключена" });
                    }
                }
            });
        });
        ui.add_space(12.0);
        card(ui, "Поддержать автора", |ui| {
            row(ui, "", "EvPlayer бесплатный и без рекламы. Если он вам нравится, можно поддержать разработку через DonationAlerts.", |ui| {
                if widgets::pill_button(ui, Some(ic::HEART_FILL), "Поддержать", true).clicked() {
                    crate::dialogs::open_path(std::path::Path::new("https://www.donationalerts.com/r/evire"));
                }
            });
        });
        card(ui, "Проект", |ui| {
            self.updates_row(ui);
            row(ui, "Исходный код", "github.com/Evire1337/evplayer", |ui| {
                if widgets::pill_button(ui, None, "Открыть", false).clicked() {
                    crate::dialogs::open_path(std::path::Path::new("https://github.com/Evire1337/evplayer"));
                }
            });
            row(ui, "Отрисовка", &crate::i18n::t("Без браузерного движка: окно рисует видеокарта, звук — свой."), |ui| {
                ui.label(egui::RichText::new(crate::app::renderer_name()).font(theme::medium(13.0)).color(INK));
            });
        });
        App::muted(ui, "Музыка — SoundCloud и YouTube, тексты — LRCLIB и Genius. EvPlayer не связан с этими сервисами.");
    }

    /// "Обновления" (filled in by the updater)
    fn updates_row(&mut self, ui: &mut Ui) {
        let st = crate::updates::status();
        row(ui, "Обновления", &crate::updates::line(&st), |ui| {
            if let crate::updates::Status::Ready { .. } = st {
                if widgets::pill_button(ui, None, "Перезапустить", true).clicked() {
                    if let Err(e) = crate::updates::install() {
                        self.toast(&e);
                    }
                }
            } else {
                let busy = matches!(st, crate::updates::Status::Checking | crate::updates::Status::Downloading { .. });
                if ui.add_enabled_ui(!busy, |ui| widgets::pill_button(ui, None, "Проверить", false)).inner.clicked() {
                    crate::updates::check_now();
                }
            }
        });
    }

    /// Диагностика (src/pages/settings/DiagnosticsTab.tsx): what the program takes right now
    /// and what to do about it. Measures only while this tab is open.
    fn settings_diagnostics(&mut self, ui: &mut Ui) {
        App::muted(ui, "Сколько памяти и процессора берёт приложение, что из этого стоит дороже всего и что с этим сделать. Замеры идут только пока открыт этот раздел.");
        ui.add_space(8.0);
        let probe = self.pages.probe.get_or_insert_with(crate::diagnostics::Probe::new);
        probe.tick(std::time::Duration::from_secs(2));
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
        let sample = probe.last.clone().unwrap_or_default();
        let settings = state::get().settings();
        let fx = state::get().effects.read().unwrap().clone();
        let (eq_on, normalize) = {
            let eq = state::get().eq.read().unwrap();
            (eq.enabled, eq.normalize)
        };
        let sound_processing = eq_on || normalize || fx.reverb || (fx.speed - 1.0).abs() > 0.001 || fx.pitch.abs() > 0.001 || settings.crossfade > 0.0;
        let mut stored: Vec<(String, usize)> = state::get().kv.snapshot().into_iter().map(|(k, v)| (k, v.len())).collect();
        stored.sort_by(|a, b| b.1.cmp(&a.1));
        let advice = crate::diagnostics::advise(&sample, &crate::diagnostics::Knobs { memory_saver: settings.memory_saver, overlay_enabled: settings.overlay_enabled, sound_processing }, &stored);
        let lib = state::get().library.read().unwrap();
        let counts = (lib.likes.len(), lib.playlists.len(), lib.playlists.iter().map(|p| p.items.len()).sum::<usize>());
        drop(lib);
        let history = state::get().history.lock().unwrap().entries.len();
        let queue = self.view.queue.len();
        let offline = self.downloads.saved().len();
        let report = {
            let mut lines = vec![
                format!("EvPlayer (нативная версия {}) — диагностика, {}", env!("CARGO_PKG_VERSION"), chrono::Local::now().format("%d.%m.%Y %H:%M:%S")),
                format!("Память: своя {:.1} МБ, в оперативной {:.1} МБ; потоков {}", sample.private_mb, sample.working_mb, sample.threads),
                format!("Процессор: {:.1}%; кадров в секунду {:.1}; звук {:.2}% ядра; отрисовка: {}", sample.cpu, sample.fps, sample.sound, crate::app::renderer_name()),
                format!("Обложек в памяти: {}; звуковая карта: {}", self.images.count(), self.player.device()),
                format!("Данные: лайков {}, плейлистов {} ({} треков), история {}, очередь {}, офлайн {}", counts.0, counts.1, counts.2, history, queue, offline),
                format!("Хранилище: {}", stored.iter().take(8).map(|(k, b)| format!("{k} {}", crate::diagnostics::mb(*b as f64))).collect::<Vec<_>>().join(", ")),
            ];
            lines.push(format!("Советы: {}", if advice.is_empty() { "нет".to_string() } else { advice.iter().map(|a| a.title).collect::<Vec<_>>().join(", ") }));
            lines.join("\n")
        };
        card(ui, "Сейчас", |ui| {
            if probe_ready(&sample) {
                row(ui, "Память", "Своя память программы, как «Память» в диспетчере задач", |ui| {
                    ui.label(egui::RichText::new(format!("{:.0} МБ", sample.private_mb)).font(theme::medium(13.5)).color(INK));
                });
                row(ui, "В оперативной памяти", &format!("{} {}", crate::i18n::t("С общими библиотеками Windows; потоков:"), sample.threads), |ui| {
                    ui.label(egui::RichText::new(format!("{:.0} МБ", sample.working_mb)).font(theme::medium(13.5)).color(INK));
                });
                row(ui, "Процессор", "Доля всего процессора, как в диспетчере задач", |ui| {
                    ui.label(egui::RichText::new(format!("{:.1}%", sample.cpu)).font(theme::medium(13.5)).color(INK));
                });
                row(ui, "Отрисовка", &format!("{}: {}", crate::i18n::t("Окно рисуется только когда что-то меняется"), crate::app::renderer_name()), |ui| {
                    ui.label(egui::RichText::new(format!("{:.0} {}", sample.fps, crate::i18n::t("кадр/с"))).font(theme::medium(13.5)).color(INK));
                });
                let dev = self.player.device();
                row(ui, "Обработка звука", &format!("{} · {}", if sound_processing { crate::i18n::t("Эквалайзер, эффекты, кроссфейд: включена") } else { crate::i18n::t("выключена") }, if dev.is_empty() { crate::i18n::t("звуковая карта не открыта") } else { dev }), |ui| {
                    ui.label(egui::RichText::new(format!("{:.2}% {}", sample.sound, crate::i18n::t("ядра"))).font(theme::medium(13.5)).color(INK));
                });
                row(ui, "Обложек в памяти", "Картинки, которые сейчас на видеокарте", |ui| {
                    ui.label(egui::RichText::new(self.images.count().to_string()).font(theme::medium(13.5)).color(INK));
                });
                if let Some(t) = self.first_frame {
                    row(ui, "Первый кадр", "С запуска программы до первого нарисованного окна", |ui| {
                        ui.label(egui::RichText::new(format!("{} мс", t.as_millis())).font(theme::medium(13.5)).color(INK));
                    });
                }
                ui.add_space(4.0);
                if widgets::pill_button(ui, None, "Скопировать отчёт", false).clicked() {
                    ui.ctx().copy_text(report.clone());
                    self.toast("Отчёт скопирован: его можно прислать разработчику");
                }
            } else {
                App::muted(ui, "Замеряю…");
            }
        });
        card(ui, "Советы", |ui| {
            if advice.is_empty() {
                App::muted(ui, "Всё в порядке: ничего лишнего не тратится.");
            }
            for a in &advice {
                let tag = match a.level {
                    0 => "важно",
                    1 => "стоит",
                    _ => "к сведению",
                };
                row(ui, a.title, &format!("{} · {}", crate::i18n::t(tag), a.detail), |ui| {
                    match a.id {
                        "memory" => {
                            if widgets::pill_button(ui, None, "Включить экономию памяти", true).clicked() {
                                state::get().set_settings(|s| s.memory_saver = true);
                            }
                        }
                        "overlay" => {
                            if widgets::pill_button(ui, None, "Выключить мини-плеер", false).clicked() {
                                state::get().set_settings(|s| s.overlay_enabled = false);
                            }
                        }
                        _ => {}
                    }
                });
            }
        });
        card(ui, "Данные", |ui| {
            row(ui, "Библиотека", &format!("{} {}, {} {}", counts.1, crate::i18n::t("плейлистов"), crate::i18n::tracks(counts.2), crate::i18n::t("в них")), |ui| {
                ui.label(egui::RichText::new(format!("{} {}", counts.0, crate::i18n::t("лайков"))).font(theme::medium(13.5)).color(INK));
            });
            row(ui, "История и очередь", &format!("{} {offline}", crate::i18n::t("Офлайн:")), |ui| {
                ui.label(egui::RichText::new(format!("{history} / {queue}")).font(theme::medium(13.5)).color(INK));
            });
            for (k, b) in stored.iter().take(5) {
                row(ui, k, "Хранится на этом устройстве", |ui| {
                    ui.label(egui::RichText::new(crate::diagnostics::mb(*b as f64)).font(theme::medium(13.5)).color(INK));
                });
            }
        });
    }
}

/// "12 КБ", "3.4 МБ"
fn size_text(bytes: u64) -> String {
    let kb = bytes as f64 / 1024.0;
    if kb > 1024.0 {
        format!("{:.1} МБ", kb / 1024.0)
    } else {
        format!("{} КБ", kb.round().max(1.0) as u64)
    }
}

fn dir_size(p: &std::path::Path) -> u64 {
    std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| match e.file_type() {
        Ok(t) if t.is_dir() => dir_size(&e.path()),
        _ => e.metadata().map(|m| m.len()).unwrap_or(0),
    }).sum()).unwrap_or(0)
}

/// the first reading is in
fn probe_ready(s: &crate::diagnostics::Sample) -> bool {
    s.private_mb > 0.0 || s.working_mb > 0.0
}
