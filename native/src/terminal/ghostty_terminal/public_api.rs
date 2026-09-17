use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use flume::{Sender, bounded};

use super::commands::{Command, Query, RunConfig, SnapshotCache};
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
        let (cwd_tx, cwd_rx) = bounded::<String>(EVENT_CHANNEL_CAPACITY);
        let (clipboard_tx, clipboard_rx) = bounded::<(String, String)>(EVENT_CHANNEL_CAPACITY);
        let (bell_tx, bell_rx) = bounded::<()>(EVENT_CHANNEL_CAPACITY);
        let pty_write_responses = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let pty_for_run = pty_write_responses.clone();
        let snapshot_rebuild_count = Arc::new(AtomicU64::new(0));
        let snapshot_rebuild_count_for_run = snapshot_rebuild_count.clone();
        let panicked = Arc::new(AtomicBool::new(false));
        let panicked_for_run = panicked.clone();
        let alt_screen_active = Arc::new(AtomicBool::new(false));
        let alt_screen_active_for_run = alt_screen_active.clone();
        let handle = thread::Builder::new()
            .name("ghostty-terminal".into())
            .spawn(move || {
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
                        snapshot_rebuild_count: snapshot_rebuild_count_for_run,
                        alt_screen_active: alt_screen_active_for_run,
                        cell_data_tx: Some(cell_data_tx),
                        cwd_tx,
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
                    // Mark all future operations as failed so callers
                    // don't silently send commands into a dead channel.
                    panicked_for_run.store(true, Ordering::Release);
                }
            })
            .map_err(TerminalError::Spawn)?;

        Ok(Self {
            cmd_tx,
            query_tx,
            cell_data_rx: Some(cell_data_rx),
            cwd_rx,
            clipboard_rx,
            bell_rx,
            handle: Some(handle),
            pty_write_responses,
            snapshot_rebuild_count,
            snapshot_cache: Mutex::new(SnapshotCache {
                cached: Arc::new(GridSnapshot::fallback(DISCONNECTED_ROWS, DISCONNECTED_COLS)),
                pending_rx: None,
                initialized: false,
            }),
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

    /// 取出 VT 线程经上游 OSC 7 回调（on_pwd_changed）上报的工作目录（不阻塞，无事件为 None）。
    /// 上游同时解析 OSC 7/9/1337，其中工作目录由 OSC 7 与 OSC 1337 CurrentDir 触发
    ///（OSC 9 进度上报不产出目录，实测 poll 无事件）。
    pub fn poll_cwd_event(&self) -> Option<String> {
        self.cwd_rx.try_recv().ok()
    }

    /// 取出 VT 线程经上游 OSC 52 回调（on_clipboard_write）上报的剪贴板写入
    ///（选择器字母，文本）。不阻塞，无事件为 None。
    pub fn poll_clipboard_event(&self) -> Option<(String, String)> {
        self.clipboard_rx.try_recv().ok()
    }

    /// 取出 VT 线程经上游 BEL 回调（on_bell）上报的振铃（不阻塞，无事件为 None）。
    pub fn poll_bell_event(&self) -> Option<()> {
        self.bell_rx.try_recv().ok()
    }

    pub fn vt_write(&mut self, data: &[u8]) {
        // Sanitize bytes that the underlying C library cannot handle.
        // NUL is an ECMA-48 ignore control character: strip it before the
        // parser sees it, otherwise one stray NUL (mpv --vo=kitty appends
        // one per frame) corrupts the whole payload, e.g. drops a Kitty
        // image entirely. 0xF8–0xFF are not valid UTF-8 lead bytes and are
        // not standard VT100 C1 control codes. The C parser may crash on
        // long runs of these bytes, so we replace them with spaces to
        // preserve input length while avoiding the crash.
        let sanitized: Vec<u8> = data
            .iter()
            .filter(|&&b| b != 0x00)
            .map(|&b| if b > 0xF7 { b' ' } else { b })
            .collect();
        let mut buf = Vec::with_capacity(data.len() + 4);
        buf.extend_from_slice(&sanitized);
        // 分片直透：上游解析器在同一 Terminal 对象上跨调用保持状态，
        // 此处不得追加 ST/SGR 提前闭合（会截断合法跨块 OSC 并洗掉颜色）。
        // try_send: a wedged VT thread must not block the caller
        // indefinitely (same policy as pty_write). vt_write is used for
        // programmatic VT data only, never on a hot path.
        if let Err(error) = self.cmd_tx.try_send(Command::Write(buf)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// Write PTY output to the terminal, converting LF (`\n`) to CR+LF (`\r\n`).
    /// This is necessary because Ghostty's VT engine treats LF as a line feed
    /// without carriage return, which produces incorrect line advancement for
    /// typical terminal output.
    ///
    /// Unlike [`Self::vt_write`], this method applies text-level `\n`→`\r\n` conversion
    /// suitable for PTY output. VT control sequences, DEC rectangle operations,
    /// and binary VT data should use [`Self::vt_write`] instead.
    pub fn pty_write(&mut self, data: &[u8]) {
        let mut buf = Vec::with_capacity(data.len() + 4);
        // Use last byte from previous call to detect `\r`/`\n` split across
        // chunk boundaries (common with PTY output on Linux). Without this
        // the LF→CRLF converter inserts a spurious `\r`, producing `\r\r\n`.
        let mut prev: u8 = self.last_pty_write_byte;
        for &raw in data {
            // NUL 是 ECMA-48 忽略控制字符：直接跳过，不计入 LF 前导判定。
            if raw == 0x00 {
                continue;
            }
            // 与 vt_write 同规则清洗：0xF8–0xFF 非法 UTF-8 首字节会使 C 解析器崩溃，保长替换为空格。
            let sanitized = if raw > 0xF7 { b' ' } else { raw };
            // Convert a bare LF to CRLF, but only when the LF is not already
            // preceded by a CR. Input that already contains CRLF (common from
            // PTY output) would otherwise become CRCRLF, producing a spurious
            // extra carriage return.
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
        // try_send: this runs on the session/render path while holding the
        // session lock. A full command channel (VT thread busy with a long
        // command) must not block the caller indefinitely; dropping a chunk
        // is acceptable — the VT engine is frame-based and the next chunk
        // carries on.
        if let Err(error) = self.cmd_tx.try_send(Command::Write(buf)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// Returns `true` if the terminal thread is still alive and accepting commands.
    /// Uses flume's built-in disconnect detection: when the terminal thread exits,
    /// its `Receiver<Command>` is dropped, causing `Sender::is_disconnected()` to
    /// return `true`.
    ///
    /// Note: there is an inherent race — the terminal can die between an
    /// `is_alive()` check and the next command send. This is acceptable for
    /// zombie-detection purposes; at most one command will silently fail before
    /// the next check detects the disconnection.
    pub fn is_alive(&self) -> bool {
        !self.panicked.load(Ordering::Acquire) && !self.cmd_tx.is_disconnected()
    }

    /// Test-only: sever the command channel so the VT thread exits and every
    /// subsequent public query takes its disconnected/fallback path
    /// (`is_alive` → false, snapshots → `GridSnapshot::fallback`). Replaces
    /// the live sender with a dud sender (whose receiver is dropped, so it
    /// reports disconnected); the VT thread sees the disconnect and breaks
    /// out of its command loop.
    #[cfg(test)]
    pub(crate) fn disconnect_for_test(&mut self) {
        let (dud_tx, _dud_rx) = bounded::<Command>(1);
        self.cmd_tx = dud_tx;
    }

    /// 等待 VT 线程处理完此前全部命令（含本次 build）。只保证 build 完成，
    /// 不保证推送 cell 数据帧：去重命中或通道满丢弃时 `receive_cell_data` 为
    /// `None`，测试需先制造内容变更再 `expect`。
    pub fn flush(&self) {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.try_send(Command::FlushAck(tx)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
            return;
        }
        // Bounded wait: if the VT thread is wedged (e.g. a pathological C
        // parser input), an unbounded recv would block the caller forever
        // while it holds the session lock, freezing every JNI entry point
        // and tripping the ANR watchdog. The timeout is deliberately far
        // above the worst legitimate backlog (a burst of writes can take a
        // while to drain in debug builds) — 5s of silence means the VT
        // thread is genuinely stuck.
        match rx.recv_timeout(std::time::Duration::from_secs(FLUSH_TIMEOUT_SECS)) {
            Ok(()) => {}
            Err(_) => {
                log::warn!("ghostty_terminal: flush_ack timed out — session may be dead");
            }
        }
    }

    pub fn set_theme(&self, background: [u8; 3], foreground: [u8; 3], ansi: [[u8; 3]; 16]) {
        // try_send: same non-blocking policy as resize.
        if let Err(error) = self.cmd_tx.try_send(Command::SetTheme {
            background,
            foreground,
            ansi,
        }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// Scroll the terminal viewport by a delta (up is negative). The
    /// delta is applied on the VT thread via `scroll_viewport`; the next
    /// CellData push carries the scrolled view.
    pub fn scroll_viewport(&self, delta: isize) -> bool {
        if let Err(error) = self.cmd_tx.try_send(Command::ScrollViewport(delta)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for scroll: {error}");
            return false;
        }
        true
    }

    /// Returns true when the resize command was accepted by the VT thread.
    /// A `false` result means the grid was NOT resized (PTY may still have
    /// been updated by the caller's `pty.resize`); the caller must not cache
    /// the new size so the next resize event retries.
    pub fn resize(&mut self, rows: u32, cols: u32) -> bool {
        // try_send, not send: a wedged VT thread must not block the caller
        // (switchSession holds the Kotlin sessionLock across this call).
        // NOTE: a dropped resize is NOT replayed — the PTY/grid keep the old
        // size until the next resize event arrives (IME change, rotation,
        // settings change, session switch). The channel capacity (1024) makes
        // loss unlikely outside a VT-thread stall /111).
        if let Err(error) = self.cmd_tx.try_send(Command::Resize { rows, cols }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for resize: {error}");
            return false;
        }
        true
    }

    /// 更新单元格像素尺寸（Kitty 放置几何依赖它；与 resize 同一非阻塞策略）。
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

    /// RIS 全重置：恢复终端初始状态并清空回滚（侧边面板“重置终端”按钮）。
    /// try_send 非阻塞：VT 线程卡住时丢弃而非阻塞调用方（与 resize 同策略）。
    pub fn reset(&self) {
        if let Err(error) = self.cmd_tx.try_send(Command::Reset) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for reset: {error}");
        }
    }

    /// 安装终端持有的活动选区（跟踪引用，随滚动/输出/重排跟随文本）。
    /// 坐标为绝对网格行（0 = 回滚顶部）与列；Block 模式传 rectangle=true。
    /// try_send 非阻塞：VT 线程卡住时丢弃而非阻塞调用方（与 resize 同策略）。
    pub fn set_selection(&self, start: (u32, u32), end: (u32, u32), rectangle: bool) {
        if let Err(error) = self.cmd_tx.try_send(Command::SetSelection {
            start,
            end,
            rectangle,
        }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for set_selection: {error}");
        }
    }

    /// 清除终端持有的活动选区（与 set_selection 同一非阻塞策略）。
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

    pub fn take_snapshot(&self) -> GridSnapshot {
        self.take_snapshot_with_scroll(0)
    }

    /// Number of times the VT thread actually rebuilt the grid snapshot
    /// (vs reusing the cached snapshot) since this terminal was created.
    /// Used by tests to prove the snapshot cache skips rebuilds on
    /// unchanged frames.
    pub fn snapshot_rebuild_count(&self) -> u64 {
        self.snapshot_rebuild_count.load(Ordering::Relaxed)
    }

    /// Returns a **fresh** grid snapshot for the current terminal state.
    ///
    /// This always blocks until the VT thread has processed the request, so
    /// callers observe the latest grid content (never a stale cached frame).
    /// The VT thread rebuilds the snapshot only when the grid or scroll offset
    /// actually changed (see `snapshot_needs_rebuild`), so the blocking cost is
    /// a single channel round-trip and is cheap when the grid is unchanged.
    pub fn take_snapshot_with_scroll(&self, scroll_offset: u32) -> GridSnapshot {
        let (tx, rx): (Sender<Arc<GridSnapshot>>, _) = bounded(1);
        if let Err(error) = self
            .cmd_tx
            .try_send(Command::TakeSnapshot { tx, scroll_offset })
        {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
            return GridSnapshot::fallback(DISCONNECTED_ROWS, DISCONNECTED_COLS);
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(snapshot) => Arc::unwrap_or_clone(snapshot),
            Err(_) => {
                log::warn!("ghostty_terminal: take_snapshot_with_scroll timed out");
                GridSnapshot::fallback(DISCONNECTED_ROWS, DISCONNECTED_COLS)
            }
        }
    }

    /// Non-blocking snapshot read for the **render hot path**.
    ///
    /// Returns `None` on the very first call (the cache is primed by issuing a
    /// command, populated on the next call). Thereafter it returns the latest
    /// available snapshot without ever blocking on the VT thread — so the
    /// render thread can call this while holding the session lock without
    /// stalling main-thread work (IME input, settings). The returned snapshot
    /// is at most 1 frame behind, which is harmless because the surface diffs
    /// against `prev_cells`.
    pub fn try_take_snapshot_with_scroll(&self, scroll_offset: u32) -> Option<GridSnapshot> {
        let mut cache = match self.snapshot_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("snapshot_cache mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };

        // Collect any pending response from the previous command.
        if let Some(rx) = &cache.pending_rx
            && let Ok(snapshot) = rx.try_recv()
        {
            cache.cached = snapshot;
        }

        if !cache.initialized {
            // First call: issue a command so the cache populates next frame,
            // then return None (the surface skips this one frame).
            let (tx, rx): (Sender<Arc<GridSnapshot>>, _) = bounded(1);
            if self
                .cmd_tx
                .try_send(Command::TakeSnapshot { tx, scroll_offset })
                .is_ok()
            {
                cache.pending_rx = Some(rx);
            }
            cache.initialized = true;
            return None;
        }

        // Issue a command for the next frame's snapshot.
        let (tx, rx): (Sender<Arc<GridSnapshot>>, _) = bounded(1);
        if self
            .cmd_tx
            .try_send(Command::TakeSnapshot { tx, scroll_offset })
            .is_ok()
        {
            cache.pending_rx = Some(rx);
        }

        Some(Arc::unwrap_or_clone(cache.cached.clone()))
    }

    pub fn take_kitty_graphics_image(&self, image_id: u32) -> Option<KittyGraphicsImageData> {
        self.query(
            |tx| Query::TakeKittyGraphicsImage { id: image_id, tx },
            None,
            "take_kitty_graphics_image",
        )
    }

    /// 采集全部可见 Kitty 放置（渲染线程按需调用；VT 繁忙时回退空列表）。
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

    /// Cursor viewport (row, col) read through `build_cell_data` — the exact
    /// source the render thread consumes. `None` when hidden or build fails.
    ///  observability: the JNI cursor query and the deterministic
    /// cursor-coordinate contract tests read through this so instrumentation
    /// sees the same coordinates the GPU draws.
    pub fn render_cursor(&self) -> Option<(u32, u32)> {
        self.query(Query::RenderCursor, None, "render_cursor")
    }

    /// Receive the most recent CellData snapshot from the ghostty thread
    /// (auto-pushed after every state mutation when the channel is enabled).
    /// Drains stale entries so the caller always gets the freshest snapshot.
    /// Returns `None` if the channel is disabled or no data is available yet.
    pub fn receive_cell_data(&self) -> Option<(Vec<CellData>, CursorInfo)> {
        let rx = self.cell_data_rx.as_ref()?;
        let mut latest = rx.try_recv().ok()?;
        while let Ok(next) = rx.try_recv() {
            latest = next;
        }
        Some(latest)
    }

    pub fn cwd(&self) -> String {
        self.query(Query::Cwd, String::new(), "cwd")
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
        // Bounded wait: a wedged VT thread must not block the caller
        // (potentially the UI thread) forever.
        rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS))
            .ok()
    }

    /// Encode a mouse event (pixel position, action, button) into terminal
    /// escape sequences using the Ghostty mouse encoder. `cell_w`/`cell_h`
    /// are the renderer's live cell dimensions so the pixel→cell mapping
    /// matches what is displayed (zelland `get_cell_size()` pattern,
    /// src-tauri/src/terminal.rs:41-90 + ghostty_mouse_encoder SGR/1006).
    ///
    /// Returns `Some(empty)` when mouse reporting is disabled (no DECSET
    /// 1000/1002/1003) or encoding fails — the caller drops the event.
    /// (Only a wedged query channel yields `None`, as with `key_encode`.)
    pub fn encode_mouse_event(
        &self,
        position: (f32, f32),
        action: u8,
        button: u8,
        cell_w: f32,
        cell_h: f32,
    ) -> Option<Vec<u8>> {
        self.query(
            |tx| Query::EncodeMouseEvent {
                position,
                action,
                button,
                cell_w,
                cell_h,
                tx,
            },
            Vec::new(),
            "encode_mouse_event",
        )
        .into()
    }

    /// Submit a key for encoding and return a receiver for the result.
    /// The caller should NOT hold any session lock while waiting on the returned receiver.
    pub fn key_encode_submit(
        &self,
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Option<flume::Receiver<Vec<u8>>> {
        let (tx, rx) = flume::bounded(1);
        // try_send: consistent with resize/set_theme — a wedged VT thread
        // must not block the caller.
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

    /// Like [`Self::mode_get`] but with a caller-provided timeout. Used by
    /// callers on latency-sensitive paths (e.g. UI-thread focus reporting)
    /// where a wedged VT thread must not stall the caller for the full
    /// query timeout.
    pub fn mode_get_with_timeout(
        &self,
        mode_num: u16,
        kind: u8,
        timeout: std::time::Duration,
    ) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Query::ModeGet(mode_num, kind, tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
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

    /// Lock-free read of the alternate-screen mirror, updated by the VT thread
    /// on every emitted frame. Safe to call from the Android
    /// input path on every touch-scroll event without blocking the UI thread.
    pub fn alt_screen_active_atomic(&self) -> bool {
        self.alt_screen_active.load(Ordering::Acquire)
    }

    /// Send a stateless [`Query`] to the VT thread and wait for its reply,
    /// falling back to `fallback` on send failure or timeout. Single call
    /// site for the bounded-channel + recv_timeout boilerplate shared by
    /// every query method (see also `Query` docs for the channel design).
    fn query<T>(&self, build: impl FnOnce(Sender<T>) -> Query, fallback: T, method: &str) -> T {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(build(tx)) {
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

    /// Extract selection text with Ghostty's native formatter (wrap-aware,
    /// wide-char safe). `start`/`end` are grid rows in screen coordinates
    /// (absolute: scrollback row 0 is the top of history; the caller's
    /// gridRow from scrollbackLine is exactly this). Returns "" on error.
    pub fn selection_text(&self, start: (u32, u32), end: (u32, u32), rectangle: bool) -> String {
        self.query(
            |tx| Query::SelectionText {
                start,
                end,
                rectangle,
                tx,
            },
            String::new(),
            "selection_text",
        )
    }

    /// Query the OSC 8 hyperlink URI at a grid cell (row 0 = top of
    /// scrollback, matching scrollbackLine). None when no link.
    pub fn hyperlink_at(&self, row: u32, col: u32) -> Option<String> {
        self.query(
            |tx| Query::HyperlinkAt { row, col, tx },
            None,
            "hyperlink_at",
        )
    }

    pub fn search_in_scrollback(&self, query: &str) -> Option<(u32, u32)> {
        self.query(
            |tx| Query::SearchInScrollback {
                query: query.to_string(),
                tx,
            },
            None,
            "search_in_scrollback",
        )
    }

    pub fn search_all_in_scrollback(&self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        self.query(
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
