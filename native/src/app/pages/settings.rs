//! Настройки, tab for tab as src/pages/SettingsPage.tsx and settings/* have them: the same
//! groups and tabs, the same cards, rows, hints and controls (kit.rs draws them the way the
//! page's CSS does).

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Stroke, Ui};

use super::kit::{self, Tone};
use crate::app::App;
use crate::state;
use crate::store::state::{Language, LyricsSize, OverlayCorner, SearchSource, StatusShows};
use crate::ui::theme::{self, ic, GROUND, INK};
use crate::ui::widgets;

/// the page's own width (.page max-width 1240, padding 8 each side)
const PAGE_W: f32 = 1224.0;
/// .settings-layout: the tabs' column and the gap after it
const NAV_W: f32 = 240.0;
const NAV_GAP: f32 = 22.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Playback,
    Sound,
    Services,
    Appearance,
    Keys,
    Overlay,
    Discord,
    Data,
    Diagnostics,
    About,
}

const TABS: [(Tab, &str, &str, &str); 10] = [
    (Tab::Playback, ic::PLAY_CIRCLE, "Воспроизведение", "Музыка"),
    (Tab::Sound, ic::EQ, "Звук и эквалайзер", "Музыка"),
    (Tab::Services, ic::PLUG, "Сервисы и импорт", "Музыка"),
    (Tab::Appearance, "@palette", "Внешний вид", "Приложение"),
    (Tab::Keys, ic::KEYBOARD, "Клавиши", "Приложение"),
    (Tab::Overlay, ic::MINI, "Мини-плеер", "Приложение"),
    (Tab::Discord, ic::DISCORD, "Discord", "Приложение"),
    (Tab::Data, ic::DATABASE, "Данные и копия", "Прочее"),
    (Tab::Diagnostics, ic::CHART, "Диагностика", "Прочее"),
    (Tab::About, ic::INFO, "О программе", "Прочее"),
];

/// the place of a tab by its old number ("settings:N" in the snapshot tool and links)
fn tab_at(i: usize) -> Tab {
    TABS.get(i).map(|t| t.0).unwrap_or(Tab::Playback)
}

fn set_settings(f: impl FnOnce(&mut crate::store::state::Settings)) {
    state::get().set_settings(f);
}

impl App {
    pub fn settings(&mut self, ui: &mut Ui) {
        self.page_title(ui, "Настройки");
        let avail = ui.available_rect_before_wrap();
        let width = avail.width().min(PAGE_W);
        let dev_tools = state::get().settings.read().unwrap().dev_tools;
        let mut tab = tab_at(self.pages.settings_tab);
        if tab == Tab::Diagnostics && !dev_tools {
            tab = Tab::Playback;
            self.pages.settings_tab = 0;
        }
        // .settings-tabs: glass, rounded 24, padding 12; groups 14 apart
        let nav_rect = Rect::from_min_size(avail.min, vec2(NAV_W, avail.height()));
        let mut nav = ui.new_child(egui::UiBuilder::new().max_rect(nav_rect.shrink(12.0)).layout(egui::Layout::top_down(egui::Align::Min)));
        let bg = nav.painter().add(egui::Shape::Noop);
        nav.spacing_mut().item_spacing.y = 2.0;
        let mut last_group = "";
        for (i, (id, icon, label, group)) in TABS.iter().enumerate() {
            if *id == Tab::Diagnostics && !dev_tools {
                continue;
            }
            if *group != last_group {
                if !last_group.is_empty() {
                    nav.add_space(12.0);
                }
                last_group = group;
                let job = egui::text::LayoutJob::single_section(crate::i18n::t(group).to_uppercase(), egui::TextFormat { font_id: theme::heavy(11.0), color: theme::ink3(), extra_letter_spacing: 0.88, ..Default::default() });
                let g = nav.painter().layout_job(job);
                let (r, _) = nav.allocate_exact_size(vec2(NAV_W - 24.0, g.size().y + 8.0), egui::Sense::hover());
                nav.painter().galley(pos2(r.left() + 12.0, r.top() + 2.0), g, theme::ink3());
            }
            let (rect, r) = nav.allocate_exact_size(vec2(NAV_W - 24.0, 38.0), egui::Sense::click());
            let on = tab == *id;
            let (fill, fg) = if on {
                (INK, GROUND)
            } else if r.hovered() {
                (theme::hover(), INK)
            } else {
                (Color32::TRANSPARENT, theme::ink2())
            };
            nav.painter().rect_filled(rect, 13, fill);
            let icon_color = if on { fg } else { fg.gamma_multiply(0.8) };
            crate::ui::icons::paint(nav.painter(), icon.trim_start_matches('@'), Rect::from_center_size(pos2(rect.left() + 12.0 + 8.5, rect.center().y), vec2(17.0, 17.0)), icon_color);
            nav.painter().text(pos2(rect.left() + 12.0 + 17.0 + 11.0, rect.center().y), Align2::LEFT_CENTER, crate::i18n::t(label), theme::medium(13.5), fg);
            if r.clicked() && !on {
                self.pages.settings_tab = i;
                self.pages.keys_listening = None;
                crate::app::keys::RECORDING.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let used = nav.min_rect().expand(12.0);
        let nav_bg = Rect::from_min_size(nav_rect.min, vec2(NAV_W, used.height()));
        nav.painter().set(bg, egui::Shape::rect_filled(nav_bg, 24, theme::glass()));
        let body = Rect::from_min_max(pos2(avail.left() + NAV_W + NAV_GAP, avail.top()), pos2(avail.left() + width, avail.bottom()));
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        egui::ScrollArea::vertical().id_salt(("settings", self.pages.settings_tab)).auto_shrink([false, false]).show(&mut bui, |ui| {
            ui.set_width(ui.available_width() - 10.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            match tab {
                Tab::Playback => self.settings_playback(ui),
                Tab::Sound => self.settings_sound(ui),
                Tab::Services => self.settings_services(ui),
                Tab::Appearance => self.settings_look(ui),
                Tab::Keys => self.settings_keys(ui),
                Tab::Overlay => self.settings_overlay(ui),
                Tab::Discord => self.settings_discord(ui),
                Tab::Data => self.settings_data(ui),
                Tab::Diagnostics => self.settings_diagnostics(ui),
                Tab::About => self.settings_about(ui),
            }
            ui.add_space(24.0);
        });
    }

    fn settings_playback(&mut self, ui: &mut Ui) {
        kit::tab_header(ui, ic::PLAY_CIRCLE, 160.0, "Воспроизведение", "Что происходит, когда музыка заканчивается, запускается или меняется устройство.");
        kit::gap(ui);
        kit::card(ui, "Очередь", "", None, |ui| {
            kit::setting(ui, "Автовоспроизведение", "Когда очередь закончится, продолжить похожими треками", |s| s.autoplay, |s, v| s.autoplay = v);
            let fade = state::get().settings.read().unwrap().crossfade.round();
            let hint = if fade > 0.0 { format!("{} {fade} {}", crate::i18n::t("Следующий трек вступает за"), crate::i18n::t("с до конца текущего")) } else { crate::i18n::t("Выключен: следующий трек готовится заранее и начинается сразу, без паузы") };
            kit::row(ui, "Плавный переход", &hint, |ui| {
                let label = if fade > 0.0 { format!("{fade} с") } else { crate::i18n::t("выкл") };
                ui.add_sized(vec2(64.0, 20.0), egui::Label::new(egui::RichText::new(label).font(theme::body(12.5)).color(theme::ink3())));
                if let Some(v) = kit::dial(ui, 160.0, fade, 0.0, 12.0, 1.0) {
                    set_settings(|s| s.crossfade = v);
                }
            });
            kit::setting(ui, "Продолжать с места", "При запуске вернуть очередь и позицию — останется только нажать «Играть»", |s| s.resume_on_start, |s, v| s.resume_on_start = v);
            kit::setting(ui, "Запуск двойным кликом", "Одиночный клик по треку только выделяет его — удобно, если часто промахиваетесь", |s| s.double_click_to_play, |s, v| s.double_click_to_play = v);
        });
        kit::card(ui, "Устройства", "", None, |ui| {
            kit::setting(ui, "Пауза при отключении наушников", "Если пропадёт устройство вывода, музыка остановится, а не заиграет из колонок", |s| s.pause_on_device_change, |s, v| s.pause_on_device_change = v);
        });
        kit::card(ui, "Поиск и тексты", "", None, |ui| {
            let src = state::get().settings.read().unwrap().default_search_source;
            let sel = match src {
                SearchSource::All => 0,
                SearchSource::Soundcloud => 1,
                SearchSource::Youtube => 2,
            };
            kit::row(ui, "Где искать по умолчанию", "", |ui| {
                if let Some(i) = kit::segmented(ui, &["Везде", "SoundCloud", "YouTube"], sel) {
                    set_settings(|s| s.default_search_source = [SearchSource::All, SearchSource::Soundcloud, SearchSource::Youtube][i]);
                }
            });
            kit::setting(ui, "Запасные тексты", "Если в LRCLIB нет текста, взять обычный (без синхронизации) с genius.com, а если и там нет — с lyrics.ovh", |s| s.genius_fallback, |s, v| s.genius_fallback = v);
        });
        kit::card(ui, "Окно", "", None, |ui| {
            kit::setting(ui, "Закрытие прячет в трей", "Крестик убирает окно к часам, музыка продолжает играть. Выйти — через меню значка", |s| s.close_to_tray, |s, v| s.close_to_tray = v);
            kit::setting(ui, "Сворачивать в трей", "Свёрнутое окно не занимает место на панели задач", |s| s.minimize_to_tray, |s, v| s.minimize_to_tray = v);
        });
    }

    fn settings_sound(&mut self, ui: &mut Ui) {
        kit::tab_header(ui, ic::EQ, 140.0, "Звук и эквалайзер", "Подстройте звучание под наушники и вкус. Скорость, тон и реверберация — в кнопке «Звук» у плеера.");
        kit::gap(ui);
        kit::card(ui, "", "", None, |ui| self.equalizer_ui(ui));
    }

    fn settings_look(&mut self, ui: &mut Ui) {
        kit::tab_header(ui, "@palette", 300.0, "Внешний вид", "Фон, свечение и текст песен.");
        kit::gap(ui);
        kit::card(ui, "Язык", "", None, |ui| {
            let lang = state::get().settings.read().unwrap().language;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                let english = crate::i18n::is_english();
                for (l, label, eng, flag) in [(Language::Auto, "Как в системе", "Same as the system", "auto"), (Language::Ru, "Русский", "Russian", "ru"), (Language::En, "English", "English", "gb")] {
                    // the names stay as they are: each language is named in its own words
                    let text = if l == Language::Auto && english { eng.to_string() } else if l == Language::Auto { label.to_string() } else { label.to_string() };
                    let _ = eng;
                    let paint = move |ui: &Ui, r: Rect| paint_flag(ui, r, flag);
                    if kit::chip_sized(ui, &text, lang == l, 36.0, Some(&paint)).clicked() {
                        set_settings(|s| s.language = l);
                        crate::app::apply_language(&state::get().settings());
                    }
                }
            });
        });
        kit::card(ui, "Экономия ресурсов", "", None, |ui| {
            kit::setting(ui, "Режим экономии памяти", "Без живого фона, свечения, размытия и анимаций; спрятанное окно освобождает память через 4 секунды. Поиск, плейлисты, музыка и мини-плеер работают как обычно", |s| s.memory_saver, |s, v| s.memory_saver = v);
            self.dont_keep(ui);
        });
        kit::card(ui, "Фон и обложка", "", None, |ui| {
            kit::setting(ui, "Живой фон из обложки", "Медленно переливающийся фон в цветах трека. Выключите на слабом компьютере", |s| s.animated_backdrop, |s, v| s.animated_backdrop = v);
            kit::setting(ui, "Панель «Далее»", "Очередь справа на экране «Сейчас играет». Можно скрыть, если не хотите знать, что будет дальше", |s| s.show_queue, |s, v| s.show_queue = v);
            kit::setting(ui, "Свечение обложки", "Мягкий ореол вокруг большой обложки на экране «Сейчас играет»", |s| s.cover_glow, |s, v| s.cover_glow = v);
        });
        kit::card(ui, "Текст песни", "", None, |ui| {
            kit::setting(ui, "Размывать дальние строки", "Текущая строка резкая, остальные чуть размыты", |s| s.lyrics_blur, |s, v| s.lyrics_blur = v);
            let size = state::get().settings.read().unwrap().lyrics_size;
            let sel = match size {
                LyricsSize::S => 0,
                LyricsSize::M => 1,
                LyricsSize::L => 2,
            };
            kit::row(ui, "Размер текста", "", |ui| {
                if let Some(i) = kit::segmented(ui, &["Меньше", "Обычный", "Крупнее"], sel) {
                    set_settings(|s| s.lyrics_size = [LyricsSize::S, LyricsSize::M, LyricsSize::L][i]);
                }
            });
        });
        kit::note(ui, "Анимации и фон сами останавливаются, когда окно свёрнуто или спрятано в трей.");
    }

    /// "Не сохранять": what EvPlayer should not remember, each with what it takes now
    fn dont_keep(&mut self, ui: &mut Ui) {
        let stale = self.pages.dont_keep.as_ref().is_none_or(|(at, _)| at.elapsed() > std::time::Duration::from_secs(5));
        if stale {
            let kv = &state::get().kv;
            let cache = crate::app::cache_dir();
            let covers = dir_size(&cache.join("covers"));
            let service = dir_size(&cache).saturating_sub(covers);
            let sizes = [covers, service, kv.size("evplayer.loudness.v1") as u64, crate::stats::bytes() as u64, kv.size(crate::state::HISTORY) as u64];
            self.pages.dont_keep = Some((std::time::Instant::now(), sizes));
        }
        let sizes = self.pages.dont_keep.as_ref().map(|d| d.1).unwrap_or_default();
        ui.add_space(6.0);
        ui.painter().hline(ui.available_rect_before_wrap().x_range(), ui.cursor().top(), Stroke::new(1.0, theme::edge()));
        ui.add_space(8.0);
        kit::eyebrow(ui, "Не сохранять");
        ui.add_space(4.0);
        let s = state::get().settings();
        let items: [(&str, &str, bool, fn(&mut crate::store::state::Settings, bool)); 5] = [
            ("Обложки и картинки", "Не копятся на диске — скачиваются заново. Включается полностью после перезапуска", s.no_cache, |s, v| s.no_cache = v),
            ("Ответы сервисов", "Рекомендации, похожие треки и тексты песен ищутся каждый раз заново", s.no_service_cache, |s, v| s.no_service_cache = v),
            ("Громкость треков", "«Выравнивать громкость» не запоминает, насколько громкий каждый трек", s.no_loudness_memory, |s, v| s.no_loudness_memory = v),
            ("Статистика", "Топы, графики и умные плейлисты перестают пополняться", s.no_stats, |s, v| s.no_stats = v),
            ("История", "«Недавно слушали» на главной больше не пополняется", s.no_history, |s, v| s.no_history = v),
        ];
        for (i, (title, hint, on, set)) in items.into_iter().enumerate() {
            if let Some(v) = kit::check_row(ui, title, hint, on, &bytes_text(sizes[i])) {
                set_settings(|s| set(s, v));
            }
        }
    }

    /// Клавиши (settings/KeysTab.tsx): the scope, a filter, each action with its keys; pressing
    /// the keys waits for a new combination
    fn settings_keys(&mut self, ui: &mut Ui) {
        use crate::app::keys::{self as k, Scope};
        kit::tab_header(ui, ic::KEYBOARD, 265.0, "Клавиши", "Нажмите на сочетание, чтобы поменять его. Esc — отмена, Backspace — убрать клавишу.");
        kit::gap(ui);
        self.record_key(ui);
        let scope = [Scope::App, Scope::Global, Scope::Overlay][self.pages.keys_scope.min(2)];
        let settings = state::get().settings();
        let taken = crate::system::taken_hotkeys();
        kit::card(ui, "", "", None, |ui| {
            ui.horizontal(|ui| {
                if let Some(i) = kit::segmented(ui, &["В окне EvPlayer", "Везде в системе", "В мини-плеере"], self.pages.keys_scope) {
                    self.pages.keys_scope = i;
                    self.pages.keys_listening = None;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(240.0, 36.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 12, Color32::from_rgba_unmultiplied(238, 247, 244, 18));
                    crate::ui::icons::paint(ui.painter(), "search", Rect::from_center_size(pos2(rect.left() + 12.0 + 7.5, rect.center().y), vec2(15.0, 15.0)), theme::ink3());
                    let inner = Rect::from_min_max(pos2(rect.left() + 12.0 + 15.0 + 8.0, rect.top()), pos2(rect.right() - 12.0, rect.bottom()));
                    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
                    c.add(egui::TextEdit::singleline(&mut self.pages.keys_query).hint_text(egui::RichText::new(crate::i18n::t("Найти действие")).color(theme::ink3())).frame(egui::Frame::NONE).font(egui::FontId::new(13.0, theme::medium500())).desired_width(inner.width()).align(egui::Align2::LEFT_CENTER));
                });
            });
            ui.add_space(6.0);
            if scope == Scope::Overlay {
                kit::note(ui, "Работают, когда мини-плеер в фокусе — кликните по нему один раз, например в игре в оконном режиме.");
            }
            if scope == Scope::Global {
                let before = settings.global_shortcuts;
                kit::setting(ui, "Глобальные клавиши включены", "Работают, даже когда EvPlayer свёрнут или в трее. Медиаклавиши клавиатуры работают всегда", |s| s.global_shortcuts, |s, v| s.global_shortcuts = v);
                if state::get().settings.read().unwrap().global_shortcuts != before {
                    self.keys.apply_global(&state::get().settings());
                }
            }
            let q = self.pages.keys_query.trim().to_lowercase();
            let list: Vec<&k::Shortcut> = k::SHORTCUTS.iter().filter(|d| d.scope == scope && (q.is_empty() || format!("{} {}", crate::i18n::t(d.title), crate::i18n::t(d.hint)).to_lowercase().contains(&q) || format!("{} {}", d.title, d.hint).to_lowercase().contains(&q))).collect();
            for (n, def) in list.iter().enumerate() {
                let acc = k::binding(&settings.shortcuts, def).map(str::to_string);
                let listening = self.pages.keys_listening == Some(def.id);
                let changed = settings.shortcuts.get(def.id).is_some_and(|v| v.as_str() != def.default);
                let failed = scope == Scope::Global && settings.global_shortcuts && acc.as_ref().is_some_and(|a| taken.contains(a));
                // .key-row: 9 by 4 padding, lines between, a soft accent while waiting for a key
                let w = ui.available_width();
                let text_w = w - 150.0 - 30.0 - 36.0 - if failed { 190.0 } else { 0.0 };
                let tg = kit::wrapped(ui, &crate::i18n::t(def.title), theme::medium(13.5), INK, text_w, 18.0);
                let hg = kit::wrapped(ui, &crate::i18n::t(def.hint), theme::body(12.0), theme::ink3(), text_w, 16.8);
                let h = (tg.size().y + 2.0 + hg.size().y).max(36.0) + 18.0;
                let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
                if n > 0 {
                    ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 15)));
                }
                if listening {
                    ui.painter().rect_filled(rect, 14, Color32::from_rgba_unmultiplied(111, 240, 196, 20));
                }
                let text_h = tg.size().y + 2.0 + hg.size().y;
                let y = rect.center().y - text_h / 2.0;
                let th = tg.size().y;
                ui.painter().galley(pos2(rect.left() + 4.0, y), tg, INK);
                ui.painter().galley(pos2(rect.left() + 4.0, y + th + 2.0), hg, theme::ink3());
                // the undo, the keys, then the "taken" note, right to left
                let undo = Rect::from_center_size(pos2(rect.right() - 4.0 - 15.0, rect.center().y), vec2(30.0, 30.0));
                if changed {
                    let r = ui.interact(undo, ui.scope_id().with(("undo", def.id)), egui::Sense::click());
                    if r.hovered() {
                        ui.painter().circle_filled(undo.center(), 15.0, theme::hover());
                    }
                    crate::ui::icons::paint(ui.painter(), "undo", Rect::from_center_size(undo.center(), vec2(15.0, 15.0)), if r.hovered() { INK } else { theme::ink2() });
                    let r = r.on_hover_text(crate::i18n::t("Вернуть как было"));
                    if r.clicked() {
                        let id = def.id;
                        set_settings(|s| {
                            s.shortcuts.remove(id);
                        });
                        self.keys.apply_global(&state::get().settings());
                    }
                }
                let key_rect = Rect::from_min_size(pos2(undo.left() - 12.0 - 150.0, rect.center().y - 18.0), vec2(150.0, 36.0));
                let r = ui.interact(key_rect, ui.scope_id().with(("key", def.id)), egui::Sense::click());
                ui.painter().rect(
                    key_rect,
                    12,
                    Color32::from_rgba_unmultiplied(238, 247, 244, if r.hovered() { 26 } else { 15 }),
                    Stroke::new(1.0, if listening { theme::ACCENT } else { theme::edge() }),
                    egui::StrokeKind::Inside,
                );
                if listening {
                    ui.painter().text(key_rect.center(), Align2::CENTER_CENTER, crate::i18n::t("Нажмите сочетание…"), theme::heavy(12.5), theme::ACCENT);
                } else if let Some(a) = &acc {
                    paint_keys(ui, key_rect, a);
                } else {
                    ui.painter().text(key_rect.center(), Align2::CENTER_CENTER, crate::i18n::t("не задано"), theme::body(12.5), theme::ink3());
                }
                let r = r.on_hover_text(crate::i18n::t(if listening { "Esc — отмена, Backspace — убрать" } else { "Нажмите, чтобы назначить" }));
                if r.clicked() {
                    self.pages.keys_listening = if listening { None } else { Some(def.id) };
                }
                if failed {
                    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(key_rect.left() - 12.0, rect.bottom()))).layout(egui::Layout::right_to_left(egui::Align::Center)));
                    kit::pill(&mut c, Tone::Warn, "занято другой программой");
                }
            }
            if list.is_empty() {
                kit::note(ui, "Ничего не нашлось.");
            }
            ui.add_space(6.0);
            if widgets::btn(ui, Some("@undo"), "Сбросить все в этом разделе", false).clicked() {
                set_settings(|s| {
                    for d in k::SHORTCUTS.iter().filter(|d| d.scope == scope) {
                        s.shortcuts.remove(d.id);
                    }
                });
                self.keys.apply_global(&state::get().settings());
                self.toast("Клавиши сброшены");
            }
        });
        crate::app::keys::RECORDING.store(self.pages.keys_listening.is_some(), std::sync::atomic::Ordering::Relaxed);
    }

    /// a key pressed while a shortcut waits for one
    fn record_key(&mut self, ui: &mut Ui) {
        use crate::app::keys as k;
        let Some(id) = self.pages.keys_listening else { return };
        let Some(def) = k::SHORTCUTS.iter().find(|d| d.id == id) else { return };
        let pressed: Vec<(egui::Key, egui::Modifiers)> = ui.ctx().input(|i| i.events.iter().filter_map(|e| if let egui::Event::Key { key, pressed: true, modifiers, .. } = e { Some((*key, *modifiers)) } else { None }).collect());
        for (key, m) in pressed {
            if key == egui::Key::Escape {
                self.pages.keys_listening = None;
                return;
            }
            if matches!(key, egui::Key::Backspace | egui::Key::Delete) {
                set_settings(|s| {
                    s.shortcuts.insert(id.to_string(), serde_json::Value::Null);
                });
                self.keys.apply_global(&state::get().settings());
                self.pages.keys_listening = None;
                return;
            }
            let Some(next) = k::accelerator(key, m) else { continue };
            let fkey = next.rsplit('+').next().is_some_and(|k| k.len() > 1 && k.starts_with('F'));
            if def.scope == k::Scope::Global && !(m.ctrl || m.alt) && !fkey {
                self.toast("Для глобальной клавиши нужен Ctrl или Alt — иначе она перехватит обычный ввод");
                return;
            }
            let current = state::get().settings().shortcuts;
            let clash = k::conflict_for(&current, id, &next);
            set_settings(|s| {
                s.shortcuts.insert(id.to_string(), serde_json::Value::String(next.clone()));
                if let Some(c) = clash {
                    s.shortcuts.insert(c.id.to_string(), serde_json::Value::Null);
                }
            });
            if let Some(c) = clash {
                self.toast(&format!("«{}» {} «{}» — {}", k::pretty(&next), crate::i18n::t("было у"), crate::i18n::t(c.title), crate::i18n::t("там теперь пусто")));
            }
            self.keys.apply_global(&state::get().settings());
            self.pages.keys_listening = None;
            return;
        }
    }

    /// Мини-плеер (settings/OverlayTab.tsx)
    fn settings_overlay(&mut self, ui: &mut Ui) {
        let s = state::get().settings();
        let shown = crate::ui::mini::is_shown();
        kit::tab_header(ui, ic::MINI, 190.0, "Мини-плеер", "Маленькое окно поверх всех программ: обложка, название, строка текста и кнопки.");
        kit::gap(ui);
        kit::card(ui, "", "", None, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 20.0;
                // .ov-sample: a drawing of the mini player
                let (r, _) = ui.allocate_exact_size(vec2(260.0, 80.0), egui::Sense::hover());
                let p = ui.painter();
                p.add(egui::Shadow { offset: [0, 16], blur: 40, spread: 0, color: Color32::from_black_alpha(89) }.as_shape(r, CornerRadius::same(20)));
                p.rect(r, 20, Color32::from_rgba_unmultiplied(6, 22, 22, 230), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
                let art = Rect::from_min_size(r.min + vec2(10.0, 10.0), vec2(60.0, 60.0));
                kit::gradient_rect(ui, art, 13.0, Color32::from_rgb(0x6f, 0xf0, 0xc4), Color32::from_rgb(0x1f, 0x6a, 0x8a), 145.0);
                let x = art.right() + 12.0;
                let lw = r.right() - 10.0 - x;
                let p = ui.painter();
                let ly = r.center().y - (8.0 + 7.0 + 8.0 + 7.0 + 4.0) / 2.0;
                p.rect_filled(Rect::from_min_size(pos2(x, ly), vec2(lw, 8.0)), 4, Color32::from_rgba_unmultiplied(238, 247, 244, 77));
                p.rect_filled(Rect::from_min_size(pos2(x, ly + 15.0), vec2(lw * 0.7, 8.0)), 4, Color32::from_rgba_unmultiplied(238, 247, 244, 38));
                p.rect_filled(Rect::from_min_size(pos2(x, ly + 30.0), vec2(lw * 0.4, 4.0)), 2, Color32::from_rgba_unmultiplied(238, 247, 244, 128));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.label(egui::RichText::new(crate::i18n::t(if shown { "Мини-плеер открыт" } else { "Мини-плеер спрятан" })).font(theme::medium(13.5)).color(INK));
                    ui.label(egui::RichText::new(crate::i18n::t("Перетаскивайте его за название трека. Спрятать — клавишей, из трея или здесь.")).font(theme::body(12.0)).color(theme::ink3()));
                    if kit::btn_if(ui, s.overlay_enabled, None, if shown { "Спрятать" } else { "Показать сейчас" }, true).clicked() {
                        crate::events::send(crate::events::Event::Action(crate::app::actions::Action::ToggleOverlay));
                    }
                });
            });
        });
        kit::card(ui, "Поведение", "", None, |ui| {
            kit::setting(ui, "Мини-плеер включён", "Если выключить, его нельзя будет открыть ни клавишей, ни из трея", |s| s.overlay_enabled, |s, v| s.overlay_enabled = v);
            kit::setting_if(ui, "Поверх всех окон", "Не прячется за браузером и играми в окне", s.overlay_enabled, |s| s.overlay_on_top, |s, v| s.overlay_on_top = v);
            kit::setting_if(ui, "Показывать при сворачивании", "Свернули EvPlayer — появился мини-плеер; развернули — исчез", s.overlay_enabled, |s| s.overlay_on_minimize, |s, v| s.overlay_on_minimize = v);
            kit::setting_if(ui, "Строка текста песни", "Вместо исполнителя показывать строку, которую сейчас поют", s.overlay_enabled, |s| s.overlay_show_lyrics, |s, v| s.overlay_show_lyrics = v);
        });
        kit::card(ui, "Где появляется", "Если вы перетащили мини-плеер, он остаётся там, пока не выберете угол заново", None, |ui| {
            use OverlayCorner::*;
            let corners = [(TopLeft, "Слева сверху"), (TopCenter, "По центру сверху"), (TopRight, "Справа сверху"), (BottomLeft, "Слева снизу"), (BottomCenter, "По центру снизу"), (BottomRight, "Справа снизу")];
            // .corner-picker: a little screen, 300 by 168, the six places on it
            let (screen, _) = ui.allocate_exact_size(vec2(300.0, 168.0), egui::Sense::hover());
            ui.painter().rect(screen, 16, Color32::from_rgba_unmultiplied(238, 247, 244, 15), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
            for (i, (c, label)) in corners.iter().enumerate() {
                let size = vec2(64.0, 30.0);
                let x = match i % 3 {
                    0 => screen.left() + 10.0,
                    1 => screen.center().x - 32.0,
                    _ => screen.right() - 10.0 - size.x,
                };
                let y = if i < 3 { screen.top() + 10.0 } else { screen.bottom() - 10.0 - size.y };
                let rect = Rect::from_min_size(pos2(x, y), size);
                let r = ui.interact(rect, ui.scope_id().with(("corner", i)), egui::Sense::click());
                let on = s.overlay_corner == *c;
                let fill = if on { theme::ACCENT } else if r.hovered() { Color32::from_rgba_unmultiplied(238, 247, 244, 89) } else { Color32::from_rgba_unmultiplied(238, 247, 244, 46) };
                ui.painter().rect_filled(Rect::from_center_size(rect.center(), vec2(52.0, 18.0)), 6, fill);
                if r.on_hover_text(crate::i18n::t(label)).clicked() {
                    let c = *c;
                    set_settings(|s| s.overlay_corner = c);
                }
            }
        });
        kit::card(ui, "Клавиши", "", None, |ui| {
            let key = |id: &str| crate::app::keys::SHORTCUTS.iter().find(|d| d.id == id).and_then(|d| crate::app::keys::binding(&s.shortcuts, d)).map(crate::app::keys::pretty).unwrap_or_else(|| crate::i18n::t("не задано"));
            kit::row(ui, "В окне EvPlayer", &key("app.overlay"), |_| {});
            kit::row(ui, "Из любой программы", &key("global.overlay"), |_| {});
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
            kit::row(ui, "Внутри мини-плеера", &inside, |_| {});
            kit::note(ui, "Поменять — во вкладке «Клавиши». Ещё мини-плеер открывается из меню значка в трее.");
        });
    }

    /// Discord (settings/DiscordTab.tsx): the account, what is shown, how friends see it, the
    /// application
    fn settings_discord(&mut self, ui: &mut Ui) {
        let s = state::get().settings();
        let st = crate::presence::discord_status();
        let has_id = !s.discord_app_id().is_empty();
        let show_setup = *self.pages.discord_setup.get_or_insert(!has_id);
        let (tone, label) = match st.status {
            "connected" => (Tone::Ok, "подключено"),
            "off" => (Tone::Off, "выключено"),
            "connecting" => (Tone::Warn, "подключаюсь…"),
            "no-discord" => (Tone::Warn, "Discord не запущен"),
            _ => (Tone::Warn, "ошибка"),
        };
        let name = st.user.as_ref().map(|u| u.global_name.clone().filter(|g| !g.is_empty()).unwrap_or_else(|| u.username.clone())).unwrap_or_else(|| crate::i18n::t("Вы"));
        // the status is asked for again every few seconds while the tab is open
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(2500));
        kit::tab_header(ui, ic::DISCORD, 235.0, "Discord", "Показывайте друзьям, что сейчас играет: трек, обложка и время прямо в профиле.");
        kit::gap(ui);
        let connected = st.status == "connected";
        let avatar = st.user.as_ref().and_then(|u| u.avatar.clone());
        let username = st.user.as_ref().map(|u| u.username.clone());
        kit::card(ui, "", "", None, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), egui::Sense::hover());
                self.round_avatar(ui, r, avatar.as_deref(), Color32::from_rgb(0x58, 0x65, 0xf2));
                let w = ui.available_width() - 14.0 - 42.0 - 14.0 - 150.0;
                ui.vertical(|ui| {
                    ui.set_width(w.max(120.0));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(if connected { name.clone() } else { "Discord".into() }).font(theme::heavy(17.0)).color(INK));
                    let sub = match (&username, connected) {
                        (Some(u), true) => format!("@{u}"),
                        _ => crate::i18n::t("Нужен запущенный Discord для компьютера"),
                    };
                    ui.label(egui::RichText::new(sub).font(theme::body(12.5)).color(theme::ink3()));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut on = s.discord_enabled;
                    ui.add_enabled_ui(has_id, |ui| {
                        if kit::switch(ui, &mut on).changed() {
                            set_settings(|s| s.discord_enabled = on);
                            crate::presence::changed();
                        }
                    });
                    kit::pill(ui, tone, label);
                });
            });
            if !has_id {
                kit::note(ui, "Сначала один раз подключите приложение ниже — это займёт минуту.");
            }
            if has_id && s.discord_enabled && st.status == "no-discord" {
                kit::note(ui, "Не вижу Discord. Запустите приложение Discord для компьютера (не браузерную версию) — EvPlayer подключится сам.");
            }
            if let Some(e) = &st.error {
                let text = if e == "Invalid Client ID" { crate::i18n::t("такого Application ID нет — проверьте его ниже") } else { e.clone() };
                kit::error(ui, &format!("Discord: {text}"));
            }
        });
        // .dc-grid: what is shown, and the preview, side by side (1.2fr and 1fr)
        let w = ui.available_width();
        let right_w = ((w - 16.0) / 2.2).max(280.0);
        let left_w = w - 16.0 - right_w;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.vertical(|ui| {
                ui.set_width(left_w);
                kit::card(ui, "Что видно", "", None, |ui| {
                    kit::setting(ui, "Обложка", "Картинка трека рядом со статусом", |s| s.discord_show_artwork, |s, v| s.discord_show_artwork = v);
                    kit::setting(ui, "Время", "Полоска прогресса и сколько осталось", |s| s.discord_show_timeline, |s, v| s.discord_show_timeline = v);
                    kit::setting(ui, "Кнопка «Слушать»", "Друзья откроют трек в EvPlayer (а без него — на SoundCloud или YouTube)", |s| s.discord_show_button, |s, v| s.discord_show_button = v);
                    kit::setting(ui, "Слушать вместе", "Пока идёт совместная сессия, под статусом есть кнопка «Слушать вместе»: друг нажмёт — и войдёт в неё", |s| s.discord_join, |s, v| s.discord_join = v);
                    kit::setting(ui, "Показывать на паузе", "Иначе статус исчезает, пока музыка стоит", |s| s.discord_show_paused, |s, v| s.discord_show_paused = v);
                    let sel = match s.discord_status_shows {
                        StatusShows::Song => 0,
                        StatusShows::Artist => 1,
                        StatusShows::App => 2,
                    };
                    kit::row(ui, "В списке участников", "Что написано под вашим именем на сервере", |ui| {
                        if let Some(i) = kit::segmented(ui, &["Трек", "Исполнитель", "EvPlayer"], sel) {
                            set_settings(|s| s.discord_status_shows = [StatusShows::Song, StatusShows::Artist, StatusShows::App][i]);
                        }
                    });
                });
                crate::presence::changed();
            });
            ui.vertical(|ui| {
                ui.set_width(right_w);
                kit::card(ui, "Так увидят друзья", "", None, |ui| self.discord_preview(ui, if connected { &name } else { "Вы" }));
            });
        });
        let hint = "В этой сборке уже есть приложение EvPlayer — менять ничего не нужно.";
        let mut toggle = false;
        let mut actions = |ui: &mut Ui| {
            if widgets::btn(ui, None, if show_setup { "Скрыть" } else { "Настроить" }, false).clicked() {
                toggle = true;
            }
        };
        kit::card(ui, "Подключение приложения", hint, Some(&mut actions), |ui| {
            if show_setup {
                let steps = [
                    "Откройте discord.com/developers/applications и нажмите «New Application».",
                    "Назовите его EvPlayer — это имя появится в «Слушает …».",
                    "На странице «General Information» скопируйте Application ID и вставьте сюда.",
                ];
                for (i, step) in steps.iter().enumerate() {
                    let w = ui.available_width() - 20.0;
                    let g = kit::wrapped(ui, &format!("{}. {}", i + 1, crate::i18n::t(step)), theme::body(13.0), theme::ink2(), w, 18.85);
                    let (r, resp) = ui.allocate_exact_size(vec2(w + 20.0, g.size().y), egui::Sense::click());
                    ui.painter().galley(r.min + vec2(4.0, 0.0), g, theme::ink2());
                    if i == 0 && resp.clicked() {
                        crate::dialogs::open_path(std::path::Path::new("https://discord.com/developers/applications"));
                    }
                }
                ui.add_space(4.0);
                let mut id = state::get().settings.read().unwrap().discord_app_id.clone();
                if kit::field(ui, "Application ID", &mut id, crate::store::state::DEFAULT_DISCORD_APP_ID, false).changed() {
                    let digits: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
                    set_settings(|s| s.discord_app_id = digits);
                    crate::presence::changed();
                }
            }
        });
        if toggle {
            self.pages.discord_setup = Some(!show_setup);
        }
    }

    /// a round picture: an avatar, or a colour with a person in it
    fn round_avatar(&mut self, ui: &mut Ui, r: Rect, src: Option<&str>, fill: Color32) {
        match src {
            Some(url) => widgets::paint_cover(ui, &mut self.images, Some(url), r, (r.width() / 2.0) as u8),
            None => {
                ui.painter().circle_filled(r.center(), r.width() / 2.0, fill);
                crate::ui::icons::paint(ui.painter(), "user", Rect::from_center_size(r.center(), vec2(r.width() * 0.46, r.width() * 0.46)), Color32::WHITE);
            }
        }
    }

    /// .dc-preview: the profile card as a friend sees it in Discord
    fn discord_preview(&mut self, ui: &mut Ui, name: &str) {
        let s = state::get().settings();
        let track = self.view.current().cloned();
        let playing = self.view.status == crate::player::Status::Playing;
        let shown = track.is_some() && (playing || s.discord_show_paused);
        let status_line = match (&track, s.discord_status_shows) {
            (None, _) => String::new(),
            (Some(_), StatusShows::App) => "EvPlayer".into(),
            (Some(t), StatusShows::Artist) => t.artist_line(),
            (Some(t), StatusShows::Song) => t.title.clone(),
        };
        if playing && s.discord_show_timeline {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        }
        egui::Frame::new().fill(Color32::from_rgb(0x11, 0x12, 0x14)).stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 15))).corner_radius(16).inner_margin(14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 15.0, Color32::from_rgb(0x58, 0x65, 0xf2));
                crate::ui::icons::paint(ui.painter(), "user", Rect::from_center_size(r.center(), vec2(14.0, 14.0)), Color32::WHITE);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(egui::RichText::new(name).font(theme::heavy(13.5)).color(Color32::from_rgb(0xf2, 0xf3, 0xf5)));
                    let line = if shown { format!("{} {status_line}", crate::i18n::t("Слушает")) } else { "—".into() };
                    ui.label(egui::RichText::new(line).font(theme::body(12.0)).color(Color32::from_rgb(0xb5, 0xba, 0xc1)));
                });
            });
            egui::Frame::new().fill(Color32::from_rgb(0x1e, 0x1f, 0x22)).corner_radius(10).inner_margin(12).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 8.0;
                let job = egui::text::LayoutJob::single_section(crate::i18n::t("Слушает EvPlayer").to_uppercase(), egui::TextFormat { font_id: theme::heavy(11.0), color: Color32::from_rgb(0xdb, 0xde, 0xe1), extra_letter_spacing: 0.44, ..Default::default() });
                ui.label(job);
                match (&track, shown) {
                    (Some(t), true) => {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), egui::Sense::hover());
                            match crate::ui::images::artwork_of(t).filter(|_| s.discord_show_artwork) {
                                Some(a) => widgets::paint_cover(ui, &mut self.images, Some(&a), r, 8),
                                None => {
                                    ui.painter().rect_filled(r, 8, Color32::from_rgb(0x2b, 0x2d, 0x31));
                                    crate::ui::icons::paint(ui.painter(), "music", Rect::from_center_size(r.center(), vec2(26.0, 26.0)), Color32::from_rgb(0x80, 0x84, 0x8e));
                                }
                            }
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                let w = ui.available_width();
                                let g = widgets::clipped(ui, &t.title, theme::heavy(13.5), Color32::from_rgb(0xf2, 0xf3, 0xf5), w);
                                ui.label(g);
                                let g = widgets::clipped(ui, &t.artist_line(), theme::body(12.5), Color32::from_rgb(0xb5, 0xba, 0xc1), w);
                                ui.label(g);
                                let dur = self.view.duration_ms;
                                if s.discord_show_timeline && playing && dur > 0.0 {
                                    let pos = (self.view.position_ms / 1000.0).floor() * 1000.0;
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        let grey = Color32::from_rgb(0xb5, 0xba, 0xc1);
                                        ui.label(egui::RichText::new(widgets::time(pos)).font(theme::body(11.0)).color(grey));
                                        let bw = (ui.available_width() - 40.0).max(20.0);
                                        let (r, _) = ui.allocate_exact_size(vec2(bw, 4.0), egui::Sense::hover());
                                        ui.painter().rect_filled(r, 2, Color32::from_rgb(0x4e, 0x50, 0x58));
                                        let f = (pos / dur).clamp(0.0, 1.0) as f32;
                                        ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(r.width() * f, 4.0)), 2, Color32::from_rgb(0xf2, 0xf3, 0xf5));
                                        ui.label(egui::RichText::new(widgets::time(dur)).font(theme::body(11.0)).color(grey));
                                    });
                                }
                                if !playing {
                                    ui.label(egui::RichText::new(crate::i18n::t("на паузе")).font(theme::body(12.5)).color(Color32::from_rgb(0xb5, 0xba, 0xc1)));
                                }
                            });
                        });
                        if s.discord_show_button && t.permalink.is_some() {
                            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), egui::Sense::hover());
                            ui.painter().rect_filled(r, 6, Color32::from_rgb(0x4e, 0x50, 0x58));
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, crate::i18n::t("Слушать в EvPlayer"), theme::medium(13.0), Color32::from_rgb(0xf2, 0xf3, 0xf5));
                        }
                    }
                    _ => kit::note(ui, if track.is_some() { "На паузе статус скрыт" } else { "Включите трек — здесь появится, как его видят друзья" }),
                }
            });
        });
    }

    /// Данные и копия (DataTab in src/pages/SettingsPage.tsx)
    fn settings_data(&mut self, ui: &mut Ui) {
        kit::tab_header(ui, ic::DATABASE, 45.0, "Данные и копия", "Всё хранится только на этом компьютере. Сделайте копию, чтобы перенести библиотеку.");
        kit::gap(ui);
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
        // .usage-grid: three tiles
        let w = ui.available_width();
        let tile_w = (w - 20.0) / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for (label, value) in [("Библиотека", size_text(library)), ("Статистика и история", size_text(stats)), ("Кэш обложек и ответов", format!("{:.0} МБ", cache as f64 / 1024.0 / 1024.0))] {
                let (r, _) = ui.allocate_exact_size(vec2(tile_w, 62.0), egui::Sense::hover());
                ui.painter().rect(r, 16, theme::glass(), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
                ui.painter().text(r.min + vec2(14.0, 12.0), Align2::LEFT_TOP, crate::i18n::t(label), theme::body(11.5), theme::ink3());
                ui.painter().text(r.min + vec2(14.0, 30.0), Align2::LEFT_TOP, value, theme::heavy(19.0), INK);
            }
        });
        kit::gap(ui);
        kit::card(ui, "Резервная копия", "Лайки, плейлисты, статистика, история, пресеты и настройки одним файлом", None, |ui| {
            ui.horizontal(|ui| {
                if widgets::btn(ui, None, "Сохранить копию", true).clicked() {
                    state::get().kv.flush();
                    let b = crate::backup::make(&state::get().kv);
                    if let Some(path) = crate::dialogs::save_file(&crate::backup::file_name(&b), "Резервная копия EvPlayer", "json") {
                        match serde_json::to_string(&b).map_err(|e| e.to_string()).and_then(|t| std::fs::write(&path, t).map_err(|e| e.to_string())) {
                            Ok(()) => self.toast("Копия сохранена"),
                            Err(e) => self.toast(&format!("Не удалось сохранить: {e}")),
                        }
                    }
                }
                if widgets::btn(ui, None, "Восстановить из копии", false).clicked() {
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
        kit::card(ui, "Очистка", "", None, |ui| {
            kit::row(ui, "Кэш", "Обложки и ответы сервисов — скачаются заново, когда понадобятся", |ui| {
                if widgets::btn(ui, None, "Очистить", false).clicked() {
                    let _ = std::fs::remove_dir_all(crate::app::cache_dir().join("covers"));
                    self.images.release_all();
                    self.pages.data_usage = None;
                    self.toast("Кэш очищен");
                }
            });
            kit::row(ui, "История прослушиваний", "«Недавно слушали» на главной и недавние запросы в поиске", |ui| {
                if widgets::btn(ui, None, "Очистить", false).clicked() {
                    state::get().update_history(|h| *h = Default::default());
                    super::search::clear_history();
                    self.pages.data_usage = None;
                    self.toast("История очищена");
                }
            });
            kit::row(ui, "Статистика", "Топы, графики и умные плейлисты строятся из неё", |ui| {
                if widgets::btn(ui, None, "Сбросить", false).clicked() {
                    self.dialog = Some(crate::app::Dialog::ResetStats);
                }
            });
            kit::row(ui, "Папка с данными", "Список ваших файлов и обложки к ним", |ui| {
                if widgets::btn(ui, None, "Открыть", false).clicked() {
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
        kit::card(ui, "Скачанное для офлайн", "Песни из SoundCloud и YouTube, сохранённые на этом компьютере: играют без интернета", None, |ui| {
            let (title, hint) = if count > 0 {
                (crate::i18n::tracks(count), format!("{:.0} {}", bytes as f64 / 1024.0 / 1024.0, crate::i18n::t("МБ на диске")))
            } else {
                (crate::i18n::t("Пока ничего не скачано"), crate::i18n::t("Скачать можно трек, плейлист или лайки: правый клик по треку или кнопка на странице списка"))
            };
            kit::row(ui, &title, &hint, |ui| {
                if widgets::btn(ui, None, "Скачать…", true).clicked() {
                    self.dialog = Some(crate::app::Dialog::Download { picked: vec![], follow: true });
                }
            });
            if !lists.is_empty() {
                let names: Vec<String> = lists.iter().map(|l| crate::keep::title_of(l)).collect();
                kit::row(ui, "Докачиваются сами", &names.join(", "), |ui| {
                    if widgets::btn(ui, None, "Перестать", false).clicked() {
                        crate::keep::remove(&lists);
                    }
                });
            }
            kit::row(ui, "Папка со скачанным", "Файлы лежат там под служебными именами", |ui| {
                if widgets::btn(ui, None, "Открыть", false).clicked() {
                    let dir = self.downloads.folder();
                    let _ = std::fs::create_dir_all(&dir);
                    crate::dialogs::open_path(&dir);
                }
            });
            if count > 0 {
                kit::row(ui, "Сохранить в папку", "Копии как обычные файлы «Исполнитель - Название» с тегами и обложкой: для другого плеера или телефона. Скачанное остаётся как было", |ui| {
                    if widgets::btn(ui, None, "Выбрать папку…", false).clicked() {
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
                kit::row(ui, "Удалить всё скачанное", "Песни останутся в плейлистах и будут играть из интернета", |ui| {
                    if widgets::btn(ui, None, "Удалить", false).clicked() {
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
        // .about-hero: a soft accent gradient, the icon, the name and the version
        let w = ui.available_width();
        let (hero, _) = ui.allocate_exact_size(vec2(w, 112.0), egui::Sense::hover());
        kit::gradient_rect(ui, hero, 24.0, Color32::from_rgba_unmultiplied(111, 240, 196, 41), Color32::from_rgba_unmultiplied(31, 106, 138, 51), 135.0);
        ui.painter().rect_stroke(hero, 24, Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let icon = Rect::from_min_size(pos2(hero.left() + 20.0, hero.center().y - 36.0), vec2(72.0, 72.0));
        // the program's icon (./icon.png in the page), loaded when this tab is first opened
        if self.logo.is_none() {
            if let Ok(img) = image::load_from_memory(include_bytes!("../../../../src-tauri/icons/128x128@2x.png")) {
                let img = img.into_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                self.logo = Some(ui.ctx().load_texture("about-icon", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR));
            }
        }
        if let Some(logo) = &self.logo {
            ui.painter().add(egui::Shape::Rect(egui::epaint::RectShape::filled(icon, 20, Color32::WHITE).with_texture(logo.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)))));
        }
        let x = icon.right() + 18.0;
        ui.painter().text(pos2(x, hero.center().y - 4.0), Align2::LEFT_BOTTOM, "EvPlayer", theme::heavy(26.0), INK);
        let line = format!("{} {} · {}", crate::i18n::t("Версия"), env!("CARGO_PKG_VERSION"), crate::i18n::t("открытый проект, лицензия MIT"));
        let g = ui.painter().layout_no_wrap(line, theme::body(14.0), theme::ink3());
        let vr = Rect::from_min_size(pos2(x, hero.center().y + 4.0), g.size());
        ui.painter().galley(vr.min, g, theme::ink3());
        // seven clicks on the version within three seconds turn the developer's tools on or off
        if ui.interact(vr, ui.scope_id().with("version"), egui::Sense::click()).clicked() {
            let now = std::time::Instant::now();
            self.pages.version_clicks.retain(|t| now.duration_since(*t) < std::time::Duration::from_secs(3));
            self.pages.version_clicks.push(now);
            if self.pages.version_clicks.len() >= 7 {
                self.pages.version_clicks.clear();
                let on = !state::get().settings.read().unwrap().dev_tools;
                set_settings(|s| s.dev_tools = on);
                self.toast(if on { "Диагностика включена: Настройки → Диагностика" } else { "Диагностика выключена" });
            }
        }
        kit::gap(ui);
        // .donate-card
        let tw = w - 40.0 - 46.0 - 16.0 - 16.0 - 140.0;
        let tg = ui.painter().layout_no_wrap(crate::i18n::t("Поддержать автора"), theme::medium(13.5), INK);
        let hg = kit::wrapped(ui, &crate::i18n::t("EvPlayer бесплатный и без рекламы. Если он вам нравится, можно поддержать разработку через DonationAlerts."), theme::body(12.0), theme::ink3(), tw, 16.8);
        let h = (tg.size().y + 3.0 + hg.size().y).max(46.0) + 36.0;
        let (card, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
        kit::gradient_rect(ui, card, 22.0, Color32::from_rgba_unmultiplied(255, 140, 60, 46), Color32::from_rgba_unmultiplied(255, 80, 120, 36), 135.0);
        ui.painter().rect_stroke(card, 22, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 170, 120, 64)), egui::StrokeKind::Inside);
        let di = Rect::from_min_size(pos2(card.left() + 20.0, card.center().y - 23.0), vec2(46.0, 46.0));
        kit::gradient_rect(ui, di, 14.0, Color32::from_rgb(0xff, 0x9a, 0x3c), Color32::from_rgb(0xff, 0x4f, 0x7a), 145.0);
        crate::ui::icons::paint(ui.painter(), "heartFill", Rect::from_center_size(di.center(), vec2(22.0, 22.0)), Color32::WHITE);
        let tx = di.right() + 16.0;
        let th = tg.size().y;
        let ty = card.center().y - (th + 3.0 + hg.size().y) / 2.0;
        ui.painter().galley(pos2(tx, ty), tg, INK);
        ui.painter().galley(pos2(tx, ty + th + 3.0), hg, theme::ink3());
        // .btn.donate
        let bg = ui.painter().layout_no_wrap(crate::i18n::t("Поддержать"), theme::medium(14.0), Color32::PLACEHOLDER);
        let br = Rect::from_min_size(pos2(card.right() - 20.0 - bg.size().x - 32.0, card.center().y - 20.0), vec2(bg.size().x + 32.0, 40.0));
        let resp = ui.interact(br, ui.scope_id().with("donate"), egui::Sense::click());
        kit::gradient_rect(ui, br, 20.0, Color32::from_rgb(0xff, 0x9a, 0x3c), Color32::from_rgb(0xff, 0x4f, 0x7a), 135.0);
        ui.painter().galley(br.center() - bg.size() / 2.0, bg, Color32::WHITE);
        if resp.clicked() {
            crate::dialogs::open_path(std::path::Path::new("https://www.donationalerts.com/r/evire"));
        }
        kit::gap(ui);
        kit::card(ui, "", "", None, |ui| {
            self.updates_row(ui);
            kit::row(ui, "Исходный код", "github.com/Evire1337/evplayer", |ui| {
                if widgets::btn(ui, None, "Открыть", false).clicked() {
                    crate::dialogs::open_path(std::path::Path::new("https://github.com/Evire1337/evplayer"));
                }
            });
        });
        kit::note(ui, "Музыка — SoundCloud и YouTube, тексты — LRCLIB и Genius. EvPlayer не связан с этими сервисами.");
    }

    /// "Обновления" (filled in by the updater)
    fn updates_row(&mut self, ui: &mut Ui) {
        let st = crate::updates::status();
        let icon = |ui: &mut Ui, r: Rect| {
            ui.painter().rect_filled(r, 10, Color32::from_rgba_unmultiplied(238, 247, 244, 18));
            crate::ui::icons::paint(ui.painter(), "refresh", Rect::from_center_size(r.center(), vec2(16.0, 16.0)), theme::ink2());
        };
        kit::row_full(ui, Some(&icon), "Обновления", &crate::updates::line(&st), false, false, |ui| {
            if let crate::updates::Status::Ready { .. } = st {
                if widgets::btn(ui, None, "Перезапустить", true).clicked() {
                    if let Err(e) = crate::updates::install() {
                        self.toast(&e);
                    }
                }
            } else {
                let busy = matches!(st, crate::updates::Status::Checking | crate::updates::Status::Downloading { .. });
                if kit::btn_if(ui, !busy, None, "Проверить", false).clicked() {
                    crate::updates::check_now();
                }
            }
        });
    }

    /// Диагностика (settings/DiagnosticsTab.tsx): what the program takes right now and what to
    /// do about it. Measures only while this tab is open.
    fn settings_diagnostics(&mut self, ui: &mut Ui) {
        kit::tab_header(ui, ic::CHART, 200.0, "Диагностика", "Сколько памяти и процессора берёт приложение, что из этого стоит дороже всего и что с этим сделать. Замеры идут только пока открыт этот раздел.");
        kit::gap(ui);
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
        let ready = probe_ready(&sample);
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
        let mut copy = false;
        let mut actions = |ui: &mut Ui| {
            if kit::btn_if(ui, ready, None, "Скопировать отчёт", false).clicked() {
                copy = true;
            }
        };
        let device = self.player.device();
        let covers = self.images.count();
        let first_frame = self.first_frame;
        kit::card(ui, "Сейчас", "", Some(&mut actions), |ui| {
            if !ready {
                kit::note(ui, "Замеряю…");
                return;
            }
            kit::value_row(ui, "Память всего", "1 процесс приложения, как «Память» в диспетчере задач", &crate::diagnostics::mb(sample.private_mb * 1024.0 * 1024.0));
            kit::value_row(ui, "Процессор", "Доля всего процессора, как в диспетчере задач", &format!("{:.1}%", sample.cpu));
            kit::value_row(ui, "В оперативной памяти", &format!("{} {}", crate::i18n::t("С общими библиотеками Windows; потоков:"), sample.threads), &crate::diagnostics::mb(sample.working_mb * 1024.0 * 1024.0));
            kit::value_row(ui, "Отрисовка", &format!("{}: {}", crate::i18n::t("Окно рисуется только когда что-то меняется"), crate::app::renderer_name()), &format!("{:.0} {}", sample.fps, crate::i18n::t("кадр/с")));
            kit::value_row(ui, "Обработка звука", &format!("{} · {}", crate::i18n::t("Эквалайзер, эффекты, кроссфейд"), if device.is_empty() { crate::i18n::t("звуковая карта не открыта") } else { device.clone() }), &if sound_processing { format!("{} · {:.2}% {}", crate::i18n::t("включена"), sample.sound, crate::i18n::t("ядра")) } else { crate::i18n::t("выключена") });
            kit::value_row(ui, "Обложек в памяти", "Картинки, которые сейчас на видеокарте", &covers.to_string());
            if let Some(t) = first_frame {
                kit::value_row(ui, "Первый кадр", "С запуска программы до первого нарисованного окна", &format!("{} мс", t.as_millis()));
            }
        });
        if copy {
            ui.ctx().copy_text(report);
            self.toast("Отчёт скопирован: его можно прислать разработчику");
        }
        let mut fix: Option<&str> = None;
        kit::card(ui, "Советы", "", None, |ui| {
            if ready && advice.is_empty() {
                kit::note(ui, "Всё в порядке: ничего лишнего не тратится.");
            }
            for a in &advice {
                let (tone, tag) = match a.level {
                    0 => (Tone::Warn, "важно"),
                    1 => (Tone::Info, "стоит"),
                    _ => (Tone::Off, "к сведению"),
                };
                let icon = move |ui: &mut Ui, r: Rect| {
                    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(r.left(), r.top() + 4.0), vec2(120.0, 24.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
                    kit::pill(&mut c, tone, tag);
                };
                let id = a.id;
                kit::row_full(ui, Some(&icon), a.title, &a.detail, false, false, |ui| match id {
                    "memory" => {
                        if widgets::btn(ui, None, "Включить экономию памяти", true).clicked() {
                            fix = Some("memory");
                        }
                    }
                    "overlay" => {
                        if widgets::btn(ui, None, "Выключить мини-плеер", true).clicked() {
                            fix = Some("overlay");
                        }
                    }
                    _ => {}
                });
            }
        });
        match fix {
            Some("memory") => set_settings(|s| s.memory_saver = true),
            Some("overlay") => set_settings(|s| s.overlay_enabled = false),
            _ => {}
        }
        if ready {
            kit::card(ui, "Данные", "", None, |ui| {
                kit::value_row(ui, "Библиотека", &format!("{} {}, {} {}", counts.1, crate::i18n::t("плейлистов"), crate::i18n::tracks(counts.2), crate::i18n::t("в них")), &format!("{} {}", counts.0, crate::i18n::t("лайков")));
                kit::value_row(ui, "История и очередь", &format!("{} {offline}", crate::i18n::t("Офлайн:")), &format!("{history} / {queue}"));
                for (k, b) in stored.iter().take(5) {
                    kit::value_row(ui, k, "Хранится на этом устройстве", &crate::diagnostics::mb(*b as f64));
                }
            });
        }
    }
}

/// a key combination as kbd boxes, centred in `rect`
fn paint_keys(ui: &Ui, rect: Rect, acc: &str) {
    let parts: Vec<String> = crate::app::keys::pretty(acc).split(" + ").map(str::to_string).collect();
    let gs: Vec<_> = parts.iter().map(|k| ui.painter().layout_no_wrap(k.clone(), theme::medium(12.0), Color32::PLACEHOLDER)).collect();
    let total: f32 = gs.iter().map(|g| g.size().x + 16.0).sum::<f32>() + 4.0 * (gs.len().saturating_sub(1)) as f32;
    let mut x = rect.center().x - total / 2.0;
    for g in gs {
        let r = Rect::from_min_size(pos2(x, rect.center().y - (g.size().y + 6.0) / 2.0), g.size() + vec2(16.0, 6.0));
        ui.painter().rect(r, 7, Color32::from_rgba_unmultiplied(238, 247, 244, 26), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let w = g.size().x;
        ui.painter().galley(r.center() - g.size() / 2.0, g, INK);
        x += w + 16.0 + 4.0;
    }
}

/// the flags of the language chips (src/ui/components/Flag.tsx), 18 by 13
fn paint_flag(ui: &Ui, r: Rect, code: &str) {
    let f = Rect::from_center_size(r.center(), vec2(18.0, 13.0));
    let p = ui.painter();
    let sx = f.width() / 24.0;
    let sy = f.height() / 17.0;
    let at = |x: f32, y: f32| pos2(f.left() + x * sx, f.top() + y * sy);
    match code {
        "ru" => {
            p.rect_filled(f, 2, Color32::from_rgb(0xd5, 0x2b, 0x1e));
            p.rect_filled(Rect::from_min_max(f.min, pos2(f.right(), f.top() + 11.4 * sy)), CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, Color32::from_rgb(0x1c, 0x57, 0xa5));
            p.rect_filled(Rect::from_min_max(f.min, pos2(f.right(), f.top() + 5.7 * sy)), CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, Color32::WHITE);
        }
        "gb" => {
            p.rect_filled(f, 2, Color32::from_rgb(0x1b, 0x3a, 0x7a));
            let clip = p.with_clip_rect(f.shrink(0.5));
            let white = Color32::WHITE;
            let red = Color32::from_rgb(0xcf, 0x14, 0x2b);
            for (w, c) in [(3.4, white), (1.2, red)] {
                clip.line_segment([at(0.0, 0.0), at(24.0, 17.0)], Stroke::new(w * sx, c));
                clip.line_segment([at(24.0, 0.0), at(0.0, 17.0)], Stroke::new(w * sx, c));
            }
            for (w, c) in [(5.6, white), (3.2, red)] {
                clip.line_segment([at(12.0, 0.0), at(12.0, 17.0)], Stroke::new(w * sx, c));
                clip.line_segment([at(0.0, 8.5), at(24.0, 8.5)], Stroke::new(w * sy, c));
            }
        }
        _ => {
            let c = theme::ink2();
            p.rect_stroke(f, 2, Stroke::new(1.0, c), egui::StrokeKind::Inside);
            p.circle_stroke(at(12.0, 8.5), 4.2 * sx, Stroke::new(0.9, c));
            p.line_segment([at(7.8, 8.5), at(16.2, 8.5)], Stroke::new(0.8, c));
            p.line_segment([at(12.0, 4.3), at(12.0, 12.7)], Stroke::new(0.8, c));
        }
    }
}

/// "12 КБ", "3.4 МБ" (formatBytes in the page)
fn bytes_text(bytes: u64) -> String {
    if bytes < 1024 {
        return if bytes > 0 { "< 1 КБ".into() } else { "0 КБ".into() };
    }
    if bytes < 1024 * 1024 {
        return format!("{} КБ", (bytes as f64 / 1024.0).round());
    }
    let mb = bytes as f64 / 1024.0 / 1024.0;
    if bytes < 10 * 1024 * 1024 {
        format!("{mb:.1} МБ")
    } else {
        format!("{mb:.0} МБ")
    }
}

/// "12 КБ", "3.4 МБ" (sizeOf in the page)
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
