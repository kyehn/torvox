use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use flume::{Sender, bounded};

use super::commands::{Command, RunConfig, SnapshotCache};
use super::types::*;

impl super::GhosttyTerminal {
    pub fn new(rows: u32, cols: u32, scrollback_lines: u32) -> Result<Self, String> {
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
        initial_bg: [u8; 3],
        initial_fg: [u8; 3],
        initial_ansi: [[u8; 3]; 16],
    ) -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = bounded::<Command>(COMMAND_CHANNEL_CAPACITY);
        let (query_tx, query_rx) = flume::bounded::<Command>(256);
        let (cell_data_tx, cell_data_rx) = flume::bounded::<(Vec<CellData>, CursorInfo)>(4);
        let pty_write_responses = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let pty_for_run = pty_write_responses.clone();
        let snapshot_rebuild_count = Arc::new(AtomicU64::new(0));
        let snapshot_rebuild_count_for_run = snapshot_rebuild_count.clone();
        let panicked = Arc::new(AtomicBool::new(false));
        let panicked_for_run = panicked.clone();
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
                        background_color: initial_bg,
                        foreground_color: initial_fg,
                        ansi_colors: initial_ansi,
                        response_buffer: pty_for_run,
                        snapshot_rebuild_count: snapshot_rebuild_count_for_run,
                        cell_data_tx: Some(cell_data_tx),
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
            .map_err(|e| format!("failed to spawn terminal thread: {e}"))?;

        Ok(Self {
            cmd_tx,
            query_tx,
            cell_data_rx: Some(cell_data_rx),
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
            last_in_string_mode: false,
        })
    }

    pub fn drain_pty_write_responses(&self) -> Vec<Vec<u8>> {
        let mut guard = self
            .pty_write_responses
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *guard)
    }

    pub fn vt_write(&mut self, data: &[u8]) {
        // Sanitize bytes that the underlying C library cannot handle.
        // 0xF8–0xFF are not valid UTF-8 lead bytes and are not standard
        // VT100 C1 control codes. The C parser may crash on long runs of
        // these bytes, so we replace them with spaces to preserve input
        // length while avoiding the crash.
        let sanitized: Vec<u8> = data
            .iter()
            .map(|&b| if b > 0xF7 { b' ' } else { b })
            .collect();
        let mut buf = Vec::with_capacity(data.len() + 4);
        buf.extend_from_slice(&sanitized);
        // Append ST + SGR reset to close any incomplete escape sequence
        // (OSC, DCS, SOS, PM, APC) that may have been truncated at the end
        // of this chunk. vt_write is only used for programmatic VT data
        // (settings, OSC sequences, test data), not for streaming PTY output,
        // so SGR reset here does NOT break colored output.
        buf.extend_from_slice(b"\x1b\\\x1b[0m");
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
    /// Unlike [`vt_write`], this method applies text-level `\n`→`\r\n` conversion
    /// suitable for PTY output. VT control sequences, DEC rectangle operations,
    /// and binary VT data should use [`vt_write`] instead.
    pub fn pty_write(&mut self, data: &[u8]) {
        let mut buf = Vec::with_capacity(data.len() + 4);
        // Use last byte from previous call to detect `\r`/`\n` split across
        // chunk boundaries (common with PTY output on Linux). Without this
        // the LF→CRLF converter inserts a spurious `\r`, producing `\r\r\n`.
        let mut prev: u8 = self.last_pty_write_byte;
        let mut in_string = self.last_in_string_mode;
        // The previous chunk may have ended with ESC (the first byte of a
        // two-byte sequence); seed the tracking state accordingly.
        let mut prev_was_esc = prev == 0x1B;
        for &b in data {
            // Convert a bare LF to CRLF, but only when the LF is not already
            // preceded by a CR. Input that already contains CRLF (common from
            // PTY output) would otherwise become CRCRLF, producing a spurious
            // extra carriage return.
            if b == b'\n' && prev != b'\r' {
                buf.push(b'\r');
            }
            buf.push(b);
            // Track OSC/DCS/SOS/PM/APC string mode: these sequences are
            // terminated by ST (ESC \) or BEL (OSC). If a chunk ends inside
            // one, the next chunk would be swallowed as string data; a CSI
            // sequence (ESC [) needs no such handling — the VT parser is a
            // state machine and resumes it across chunks on its own.
            if in_string {
                // ST = ESC \ ; detect the backslash following the ESC byte.
                if b == 0x07 || (b == b'\\' && prev_was_esc) {
                    in_string = false;
                }
            } else if prev_was_esc {
                match b {
                    b']' | b'P' | b'X' | b'^' | b'_' => in_string = true,
                    _ => {}
                }
            }
            prev_was_esc = b == 0x1B;
            prev = b;
        }
        // Close an unterminated string with ST so the parser never stays in
        // string mode across chunks. No SGR reset here: it would break a
        // colour run split across a chunk boundary.
        if in_string {
            buf.extend_from_slice(b"\x1b\\");
        }
        self.last_pty_write_byte = prev;
        self.last_in_string_mode = in_string;
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

    pub fn flush(&self) {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.send(Command::FlushAck(tx)) {
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
        // try_send: same non-blocking policy as resize (round-111).
        if let Err(error) = self.cmd_tx.try_send(Command::SetTheme {
            background,
            foreground,
            ansi,
        }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
    }

    /// Returns true when the resize command was accepted by the VT thread.
    /// A `false` result means the grid was NOT resized (PTY may still have
    /// been updated by the caller's `pty.resize`); the caller must not cache
    /// the new size so the next resize event retries (round-112).
    /// Scroll the terminal viewport by a delta (up is negative). The
    /// delta is applied on the VT thread via `scroll_viewport`; the next
    /// CellData push carries the scrolled view (round-205).
    pub fn scroll_viewport(&self, delta: isize) -> bool {
        if let Err(error) = self.cmd_tx.try_send(Command::ScrollViewport(delta)) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for scroll: {error}");
            return false;
        }
        true
    }

    pub fn resize(&mut self, rows: u32, cols: u32) -> bool {
        // try_send, not send: a wedged VT thread must not block the caller
        // (switchSession holds the Kotlin sessionLock across this call).
        // NOTE: a dropped resize is NOT replayed — the PTY/grid keep the old
        // size until the next resize event arrives (IME change, rotation,
        // settings change, session switch). The channel capacity (1024) makes
        // loss unlikely outside a VT-thread stall (round-110/111).
        if let Err(error) = self.cmd_tx.try_send(Command::Resize { rows, cols }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed for resize: {error}");
            return false;
        }
        true
    }

    pub fn rows(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::Rows(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped for rows: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_ROWS, "rows")
    }

    pub fn cols(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::Cols(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_COLS, "cols")
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
            .send(Command::TakeSnapshot { tx, scroll_offset })
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
                .send(Command::TakeSnapshot { tx, scroll_offset })
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
            .send(Command::TakeSnapshot { tx, scroll_offset })
            .is_ok()
        {
            cache.pending_rx = Some(rx);
        }

        Some(Arc::unwrap_or_clone(cache.cached.clone()))
    }

    pub fn take_kitty_graphics_image(&self, image_id: u32) -> Option<KittyGraphicsImageData> {
        let (tx, rx) = bounded(1);
        if let Err(error) = self
            .cmd_tx
            .send(Command::TakeKittyGraphicsImage { id: image_id, tx })
        {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(result) => result,
            Err(error) => {
                log::warn!(
                    "ghostty_terminal: take_kitty_graphics_image timed out or disconnected — terminal may be dead: {error}"
                );
                None
            }
        }
    }

    pub fn cursor_x(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::CursorX(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_CURSOR_X, "cursor_x")
    }

    pub fn cursor_y(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::CursorY(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_CURSOR_Y, "cursor_y")
    }

    pub fn cursor_visible(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::CursorVisible(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_CURSOR_VISIBLE, "cursor_visible")
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
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::Cwd(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(cwd) => cwd,
            Err(_) => {
                log::warn!("ghostty_terminal: cwd timed out or disconnected — returning empty cwd");
                String::new()
            }
        }
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
        // must not block the caller (round-112).
        self.cmd_tx
            .try_send(Command::KeyEncode {
                key_code,
                modifiers,
                action,
                unicode_char,
                unshifted_char,
                tx,
            })
            .ok()?;
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
        if let Err(error) = self.query_tx.try_send(Command::ModeGet(mode_num, kind, tx)) {
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

    pub fn origin_mode(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::OriginMode(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_MODE_ORIGIN, "origin_mode")
    }

    pub fn autowrap(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::Autowrap(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_MODE_AUTOWRAP, "autowrap")
    }

    pub fn alt_screen(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::AltScreen(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(alt) => alt,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: alt_screen timed out or disconnected — returning false"
                );
                false
            }
        }
    }

    pub fn is_mouse_tracking_active(&self) -> bool {
        self.mode_get(1000, 0) || self.mode_get(1002, 0) || self.mode_get(1003, 0)
    }

    pub fn is_cursor_enabled(&self) -> bool {
        self.mode_get(25, 0)
    }

    pub fn is_bracketed_paste_active(&self) -> bool {
        self.mode_get(2004, 0)
    }

    pub fn is_origin_mode(&self) -> bool {
        self.origin_mode()
    }

    pub fn is_autowrap_enabled(&self) -> bool {
        self.autowrap()
    }

    pub fn is_alt_screen_active(&self) -> bool {
        self.alt_screen()
    }

    pub fn title(&self) -> String {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::Title(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_TITLE.to_string(), "title")
    }

    pub fn scrollback_length(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::ScrollbackLength(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(len) => len,
            Err(_) => {
                log::warn!("ghostty_terminal: scrollback_length timed out, returning cached value");
                DISCONNECTED_SCROLLBACK
            }
        }
    }

    pub fn read_line_text(&self, row: u32) -> Option<String> {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::ReadLineText { row, tx }) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed for ReadLineText: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(text) => text,
            Err(_) => {
                log::warn!("ghostty_terminal: read_line_text({row}) timed out or disconnected");
                None
            }
        }
    }

    pub fn read_visible_text(&self) -> String {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.try_send(Command::ReadVisibleText(tx)) {
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(text) => text,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: read_visible_text timed out or disconnected — returning empty string"
                );
                String::new()
            }
        }
    }

    pub fn search_in_scrollback(&self, query: &str) -> Option<(u32, u32)> {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.send(Command::SearchInScrollback {
            query: query.to_string(),
            tx,
        }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(result) => result,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: search_in_scrollback timed out or disconnected — returning None"
                );
                None
            }
        }
    }

    pub fn search_all_in_scrollback(
        &self,
        query: &str,
        case_sensitive: bool,
        fuzzy: bool,
    ) -> Vec<SearchMatch> {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.send(Command::SearchInScrollbackAll {
            query: query.to_string(),
            case_sensitive,
            fuzzy,
            tx,
        }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(result) => result,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: search_all_in_scrollback timed out or disconnected — returning empty results"
                );
                Vec::new()
            }
        }
    }

    pub fn dump_grid(&self) -> DumpedGrid {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.send(Command::DumpGrid { tx }) {
            log::warn!("ghostty_terminal: cmd_tx full/dropped failed: {error}");
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(grid) => grid,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: dump_grid timed out or disconnected — returning empty grid"
                );
                DumpedGrid {
                    rows: 0,
                    cols: 0,
                    visible: Vec::new(),
                    scrollback: Vec::new(),
                }
            }
        }
    }

    /// DECFRA: Fill rectangle with char_code (rows top..bottom, cols left..right, 1-indexed).
    pub fn dec_fill_rect(&mut self, char_code: u8, top: u32, left: u32, bottom: u32, right: u32) {
        let count = (right - left + 1) as usize;
        for row in top..=bottom {
            // Build the full cursor-move + fill sequence in one buffer so the
            // single `vt_write` call contains a complete, self-terminated
            // sequence (see `vt_write` contract — never split one sequence).
            let mut buf = Vec::with_capacity(count + 16);
            let pos = format!("\x1b[{};{}H", row, left);
            buf.extend_from_slice(pos.as_bytes());
            buf.extend(std::iter::repeat_n(char_code, count));
            self.vt_write(&buf);
        }
        self.flush();
    }

    /// DECERA: Erase rectangle (fill with spaces).
    pub fn dec_erase_rect(&mut self, top: u32, left: u32, bottom: u32, right: u32) {
        self.dec_fill_rect(b' ', top, left, bottom, right);
    }

    /// DECCARA: Change attribute in rectangle.
    /// Writes spaces with the given SGR attribute applied.
    pub fn dec_change_attr_rect(
        &mut self,
        sgr_seq: &[u8],
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) {
        let count = (right - left + 1) as usize;
        for row in top..=bottom {
            // Build the entire cursor-move + SGR + fill sequence in one buffer.
            // Splitting the SGR escape sequence (`\x1b[` + params + `m`) across
            // multiple `vt_write` calls would inject a stray ST/SGR reset inside
            // the sequence and is therefore forbidden by the `vt_write` contract.
            let mut buf = Vec::with_capacity(count + sgr_seq.len() + 16);
            let pos = format!("\x1b[{};{}H", row, left);
            buf.extend_from_slice(pos.as_bytes());
            buf.extend_from_slice(b"\x1b[");
            buf.extend_from_slice(sgr_seq);
            buf.extend_from_slice(b"m");
            buf.extend(std::iter::repeat_n(b' ', count));
            self.vt_write(&buf);
        }
        self.flush();
    }
}
