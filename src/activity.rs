//! Global in-memory activity log backing the Live Activity panel.
//!
//! Every protocol layer (BLE, mDNS, HTTPS server, transfers) reports
//! human-readable events here so failures can be diagnosed in the field —
//! e.g. an iPhone that connects but aborts at the TLS handshake.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

const MAX_EVENTS: usize = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Bluetooth,
    Mdns,
    Server,
    Transfer,
    Error,
}

impl Category {
    pub fn icon(&self) -> &'static str {
        match self {
            Category::Bluetooth => "🛜",
            Category::Mdns => "📡",
            Category::Server => "🌐",
            Category::Transfer => "📦",
            Category::Error => "⚠️",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    pub at: chrono::DateTime<chrono::Local>,
    pub category: Category,
    pub message: String,
}

fn store() -> &'static Mutex<VecDeque<Event>> {
    static STORE: OnceLock<Mutex<VecDeque<Event>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Record an event (newest first). Callable from any thread or task.
pub fn log(category: Category, message: impl Into<String>) {
    if let Ok(mut q) = store().lock() {
        q.push_front(Event {
            at: chrono::Local::now(),
            category,
            message: message.into(),
        });
        q.truncate(MAX_EVENTS);
    }
}

/// Current events, newest first.
pub fn snapshot() -> Vec<Event> {
    store()
        .lock()
        .map(|q| q.iter().cloned().collect())
        .unwrap_or_default()
}

/// Clear the log (Activity panel "Clear" button).
pub fn clear() {
    if let Ok(mut q) = store().lock() {
        q.clear();
    }
}
