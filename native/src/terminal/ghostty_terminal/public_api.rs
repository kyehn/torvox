use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use flume::{Sender, bounded};
use libghostty_vt::key;

use super::commands::{Command, Query, RunConfig};
use super::frame_mailbox::FrameMailbox;
#[cfg(test)]
use super::internal::CacheStats;
use super::types::*;

impl super::GhosttyTerminal {
    pub fn new(rows: u32, cols: u32, scrollback_lines: u32) -> Result<Self, TerminalError> {
        Self::new_with_theme(
            rows,
            cols,
            scrollback_lines,
            crate::terminal::theme::default_dark(),
            None,
        )
    }

    /// 以指定主题创建。主题是静态清单项（见 [`crate::terminal::theme`]），故取用
    /// 不复制调色板，换主题即换引用。
    pub fn new_with_theme(
        rows: u32,
        cols: u32,
        scrollback_lines: u32,
        theme: &'static crate::terminal::theme::Theme,
        default_cursor_style: Option<crate::terminal::ghostty_terminal::CursorStyle>,
    ) -> Result<Self, TerminalError> {
        Self::new_with_frame_wake(
            rows,
            cols,
            scrollback_lines,
            theme,
            None,
            default_cursor_style,
        )
    }

    /// [`Self::new_with_theme`] + 帧入队唤醒：VT 线程每推送一帧就调用一次
    /// `frame_wake`，让渲染线程不必靠空闲闭锁超时才发现新帧。
    pub fn new_with_frame_wake(
        rows: u32,
        cols: u32,
        scrollback_lines: u32,
        theme: &'static crate::terminal::theme::Theme,
        frame_wake: Option<crate::terminal::session::OutputWake>,
        default_cursor_style: Option<crate::terminal::ghostty_terminal::CursorStyle>,
    ) -> Result<Self, TerminalError> {
        Self::new_with_frame_wake_and_temp_dir(
            rows,
            cols,
            scrollback_lines,
            theme,
            frame_wake,
            Some(std::env::temp_dir()),
            default_cursor_style,
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
        default_cursor_style: Option<crate::terminal::ghostty_terminal::CursorStyle>,
    ) -> Result<Self, TerminalError> {
        let (cmd_tx, cmd_rx) = bounded::<Command>(COMMAND_CHANNEL_CAPACITY);
        let (query_tx, query_rx) = flume::bounded::<Query>(QUERY_CHANNEL_CAPACITY);
        let cell_data_mailbox = FrameMailbox::new();
        let cell_data_mailbox_for_run = cell_data_mailbox.clone();
        let (clipboard_tx, clipboard_rx) = bounded::<(String, String)>(EVENT_CHANNEL_CAPACITY);
        let pty_write_responses = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let pty_for_run = pty_write_responses.clone();
        let alt_screen_active = Arc::new(AtomicBool::new(false));
        let alt_screen_active_for_run = alt_screen_active.clone();
        let mouse_tracking_active = Arc::new(AtomicBool::new(false));
        let mouse_tracking_active_for_run = mouse_tracking_active.clone();
        let resize_failed = Arc::new(AtomicBool::new(false));
        let resize_failed_for_run = resize_failed.clone();
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
        let (kitty_stats_for_run, kitty_stats_for_self) = {
            let shared = CacheStats::default();
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
                        default_cursor_style,
                        response_buffer: pty_for_run,
                        alt_screen_active: alt_screen_active_for_run,
                        mouse_tracking_active: mouse_tracking_active_for_run,
                        resize_failed: resize_failed_for_run,
                        cell_size_px: cell_size_px_for_run,
                        cell_data_mailbox: cell_data_mailbox_for_run.clone(),
                        frame_wake,
                        clipboard_tx,
                        #[cfg(test)]
                        cache_stats: cache_stats_for_run.clone(),
                        #[cfg(test)]
                        kitty_stats: kitty_stats_for_run.clone(),
                    })
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
            handle: Some(handle),
            pty_write_responses,
            last_pty_write_byte: 0,
            alt_screen_active,
            mouse_tracking_active,
            resize_failed,
            latest_scroll_request_id: AtomicU64::new(0),
            #[cfg(test)]
            cache_stats: cache_stats_for_self,
            #[cfg(test)]
            kitty_stats: kitty_stats_for_self,
        })
    }

    /// 本终端的行级抽取计数：取出并清零，返回（复用行数，逐格行数）。
    #[cfg(test)]
    pub(crate) fn take_cache_stats(&self) -> (u64, u64) {
        self.cache_stats.take()
    }

    /// 本终端的占位符扫描计数：取出并清零，返回（复用行数，扫描行数）。
    #[cfg(test)]
    pub(crate) fn take_kitty_scan_stats(&self) -> (u64, u64) {
        self.kitty_stats.take()
    }

    pub fn drain_pty_write_responses(&self) -> Vec<Vec<u8>> {
        let mut guard = self
            .pty_write_responses
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        std::mem::take(&mut *guard)
    }

    pub fn poll_clipboard_event(&self) -> Option<(String, String)> {
        self.clipboard_rx.try_recv().ok()
    }

    pub fn vt_write(&mut self, data: &[u8]) {
        let sanitized = sanitize_vt_input(data);
        let mut buffer = Vec::with_capacity(data.len() + 4);
        buffer.extend_from_slice(&sanitized);
        // 分片直透：上游解析器在同一 Terminal 对象上跨调用保持状态，此处不得追加
        // ST/SGR 提前闭合（会截断合法跨块 OSC 并洗掉颜色）。用 try_send：VT 线程卡住时
        // 不得无限期阻塞调用方（与 pty_write 同策略）。
        if let Err(error) = self.cmd_tx.try_send(Command::Write(buffer)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// 写入 PTY 输出并把 LF（`\n`）转成 CR+LF（`\r\n`）——Ghostty 的 VT 引擎把 LF 当作
    /// 无回车的换行，不转换则常规输出的行推进会出错。
    ///
    /// 适用于 PTY 输出的文本级 `\n`→`\r\n` 转换；VT 控制序列、DEC 矩形操作与二进制
    /// VT 数据应改用 [`Self::vt_write`]。
    pub fn pty_write(&mut self, data: &[u8]) {
        let mut buffer = Vec::with_capacity(data.len() + 4);
        // 用上次调用的末字节识别跨块拆分的 `\r`/`\n`（Linux PTY 输出常见）；否则
        // LF→CRLF 转换会多插入一个 `\r`，产生 `\r\r\n`。
        let mut prev: u8 = self.last_pty_write_byte;
        for &raw in data {
            // 仅剔除 NUL（与 `vt_write` 同策略，二进制安全）：0xF8 以上字节在合法
            // UTF-8 中不会出现，但在 Kitty 直接 RGB 等二进制载荷中合法，改为空格
            // 等于静默损坏图像，此处直透交由上游解析器处理。
            if raw == 0x00 {
                continue;
            }
            // 裸 LF 转成 CRLF，但仅当其前一个字节不是 CR；否则本已含 CRLF 的输入
            // （PTY 输出常见）会变成 CRCRLF，多出一个回车。
            if raw == b'\n' && prev != b'\r' {
                buffer.push(b'\r');
            }
            buffer.push(raw);
            prev = raw;
        }
        // 分片直透：上游解析器在同一 Terminal 对象上跨调用保持状态，
        // CSI/OSC/DCS 分片由上游增量重组，此处不得提前闭合（ST 自动闭合
        // 会截断合法跨块 OSC，实测分片 OSC 52 被截为空内容）。
        self.last_pty_write_byte = prev;
        // 用 try_send：本方法在持有会话锁的会话/渲染路径上运行，命令通道满（VT 线程
        // 忙于长命令）时不得无限期阻塞调用方；丢弃一块可接受——VT 引擎是帧式的。
        if let Err(error) = self.cmd_tx.try_send(Command::Write(buffer)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
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

    pub fn flush(&self) {
        self.flush_with_timeout(std::time::Duration::from_secs(FLUSH_TIMEOUT_SECS));
    }

    /// 带期限的 `flush` 变体：锁内渲染路径（`poll_pty_output`）用零/极短期限，
    /// VT 线程卡住时跳过本次排空而非阻塞会话锁（写锁饥饿见 N1-20）。
    /// 返回真表示 VT 线程已确认排空，调用方才可收割回调事件与回写应答。
    pub fn flush_with_timeout(&self, timeout: std::time::Duration) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.try_send(Command::FlushAck(tx)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
            return false;
        }
        // 有界等待：VT 线程卡死（如病态的 C 解析器输入）时，无限 recv 会在持有会话锁
        // 期间永久阻塞调用方，冻结所有 JNI 入口并触发 ANR 看门狗。超时远大于合法积压
        // 上限，静默 5s 即 VT 线程确实卡死。
        match rx.recv_timeout(timeout) {
            Ok(()) => true,
            Err(_) => {
                log::warn!("ghostty_terminal: flush_ack timed out — session may be dead");
                false
            }
        }
    }

    /// 换主题。主题取自静态清单，故命令只携带引用，不复制调色板。
    pub fn set_theme(&self, theme: &'static crate::terminal::theme::Theme) {
        // try_send：与 resize 同非阻塞策略。
        if let Err(error) = self.cmd_tx.try_send(Command::SetTheme(theme)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// 用任意调色板覆盖主题色（`~/.termux/colors.properties`）。
    ///
    /// 调色板按值移交给 VT 线程：它是用户文件的一次性解析结果，生命周期
    /// 与本命令相同，没有可借用的静态源。
    pub fn set_colors(&self, palette: crate::terminal::theme::Palette) {
        if let Err(error) = self.cmd_tx.try_send(Command::SetColors(palette)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// 下发视口滚动命令（向上为负）。成功返回 true。
    ///
    /// 每次下发分配一个请求序号（进程级单调递增）：VT 处理命令后把序号随帧回传，
    /// 渲染侧据此判断收到的帧是否早于在途的滚动命令——在途期间到达的旧视口帧
    /// 不得呈现（旧视口配新余量会让内容先上跳半行再下跳整行）。
    pub fn scroll_viewport(&self, delta: isize) -> bool {
        let request_id = super::commands::next_scroll_request_id();
        if let Err(error) = self
            .cmd_tx
            .try_send(Command::ScrollViewport { delta, request_id })
        {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for scroll: {error}");
            return false;
        }
        // 只在确认送达后推进「最新请求序号」：未送达的序号不得让渲染侧把
        // 后续的帧判成陈旧（那会让滚动后的帧永远不被呈现）。
        self.latest_scroll_request_id
            .store(request_id, Ordering::Release);
        true
    }

    /// 最近一次**已送达**的滚动请求序号（0 = 从未下发过滚动命令）。
    ///
    /// 渲染侧拿它与帧携带的「VT 已处理的最后一个序号」比较：帧的序号更小即
    /// 说明该帧早于在途的滚动命令。
    pub fn latest_scroll_request_id(&self) -> u64 {
        self.latest_scroll_request_id.load(Ordering::Acquire)
    }

    /// VT 线程接受 resize 命令时为 true。为 false 表示网格**未**缩放（调用方的
    /// `pty.resize` 可能已更新 PTY）；调用方不得缓存新尺寸，以便下次 resize 事件重试。
    pub fn resize(&mut self, rows: u32, cols: u32) -> bool {
        // 用 try_send 而非 send：VT 线程卡住时不得阻塞调用方（switchSession 在此调用
        // 期间持有 Kotlin sessionLock）。注意：被丢弃的 resize **不会重放**——PTY/网格
        // 保持旧尺寸直到下次 resize 事件（输入法变化、旋转、设置变更、切换会话）。
        if let Err(error) = self.cmd_tx.try_send(Command::Resize { rows, cols }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for resize: {error}");
            return false;
        }
        true
    }

    /// 上一次入队的 resize 是否在 VT 线程上真的失败过（网格仍是旧尺寸）。
    /// 调用方据此让下一次 resize 事件重试，而非按「尺寸未变」短路。
    pub fn resize_failed(&self) -> bool {
        self.resize_failed.load(Ordering::Acquire)
    }

    /// 测试钩子：直接置位/清除 [Self::resize_failed]。
    pub fn set_resize_failed_for_test(&self, failed: bool) {
        self.resize_failed.store(failed, Ordering::Release);
    }

    pub fn set_cell_pixel_size(&self, cell_width: u32, cell_height: u32) {
        if let Err(error) = self.cmd_tx.try_send(Command::SetCellPixelSize {
            cell_width,
            cell_height,
        }) {
            log::warn!(
                "ghostty_terminal: cmd_tx full/dropped failed for set_cell_pixel_size: {error}"
            );
        }
    }

    pub fn reset(&self) {
        if let Err(error) = self.cmd_tx.try_send(Command::Reset) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for reset: {error}");
        }
    }

    /// 请求按当前状态重推一帧 CellData，不作任何状态改变。内容去重对显式请求不适用，
    /// 故去重基线在 VT 线程上被失效，帧必定到达（邮箱只留最新帧，重推覆盖旧帧）。
    pub fn request_repaint(&self) {
        if let Err(error) = self.cmd_tx.try_send(Command::Repaint) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for repaint: {error}");
        }
    }

    /// 安装终端持有的线性活动选区（跟踪引用，随滚动/输出/重排跟随文本）。
    /// 坐标为绝对网格行（0 = 回滚顶部）与列。
    /// try_send 非阻塞：VT 线程卡住时丢弃而非阻塞调用方（与 resize 同策略）。
    pub fn set_selection(&self, start: (u32, u32), end: (u32, u32)) {
        if let Err(error) = self.cmd_tx.try_send(Command::SetSelection { start, end }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for set_selection: {error}");
        }
    }

    pub fn clear_selection(&self) {
        if let Err(error) = self.cmd_tx.try_send(Command::ClearSelection) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for clear_selection: {error}");
        }
    }

    pub fn rows(&self) -> u32 {
        self.query(Query::Rows, DISCONNECTED_ROWS, "rows")
    }

    pub fn cols(&self) -> u32 {
        self.query(Query::Cols, DISCONNECTED_COLS, "cols")
    }

    /// 返回当前视口的**最新**网格快照：始终阻塞到 VT 线程处理完请求，调用方不会看到
    /// 陈旧缓存帧。VT 线程仅在网格实际变化时重建快照（见 `snapshot_needs_rebuild`），
    /// 故阻塞代价仅一次通道往返。
    ///
    /// 通道满或超时说明 VT 线程已卡死：记错误后 panic，不回退到空白网格。
    pub fn take_snapshot(&self) -> GridSnapshot {
        let (tx, rx): (Sender<Arc<GridSnapshot>>, _) = bounded(1);
        if let Err(error) = self.cmd_tx.try_send(Command::TakeSnapshot { tx }) {
            log::error!("ghostty_terminal: 快照命令入队失败（VT 线程已卡死）: {error}");
            panic!("ghostty_terminal: 快照命令入队失败: {error}");
        }
        let snapshot = rx
            .recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS))
            .unwrap_or_else(|error| {
                log::error!("ghostty_terminal: 等待 VT 线程快照超时: {error}");
                panic!("ghostty_terminal: 等待 VT 线程快照超时: {error}");
            });
        Arc::unwrap_or_clone(snapshot)
    }

    pub fn take_kitty_graphics_image(&self, image_id: u32) -> Option<KittyGraphicsImageData> {
        self.query(
            |tx| Query::TakeKittyGraphicsImage { id: image_id, tx },
            None,
            "take_kitty_graphics_image",
        )
    }

    pub fn take_kitty_placements(&self) -> Vec<KittyPlacementFrame> {
        Self::take_kitty_placements_on(&self.query_tx, None)
    }

    /// [`Self::take_kitty_placements`] 的通道外置版，理由同 [`Self::title_on`]。
    ///
    /// `row_signatures` 非空时按视口行复用 VT 侧扫描缓存（见 `Query::TakeKittyPlacements`），
    /// 纯滚动与滚动落定后的重查不再逐格走 FFI；传 `None` 则无条件全量扫描。
    pub(crate) fn take_kitty_placements_on(
        query_tx: &Sender<Query>,
        row_signatures: Option<std::sync::Arc<[u64]>>,
    ) -> Vec<KittyPlacementFrame> {
        Self::query_on(
            query_tx,
            |tx| Query::TakeKittyPlacements {
                row_signatures: row_signatures.clone(),
                tx,
            },
            Vec::new(),
            "take_kitty_placements",
        )
    }

    pub fn cursor_x(&self) -> u32 {
        self.query(Query::CursorX, DISCONNECTED_CURSOR_X, "cursor_x")
    }

    pub fn cursor_y(&self) -> u32 {
        self.query(Query::CursorY, DISCONNECTED_CURSOR_Y, "cursor_y")
    }

    pub fn cursor_visible(&self) -> bool {
        self.query(
            Query::CursorVisible,
            DISCONNECTED_CURSOR_VISIBLE,
            "cursor_visible",
        )
    }

    pub fn render_cursor(&self) -> Option<(u32, u32)> {
        Self::render_cursor_on(&self.query_tx)
    }

    /// [`Self::render_cursor`] 的通道外置版，理由同 [`Self::title_on`]。
    ///
    /// 这一条比其余查询更该走锁外：光标坐标取自 `build_cell_data`，即与渲染同源的
    /// **整网格**构建，耗时随网格面积走（忙时上限两次查询超时）。持着注册表读锁与
    /// 会话锁等它，等于让每次取光标都把按帧取这两把锁的渲染线程堵住。
    pub(crate) fn render_cursor_on(query_tx: &Sender<Query>) -> Option<(u32, u32)> {
        Self::query_on(query_tx, Query::RenderCursor, None, "render_cursor")
    }

    /// 取走邮箱里的最新一帧：邮箱只保留最新帧，故取到的必是当前画面。
    pub fn receive_cell_data(&self) -> Option<(Vec<CellData>, CursorInfo)> {
        self.cell_data_mailbox.take()
    }

    /// 用 Ghostty 鼠标编码器把鼠标事件（像素位置、动作、按键）编码为终端转义序列。
    /// `cell_width`/`cell_height` 取渲染器的实时单元格尺寸，使像素→单元映射与实际显示一致。
    /// `modifiers` 是上游 `key.Mods` 原始位（Shift/Ctrl/Alt/Super）。
    ///
    /// 鼠标上报未启用（无 DECSET 1000/1002/1003）或编码失败时返回 `Some(空)`，
    /// 由调用方丢弃该事件；仅查询通道卡死时返回 `None`。
    pub fn encode_mouse_event(
        &self,
        position: (f32, f32),
        action: u8,
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<Vec<u8>> {
        Self::encode_mouse_event_on(
            &self.query_tx,
            position,
            action,
            button,
            modifiers,
            cell_width,
            cell_height,
        )
    }

    /// [`Self::encode_mouse_event`] 的通道外置版：调用方在会话锁内克隆查询通道，
    /// 随后在锁外编码，使每次鼠标移动不持有会话锁与注册表锁等待 VT 应答。
    pub(crate) fn encode_mouse_event_on(
        query_tx: &Sender<Query>,
        position: (f32, f32),
        action: u8,
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodeMouseEvent {
                position,
                action,
                button,
                modifiers,
                cell_width,
                cell_height,
                tx,
            },
            Vec::new(),
            "encode_mouse_event",
        )
        .into()
    }

    /// 把 `repeat` 档滚轮编码为一次应答：每档一个 press + release 对。
    ///
    /// 触摸拖动一个事件可能滚过数行，逐档往返会把 UI 线程按在 VT 线程上
    /// （每档两次应答）；合并后一次事件只有一次往返，PTY 也只写一次。
    /// `button` 3 为向上（更旧）、4 为向下（更新）。鼠标上报未启用时返回空。
    pub fn encode_mouse_wheel(
        &self,
        position: (f32, f32),
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
        repeat: u32,
    ) -> Option<Vec<u8>> {
        Self::encode_mouse_wheel_on(
            &self.query_tx,
            position,
            button,
            modifiers,
            cell_width,
            cell_height,
            repeat,
        )
    }

    /// [`Self::encode_mouse_wheel`] 的通道外置版，语义同
    /// [`Self::encode_mouse_event_on`]。
    pub(crate) fn encode_mouse_wheel_on(
        query_tx: &Sender<Query>,
        position: (f32, f32),
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
        repeat: u32,
    ) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodeMouseWheel {
                position,
                button,
                modifiers,
                cell_width,
                cell_height,
                repeat: repeat.max(1),
                tx,
            },
            Vec::new(),
            "encode_mouse_wheel",
        )
        .into()
    }

    /// 用上游 `key::Encoder` 把按键事件编码为终端转义序列。
    ///
    /// 编码器的全部选项（DECCKM、keypad 应用模式、modifyOtherKeys、kitty 键盘
    /// 协议、backarrow key mode）在每次按键前从终端状态重取，故调用方无需查询
    /// 任何模式：应用改了 DECSET，本函数下一次调用即按新模式编码。
    ///
    /// `text` 是平台按当前布局与修饰生成的文本（Shift+A 的 `A`），无文本的键传
    /// `None`；`unshifted` 是该键无修饰时的码点，kitty 键盘协议据此区分档位。
    /// 单独的修饰键等不产生序列的键返回 `Some(空)`，由调用方丢弃。
    /// 仅查询通道卡死时返回 `None`。
    pub fn encode_key_event(
        &self,
        key: key::Key,
        action: key::Action,
        modifiers: u16,
        text: Option<char>,
        unshifted: Option<char>,
    ) -> Option<Vec<u8>> {
        Self::encode_key_event_on(&self.query_tx, key, action, modifiers, text, unshifted)
    }

    /// [`Self::encode_key_event`] 的通道外置版：调用方在会话锁内克隆查询通道，
    /// 随后在锁外编码，使每次击键不持有会话锁与注册表锁等待 VT 应答——否则重负载
    /// VT 线程下每次按键会连同按帧取这两把锁的渲染一起卡住一个查询超时。
    pub(crate) fn encode_key_event_on(
        query_tx: &Sender<Query>,
        key: key::Key,
        action: key::Action,
        modifiers: u16,
        text: Option<char>,
        unshifted: Option<char>,
    ) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodeKeyEvent {
                key,
                action,
                modifiers,
                text,
                unshifted,
                tx,
            },
            Vec::new(),
            "encode_key_event",
        )
        .into()
    }

    /// 用上游 `paste::encode` 把剪贴板文本编码为 PTY 写入字节。
    ///
    /// bracketed 标志取自终端模式 2004 的活体状态：启用时包裹并保留换行，
    /// 未启用时换行转回车；控制字节由上游中化。不取 `Terminal::paste`
    /// （其拒绝流程要求确认对话框，PROHIBITED 明禁）。
    ///
    /// 编码失败返回 `Some(空)`，由调用方丢弃本次粘贴；
    /// 仅查询通道卡死时返回 `None`。
    pub fn encode_paste(&self, data: &[u8]) -> Option<Vec<u8>> {
        Self::encode_paste_on(&self.query_tx, data)
    }

    /// [`Self::encode_paste`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    pub(crate) fn encode_paste_on(query_tx: &Sender<Query>, data: &[u8]) -> Option<Vec<u8>> {
        Self::query_on(
            query_tx,
            |tx| Query::EncodePaste {
                data: data.to_vec(),
                tx,
            },
            Vec::new(),
            "encode_paste",
        )
        .into()
    }

    pub fn mode_get_with_timeout(
        &self,
        mode: libghostty_vt::terminal::Mode,
        timeout: std::time::Duration,
    ) -> bool {
        Self::mode_get_on(&self.query_tx, mode, timeout)
    }

    /// [`Self::mode_get_with_timeout`] 的通道外置版，理由同 [`Self::title_on`]。
    /// 本查询自带超时参数（其余走 `query_on` 的统一期限），故不并入 `query_on`。
    pub(crate) fn mode_get_on(
        query_tx: &Sender<Query>,
        mode: libghostty_vt::terminal::Mode,
        timeout: std::time::Duration,
    ) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = query_tx.try_send(Query::ModeGet(mode, tx)) {
            // 发送失败即无人会应答：继续 recv 只会白等满整个 timeout，
            // 而调用方普遍持有会话锁，故立即返回（与 query() 同策略）。
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
            return false;
        }
        match rx.recv_timeout(timeout) {
            Ok(mode) => mode,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: mode_get({mode:?}) timed out or disconnected — returning false"
                );
                false
            }
        }
    }

    pub fn alt_screen_active_atomic(&self) -> bool {
        self.alt_screen_active.load(Ordering::Acquire)
    }

    /// 鼠标上报（DECSET 1000/1002/1003）是否启用。
    ///
    /// 镜像由 VT 线程每帧刷新，故本调用不碰查询通道、不等待 VT 线程——
    /// 触摸拖动的每个事件都要问一次，等一次应答就够卡住手势了。
    pub fn mouse_tracking_active_atomic(&self) -> bool {
        self.mouse_tracking_active.load(Ordering::Acquire)
    }

    /// 向 VT 线程发送无状态 [`Query`] 并等待应答，发送失败或超时回退到 `fallback`；
    /// 全部查询方法共用这唯一一处有界通道 + 超时样板。
    fn query<T>(&self, build: impl Fn(Sender<T>) -> Query, fallback: T, method: &str) -> T {
        Self::query_on(&self.query_tx, build, fallback, method)
    }

    /// [`Self::query`] 的通道外置版：调用方在会话锁内克隆查询通道，
    /// 随后在锁外执行长查询，使大回滚搜索不冻结按帧取锁的渲染。
    /// 查询重试次数：VT 线程正常在 `VT_IDLE_TICK_MS` 内应答，但系统过载（并行测试、
    /// 后台编译）时可能整体错过一个 `QUERY_TIMEOUT_MS` 窗口。重试一次可把「偶发
    /// 错过」与「VT 线程卡死」区分开：幂等查询的第二次尝试通常直接命中（读缓存
    /// 查询会命中上一轮已落盘的缓存，直读查询等到线程恢复后的第一次应答），后者
    /// 只会再等满一个期限后仍回退——调用方拿到的兜底值语义不变。
    /// 有副作用的动作型查询（如 `SearchSelect` 推进搜索游标）经
    /// [`Self::query_on_attempts`] 只发一次，防止滞留首查应答丢失后重发双跳。
    const QUERY_ATTEMPTS: u8 = 2;

    fn query_on<T>(
        query_tx: &Sender<Query>,
        build: impl Fn(Sender<T>) -> Query,
        fallback: T,
        method: &str,
    ) -> T {
        Self::query_on_attempts(query_tx, build, fallback, method, Self::QUERY_ATTEMPTS)
    }

    fn query_on_attempts<T>(
        query_tx: &Sender<Query>,
        build: impl Fn(Sender<T>) -> Query,
        fallback: T,
        method: &str,
        attempts: u8,
    ) -> T {
        for attempt in 1..=attempts {
            let (tx, rx) = bounded(1);
            if let Err(error) = query_tx.try_send(build(tx)) {
                log::warn!("ghostty_terminal: query_tx full/dropped failed for {method}: {error}");
                return fallback;
            }
            match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
                Ok(value) => return value,
                Err(error) if attempt < attempts => {
                    log::warn!(
                        "ghostty_terminal: {method} query attempt {attempt} hit deadline ({error:?}); retrying once"
                    );
                }
                Err(error) => {
                    log::warn!(
                        "ghostty_terminal: {method} timed out or disconnected after {attempts} attempt(s) ({error:?}) — returning fallback"
                    );
                }
            }
        }
        fallback
    }

    /// 克隆 VT 查询通道（见 [`Self::query_on`]）。
    pub(crate) fn query_channel(&self) -> Sender<Query> {
        self.query_tx.clone()
    }

    pub fn title(&self) -> String {
        Self::title_on(&self.query_tx)
    }

    /// [`Self::title`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    /// 标题随每次状态更新从 Kotlin 侧拉取，频率与击键同级。
    pub(crate) fn title_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(
            query_tx,
            Query::Title,
            DISCONNECTED_TITLE.to_string(),
            "title",
        )
    }

    /// 终端从 OSC 7 记录的工作目录（原始值，通常是 `file://host/path`）。
    ///
    /// shell 在每次提示符前上报；空串表示尚未上报，调用方不得把它当成「根目录」。
    pub fn pwd(&self) -> String {
        Self::pwd_on(&self.query_tx)
    }

    /// [`Self::pwd`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn pwd_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(query_tx, Query::Pwd, DISCONNECTED_PWD.to_string(), "pwd")
    }

    pub fn scrollback_length(&self) -> u32 {
        Self::scrollback_length_on(&self.query_tx)
    }

    /// [`Self::scrollback_length`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn scrollback_length_on(query_tx: &Sender<Query>) -> u32 {
        Self::query_on(
            query_tx,
            Query::ScrollbackLength,
            DISCONNECTED_SCROLLBACK,
            "scrollback_length",
        )
    }

    pub fn read_line_text(&self, row: u32) -> Option<String> {
        Self::read_line_text_on(&self.query_tx, row)
    }

    /// [`Self::read_line_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn read_line_text_on(query_tx: &Sender<Query>, row: u32) -> Option<String> {
        Self::query_on(
            query_tx,
            |tx| Query::ReadLineText { row, tx },
            None,
            "read_line_text",
        )
    }

    pub fn read_visible_text(&self) -> String {
        Self::read_visible_text_on(&self.query_tx)
    }

    /// [`Self::read_visible_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn read_visible_text_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(
            query_tx,
            Query::ReadVisibleText,
            String::new(),
            "read_visible_text",
        )
    }

    /// 视口 + 回滚区全量文本。`dump_grid` 的等价替代：调用方只需字符，无需样式。
    pub fn read_all_text(&self) -> String {
        Self::read_all_text_on(&self.query_tx)
    }

    /// [`Self::read_all_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn read_all_text_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(query_tx, Query::ReadAllText, String::new(), "read_all_text")
    }

    pub fn selection_text(&self, start: (u32, u32), end: (u32, u32)) -> String {
        Self::selection_text_on(&self.query_tx, start, end)
    }

    /// [`Self::selection_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn selection_text_on(
        query_tx: &Sender<Query>,
        start: (u32, u32),
        end: (u32, u32),
    ) -> String {
        Self::query_on(
            query_tx,
            |tx| Query::SelectionText { start, end, tx },
            String::new(),
            "selection_text",
        )
    }

    pub fn select_word_at(&self, row: u32, col: u32) -> Option<((u32, u32), (u32, u32))> {
        Self::select_word_at_on(&self.query_tx, row, col)
    }

    /// [`Self::select_word_at`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn select_word_at_on(
        query_tx: &Sender<Query>,
        row: u32,
        col: u32,
    ) -> Option<((u32, u32), (u32, u32))> {
        Self::query_on(
            query_tx,
            |tx| Query::SelectWordAt { row, col, tx },
            None,
            "select_word_at",
        )
    }

    /// 该行中作为宽字符后半格的列号，升序。查询失败时为空 vec（等价于无吸附）：
    /// 保持原列，不猜。
    pub fn wide_char_tail_cols(&self, row: u32) -> Vec<u32> {
        Self::wide_char_tail_cols_on(&self.query_tx, row)
    }

    /// [`Self::wide_char_tail_cols`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    /// 本方法每次拖动手柄都调用，故不能持双锁等 VT 应答。
    pub(crate) fn wide_char_tail_cols_on(query_tx: &Sender<Query>, row: u32) -> Vec<u32> {
        Self::query_on(
            query_tx,
            |tx| Query::WideCharTailCols { row, tx },
            Vec::new(),
            "wide_char_tail_cols",
        )
    }

    pub fn select_all(&self) -> Option<((u32, u32), (u32, u32))> {
        Self::select_all_on(&self.query_tx)
    }

    /// [`Self::select_all`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    pub(crate) fn select_all_on(query_tx: &Sender<Query>) -> Option<((u32, u32), (u32, u32))> {
        Self::query_on(query_tx, |tx| Query::SelectAll { tx }, None, "select_all")
    }

    /// 查询网格单元处的 OSC 8 超链接 URI（行 0 = 回滚顶部，与 scrollbackLine 一致）；
    /// 无链接时为 None。
    pub fn hyperlink_at(&self, row: u32, col: u32) -> Option<String> {
        Self::hyperlink_at_on(&self.query_tx, row, col)
    }

    /// [`Self::hyperlink_at`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn hyperlink_at_on(query_tx: &Sender<Query>, row: u32, col: u32) -> Option<String> {
        Self::query_on(
            query_tx,
            |tx| Query::HyperlinkAt { row, col, tx },
            None,
            "hyperlink_at",
        )
    }

    /// 设置搜索 needle 并同步跑完搜索，返回命中总数；空 needle 清除搜索。
    ///
    /// 匹配、软换行跨行与增量同步均由上游 `search::Search` 承担，搜索对象随
    /// VT 线程存活，故连续查询复用同一份索引状态。
    pub fn search_set_needle(&self, needle: &str) -> usize {
        Self::search_set_needle_on(&self.query_tx, needle)
    }

    /// [`Self::search_set_needle`] 的通道外置版（见 [`Self::query_on`]）。
    pub(crate) fn search_set_needle_on(query_tx: &Sender<Query>, needle: &str) -> usize {
        Self::query_on(
            query_tx,
            |tx| Query::SearchSetNeedle {
                needle: needle.to_string(),
                tx,
            },
            0,
            "search_set_needle",
        )
    }

    /// 选中下一个（`forward = true`，朝更旧内容）或上一个命中，返回选中态。
    /// 无命中时 None；上游不移动视口（`Scroll::None`），居中由宿主完成。
    pub fn search_select(&self, forward: bool) -> Option<SearchSelection> {
        Self::search_select_on(&self.query_tx, forward)
    }

    /// [`Self::search_select`] 的通道外置版（见 [`Self::query_on`]）。
    /// 推进搜索游标非幂等，故只发一次。
    pub(crate) fn search_select_on(
        query_tx: &Sender<Query>,
        forward: bool,
    ) -> Option<SearchSelection> {
        Self::query_on_attempts(
            query_tx,
            |tx| Query::SearchSelect { forward, tx },
            None,
            "search_select",
            1,
        )
    }

    /// 绘制视口高亮所需的全部输入（视口命中、选中命中、回滚长度）。
    pub fn search_viewport_highlights(&self) -> SearchViewportHighlights {
        Self::search_viewport_highlights_on(&self.query_tx)
    }

    /// [`Self::search_viewport_highlights`] 的通道外置版（见 [`Self::query_on`]）。
    pub(crate) fn search_viewport_highlights_on(
        query_tx: &Sender<Query>,
    ) -> SearchViewportHighlights {
        Self::query_on(
            query_tx,
            |tx| Query::SearchViewportHighlights { tx },
            SearchViewportHighlights {
                matches: Vec::new(),
                selected: Vec::new(),
                scrollback_lines: 0,
            },
            "search_viewport_highlights",
        )
    }

    pub fn dump_grid(&self) -> DumpedGrid {
        self.query(
            |tx| Query::DumpGrid { tx },
            DumpedGrid {
                rows: 0,
                cols: 0,
                visible: Vec::new(),
                scrollback: Vec::new(),
            },
            "dump_grid",
        )
    }
}

/// `vt_write` 的输入清洗：只剔除 NUL。
///
/// NUL 是 ECMA-48 忽略控制字符，必须在解析器看到前剔除，否则一个游离 NUL
///（mpv --vo=kitty 每帧追加一个）就会毁掉整块载荷，甚至让整张 Kitty 图像丢失。
/// 0xF8–0xFF 不在此处理：合法 UTF-8 里本就不会出现（4 字节上限 F4），而原始二进制
/// VT 载荷（如 Kitty `m=1` 直接 RGB）里它们是合法数据——静默替换为空格等于静默损坏
/// 图像。此函数是 `vt_write` 承诺的二进制安全路径的兑现点。
pub(crate) fn sanitize_vt_input(data: &[u8]) -> Vec<u8> {
    data.iter().filter(|&&b| b != 0x00).copied().collect()
}
