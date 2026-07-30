//! Neutral event types and thread-safe event queue.
//!
//! Events are shared between JNI bridge and MCP server. The `EventQueue`
//! is the central rendezvous point: terminal/MCP push events into it,
//! Kotlin `pollEvent()` drains them in FIFO order.

use std::collections::VecDeque;
use std::sync::Mutex;

/// An event sent from Rust to the Kotlin UI layer.
///
/// # Trigger sources
///
/// | Variant | Triggered by | From module |
/// |---------|-------------|-------------|
/// | `Bell`  | `process_output()` detects BEL character | `session` |
/// | `Clipboard` | `poll_clipboard()` returns OSC 52 read | `session` |
/// | `Exit`  | `session.is_exited()` becomes true | `session` / `pollEvent` |
/// | `ShowDialog` | MCP `dialog` tool call | `mcp` (via ffi callback) |
/// | `PickFile` | MCP `pick_file` tool call | `mcp` (via ffi callback) |
///
/// All events are serialised as JSON before crossing the JNI boundary.
/// Uses internal tagging (`#[serde(tag = "event")]`) so Kotlin can match on `event` field.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// Terminal bell character (^G) received.
    Bell { session_id: u64 },
    /// Clipboard content requested by the terminal (OSC 52 read).
    Clipboard { session_id: u64, text: String },
    /// Child process exited.
    Exit { session_id: u64, code: i32 },
    /// Request Kotlin to show a dialog (input/confirm/select).
    /// Kotlin responds by calling `dialogResult()` JNI.
    #[cfg(feature = "mcp")]
    ShowDialog {
        session_id: u64,
        request_id: u64,
        dialog_type: String,
        title: String,
        message: String,
        options: Vec<String>,
    },
    /// Request Kotlin to show a file picker (Android SAF / desktop).
    /// Kotlin responds by calling `filePicked()` JNI.
    #[cfg(feature = "mcp")]
    PickFile {
        session_id: u64,
        request_id: u64,
        starting_path: String,
        filter: String,
    },
}

/// A thread-safe event queue shared between Rust and Kotlin.
///
/// Events are consumed in FIFO order: `push()` adds to the back,
/// `pop()` removes from the front.
///
/// # Lock ordering
/// If both [`crate::android::ffi::SESSION_REGISTRY`] and this queue
/// must be held, always lock SESSION_REGISTRY first, then the queue.
/// The reverse order will deadlock.
pub struct EventQueue {
    inner: Mutex<VecDeque<Event>>,
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl EventQueue {
    /// Create a new empty event queue.
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
        }
    }

    /// Push an event to the back of the queue (FIFO).
    pub fn push(&self, event: Event) {
        match self.inner.lock() {
            Ok(mut guard) => guard.push_back(event),
            Err(poisoned) => {
                log::warn!("EventQueue: push lock poisoned, recovered");
                poisoned.into_inner().push_back(event);
            }
        }
    }

    /// Pop the oldest event from the front of the queue (FIFO).
    pub fn pop(&self) -> Option<Event> {
        match self.inner.lock() {
            Ok(mut guard) => guard.pop_front(),
            Err(poisoned) => {
                log::warn!("EventQueue: pop lock poisoned, recovered");
                poisoned.into_inner().pop_front()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_fifo_order() {
        let q = EventQueue::new();
        q.push(Event::Bell { session_id: 1 });
        q.push(Event::Bell { session_id: 2 });
        q.push(Event::Bell { session_id: 3 });
        assert_eq!(q.pop(), Some(Event::Bell { session_id: 1 }));
        assert_eq!(q.pop(), Some(Event::Bell { session_id: 2 }));
        assert_eq!(q.pop(), Some(Event::Bell { session_id: 3 }));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn pop_empty_returns_none() {
        let q = EventQueue::new();
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn multiple_events_interleaved() {
        let q = EventQueue::new();
        q.push(Event::Bell { session_id: 1 });
        q.push(Event::Exit {
            session_id: 2,
            code: 0,
        });
        q.push(Event::Bell { session_id: 3 });
        assert_eq!(q.pop(), Some(Event::Bell { session_id: 1 }));
        q.push(Event::Clipboard {
            session_id: 4,
            text: "hello".into(),
        });
        assert_eq!(
            q.pop(),
            Some(Event::Exit {
                session_id: 2,
                code: 0
            })
        );
        assert_eq!(q.pop(), Some(Event::Bell { session_id: 3 }));
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 4,
                text: "hello".into(),
            })
        );
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn push_pop_default_works() {
        let q: EventQueue = Default::default();
        q.push(Event::Exit {
            session_id: 1,
            code: 0,
        });
        assert_eq!(
            q.pop(),
            Some(Event::Exit {
                session_id: 1,
                code: 0
            })
        );
        assert_eq!(q.pop(), None);
    }
}
