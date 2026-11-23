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
use crate::app::App;
use crate::events::{self, Event};

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
        let input = state.take_egui_input(&window);
        egui_winit::update_viewport_info(&mut self.info, &self.ctx, &window, false);
        let app = &mut self.app;
        let out = self.ctx.run_ui(input, |ui| app.ui(ui));
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
        match self.painter.as_mut() {
            Some(Painter::Gpu(g)) => {
                g.set_textures(&out.textures_delta);
                if !same {
                    g.resize(size.width, size.height);
                    g.render(&prims, ppp);
                }
                g.free_textures(&out.textures_delta);
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
        // applied above: egui wants the delta emptied before it goes
        let mut delta = out.textures_delta;
        delta.clear();
        self.dirty = false;
    }

    fn schedule(&mut self, el: &ActiveEventLoop) {
        let visible = self.visible();
        if !visible && self.hidden_since.is_none() {
            self.hidden_since = Some(Instant::now());
        }
        match (self.next_frame, visible) {
            (Some(t), true) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            // out of sight: wake once more to let go of the video card, then sleep
            (_, false) if !self.asleep => el.set_control_flow(ControlFlow::WaitUntil(self.hidden_since.unwrap_or_else(Instant::now) + sleep_after())),
            _ => el.set_control_flow(ControlFlow::Wait),
        }
    }

    fn show(&mut self) {
        if let Some(w) = &self.window {
            w.set_visible(true);
            w.set_minimized(false);
            w.focus_window();
            self.dirty = true;
            w.request_redraw();
        }
        self.app.window_shown(true);
    }

    fn hide(&mut self) {
        if let Some(w) = &self.window {
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
            window.set_visible(true);
            self.app.window_shown(true);
        } else {
            self.hidden_since = Some(Instant::now());
            self.asleep = true;
        }
        window.request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
            WindowEvent::DroppedFile(path) => self.app.dropped_file(path),
            _ => {}
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
                } else if self.next_frame.is_none_or(|t| at < t) {
                    self.next_frame = Some(at);
                }
            }
            Event::Action(a) => {
                self.app.action(a);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
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
        if self.app.wants_hide() {
            self.hide();
        }
        if self.app.wants_show() {
            self.show();
        }
        self.schedule(el);
    }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let winit::event::StartCause::ResumeTimeReached { .. } = cause {
            if !self.visible() {
                if self.hidden_since.is_some_and(|t| t.elapsed() >= sleep_after()) {
                    self.sleep();
                }
            } else {
                self.next_frame = None;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
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
    let mut shell = Shell { app, ctx, window: None, state: None, painter: None, info: Default::default(), next_frame: None, last_print: 0, dirty: true, hidden_since: None, asleep: false };
    if let Err(e) = event_loop.run_app(&mut shell) {
        eprintln!("[window] {e}");
    }
}
