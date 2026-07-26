//! Terminal session orchestration.
//!
//! This crate owns the PTY lifecycle, the VT parsing engine
//! ([`ghostty_terminal`], wrapping `libghostty-vt`), and the [`session`]
//! coordinator that wires the PTY reader, input writer, process waiter, and
//! renderer together. It depends on the core crate (data model) and
//! `libghostty-vt` (vendored VT parser) and is depended on by
//! the renderer and GUI crates.
//!
//! Key realities (post-overhaul):
//! * The Ghostty key encoder (`key::Encoder` + `key::Event`) is allocated
//!   **once per terminal worker** and reused across keystrokes; encoder modes
//!   are re-synced every key via `set_options_from_terminal`.
//! * OSC 7 (cwd) is intercepted by [`osc_handler`] and surfaced as
//!   `OscEvent::Cwd`; the session stores it in [`session::Session::cwd`].
//! * PTY hygiene (setsid + controlling tty, IUTF8, IXON/IXOFF cleared,
//!   `ws_xpixel`/`ws_ypixel`, stray-fd close) is configured in [`pty`].

pub mod action_parser;
pub mod cursor_cmds;
pub mod ghostty_terminal;
mod lock_util;
pub mod mock_pty;
pub mod osc_handler;
pub mod output_processor;
pub mod pty;
pub mod session;
pub mod sgr_parser;
pub mod shell_env;

pub mod snapshot_test;
pub mod test_helpers;
pub mod vt_conformance;

pub use mock_pty::{MockPty, MockPtyHandle};
pub use pty::{Pty, PtyError, PtyPair};
pub use shell_env::ShellEnv;
