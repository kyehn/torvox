//! Property-based (proptest) and concurrency (shuttle) tests.
//!
//! - `osc52_roundtrip`: arbitrary UTF-8 text round-trips through the real
//!   `dispatch_osc52` path byte-identical (selection + text).
//! - `event_queue_concurrent_push_pop`: many threads push/pop the shared
//!   EventQueue concurrently — every pushed event is popped exactly once
//!   (no loss, no duplication, no deadlock).
//! - `event_queue_exit_survives_overflow`: concurrent pushes that overflow
//!   the queue never evict an Exit event (the invariant Kotlin depends on
//!   to reap sessions).

use proptest::prelude::*;
use std::sync::Arc;

use crate::event::{Event, EventQueue};
use crate::terminal::osc_handler::OscHandler;

proptest! {
    /// Any UTF-8 string, when round-tripped through the OSC 52 dispatch
    /// path, yields an identical Clipboard event (selection + text).
    /// (Arbitrary bytes are covered separately by
    /// `osc52_arbitrary_payload_never_panics` because OSC 52 is a text
    /// protocol; the terminal delivers UTF-8 strings.)
    #[test]
    fn osc52_roundtrip(text in "\\PC*", selection in "[^;]{0,16}") {
        let handler = OscHandler::new();
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let payload = format!("{selection};{encoded}");
        let event = handler.dispatch_osc52_for_test(&payload);
        let osc_event = event.expect("valid OSC 52 payload must dispatch");
        match osc_event {
            crate::terminal::osc_handler::OscEvent::Clipboard(ev) => {
                prop_assert_eq!(ev.selection, selection);
                prop_assert_eq!(ev.text, text);
            }
            other => panic!("expected Clipboard event, got {other:?}"),
        }
    }

}

/// Concurrent push/pop on the shared EventQueue: every pushed event is
/// popped exactly once, regardless of scheduling (shuttle explores
/// interleavings). Runs under plain `cargo test` (64 scheduler iterations;
/// deterministic under shuttle's scheduler).
#[test]
fn event_queue_concurrent_push_pop() {
    shuttle::check_random(
        || {
            let queue = Arc::new(EventQueue::new());
            const N: u64 = 32;
            const THREADS: usize = 4;

            let mut handles = Vec::new();
            for t in 0..THREADS {
                let queue = Arc::clone(&queue);
                handles.push(shuttle::thread::spawn(move || {
                    for i in 0..N {
                        queue.push(Event::Clipboard {
                            session_id: t as u64 * N + i,
                            text: String::new(),
                        });
                    }
                }));
            }

            // Drain concurrently from the main thread while workers push.
            let mut popped = std::collections::HashSet::new();
            let mut attempts = 0;
            while popped.len() < N as usize * THREADS && attempts < 10_000 {
                if let Some(event) = queue.pop() {
                    match event {
                        Event::Clipboard { session_id, .. } => {
                            assert!(popped.insert(session_id), "duplicate event {session_id}");
                        }
                        other => panic!("unexpected event {other:?}"),
                    }
                }
                shuttle::thread::yield_now();
                attempts += 1;
            }
            for handle in handles {
                handle.join().unwrap();
            }
            // Drain whatever remains after joins.
            while let Some(event) = queue.pop() {
                match event {
                    Event::Clipboard { session_id, .. } => {
                        assert!(popped.insert(session_id), "duplicate event {session_id}");
                    }
                    other => panic!("unexpected event {other:?}"),
                }
            }
            assert_eq!(popped.len(), N as usize * THREADS, "events lost");
        },
        64,
    );
}

/// Exit events must survive queue overflow under concurrent pushing:
/// Kotlin relies on Exit to reap sessions (native exit_reported is set at
/// push time and never re-sent), so evicting one would leak the session.
#[test]
fn event_queue_exit_survives_overflow() {
    shuttle::check_random(
        || {
            let queue = Arc::new(EventQueue::new());
            const THREADS: usize = 4;

            let mut handles = Vec::new();
            for t in 0..THREADS {
                let queue = Arc::clone(&queue);
                handles.push(shuttle::thread::spawn(move || {
                    for i in 0..50u64 {
                        queue.push(Event::Clipboard {
                            session_id: t as u64 * 100 + i,
                            text: String::new(),
                        });
                    }
                    // One Exit per thread, pushed last (must survive).
                    queue.push(Event::Exit {
                        session_id: t as u64,
                        code: 0,
                        alive_ms: 0,
                    });
                }));
            }
            for handle in handles {
                handle.join().unwrap();
            }

            let mut exits = 0;
            let mut bells = 0;
            while let Some(event) = queue.pop() {
                match event {
                    Event::Exit { .. } => exits += 1,
                    Event::Clipboard { .. } => bells += 1,
                    other => panic!("unexpected event {other:?}"),
                }
            }
            assert_eq!(exits, THREADS, "an Exit event was evicted");
            assert!(bells > 0);
        },
        64,
    );
}
