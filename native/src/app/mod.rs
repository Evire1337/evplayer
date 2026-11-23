//! The player's window: the frame of the page (title bar, rail, mini player, backdrop), the
//! pages, the menus and dialogs. The layout follows src/App.tsx and src/styles/app.css.
//!
//! Hidden in the tray or minimized for a few seconds, the window lets go of its pages and
//! pictures (like the page's useAsleep), so their memory goes back to the system; the music
//! plays on.

pub mod actions;
pub mod keys;
mod pages;
pub mod task;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Ui};
use winit::window::Window;

use crate::model::Track;
use crate::player::{Player, Status, View};
use crate::state;
use crate::ui::images::{artwork_of, Images};
use crate::ui::theme::{self, ic, INK};
use crate::ui::widgets;
use actions::Action;
use pages::Pages;

static RENDERER: OnceLock<&'static str> = OnceLock::new();

/// what draws the window, for Настройки → Диагностика
pub fn set_renderer_name(name: &'static str) {
    let _ = RENDERER.set(name);
}

pub fn renderer_name() -> &'static str {
    RENDERER.get().copied().unwrap_or("—")
}

/// the position moves on screen this often while a song plays
const TICK_PLAYING: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Home,
    Now,
    Search,
    Library,
    /// a playlist of yours by id, or "likes"
    Playlist(String),
    Files,
    Settings,
    Artist(String),
    /// an album or playlist on a service ("scp:…", "ytb:…")
    Collection(String),
}

impl Page {
    fn name(&self) -> &'static str {
        match self {
            Page::Home => "home",
            Page::Now => "now",
            Page::Search => "search",
            Page::Library => "library",
            Page::Playlist(_) => "playlist",
            Page::Files => "files",
            Page::Settings => "settings",
            Page::Artist(_) => "artist",
            Page::Collection(_) => "collection",
        }
    }
}

/// a short message at the bottom ("Добавлено в «Утро»")
pub struct Toast {
    pub text: String,
    pub until: Instant,
}

/// the menu of a song (right click, "…")
pub struct Menu {
    pub track: Track,
    pub at: egui::Pos2,
    /// the playlist it is in (to offer "remove from the playlist")
    pub playlist_item: Option<(String, String)>,
}

pub enum Dialog {
    NewPlaylist { title: String, add: Vec<Track> },
    Rename { id: String, title: String, description: String },
    ConfirmDelete { id: String, title: String },
}

pub struct App {
    pub data_dir: PathBuf,
    pub player: Arc<Player>,
    pub library: Arc<crate::library::Library>,
    pub offline: Arc<crate::offline::Offline>,
    pub images: Images,
    pub page: Page,
    back: Vec<Page>,
    pub pages: Pages,
    pub toasts: Vec<Toast>,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    window: Option<Arc<WindowHandle>>,
    hidden_since: Option<Instant>,
    asleep: bool,
    want_hide: bool,
    want_show: bool,
    quitting: bool,
    started: Instant,
    pub first_frame: Option<Duration>,
    /// the last view of the player, taken once per frame
    pub view: View,
    pub tray: bool,
    logo: Option<egui::TextureHandle>,
    pub resolver: Arc<crate::player::resolve::Resolver>,
    pub keys: keys::Keys,
}

/// what the app keeps of the window (to drag it, minimize it, know if it is maximized)
pub struct WindowHandle {
    pub window: std::rc::Weak<Window>,
}

thread_local! {
    static WINDOW: std::cell::RefCell<Option<std::rc::Rc<Window>>> = const { std::cell::RefCell::new(None) };
}

fn with_window<R>(f: impl FnOnce(&Window) -> R) -> Option<R> {
    WINDOW.with(|w| w.borrow().as_ref().map(|w| f(w)))
}

impl App {
    pub fn new(ctx: &egui::Context, data_dir: PathBuf, started: Instant) -> Self {
        theme::apply(ctx);
        let st = state::get();
        apply_language(&st.settings());
        let library = crate::library::Library::new(data_dir.clone());
        let offline = crate::offline::Offline::new(&data_dir);
        let resolver = crate::player::resolve::Resolver::new(library.clone(), offline.clone());
        let player = Player::new(resolver.clone(), || {
            crate::events::repaint();
            crate::presence::changed();
        });
        if st.settings.read().unwrap().resume_on_start {
            if let Some(s) = st.session() {
                if !s.queue.is_empty() {
                    player.restore(s);
                }
            }
        }
        let cache = (!st.settings.read().unwrap().no_cache).then(|| cache_dir().join("covers"));
        let images = Images::new(cache, data_dir.join("local-covers"));
        let keys = keys::Keys::new();
        Self {
            data_dir,
            player,
            library,
            offline,
            images,
            page: Page::Home,
            back: Vec::new(),
            pages: Pages::default(),
            toasts: Vec::new(),
            menu: None,
            dialog: None,
            window: None,
            hidden_since: None,
            asleep: false,
            want_hide: false,
            want_show: false,
            quitting: false,
            started,
            first_frame: None,
            view: View::default(),
            tray: false,
            logo: None,
            resolver,
            keys,
        }
    }

    // ---- what the window asks of the app ---------------------------------------------------

    pub fn attach_window(&mut self, window: &std::rc::Rc<Window>) {
        WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
        self.window = Some(Arc::new(WindowHandle { window: std::rc::Rc::downgrade(window) }));
        crate::system::start();
        self.tray = true;
        self.keys.apply_global(&state::get().settings());
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            _ => 0,
        };
        crate::presence::start(self.player.clone(), hwnd);
        crate::presence::changed();
    }

    /// out of sight a while: the pictures and the pages go (the window drops its painter too)
    pub fn sleep(&mut self) {
        self.asleep = true;
        self.images.release_all();
        self.pages = Pages::default();
        self.logo = None;
        self.menu = None;
    }

    /// started by Windows at sign-in with --hidden: straight to the tray
    pub fn starts_hidden(&self) -> bool {
        std::env::args().any(|a| a == "--hidden" || a == "--minimized")
    }

    pub fn window_shown(&mut self, shown: bool) {
        if shown {
            self.hidden_since = None;
            self.asleep = false;
        } else if self.hidden_since.is_none() {
            self.hidden_since = Some(Instant::now());
        }
    }

    pub fn window_minimized(&mut self) {
        if state::get().settings.read().unwrap().minimize_to_tray {
            self.want_hide = true;
        }
        self.window_shown(false);
    }

    pub fn close_to_tray(&self) -> bool {
        !self.quitting && state::get().settings.read().unwrap().close_to_tray && self.tray
    }

    pub fn quit(&mut self) {
        self.quitting = true;
        self.player.save_session();
        self.player.shutdown();
        state::get().kv.flush();
        crate::presence::quit();
        crate::system::quit();
    }

    pub fn dropped_file(&mut self, path: PathBuf) {
        let lib = self.library.clone();
        std::thread::spawn(move || {
            let mut files = Vec::new();
            crate::library::Library::collect_audio_files(&[path], 3, &mut files);
            let _ = lib.add(&files);
            crate::events::repaint();
        });
        self.go(Page::Files);
    }

    pub fn action(&mut self, a: Action) {
        let p = self.player.clone();
        match a {
            Action::PlayPause => p.toggle(),
            Action::Next => p.next(),
            Action::Prev => p.prev(),
            Action::SeekForward => p.seek_by(5000.0),
            Action::SeekBack => p.seek_by(-5000.0),
            Action::VolumeUp => {
                let v = p.view().volume;
                p.set_volume(v + 0.05)
            }
            Action::VolumeDown => {
                let v = p.view().volume;
                p.set_volume(v - 0.05)
            }
            Action::Mute => p.toggle_mute(),
            Action::Like => {
                if let Some(t) = p.current() {
                    self.toggle_like(&t);
                }
            }
            Action::Shuffle => p.toggle_shuffle(),
            Action::Repeat => p.cycle_repeat(),
            Action::ShowWindow => self.want_show = true,
            Action::ToggleOverlay => {}
            Action::Search => {
                self.want_show = true;
                self.go(Page::Search);
                self.pages.search.focus = true;
            }
            Action::Settings => {
                self.want_show = true;
                self.go(Page::Settings);
            }
            Action::NowPlaying => {
                self.want_show = true;
                self.go(Page::Now);
            }
            Action::Quit => crate::events::send(crate::events::Event::Quit),
            Action::Noop => {}
        }
    }

    pub fn open_link(&mut self, link: &str) {
        self.toast(&format!("Ссылка: {link}"));
    }

    pub fn wants_hide(&mut self) -> bool {
        std::mem::take(&mut self.want_hide)
    }

    pub fn wants_show(&mut self) -> bool {
        std::mem::take(&mut self.want_show)
    }

    /// for --snapshot: open a page by its name
    pub fn open_page_by_name(&mut self, name: &str) {
        self.page = match name {
            "now" => Page::Now,
            "search" => Page::Search,
            "library" => Page::Library,
            "likes" => Page::Playlist("likes".into()),
            "files" => Page::Files,
            "settings" => Page::Settings,
            "sound" => {
                self.pages.sound_open = true;
                Page::Library
            }
            _ => Page::Home,
        };
    }

    // ---- helpers for the pages ----------------------------------------------------------------

    pub fn go(&mut self, page: Page) {
        if page != self.page {
            let old = std::mem::replace(&mut self.page, page);
            self.back.push(old);
            if self.back.len() > 50 {
                self.back.remove(0);
            }
        }
        self.menu = None;
    }

    pub fn go_back(&mut self) {
        match self.back.pop() {
            Some(p) => self.page = p,
            None => self.page = Page::Home,
        }
    }

    pub fn toast(&mut self, text: &str) {
        self.toasts.push(Toast { text: crate::i18n::t(text), until: Instant::now() + Duration::from_millis(3200) });
        crate::events::send(crate::events::Event::Repaint(Duration::from_millis(3300)));
    }

    pub fn is_liked(&self, id: &str) -> bool {
        state::get().library.read().unwrap().is_liked(id)
    }

    pub fn toggle_like(&mut self, t: &Track) {
        let liked = state::get().update_library(|l| l.toggle_like(t));
        self.toast(if liked { "Добавлено в «Любимое»" } else { "Убрано из «Любимого»" });
    }

    /// a song in a list was pressed: the list becomes the queue
    pub fn play_list(&mut self, tracks: &[Track], index: usize, source: Option<crate::store::state::PlaySource>) {
        self.player.play_from(tracks.to_vec(), index, source);
    }

    // ---- the frame -------------------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut Ui) {
        if self.first_frame.is_none() {
            self.first_frame = Some(self.started.elapsed());
        }
        let ctx = ui.ctx().clone();
        self.images.begin_frame(&ctx);
        self.view = self.player.view();
        keys::Keys::window_keys(&ctx, self);
        if matches!(self.view.status, Status::Playing | Status::Loading) {
            ctx.request_repaint_after(TICK_PLAYING);
        }
        let screen = ui.max_rect();
        self.backdrop(ui, screen);
        self.titlebar(ui, screen);
        self.rail(ui, screen);
        let bottom = if self.page == Page::Now { screen.bottom() - 14.0 } else { screen.bottom() - 110.0 };
        let content = Rect::from_min_max(pos2(screen.left() + 92.0, screen.top() + 40.0), pos2(screen.right() - 14.0, bottom));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(content.expand2(vec2(12.0, 0.0)));
        self.page_ui(&mut child);
        if self.page != Page::Now {
            self.mini_player(ui, Rect::from_min_max(pos2(screen.left() + 92.0, screen.bottom() - 96.0), pos2(screen.right() - 14.0, screen.bottom() - 14.0)));
        }
        self.track_menu(&ctx);
        self.dialogs(&ctx);
        self.toasts_ui(ui, screen);
        self.resize_edges(ui, screen);
    }

    fn page_ui(&mut self, ui: &mut Ui) {
        match self.page.clone() {
            Page::Home => self.home(ui),
            Page::Now => self.now_playing(ui),
            Page::Search => self.search(ui),
            Page::Library => self.library_page(ui),
            Page::Playlist(id) => self.playlist(ui, &id),
            Page::Files => self.files(ui),
            Page::Settings => self.settings(ui),
            Page::Artist(id) => self.artist(ui, &id),
            Page::Collection(id) => self.collection(ui, &id),
        }
    }

    /// the ground and a soft glow in the colour of the cover playing
    fn backdrop(&mut self, ui: &Ui, rect: Rect) {
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::ZERO, theme::GROUND);
        let tint = self.view.current().and_then(artwork_of).and_then(|s| self.images.tint(&s)).unwrap_or(Color32::from_rgb(20, 96, 86));
        let glow = |a: u8| Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), a);
        let mut mesh = egui::Mesh::default();
        let c = [glow(70), glow(22), glow(10), glow(46)];
        let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
        for (pos, col) in corners.iter().zip(c) {
            mesh.colored_vertex(*pos, col);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(egui::Shape::mesh(mesh));
    }

    fn titlebar(&mut self, ui: &mut Ui, screen: Rect) {
        let bar = Rect::from_min_max(screen.min, pos2(screen.right(), screen.top() + 40.0));
        let drag = ui.interact(bar, ui.scope_id().with("titlebar"), Sense::click_and_drag());
        if drag.drag_started() {
            with_window(|w| {
                let _ = w.drag_window();
            });
        }
        if drag.double_clicked() {
            with_window(|w| w.set_maximized(!w.is_maximized()));
        }
        // back
        if !self.back.is_empty() || self.page == Page::Now {
            let r = Rect::from_center_size(pos2(screen.left() + 126.0, bar.center().y), vec2(30.0, 30.0));
            let resp = ui.interact(r, ui.scope_id().with("back"), Sense::click());
            widgets::paint_icon_button(ui, r, &resp, ic::BACK, false);
            if resp.on_hover_text(crate::i18n::t("Назад")).clicked() {
                self.go_back();
            }
        }
        // the window's buttons
        let maximized = with_window(|w| w.is_maximized()).unwrap_or(false);
        let buttons = [(ic::MINIMIZE, "Свернуть"), (if maximized { ic::RESTORE } else { ic::MAXIMIZE }, if maximized { "Восстановить" } else { "Развернуть" }), (ic::CLOSE, "Закрыть")];
        for (i, (icon, tip)) in buttons.iter().enumerate() {
            let r = Rect::from_min_size(pos2(screen.right() - 46.0 * (3 - i) as f32, screen.top()), vec2(46.0, 32.0));
            let resp = ui.interact(r, ui.scope_id().with(("winbtn", i)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::ZERO, if i == 2 { Color32::from_rgb(196, 43, 28) } else { theme::hover() });
            }
            ui.painter().text(r.center(), Align2::CENTER_CENTER, *icon, theme::icon(10.0), INK);
            if resp.on_hover_text(crate::i18n::t(tip)).clicked() {
                match i {
                    0 => {
                        with_window(|w| w.set_minimized(true));
                    }
                    1 => {
                        with_window(|w| w.set_maximized(!maximized));
                    }
                    _ => crate::events::send(crate::events::Event::Close),
                }
            }
        }
    }

    /// the rail of icons on the left
    fn rail(&mut self, ui: &mut Ui, screen: Rect) {
        let items: [(&str, &str, Page); 6] = [
            (ic::DISC, "Сейчас играет", Page::Now),
            (ic::HOME, "Главная", Page::Home),
            (ic::SEARCH, "Поиск", Page::Search),
            (ic::LIBRARY, "Библиотека", Page::Library),
            (ic::FOLDER, "Мои файлы", Page::Files),
            (ic::SETTINGS, "Настройки", Page::Settings),
        ];
        let h = 52.0 + items.len() as f32 * 44.0 + 14.0;
        let top = (screen.center().y - h / 2.0 - 30.0).max(screen.top() + 48.0);
        let rail = Rect::from_min_size(pos2(screen.left() + 16.0, top), vec2(56.0, h));
        ui.painter().rect_filled(rail, CornerRadius::same(28), theme::glass());
        ui.painter().rect_stroke(rail, CornerRadius::same(28), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        // the logo
        let logo = self.logo.get_or_insert_with(|| {
            let img = image::load_from_memory(include_bytes!("../../../src-tauri/icons/64x64.png")).map(|i| i.into_rgba8()).unwrap_or_default();
            let (w, h) = img.dimensions();
            ui.ctx().load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw()), egui::TextureOptions::LINEAR)
        });
        let lr = Rect::from_center_size(pos2(rail.center().x, rail.top() + 28.0), vec2(28.0, 28.0));
        egui::Image::new(egui::load::SizedTexture::new(logo.id(), lr.size())).paint_at(ui, lr);
        for (i, (icon, tip, page)) in items.into_iter().enumerate() {
            let y = rail.top() + 64.0 + i as f32 * 44.0 + if i == items_len() - 1 { 10.0 } else { 0.0 };
            let r = Rect::from_center_size(pos2(rail.center().x, y), vec2(40.0, 40.0));
            let resp = ui.interact(r, ui.scope_id().with(("rail", i)), Sense::click());
            let on = self.page.name() == page.name() || (page == Page::Library && matches!(self.page, Page::Playlist(_)));
            if on {
                ui.painter().rect_filled(r, CornerRadius::same(13), theme::active());
            } else if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(13), theme::hover());
            }
            ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, theme::icon(15.0), if on { INK } else { theme::ink2() });
            if resp.on_hover_text(crate::i18n::t(tip)).clicked() {
                self.go(page);
            }
        }
    }

    /// the player at the bottom of every page but "Сейчас играет"
    fn mini_player(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(24), theme::glass_strong());
        ui.painter().rect_stroke(rect, CornerRadius::same(24), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        let v = self.view.clone();
        let cur = v.current().cloned();
        // the song
        let art = Rect::from_min_size(pos2(rect.left() + 14.0, rect.center().y - 26.0), vec2(52.0, 52.0));
        let src = cur.as_ref().and_then(artwork_of);
        widgets::paint_cover(ui, &mut self.images, src.as_deref(), art, 12);
        let open = ui.interact(art, ui.scope_id().with("mini-art"), Sense::click());
        if open.on_hover_text(crate::i18n::t("Открыть «Сейчас играет»")).clicked() {
            self.go(Page::Now);
        }
        let text_w = (rect.width() * 0.28 - 90.0).max(80.0);
        let (title, sub) = match &cur {
            Some(t) => (t.title.clone(), t.artist_line()),
            None => (crate::i18n::t("Ничего не играет"), crate::i18n::t("Найдите трек в поиске")),
        };
        let g = widgets::clipped(ui, &title, theme::medium(14.0), INK, text_w);
        ui.painter().galley(pos2(art.right() + 12.0, rect.center().y - 19.0), g, INK);
        let sub_line = if let (Some(e), Status::Error) = (&v.error, v.status) { e.clone() } else { sub };
        let sub_color = if v.status == Status::Error { theme::DANGER } else { theme::ink2() };
        let g = widgets::clipped(ui, &sub_line, theme::body(12.5), sub_color, text_w);
        ui.painter().galley(pos2(art.right() + 12.0, rect.center().y + 1.0), g, sub_color);
        if let Some(t) = &cur {
            let heart = Rect::from_center_size(pos2(art.right() + 24.0 + text_w, rect.center().y), vec2(30.0, 30.0));
            let liked = self.is_liked(&t.id);
            let r = ui.interact(heart, ui.scope_id().with("mini-like"), Sense::click());
            widgets::paint_icon_button(ui, heart, &r, if liked { ic::HEART_FILL } else { ic::HEART }, liked);
            if r.on_hover_text(crate::i18n::t(if liked { "Убрать из «Любимого»" } else { "Нравится" })).clicked() {
                let t = t.clone();
                self.toggle_like(&t);
            }
        }
        // the controls and the position
        let mid = rect.center().x;
        let center = Rect::from_center_size(pos2(mid, rect.top() + 30.0), vec2(220.0, 44.0));
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(center).layout(egui::Layout::left_to_right(egui::Align::Center)));
        cui.spacing_mut().item_spacing.x = 6.0;
        self.transport(&mut cui, 36.0, 44.0);
        let bar_w = (rect.width() * 0.42).clamp(220.0, 560.0);
        let y = rect.top() + 64.0;
        let pos = v.position_ms;
        let dur = v.duration_ms.max(1.0);
        ui.painter().text(pos2(mid - bar_w / 2.0 - 8.0, y), Align2::RIGHT_CENTER, widgets::time(pos), theme::body(11.5), theme::ink2());
        ui.painter().text(pos2(mid + bar_w / 2.0 + 8.0, y), Align2::LEFT_CENTER, widgets::time(v.duration_ms), theme::body(11.5), theme::ink2());
        let bar = Rect::from_center_size(pos2(mid, y), vec2(bar_w, 14.0));
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(bar));
        if let Some(f) = widgets::seek_bar(&mut bui, bar_w, (pos / dur) as f32, v.buffered) {
            self.player.seek(f as f64 * dur);
        }
        // volume and the full screen
        let right = Rect::from_min_max(pos2(rect.right() - 210.0, rect.center().y - 16.0), pos2(rect.right() - 14.0, rect.center().y + 16.0));
        let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if widgets::icon_button(&mut rui, ic::FULLSCREEN, 32.0, false, "Сейчас играет — во весь экран").clicked() {
            self.go(Page::Now);
        }
        let mut vol = if v.muted { 0.0 } else { v.volume };
        if widgets::level(&mut rui, 90.0, &mut vol).changed() {
            self.player.set_volume(vol);
        }
        if widgets::icon_button(&mut rui, if v.muted || v.volume == 0.0 { ic::MUTE } else { ic::VOLUME }, 32.0, false, "Без звука").clicked() {
            self.player.toggle_mute();
        }
        if widgets::icon_button(&mut rui, ic::EQ, 32.0, self.pages.sound_open, "Звук и эффекты").clicked() {
            self.pages.sound_open = !self.pages.sound_open;
        }
        if self.pages.sound_open {
            self.sound_panel(ui, Rect::from_min_size(pos2(rect.right() - 380.0, rect.top() - 420.0), vec2(366.0, 410.0)));
        }
    }

    /// shuffle, previous, play, next, repeat
    pub fn transport(&mut self, ui: &mut Ui, small: f32, big: f32) {
        let v = &self.view;
        let p = self.player.clone();
        if widgets::icon_button(ui, ic::SHUFFLE, small, v.shuffle, "Перемешать").clicked() {
            p.toggle_shuffle();
        }
        if widgets::icon_button(ui, ic::PREV, small, false, "Предыдущий").clicked() {
            p.prev();
        }
        let playing = matches!(v.status, Status::Playing | Status::Loading);
        if widgets::play_button(ui, playing, big).clicked() {
            p.toggle();
        }
        if widgets::icon_button(ui, ic::NEXT, small, false, "Следующий").clicked() {
            p.next();
        }
        let (icon, tip) = match v.repeat {
            crate::player::Repeat::One => (ic::REPEAT_ONE, "Повтор: трек"),
            crate::player::Repeat::All => (ic::REPEAT, "Повтор: вся очередь"),
            crate::player::Repeat::Off => (ic::REPEAT, "Повтор: выключен"),
        };
        if widgets::icon_button(ui, icon, small, v.repeat != crate::player::Repeat::Off, tip).clicked() {
            p.cycle_repeat();
        }
    }

    fn toasts_ui(&mut self, ui: &mut Ui, screen: Rect) {
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        let mut y = screen.bottom() - 120.0;
        for t in self.toasts.iter().rev().take(3) {
            let g = ui.painter().layout_no_wrap(t.text.clone(), theme::medium(13.0), INK);
            let r = Rect::from_center_size(pos2(screen.center().x + 40.0, y), g.size() + vec2(32.0, 18.0));
            ui.painter().rect_filled(r, CornerRadius::same(18), Color32::from_rgb(0x12, 0x3a, 0x35));
            ui.painter().galley(r.min + vec2(16.0, 9.0), g, INK);
            y -= r.height() + 8.0;
        }
    }

    /// the borderless window is resized from its edges
    fn resize_edges(&mut self, ui: &mut Ui, screen: Rect) {
        use winit::window::ResizeDirection as D;
        if with_window(|w| w.is_maximized()).unwrap_or(true) {
            return;
        }
        let m = 5.0;
        let edges = [
            (Rect::from_min_max(screen.min, pos2(screen.left() + m, screen.bottom())), D::West, egui::CursorIcon::ResizeHorizontal),
            (Rect::from_min_max(pos2(screen.right() - m, screen.top()), screen.max), D::East, egui::CursorIcon::ResizeHorizontal),
            (Rect::from_min_max(pos2(screen.left(), screen.bottom() - m), screen.max), D::South, egui::CursorIcon::ResizeVertical),
            (Rect::from_min_max(screen.min, pos2(screen.right() - 140.0, screen.top() + 3.0)), D::North, egui::CursorIcon::ResizeVertical),
            (Rect::from_min_max(pos2(screen.right() - 12.0, screen.bottom() - 12.0), screen.max), D::SouthEast, egui::CursorIcon::ResizeNwSe),
            (Rect::from_min_max(pos2(screen.left(), screen.bottom() - 12.0), pos2(screen.left() + 12.0, screen.bottom())), D::SouthWest, egui::CursorIcon::ResizeNeSw),
        ];
        for (i, (r, dir, cursor)) in edges.into_iter().enumerate() {
            let resp = ui.interact(r, ui.scope_id().with(("edge", i)), Sense::drag());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(cursor);
            }
            if resp.drag_started() {
                with_window(|w| {
                    let _ = w.drag_resize_window(dir);
                });
            }
        }
    }
}

fn items_len() -> usize {
    6
}

/// the language of the interface, from the settings
pub fn apply_language(s: &crate::store::state::Settings) {
    use crate::store::state::Language;
    let english = match s.language {
        Language::En => true,
        Language::Ru => false,
        Language::Auto => !crate::i18n::system_is_russian(),
    };
    crate::i18n::set_english(english);
}

/// %LOCALAPPDATA%\EvPlayer: what can be fetched again (covers)
pub fn cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("EvPlayer").join("native-cache")
}
