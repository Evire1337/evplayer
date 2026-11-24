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

/// a line shows a moment before it is sung, so it is read as the singer starts it
const LEAD_MS: f64 = 120.0;
/// after scrolling the lyrics by hand, they follow the song again this much later
const HAND_SCROLL_PAUSE: std::time::Duration = std::time::Duration::from_secs(4);
/// a press of "−" or "+" moves the text this much; the most it can be moved
const OFFSET_STEP_MS: f64 = 250.0;
const MAX_OFFSET_MS: f64 = 10_000.0;
/// the shifts of this many songs are remembered
const REMEMBERED: usize = 300;
/// the shifts as src/ui/lyricsTiming.ts keeps them: per song, + shows the text earlier
const OFFSETS_KEY: &str = "evplayer.lyricsOffset.v1";

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct LyricsOffsets {
    #[serde(rename = "byTrack", default)]
    by_track: serde_json::Map<String, serde_json::Value>,
}

/// the shift of a song's lyrics, ms
fn lyrics_offset(id: &str) -> f64 {
    crate::store::kv::read_state::<LyricsOffsets>(&state::get().kv, OFFSETS_KEY).and_then(|o| o.by_track.get(id).and_then(serde_json::Value::as_f64)).unwrap_or(0.0)
}

/// "+0,25 с", "−1,5 с"
fn shift_text(ms: f64) -> String {
    let mut v = format!("{:.2}", (ms / 1000.0).abs());
    if v.ends_with('0') {
        v.pop();
    }
    let sign = if ms > 0.0 { "+" } else if ms < 0.0 { "−" } else { "" };
    let sep = if crate::i18n::is_english() { "." } else { "," };
    format!("{sign}{} {}", v.replace('.', sep), crate::i18n::t("с"))
}

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

    /// Lyrics.tsx: the lines light up as they are sung and the current one is kept in the
    /// middle; a click jumps there. Scrolled by hand, the text stays where it was put for a few
    /// seconds, then glides back to the line being sung. The text can be shifted for a slightly
    /// different version of the song.
    fn lyrics_view(&mut self, ui: &mut Ui, rect: Rect, cur: Option<&crate::model::Track>) {
        let empty = |ui: &mut Ui, text: &str| {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, crate::i18n::t(text), theme::body(15.0), theme::ink3());
        };
        let Some(track) = cur.cloned() else {
            empty(ui, "Включите трек, и здесь появится его текст.");
            return;
        };
        let key = track.id.clone();
        let t2 = track.clone();
        let found = LYRICS.with(|l| l.borrow_mut().get(&key, move || lyrics::find(&t2)).cloned());
        let Some(found) = found else {
            empty(ui, "Ищем текст…");
            return;
        };
        let Some(lyr) = found else {
            empty(ui, "Текста для этого трека пока нет.");
            return;
        };
        let (mult, saver) = {
            let s = state::get().settings.read().unwrap();
            (
                match s.lyrics_size {
                    crate::store::state::LyricsSize::S => 0.8,
                    crate::store::state::LyricsSize::M => 1.0,
                    crate::store::state::LyricsSize::L => 1.25,
                },
                s.memory_saver,
            )
        };
        let w = rect.width();
        let synced = lyr.synced;
        // the text grows with the column it gets (.now-lyrics: --lyric-scale 5.4cqi)
        let size = if synced { (w * 0.054).clamp(24.0, 64.0) } else { (w * 0.054 * 0.75).clamp(20.0, 44.0) } * mult;
        let gap = (w * 0.017).clamp(12.0, 26.0);
        let wrap = (w * 0.94).min(size * 20.0);
        let offset = lyrics_offset(&track.id);
        // the sound heard is a little behind the position; a line shows a moment before it is sung
        let pos = self.view.position_ms - self.player.latency_ms() + LEAD_MS + offset;
        let active = if synced { lyrics::active_line(&lyr.lines, pos) } else { None };

        // a turn of the wheel over the text: it stops following the song for a while
        let over = ui.rect_contains_pointer(rect);
        let wheel = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::MouseWheel { .. })));
        if over && wheel {
            self.pages.lyrics_hand = Some(std::time::Instant::now());
        }
        let by_hand = self.pages.lyrics_hand.is_some_and(|t| t.elapsed() < HAND_SCROLL_PAUSE);
        if let Some(t) = self.pages.lyrics_hand.filter(|_| by_hand) {
            ui.ctx().request_repaint_after(HAND_SCROLL_PAUSE.saturating_sub(t.elapsed()) + std::time::Duration::from_millis(20));
        }
        // where the line being sung was laid out last frame: the text glides to centre it
        let lines_id = egui::Id::unique(("lyrics-lines", &track.id));
        let laid: Option<Vec<(f32, f32)>> = ui.ctx().data(|d| d.get_temp(lines_id));
        let mut area = egui::ScrollArea::vertical().id_salt("lyrics-scroll").auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        if let (false, Some(a), Some(laid)) = (by_hand, active, laid.as_ref()) {
            if let Some((top, h)) = laid.get(a) {
                let target = (top + h / 2.0 - rect.height() / 2.0).max(0.0);
                let now = self.pages.lyrics_y;
                let next = if saver || (target - now).abs() > rect.height() * 3.0 { target } else { now + (target - now) * 0.22 };
                if (target - next).abs() > 0.5 {
                    ui.ctx().request_repaint();
                }
                area = area.vertical_scroll_offset(next);
            }
        }
        let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        let mut seek_to = None;
        let mut positions = Vec::with_capacity(lyr.lines.len());
        let out = area.show(&mut sui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let pad = rect.height() * 0.35;
            ui.add_space(pad);
            let content_top = ui.min_rect().top();
            for (i, line) in lyr.lines.iter().enumerate() {
                if i > 0 {
                    ui.add_space(gap);
                }
                let text = if line.text.trim().is_empty() && synced { "♪".to_string() } else { line.text.clone() };
                let is_active = active == Some(i);
                let fs = if is_active { size * 1.06 } else { size };
                let font = if synced { theme::heavy(fs) } else { theme::heavy(fs) };
                let mut job = egui::text::LayoutJob::single_section(text, egui::TextFormat { font_id: font, color: Color32::PLACEHOLDER, line_height: Some(fs * 1.16), ..Default::default() });
                job.wrap.max_width = if is_active { wrap * 1.06 } else { wrap };
                let g = ui.painter().layout_job(job);
                let (r, resp) = ui.allocate_exact_size(vec2(w, g.size().y), if synced { Sense::click() } else { Sense::hover() });
                positions.push((r.top() - content_top + pad, r.height()));
                // .lyric-line colours: sung white, the next ones brighter than the far ones,
                // the sung ones faint; plain text in ink
                let distance = active.map(|a| (i as i64 - a as i64).unsigned_abs()).unwrap_or(3);
                let mut alpha = if !synced {
                    1.0
                } else if is_active {
                    1.0
                } else if active.is_some_and(|a| i < a) {
                    0.28
                } else if distance == 1 {
                    0.55
                } else {
                    0.42
                };
                if synced && !is_active && resp.hovered() {
                    alpha = 0.7;
                }
                // the mask of the page: lines fade out at the top and bottom 18% of the box
                let y = r.center().y;
                let edge = rect.height() * 0.18;
                let fade = ((y - rect.top()) / edge).min((rect.bottom() - y) / edge).clamp(0.0, 1.0);
                let color = Color32::from_rgba_unmultiplied(if is_active { 255 } else { 238 }, if is_active { 255 } else { 247 }, if is_active { 255 } else { 244 }, (alpha * fade * 255.0) as u8);
                ui.painter().galley(r.min, g, color);
                if synced && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    seek_to = Some((line.time_ms - offset).max(0.0));
                }
            }
            ui.add_space(pad);
            ui.label(egui::RichText::new(format!("{} LRCLIB", crate::i18n::t("Текст:"))).font(theme::body(11.5)).color(theme::ink3()));
            ui.add_space(12.0);
        });
        self.pages.lyrics_y = out.state.clamped_offset().y;
        ui.ctx().data_mut(|d| d.insert_temp(lines_id, positions));
        if let Some(ms) = seek_to {
            self.player.seek(ms);
            self.pages.lyrics_hand = None;
        }
        if active.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        // .lyrics-shift: "−", the shift, "+" at the bottom right, on hover or once used
        if synced && (over || offset != 0.0) {
            self.lyrics_shift(ui, rect, &track.id, offset);
        }
    }

    fn lyrics_shift(&mut self, ui: &mut Ui, rect: Rect, id: &str, offset: f64) {
        let value = if offset != 0.0 { format!("{} {}", crate::i18n::t("сдвиг"), shift_text(offset)) } else { crate::i18n::t("синхронно") };
        let g = ui.painter().layout_no_wrap(value, theme::body(12.0), Color32::PLACEHOLDER);
        let w = 3.0 + 28.0 + 2.0 + g.size().x + 20.0 + 2.0 + 28.0 + 3.0;
        let pill = Rect::from_min_size(pos2(rect.right() - 4.0 - w, rect.bottom() - 10.0 - 34.0), vec2(w, 34.0));
        ui.painter().rect(pill, 17, theme::glass_strong(), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let minus = Rect::from_min_size(pill.min + vec2(3.0, 3.0), vec2(28.0, 28.0));
        let mid = Rect::from_min_size(pos2(minus.right() + 2.0, pill.top() + 3.0), vec2(g.size().x + 20.0, 28.0));
        let plus = Rect::from_min_size(pos2(mid.right() + 2.0, pill.top() + 3.0), vec2(28.0, 28.0));
        let mut nudge = 0.0;
        for (r, text, tip, d) in [(minus, "−", "Текст позже", -OFFSET_STEP_MS), (plus, "+", "Текст раньше", OFFSET_STEP_MS)] {
            let resp = ui.interact(r, ui.scope_id().with(("lyrics-shift", text)), Sense::click());
            if resp.hovered() {
                ui.painter().circle_filled(r.center(), 14.0, theme::hover());
            }
            ui.painter().text(r.center(), Align2::CENTER_CENTER, text, theme::heavy(17.0), if resp.hovered() { INK } else { theme::ink2() });
            if resp.on_hover_text(crate::i18n::t(tip)).clicked() {
                nudge = d;
            }
        }
        let resp = ui.interact(mid, ui.scope_id().with("lyrics-shift-value"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(mid, 14, theme::hover());
        }
        let color = if offset != 0.0 { ACCENT } else if resp.hovered() { INK } else { theme::ink3() };
        ui.painter().galley(mid.center() - g.size() / 2.0, g, color);
        let reset = resp.on_hover_text(crate::i18n::t(if offset != 0.0 { "Сбросить сдвиг" } else { "Текст отстаёт или спешит? Сдвиньте его" })).clicked();
        if nudge != 0.0 || reset {
            let mut o: LyricsOffsets = crate::store::kv::read_state(&state::get().kv, OFFSETS_KEY).unwrap_or_default();
            let value = if reset { 0.0 } else { (o.by_track.get(id).and_then(serde_json::Value::as_f64).unwrap_or(0.0) + nudge).clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS) };
            o.by_track.remove(id);
            while o.by_track.len() >= REMEMBERED {
                let Some(k) = o.by_track.keys().next().cloned() else { break };
                o.by_track.remove(&k);
            }
            if value != 0.0 {
                o.by_track.insert(id.to_string(), serde_json::Value::from(value));
            }
            crate::store::kv::write_state(&state::get().kv, OFFSETS_KEY, &o, 0);
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
                    self.menu = Some(crate::app::Menu { track: t.clone(), at: ui.ctx().pointer_latest_pos().unwrap_or(row.center()), playlist_item: None, queue_index: Some(*i), sub: false, files_page: false });
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
        let i = lyrics::active_line(&lyr.lines, pos_ms + LEAD_MS + lyrics_offset(&track.id))?;
        Some(lyr.lines[i].text.trim().to_string()).filter(|s| !s.is_empty())
    })
}
