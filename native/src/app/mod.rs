//! The player's window: the frame of the page (title bar, rail, mini player, backdrop), the
//! pages, the menus and dialogs. The layout follows src/App.tsx and src/styles/app.css.
//!
//! Hidden in the tray or minimized for a few seconds, the window lets go of its pages and
//! pictures (like the page's useAsleep), so their memory goes back to the system; the music
//! plays on.

pub mod actions;
pub mod keys;
mod pages;
pub use pages::now::sung_line;
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

/// toasts from other threads (a copy-out that finished), shown on the next frame
static LATER: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

impl App {
    /// "Вышла версия X" with a restart button (UpdateToast.tsx); nothing until there is one
    fn update_toast(&mut self, ui: &mut Ui, screen: Rect) {
        let crate::updates::Status::Ready { version } = crate::updates::status() else { return };
        if self.update_dismissed {
            return;
        }
        let w = 520.0_f32.min(screen.width() - 32.0);
        let rect = Rect::from_min_size(egui::pos2(screen.center().x - w / 2.0, screen.bottom() - 190.0), egui::vec2(w, 52.0));
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
        egui::Frame::new().fill(egui::Color32::from_rgba_unmultiplied(8, 22, 24, 245)).corner_radius(16).inner_margin(egui::Margin::symmetric(14, 8)).show(&mut cui, |ui| {
            ui.set_width(w - 28.0);
            ui.label(egui::RichText::new(format!("{} {version}. {}", crate::i18n::t("Вышла версия"), crate::i18n::t("Она установится при перезапуске."))).color(INK));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::pill_button(ui, None, "Позже", false).clicked() {
                    self.update_dismissed = true;
                }
                if widgets::pill_button(ui, None, "Перезапустить", true).clicked() {
                    if let Err(e) = crate::updates::install() {
                        self.toast(&e);
                    }
                }
            });
        });
    }
}

pub fn toast_later(text: impl Into<String>) {
    LATER.lock().unwrap().push(text.into());
    crate::events::repaint();
}

pub enum Dialog {
    NewPlaylist { title: String, add: Vec<Track> },
    Rename { id: String, title: String, description: String },
    ConfirmDelete { id: String, title: String },
    /// "Импорт по ссылке или коду": the text pasted, and whether it goes to the likes
    Import { text: String, likes: bool },
    /// "Скачать для офлайн": the lists ticked, and whether they keep themselves complete
    Download { picked: Vec<String>, follow: bool },
    /// "Восстановить из копии?"
    Restore(crate::backup::BackupFile),
    /// "Сбросить статистику?"
    ResetStats,
    /// "Включить полную синхронизацию?" with SoundCloud
    FullSync,
    /// Spotify's data archive: the lists found in it and which are ticked
    SpotifyExport { lists: Vec<crate::spotify_export::List>, chosen: Vec<bool>, error: String },
}

/// an import running (or just finished): its panel at the bottom
pub struct Importing {
    pub cancel: Arc<std::sync::atomic::AtomicBool>,
    pub progress: Arc<std::sync::Mutex<crate::import::Progress>>,
    pub finished_at: Option<Instant>,
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
    pub importing: Option<Importing>,
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
    /// "Позже" on the new version's toast
    pub update_dismissed: bool,
    logo: Option<egui::TextureHandle>,
    pub resolver: Arc<crate::player::resolve::Resolver>,
    pub downloads: Arc<crate::downloads::Downloads>,
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
            offline: offline.clone(),
            images,
            page: Page::Home,
            back: Vec::new(),
            pages: Pages::default(),
            toasts: Vec::new(),
            menu: None,
            dialog: None,
            importing: None,
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
            update_dismissed: false,
            logo: None,
            downloads: {
                let d = crate::downloads::Downloads::new(offline.clone(), resolver.clone());
                // a picture of a page (--snapshot) fetches nothing
                if !std::env::args().any(|a| a == "--snapshot") {
                    crate::keep::start(d.clone());
                    crate::updates::start();
                    crate::services::scaccount::start();
                }
                d
            },
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
            // the mini player is the event loop's: it goes there
            Action::ToggleOverlay | Action::HideOverlay => crate::events::send(crate::events::Event::Action(a)),
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
        // "settings:8" opens that tab of the settings
        if let Some(tab) = name.strip_prefix("settings:").and_then(|t| t.parse().ok()) {
            self.pages.settings_tab = tab;
            // the picture of the diagnostics tab shows it without turning it on for good
            if tab == 9 {
                state::get().settings.write().unwrap().dev_tools = true;
            }
            self.page = Page::Settings;
            return;
        }
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

    /// read a link, a code or a list of songs into a playlist (or the likes)
    pub fn start_import(&mut self, text: String, to_likes: Option<bool>) {
        if let Some(i) = &self.importing {
            i.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let progress = Arc::new(std::sync::Mutex::new(crate::import::Progress::default()));
        let (c, p) = (cancel.clone(), progress.clone());
        std::thread::spawn(move || crate::import::run(text, to_likes, c, p));
        self.importing = Some(Importing { cancel, progress, finished_at: None });
    }

    /// the import's panel (the page's ImportPill): what it does now, cancel; the result a moment
    fn import_pill(&mut self, ui: &mut Ui, screen: Rect) {
        let Some(imp) = &mut self.importing else { return };
        let p = imp.progress.lock().unwrap().clone();
        if p.done && imp.finished_at.is_none() {
            imp.finished_at = Some(Instant::now());
            if p.ok {
                match p.opened.as_deref() {
                    Some("library") => self.page = Page::Library,
                    Some(id) => self.page = Page::Playlist(id.to_string()),
                    None => {}
                }
            }
        }
        let Some(imp) = &mut self.importing else { return };
        if imp.finished_at.is_some_and(|t| t.elapsed() > Duration::from_secs(6)) {
            self.importing = None;
            return;
        }
        if imp.finished_at.is_some() {
            ui.ctx().request_repaint_after(Duration::from_secs(6));
        }
        let text = if p.text.is_empty() { crate::i18n::t("Читаю…") } else { p.text.clone() };
        let w = 420.0f32.min(screen.width() - 140.0);
        let g = widgets::clipped(ui, &text, theme::medium(13.0), INK, w - 60.0);
        let r = Rect::from_min_size(pos2(screen.right() - w - 24.0, screen.top() + 48.0), vec2(w, 46.0));
        ui.painter().rect_filled(r, CornerRadius::same(20), Color32::from_rgba_unmultiplied(6, 18, 20, 240));
        let color = if p.done && !p.ok { theme::DANGER } else { INK };
        ui.painter().galley(pos2(r.left() + 16.0, r.center().y - g.size().y / 2.0), g, color);
        let x = Rect::from_center_size(pos2(r.right() - 24.0, r.center().y), vec2(28.0, 28.0));
        let xr = ui.interact(x, ui.scope_id().with("import-x"), Sense::click());
        widgets::paint_icon_button(ui, x, &xr, ic::CANCEL, false);
        if xr.on_hover_text(crate::i18n::t(if p.done { "Закрыть" } else { "Отменить" })).clicked() {
            if let Some(imp) = &self.importing {
                imp.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            self.importing = None;
        }
    }

    /// "Скачиваю: N" while songs are saved for offline, with a stop
    fn download_pill(&mut self, ui: &mut Ui, screen: Rect) {
        let st = self.downloads.status();
        let busy = st.running.len() + st.waiting;
        if busy == 0 {
            return;
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        let (done, total) = st.running.values().fold((0u64, 0u64), |a, b| (a.0 + b.0, a.1 + b.1));
        let pct = if total > 0 { format!(" · {}%", done * 100 / total) } else { String::new() };
        let text = format!("{}: {busy}{pct}", crate::i18n::t("Скачиваю для офлайн"));
        let w = 300.0;
        let top = if self.importing.is_some() { screen.top() + 100.0 } else { screen.top() + 48.0 };
        let r = Rect::from_min_size(pos2(screen.right() - w - 24.0, top), vec2(w, 42.0));
        ui.painter().rect_filled(r, CornerRadius::same(20), Color32::from_rgba_unmultiplied(6, 18, 20, 240));
        crate::ui::theme::paint_icon(&ui.painter(), pos2(r.left() + 16.0, r.center().y), Align2::LEFT_CENTER, ic::DOWNLOAD, 13.0, theme::ACCENT);
        ui.painter().text(pos2(r.left() + 38.0, r.center().y), Align2::LEFT_CENTER, text, theme::medium(13.0), INK);
        let x = Rect::from_center_size(pos2(r.right() - 22.0, r.center().y), vec2(28.0, 28.0));
        let xr = ui.interact(x, ui.scope_id().with("dl-x"), Sense::click());
        widgets::paint_icon_button(ui, x, &xr, ic::CANCEL, false);
        if xr.on_hover_text(crate::i18n::t("Остановить")).clicked() {
            self.downloads.cancel_all();
        }
    }

    /// queue songs for offline and say how many
    pub fn download(&mut self, tracks: &[Track]) {
        let n = self.downloads.add(tracks);
        self.toast(&if n == 0 { "Уже скачано".to_string() } else { format!("Скачать для офлайн: {}", crate::i18n::tracks(n)) });
    }

    /// the songs saved for offline that the app knows the names of (likes, playlists, history)
    pub fn saved_tracks(&self) -> Vec<Track> {
        let saved: std::collections::HashSet<String> = self.downloads.saved().into_iter().map(|e| e.id).collect();
        let lib = state::get().library.read().unwrap();
        let hist = state::get().history.lock().unwrap();
        let mut seen = std::collections::HashSet::new();
        lib.likes.iter().map(|l| &l.track)
            .chain(lib.playlists.iter().flat_map(|p| p.items.iter().map(|i| &i.track)))
            .chain(hist.entries.iter().map(|e| &e.track))
            .chain(self.view.queue.iter())
            .filter(|t| saved.contains(&t.id) && seen.insert(t.id.clone()))
            .cloned()
            .collect()
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
        let bottom = if self.page == Page::Now { screen.bottom() - 14.0 } else { screen.bottom() - 118.0 };
        let content = Rect::from_min_max(pos2(screen.left() + 92.0, screen.top() + 40.0), pos2(screen.right() - 14.0, bottom));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(content.expand2(vec2(12.0, 0.0)));
        self.page_ui(&mut child);
        if self.page != Page::Now {
            self.mini_player(ui, Rect::from_min_max(pos2(screen.left() + 92.0, screen.bottom() - 104.0), pos2(screen.right() - 20.0, screen.bottom() - 20.0)));
        }
        self.track_menu(&ctx);
        self.dialogs(&ctx);
        self.import_pill(ui, screen);
        self.download_pill(ui, screen);
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

    /// The backdrop (src/ui/backdrop): the cover playing, blurred and rich, filling the window,
    /// with the page's vignette and shade; a still picture, so it costs nothing at rest. With
    /// nothing playing, the page's teal glow.
    fn backdrop(&mut self, ui: &Ui, rect: Rect) {
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::ZERO, theme::GROUND);
        // the video card draws the live background with the page's own shader (ui/backdrop.rs);
        // the processor (no video card) shows the same picture held still
        if renderer_name() == "Direct3D 11" {
            let (animated, saver) = {
                let s = state::get().settings.read().unwrap();
                (s.animated_backdrop, s.memory_saver)
            };
            let src = self.view.current().and_then(artwork_of);
            let playing = self.view.status == Status::Playing;
            let images = &self.images;
            crate::ui::backdrop::want(src, playing, animated && !saver, || images.loader());
            p.image(crate::ui::backdrop::TEXTURE, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            // .backdrop-shade: a little darker toward the bottom
            p.add(grid_mesh(rect, 2, |_, v| Color32::from_rgba_unmultiplied(3, 10, 12, ((0.15 + 0.30 * v) * 255.0) as u8)));
            return;
        }
        let tex = self.view.current().and_then(artwork_of).and_then(|s| self.images.backdrop(&s));
        match tex {
            Some(id) => {
                // as the shader mapped it: the width limited to an aspect of 1.8, zoomed to 82 %
                let aspect = rect.width() / rect.height().max(1.0);
                let half_u = 0.5 * 0.82 * aspect.min(1.8) / 1.8;
                let half_v = 0.5 * 0.82;
                let uv = Rect::from_min_max(pos2(0.5 - half_u, 0.5 - half_v), pos2(0.5 + half_u, 0.5 + half_v));
                p.image(id, rect, uv, Color32::WHITE);
                // the vignette: dimmer toward the edges, from the point a little above the middle
                p.add(grid_mesh(rect, 10, |u, v| {
                    let d = ((u - 0.5).powi(2) + (v - 0.45).powi(2)).sqrt();
                    let t = ((1.15 - d) / (1.15 - 0.25)).clamp(0.0, 1.0);
                    let vig = t * t * (3.0 - 2.0 * t);
                    let keep = 0.55 + 0.45 * vig;
                    Color32::from_black_alpha(((1.0 - keep) * 255.0) as u8)
                }));
            }
            None => {
                // radial-gradient(circle at 25% 30%, #1f8a73, ground 65%)
                let (w, h) = (rect.width(), rect.height());
                let far = (w * w + h * h).sqrt() * 0.75;
                p.add(grid_mesh(rect, 16, |u, v| {
                    let d = ((u - 0.25) * w).hypot((v - 0.3) * h) / far;
                    let t = (d / 0.65).clamp(0.0, 1.0);
                    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
                    Color32::from_rgb(mix(0x1f, 0x06), mix(0x8a, 0x20), mix(0x73, 0x1d))
                }));
            }
        }
        // .backdrop-shade: a little darker toward the bottom
        p.add(grid_mesh(rect, 2, |_, v| Color32::from_rgba_unmultiplied(3, 10, 12, ((0.15 + 0.30 * v) * 255.0) as u8)));
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
            crate::ui::theme::paint_icon(&ui.painter(), r.center(), Align2::CENTER_CENTER, *icon, 10.0, INK);
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
        // .rail: padding 10 × 8, a gap of 6 between its children, 50 px buttons rounded 19, the
        // logo 32 px with 4 above and 10 below it, 10 more before "Настройки"; centred on the window
        const BTN: f32 = 50.0;
        const GAP: f32 = 6.0;
        let logo_block = 4.0 + 32.0 + 10.0;
        let h = 10.0 + logo_block + items.len() as f32 * (BTN + GAP) + (10.0 + GAP) + 10.0;
        let top = (screen.center().y - h / 2.0).max(screen.top() + 44.0);
        let rail = Rect::from_min_size(pos2(screen.left() + 20.0, top), vec2(BTN + 16.0, h));
        ui.painter().rect_filled(rail, CornerRadius::same(28), theme::glass());
        ui.painter().rect_stroke(rail, CornerRadius::same(28), egui::Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
        // the logo: the page's own snail (src/logo.svg), white at 90 %
        let lr = Rect::from_min_size(pos2(rail.center().x - 16.0, rail.top() + 10.0 + 4.0), vec2(32.0, 32.0));
        let lw = 32.0;
        let lh = lw * crate::ui::icon_data::LOGO_VIEW.1 / crate::ui::icon_data::LOGO_VIEW.0;
        crate::ui::icons::paint(ui.painter(), "logo", Rect::from_center_size(lr.center(), vec2(lw, lh)), Color32::from_white_alpha(230));
        let mut y = rail.top() + 10.0 + logo_block + GAP;
        let last = items.len() - 1;
        for (i, (icon, tip, page)) in items.into_iter().enumerate() {
            if i == last {
                // .rail-gap before the settings
                y += 10.0 + GAP;
            }
            let r = Rect::from_min_size(pos2(rail.center().x - BTN / 2.0, y), vec2(BTN, BTN));
            y += BTN + GAP;
            let resp = ui.interact(r, ui.scope_id().with(("rail", i)), Sense::click());
            let on = self.page.name() == page.name() || (page == Page::Library && matches!(self.page, Page::Playlist(_)));
            if on {
                ui.painter().rect_filled(r, CornerRadius::same(19), theme::active());
            } else if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(19), theme::hover());
            }
            let color = if on || resp.hovered() { INK } else { theme::ink2() };
            let name = icon.trim_start_matches('@');
            crate::ui::icons::paint(ui.painter(), name, Rect::from_center_size(r.center(), vec2(21.0, 21.0)), color);
            // .rail-tip: to the right of the button
            if resp.hovered() {
                let text = crate::i18n::t(tip);
                let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("rail-tip")));
                let g = painter.layout_no_wrap(text, theme::medium(12.5), INK);
                let tr = Rect::from_min_size(pos2(r.right() + 12.0 + 8.0, r.center().y - g.size().y / 2.0 - 6.0), g.size() + vec2(20.0, 12.0));
                painter.rect_filled(tr, CornerRadius::same(10), theme::glass_strong());
                painter.galley(tr.min + vec2(10.0, 6.0), g, INK);
            }
            if resp.clicked() {
                self.go(page);
            }
        }
    }

    /// the player at the bottom of every page but "Сейчас играет" (.mini-player: 84 px high,
    /// rounded 28, the song, the controls and the tools in columns of 1 : 1.3 : 1)
    fn mini_player(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(28), theme::glass_strong());
        let v = self.view.clone();
        let cur = v.current().cloned();
        let inner = Rect::from_min_max(pos2(rect.left() + 12.0, rect.top()), pos2(rect.right() - 14.0, rect.bottom()));
        let unit = (inner.width() - 32.0) / 3.3;
        let left = Rect::from_min_size(inner.min, vec2(unit, inner.height()));
        let center = Rect::from_min_size(pos2(left.right() + 16.0, inner.top()), vec2(unit * 1.3, inner.height()));
        let right = Rect::from_min_max(pos2(center.right() + 16.0, inner.top()), inner.max);

        // the song: cover, title and artist, like
        let art = Rect::from_min_size(pos2(left.left(), left.center().y - 30.0), vec2(60.0, 60.0));
        let src = cur.as_ref().and_then(artwork_of);
        widgets::paint_cover(ui, &mut self.images, src.as_deref(), art, 16);
        let open = ui.interact(art, ui.scope_id().with("mini-art"), Sense::click());
        if open.on_hover_text(crate::i18n::t("Открыть «Сейчас играет»")).clicked() {
            self.go(Page::Now);
        }
        let text_w = (left.right() - art.right() - 12.0 - 46.0).max(60.0);
        let (title, sub) = match &cur {
            Some(t) => (t.title.clone(), t.artist_line()),
            None => (crate::i18n::t("Ничего не играет"), crate::i18n::t("Найдите трек в поиске")),
        };
        let g = widgets::clipped(ui, &title, theme::medium(14.0), INK, text_w);
        let title_w = g.size().x;
        ui.painter().galley(pos2(art.right() + 12.0, left.center().y - 20.0), g, INK);
        let sub_line = if let (Some(e), Status::Error) = (&v.error, v.status) { e.clone() } else { sub };
        let sub_color = if v.status == Status::Error { theme::DANGER } else { theme::ink2() };
        let g = widgets::clipped(ui, &sub_line, theme::body(13.0), sub_color, text_w);
        let sub_w = g.size().x;
        ui.painter().galley(pos2(art.right() + 12.0, left.center().y + 3.0), g, sub_color);
        if let Some(t) = &cur {
            let x = art.right() + 12.0 + title_w.max(sub_w) + 6.0;
            let heart = Rect::from_min_size(pos2(x, left.center().y - 20.0), vec2(40.0, 40.0));
            let liked = self.is_liked(&t.id);
            let mut hui = ui.new_child(egui::UiBuilder::new().max_rect(heart));
            if widgets::tool_button(&mut hui, if liked { ic::HEART_FILL } else { ic::HEART }, false, if liked { "Убрать из «Любимого»" } else { "Нравится" }).clicked() {
                let t = t.clone();
                self.toggle_like(&t);
            }
            if liked {
                crate::ui::theme::paint_icon(&ui.painter(), heart.center(), Align2::CENTER_CENTER, ic::HEART_FILL, 15.0, theme::ACCENT);
            }
        }

        // the controls, and under them the position with the times above the bar
        let controls = Rect::from_center_size(pos2(center.center().x, center.top() + 30.0), vec2(40.0 * 2.0 + 36.0 * 2.0 + 42.0 + 40.0, 42.0));
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(controls).layout(egui::Layout::left_to_right(egui::Align::Center)));
        cui.spacing_mut().item_spacing.x = 10.0;
        self.transport(&mut cui, 40.0, 36.0, 42.0);
        let bar_w = center.width().min(560.0);
        let bar_left = center.center().x - bar_w / 2.0;
        let pos = v.position_ms;
        let dur = v.duration_ms.max(1.0);
        let times_y = center.top() + 58.0;
        ui.painter().text(pos2(bar_left + 2.0, times_y), Align2::LEFT_CENTER, widgets::time(pos), theme::body(11.0), theme::ink3());
        ui.painter().text(pos2(bar_left + bar_w - 2.0, times_y), Align2::RIGHT_CENTER, widgets::time(v.duration_ms), theme::body(11.0), theme::ink3());
        let bar = Rect::from_min_size(pos2(bar_left, center.top() + 62.0), vec2(bar_w, 16.0));
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(bar));
        if let Some(f) = widgets::seek_bar(&mut bui, bar_w, (pos / dur) as f32, v.buffered) {
            self.player.seek(f as f64 * dur);
        }

        // the tools: sound and effects, the volume, the full screen
        let tools = Rect::from_min_max(pos2(right.left(), right.center().y - 20.0), pos2(right.right(), right.center().y + 20.0));
        let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(tools).layout(egui::Layout::right_to_left(egui::Align::Center)));
        rui.spacing_mut().item_spacing.x = 4.0;
        if widgets::tool_button(&mut rui, ic::FULLSCREEN, false, "Сейчас играет — во весь экран").clicked() {
            self.go(Page::Now);
        }
        self.player_tools(&mut rui);
    }

    /// the sound panel's and the volume's buttons, each opening its panel above it
    pub fn player_tools(&mut self, ui: &mut Ui) {
        let v = self.view.clone();
        let vol_icon = if v.muted || v.volume == 0.0 { ic::MUTE } else { ic::VOLUME };
        let vr = widgets::tool_button(ui, vol_icon, self.pages.volume_open, if self.pages.volume_open { "" } else { "Громкость" });
        if vr.clicked() {
            self.pages.volume_open = !self.pages.volume_open;
        }
        let fx = state::get().effects.read().unwrap().clone();
        let tuned = !fx.is_neutral();
        let sr = widgets::tool_button(ui, ic::SLIDERS, self.pages.sound_open || tuned, if self.pages.sound_open { "" } else { "Звук, скорость и тон" });
        if sr.clicked() {
            self.pages.sound_open = !self.pages.sound_open;
        }
        let mut open = self.pages.sound_open;
        let mut changed = false;
        widgets::popover(ui, "sound-panel", sr.rect, 292.0, &mut open, |ui| {
            changed = sound_panel(ui);
        });
        self.pages.sound_open = open;
        if changed {
            self.player.apply_sound();
        }
        let mut open = self.pages.volume_open;
        let mut vol = if v.muted { 0.0 } else { v.volume };
        let mut set = None;
        widgets::popover(ui, "volume-panel", vr.rect, 36.0 + 24.0, &mut open, |ui| {
            ui.vertical_centered(|ui| {
                if widgets::vertical_level(ui, &mut vol).changed() {
                    set = Some(vol);
                }
                ui.label(egui::RichText::new(format!("{}", (vol * 100.0).round())).font(theme::medium(12.5)).color(INK));
            });
        });
        self.pages.volume_open = open;
        if let Some(x) = set {
            self.player.set_volume(x);
        }
    }

    /// shuffle, previous, play, next, repeat (.transport): `ghost`, `soft` and `main` are the
    /// sizes of the three kinds of round buttons
    pub fn transport(&mut self, ui: &mut Ui, ghost: f32, soft: f32, main: f32) {
        use widgets::Round;
        let v = &self.view;
        let p = self.player.clone();
        if widgets::round_button(ui, ic::SHUFFLE, ghost, Round::Ghost, v.shuffle, "Перемешать").clicked() {
            p.toggle_shuffle();
        }
        if widgets::round_button(ui, ic::PREV, soft, Round::Soft, false, "Предыдущий").clicked() {
            p.prev();
        }
        let playing = matches!(v.status, Status::Playing | Status::Loading);
        if widgets::round_button(ui, if playing { ic::PAUSE } else { ic::PLAY }, main, Round::Main, false, if playing { "Пауза" } else { "Играть" }).clicked() {
            p.toggle();
        }
        if widgets::round_button(ui, ic::NEXT, soft, Round::Soft, false, "Следующий").clicked() {
            p.next();
        }
        let (icon, tip) = match v.repeat {
            crate::player::Repeat::One => (ic::REPEAT_ONE, "Повтор: трек"),
            crate::player::Repeat::All => (ic::REPEAT, "Повтор: вся очередь"),
            crate::player::Repeat::Off => (ic::REPEAT, "Повтор: выключен"),
        };
        if widgets::round_button(ui, icon, ghost, Round::Ghost, v.repeat != crate::player::Repeat::Off, tip).clicked() {
            p.cycle_repeat();
        }
    }

    fn toasts_ui(&mut self, ui: &mut Ui, screen: Rect) {
        for text in std::mem::take(&mut *LATER.lock().unwrap()) {
            self.toast(&text);
        }
        self.update_toast(ui, screen);
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

/// the panel "Звук, скорость и тон" (src/ui/components/SoundPanel.tsx); true when changed
pub fn sound_panel(ui: &mut Ui) -> bool {
    use crate::store::state::Effects;
    let fx = state::get().effects.read().unwrap().clone();
    let mut next = fx.clone();
    const SPEEDS: [f64; 6] = [0.8, 0.9, 1.0, 1.15, 1.25, 1.35];
    let labels: Vec<String> = SPEEDS.iter().map(|p| if *p == 1.0 { "×1.0".into() } else { format!("×{p}") }).collect();
    let on = SPEEDS.iter().position(|p| (fx.speed - p).abs() < 0.001);
    if let Some(i) = widgets::preset_row(ui, &labels, on) {
        next.speed = SPEEDS[i];
    }
    if let Some(v) = widgets::stepper(ui, "Скорость", fx.speed, &format!("×{:.2}", fx.speed), 0.5, 2.0, 0.01, false) {
        next.speed = (v * 100.0).round() / 100.0;
    }
    // linked: the pitch is the speed's own shift, shown and not set
    let pitch = if fx.linked { 12.0 * fx.speed.log2() } else { fx.pitch };
    let label = if fx.linked { "Тон (идёт за скоростью)" } else { "Тон" };
    let shown = format!("{}{:.1} пт", if pitch > 0.0 { "+" } else { "" }, pitch);
    if let Some(v) = widgets::stepper(ui, label, (pitch * 10.0).round() / 10.0, &shown, -12.0, 12.0, 0.1, fx.linked) {
        next.pitch = (v * 10.0).round() / 10.0;
    }
    if let Some(v) = widgets::toggle_row(ui, "Тон вместе со скоростью", "Как пластинка: быстрее — выше, медленнее — ниже", fx.linked, true) {
        next.linked = v;
    }
    if let Some(v) = widgets::toggle_row(ui, "Оставлять для следующих треков", "Иначе всё сбросится, когда начнётся новый трек", fx.keep, true) {
        next.keep = v;
    }
    if let Some(v) = widgets::toggle_row(ui, "Реверберация", "Объём и пространство, как в зале", fx.reverb, true) {
        next.reverb = v;
    }
    if fx.reverb {
        let pct = (fx.reverb_amount * 100.0).round();
        if let Some(v) = widgets::stepper(ui, "Сила реверберации", pct, &format!("{pct:.0}%"), 0.0, 100.0, 1.0, false) {
            next.reverb_amount = (v / 100.0).clamp(0.0, 1.0);
        }
    }
    if ui.add(egui::Label::new(egui::RichText::new(crate::i18n::t("Сбросить")).font(theme::body(12.5)).color(theme::ink3())).sense(Sense::click())).clicked() {
        next = Effects { speed: 1.0, pitch: 0.0, linked: false, reverb: false, ..next };
    }
    if next != fx {
        state::get().set_effects(|e| *e = next);
        true
    } else {
        false
    }
}

/// a rectangle cut into n x n quads, each corner coloured by `f(u, v)` (0..1 across the
/// rectangle): smooth gradients drawn by the video card from vertex colours
fn grid_mesh(rect: Rect, n: usize, f: impl Fn(f32, f32) -> Color32) -> egui::Shape {
    let mut m = egui::Mesh::default();
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            m.colored_vertex(pos2(rect.left() + rect.width() * u, rect.top() + rect.height() * v), f(u, v));
        }
    }
    let row = (n + 1) as u32;
    for j in 0..n as u32 {
        for i in 0..n as u32 {
            let a = j * row + i;
            m.add_triangle(a, a + 1, a + row);
            m.add_triangle(a + 1, a + row + 1, a + row);
        }
    }
    egui::Shape::mesh(m)
}
