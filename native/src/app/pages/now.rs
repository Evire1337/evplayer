//! Сейчас играет: the big cover and the controls, the lyrics line by line, and "Далее"
//! (the queue) at the side (src/pages/NowPlayingPage.tsx).

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Ui};

use super::Loaded;
use crate::app::App;
use crate::player::Status;
use crate::services::lyrics::{self, Lyrics};
use crate::state;
use crate::ui::images::artwork_of;
use crate::ui::theme::{self, ic, ACCENT, INK};
use crate::ui::widgets;

thread_local! {
    static LYRICS: std::cell::RefCell<Loaded<Option<Lyrics>>> = std::cell::RefCell::new(Loaded::default());
}

impl App {
    pub fn now_playing(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        let show_queue = self.pages.show_queue.unwrap_or_else(|| state::get().settings.read().unwrap().show_queue);
        let queue_w = if show_queue { 300.0 } else { 0.0 };
        let left_w = (full.width() * 0.34).clamp(300.0, 420.0);
        let left = Rect::from_min_size(full.min + vec2(28.0, 10.0), vec2(left_w, full.height() - 10.0));
        let queue = Rect::from_min_max(pos2(full.right() - queue_w, full.top() + 6.0), full.max);
        let middle = Rect::from_min_max(pos2(left.right() + 30.0, full.top()), pos2(queue.left() - 20.0, full.bottom()));
        let v = self.view.clone();
        let cur = v.current().cloned();

        // the cover and the controls
        let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(left).layout(egui::Layout::top_down(egui::Align::Min)));
        let art = (left_w - 20.0).min(full.height() - 330.0).max(160.0);
        let src = cur.as_ref().and_then(artwork_of);
        lui.add_space(16.0);
        widgets::cover(&mut lui, &mut self.images, src.as_deref(), art, 26);
        lui.add_space(18.0);
        let (title, sub) = match &cur {
            Some(t) => (t.title.clone(), t.artist_line()),
            None => (crate::i18n::t("Ничего не играет"), crate::i18n::t("Выберите песню на главной или в поиске")),
        };
        // .now-title: 28 px, 800; .now-artist: 16 px
        let g = widgets::clipped(&lui, &title, theme::heavy(28.0), INK, art);
        let (r, _) = lui.allocate_exact_size(vec2(art, 34.0), Sense::hover());
        lui.painter().galley(r.min, g, INK);
        let artist_resp = lui.add(egui::Label::new(egui::RichText::new(&sub).font(theme::body(16.0)).color(theme::ink2())).sense(Sense::click()).truncate());
        if let Some(a) = cur.as_ref().and_then(|t| t.artists.first()).filter(|a| a.id.starts_with("scu:") || a.id.starts_with("yta:")) {
            if artist_resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.go(crate::app::Page::Artist(a.id.clone()));
            }
        }
        // where the song is from, the copy playing instead, and what the sound is (.now-source:
        // "SoundCloud · играет копия с YouTube · opus 160 кбит/с")
        if v.status == Status::Error {
            if let Some(e) = &v.error {
                lui.label(egui::RichText::new(e).font(theme::body(12.5)).color(theme::DANGER));
            }
        } else if let Some(t) = &cur {
            let mut line = crate::i18n::t(match t.source {
                crate::model::Source::Soundcloud => "SoundCloud",
                crate::model::Source::Youtube => "YouTube Music",
                crate::model::Source::Local => "Мои файлы",
            });
            match v.route {
                Some(crate::model::Source::Soundcloud) => line.push_str(&format!(" · {}", crate::i18n::t("играет копия с SoundCloud"))),
                Some(crate::model::Source::Youtube) => line.push_str(&format!(" · {}", crate::i18n::t("играет копия с YouTube"))),
                _ => {}
            }
            if let Some(q) = &v.quality {
                line.push_str(&format!(" · {}", crate::i18n::t(q)));
            }
            lui.label(egui::RichText::new(line).font(theme::body(12.5)).color(theme::ink3()));
        }
        // a session on: who listens along
        lui.scope(|ui| {
            ui.set_width(art);
            self.jam_banner(ui);
        });
        lui.add_space(10.0);
        // the big progress (7 px), the times under it
        if let Some(f) = widgets::seek_bar_sized(&mut lui, art, (v.position_ms / v.duration_ms.max(1.0)) as f32, v.buffered, 7.0) {
            self.player.seek(f as f64 * v.duration_ms);
        }
        lui.horizontal(|ui| {
            ui.set_width(art);
            ui.label(egui::RichText::new(widgets::time(v.position_ms)).font(theme::body(11.5)).color(theme::ink3()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("-{}", widgets::time((v.duration_ms - v.position_ms).max(0.0)))).font(theme::body(11.5)).color(theme::ink3()));
            });
        });
        lui.add_space(10.0);
        // the transport across the column (.transport.is-big: 46, 58, 78, space between)
        let (row, _) = lui.allocate_exact_size(vec2(art, 78.0), Sense::hover());
        let used = 46.0 * 2.0 + 58.0 * 2.0 + 78.0;
        let gap = ((art - used) / 4.0).max(4.0);
        let mut tui = lui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        tui.spacing_mut().item_spacing.x = gap;
        self.transport(&mut tui, 46.0, 58.0, 78.0);
        lui.add_space(12.0);
        // .now-tools: the like, "Слушать вместе", the sound panel, the volume, "Далее"; 40 px
        // buttons 4 apart, centred under the transport
        let (row, _) = lui.allocate_exact_size(vec2(art, 40.0), Sense::hover());
        let n = if cur.is_some() { 5.0 } else { 4.0 };
        let tools_w = n * 40.0 + (n - 1.0) * 4.0;
        let tools = Rect::from_center_size(row.center(), vec2(tools_w, 40.0));
        let mut oui = lui.new_child(egui::UiBuilder::new().max_rect(tools).layout(egui::Layout::left_to_right(egui::Align::Center)));
        oui.spacing_mut().item_spacing.x = 4.0;
        if let Some(t) = &cur {
            let liked = self.is_liked(&t.id);
            if widgets::tool_button(&mut oui, if liked { ic::HEART_FILL } else { ic::HEART }, liked, if liked { "Убрать из понравившихся" } else { "Нравится" }).clicked() {
                let t = t.clone();
                self.toggle_like(&t);
            }
        }
        // the three of the player bar, in its order (it lays them out right to left)
        let three = 3.0 * 40.0 + 2.0 * 4.0;
        let (slot, _) = oui.allocate_exact_size(vec2(three, 40.0), Sense::hover());
        let mut pui = oui.new_child(egui::UiBuilder::new().max_rect(slot).layout(egui::Layout::right_to_left(egui::Align::Center)));
        pui.spacing_mut().item_spacing.x = 4.0;
        self.player_tools(&mut pui);
        if widgets::tool_button(&mut oui, ic::QUEUE, show_queue, if show_queue { "Скрыть «Далее»" } else { "Показать «Далее»" }).clicked() {
            self.pages.show_queue = Some(!show_queue);
            state::get().set_settings(|s| s.show_queue = !show_queue);
        }

        // the lyrics
        let mut mui = ui.new_child(egui::UiBuilder::new().max_rect(middle).layout(egui::Layout::top_down(egui::Align::Min)));
        self.lyrics_view(&mut mui, middle, cur.as_ref());

        // the queue
        if show_queue {
            self.queue_panel(ui, queue);
        }
    }

    fn lyrics_view(&mut self, ui: &mut Ui, rect: Rect, cur: Option<&crate::model::Track>) {
        let Some(track) = cur.cloned() else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, crate::i18n::t("Включите песню — здесь появится её текст."), theme::body(14.0), theme::ink3());
            return;
        };
        let key = track.id.clone();
        let found = LYRICS.with(|l| l.borrow_mut().get(&key, move || lyrics::find(&track)).cloned());
        let Some(found) = found else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, crate::i18n::t("Ищу текст…"), theme::body(14.0), theme::ink3());
            return;
        };
        let Some(lyr) = found else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, crate::i18n::t("Текст этой песни не нашёлся"), theme::body(14.0), theme::ink3());
            return;
        };
        let size = match state::get().settings.read().unwrap().lyrics_size {
            crate::store::state::LyricsSize::S => 20.0,
            crate::store::state::LyricsSize::M => 24.0,
            crate::store::state::LyricsSize::L => 30.0,
        };
        // the sound heard is a little behind the position: so are the lines
        let pos = self.view.position_ms - self.player.latency_ms();
        let active = if lyr.synced { lyrics::active_line(&lyr.lines, pos) } else { None };
        let line_h = size * 1.75;
        let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        let id = egui::Id::unique("lyrics-scroll");
        let mut area = egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        if let Some(a) = active {
            // the line being sung stays at a third of the height
            let target = (a as f32 * line_h - rect.height() / 3.0).max(0.0);
            area = area.vertical_scroll_offset(target);
        }
        area.show(&mut sui, |ui| {
            ui.add_space(rect.height() / 3.0);
            for (i, line) in lyr.lines.iter().enumerate() {
                let color = match active {
                    Some(a) if a == i => INK,
                    Some(a) if i < a => Color32::from_rgba_unmultiplied(238, 247, 244, 90),
                    _ if !lyr.synced => theme::ink2(),
                    _ => Color32::from_rgba_unmultiplied(238, 247, 244, 70),
                };
                let r = ui.add(egui::Label::new(egui::RichText::new(&line.text).font(theme::heavy(size)).color(color)).wrap().sense(Sense::click()));
                ui.add_space(line_h - size * 1.3);
                if lyr.synced && r.clicked() {
                    self.player.seek(line.time_ms);
                }
            }
            ui.add_space(rect.height() / 2.0);
        });
        if active.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn queue_panel(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(24), theme::glass());
        ui.painter().rect_stroke(rect, CornerRadius::same(24), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let inner = rect.shrink(14.0);
        let mut q = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        q.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("Далее")).font(theme::medium(15.0)).color(INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Label::new(egui::RichText::new(crate::i18n::t("Очистить")).font(theme::body(12.0)).color(theme::ink3())).sense(Sense::click())).clicked() {
                    self.player.clear_upcoming();
                }
            });
        });
        q.add_space(6.0);
        let v = &self.view;
        let upcoming: Vec<(usize, crate::model::Track)> = v.queue.iter().cloned().enumerate().skip(v.index + 1).collect();
        if upcoming.is_empty() {
            q.label(egui::RichText::new(crate::i18n::t("Очередь пуста — после этой песни заиграют похожие.")).font(theme::body(12.5)).color(theme::ink2()));
            return;
        }
        let w = inner.width();
        egui::ScrollArea::vertical().id_salt("queue").auto_shrink([false, false]).show_rows(&mut q, 50.0, upcoming.len(), |ui, range| {
            for (i, t) in &upcoming[range] {
                let (row, r) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
                if r.hovered() {
                    ui.painter().rect_filled(row, 10, theme::hover());
                }
                let art = Rect::from_min_size(row.min + vec2(4.0, 5.0), vec2(36.0, 36.0));
                let src = artwork_of(t);
                widgets::paint_cover(ui, &mut self.images, src.as_deref(), art, 8);
                let tw = w - 90.0;
                let g = widgets::clipped(ui, &t.title, theme::medium(12.5), INK, tw);
                ui.painter().galley(pos2(art.right() + 10.0, row.top() + 7.0), g, INK);
                let g = widgets::clipped(ui, &t.artist_line(), theme::body(11.5), theme::ink2(), tw);
                ui.painter().galley(pos2(art.right() + 10.0, row.top() + 25.0), g, theme::ink2());
                let x = Rect::from_center_size(pos2(row.right() - 16.0, row.center().y), vec2(24.0, 24.0));
                let xr = ui.interact(x, r.id.with("rm"), Sense::click());
                if r.hovered() || xr.hovered() {
                    crate::ui::theme::paint_icon(&ui.painter(), x.center(), Align2::CENTER_CENTER, ic::CANCEL, 10.0, if xr.hovered() { ACCENT } else { theme::ink2() });
                }
                if xr.clicked() {
                    self.player.remove(*i);
                } else if r.double_clicked() || r.clicked() {
                    self.player.play_index(*i);
                }
                if r.secondary_clicked() {
                    self.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(row.center()), playlist_item: None });
                }
            }
        });
    }
}

/// The line being sung now, for the mini player's "Строка текста песни". The lyrics are looked
/// up once per song and shared with this page.
pub fn sung_line(track: &crate::model::Track, pos_ms: f64) -> Option<String> {
    let t = track.clone();
    LYRICS.with(|l| {
        let mut l = l.borrow_mut();
        let lyr = l.get(&track.id, move || lyrics::find(&t))?.as_ref()?;
        if !lyr.synced {
            return None;
        }
        let i = lyrics::active_line(&lyr.lines, pos_ms)?;
        Some(lyr.lines[i].text.trim().to_string()).filter(|s| !s.is_empty())
    })
}
