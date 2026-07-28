//! Terminal session orchestration.
//!
//! This crate owns the PTY lifecycle, the VT parsing engine
//! ([`ghostty_terminal`], wrapping `libghostty-vt`), and the [`session`]
//! coordinator that wires the PTY reader, input writer, process waiter, and
//! renderer together.
//!
//! Key realities (post-overhaul):
//! * The Ghostty key encoder (`key::Encoder` + `key::Event`) is allocated
//!   **once per terminal worker** and reused across keystrokes; encoder modes
//!   are re-synced every key via `set_options_from_terminal`.
//! * OSC 7 (cwd) is intercepted by [`osc_handler`] and surfaced as
//!   `OscEvent::Cwd`; the session stores it in [`session::Session::cwd`].
//! * PTY hygiene (setsid + controlling tty, IUTF8, IXON/IXOFF cleared,
//!   `ws_xpixel`/`ws_ypixel`, stray-fd close) is configured in [`pty`].

pub mod ghostty_terminal;
pub mod mock_pty;
pub mod osc_handler;
pub mod output_processor;
pub mod pty;
pub mod session;
pub use session::ThemeConfig;
pub mod shell_env;

#[cfg(any(test, feature = "test-util"))]
pub(crate) mod action_parser;
#[cfg(any(test, feature = "test-util"))]
pub(crate) mod cursor_cmds;
#[cfg(any(test, feature = "test-util"))]
pub(crate) mod sgr_parser;
#[cfg(any(test, feature = "test-util"))]
pub(crate) mod snapshot_test;
#[cfg(any(test, feature = "test-util"))]
pub(crate) mod test_helpers;
#[cfg(any(test, feature = "test-util"))]
pub(crate) mod vt_conformance;

pub use mock_pty::{MockPty, MockPtyHandle};
pub use pty::{Pty, PtyError, PtyPair};
pub use shell_env::ShellEnv;

// Re-export core types that were formerly in terminal-core.
pub use ghostty_terminal::{CellData, CursorInfo, CursorStyle, SelectionMode, is_wide};
