//! Ghostty terminal engine — VT parser, command dispatch, and public API.
//!
//! Wraps the Ghostty VT parser in a thread-safe terminal engine with
//! command-based communication between the PTY reader and render thread.

use std::sync::atomic::AtomicU64;
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
#[cfg(test)]
pub(crate) use internal::snapshot_needs_rebuild;
pub use types::*;

pub struct GhosttyTerminal {
    pub(crate) cmd_tx: Sender<Command>,
    pub(crate) query_tx: Sender<Command>,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
    pub(crate) pty_write_responses: Arc<Mutex<Vec<Vec<u8>>>>,
    pub(crate) snapshot_cache: Mutex<SnapshotCache>,
    pub(crate) snapshot_rebuild_count: Arc<AtomicU64>,
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
