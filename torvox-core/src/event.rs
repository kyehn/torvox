//! Terminal event types — focus, cwd, clipboard, notification.
//!
//! # Requirements
//! - [FR-028](crate) — Process: exited callback
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::cursor::CursorState;
use crate::selection::Selection;

/// Events produced by the terminal emulator for consumption by the UI layer.
///
/// Each variant represents a distinct category of terminal output:
///
/// * **Output events** — `OutputReady`, `Bell` — signal that new data is available
///   or an audible alert is requested.
/// * **State change events** — `TitleChanged`, `CursorChanged`, `SelectionChanged` —
///   notify the UI of changes to terminal metadata.
/// * **Input/request events** — `ClipboardRequest`, `HyperlinkHover` — request
///   interaction with the host system (clipboard access, URL hover display).
/// * **Lifecycle events** — `ProcessExited` — signals that the child process has
///   terminated.
/// * **Rendering events** — `DirtyRegion` — identifies which rows of the grid
///   have changed and need re-rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum TerminalEvent {
    /// New output is available from the PTY — trigger a read cycle.
    OutputReady,
    /// The terminal requested an audible bell alert.
    Bell,
    /// The window or tab title was changed (e.g. via OSC escape sequence).
    TitleChanged(String),
    /// A clipboard operation was requested (e.g. paste, copy).
    ClipboardRequest(String),
    /// Hyperlink hover state changed — `Some(url)` on hover, `None` on exit.
    HyperlinkHover(Option<String>),
    /// The child process exited with the given exit code.
    ProcessExited(i32),
    /// The cursor position or style changed — UI should redraw the cursor.
    CursorChanged(CursorState),
    /// The text selection was changed or cleared.
    SelectionChanged(Option<Selection>),
    /// A range of rows in the viewport was modified and needs re-rendering.
    DirtyRegion(DirtyRegion),
}

/// A range of rows in the terminal grid that have been modified.
///
/// The `start_row` and `end_row` define an inclusive range of row indices
/// that contain dirty (modified) content since the last render frame.
/// The renderer uses this information to only repaint affected rows.
///
/// Row indices are 0-based and reference the visible viewport rows,
/// not scrollback history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct DirtyRegion {
    /// First modified row (inclusive).
    pub start_row: u32,
    /// Last modified row (inclusive).
    pub end_row: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::{CursorState, CursorStyle};
    use crate::selection::{Selection, SelectionAnchor, SelectionMode};
    use alloc::format;

    #[test]
    fn event_title_changed_carries_string() {
        let e = TerminalEvent::TitleChanged(String::from("My Terminal"));
        if let TerminalEvent::TitleChanged(title) = &e {
            assert_eq!(title.as_str(), "My Terminal");
        } else {
            panic!("Expected TitleChanged variant");
        }
    }

    #[test]
    fn event_title_changed_inequality() {
        let e1 = TerminalEvent::TitleChanged(String::from("a"));
        let e2 = TerminalEvent::TitleChanged(String::from("b"));
        assert_ne!(e1, e2);
    }

    #[test]
    fn event_clipboard_request_carries_operation() {
        let e = TerminalEvent::ClipboardRequest(String::from("paste"));
        if let TerminalEvent::ClipboardRequest(op) = &e {
            assert_eq!(op.as_str(), "paste");
        } else {
            panic!("Expected ClipboardRequest variant");
        }
    }

    #[test]
    fn event_hyperlink_hover_some_vs_none() {
        let some = TerminalEvent::HyperlinkHover(Some(String::from("https://x")));
        let none = TerminalEvent::HyperlinkHover(None);
        assert_ne!(some, none);
    }

    #[test]
    fn event_hyperlink_hover_none_is_distinguishable() {
        let e = TerminalEvent::HyperlinkHover(None);
        if let TerminalEvent::HyperlinkHover(url) = &e {
            assert!(url.is_none());
        } else {
            panic!("Expected HyperlinkHover variant");
        }
    }

    #[test]
    fn event_process_exit_code_distinguishes_success_from_failure() {
        let success = TerminalEvent::ProcessExited(0);
        let failure = TerminalEvent::ProcessExited(1);
        assert_ne!(success, failure, "exit code 0 and 1 should differ");
    }

    #[test]
    fn event_process_exited_carries_code() {
        let e = TerminalEvent::ProcessExited(42);
        if let TerminalEvent::ProcessExited(code) = e {
            assert_eq!(code, 42);
        } else {
            panic!("Expected ProcessExited variant");
        }
    }

    #[test]
    fn event_cursor_changed_carries_state() {
        let cursor = CursorState::new(5, 10);
        let e = TerminalEvent::CursorChanged(cursor);
        if let TerminalEvent::CursorChanged(c) = e {
            assert_eq!(c.row, 5);
            assert_eq!(c.col, 10);
            assert_eq!(c.style, CursorStyle::Block);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn event_selection_changed_none_distinguishes_from_some() {
        let none = TerminalEvent::SelectionChanged(None);
        let sel = Selection::new(
            SelectionAnchor { row: 1, col: 2 },
            SelectionAnchor { row: 3, col: 4 },
            SelectionMode::Char,
        );
        let some = TerminalEvent::SelectionChanged(Some(sel));
        assert_ne!(none, some, "None and Some selection should differ");
    }

    #[test]
    fn event_dirty_region_carries_bounds() {
        let region = DirtyRegion {
            start_row: 10,
            end_row: 20,
        };
        let e = TerminalEvent::DirtyRegion(region.clone());
        if let TerminalEvent::DirtyRegion(r) = &e {
            assert_eq!(r.start_row, 10);
            assert_eq!(r.end_row, 20);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn event_distinct_variants_not_equal() {
        assert_ne!(TerminalEvent::OutputReady, TerminalEvent::Bell);
        assert_ne!(
            TerminalEvent::OutputReady,
            TerminalEvent::TitleChanged(String::from("a"))
        );
    }

    #[test]
    fn event_serde_json_roundtrip() {
        use alloc::vec;
        let events = vec![
            TerminalEvent::OutputReady,
            TerminalEvent::Bell,
            TerminalEvent::TitleChanged(String::from("hello world")),
            TerminalEvent::ClipboardRequest(String::from("x")),
            TerminalEvent::HyperlinkHover(Some(String::from("https://a"))),
            TerminalEvent::HyperlinkHover(None),
            TerminalEvent::ProcessExited(0),
            TerminalEvent::ProcessExited(42),
            TerminalEvent::CursorChanged(CursorState {
                row: 1,
                col: 2,
                style: CursorStyle::Bar,
                visible: false,
            }),
            TerminalEvent::SelectionChanged(None),
            TerminalEvent::DirtyRegion(DirtyRegion {
                start_row: 0,
                end_row: 24,
            }),
        ];
        for e in events {
            let json = serde_json::to_string(&e).expect("serialize");
            let back: TerminalEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(e, back);
        }
    }

    #[test]
    fn dirty_region_bounds_are_correct() {
        let region = DirtyRegion {
            start_row: 3,
            end_row: 7,
        };
        assert_eq!(
            region.end_row - region.start_row,
            4,
            "region should span 4 rows"
        );
    }

    #[cfg(feature = "rkyv")]
    #[test]
    fn event_rkyv_roundtrip() {
        use alloc::vec;
        use rkyv::rancor::Error;
        let events = vec![
            TerminalEvent::OutputReady,
            TerminalEvent::Bell,
            TerminalEvent::TitleChanged(String::from("hello")),
            TerminalEvent::ClipboardRequest(String::from("paste")),
            TerminalEvent::HyperlinkHover(Some(String::from("https://example.com"))),
            TerminalEvent::HyperlinkHover(None),
            TerminalEvent::ProcessExited(42),
            TerminalEvent::CursorChanged(CursorState::new(5, 10)),
            TerminalEvent::SelectionChanged(None),
            TerminalEvent::DirtyRegion(DirtyRegion {
                start_row: 0,
                end_row: 24,
            }),
        ];
        for e in events {
            let bytes = rkyv::to_bytes::<Error>(&e).expect("rkyv serialize");
            let archived =
                rkyv::access::<<TerminalEvent as rkyv::Archive>::Archived, Error>(&bytes)
                    .expect("rkyv access");
            let restored: TerminalEvent =
                rkyv::deserialize::<TerminalEvent, Error>(archived).expect("rkyv deserialize");
            assert_eq!(e, restored);
        }
    }

    #[test]
    fn terminal_event_debug_not_empty() {
        let events = [
            TerminalEvent::OutputReady,
            TerminalEvent::Bell,
            TerminalEvent::TitleChanged(String::from("hello")),
            TerminalEvent::ClipboardRequest(String::from("text")),
            TerminalEvent::HyperlinkHover(Some(String::from("https://example.com"))),
            TerminalEvent::HyperlinkHover(None),
            TerminalEvent::ProcessExited(0),
            TerminalEvent::CursorChanged(CursorState::new(1, 2)),
            TerminalEvent::SelectionChanged(None),
            TerminalEvent::DirtyRegion(DirtyRegion {
                start_row: 0,
                end_row: 24,
            }),
        ];
        for event in &events {
            let debug = format!("{:?}", event);
            assert!(!debug.is_empty(), "Event should have non-empty debug");
        }
    }
}
