//! Ghostty 终端引擎：VT 解析、命令分发与公开 API。
//! 以命令方式在 PTY 读取线程与渲染线程间通信。

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flume::Sender;

mod commands;
mod internal;
mod keymap;
mod public_api;
mod types;

pub use commands::Command;
pub use commands::Query;
pub use types::*;

/// VT 线程析构等待上限，与会话拆卸宽限一致，避免无界阻塞。
const TERMINAL_THREAD_JOIN_TIMEOUT: Duration = Duration::from_millis(50);

pub struct GhosttyTerminal {
    pub(crate) cmd_tx: Sender<Command>,
    pub(crate) query_tx: Sender<Query>,
    pub(crate) cell_data_rx: Option<flume::Receiver<(Vec<CellData>, CursorInfo)>>,
    /// 上游 OSC 52 回调事件接收端（VT 线程经 on_clipboard_write 推送）。
    /// session 在 flush 后收割到锁存槽；BDD 直接轮询断言。
    pub(crate) clipboard_rx: flume::Receiver<(String, String)>,
    /// 上游 BEL 回调事件接收端（VT 线程经 on_bell 推送，每次振铃一个空消息）。
    pub(crate) bell_rx: flume::Receiver<()>,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
    pub(crate) pty_write_responses: Arc<Mutex<Vec<Vec<u8>>>>,
    /// 终端线程 panic 后置真：后续操作一律返回错误，不再向死信道静默发命令。
    pub(crate) panicked: Arc<AtomicBool>,
    /// `pty_write()` 末次写入的字节，用于识别 `\r`/`\n` 跨写入块拆分，避免多余的 `\r\r\n`。
    pub(crate) last_pty_write_byte: u8,
    /// `Terminal::active_screen() == Alternate` 的无锁镜像，由 VT 线程每帧更新。
    /// 供 Android 输入路径免阻塞 RPC 检出备用屏（vim/less/htop），使触摸滚动
    /// 转发为滚轮转义序列而非滚动本地回滚（备用屏会吞掉滚轮）。
    pub(crate) alt_screen_active: Arc<AtomicBool>,
}

impl Drop for GhosttyTerminal {
    fn drop(&mut self) {
        // try_send：VT 线程卡住时不得阻塞析构（与全仓非阻塞策略一致），失败仅记日志后限时等待。
        if let Err(error) = self.cmd_tx.try_send(Command::Terminate) {
            log::error!("ghostty_terminal: cmd_tx send Terminate failed: {error}");
        }
        crate::terminal::session::join_with_timeout(&mut self.handle, TERMINAL_THREAD_JOIN_TIMEOUT);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod input_output_tests;

#[cfg(test)]
mod snapshot_cache_unit_tests;
