use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use flume::{Receiver, Sender, bounded};

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
        let (query_tx, query_rx) = flume::unbounded::<Command>();
        let pty_write_responses = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let pty_for_run = pty_write_responses.clone();
        let snapshot_rebuild_count = Arc::new(AtomicU64::new(0));
        let snapshot_rebuild_count_for_run = snapshot_rebuild_count.clone();
        let handle = thread::Builder::new()
            .name("ghostty-terminal".into())
            .spawn(move || {
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
                })
            })
            .map_err(|e| format!("failed to spawn terminal thread: {e}"))?;

        Ok(Self {
            cmd_tx,
            query_tx,
            handle: Some(handle),
            pty_write_responses,
            snapshot_rebuild_count,
            snapshot_cache: Mutex::new(SnapshotCache {
                cached: GridSnapshot::fallback(DISCONNECTED_ROWS, DISCONNECTED_COLS),
                pending_rx: None,
                initialized: false,
            }),
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
        if let Err(error) = self.cmd_tx.send(Command::Write(buf)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
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
        let mut prev: u8 = 0;
        for &b in data {
            // Convert a bare LF to CRLF, but only when the LF is not already
            // preceded by a CR. Input that already contains CRLF (common from
            // PTY output) would otherwise become CRCRLF, producing a spurious
            // extra carriage return.
            if b == b'\n' && prev != b'\r' {
                buf.push(b'\r');
            }
            buf.push(b);
            prev = b;
        }
        // Append ST (String Terminator) and SGR reset to close any incomplete
        // escape sequence that may have been truncated at the end of this chunk.
        // This prevents the Ghostty parser from staying in string mode and
        // consuming the next chunk as sequence data.
        buf.extend_from_slice(b"\x1b\\\x1b[0m");
        if let Err(error) = self.cmd_tx.send(Command::Write(buf)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
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
        !self.cmd_tx.is_disconnected()
    }

    pub fn flush(&self) {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.send(Command::FlushAck(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        if rx.recv().is_err() {
            log::warn!("ghostty_terminal: flush_ack recv failed — session may be dead");
        }
    }

    pub fn set_theme(&self, background: [u8; 3], foreground: [u8; 3], ansi: [[u8; 3]; 16]) {
        if let Err(error) = self.cmd_tx.send(Command::SetTheme {
            background,
            foreground,
            ansi,
        }) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
    }

    pub fn resize(&mut self, rows: u32, cols: u32) {
        if let Err(error) = self.cmd_tx.send(Command::Resize { rows, cols }) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
    }

    pub fn rows(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::Rows(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_ROWS, "rows")
    }

    pub fn cols(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::Cols(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
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
        let (tx, rx) = bounded(1);
        if let Err(error) = self
            .cmd_tx
            .send(Command::TakeSnapshot { tx, scroll_offset })
        {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
            return GridSnapshot::fallback(DISCONNECTED_ROWS, DISCONNECTED_COLS);
        }
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(snapshot) => snapshot,
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
            let (tx, rx) = bounded(1);
            let _ = self
                .cmd_tx
                .send(Command::TakeSnapshot { tx, scroll_offset });
            cache.pending_rx = Some(rx);
            cache.initialized = true;
            return None;
        }

        // Issue a command for the next frame's snapshot.
        let (tx, rx) = bounded(1);
        let _ = self
            .cmd_tx
            .send(Command::TakeSnapshot { tx, scroll_offset });
        cache.pending_rx = Some(rx);

        Some(cache.cached.clone())
    }

    pub fn take_kgp_image(&self, image_id: u32) -> Option<KgpImageData> {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.send(Command::TakeKgpImage { id: image_id, tx }) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(result) => result,
            Err(error) => {
                log::warn!(
                    "ghostty_terminal: take_kgp_image recv failed — terminal may be dead: {error}"
                );
                None
            }
        }
    }

    pub fn cursor_x(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::CursorX(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_CURSOR_X, "cursor_x")
    }

    pub fn cursor_y(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::CursorY(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_CURSOR_Y, "cursor_y")
    }

    pub fn cursor_visible(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::CursorVisible(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_CURSOR_VISIBLE, "cursor_visible")
    }

    pub fn cwd(&self) -> String {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::Cwd(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(cwd) => cwd,
            Err(_) => {
                log::warn!("ghostty_terminal: terminal thread disconnected — returning empty cwd");
                String::new()
            }
        }
    }

    /// Returns a clone of the lock-free command sender for direct ghostty thread access.
    /// The caller uses this with Self::key_encode_submit_via to bypass the session mutex,
    /// eliminating UI-thread stalls when the render thread holds the session lock.
    pub fn clone_cmd_tx(&self) -> Sender<Command> {
        self.cmd_tx.clone()
    }

    /// Send a key event for encoding using a pre-cloned sender.
    /// Returns a receiver for the encoded result. This function does NOT need
    /// `&self` — the caller can submit directly to ghostty without any session lock.
    pub fn key_encode_submit_via(
        cmd_tx: &Sender<Command>,
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Option<Receiver<Vec<u8>>> {
        let (tx, rx) = flume::bounded(1);
        cmd_tx
            .send(Command::KeyEncode {
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

    pub fn key_encode(
        &self,
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Option<Vec<u8>> {
        self.key_encode_submit(key_code, modifiers, action, unicode_char, unshifted_char)?
            .recv()
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
        self.cmd_tx
            .send(Command::KeyEncode {
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
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::ModeGet(mode_num, kind, tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(mode) => mode,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: terminal thread disconnected — returning false for mode_get({mode_num}, {kind})"
                );
                false
            }
        }
    }

    pub fn origin_mode(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::OriginMode(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_MODE_ORIGIN, "origin_mode")
    }

    pub fn autowrap(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::Autowrap(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_MODE_AUTOWRAP, "autowrap")
    }

    pub fn alt_screen(&self) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::AltScreen(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(alt) => alt,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: terminal thread disconnected — returning false for alt_screen"
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
        if let Err(error) = self.query_tx.send(Command::Title(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        Self::recv_or_fallback(rx, DISCONNECTED_TITLE.to_string(), "title")
    }

    pub fn scrollback_length(&self) -> u32 {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.query_tx.send(Command::ScrollbackLength(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
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
        if let Err(error) = self.query_tx.send(Command::ReadLineText { row, tx }) {
            log::error!("ghostty_terminal: query_tx send failed for ReadLineText: {error}");
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
        if let Err(error) = self.query_tx.send(Command::ReadVisibleText(tx)) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(text) => text,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: terminal thread disconnected — returning empty string for read_visible_text"
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
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(result) => result,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: terminal thread disconnected — returning None for search_in_scrollback"
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
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(result) => result,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: terminal thread disconnected — returning empty results for search_all_in_scrollback"
                );
                Vec::new()
            }
        }
    }

    pub fn dump_grid(&self) -> DumpedGrid {
        let (tx, rx) = bounded(1);
        if let Err(error) = self.cmd_tx.send(Command::DumpGrid { tx }) {
            log::error!("ghostty_terminal: cmd_tx send failed: {error}");
        }
        match rx.recv() {
            Ok(grid) => grid,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: terminal thread disconnected — returning empty grid for dump_grid"
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

    // ── DEC Rectangle Operations ──
    //
    // Ghostty does not handle CSI $ intermediate sequences (DECFRA, DECERA,
    // DECCARA, etc.). These helpers decompose rectangle operations into
    // primitive VT sequences that Ghostty supports natively.
    // ────────────────────────────────────────────────────

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
