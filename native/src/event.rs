//! Neutral event types and thread-safe event queue.
//!
//! Events are shared between JNI bridge and MCP server. The `EventQueue`
//! is the central rendezvous point: terminal/MCP push events into it,
//! Kotlin `pollEvent()` drains them in FIFO order.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

/// Maximum number of events buffered before the oldest are dropped.
/// Bounds memory when a burst of MCP/terminal events outpaces the
/// UI drain rate (Kotlin drains up to 32 events per frame per session).
const MAX_QUEUED_EVENTS: usize = 1024;

/// Minimum interval between overflow warnings (rate limiting).
const OVERFLOW_WARN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// An event sent from Rust to the Kotlin UI layer.
///
/// # Trigger sources
///
/// | Variant | Triggered by | From module |
/// |---------|-------------|-------------|
/// | `Bell`  | `process_output()` detects BEL character | `session` |
/// | `Clipboard` | OSC 52 **set** (terminal→clipboard) or MCP `clipboard_set` | `session` / `mcp` |
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
    /// Clipboard write content: text the terminal (OSC 52 set) or MCP
    /// `clipboard_set` wants placed in the SYSTEM clipboard. Kotlin applies
    /// it via `setPrimaryClip`. (OSC 52 read is not implemented —
    /// osc_handler logs and ignores read requests.)
    Clipboard { session_id: u64, text: String },
    /// Desktop notification requested by the terminal (OSC 9).
    Notification {
        session_id: u64,
        title: String,
        body: String,
    },
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
    /// Kotlin answers via the shared `dialogResult()` JNI, keyed by
    /// (session_id, request_id) — same routing as ShowDialog.
    #[cfg(feature = "mcp")]
    PickFile {
        session_id: u64,
        request_id: u64,
        starting_path: String,
        filter: String,
    },
    /// MCP `clipboard_get`: request Kotlin to read the system clipboard and
    /// answer via `clipboardResult()` JNI.
    #[cfg(feature = "mcp")]
    GetClipboard { session_id: u64, request_id: u64 },
    /// MCP `toast`: request Kotlin to show a brief toast message.
    #[cfg(feature = "mcp")]
    Toast { text: String },
    /// MCP `open_url`: request Kotlin to open a URL in the default browser.
    #[cfg(feature = "mcp")]
    OpenUrl { url: String },
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
    /// Last time an overflow warning was logged (rate limiting).
    last_overflow_warn: Mutex<Option<Instant>>,
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
            last_overflow_warn: Mutex::new(None),
        }
    }

    /// Push an event to the back of the queue (FIFO).
    ///
    /// If the queue is at capacity, an `Exit` event is never dropped:
    /// the oldest non-Exit event is evicted instead. Kotlin depends on
    /// `Exit` to reap the session (the native `exit_reported` flag is
    /// set at push time and never re-sent), so losing it would leak the
    /// session permanently. Overflow warnings are rate-limited to one
    /// per second.
    ///
    /// Exception: when the queue holds ONLY Exit events at capacity, the
    /// NEW event is dropped instead (evicting any Exit would strand its
    /// session forever). Reaching this state requires 1024 concurrent
    /// un-reaped exits — practically unreachable (Kotlin reaps within a
    /// frame), but the invariant is deliberate.
    pub fn push(&self, event: Event) {
        match self.inner.lock() {
            Ok(mut guard) => {
                if guard.len() >= MAX_QUEUED_EVENTS {
                    self.warn_overflow_once();
                    let evict_idx = guard.iter().position(|e| !matches!(e, Event::Exit { .. }));
                    match evict_idx {
                        Some(idx) => {
                            guard.remove(idx);
                        }
                        None => {
                            // Queue holds only Exit events: evicting one
                            // would strand that session forever (its
                            // exit_reported flag is already set), so drop
                            // the NEW event instead.
                            return;
                        }
                    }
                }
                guard.push_back(event);
            }
            Err(poisoned) => {
                log::warn!("EventQueue: push lock poisoned, recovered");
                let mut guard = poisoned.into_inner();
                if guard.len() >= MAX_QUEUED_EVENTS {
                    self.warn_overflow_once();
                    let evict_idx = guard.iter().position(|e| !matches!(e, Event::Exit { .. }));
                    match evict_idx {
                        Some(idx) => {
                            guard.remove(idx);
                        }
                        None => {
                            return;
                        }
                    }
                }
                guard.push_back(event);
            }
        }
    }

    /// Log the queue-overflow warning at most once per second.
    fn warn_overflow_once(&self) {
        let now = Instant::now();
        let mut last = match self.last_overflow_warn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if last.is_none_or(|t| now.duration_since(t) >= OVERFLOW_WARN_INTERVAL) {
            log::warn!("EventQueue: dropping oldest event (queue full at {MAX_QUEUED_EVENTS})");
            *last = Some(now);
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
    fn push_drops_oldest_when_full() {
        let q = EventQueue::new();
        for i in 0..(MAX_QUEUED_EVENTS + 8) {
            q.push(Event::Bell {
                session_id: i as u64,
            });
        }
        // Oldest events must have been dropped, newest retained.
        assert_eq!(q.pop(), Some(Event::Bell { session_id: 8 }));
        assert_eq!(q.pop(), Some(Event::Bell { session_id: 9 }));
    }

    #[test]
    fn push_never_evicts_exit_events() {
        let q = EventQueue::new();
        // Fill the queue with Exit events, then push more non-Exit events
        // than fit: Exits must ALL survive (their exit_reported flags are
        // already set natively and would never be re-sent), the new events
        // are dropped instead.
        for i in 0..MAX_QUEUED_EVENTS {
            q.push(Event::Exit {
                session_id: i as u64,
                code: 0,
            });
        }
        q.push(Event::Bell { session_id: 999 });
        q.push(Event::Bell { session_id: 1000 });
        // Every Exit survives; the Bells were dropped.
        let mut exits = 0;
        while let Some(event) = q.pop() {
            assert!(
                matches!(event, Event::Exit { .. }),
                "non-Exit evicted: {event:?}"
            );
            exits += 1;
        }
        assert_eq!(exits, MAX_QUEUED_EVENTS);
    }

    #[test]
    fn push_evicts_oldest_non_exit_when_mixed() {
        let q = EventQueue::new();
        // Fill with Bell events, then cap with one Exit at the back.
        for i in 0..(MAX_QUEUED_EVENTS - 1) {
            q.push(Event::Bell {
                session_id: i as u64,
            });
        }
        q.push(Event::Exit {
            session_id: 42,
            code: 7,
        });
        // Queue is now full; pushing a new Bell must evict the OLDEST
        // Bell (session 0), never the Exit.
        q.push(Event::Bell { session_id: 1000 });
        let popped = (0..MAX_QUEUED_EVENTS)
            .filter_map(|_| q.pop())
            .collect::<Vec<_>>();
        assert_eq!(popped[0], Event::Bell { session_id: 1 });
        assert!(popped.contains(&Event::Exit {
            session_id: 42,
            code: 7
        }));
        assert_eq!(
            popped[MAX_QUEUED_EVENTS - 1],
            Event::Bell { session_id: 1000 }
        );
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
