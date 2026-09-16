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
pub use commands::Query;
pub(crate) use commands::SnapshotCache;
pub use types::*;

pub struct GhosttyTerminal {
    pub(crate) cmd_tx: Sender<Command>,
    pub(crate) query_tx: Sender<Query>,
    pub(crate) cell_data_rx: Option<flume::Receiver<(Vec<CellData>, CursorInfo)>>,
    /// 上游 OSC 回调事件接收端（VT 线程经 on_pwd_changed / on_clipboard_write 推送）。
    /// session 在 flush 后收割到锁存槽；BDD 直接轮询断言。
    pub(crate) cwd_rx: flume::Receiver<String>,
    pub(crate) clipboard_rx: flume::Receiver<(String, String)>,
    /// 上游 BEL 回调事件接收端（VT 线程经 on_bell 推送，每次振铃一个空消息）。
    pub(crate) bell_rx: flume::Receiver<()>,
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
    /// Mirror of `Terminal::active_screen() == Alternate`, updated lock-free
    /// by the VT thread on every emitted frame (internal.rs build_cell_data).
    /// Lets the Android input path detect the alternate screen buffer
    /// (vim/less/htop) without a blocking RPC, so touch-scroll gestures can
    /// be forwarded as mouse-wheel escapes instead of scrolling local
    /// scrollback (Haven research: altScreen wheel consumption).
    pub(crate) alt_screen_active: Arc<AtomicBool>,
}

impl Drop for GhosttyTerminal {
    fn drop(&mut self) {
        // try_send：VT 线程卡住时不得阻塞析构（与全仓非阻塞策略一致），失败仅记日志后 join。
        if let Err(error) = self.cmd_tx.try_send(Command::Terminate) {
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
