use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use flume::{Sender, bounded};

use super::commands::{Command, Query, RunConfig};
use super::types::*;

impl super::GhosttyTerminal {
    pub fn new(rows: u32, cols: u32, scrollback_lines: u32) -> Result<Self, TerminalError> {
        let (ansi, background, foreground) = Self::catppuccin_mocha_palette();
        Self::new_with_theme(rows, cols, scrollback_lines, background, foreground, ansi)
    }

    pub fn catppuccin_mocha_palette() -> ([[u8; 3]; 16], [u8; 3], [u8; 3]) {
        let ansi = [
            [24, 24, 37],
            [243, 139, 168],
            [166, 227, 161],
            [249, 226, 175],
            [137, 180, 250],
            [203, 166, 247],
            [148, 226, 213],
            [205, 214, 244],
            [108, 112, 134],
            [243, 139, 168],
            [166, 227, 161],
            [249, 226, 175],
            [137, 180, 250],
            [203, 166, 247],
            [148, 226, 213],
            [187, 194, 222],
        ];
        (ansi, [30, 30, 46], [205, 214, 244])
    }

    pub fn new_with_theme(
        rows: u32,
        cols: u32,
        scrollback_lines: u32,
        initial_background: [u8; 3],
        initial_foreground: [u8; 3],
        initial_ansi: [[u8; 3]; 16],
    ) -> Result<Self, TerminalError> {
        let (cmd_tx, cmd_rx) = bounded::<Command>(COMMAND_CHANNEL_CAPACITY);
        let (query_tx, query_rx) = flume::bounded::<Query>(QUERY_CHANNEL_CAPACITY);
        let (cell_data_tx, cell_data_rx) =
            flume::bounded::<(Vec<CellData>, CursorInfo)>(CELL_DATA_CHANNEL_CAPACITY);
        let (clipboard_tx, clipboard_rx) = bounded::<(String, String)>(EVENT_CHANNEL_CAPACITY);
        let (bell_tx, bell_rx) = bounded::<()>(EVENT_CHANNEL_CAPACITY);
        let pty_write_responses = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let pty_for_run = pty_write_responses.clone();
        let panicked = Arc::new(AtomicBool::new(false));
        let panicked_for_run = panicked.clone();
        let alt_screen_active = Arc::new(AtomicBool::new(false));
        let alt_screen_active_for_run = alt_screen_active.clone();
        let cell_size_px_for_run = Arc::new((
            AtomicU32::new(DEFAULT_CELL_WIDTH),
            AtomicU32::new(DEFAULT_CELL_HEIGHT),
        ));
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
                        background_color: initial_background,
                        foreground_color: initial_foreground,
                        ansi_colors: initial_ansi,
                        response_buffer: pty_for_run,
                        alt_screen_active: alt_screen_active_for_run,
                        cell_size_px: cell_size_px_for_run,
                        cell_data_tx: Some(cell_data_tx),
                        clipboard_tx,
                        bell_tx,
                    })
                }));
                if let Err(panic) = result {
                    let msg = panic
                        .downcast_ref::<String>()
                        .map(|s| s.as_str())
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap_or("unknown panic payload");
                    log::error!("ghostty_terminal thread panicked: {msg}");
                    // 标记后续所有操作失败，避免调用方向死信道静默发命令。
                    panicked_for_run.store(true, Ordering::Release);
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
            cell_data_rx: Some(cell_data_rx),
            clipboard_rx,
            bell_rx,
            handle: Some(handle),
            pty_write_responses,
            panicked,
            last_pty_write_byte: 0,
            alt_screen_active,
        })
    }

    pub fn drain_pty_write_responses(&self) -> Vec<Vec<u8>> {
        let mut guard = self
            .pty_write_responses
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *guard)
    }

    pub fn poll_clipboard_event(&self) -> Option<(String, String)> {
        self.clipboard_rx.try_recv().ok()
    }

    pub fn poll_bell_event(&self) -> Option<()> {
        self.bell_rx.try_recv().ok()
    }

    pub fn vt_write(&mut self, data: &[u8]) {
        let sanitized = sanitize_vt_input(data);
        let mut buf = Vec::with_capacity(data.len() + 4);
        buf.extend_from_slice(&sanitized);
        // 分片直透：上游解析器在同一 Terminal 对象上跨调用保持状态，此处不得追加
        // ST/SGR 提前闭合（会截断合法跨块 OSC 并洗掉颜色）。用 try_send：VT 线程卡住时
        // 不得无限期阻塞调用方（与 pty_write 同策略）。
        if let Err(error) = self.cmd_tx.try_send(Command::Write(buf)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// 写入 PTY 输出并把 LF（`\n`）转成 CR+LF（`\r\n`）——Ghostty 的 VT 引擎把 LF 当作
    /// 无回车的换行，不转换则常规输出的行推进会出错。
    ///
    /// 适用于 PTY 输出的文本级 `\n`→`\r\n` 转换；VT 控制序列、DEC 矩形操作与二进制
    /// VT 数据应改用 [`Self::vt_write`]。
    pub fn pty_write(&mut self, data: &[u8]) {
        let mut buf = Vec::with_capacity(data.len() + 4);
        // 用上次调用的末字节识别跨块拆分的 `\r`/`\n`（Linux PTY 输出常见）；否则
        // LF→CRLF 转换会多插入一个 `\r`，产生 `\r\r\n`。
        let mut prev: u8 = self.last_pty_write_byte;
        for &raw in data {
            if raw == 0x00 {
                continue;
            }
            let sanitized = if raw > 0xF7 { b' ' } else { raw };
            // 裸 LF 转成 CRLF，但仅当其前一个字节不是 CR；否则本已含 CRLF 的输入
            // （PTY 输出常见）会变成 CRCRLF，多出一个回车。
            if sanitized == b'\n' && prev != b'\r' {
                buf.push(b'\r');
            }
            buf.push(sanitized);
            prev = sanitized;
        }
        // 分片直透：上游解析器在同一 Terminal 对象上跨调用保持状态，
        // CSI/OSC/DCS 分片由上游增量重组，此处不得提前闭合（ST 自动闭合
        // 会截断合法跨块 OSC，实测分片 OSC 52 被截为空内容）。
        self.last_pty_write_byte = prev;
        // 用 try_send：本方法在持有会话锁的会话/渲染路径上运行，命令通道满（VT 线程
        // 忙于长命令）时不得无限期阻塞调用方；丢弃一块可接受——VT 引擎是帧式的。
        if let Err(error) = self.cmd_tx.try_send(Command::Write(buf)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.panicked.load(Ordering::Acquire) && !self.cmd_tx.is_disconnected()
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

    pub fn set_theme(&self, background: [u8; 3], foreground: [u8; 3], ansi: [[u8; 3]; 16]) {
        // try_send：与 resize 同非阻塞策略。
        if let Err(error) = self.cmd_tx.try_send(Command::SetTheme {
            background,
            foreground,
            ansi,
        }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    pub fn scroll_viewport(&self, delta: isize) -> bool {
        if let Err(error) = self.cmd_tx.try_send(Command::ScrollViewport(delta)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for scroll: {error}");
            return false;
        }
        true
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
        self.query(
            |tx| Query::TakeKittyPlacements { tx },
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
        self.query(Query::RenderCursor, None, "render_cursor")
    }

    pub fn receive_cell_data(&self) -> Option<(Vec<CellData>, CursorInfo)> {
        let rx = self.cell_data_rx.as_ref()?;
        let mut latest = rx.try_recv().ok()?;
        while let Ok(next) = rx.try_recv() {
            latest = next;
        }
        Some(latest)
    }

    pub fn key_encode(
        &self,
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Option<Vec<u8>> {
        let rx =
            self.key_encode_submit(key_code, modifiers, action, unicode_char, unshifted_char)?;
        // 有界等待：VT 线程卡住时不得永久阻塞调用方（可能是 UI 线程）。
        rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS))
            .ok()
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
        self.query(
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

    pub fn key_encode_submit(
        &self,
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Option<flume::Receiver<Vec<u8>>> {
        let (tx, rx) = flume::bounded(1);
        if let Err(error) = self.query_tx.try_send(Query::KeyEncode {
            key_code,
            modifiers,
            action,
            unicode_char,
            unshifted_char,
            tx,
        }) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed for key_encode: {error}");
            return None;
        }
        Some(rx)
    }

    pub fn mode_get(&self, mode_num: u16, kind: u8) -> bool {
        self.mode_get_with_timeout(
            mode_num,
            kind,
            std::time::Duration::from_millis(QUERY_TIMEOUT_MS),
        )
    }

    pub fn mode_get_with_timeout(
        &self,
        mode_num: u16,
        kind: u8,
        timeout: std::time::Duration,
    ) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Query::ModeGet(mode_num, kind, tx)) {
            // 发送失败即无人会应答：继续 recv 只会白等满整个 timeout，
            // 而调用方普遍持有会话锁，故立即返回（与 query() 同策略）。
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
            return false;
        }
        match rx.recv_timeout(timeout) {
            Ok(mode) => mode,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: mode_get({mode_num}, {kind}) timed out or disconnected — returning false"
                );
                false
            }
        }
    }

    pub fn alt_screen_active_atomic(&self) -> bool {
        self.alt_screen_active.load(Ordering::Acquire)
    }

    /// 向 VT 线程发送无状态 [`Query`] 并等待应答，发送失败或超时回退到 `fallback`；
    /// 全部查询方法共用这唯一一处有界通道 + 超时样板。
    fn query<T>(&self, build: impl FnOnce(Sender<T>) -> Query, fallback: T, method: &str) -> T {
        Self::query_on(&self.query_tx, build, fallback, method)
    }

    /// [`Self::query`] 的通道外置版：调用方在会话锁内克隆查询通道，
    /// 随后在锁外执行长查询，使大回滚搜索不冻结按帧取锁的渲染（R21-T1）。
    fn query_on<T>(
        query_tx: &Sender<Query>,
        build: impl FnOnce(Sender<T>) -> Query,
        fallback: T,
        method: &str,
    ) -> T {
        let (tx, rx) = bounded(1);
        if let Err(error) = query_tx.try_send(build(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed for {method}: {error}");
            return fallback;
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(value) => value,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: {method} timed out or disconnected — returning fallback"
                );
                fallback
            }
        }
    }

    /// 克隆 VT 查询通道（见 [`Self::query_on`]）。
    pub(crate) fn query_channel(&self) -> Sender<Query> {
        self.query_tx.clone()
    }

    pub fn title(&self) -> String {
        self.query(Query::Title, DISCONNECTED_TITLE.to_string(), "title")
    }

    pub fn scrollback_length(&self) -> u32 {
        self.query(
            Query::ScrollbackLength,
            DISCONNECTED_SCROLLBACK,
            "scrollback_length",
        )
    }

    pub fn read_line_text(&self, row: u32) -> Option<String> {
        self.query(|tx| Query::ReadLineText { row, tx }, None, "read_line_text")
    }

    pub fn read_visible_text(&self) -> String {
        self.query(Query::ReadVisibleText, String::new(), "read_visible_text")
    }

    pub fn selection_text(&self, start: (u32, u32), end: (u32, u32)) -> String {
        self.query(
            |tx| Query::SelectionText { start, end, tx },
            String::new(),
            "selection_text",
        )
    }

    pub fn select_word_at(&self, row: u32, col: u32) -> Option<((u32, u32), (u32, u32))> {
        self.query(
            |tx| Query::SelectWordAt { row, col, tx },
            None,
            "select_word_at",
        )
    }

    pub fn select_line_at(&self, row: u32, col: u32) -> Option<((u32, u32), (u32, u32))> {
        self.query(
            |tx| Query::SelectLineAt { row, col, tx },
            None,
            "select_line_at",
        )
    }

    /// 该行中作为宽字符后半格的列号，升序。查询失败时为空 vec（等价于无吸附）：
    /// 保持原列，不猜。
    pub fn wide_char_tail_cols(&self, row: u32) -> Vec<u32> {
        self.query(
            |tx| Query::WideCharTailCols { row, tx },
            Vec::new(),
            "wide_char_tail_cols",
        )
    }

    pub fn select_all(&self) -> Option<((u32, u32), (u32, u32))> {
        self.query(|tx| Query::SelectAll { tx }, None, "select_all")
    }

    /// 查询网格单元处的 OSC 8 超链接 URI（行 0 = 回滚顶部，与 scrollbackLine 一致）；
    /// 无链接时为 None。
    pub fn hyperlink_at(&self, row: u32, col: u32) -> Option<String> {
        self.query(
            |tx| Query::HyperlinkAt { row, col, tx },
            None,
            "hyperlink_at",
        )
    }

    pub fn search_all_in_scrollback(&self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        Self::search_all_in_scrollback_on(&self.query_tx, query, case_sensitive)
    }

    /// [`Self::search_all_in_scrollback`] 的通道外置版（见 [`Self::query_on`]）。
    pub(crate) fn search_all_in_scrollback_on(
        query_tx: &Sender<Query>,
        query: &str,
        case_sensitive: bool,
    ) -> Vec<SearchMatch> {
        Self::query_on(
            query_tx,
            |tx| Query::SearchInScrollbackAll {
                query: query.to_string(),
                case_sensitive,
                tx,
            },
            Vec::new(),
            "search_all_in_scrollback",
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
