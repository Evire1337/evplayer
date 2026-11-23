//! How the rest of the program wakes the window: a sound that started or ended, a search that
//! answered, a key pressed while another program is in front, a click in the tray. Any thread
//! may send; the window's event loop gets it, and the interface is drawn again when it should.

use std::sync::OnceLock;
use std::time::Duration;

use winit::event_loop::EventLoopProxy;

use crate::app::actions::Action;

#[derive(Debug, Clone)]
pub enum Event {
    /// draw again after this long (egui asks this way, so do the background threads)
    Repaint(Duration),
    /// something the person did outside the window: a global key, the tray, media keys
    Action(Action),
    /// bring the window up (the tray, a second start of the program)
    ShowWindow,
    /// a link given to a second start of the program (evplayer://...)
    OpenLink(String),
    /// close for real (the tray's "Выход")
    Quit,
    /// the window's own close button (to the tray or quit, by the settings)
    Close,
}

static PROXY: OnceLock<EventLoopProxy<Event>> = OnceLock::new();

pub fn install(proxy: EventLoopProxy<Event>) {
    let _ = PROXY.set(proxy);
}

pub fn send(event: Event) {
    if let Some(p) = PROXY.get() {
        let _ = p.send_event(event);
    }
}

/// the interface has something new to show
pub fn repaint() {
    send(Event::Repaint(Duration::ZERO));
}
