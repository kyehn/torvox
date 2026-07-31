//! Ghostty terminal engine — VT parser, command dispatch, and public API.
//!
//! Wraps the Ghostty VT parser in a thread-safe terminal engine with
//! command-based communication between the PTY reader and render thread.

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::thread;

use flume::Sender;

mod commands;
mod internal;
mod keymap;
mod public_api;
mod types;

pub use commands::Command;
pub(crate) use commands::SnapshotCache;
pub use types::*;

pub struct GhosttyTerminal {
    pub(crate) cmd_tx: Sender<Command>,
    pub(crate) query_tx: Sender<Command>,
    pub(crate) cell_data_rx: Option<flume::Receiver<(Vec<CellData>, CursorInfo)>>,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
    pub(crate) pty_write_responses: Arc<Mutex<Vec<Vec<u8>>>>,
    pub(crate) snapshot_cache: Mutex<SnapshotCache>,
    pub(crate) snapshot_rebuild_count: Arc<AtomicU64>,
    /// Set to true if the terminal thread panicked. All subsequent operations
    /// return errors instead of silently sending commands into a dead channel.
    pub(crate) panicked: Arc<AtomicBool>,
    /// Last byte written by `pty_write()`, used to detect `\r`/`\n` split
    /// across consecutive write chunks. Prevents spurious `\r\r\n`.
    pub(crate) last_pty_write_byte: u8,
    /// True when the last `pty_write()` chunk ended inside an unterminated
    /// OSC/DCS string; `pty_write()` closes it with ST on the next chunk.
    pub(crate) last_in_string_mode: bool,
}

impl Drop for GhosttyTerminal {
    fn drop(&mut self) {
        if let Err(error) = self.cmd_tx.send(Command::Terminate) {
            log::error!("ghostty_terminal: cmd_tx send Terminate failed: {error}");
        }
        if let Some(handle) = self.handle.take()
            && let Err(error) = handle.join()
        {
            log::error!("ghostty_terminal: thread join failed: {:?}", error);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_s2_fixes;

#[cfg(test)]
mod snapshot_cache_unit_tests;
