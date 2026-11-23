//! EvPlayer without a browser engine. The same player as the Tauri build (src-tauri + src/),
//! written as one Rust program: a window drawn by the video card through Direct3D 11 (egui
//! lays it out), sound decoded and mixed here and played through WASAPI, the music services
//! asked directly. The parts of src-tauri that never needed the browser (song splitting,
//! offline copies, local files, Discord, the services' sign-ins) are compiled in from there
//! unchanged, so both builds keep one copy of them.

// the shared modules of src-tauri, as they are
#[path = "../../src-tauri/src/discord.rs"]
#[allow(dead_code)]
pub mod discord;
#[path = "../../src-tauri/src/identity.rs"]
pub mod identity;
#[path = "../../src-tauri/src/library.rs"]
#[allow(dead_code)]
pub mod library;
#[path = "../../src-tauri/src/media_server.rs"]
#[allow(dead_code)]
pub mod media_server;
#[path = "../../src-tauri/src/migrate.rs"]
#[allow(dead_code)]
pub mod migrate;
#[path = "../../src-tauri/src/offline.rs"]
#[allow(dead_code)]
pub mod offline;
#[path = "../../src-tauri/src/proxy.rs"]
#[allow(dead_code)]
pub mod proxy;
#[path = "../../src-tauri/src/services.rs"]
#[allow(dead_code)]
pub mod services;
#[path = "../../src-tauri/src/stems.rs"]
#[allow(dead_code, unused_imports)]
pub mod stems;
#[path = "../../src-tauri/src/userproxy.rs"]
#[allow(dead_code)]
pub mod userproxy;

pub mod app;
pub mod audio;
pub mod events;
pub mod ui;

use std::path::PathBuf;

/// %APPDATA%\EvPlayer: the folder the Tauri and Electron versions use, so local files,
/// downloads, split songs and the services' ids carry over as they are
pub fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("EvPlayer")
}

fn main() {
    let started = std::time::Instant::now();
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    userproxy::load(&dir);
    identity::register(&dir);
    let event_loop = match winit::event_loop::EventLoop::<events::Event>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("[main] no event loop: {e}");
            return;
        }
    };
    let ctx = egui::Context::default();
    let app = app::App::new(&ctx, dir, started);
    ui::window::run(app, ctx, event_loop);
}
