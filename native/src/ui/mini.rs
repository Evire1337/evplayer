//! The mini player (src-tauri/src/overlay.rs and src/overlay/OverlayApp.tsx): a slim bar above
//! all windows with the cover, the title (or the line being sung) and the basic buttons.
//!
//! It is a second window of the same event loop with an interface context of its own, drawn
//! by the processor: the bar is 330×52, a few thousand pixels, and only the parts that changed
//! are drawn again, so it needs no video card memory at all. It wakes about twice a second
//! while a song plays (the progress line moves a pixel at a time), never while paused.
//!
//! Games put themselves on top too and Windows lets the newest one win; "show desktop" and
//! resolution changes also hide or move small tool windows. While the mini player should be
//! visible, it checks itself every 1.5 s and comes back: shown, on a screen, on top. A game in
//! exclusive full screen is left alone. A mini player hidden for a minute is closed whole.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, ViewportId};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};

use super::images::CoverLoader;
use super::soft::SoftRenderer;
use super::theme::{self, ic, ACCENT, INK};
use crate::app::actions::Action;
use crate::app::keys::{self, Scope};
use crate::app::App;
use crate::events::{self, Event};
use crate::player::Status;
use crate::store::state::OverlayCorner;

/// logical size: a slim bar that covers nothing important in a game
pub const WIDTH: f64 = 330.0;
pub const HEIGHT: f64 = 52.0;
const MARGIN: f64 = 24.0;
const RADIUS: f64 = 16.0;
const KEEP_ON_TOP: Duration = Duration::from_millis(1500);
/// a hidden mini player is closed after a while so it costs no memory
const CLOSE_HIDDEN_AFTER: Duration = Duration::from_secs(60);
/// the bar's ground: the page's rgba(8, 18, 20, 0.94) over a dark desktop
const GROUND: [u8; 3] = [9, 19, 21];

static SHOWN: AtomicBool = AtomicBool::new(false);

/// the mini player is on screen (for Настройки → Мини-плеер)
pub fn is_shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

/// where the bar goes in a corner of the work area `(x, y, w, h)`, in pixels
pub fn corner_position(corner: OverlayCorner, area: (i32, i32, i32, i32), scale: f64) -> (i32, i32) {
    let w = (WIDTH * scale).round() as i32;
    let h = (HEIGHT * scale).round() as i32;
    let m = (MARGIN * scale).round() as i32;
    let (ax, ay, aw, ah) = area;
    use OverlayCorner::*;
    let x = match corner {
        TopCenter | BottomCenter => ax + (aw - w) / 2,
        TopLeft | BottomLeft => ax + m,
        TopRight | BottomRight => ax + aw - w - m,
    };
    let y = match corner {
        TopLeft | TopCenter | TopRight => ay + m,
        _ => ay + ah - h - m,
    };
    (x, y)
}

/// what a press in the bar asks for
enum Cmd {
    Action(Action),
    Seek(f64),
    Open,
    Drag,
}

/// what the bar shows; drawn again only when this changes
#[derive(PartialEq, Clone, Default)]
struct Look {
    track: Option<String>,
    playing: bool,
    liked: bool,
    line: Option<String>,
    /// the progress line's length in pixels
    progress_px: i32,
}

struct Win {
    window: Rc<Window>,
    ctx: egui::Context,
    state: egui_winit::State,
    /// the bar's pixels, kept between frames (only the changed parts are drawn again)
    pixels: Vec<u32>,
    layer: Layer,
    renderer: SoftRenderer,
    last_print: u64,
    dirty: bool,
    next_frame: Option<Instant>,
    cover: Option<(String, egui::TextureHandle)>,
    look: Look,
}

pub struct Mini {
    win: Option<Win>,
    /// the person wants it on screen
    wanted: bool,
    /// shown because the main window was minimized: restoring the window hides it again
    auto_shown: bool,
    /// hidden for a game in exclusive full screen; comes back when the game is left
    stepped_aside: bool,
    /// where the person dragged it; None = the corner from the settings
    moved: Option<(i32, i32)>,
    hidden_at: Option<Instant>,
    watch_at: Option<Instant>,
    /// the settings last applied, to notice a change
    corner: OverlayCorner,
    on_top: bool,
    /// a cover loaded by a thread: (its address, the picture)
    arrived: Arc<Mutex<Option<(String, Option<egui::ColorImage>)>>>,
    loading: Option<String>,
}

fn hwnd_of(w: &Window) -> Option<isize> {
    match w.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => Some(h.hwnd.get()),
        _ => None,
    }
}

impl Default for Mini {
    fn default() -> Self {
        Self::new()
    }
}

impl Mini {
    pub fn new() -> Self {
        Self { win: None, wanted: false, auto_shown: false, stepped_aside: false, moved: None, hidden_at: None, watch_at: None, corner: OverlayCorner::TopCenter, on_top: true, arrived: Arc::new(Mutex::new(None)), loading: None }
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.win.as_ref().is_some_and(|w| w.window.id() == id)
    }

    fn visible(&self) -> bool {
        self.win.as_ref().is_some_and(|w| w.window.is_visible().unwrap_or(false))
    }

    pub fn toggle(&mut self, el: &ActiveEventLoop) {
        // by what the person asked for, not by what Windows says: a game may have covered or
        // hidden the window, and then the key should still hide it, not show it again
        if self.wanted {
            self.hide()
        } else {
            self.show(el)
        }
    }

    pub fn show_for_minimize(&mut self, el: &ActiveEventLoop) {
        let s = crate::state::get().settings();
        if std::env::var_os("EVPLAYER_TRACE").is_some() {
            eprintln!("[mini] minimized: on_minimize={} visible={}", s.overlay_on_minimize, self.visible());
        }
        if !s.overlay_on_minimize || self.visible() {
            return;
        }
        self.show(el);
        self.auto_shown = self.wanted;
    }

    /// the main window came back: the mini player stood in for it, so it goes away
    pub fn hide_for_restore(&mut self) {
        if self.auto_shown || self.wanted {
            self.hide();
        }
    }

    pub fn show(&mut self, el: &ActiveEventLoop) {
        let s = crate::state::get().settings();
        if !s.overlay_enabled {
            return;
        }
        self.auto_shown = false;
        self.wanted = true;
        self.hidden_at = None;
        self.corner = s.overlay_corner;
        self.on_top = s.overlay_on_top;
        if self.win.is_none() {
            self.win = self.create(el);
        }
        let Some(w) = &self.win else {
            self.wanted = false;
            return;
        };
        let window = w.window.clone();
        self.place(&window);
        // a full-screen game is in front: wait (the check shows it once the game is left)
        if exclusive_fullscreen() {
            self.stepped_aside = true;
        } else {
            self.stepped_aside = false;
            show_inactive(&window);
        }
        self.apply_top(&window);
        if let Some(w) = self.win.as_mut() {
            w.dirty = true;
        }
        window.request_redraw();
        self.watch_at = Some(Instant::now() + KEEP_ON_TOP);
        SHOWN.store(true, Ordering::Relaxed);
        events::repaint();
    }

    pub fn hide(&mut self) {
        if std::env::var_os("EVPLAYER_TRACE").is_some() {
            eprintln!("[mini] hide (wanted {}, auto {})", self.wanted, self.auto_shown);
        }
        self.wanted = false;
        self.stepped_aside = false;
        self.auto_shown = false;
        self.watch_at = None;
        if let Some(w) = &self.win {
            hide_window(&w.window);
            self.hidden_at = Some(Instant::now());
        }
        SHOWN.store(false, Ordering::Relaxed);
        events::repaint();
    }

    fn create(&mut self, el: &ActiveEventLoop) -> Option<Win> {
        use winit::platform::windows::WindowAttributesExtWindows;
        let attrs = Window::default_attributes()
            .with_title(crate::i18n::t("EvPlayer — мини-плеер"))
            .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
            .with_resizable(false)
            .with_decorations(false)
            .with_visible(false)
            .with_active(false)
            .with_enabled_buttons(winit::window::WindowButtons::empty())
            .with_window_level(if self.on_top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal })
            .with_skip_taskbar(true);
        let window = Rc::new(el.create_window(attrs).map_err(|e| eprintln!("[mini] {e}")).ok()?);
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        ctx.set_request_repaint_callback(|info| events::send(Event::MiniRepaint(info.delay)));
        let state = egui_winit::State::new(ctx.clone(), ViewportId::ROOT, &window, Some(window.scale_factor() as f32), window.theme(), Some(2048));
        make_layered(&window);
        let [r, g, b] = GROUND;
        let renderer = SoftRenderer::new(((r as u32) << 16) | ((g as u32) << 8) | b as u32);
        Some(Win { window, ctx, state, pixels: Vec::new(), layer: Layer::default(), renderer, last_print: 0, dirty: true, next_frame: None, cover: None, look: Look::default() })
    }

    fn apply_top(&self, window: &Window) {
        window.set_window_level(if self.on_top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal });
        if self.on_top {
            force_topmost(window);
        }
    }

    fn place(&mut self, window: &Window) {
        let on_screen = |(x, y): (i32, i32)| {
            let w = (WIDTH * window.scale_factor()) as i32;
            let h = (HEIGHT * window.scale_factor()) as i32;
            window.available_monitors().any(|m| {
                let (p, s) = (m.position(), m.size());
                x >= p.x - w / 2 && y >= p.y && x + w / 2 <= p.x + s.width as i32 && y + h <= p.y + s.height as i32
            })
        };
        if let Some(p) = self.moved.filter(|p| on_screen(*p)) {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(p.0, p.1));
            return;
        }
        self.moved = None;
        let scale = window.primary_monitor().map(|m| m.scale_factor()).unwrap_or(1.0);
        let (x, y) = corner_position(self.corner, work_area(), scale);
        window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    }

    /// when the mini player wants to be woken next
    pub fn deadline(&self) -> Option<Instant> {
        let frame = self.win.as_ref().filter(|_| self.visible()).and_then(|w| w.next_frame);
        let close = self.hidden_at.map(|t| t + CLOSE_HIDDEN_AFTER);
        [frame, self.watch_at, close].into_iter().flatten().min()
    }

    /// a woken loop: the bar's next frame, the check on top, closing a long hidden bar
    pub fn tick(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        if let Some(w) = self.win.as_mut() {
            if w.next_frame.is_some_and(|t| t <= now) {
                w.next_frame = None;
                w.window.request_redraw();
            }
        }
        if self.hidden_at.is_some_and(|t| now >= t + CLOSE_HIDDEN_AFTER) {
            self.hidden_at = None;
            self.win = None;
            self.loading = None;
        }
        if self.watch_at.is_some_and(|t| t <= now) {
            self.watch_at = Some(now + KEEP_ON_TOP);
            self.watch(el);
        }
    }

    fn watch(&mut self, el: &ActiveEventLoop) {
        let s = crate::state::get().settings();
        if !s.overlay_enabled {
            self.hide();
            return;
        }
        let Some(window) = self.win.as_ref().map(|w| w.window.clone()) else {
            self.show(el);
            return;
        };
        if s.overlay_corner != self.corner {
            self.corner = s.overlay_corner;
            self.moved = None;
            self.place(&window);
        }
        if s.overlay_on_top != self.on_top {
            self.on_top = s.overlay_on_top;
            self.apply_top(&window);
        }
        // A game in exclusive full screen owns the display: no window can be drawn over it,
        // and pushing one up only makes Windows drop the game out of full screen and back,
        // again and again. Step aside until the game is left.
        if exclusive_fullscreen() {
            if !self.stepped_aside {
                self.stepped_aside = true;
                hide_window(&window);
            }
            return;
        }
        if std::mem::replace(&mut self.stepped_aside, false) || !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false) {
            show_inactive(&window);
        }
        // only when something actually sits above it: a constant push to the top makes
        // full-screen games and video players stutter
        if self.on_top && covered(&window) {
            force_topmost(&window);
        }
    }

    /// the player changed (a song, a pause, a like): draw again if the bar would look different
    pub fn changed(&mut self, app: &App) {
        if !self.visible() {
            return;
        }
        let look = look_of(app, self.win.as_ref().map(|w| w.window.scale_factor()).unwrap_or(1.0));
        if let Some(w) = self.win.as_mut() {
            if look != w.look {
                w.window.request_redraw();
            }
        }
    }

    /// egui of the bar asked for a frame
    pub fn repaint(&mut self, delay: Duration) {
        let Some(w) = self.win.as_mut() else { return };
        if delay.is_zero() {
            w.window.request_redraw();
        } else {
            let at = Instant::now() + delay;
            if w.next_frame.is_none_or(|t| at < t) {
                w.next_frame = Some(at);
            }
        }
    }

    pub fn window_event(&mut self, el: &ActiveEventLoop, event: &WindowEvent, app: &mut App) {
        let Some(w) = self.win.as_mut() else { return };
        let window = w.window.clone();
        if !matches!(event, WindowEvent::RedrawRequested) && w.state.on_window_event(&window, event).repaint {
            window.request_redraw();
        }
        match event {
            // Alt+F4 on the mini player hides it like its own key does
            WindowEvent::CloseRequested => self.hide(),
            WindowEvent::RedrawRequested => self.paint(el, app),
            WindowEvent::Moved(p) => {
                if window.is_visible().unwrap_or(false) && !self.stepped_aside {
                    self.moved = Some((p.x, p.y));
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Occluded(false) => {
                w.dirty = true;
                window.request_redraw();
            }
            _ => {}
        }
    }

    fn paint(&mut self, el: &ActiveEventLoop, app: &mut App) {
        // a cover that arrived from its thread
        if let Some((src, img)) = self.arrived.lock().unwrap().take() {
            self.loading = None;
            if let (Some(w), Some(img)) = (self.win.as_mut(), img) {
                let tex = w.ctx.load_texture("mini-cover", img, egui::TextureOptions::LINEAR);
                w.cover = Some((src, tex));
            }
        }
        let Some(w) = self.win.as_mut() else { return };
        if !w.window.is_visible().unwrap_or(false) {
            return;
        }
        let window = w.window.clone();
        let ppp = window.scale_factor();
        let look = look_of(app, ppp);
        // the cover of the song playing, loaded once by a thread
        let src = app.player.current().as_ref().and_then(super::images::artwork_of);
        let have = w.cover.as_ref().map(|c| c.0.as_str());
        if src.as_deref() != have {
            if src.is_none() {
                w.cover = None;
            } else if self.loading != src {
                self.loading = src.clone();
                let src = src.clone().unwrap_or_default();
                let loader: CoverLoader = app.images.loader();
                let arrived = self.arrived.clone();
                let px = (38.0 * ppp).ceil() as u32;
                std::thread::spawn(move || {
                    let img = loader.load(&src, px);
                    *arrived.lock().unwrap() = Some((src, img));
                    events::send(Event::MiniRepaint(Duration::ZERO));
                });
            }
        }
        let input = w.state.take_egui_input(&window);
        let cover = w.cover.as_ref().map(|c| c.1.id());
        let mut cmds = Vec::new();
        let out = w.ctx.run_ui(input, |ui| bar(ui, app, &look, cover, &mut cmds));
        w.state.handle_platform_output_with_event_loop(&window, el, out.platform_output);
        if let Some(root) = out.viewport_output.get(&ViewportId::ROOT) {
            w.next_frame = if root.repaint_delay == Duration::MAX { None } else { Some(Instant::now() + root.repaint_delay) };
        }
        // the progress line moves a pixel at a time: wake when it would
        if look.playing {
            let dur = app.player.duration_ms();
            let px = (WIDTH * ppp).max(1.0);
            let step = Duration::from_millis(((dur / px).clamp(250.0, 2000.0)) as u64);
            let step = if look.line.is_some() { step.min(Duration::from_millis(250)) } else { step };
            let at = Instant::now() + step;
            if w.next_frame.is_none_or(|t| at < t) {
                w.next_frame = Some(at);
            }
        }
        w.look = look;
        let ppp = out.pixels_per_point;
        let prims = w.ctx.tessellate(out.shapes, ppp);
        let size = window.inner_size();
        let textures_changed = !out.textures_delta.set.is_empty();
        let print = super::soft::frame_print(&prims, size.width, size.height, ppp);
        let same = !w.dirty && !textures_changed && print == w.last_print;
        w.last_print = print;
        w.renderer.set_textures(&out.textures_delta);
        if !same && size.width > 0 && size.height > 0 {
            let (pw, ph) = (size.width as usize, size.height as usize);
            let fresh = w.pixels.len() != pw * ph;
            if fresh {
                w.pixels = vec![0; pw * ph];
            }
            let dmg = w.renderer.render(&prims, ppp, &mut w.pixels, pw, ph, w.dirty || fresh);
            if !dmg.is_empty() || fresh {
                w.layer.present(&window, &w.pixels, pw, ph, (RADIUS * window.scale_factor()) as f32);
            }
        }
        w.renderer.free_textures(&out.textures_delta);
        let mut delta = out.textures_delta;
        delta.clear();
        w.dirty = false;
        for c in cmds {
            match c {
                Cmd::Action(Action::HideOverlay) | Cmd::Action(Action::ToggleOverlay) => self.hide(),
                Cmd::Action(Action::ShowWindow) | Cmd::Open => events::send(Event::ShowWindow),
                Cmd::Action(a) => app.action(a),
                Cmd::Seek(ms) => app.player.seek(ms),
                Cmd::Drag => {
                    let _ = window.drag_window();
                }
            }
        }
    }
}

fn look_of(app: &App, ppp: f64) -> Look {
    let cur = app.player.current();
    let status = app.player.status();
    let playing = status == Status::Playing;
    let pos = app.player.position_ms();
    let dur = app.player.duration_ms();
    let line = match &cur {
        Some(t) if crate::state::get().settings.read().unwrap().overlay_show_lyrics => crate::app::sung_line(t, pos - app.player.latency_ms()),
        _ => None,
    };
    let progress_px = if dur > 0.0 { ((pos / dur).clamp(0.0, 1.0) * WIDTH * ppp) as i32 } else { 0 };
    Look { liked: cur.as_ref().is_some_and(|t| app.is_liked(&t.id)), track: cur.map(|t| t.id), playing, line, progress_px }
}

/// the bar: cover, title and artist (or the line being sung), the buttons, the progress line
fn bar(ui: &mut egui::Ui, app: &App, look: &Look, cover: Option<egui::TextureId>, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let p = ui.painter().clone();
    p.rect_filled(full, CornerRadius::same(RADIUS as u8), Color32::from_rgb(GROUND[0], GROUND[1], GROUND[2]));
    p.rect_stroke(full.shrink(0.5), CornerRadius::same(RADIUS as u8), egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(238, 247, 244, 31)), egui::StrokeKind::Inside);
    let track = app.player.current();
    let has = track.is_some();

    // the cover: opens the main window
    let art = Rect::from_min_size(pos2(full.left() + 7.0, full.center().y - 19.0), vec2(38.0, 38.0));
    let r = ui.interact(art, ui.scope_id().with("art"), Sense::click());
    match cover {
        Some(id) => {
            egui::Image::new(egui::load::SizedTexture::new(id, art.size())).corner_radius(CornerRadius::same(10)).paint_at(ui, art);
        }
        None => {
            p.rect_filled(art, CornerRadius::same(10), Color32::from_rgba_unmultiplied(238, 247, 244, 20));
            crate::ui::theme::paint_icon(&p, art.center(), Align2::CENTER_CENTER, ic::NOTE, 15.0 / 1.4, theme::ink3());
        }
    }
    if r.hovered() {
        p.rect_filled(art, CornerRadius::same(10), Color32::from_black_alpha(128));
        crate::ui::theme::paint_icon(&p, art.center(), Align2::CENTER_CENTER, ic::FULLSCREEN, 13.0 / 1.4, INK);
    }
    if r.clicked() {
        cmds.push(Cmd::Open);
    }
    r.on_hover_text(crate::i18n::t("Открыть EvPlayer"));

    // the buttons at the right: like, previous, play, next
    let mut x = full.right() - 8.0;
    let mid = full.center().y;
    let mut button = |icon: &str, size: f32, main: bool, on: bool, tip: &str, action: Action| {
        let rect = Rect::from_center_size(pos2(x - size / 2.0, mid), vec2(size, size));
        x -= size + if main { 5.0 } else { 1.0 };
        let r = ui.interact(rect, ui.scope_id().with(tip), if has { Sense::click() } else { Sense::hover() });
        let alpha = if has { 1.0 } else { 0.4 };
        if main {
            p.circle_filled(rect.center(), size / 2.0, INK.gamma_multiply(alpha));
            crate::ui::theme::paint_icon(&p, rect.center(), Align2::CENTER_CENTER, icon, 15.0 / 1.4, theme::GROUND);
        } else {
            if r.hovered() && has {
                p.circle_filled(rect.center(), size / 2.0, theme::hover());
            }
            let color = if on { ACCENT } else if r.hovered() && has { INK } else { theme::ink2() };
            crate::ui::theme::paint_icon(&p, rect.center(), Align2::CENTER_CENTER, icon, 15.0 / 1.4, color.gamma_multiply(alpha));
        }
        if r.clicked() {
            cmds.push(Cmd::Action(action));
        }
        r.on_hover_text(crate::i18n::t(tip));
    };
    button(ic::NEXT, 28.0, false, false, "Следующий", Action::Next);
    button(if look.playing { ic::PAUSE } else { ic::PLAY }, 32.0, true, false, if look.playing { "Пауза" } else { "Играть" }, Action::PlayPause);
    button(ic::PREV, 28.0, false, false, "Предыдущий", Action::Prev);
    button(if look.liked { ic::HEART_FILL } else { ic::HEART }, 28.0, false, look.liked, "Нравится", Action::Like);

    // the title area moves the window (the buttons stay outside it, so their clicks land)
    let text = Rect::from_min_max(pos2(art.right() + 10.0, full.top()), pos2(x - 8.0, full.bottom() - 4.0));
    let r = ui.interact(text, ui.scope_id().with("drag"), Sense::drag());
    if r.drag_started() {
        cmds.push(Cmd::Drag);
    }
    let (title, sub) = match &track {
        Some(t) => (t.title.clone(), look.line.clone().unwrap_or_else(|| t.artist_line())),
        None => (crate::i18n::t("Ничего не играет"), crate::i18n::t("Включите трек в EvPlayer")),
    };
    let g = super::widgets::clipped(ui, &title, theme::medium(13.0), INK, text.width());
    p.galley(pos2(text.left(), mid - 16.0), g, INK);
    let g = super::widgets::clipped(ui, &sub, theme::body(11.5), theme::ink3(), text.width());
    p.galley(pos2(text.left(), mid + 1.5), g, theme::ink3());

    // the progress line along the bottom: a click jumps there
    let dur = app.player.duration_ms();
    let strip = Rect::from_min_max(pos2(full.left(), full.bottom() - 6.0), full.max);
    let r = ui.interact(strip, ui.scope_id().with("progress"), Sense::click());
    let h = if r.hovered() && has { 5.0 } else { 3.0 };
    let line = Rect::from_min_max(pos2(full.left(), full.bottom() - h), full.max);
    let p = ui.painter_at(full.shrink(0.0));
    p.rect_filled(line, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(238, 247, 244, 20));
    let ppp = ui.ctx().pixels_per_point();
    let fill = Rect::from_min_size(line.min, vec2(look.progress_px as f32 / ppp, h));
    p.rect_filled(fill, CornerRadius::ZERO, ACCENT);
    if has && dur > 0.0 {
        if let Some(at) = r.interact_pointer_pos().filter(|_| r.clicked()) {
            cmds.push(Cmd::Seek(((at.x - full.left()) / full.width()).clamp(0.0, 1.0) as f64 * dur));
        }
        let pos = app.player.position_ms();
        r.on_hover_text(format!("{} / {}", super::widgets::time(pos), super::widgets::time(dur)));
    }

    // the mini player's own keys (Настройки → Клавиши, "в мини-плеере")
    let pressed: Vec<(egui::Key, egui::Modifiers)> = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Key { key, pressed: true, modifiers, .. } = e { Some((*key, *modifiers)) } else { None }).collect());
    if !pressed.is_empty() {
        let overrides = crate::state::get().settings.read().unwrap().shortcuts.clone();
        for (key, m) in pressed {
            if let Some(a) = keys::action_for(&overrides, Scope::Overlay, key, m) {
                cmds.push(Cmd::Action(a));
            }
        }
    }
}

/// the primary screen's area without the taskbar, in pixels
fn work_area() -> (i32, i32, i32, i32) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut RECT as *mut _, 0) != 0 {
            return (r.left, r.top, r.right - r.left, r.bottom - r.top);
        }
    }
    (0, 0, 1920, 1040)
}

/// The bar is a layered window: its pixels go to Windows with their own transparency, so the
/// rounded corners are smooth (a window region would cut them pixel by pixel) and the desktop
/// shows through them.
fn make_layered(window: &Window) {
    #[cfg(windows)]
    if let Some(h) = hwnd_of(window) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED};
        unsafe {
            let ex = GetWindowLongPtrW(h as _, GWL_EXSTYLE);
            SetWindowLongPtrW(h as _, GWL_EXSTYLE, ex | WS_EX_LAYERED as isize);
        }
    }
}

/// how much of a pixel lies inside a rounded rectangle of `w`×`h` with corner radius `r`
/// (0..=255; the edge is smoothed over one pixel)
pub fn corner_coverage(x: usize, y: usize, w: usize, h: usize, r: f32) -> u8 {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let cx = px.clamp(r, w as f32 - r);
    let cy = py.clamp(r, h as f32 - r);
    let (dx, dy) = (px - cx, py - cy);
    if dx == 0.0 && dy == 0.0 {
        return 255;
    }
    let d = (dx * dx + dy * dy).sqrt();
    ((r - d + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8
}

/// the picture handed to Windows: a 32-bit bitmap with premultiplied transparency
#[derive(Default)]
struct Layer {
    #[cfg(windows)]
    dc: isize,
    #[cfg(windows)]
    bitmap: isize,
    #[cfg(windows)]
    bits: usize,
    size: (usize, usize),
    /// the corners' coverage for this size
    mask: Vec<u8>,
}

impl Layer {
    fn present(&mut self, window: &Window, pixels: &[u32], w: usize, h: usize, radius: f32) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Foundation::{POINT, SIZE};
            use windows_sys::Win32::Graphics::Gdi::*;
            use windows_sys::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};
            let Some(hwnd) = hwnd_of(window) else { return };
            if self.size != (w, h) || self.dc == 0 {
                self.release();
                let dc = CreateCompatibleDC(std::ptr::null_mut());
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = w as i32;
                // top-down rows
                info.bmiHeader.biHeight = -(h as i32);
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let mut bits = std::ptr::null_mut();
                let bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
                if bmp.is_null() || bits.is_null() {
                    DeleteDC(dc);
                    return;
                }
                SelectObject(dc, bmp as _);
                self.dc = dc as isize;
                self.bitmap = bmp as isize;
                self.bits = bits as usize;
                self.size = (w, h);
                self.mask = (0..w * h).map(|i| corner_coverage(i % w, i / w, w, h, radius)).collect();
            }
            let out = std::slice::from_raw_parts_mut(self.bits as *mut u32, w * h);
            for ((o, &p), &a) in out.iter_mut().zip(pixels).zip(&self.mask) {
                *o = if a == 255 {
                    p | 0xff00_0000
                } else {
                    let a32 = a as u32;
                    let ch = |sh: u32| (((p >> sh) & 0xff) * a32 / 255) << sh;
                    (a32 << 24) | ch(16) | ch(8) | ch(0)
                };
            }
            let size = SIZE { cx: w as i32, cy: h as i32 };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
            UpdateLayeredWindow(hwnd as _, std::ptr::null_mut(), std::ptr::null(), &size, self.dc as _, &src, 0, &blend, ULW_ALPHA);
        }
        #[cfg(not(windows))]
        let _ = (window, pixels, w, h, radius);
    }

    fn release(&mut self) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::{DeleteDC, DeleteObject};
            if self.dc != 0 {
                DeleteDC(self.dc as _);
                DeleteObject(self.bitmap as _);
            }
            self.dc = 0;
            self.bitmap = 0;
            self.bits = 0;
        }
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        self.release();
    }
}

/// show without taking the keyboard away from a game
fn show_inactive(window: &Window) {
    #[cfg(windows)]
    if let Some(h) = hwnd_of(window) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
        unsafe { ShowWindow(h as _, SW_SHOWNOACTIVATE) };
        return;
    }
    window.set_visible(true);
}

fn hide_window(window: &Window) {
    #[cfg(windows)]
    if let Some(h) = hwnd_of(window) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        unsafe { ShowWindow(h as _, SW_HIDE) };
        return;
    }
    window.set_visible(false);
}

/// a Direct3D game in exclusive full screen is in front (Windows tells "do not disturb")
fn exclusive_fullscreen() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
        let mut state = 0;
        let ok = unsafe { SHQueryUserNotificationState(&mut state) } >= 0;
        return ok && state == QUNS_RUNNING_D3D_FULL_SCREEN;
    }
    #[allow(unreachable_code)]
    false
}

/// is some other visible window above the mini player and overlapping it
fn covered(window: &Window) -> bool {
    #[cfg(windows)]
    if let Some(h) = hwnd_of(window) {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindow, GetWindowRect, IsWindowVisible, GW_HWNDPREV};
        let me = h as windows_sys::Win32::Foundation::HWND;
        let mut mine = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(me, &mut mine) };
        let mut above = unsafe { GetWindow(me, GW_HWNDPREV) };
        let mut steps = 0;
        while !above.is_null() && steps < 500 {
            steps += 1;
            if unsafe { IsWindowVisible(above) } != 0 {
                let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                unsafe { GetWindowRect(above, &mut r) };
                if r.left < mine.right && r.right > mine.left && r.top < mine.bottom && r.bottom > mine.top {
                    return true;
                }
            }
            above = unsafe { GetWindow(above, GW_HWNDPREV) };
        }
        return false;
    }
    let _ = window;
    true
}

/// put it above everything right now, also above windows that made themselves topmost later
fn force_topmost(window: &Window) {
    #[cfg(windows)]
    if let Some(h) = hwnd_of(window) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE};
        unsafe { SetWindowPos(h as _, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corners_are_smooth_and_the_middle_is_solid() {
        let (w, h, r) = (330, 52, 16.0);
        assert_eq!(corner_coverage(165, 26, w, h, r), 255);
        assert_eq!(corner_coverage(0, 0, w, h, r), 0);
        assert_eq!(corner_coverage(16, 0, w, h, r), 255);
        // somewhere along the curve a pixel is partly in
        assert!((0..16).any(|i| (1..255).contains(&corner_coverage(i, i, w, h, r))));
        assert_eq!(corner_coverage(329, 51, w, h, r), 0);
    }

    #[test]
    fn the_bar_goes_into_the_chosen_corner() {
        let area = (0, 0, 1920, 1040);
        assert_eq!(corner_position(OverlayCorner::BottomRight, area, 1.0), (1920 - 330 - 24, 1040 - 52 - 24));
        assert_eq!(corner_position(OverlayCorner::TopLeft, area, 1.0), (24, 24));
        assert_eq!(corner_position(OverlayCorner::TopCenter, area, 1.0), ((1920 - 330) / 2, 24));
        assert_eq!(corner_position(OverlayCorner::BottomCenter, area, 1.5), ((1920 - 495) / 2, 1040 - 78 - 36));
    }
}

/// the bar for `--snapshot --page mini`: drawn with no window, the cover loaded at once
pub fn snapshot_bar(ui: &mut egui::Ui, app: &App, cover: &mut Option<egui::TextureHandle>) {
    let ppp = ui.ctx().pixels_per_point() as f64;
    if cover.is_none() {
        if let Some(src) = app.player.current().as_ref().and_then(super::images::artwork_of) {
            if let Some(img) = app.images.loader().load(&src, (38.0 * ppp).ceil() as u32) {
                *cover = Some(ui.ctx().load_texture("mini-cover", img, egui::TextureOptions::LINEAR));
            }
        }
    }
    let look = look_of(app, ppp);
    let mut cmds = Vec::new();
    bar(ui, app, &look, cover.as_ref().map(|c| c.id()), &mut cmds);
}
