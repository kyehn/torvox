use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use terminal_core::line::Line;
use terminal_core::snapshot::SessionSnapshot;

use terminal_engine::session::Session;
use terminal_engine::shell_env::ShellEnv;

use super::{AndroidSurface, SurfaceError};
use super::{DEFAULT_MAX_SCROLLBACK, MAX_SURFACE_DIMENSION, SYNC_MODE_NUMBER};

use crate::bridge::PollAllResult;
use crate::lock_util::lock_or_recover;

pub(crate) fn cell_to_line(
    cells: &[terminal_engine::ghostty_terminal::CellSnapshot],
    cols: u32,
) -> Line {
    let mut line = Line::new(cols);
    for col in 0..cols as usize {
        if let Some(cs) = cells.get(col)
            && let Some(cell) = line.get_mut(col as u32)
        {
            cell.char = char::from_u32(cs.codepoint).unwrap_or(' ');
            cell.foreground = terminal_core::cell::Color {
                r: (cs.foreground[0] * 255.0) as u8,
                g: (cs.foreground[1] * 255.0) as u8,
                b: (cs.foreground[2] * 255.0) as u8,
                a: (cs.foreground[3] * 255.0) as u8,
            };
            cell.background = terminal_core::cell::Color {
                r: (cs.background[0] * 255.0) as u8,
                g: (cs.background[1] * 255.0) as u8,
                b: (cs.background[2] * 255.0) as u8,
                a: (cs.background[3] * 255.0) as u8,
            };
            cell.attrs.bold = cs.bold;
            cell.attrs.italic = cs.italic;
            cell.attrs.underline = cs.underline;
            cell.attrs.reverse = cs.reverse;
        }
    }
    line
}

pub(crate) fn line_to_text(line: &Line) -> String {
    (0..line.len())
        .filter_map(|c| line.get(c))
        .map(|cell| cell.char)
        .collect()
}

impl AndroidSurface {
    pub fn spawn_session(
        &mut self,
        shell: &str,
        env: &ShellEnv,
    ) -> Result<Arc<Mutex<Session>>, SurfaceError> {
        let (background, foreground) = (self.theme.background, self.theme.foreground);
        let ansi = self.theme.ansi;
        let session = Session::spawn_with_theme(
            shell,
            self.rows,
            self.cols,
            env,
            background,
            foreground,
            ansi,
            self.scrollback_lines,
        )
        .map_err(|e| SurfaceError::Session(e.to_string()))?;
        let session_arc = Arc::new(Mutex::new(session));
        {
            let mut guard = lock_or_recover(&session_arc, "spawn_session");
            self.exited = guard.exited_flag().clone();
            guard.set_pixel_size(
                (self.surface_width.load(Ordering::Relaxed) as u16).min(MAX_SURFACE_DIMENSION),
                (self.surface_height.load(Ordering::Relaxed) as u16).min(MAX_SURFACE_DIMENSION),
            );
        }
        self.session = Some(session_arc.clone());

        Ok(session_arc)
    }

    pub fn write_to_pty(&mut self, data: &[u8]) {
        if let Some(ref session_arc) = self.session {
            let mut session = lock_or_recover(session_arc, "write_to_pty");
            if let Err(error) = session.write(data) {
                log::error!("surface: PTY write failed: {error}");
            }
        } else {
            log::warn!("surface: write_to_pty skipped — session not available");
        }
    }

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn poll_bel(&mut self) -> bool {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_bel();
        }
        false
    }

    pub fn poll_clipboard(&mut self) -> Option<String> {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_clipboard();
        }
        None
    }

    pub fn poll_notification(&mut self) -> Option<(String, String)> {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_notification();
        }
        None
    }

    pub fn poll_sync_active(&mut self) -> bool {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.mode_get(SYNC_MODE_NUMBER, 0);
        }
        false
    }

    pub fn poll_shell_integration(&mut self) -> u8 {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.poll_shell_integration() as u8;
        }
        0
    }

    /// Poll all deferred events (BEL, clipboard, notification, sync mode, shell
    /// integration) in a single session lock acquisition. This avoids the
    /// per-poll session-lock churn that the individual `poll_*` methods incur.
    pub fn poll_all(&mut self) -> PollAllResult {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            let notification = session.poll_notification();
            let (notification_title, notification_body) = match notification {
                Some((t, b)) => (Some(t), Some(b)),
                None => (None, None),
            };
            return PollAllResult {
                bel: session.poll_bel(),
                clipboard: session.poll_clipboard(),
                notification_title,
                notification_body,
                sync_active: session.mode_get(SYNC_MODE_NUMBER, 0),
                shell_integration: session.poll_shell_integration() as u8,
            };
        }
        PollAllResult::default()
    }

    pub fn cwd(&self) -> String {
        if let Some(ref session_arc) = self.session
            && let Ok(session) = session_arc.lock()
        {
            return session.cwd();
        }
        String::new()
    }

    pub fn focus_event(&mut self, focused: bool) {
        if let Some(ref session_arc) = self.session
            && let Ok(mut session) = session_arc.lock()
        {
            session.focus_event(focused);
        }
    }

    pub fn recompute_grid(&mut self, width: u32, height: u32) {
        let (cw, ch) = self.font_pipeline.cell_metrics();
        let new_cols = (width as f32 / cw)
            .floor()
            .clamp(super::MIN_COLS, super::MAX_COLS) as u32;
        let new_rows = (height as f32 / ch)
            .floor()
            .clamp(super::MIN_ROWS, super::MAX_ROWS) as u32;

        self.render_width = width;
        self.render_height = height;

        if width != self.surface_width.load(Ordering::Relaxed)
            || height != self.surface_height.load(Ordering::Relaxed)
        {
            self.surface_width.store(width, Ordering::Relaxed);
            self.surface_height.store(height, Ordering::Relaxed);
        }

        if new_cols != self.cols || new_rows != self.rows {
            log::info!(
                "RECOMPUTE_GRID: {}x{} -> {}x{} (cell={:.1}x{:.1})",
                self.rows,
                self.cols,
                new_rows,
                new_cols,
                cw,
                ch,
            );
            self.rows = new_rows;
            self.cols = new_cols;
            if let Some(ref session_arc) = self.session
                && let Ok(mut session) = session_arc.lock()
                && let Err(error) = session.resize(new_rows, new_cols)
            {
                log::error!("surface: session resize failed: {error}");
            }
        }
    }

    pub fn resize(&mut self, rows: u32, cols: u32) {
        log::trace!(
            "SURFACE_RESIZE: rows={} cols={} has_session={}",
            rows,
            cols,
            self.session.is_some(),
        );
        self.rows = rows;
        self.cols = cols;
        if let Some(ref session_arc) = self.session {
            let mut session = lock_or_recover(session_arc, "resize");
            if let Err(error) = session.resize(rows, cols) {
                log::error!("surface: session resize failed: {error}");
            }
        }
    }

    pub fn save_session(&self, path: &str) -> Result<(), SurfaceError> {
        use std::fs;

        let session_ref = self.session.as_ref().ok_or(SurfaceError::NoSession)?;
        let guard = session_ref.lock().map_err(|_| SurfaceError::NoSession)?;
        let dumped = guard.terminal().dump_grid();
        let (rows, cols) = (dumped.rows, dumped.cols);

        let mut visible_lines = Vec::with_capacity(rows as usize);
        for row in 0..rows as usize {
            let start = row * cols as usize;
            let row_cells = &dumped.visible[start..start + cols as usize];
            visible_lines.push(cell_to_line(row_cells, cols));
        }

        let mut scrollback_lines = Vec::with_capacity(dumped.scrollback.len());
        for sb_row in &dumped.scrollback {
            scrollback_lines.push(cell_to_line(sb_row, cols));
        }

        let snapshot = SessionSnapshot {
            visible_lines,
            scrollback_lines,
            rows,
            cols,
            max_scrollback: DEFAULT_MAX_SCROLLBACK,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot)
            .map_err(|e| SurfaceError::Session(format!("rkyv serialize: {e}")))?;
        fs::write(path, &bytes).map_err(|e| SurfaceError::Session(format!("write failed: {e}")))?;
        Ok(())
    }

    /// Requirement 4 (session restore, Fix G): rebuild the scrollback/visible text
    /// so a restored session matches the saved row count without a spurious
    /// trailing blank line. Pure + testable.
    ///
    /// 1. NUL padding becomes a real space (cell_to_line already maps a NUL
    ///    codepoint to ' ', but normalize defensively) so blank rows carry
    ///    visible blank content instead of garbage.
    /// 2. Do NOT per-line `trim_end`: an intentional blank line is spaces, and
    ///    trimming would collapse it. Middle blank lines must survive.
    /// 3. Trim only genuinely-empty TRAILING lines (whitespace-only). The shell
    ///    echoes the re-fed text and emits one prompt newline; a trailing
    ///    whitespace-only row from the save/restore is the off-by-one extra
    ///    blank line, so it is dropped here. Rows are joined with a single '\n'
    ///    and NO trailing newline, which avoids advancing the cursor onto an
    ///    extra empty row.
    pub fn restore_session_lines_to_text(snapshot: &SessionSnapshot) -> String {
        let mut lines: Vec<String> = snapshot
            .scrollback_lines
            .iter()
            .chain(&snapshot.visible_lines)
            .map(|line| line_to_text(line).replace('\0', " "))
            .collect();
        while let Some(last) = lines.last()
            && last.trim().is_empty()
        {
            lines.pop();
        }
        lines.join("\n")
    }

    pub fn restore_session(&mut self, path: &str) -> Result<(), SurfaceError> {
        use rkyv::rancor;
        use std::fs;

        let data =
            fs::read(path).map_err(|e| SurfaceError::Session(format!("read failed: {e}")))?;
        let snapshot = rkyv::from_bytes::<SessionSnapshot, rancor::Error>(&data)
            .map_err(|e| SurfaceError::Session(format!("rkyv deserialize: {e}")))?;

        if let Some(ref session_arc) = self.session
            && let Ok(mut session) = session_arc.lock()
        {
            let text = Self::restore_session_lines_to_text(&snapshot);
            if !text.is_empty() {
                session.terminal_mut().pty_write(text.as_bytes());
            }
        }

        if let Err(error) = fs::remove_file(path) {
            log::warn!("surface: failed to remove temp file {path:?}: {error}");
        }
        Ok(())
    }

    pub fn has_saved_session(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}
