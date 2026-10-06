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
    pub(crate) handle: Option<thread::JoinHandle<()>>,
    pub(crate) pty_write_responses: Arc<Mutex<Vec<Vec<u8>>>>,
    /// `pty_write()` 末次写入的字节，用于识别 `\r`/`\n` 跨写入块拆分，避免多余的 `\r\r\n`。
    pub(crate) last_pty_write_byte: u8,
    /// `Terminal::active_screen() == Alternate` 的无锁镜像，由 VT 线程每帧更新。
    /// 供 Android 输入路径免阻塞 RPC 检出备用屏（vim/less/htop），使触摸滚动
    /// 转发为滚轮转义序列而非滚动本地回滚（备用屏会吞掉滚轮）。
    pub(crate) alt_screen_active: Arc<AtomicBool>,
}

/// Terminate 投递的短时等待：try_send 在队列满时失败，但 VT 线程按批排空
/// 有界队列，短等待通常即可送达；超时后放弃，析构继续走限时 join。
const TERMINATE_SEND_TIMEOUT: Duration = Duration::from_millis(50);

/// 投递 `Command::Terminate` 并报告是否送达，`timeout` 为队列满时的等待上限。
///
/// 队列满是常态而非异常：VT 线程按批排空有界队列，一次 `try_send` 失败就放弃
/// 等于把终止推迟到信道彻底断开，`join` 随后超时、线程被 detach，回滚缓冲与
/// 终端存储的回收都被推到不确定的更晚时刻。故失败时给一次有界等待。
fn deliver_terminate(cmd_tx: &Sender<Command>, timeout: Duration) -> bool {
    match cmd_tx.try_send(Command::Terminate) {
        Ok(()) => true,
        Err(error) => {
            log::warn!(
                "ghostty_terminal: try_send Terminate failed ({error}), retrying with timeout"
            );
            cmd_tx.send_timeout(Command::Terminate, timeout).is_ok()
        }
    }
}

impl Drop for GhosttyTerminal {
    fn drop(&mut self) {
        if !deliver_terminate(&self.cmd_tx, TERMINATE_SEND_TIMEOUT) {
            log::error!("ghostty_terminal: failed to deliver Command::Terminate");
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
