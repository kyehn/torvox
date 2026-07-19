use crate::lock_util::lock_or_recover;

use super::types::*;

#[cfg(target_os = "android")]
unsafe extern "C" {
    fn torvox_bridge_poll_all(handle: i64) -> i64;
}

pub struct TorvoxBridge {
    pub(crate) config: TerminalConfig,
    pub(crate) surface: std::sync::Mutex<Option<crate::surface::AndroidSurface>>,
    pub(crate) session: std::sync::Mutex<
        Option<std::sync::Arc<std::sync::Mutex<torvox_terminal::session::Session>>>,
    >,
    pub(crate) scroll_offset: std::sync::atomic::AtomicU32,
    pub(crate) surface_ready: std::sync::atomic::AtomicBool,
    pub(crate) cell_width: std::sync::atomic::AtomicU32,
    pub(crate) cell_height: std::sync::atomic::AtomicU32,
    pub(crate) scrollback_length: std::sync::atomic::AtomicU32,
    pub(crate) user_write_tx: std::sync::Mutex<Option<flume::Sender<Vec<u8>>>>,
    pub(crate) key_cmd_tx:
        std::sync::Mutex<Option<flume::Sender<torvox_terminal::ghostty_terminal::Command>>>,
}

impl TorvoxBridge {
    pub(crate) fn active_session(
        &self,
    ) -> Result<std::sync::Arc<std::sync::Mutex<torvox_terminal::session::Session>>, BridgeError>
    {
        let guard = lock_session!(self);
        guard
            .as_ref()
            .cloned()
            .ok_or(BridgeError::SessionUnavailable {
                detail: "no active session".into(),
            })
    }

    fn shell_env(&self) -> torvox_terminal::shell_env::ShellEnv {
        if self.config.home.is_empty() {
            torvox_terminal::shell_env::ShellEnv::default()
        } else {
            let working_directory = if self.config.working_directory.is_empty() {
                self.config.home.clone()
            } else {
                self.config.working_directory.clone()
            };
            let android_vars = [
                "ANDROID_ASSETS",
                "ANDROID_DATA",
                "ANDROID_ROOT",
                "ANDROID_STORAGE",
                "ANDROID_RUNTIME_ROOT",
                "ANDROID_ART_ROOT",
                "ANDROID_I18N_ROOT",
                "ANDROID_TZDATA_ROOT",
                "BOOTCLASSPATH",
                "EXTERNAL_STORAGE",
            ];
            let extra: Vec<(String, String)> = android_vars
                .into_iter()
                .filter_map(|key| std::env::var(key).ok().map(|val| (key.to_string(), val)))
                .collect();
            let prefix = if self.config.prefix.is_empty() {
                None
            } else {
                Some(self.config.prefix.clone())
            };
            torvox_terminal::shell_env::ShellEnv {
                home: self.config.home.clone(),
                user: self.config.user.clone(),
                path: self.config.path.clone(),
                working_directory,
                prefix,
                extra,
            }
        }
    }

    pub(crate) fn store_cell_metrics(&self, surface: &crate::surface::AndroidSurface) {
        let (cell_width, cell_height) = surface.font_pipeline().cell_metrics();
        let prev_w = f32::from_bits(self.cell_width.load(std::sync::atomic::Ordering::Relaxed));
        let prev_h = f32::from_bits(self.cell_height.load(std::sync::atomic::Ordering::Relaxed));
        if (prev_w - cell_width).abs() > 0.01 || (prev_h - cell_height).abs() > 0.01 {
            log::trace!(
                "store_cell_metrics: cell_width={} cell_height={}",
                cell_width,
                cell_height
            );
        }
        self.cell_width
            .store(cell_width.to_bits(), std::sync::atomic::Ordering::Relaxed);
        self.cell_height
            .store(cell_height.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }

    fn process_session_for_render(
        &self,
    ) -> Result<(bool, torvox_terminal::ghostty_terminal::GridSnapshot), BridgeError> {
        let session_arc = self.active_session()?;
        let mut session_guard = session_arc.lock().map_err(|_| BridgeError::Lock {
            context: "session inner".into(),
        })?;
        let scroll_offset = self
            .scroll_offset
            .load(std::sync::atomic::Ordering::Relaxed);
        let had_output = session_guard.process_output();
        let mut attempts = 0;
        let snapshot = loop {
            if let Some(snap) = session_guard
                .terminal()
                .try_take_snapshot_with_scroll(scroll_offset)
            {
                break snap;
            }
            attempts += 1;
            if attempts >= 5 {
                return Err(BridgeError::Pty(
                    "snapshot unavailable — VT thread busy".into(),
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            session_guard.process_output();
        };
        Ok((had_output, snapshot))
    }

    fn process_session_for_render_skip_output(
        &self,
    ) -> Result<(bool, torvox_terminal::ghostty_terminal::GridSnapshot), BridgeError> {
        let session_arc = self.active_session()?;
        let session_guard = session_arc.lock().map_err(|_| BridgeError::Lock {
            context: "session inner".into(),
        })?;
        let scroll_offset = self
            .scroll_offset
            .load(std::sync::atomic::Ordering::Relaxed);
        let mut attempts = 0;
        let snapshot = loop {
            if let Some(snap) = session_guard
                .terminal()
                .try_take_snapshot_with_scroll(scroll_offset)
            {
                break snap;
            }
            attempts += 1;
            if attempts >= 5 {
                return Err(BridgeError::Pty(
                    "snapshot unavailable — VT thread busy".into(),
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        Ok((false, snapshot))
    }
}

pub fn with_bridge<F, T>(handle: i64, f: F) -> Result<T, TerminalError>
where
    F: FnOnce(&TorvoxBridge) -> Result<T, BridgeError>,
{
    let ptr = handle as *const TorvoxBridge;
    if ptr.is_null() {
        return Err(TerminalError::InvalidConfig {
            detail: "null bridge handle".to_string(),
        });
    }
    // SAFETY: ptr is non-null (checked above) and was created from a Box<TorvoxBridge>
    // that was leaked with Box::into_raw() in the caller. Alignment is guaranteed because
    // the pointer came from a well-aligned Box allocation. The lifetime is bounded by
    // catch_unwind which prevents unwinding past the FFI boundary. The caller must
    // ensure the handle remains valid for the duration of the call and is only freed
    // once after all concurrent calls complete.
    let bridge = unsafe { &*ptr };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(bridge))) {
        Ok(result) => result.map_err(TerminalError::from),
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic: {}", message);
            Err(TerminalError::PtyError { detail: message })
        }
    }
}

#[boltffi::export]
impl TorvoxBridge {
    pub fn new(config: TerminalConfig) -> Self {
        #[cfg(target_os = "android")]
        crate::logging::init();
        std::panic::set_hook(Box::new(|info| {
            log::error!("PANIC: {info}");
            if let Some(location) = info.location() {
                log::error!("  at {}:{}", location.file(), location.line());
            }
        }));
        Self {
            config,
            surface: std::sync::Mutex::new(None),
            session: std::sync::Mutex::new(None),
            scroll_offset: std::sync::atomic::AtomicU32::new(0),
            cell_width: std::sync::atomic::AtomicU32::new(0),
            cell_height: std::sync::atomic::AtomicU32::new(0),
            surface_ready: std::sync::atomic::AtomicBool::new(false),
            scrollback_length: std::sync::atomic::AtomicU32::new(0),
            user_write_tx: std::sync::Mutex::new(None),
            key_cmd_tx: std::sync::Mutex::new(None),
        }
    }

    pub fn ping(&self) -> Result<String, BridgeError> {
        let ptr = self as *const TorvoxBridge;
        log::info!(
            "ping: self={:p}, aligned={}",
            ptr,
            (ptr as usize).is_multiple_of(8)
        );
        Ok("pong".to_string())
    }

    pub fn spawn_terminal(&self, _rows: u32, _cols: u32) -> Result<i32, BridgeError> {
        let shell: torvox_core::config::Shell = self.config.shell.clone().into();
        let shell_path = match &shell {
            torvox_core::config::Shell::SystemDefault => "/system/bin/sh",
            torvox_core::config::Shell::Custom(path) => path.as_str(),
        };
        let mut surface_guard = lock_surface!(self);
        let surface = surface_guard.as_mut().ok_or(BridgeError::InvalidConfig {
            detail: "no surface — call set_native_window first".into(),
        })?;
        let env = self.shell_env();
        let session_arc = surface
            .spawn_session(shell_path, &env)
            .map_err(|e| BridgeError::Pty(e.to_string()))?;
        let mut session_guard = lock_or_recover(&self.session, "spawn_terminal");
        *session_guard = Some(session_arc.clone());
        if let Ok(session_guard) = session_arc.lock() {
            let user_tx = session_guard.user_write_sender();
            if let Ok(mut guard) = self.user_write_tx.lock() {
                *guard = Some(user_tx);
            }
            let key_tx = session_guard.clone_cmd_tx();
            if let Ok(mut guard) = self.key_cmd_tx.lock() {
                *guard = Some(key_tx);
            }
        }
        Ok(0)
    }

    pub fn set_native_window(
        &self,
        window_ptr: i64,
        width: u32,
        height: u32,
    ) -> Result<(), BridgeError> {
        log::debug!(
            "set_native_window: window_ptr={:#x}, width={}, height={}",
            window_ptr,
            width,
            height
        );
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .update_native_window(window_ptr as *mut std::ffi::c_void, width, height)
                .map_err(|e| BridgeError::Render(e.to_string()))?;
        } else {
            let mut surface = crate::surface::AndroidSurface::new(
                self.config.rows,
                self.config.cols,
                self.config.scrollback_lines,
                self.config.font_size_tenths as f32 / 10.0,
            );
            surface
                .set_native_window(
                    window_ptr as *mut std::ffi::c_void,
                    width,
                    height,
                    self.config.font_size_tenths,
                )
                .map_err(|e| BridgeError::Render(e.to_string()))?;
            {
                let t = &self.config.theme;
                let bg = [
                    ((t.bg >> 16) & 0xFF) as u8,
                    ((t.bg >> 8) & 0xFF) as u8,
                    (t.bg & 0xFF) as u8,
                ];
                let fg = [
                    ((t.fg >> 16) & 0xFF) as u8,
                    ((t.fg >> 8) & 0xFF) as u8,
                    (t.fg & 0xFF) as u8,
                ];
                let cursor = [
                    ((t.cursor >> 16) & 0xFF) as u8,
                    ((t.cursor >> 8) & 0xFF) as u8,
                    (t.cursor & 0xFF) as u8,
                ];
                log::info!(
                    "BRIDGE_DIAG: set_theme name='{}' bg={:?} fg={:?} cursor={:?}",
                    t.name,
                    bg,
                    fg,
                    cursor,
                );
            }
            surface.set_theme(self.config.theme.clone().into());
            *surface_guard = Some(surface);
        }
        if let Some(surface) = surface_guard.as_ref() {
            self.store_cell_metrics(surface);
        }
        self.surface_ready
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    pub fn render(&self, skip_output: bool) -> Result<bool, BridgeError> {
        let session_out = if skip_output {
            self.process_session_for_render_skip_output()?
        } else {
            self.process_session_for_render()?
        };

        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            let scroll_offset = self
                .scroll_offset
                .load(std::sync::atomic::Ordering::Relaxed);
            let result = surface
                .render_frame(scroll_offset, session_out.0, session_out.1)
                .map_err(|e| BridgeError::Render(e.to_string()));
            self.store_cell_metrics(surface);
            if session_out.0
                && let Some(session_arc) =
                    self.session.lock().ok().and_then(|g| g.as_ref().cloned())
                && let Ok(session) = session_arc.lock()
            {
                self.scrollback_length.store(
                    session.terminal().scrollback_length(),
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
            result
        } else {
            Ok(false)
        }
    }

    pub fn save_test_frame(&self, data_dir: &str) -> Result<String, BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .save_test_frame(data_dir)
                .map_err(|e| BridgeError::Render(e.to_string()))
        } else {
            Err(BridgeError::Render("no surface".into()))
        }
    }

    pub fn save_test_frame_with_selection(
        &self,
        data_dir: &str,
        start_row: i32,
        start_col: i32,
        end_row: i32,
        end_col: i32,
        active: bool,
        mode: u8,
    ) -> Result<String, BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            if active && start_row >= 0 && end_row >= 0 {
                surface.set_selection(Some(torvox_renderer::gpu::SelectionRange {
                    start_row,
                    start_col,
                    end_row,
                    end_col,
                    active: true,
                    mode: torvox_core::selection::SelectionMode::from_u8(mode),
                    origin: None,
                    is_empty: false,
                }));
            } else {
                surface.set_selection(None);
            }
            surface
                .save_test_frame(data_dir)
                .map_err(|e| BridgeError::Render(e.to_string()))
        } else {
            Err(BridgeError::Render("no surface".into()))
        }
    }

    pub fn poll_bel(&self) -> bool {
        let mut surface_guard = lock_or_recover(&self.surface, "poll_bel");
        surface_guard
            .as_mut()
            .map(|s| s.poll_bel())
            .unwrap_or(false)
    }

    pub fn poll_clipboard(&self) -> Option<String> {
        let mut surface_guard = lock_or_recover(&self.surface, "poll_clipboard");
        surface_guard.as_mut()?.poll_clipboard()
    }

    pub(crate) fn poll_notification_raw(&self) -> Option<(String, String)> {
        let mut surface_guard = lock_or_recover(&self.surface, "poll_notification_raw");
        surface_guard.as_mut()?.poll_notification()
    }

    pub fn poll_shell_integration(&self) -> u8 {
        let mut surface_guard = lock_or_recover(&self.surface, "poll_shell_integration");
        surface_guard
            .as_mut()
            .map(|s| s.poll_shell_integration())
            .unwrap_or(0)
    }

    pub fn poll_sync_active(&self) -> bool {
        let mut surface_guard = lock_or_recover(&self.surface, "poll_sync_active");
        surface_guard
            .as_mut()
            .map(|s| s.poll_sync_active())
            .unwrap_or(false)
    }

    pub fn cwd(&self) -> String {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            return session.cwd();
        }
        String::new()
    }

    pub fn focus_event(&self, focused: bool) {
        let Ok(mut guard) = self.surface.lock() else {
            return;
        };
        let Some(s) = guard.as_mut() else { return };
        s.focus_event(focused);
    }

    pub fn resize(&self, rows: u32, cols: u32) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.resize(rows, cols);
        }
        Ok(())
    }

    pub fn set_surface_size(&self, width: u32, height: u32) {
        if let Ok(mut guard) = self.surface.lock()
            && let Some(surface) = guard.as_mut()
        {
            surface.set_surface_size(width, height);
        }
    }

    pub fn recompute_grid(&self, width: u32, height: u32) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.recompute_grid(width, height);
        }
        Ok(())
    }

    pub fn update_native_window(
        &self,
        window_ptr: i64,
        width: u32,
        height: u32,
    ) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_theme(self.config.theme.clone().into());
            surface
                .update_native_window(window_ptr as *mut std::ffi::c_void, width, height)
                .map_err(|e| BridgeError::Render(e.to_string()))?;
            self.store_cell_metrics(surface);
        }
        self.surface_ready
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    pub fn release_surface(&self) {
        self.surface_ready
            .store(false, std::sync::atomic::Ordering::Release);
        if let Ok(mut guard) = self.surface.lock() {
            *guard = None;
        }
    }

    pub fn release_gpu_surface(&self) {
        if let Ok(mut guard) = self.surface.lock()
            && let Some(surface) = guard.as_mut()
        {
            surface.release_gpu_surface();
        }
    }

    pub fn set_scroll_offset(&self, offset: u32) {
        self.scroll_offset
            .store(offset, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut guard) = self.surface.lock()
            && let Some(surface) = guard.as_mut()
        {
            surface.set_render_requested(true);
        }
    }

    pub fn wait_until_ready_for_render(&self) {
        let mut attempts = 0u32;
        while !self
            .surface_ready
            .load(std::sync::atomic::Ordering::Acquire)
            && attempts < 50
        {
            std::thread::sleep(std::time::Duration::from_millis(1));
            attempts += 1;
        }
        if attempts >= 50 {
            log::warn!("wait_until_ready_for_render: surface not ready after 50ms");
        }
    }

    pub fn scrollback_length(&self) -> u32 {
        self.scrollback_length
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn scrollback_line(&self, index: u32) -> Option<String> {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            return session.terminal().read_line_text(index);
        }
        None
    }

    pub fn get_config(&self) -> TerminalConfig {
        self.config.clone()
    }

    pub fn get_theme_names(&self) -> String {
        torvox_core::config::Theme::all_built_in()
            .into_iter()
            .map(|t| t.name)
            .collect::<Vec<_>>()
            .join(LIST_SEPARATOR)
    }

    pub fn get_theme(&self, name: String) -> Option<BridgeTheme> {
        torvox_core::config::Theme::all_built_in()
            .into_iter()
            .find(|t| t.name == name)
            .map(|t| t.into())
    }

    pub fn set_font_size(&self, size_tenths: u32) -> Result<(), BridgeError> {
        let size = size_tenths as f32 / 10.0;
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_font_size(size);
        }
        Ok(())
    }

    pub fn set_extra_font_paths(&self, _paths: Vec<String>) {
        #[cfg(target_os = "android")]
        {
            let path_bufs: Vec<std::path::PathBuf> =
                _paths.into_iter().map(std::path::PathBuf::from).collect();
            torvox_renderer::font::set_extra_font_paths(path_bufs);
        }
    }

    pub fn set_font_size_in_place(&self, size_tenths: u32) -> Result<(), BridgeError> {
        let size = size_tenths as f32 / 10.0;
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_font_size_in_place(size);
        }
        Ok(())
    }

    pub fn set_selection(
        &self,
        start_row: i32,
        start_col: i32,
        end_row: i32,
        end_col: i32,
        active: bool,
        mode: u8,
    ) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            if active && start_row >= 0 && end_row >= 0 {
                let range = crate::bridge::selection::range_from(
                    torvox_core::selection::SelectionAnchor {
                        row: start_row as u32,
                        col: start_col as u32,
                    },
                    torvox_core::selection::SelectionAnchor {
                        row: end_row as u32,
                        col: end_col as u32,
                    },
                    torvox_core::selection::SelectionMode::from_u8(mode),
                    None,
                    false,
                );
                surface.set_selection(Some(range));
            } else {
                surface.set_selection(None);
            }
        }
        Ok(())
    }

    pub fn get_selected_text(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            let terminal = session.terminal();
            let mut result = String::new();
            for row in start_row..=end_row {
                if let Some(line) = terminal.read_line_text(row) {
                    let from = if row == start_row {
                        start_col as usize
                    } else {
                        0
                    };
                    let to = if row == end_row {
                        (end_col as usize).min(line.len())
                    } else {
                        line.len()
                    };
                    if from < to {
                        result.push_str(&line[from..to]);
                        result.push('\n');
                    }
                }
            }
            let trimmed = result.trim_end_matches('\n').to_string();
            return trimmed;
        }
        String::new()
    }

    pub fn is_cell_empty(&self, row: u32, col: u32) -> bool {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            if let Ok(guard) = self.session.lock()
                && let Some(session_arc) = guard.as_ref()
                && let Ok(session) = session_arc.lock()
            {
                let snapshot = session.terminal().take_snapshot();
                let idx = (row * snapshot.cols + col) as usize;
                let ch = snapshot.cells.get(idx).and_then(|c| char::from_u32(c.codepoint));
                return matches!(ch, None | Some('\0'));
            }
        }
        true
    }

    pub fn has_text_in_range(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> bool {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            if let Ok(guard) = self.session.lock()
                && let Some(session_arc) = guard.as_ref()
                && let Ok(session) = session_arc.lock()
            {
                let snapshot = session.terminal().take_snapshot();
                for r in start_row..=end_row {
                    let cstart = if r == start_row { start_col } else { 0 };
                    let cend = if r == end_row { end_col } else { snapshot.cols - 1 };
                    for c in cstart..=cend {
                        let idx = (r * snapshot.cols + c) as usize;
                        if let Some(cell) = snapshot.cells.get(idx) {
                            if let Some(ch) = char::from_u32(cell.codepoint) {
                                if ch != '\0' && !ch.is_whitespace() {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub fn expand_and_set_selection(
        &self,
        row: u32,
        col: u32,
        mode: u8,
    ) -> Result<(u32, u32, u32, u32), BridgeError> {
        let mode_enum = torvox_core::selection::SelectionMode::from_u8(mode);
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            if let Ok(guard) = self.session.lock()
                && let Some(session_arc) = guard.as_ref()
                && let Ok(session) = session_arc.lock()
            {
                let snapshot = session.terminal().take_snapshot();
                let anchor = torvox_core::selection::SelectionAnchor { row, col };
                let (start, end) = crate::bridge::selection::expand_at(
                    &snapshot.cells,
                    snapshot.cols,
                    anchor,
                    mode_enum,
                    torvox_core::selection::ExpansionOptions::default(),
                );

                surface.set_selection(Some(crate::bridge::selection::range_from(
                    start,
                    end,
                    mode_enum,
                    Some((row as i32, col as i32)),
                    false,
                )));
                return Ok((start.row, start.col, end.row, end.col));
            }
            surface.set_selection(None);
        }
        Ok((row, col, row, col))
    }

    pub fn set_selection_endpoint(
        &self,
        params: SelectionEndpointParams,
    ) -> Result<(u32, u32, u32, u32), BridgeError> {
        let mode_enum = torvox_core::selection::SelectionMode::from_u8(params.mode);
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            if let Ok(guard) = self.session.lock()
                && let Some(session_arc) = guard.as_ref()
                && let Ok(session) = session_arc.lock()
            {
                let snapshot = session.terminal().take_snapshot();

                let fixed = torvox_core::selection::SelectionAnchor {
                    row: params.other_row.max(0) as u32,
                    col: params.other_col.max(0) as u32,
                };
                let moved = torvox_core::selection::SelectionAnchor {
                    row: params.anchor_row.max(0) as u32,
                    col: params.anchor_col.max(0) as u32,
                };

                let (new_start, new_end) =
                    if mode_enum == torvox_core::selection::SelectionMode::Word {
                        let (mws, mwe) = crate::bridge::selection::expand_at(
                            &snapshot.cells,
                            snapshot.cols,
                            moved,
                            mode_enum,
                            torvox_core::selection::ExpansionOptions::default(),
                        );
                        if params.handle_side == 0 {
                            (mws, fixed)
                        } else {
                            (fixed, mwe)
                        }
                    } else if params.handle_side == 0 {
                        (moved, fixed)
                    } else {
                        (fixed, moved)
                    };

                surface.set_selection(Some(crate::bridge::selection::range_from(
                    new_start,
                    new_end,
                    mode_enum,
                    Some((params.origin_row, params.origin_col)),
                    false,
                )));
                let (lo, hi) = if new_start.row < new_end.row
                    || (new_start.row == new_end.row && new_start.col <= new_end.col)
                {
                    (new_start, new_end)
                } else {
                    (new_end, new_start)
                };
                log::debug!(
                    "set_selection_endpoint handle={} mode={} -> start=({},{}), end=({},{}), anchor=({},{}), fixed=({},{}), origin=({},{}), moved_word=({},{}),({},{}))",
                    params.handle_side,
                    params.mode,
                    lo.row,
                    lo.col,
                    hi.row,
                    hi.col,
                    params.anchor_row,
                    params.anchor_col,
                    params.other_row,
                    params.other_col,
                    params.origin_row,
                    params.origin_col,
                    new_start.row,
                    new_start.col,
                    new_end.row,
                    new_end.col,
                );
                return Ok((lo.row, lo.col, hi.row, hi.col));
            }
            surface.set_selection(None);
        }
        Ok((
            params.anchor_row.max(0) as u32,
            params.anchor_col.max(0) as u32,
            params.anchor_row.max(0) as u32,
            params.anchor_col.max(0) as u32,
        ))
    }

    pub fn set_search_highlights(&self, serialized: Vec<u8>) -> Result<(), BridgeError> {
        let data = serialized;
        if data.len() < 4 {
            let mut surface_guard = lock_surface!(self);
            if let Some(surface) = surface_guard.as_mut() {
                surface.clear_search_highlights();
            }
            return Ok(());
        }
        let count = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        log::info!(
            "set_search_highlights: count={}, data.len={}",
            count,
            data.len()
        );
        let mut highlights = Vec::with_capacity(count.max(0) as usize);
        let mut pos = 4usize;
        for _ in 0..count.max(0) {
            if pos + 16 > data.len() {
                break;
            }
            let row = i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let start_col =
                i32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
            let end_col_exclusive =
                i32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
            let red = data[pos + 12];
            let green = data[pos + 13];
            let blue = data[pos + 14];
            let alpha = data[pos + 15];
            log::info!(
                "  highlight[{}]: row={}, cols={}..{}, rgba=({},{},{},{})",
                highlights.len(),
                row,
                start_col,
                end_col_exclusive,
                red,
                green,
                blue,
                alpha
            );
            highlights.push(torvox_renderer::gpu::SearchHighlight {
                row,
                start_col,
                end_col_exclusive,
                color: [red, green, blue, alpha],
            });
            pos += 16;
        }
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_search_highlights(highlights);
        }
        Ok(())
    }

    pub fn set_font_family(&self, family_name: String) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        let surface = surface_guard.as_mut().ok_or(BridgeError::InvalidConfig {
            detail: "no surface available".into(),
        })?;
        if !surface.set_font_family(&family_name) {
            return Err(BridgeError::InvalidConfig {
                detail: "font family not found".into(),
            });
        }
        Ok(())
    }

    pub fn set_theme(&self, theme: BridgeTheme) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_theme(theme.into());
        }
        Ok(())
    }

    pub fn search_in_scrollback(&self, query: String) -> Option<String> {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            return session
                .terminal()
                .search_in_scrollback(&query)
                .map(|(r, c)| format!("{r},{c}"));
        }
        None
    }

    pub fn search_all_in_scrollback(
        &self,
        query: String,
        case_sensitive: bool,
        fuzzy: bool,
    ) -> String {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            let matches =
                session
                    .terminal()
                    .search_all_in_scrollback(&query, case_sensitive, fuzzy);
            if matches.is_empty() {
                return String::new();
            }
            use std::fmt::Write;
            let mut result = String::with_capacity(matches.len() * 16);
            for m in &matches {
                if !result.is_empty() {
                    result.push(';');
                }
                let _ = write!(result, "{},{},{}", m.row, m.start_col, m.end_col);
            }
            return result;
        }
        String::new()
    }

    pub fn list_fonts(&self) -> String {
        let guard = lock_or_recover(&self.surface, "list_fonts");
        guard
            .as_ref()
            .map(|s| {
                s.font_pipeline()
                    .list_monospace_fonts()
                    .join(LIST_SEPARATOR)
            })
            .unwrap_or_default()
    }

    pub fn get_default_font_name(&self) -> String {
        self.surface
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.font_pipeline().default_font_name()))
            .unwrap_or_else(|| "monospace".to_string())
    }

    pub fn set_system_locale(&self, locale: &str) {
        if let Ok(mut guard) = self.surface.lock()
            && let Some(surface) = guard.as_mut()
        {
            surface.font_pipeline_mut().set_system_locale(locale);
        }
    }

    pub fn get_font_info(&self) -> String {
        self.surface
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.font_pipeline().font_information()))
            .unwrap_or_else(|| "No font loaded".to_string())
    }

    pub fn list_font_families(&self) -> String {
        let guard = lock_or_recover(&self.surface, "list_font_families");
        guard
            .as_ref()
            .map(|s| s.font_pipeline().list_all_font_families().join("\x1f"))
            .unwrap_or_default()
    }

    pub fn load_font_file(&self, path: String) -> Option<String> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut surface_guard = lock_or_recover(&self.surface, "load_font_file");
            let surface = surface_guard.as_mut()?;
            let std_path = std::path::PathBuf::from(&path);
            let family = surface.load_font_file(&std_path);
            if let Some(ref name) = family {
                log::info!("FONT_LOAD_FILE: loaded '{}' -> family '{}'", path, name);
            } else {
                log::warn!("FONT_LOAD_FILE: failed to load '{}'", path);
            }
            family
        }));
        match result {
            Ok(family) => family,
            Err(_) => {
                log::error!("FONT_LOAD_FILE: panic in load_font_file for '{}'", path);
                None
            }
        }
    }

    pub fn set_save_path(&self, path: String) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_save_path(path);
        }
        Ok(())
    }

    pub fn save_session(&self, path: String) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .save_session(&path)
                .map_err(|e| BridgeError::Render(e.to_string()))?;
        }
        Ok(())
    }

    pub fn restore_session(&self, path: String) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .restore_session(&path)
                .map_err(|e| BridgeError::Render(e.to_string()))?;
        }
        Ok(())
    }

    pub fn has_saved_session(&self, path: String) -> bool {
        crate::surface::AndroidSurface::has_saved_session(&path)
    }

    pub fn set_mouse_position(&self, row: u32, col: u32) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_mouse_position(row, col);
        }
        Ok(())
    }

    pub fn get_hovered_url(&self) -> Option<String> {
        self.surface
            .lock()
            .ok()
            .and_then(|g| g.as_ref().and_then(|s| s.get_hovered_url()))
    }

    pub fn get_terminal_text(&self) -> String {
        let text = if let Ok(guard) = self.session.lock() {
            if let Some(session_arc) = guard.as_ref() {
                if let Ok(session) = session_arc.lock() {
                    session.terminal().read_visible_text()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        let preview_end = text
            .char_indices()
            .nth(TEXT_PREVIEW_MAX_CHARS)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        log::debug!(
            "get_terminal_text: len={}, text={:?}",
            text.len(),
            &text[..preview_end]
        );
        text
    }

    pub fn get_active_session_title(&self) -> String {
        self.surface
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.get_title()))
            .unwrap_or_default()
    }

    pub fn get_grid_rows(&self) -> u32 {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            return session.terminal().rows();
        }
        DEFAULT_GRID_ROWS
    }

    pub fn get_grid_cols(&self) -> u32 {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            return session.terminal().cols();
        }
        DEFAULT_GRID_COLS
    }

    pub fn get_grid_rows_cols(&self) -> (u32, u32) {
        if let Ok(guard) = self.session.lock()
            && let Some(session_arc) = guard.as_ref()
            && let Ok(session) = session_arc.lock()
        {
            let terminal = session.terminal();
            return (terminal.rows(), terminal.cols());
        }
        (DEFAULT_GRID_ROWS, DEFAULT_GRID_COLS)
    }

    pub fn get_cell_width(&self) -> f32 {
        f32::from_bits(self.cell_width.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn get_cell_height(&self) -> f32 {
        f32::from_bits(self.cell_height.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn write_to_pty(&self, data: Vec<u8>) -> Result<(), BridgeError> {
        let guard = self
            .user_write_tx
            .lock()
            .map_err(|_| BridgeError::SessionUnavailable {
                detail: "user-write channel mutex poisoned".into(),
            })?;
        match guard.as_ref() {
            Some(sender) => sender.send(data).map_err(|error| {
                log::error!("bridge: user PTY write channel closed: {error}");
                BridgeError::Pty(format!("user PTY write channel closed: {error}"))
            }),
            None => Err(BridgeError::SessionUnavailable {
                detail: "no active session — user-write channel not initialized".into(),
            }),
        }
    }

    pub fn process_key_event(
        &self,
        key_code: u32,
        modifiers: u8,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Result<(), BridgeError> {
        let rx = {
            let guard = self
                .key_cmd_tx
                .lock()
                .map_err(|_| BridgeError::SessionUnavailable {
                    detail: "key-cmd channel mutex poisoned".into(),
                })?;
            match guard.as_ref() {
                Some(cmd_tx) => {
                    torvox_terminal::ghostty_terminal::GhosttyTerminal::key_encode_submit_via(
                        cmd_tx,
                        key_code,
                        modifiers as u16,
                        action,
                        unicode_char,
                        unshifted_char,
                    )
                }
                None => {
                    return Err(BridgeError::SessionUnavailable {
                        detail: "no active session — key-cmd channel not initialized".into(),
                    });
                }
            }
        };
        let encoded = rx.and_then(|r| r.recv().ok());
        if let Some(encoded) = encoded
            && !encoded.is_empty()
        {
            log::trace!(
                "bridge: process_key_event key_code={key_code} modifiers={modifiers} action={action} unicode={unicode_char} encoded_len={}",
                encoded.len()
            );
            let guard = self
                .user_write_tx
                .lock()
                .map_err(|_| BridgeError::SessionUnavailable {
                    detail: "user-write channel mutex poisoned".into(),
                })?;
            match guard.as_ref() {
                Some(sender) => sender.send(encoded).map_err(|error| {
                    log::error!("bridge: key PTY write channel closed: {error}");
                    BridgeError::Pty(format!("key PTY write channel closed: {error}"))
                }),
                None => Err(BridgeError::SessionUnavailable {
                    detail: "no active session — user-write channel not initialized".into(),
                }),
            }
        } else {
            Ok(())
        }
    }

    pub fn set_background_image(
        &self,
        rgba_data: Vec<u8>,
        width: u32,
        height: u32,
    ) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_background_image(&rgba_data, width, height);
        }
        Ok(())
    }

    pub fn clear_background_image(&self) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.clear_background_image();
        }
        Ok(())
    }

    pub fn set_background_params(
        &self,
        blur_radius: i32,
        alpha_tenths: i32,
    ) -> Result<(), BridgeError> {
        let blur = blur_radius as f32;
        let alpha = alpha_tenths as f32 / 10.0;
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_background_params(blur, alpha);
        }
        Ok(())
    }

    pub fn set_render_paused(&self, paused: bool) {
        if let Ok(mut guard) = self.surface.lock()
            && let Some(surface) = guard.as_mut()
            && let Some(gpu) = surface.gpu_mut()
        {
            gpu.set_render_paused(paused);
        }
    }

    pub fn set_cursor_blink_enabled(&self, enabled: bool) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_blink_enabled(enabled);
        }
        Ok(())
    }

    pub fn set_cursor_blink_speed_ms(&self, speed_ms: u32) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_blink_speed_ms(speed_ms);
        }
        Ok(())
    }

    pub fn reset_cursor_blink(&self) -> Result<(), BridgeError> {
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.reset_blink();
        }
        Ok(())
    }

    pub fn set_cursor_style(&self, style: String) -> Result<(), BridgeError> {
        let cursor_style = match style.as_str() {
            "bar" => torvox_core::cursor::CursorStyle::Bar,
            "underline" => torvox_core::cursor::CursorStyle::Underline,
            _ => torvox_core::cursor::CursorStyle::Block,
        };
        let mut surface_guard = lock_surface!(self);
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_cursor_style(cursor_style);
        }
        Ok(())
    }
}

impl TorvoxBridge {
    pub fn poll_all(&self) -> PollAllResult {
        let mut surface_guard = lock_or_recover(&self.surface, "poll_all");
        surface_guard
            .as_mut()
            .map(|s| s.poll_all())
            .unwrap_or_default()
    }

    /// Wait for PTY output or timeout. Returns `true` if output arrived.
    /// This does NOT hold the session mutex — only the Condvar mutex.
    pub fn wait_for_output_timeout(&self, timeout_ms: u64) -> bool {
        let session_arc = {
            let Ok(guard) = self.session.lock() else {
                return false;
            };
            let Some(arc) = guard.as_ref().cloned() else {
                return false;
            };
            arc
        };
        let Ok(session) = session_arc.lock() else {
            return false;
        };
        session.wait_for_output_timeout(std::time::Duration::from_millis(timeout_ms))
    }
}
