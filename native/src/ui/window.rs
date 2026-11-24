//! The main window: winit's event loop, egui fed from it, and a painter (the video card when
//! there is one, else the processor).
//!
//! Nothing runs while nothing happens. The loop sleeps until an input, a wake from another
//! thread (events.rs) or the moment egui asked to be drawn again; a frame whose picture did
//! not change is not drawn; a hidden or minimized window is not laid out or drawn at all.
//!
//! The video card is used only while the window is seen: the Direct3D device is made when the
//! window is shown and let go a few seconds after it is hidden (with the pictures and pages),
//! so a player living in the tray holds no video driver memory at all. Started hidden
//! (`--hidden`), it never touches the video card until it is opened.

use std::rc::Rc;
use std::time::{Duration, Instant};

use egui::ViewportId;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Icon, Window, WindowId};

use super::gpu::GpuRenderer;
use super::soft::SoftRenderer;
use crate::app::actions::Action;
use crate::app::App;
use crate::events::{self, Event};

/// frames actually drawn since the start (a frame that would look the same is skipped and not
/// counted), for Настройки → Диагностика
pub static FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// the window's colour before anything is drawn: the app's ground (#06201d)
pub const GROUND: [u8; 3] = [6, 32, 29];

enum Painter {
    Gpu(GpuRenderer),
    Soft { surface: softbuffer::Surface<Rc<Window>, Rc<Window>>, renderer: SoftRenderer, force: bool },
}

struct Shell {
    app: App,
    ctx: egui::Context,
    window: Option<Rc<Window>>,
    state: Option<egui_winit::State>,
    painter: Option<Painter>,
    info: egui::ViewportInfo,
    /// when egui wants the next frame (None: only on input)
    next_frame: Option<Instant>,
    /// what the last drawn frame looked like, to skip drawing the same picture again
    last_print: u64,
    /// the window's pixels must be drawn whole (it was resized, shown again)
    dirty: bool,
    /// out of sight since then (hidden in the tray or minimized)
    hidden_since: Option<Instant>,
    /// the window let go of its painter and pages; they come back when it is shown
    asleep: bool,
    /// the window was minimized (the mini player stands in for it then)
    minimized: bool,
    /// the window's own place and size, kept when it is hidden (a minimized window hidden in
    /// the tray loses them)
    placement: Option<Placement>,
    /// the mini player above all windows
    mini: super::mini::Mini,
    /// when the live background draws its next frame (between the interface's own frames)
    anim_at: Option<Instant>,
}

/// after this long out of sight the window lets go of the video card and its pages
fn sleep_after() -> Duration {
    if crate::state::get().settings.read().unwrap().memory_saver {
        Duration::from_secs(2)
    } else {
        Duration::from_secs(5)
    }
}

fn window_icon() -> Option<Icon> {
    let img = image::load_from_memory(include_bytes!("../../../src-tauri/icons/32x32.png")).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

impl Shell {
    fn visible(&self) -> bool {
        self.window.as_ref().map(|w| w.is_visible().unwrap_or(true) && !w.is_minimized().unwrap_or(false)).unwrap_or(false)
    }

    /// the painter for the window: the video card when there is one, else the processor
    fn make_painter(&mut self, window: &Rc<Window>) {
        let size = window.inner_size();
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => Some(h.hwnd.get()),
            _ => None,
        };
        let force_soft = std::env::var_os("EVPLAYER_SOFTWARE").is_some();
        let gpu = if force_soft { None } else { hwnd.and_then(|h| GpuRenderer::new(h, size.width, size.height)) };
        let [r, g, b] = GROUND;
        self.painter = match gpu {
            Some(mut gpu) => {
                gpu.clear = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
                Some(Painter::Gpu(gpu))
            }
            None => {
                let ctx = softbuffer::Context::new(window.clone());
                let surface = ctx.ok().and_then(|c| softbuffer::Surface::new(&c, window.clone()).ok());
                surface.map(|surface| Painter::Soft { surface, renderer: SoftRenderer::new(((r as u32) << 16) | ((g as u32) << 8) | b as u32), force: true })
            }
        };
        crate::app::set_renderer_name(match &self.painter {
            Some(Painter::Gpu(_)) => "Direct3D 11",
            Some(Painter::Soft { .. }) => "процессор",
            None => "нет",
        });
        self.dirty = true;
    }

    /// out of sight a while: the video card, the pictures and the pages go. The interface
    /// context is made anew on waking (the textures egui knew of were on the device that went).
    fn sleep(&mut self) {
        if self.asleep {
            return;
        }
        self.asleep = true;
        self.painter = None;
        self.anim_at = None;
        // the covers on the card went with it: they go up again when the window is back
        super::backdrop::forget_shown();
        self.app.sleep();
        let ctx = egui::Context::default();
        crate::ui::theme::apply(&ctx);
        ctx.set_request_repaint_callback(|info| events::send(Event::Repaint(info.delay)));
        self.ctx = ctx;
        if let Some(window) = &self.window {
            self.state = Some(egui_winit::State::new(self.ctx.clone(), ViewportId::ROOT, window, Some(window.scale_factor() as f32), window.theme(), Some(8192)));
        }
        self.last_print = 0;
    }

    fn paint(&mut self, el: &ActiveEventLoop) {
        if !self.visible() {
            return;
        }
        self.hidden_since = None;
        self.asleep = false;
        if self.painter.is_none() {
            if let Some(w) = self.window.clone() {
                self.make_painter(&w);
            }
        }
        let (Some(window), Some(state)) = (self.window.clone(), self.state.as_mut()) else { return };
        // while Windows restores a minimized window it may report the icon's tiny size for a
        // moment; laid out that small the pages would have negative room (egui stops on that)
        let sz = window.inner_size();
        if (sz.width as f64) < 400.0 * window.scale_factor() || (sz.height as f64) < 300.0 * window.scale_factor() {
            return;
        }
        let input = state.take_egui_input(&window);
        egui_winit::update_viewport_info(&mut self.info, &self.ctx, &window, false);
        let app = &mut self.app;
        let out = self.ctx.run_ui(input, |ui| app.ui(ui));
        if FRAMES.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            crate::stage("first frame laid out");
        }
        state.handle_platform_output_with_event_loop(&window, el, out.platform_output);
        let mut viewport = out.viewport_output;
        let root = viewport.remove(&ViewportId::ROOT);
        if let Some(root) = &root {
            let mut actions = Vec::new();
            egui_winit::process_viewport_commands(&self.ctx, &mut self.info, root.commands.iter().cloned(), &window, &mut actions);
            self.next_frame = if root.repaint_delay == Duration::MAX { None } else { Some(Instant::now() + root.repaint_delay) };
        }
        if std::env::var_os("EVPLAYER_TRACE").is_some() {
            eprintln!("[frame] delay={:?} causes={:?}", root.as_ref().map(|r| r.repaint_delay), self.ctx.repaint_causes());
        }
        let ppp = out.pixels_per_point;
        let prims = self.ctx.tessellate(out.shapes, ppp);
        let size = window.inner_size();
        let textures_changed = !out.textures_delta.set.is_empty();
        let print = super::soft::frame_print(&prims, size.width, size.height, ppp);
        let same = !self.dirty && !textures_changed && print == self.last_print;
        self.last_print = print;
        if !same {
            FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        match self.painter.as_mut() {
            Some(Painter::Gpu(g)) => {
                g.set_textures(&out.textures_delta);
                if !same {
                    g.resize(size.width, size.height);
                    g.render(&prims, ppp);
                }
                g.free_textures(&out.textures_delta);
                if FRAMES.load(std::sync::atomic::Ordering::Relaxed) == 1 && !same {
                    crate::stage("first frame on screen");
                }
            }
            Some(Painter::Soft { surface, renderer, force }) => {
                renderer.set_textures(&out.textures_delta);
                if !same {
                    if let (Some(w), Some(h)) = (std::num::NonZeroU32::new(size.width), std::num::NonZeroU32::new(size.height)) {
                        let _ = surface.resize(w, h);
                        if let Ok(mut buf) = surface.buffer_mut() {
                            let dmg = renderer.render(&prims, ppp, &mut buf, size.width as usize, size.height as usize, *force || self.dirty);
                            *force = false;
                            if !dmg.is_empty() {
                                let rect = softbuffer::Rect {
                                    x: dmg.x0 as u32,
                                    y: dmg.y0 as u32,
                                    width: std::num::NonZeroU32::new((dmg.x1 - dmg.x0) as u32).unwrap_or(w),
                                    height: std::num::NonZeroU32::new((dmg.y1 - dmg.y0) as u32).unwrap_or(h),
                                };
                                let _ = buf.present_with_damage(&[rect]);
                            }
                        }
                    }
                }
                renderer.free_textures(&out.textures_delta);
            }
            None => {}
        }
        self.recover_if_lost();
        self.plan_animation();
        // applied above: egui wants the delta emptied before it goes
        let mut delta = out.textures_delta;
        delta.clear();
        self.dirty = false;
    }

    /// the live background's next frame, if it moves now (or a new cover fades in)
    fn plan_animation(&mut self) {
        self.anim_at = match self.painter.as_ref() {
            Some(Painter::Gpu(g)) if g.backdrop_used() => super::backdrop::next_frame(g.backdrop_fading()).map(|d| Instant::now() + d),
            _ => None,
        };
    }

    /// a background frame: the video card draws it under the last interface frame
    fn animate(&mut self) {
        if let Some(Painter::Gpu(g)) = self.painter.as_mut() {
            g.animate();
            FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        self.recover_if_lost();
        self.plan_animation();
    }

    /// The video card was lost (an AMD, Intel or NVIDIA driver reset or update, a laptop
    /// switching cards, a remote session): the old renderer is dropped and the next frame makes
    /// a new one, on whatever card there is now, or on the processor when there is none.
    fn recover_if_lost(&mut self) {
        if !matches!(&self.painter, Some(Painter::Gpu(g)) if g.lost()) {
            return;
        }
        self.painter = None;
        self.app.gpu_lost(&self.ctx);
        self.dirty = true;
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn schedule(&mut self, el: &ActiveEventLoop) {
        let visible = self.visible();
        if !visible && self.hidden_since.is_none() {
            self.hidden_since = Some(Instant::now());
        }
        let anim = self.anim_at.filter(|_| visible);
        let main = match (self.next_frame.into_iter().chain(anim).min(), visible) {
            (Some(t), true) => Some(t),
            // out of sight: wake once more to let go of the video card, then sleep
            (_, false) if !self.asleep => Some(self.hidden_since.unwrap_or_else(Instant::now) + sleep_after()),
            _ => None,
        };
        match [main, self.mini.deadline()].into_iter().flatten().min() {
            Some(t) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }

    fn show(&mut self) {
        if std::env::var_os("EVPLAYER_TRACE").is_some() {
            eprintln!("[window] show, iconic={}", self.window.as_ref().map(|w| iconic(w)).unwrap_or(false));
        }
        self.mini.hide_for_restore();
        if let Some(w) = &self.window {
            w.set_visible(true);
            restore(w, self.placement.take());
            w.focus_window();
            self.dirty = true;
            w.request_redraw();
        }
        self.app.window_shown(true);
    }

    /// what the interface asked of the window (shown from the tray, hidden on close)
    fn after_app(&mut self) {
        if self.app.wants_hide() {
            self.hide();
        }
        if self.app.wants_show() {
            self.show();
        }
    }

    fn hide(&mut self) {
        if let Some(w) = &self.window {
            self.placement = placement_of(w);
            w.set_visible(false);
        }
        self.hidden_since = Some(Instant::now());
        self.app.window_shown(false);
    }
}

impl ApplicationHandler<Event> for Shell {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        crate::stage("event loop running");
        let attrs = Window::default_attributes()
            .with_title("EvPlayer")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(900.0, 600.0))
            .with_decorations(false)
            .with_window_icon(window_icon())
            .with_visible(false);
        let window = Rc::new(match el.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[window] {e}");
                el.exit();
                return;
            }
        });
        crate::stage("window created");
        let state = egui_winit::State::new(self.ctx.clone(), ViewportId::ROOT, &window, Some(window.scale_factor() as f32), window.theme(), Some(8192));
        self.state = Some(state);
        egui_winit::update_viewport_info(&mut self.info, &self.ctx, &window, true);
        self.app.attach_window(&window);
        let start_hidden = self.app.starts_hidden();
        self.window = Some(window.clone());
        self.dirty = true;
        if !start_hidden {
            // drawn first, then shown: no white or empty frame flashes up
            self.make_painter(&window);
            crate::stage("Direct3D ready");
            window.set_visible(true);
            crate::stage("window shown");
            self.app.window_shown(true);
        } else {
            self.hidden_since = Some(Instant::now());
            self.asleep = true;
        }
        window.request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.mini.owns(id) {
            self.mini.window_event(el, &event, &mut self.app);
            self.after_app();
            self.schedule(el);
            return;
        }
        let Some(window) = self.window.clone() else { return };
        // a redraw is not an input: passed to egui it asks for another redraw, forever
        if let (Some(state), false) = (self.state.as_mut(), matches!(event, WindowEvent::RedrawRequested)) {
            let res = state.on_window_event(&window, &event);
            if res.repaint {
                window.request_redraw();
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.app.close_to_tray() {
                    self.hide();
                } else {
                    self.app.quit();
                    el.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                self.paint(el);
            }
            WindowEvent::Resized(size) => {
                self.dirty = true;
                if size.width == 0 || size.height == 0 || window.is_minimized().unwrap_or(false) {
                    self.app.window_minimized();
                } else {
                    self.app.window_shown(true);
                }
                window.request_redraw();
            }
            WindowEvent::Occluded(false) | WindowEvent::Focused(true) | WindowEvent::ScaleFactorChanged { .. } => {
                self.dirty = true;
                window.request_redraw();
            }
            WindowEvent::DroppedFile(path) => {
                self.app.dropped_file(path);
                window.request_redraw();
            }
            WindowEvent::HoveredFile(_) | WindowEvent::HoveredFileCancelled => {
                self.app.dragging_files = matches!(event, WindowEvent::HoveredFile(_));
                window.request_redraw();
            }
            _ => {}
        }
        // minimized or back, looked at after every event: winit may still call the window
        // minimized while it reports the restoring resize
        let minimized = iconic(&window);
        if minimized != self.minimized {
            if std::env::var_os("EVPLAYER_TRACE").is_some() {
                eprintln!("[window] minimized {} -> {minimized}", self.minimized);
            }
            self.minimized = minimized;
            if minimized {
                self.mini.show_for_minimize(el);
            } else {
                self.mini.hide_for_restore();
            }
        }
        self.schedule(el);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: Event) {
        match event {
            Event::Repaint(delay) => {
                let at = Instant::now() + delay;
                if delay.is_zero() {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    // a song, a pause, a like: the mini player looks again
                    self.mini.changed(&self.app);
                } else if self.next_frame.is_none_or(|t| at < t) {
                    self.next_frame = Some(at);
                }
            }
            Event::MiniRepaint(delay) => self.mini.repaint(delay),
            Event::Action(Action::ToggleOverlay) => self.mini.toggle(el),
            Event::Action(Action::HideOverlay) => self.mini.hide(),
            Event::Action(a) => {
                self.app.action(a);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                self.mini.changed(&self.app);
            }
            Event::ShowWindow => self.show(),
            Event::OpenLink(link) => {
                self.show();
                self.app.open_link(&link);
            }
            Event::Quit => {
                self.app.quit();
                el.exit();
                return;
            }
            Event::Close => {
                if self.app.close_to_tray() {
                    self.hide();
                } else {
                    self.app.quit();
                    el.exit();
                    return;
                }
            }
        }
        self.after_app();
        self.schedule(el);
    }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let winit::event::StartCause::ResumeTimeReached { .. } = cause {
            let now = Instant::now();
            if !self.visible() {
                if self.hidden_since.is_some_and(|t| t.elapsed() >= sleep_after()) {
                    self.sleep();
                }
            } else if self.next_frame.is_some_and(|t| t <= now) {
                // the wake may have been the mini player's: the window draws only when it asked
                self.next_frame = None;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            } else if self.anim_at.is_some_and(|t| t <= now) {
                // only the background moves: the interface is not laid out again
                self.animate();
            }
            self.mini.tick(el);
        }
        let _ = el;
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.schedule(el);
    }
}

pub fn run(app: App, ctx: egui::Context, event_loop: EventLoop<Event>) {
    events::install(event_loop.create_proxy());
    // egui asks for frames from any thread (an animation, a picture that arrived)
    ctx.set_request_repaint_callback(|info| events::send(Event::Repaint(info.delay)));
    let mut shell = Shell { app, ctx, window: None, state: None, painter: None, info: Default::default(), next_frame: None, last_print: 0, dirty: true, hidden_since: None, asleep: false, minimized: false, placement: None, mini: super::mini::Mini::new(), anim_at: None };
    if let Err(e) = event_loop.run_app(&mut shell) {
        eprintln!("[window] {e}");
    }
}

fn hwnd(window: &Window) -> Option<isize> {
    match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// Minimized, as Windows has it. A minimized window hidden in the tray stops being "iconic"
/// (winit rewrites its style on hiding) but stays parked at −32000, so that counts too.
fn iconic(window: &Window) -> bool {
    #[cfg(windows)]
    if let Some(h) = hwnd(window) {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic};
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe {
            GetWindowRect(h as _, &mut r);
            return IsIconic(h as _) != 0 || r.left <= -30000;
        }
    }
    window.is_minimized().unwrap_or(false)
}

/// a window's normal rectangle and whether it comes back maximized
#[derive(Clone, Copy)]
struct Placement {
    rect: (i32, i32, i32, i32),
    maximized: bool,
}

fn placement_of(window: &Window) -> Option<Placement> {
    #[cfg(windows)]
    if let Some(h) = hwnd(window) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowPlacement, SW_SHOWMAXIMIZED, WINDOWPLACEMENT, WPF_RESTORETOMAXIMIZED};
        unsafe {
            let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
            wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
            if GetWindowPlacement(h as _, &mut wp) == 0 {
                return None;
            }
            let r = wp.rcNormalPosition;
            let maximized = wp.showCmd == SW_SHOWMAXIMIZED as u32 || (wp.flags & WPF_RESTORETOMAXIMIZED) != 0;
            return Some(Placement { rect: (r.left, r.top, r.right, r.bottom), maximized });
        }
    }
    let _ = window;
    None
}

/// back to its own place and size: a window minimized and then hidden in the tray would
/// otherwise come back at -32000, off every screen, or the size of a taskbar button
fn restore(window: &Window, saved: Option<Placement>) {
    #[cfg(windows)]
    if let Some(h) = hwnd(window) {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowPlacement, SetWindowPlacement, SW_RESTORE, SW_SHOWMAXIMIZED, WINDOWPLACEMENT};
        if iconic(window) {
            unsafe {
                let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
                wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
                if GetWindowPlacement(h as _, &mut wp) != 0 {
                    wp.showCmd = SW_RESTORE as u32;
                    if let Some(p) = saved.filter(|p| p.rect.0 > -30000 && p.rect.2 - p.rect.0 >= 400) {
                        let (left, top, right, bottom) = p.rect;
                        wp.rcNormalPosition = RECT { left, top, right, bottom };
                        if p.maximized {
                            wp.showCmd = SW_SHOWMAXIMIZED as u32;
                        }
                    }
                    SetWindowPlacement(h as _, &wp);
                }
            }
        }
        return;
    }
    let _ = saved;
    window.set_minimized(false);
}
