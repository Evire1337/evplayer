//! Настройки, with the page's sections (src/pages/SettingsPage.tsx and settings/*): the
//! playback, the sound, the services, the look, the keys, the window, Discord, the data, and
//! "О программе" with what the program takes from the computer right now.

use egui::{vec2, Ui};

use crate::app::App;
use crate::state;
use crate::store::state::{Language, LyricsSize, Settings};
use crate::ui::theme::{self, ic, ACCENT, INK};
use crate::ui::widgets;

const TABS: [(&str, &str, &str); 9] = [
    ("Музыка", ic::PLAY, "Воспроизведение"),
    ("", ic::EQ, "Звук и эквалайзер"),
    ("", ic::SYNC, "Сервисы и импорт"),
    ("Приложение", ic::SPARK, "Внешний вид"),
    ("", ic::KEYBOARD, "Клавиши"),
    ("", ic::MINI, "Окно и трей"),
    ("", ic::DISCORD, "Discord"),
    ("Другое", ic::FOLDER, "Данные и резервная копия"),
    ("", ic::INFO, "О программе"),
];

/// a setting: its name and hint on the left, its control on the right
fn row(ui: &mut Ui, title: &str, hint: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(42.0);
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() - 200.0);
            ui.label(egui::RichText::new(crate::i18n::t(title)).font(theme::medium(13.5)).color(INK));
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
        for (i, (group, icon, name)) in TABS.iter().enumerate() {
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
                5 => self.settings_window(ui),
                6 => self.settings_discord(ui),
                7 => self.settings_data(ui),
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
        card(ui, "Эквалайзер и эффекты", |ui| {
            App::muted(ui, "Эквалайзер, выравнивание громкости, скорость, тон и реверберация — в панели «Звук и эффекты» (кнопка с ползунками внизу).");
            ui.add_space(6.0);
            if widgets::pill_button(ui, Some(ic::EQ), "Открыть панель звука", false).clicked() {
                self.pages.sound_open = true;
            }
        });
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

    fn settings_data(&mut self, ui: &mut Ui) {
        card(ui, "Данные", |ui| {
            row(ui, "Папка с данными", "Библиотека, плейлисты, настройки, скачанное", |ui| {
                if widgets::pill_button(ui, Some(ic::OPEN), "Открыть", false).clicked() {
                    crate::dialogs::open_path(&self.data_dir);
                }
            });
            let size = dir_size(&crate::app::cache_dir());
            row(ui, "Кэш обложек", &format!("{:.1} МБ", size as f64 / 1_048_576.0), |ui| {
                if widgets::pill_button(ui, Some(ic::DELETE), "Очистить", false).clicked() {
                    let _ = std::fs::remove_dir_all(crate::app::cache_dir().join("covers"));
                    self.images.release_all();
                }
            });
        });
    }

    fn settings_about(&mut self, ui: &mut Ui) {
        card(ui, "EvPlayer", |ui| {
            App::muted(ui, &format!("{} {} · {}", crate::i18n::t("Версия"), env!("CARGO_PKG_VERSION"), crate::i18n::t("Нативная версия")));
            ui.add_space(4.0);
            ui.label(egui::RichText::new(crate::i18n::t("Музыкальный плеер для SoundCloud, YouTube Music и ваших файлов. Без браузерного движка: окно рисует видеокарта, звук — свой.")).color(theme::ink2()));
        });
        card(ui, "Диагностика", |ui| {
            let mem = crate::diagnostics::memory();
            let line = |ui: &mut Ui, k: &str, v: String| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t(k)).color(theme::ink2()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(v).font(theme::medium(13.0)).color(INK));
                    });
                });
            };
            line(ui, "Отрисовка", crate::app::renderer_name().into());
            line(ui, "Память программы", format!("{:.1} МБ ({} {:.1} МБ)", mem.private_mb, crate::i18n::t("рабочий набор"), mem.working_mb));
            line(ui, "Обложек в памяти", self.images.count().to_string());
            let dev = self.player.device();
            line(ui, "Звуковая карта", if dev.is_empty() { crate::i18n::t("не открыта (тишина)") } else { dev });
            if let Some(t) = self.first_frame {
                line(ui, "Первый кадр", format!("{} мс", t.as_millis()));
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new(crate::i18n::t("Обновляется раз в секунду, пока открыта эта вкладка.")).font(theme::body(11.5)).color(theme::ink3()));
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        });
        let _ = ACCENT;
    }
}

fn dir_size(p: &std::path::Path) -> u64 {
    std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| match e.file_type() {
        Ok(t) if t.is_dir() => dir_size(&e.path()),
        _ => e.metadata().map(|m| m.len()).unwrap_or(0),
    }).sum()).unwrap_or(0)
}
