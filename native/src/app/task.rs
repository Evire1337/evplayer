//! Work that waits on the network, done off the window's thread: a page asks, shows "loading",
//! and is drawn again when the answer is in. A task whose page was left just finishes into
//! nothing.

use std::sync::{Arc, Mutex};

pub struct Task<T> {
    slot: Arc<Mutex<Option<T>>>,
}

impl<T: Send + 'static> Task<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Self {
        let slot = Arc::new(Mutex::new(None));
        let s = slot.clone();
        let _ = std::thread::Builder::new().name("task".into()).spawn(move || {
            let v = f();
            *s.lock().unwrap() = Some(v);
            crate::events::repaint();
        });
        Self { slot }
    }

    /// the answer, once (None while it is not in yet)
    pub fn take(&self) -> Option<T> {
        self.slot.lock().unwrap().take()
    }
}

/// something a page shows that comes from the network
pub enum Load<T> {
    Idle,
    Loading(Task<T>),
    Ready(T),
}

impl<T: Send + 'static> Load<T> {
    pub fn start(f: impl FnOnce() -> T + Send + 'static) -> Self {
        Load::Loading(Task::spawn(f))
    }

    /// moves a finished task into Ready; true while it still loads
    pub fn poll(&mut self) -> bool {
        if let Load::Loading(t) = self {
            if let Some(v) = t.take() {
                *self = Load::Ready(v);
                return false;
            }
            return true;
        }
        false
    }

    pub fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, Load::Idle)
    }
}
