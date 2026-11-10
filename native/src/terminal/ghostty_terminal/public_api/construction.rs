//! 构造与生命周期：启动 VT 线程、排空写入、判定线程存活。

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::{Arc, Mutex};
use std::thread;

use flume::bounded;

use super::super::GhosttyTerminal;
use super::super::commands::{Command, Query, RunConfig};
use super::super::frame_mailbox::FrameMailbox;
#[cfg(test)]
use super::super::internal::{BatchStats, CacheStats};
use super::super::notification_queue::NotificationQueue;
use super::super::types::{
    COMMAND_CHANNEL_CAPACITY, DEFAULT_CELL_HEIGHT, DEFAULT_CELL_WIDTH, EVENT_CHANNEL_CAPACITY,
    FLUSH_TIMEOUT_SECS, QUERY_CHANNEL_CAPACITY, TerminalError, VT_READY_TIMEOUT_MS,
};

impl GhosttyTerminal {
    /// 新建 VT 引擎：启动 VT 线程并设定网格尺寸与回滚行数。
    pub fn new(rows: u32, cols: u32, scrollback_lines: u32) -> Result<Self, TerminalError> {
        Self::new_with_theme(
            rows,
            cols,
            scrollback_lines,
            crate::terminal::theme::default_dark(),
        )
    }

    /// 以指定主题创建。主题是静态清单项（见 [`crate::terminal::theme`]），故取用
    /// 不复制调色板，换主题即换引用。
    pub fn new_with_theme(
        rows: u32,
        cols: u32,
        scrollback_lines: u32,
        theme: &'static crate::terminal::theme::Theme,
    ) -> Result<Self, TerminalError> {
        Self::new_with_frame_wake(rows, cols, scrollback_lines, theme, None)
    }

    /// [`Self::new_with_theme`] + 帧入队唤醒：VT 线程每推送一帧就调用一次
    /// `frame_wake`，让渲染线程不必靠空闲闭锁超时才发现新帧。
    pub fn new_with_frame_wake(
        rows: u32,
        cols: u32,
        scrollback_lines: u32,
        theme: &'static crate::terminal::theme::Theme,
        frame_wake: Option<crate::terminal::session::OutputWake>,
    ) -> Result<Self, TerminalError> {
        Self::new_with_frame_wake_and_temp_dir(
            rows,
            cols,
            scrollback_lines,
            theme,
            frame_wake,
            Some(std::env::temp_dir()),
        )
    }

    /// [`Self::new_with_frame_wake`] + Kitty 文件介质白名单目录（生产会话传
    /// `$PREFIX/tmp`，与子进程 `TMPDIR` 同源）。
    pub fn new_with_frame_wake_and_temp_dir(
        rows: u32,
        cols: u32,
        scrollback_lines: u32,
        theme: &'static crate::terminal::theme::Theme,
        frame_wake: Option<crate::terminal::session::OutputWake>,
        kitty_temp_dir: Option<std::path::PathBuf>,
    ) -> Result<Self, TerminalError> {
        let (cmd_tx, cmd_rx) = bounded::<Command>(COMMAND_CHANNEL_CAPACITY);
        let (query_tx, query_rx) = flume::bounded::<Query>(QUERY_CHANNEL_CAPACITY);
        let cell_data_mailbox = FrameMailbox::new();
        let cell_data_mailbox_for_run = Arc::clone(&cell_data_mailbox);
        let (clipboard_tx, clipboard_rx) = bounded::<(String, String)>(EVENT_CHANNEL_CAPACITY);
        let notification_queue = Arc::new(NotificationQueue::new());
        let notification_queue_for_run = Arc::clone(&notification_queue);
        let pty_write_responses = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let pty_for_run = Arc::clone(&pty_write_responses);
        let alt_screen_active = Arc::new(AtomicBool::new(false));
        let alt_screen_active_for_run = Arc::clone(&alt_screen_active);
        let mouse_tracking_active = Arc::new(AtomicBool::new(false));
        let mouse_tracking_active_for_run = Arc::clone(&mouse_tracking_active);
        let bell_flag = Arc::new(AtomicBool::new(false));
        let bell_flag_for_run = Arc::clone(&bell_flag);
        let resize_failed = Arc::new(AtomicBool::new(false));
        let resize_failed_for_run = Arc::clone(&resize_failed);
        let vt_busy_since_nanos = Arc::new(AtomicU64::new(0));
        let vt_busy_since_nanos_for_run = Arc::clone(&vt_busy_since_nanos);
        let cell_size_px_for_run = Arc::new((
            AtomicU32::new(DEFAULT_CELL_WIDTH),
            AtomicU32::new(DEFAULT_CELL_HEIGHT),
        ));
        // 行级计数句柄：闭包 `move` 取一份，构造体留一份（内部是 `Arc`，
        // 两份副本共享同一组原子量）。生产构建无此变量。
        #[cfg(test)]
        let (cache_stats_for_run, cache_stats_for_self) = {
            let shared = CacheStats::default();
            (shared.clone(), shared)
        };
        #[cfg(test)]
        let (batch_stats_for_run, batch_stats_for_self) = {
            let shared = BatchStats::default();
            (shared.clone(), shared)
        };
        // 就绪握手：`thread::spawn` 返回时新线程可能尚未被调度，此时构造完成、
        // 调用方立刻发查询，查询会在 QUERY_TIMEOUT_MS 内无人应答而回退到
        // DISCONNECTED_*（表现为「spawn 完马上 resize 却拿到旧网格」的竞态）。
        // 零容量同步通道：VT 线程一进入闭包就握手成功。
        let (vt_ready_tx, vt_ready_rx) = std::sync::mpsc::sync_channel(0);
        let handle = thread::Builder::new()
            .name("ghostty-terminal".into())
            .spawn(move || {
                // 先握手再进 run：此时命令/查询通道已就位，run 一进入 select 即可应答。
                let _ = vt_ready_tx.send(());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Self::run(RunConfig {
                        command_receiver: cmd_rx,
                        query_receiver: query_rx,
                        rows,
                        cols,
                        scrollback_lines,
                        theme,
                        kitty_temp_dir,
                        response_buffer: pty_for_run,
                        alt_screen_active: alt_screen_active_for_run,
                        mouse_tracking_active: mouse_tracking_active_for_run,
                        bell_flag: bell_flag_for_run,
                        resize_failed: resize_failed_for_run,
                        vt_busy_since_nanos: vt_busy_since_nanos_for_run,
                        cell_size_px: cell_size_px_for_run,
                        cell_data_mailbox: Arc::clone(&cell_data_mailbox_for_run),
                        frame_wake,
                        clipboard_tx,
                        notification_queue: notification_queue_for_run,
                        #[cfg(test)]
                        cache_stats: cache_stats_for_run.clone(),
                        #[cfg(test)]
                        batch_stats: batch_stats_for_run.clone(),
                    });
                }));
                if let Err(panic) = result {
                    let panic_message = crate::panic_text(panic.as_ref());
                    // 接收端随 run 的栈帧一同析构，命令/查询通道随即断开，
                    // 后续操作在入队处显式失败——无需另设标志位。
                    log::error!("ghostty_terminal thread panicked: {panic_message}");
                }
            })
            .map_err(TerminalError::Spawn)?;

        // 握手超时说明 VT 线程连闭包都没进入：此时任何查询都必然回退到断开值，
        // 与其把一个注定答不出查询的会话交给调用方，不如在此显式失败。
        if vt_ready_rx
            .recv_timeout(std::time::Duration::from_millis(VT_READY_TIMEOUT_MS))
            .is_err()
        {
            log::error!("ghostty_terminal: VT thread did not start within {VT_READY_TIMEOUT_MS}ms");
            return Err(TerminalError::Spawn(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "ghostty VT thread did not start",
            )));
        }

        Ok(Self {
            cmd_tx,
            query_tx,
            cell_data_mailbox,
            clipboard_rx,
            notification_queue,
            handle: Some(handle),
            pty_write_responses,
            last_pty_write_byte: 0,
            alt_screen_active,
            mouse_tracking_active,
            bell_flag,
            resize_failed,
            vt_busy_since_nanos,
            latest_scroll_request_id: AtomicU64::new(0),
            cmd_drop_log: crate::slow_log::CountedLogThrottle::new(),
            #[cfg(test)]
            cache_stats: cache_stats_for_self,
            #[cfg(test)]
            batch_stats: batch_stats_for_self,
        })
    }

    /// 本终端的行级抽取计数：取出并清零，返回（复用行数，逐格行数）。
    #[cfg(test)]
    pub(crate) fn take_cache_stats(&self) -> (u64, u64) {
        self.cache_stats.take()
    }

    /// 本终端的批大小计数：取出并清零，返回（批数，最大批条数）。
    #[cfg(test)]
    pub(crate) fn take_batch_stats(&self) -> (u64, u64) {
        self.batch_stats.take()
    }

    /// VT 线程是否仍在：命令通道断开（含线程 panic 后接收端析构）即视为已死。
    #[cfg(test)]
    pub fn is_alive(&self) -> bool {
        !self.cmd_tx.is_disconnected()
    }

    #[cfg(test)]
    pub(crate) fn disconnect_for_test(&mut self) {
        let (dud_tx, _dud_rx) = bounded::<Command>(1);
        self.cmd_tx = dud_tx;
    }

    /// 等待 VT 线程排空此前写入的全部内容。
    pub fn flush(&self) {
        self.flush_with_timeout(std::time::Duration::from_secs(FLUSH_TIMEOUT_SECS));
    }

    /// 带期限的 `flush` 变体：锁内渲染路径（`poll_pty_output`）用零/极短期限，
    /// VT 线程卡住时跳过本次排空而非阻塞会话锁（写锁饥饿见 N1-20）。
    /// 返回真表示 VT 线程已确认排空，调用方才可收割回调事件与回写应答。
    pub fn flush_with_timeout(&self, timeout: std::time::Duration) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.try_send(Command::FlushAck(tx)) {
            self.warn_command_dropped("flush", &error);
            return false;
        }
        // 有界等待：VT 线程卡死（如病态的 C 解析器输入）时，无限 recv 会在持有会话锁
        // 期间永久阻塞调用方，冻结所有 JNI 入口并触发 ANR 看门狗。超时远大于合法积压
        // 上限，静默 5s 即 VT 线程确实卡死。
        if let Ok(()) = rx.recv_timeout(timeout) {
            true
        } else {
            log::warn!("ghostty_terminal: flush_ack timed out — session may be dead");
            false
        }
    }
}
