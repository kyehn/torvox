//! BoltFFI data bridge — single export location for Rust↔Kotlin bridge types.
//!
//! # Requirements
//! - [FR-039](crate) — MCP: server lifecycle
//! - [FR-049](crate) — Bridge: boltffi ↔ JNA wire format
//! - [FR-050](crate) — Bridge: rkyv serialization

const DEFAULT_GRID_ROWS: u32 = 24;
const DEFAULT_GRID_COLS: u32 = 80;
/// FFI-safe bridge type for a terminal cell.
/// Maps to/from `torvox_core::cell::Cell`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub struct BridgeCell {
    pub char_code: u32,
    pub fg: u32,
    pub bg: u32,
    pub attrs: BridgeAttrs,
}

/// FFI-safe bridge type for text attributes (bold, italic, underline, etc.).
/// Maps to/from `torvox_core::cell::Attrs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[boltffi::data]
pub struct BridgeAttrs {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub double_underline: bool,
    pub reverse: bool,
    pub strikethrough: bool,
    pub blink: bool,
    pub hidden: bool,
    pub overline: bool,
    pub protected: bool,
    pub double_width: bool,
    pub double_height_top: bool,
    pub double_height_bottom: bool,
}

impl From<torvox_core::cell::Attrs> for BridgeAttrs {
    fn from(attributes: torvox_core::cell::Attrs) -> Self {
        Self {
            bold: attributes.bold,
            dim: attributes.dim,
            italic: attributes.italic,
            underline: attributes.underline,
            double_underline: attributes.double_underline,
            reverse: attributes.reverse,
            strikethrough: attributes.strikethrough,
            blink: attributes.blink,
            hidden: attributes.hidden,
            overline: attributes.overline,
            protected: attributes.protected,
            double_width: attributes.double_width,
            double_height_top: attributes.double_height_top,
            double_height_bottom: attributes.double_height_bottom,
        }
    }
}

impl From<BridgeAttrs> for torvox_core::cell::Attrs {
    fn from(attributes: BridgeAttrs) -> Self {
        Self {
            bold: attributes.bold,
            dim: attributes.dim,
            italic: attributes.italic,
            underline: attributes.underline,
            double_underline: attributes.double_underline,
            reverse: attributes.reverse,
            strikethrough: attributes.strikethrough,
            blink: attributes.blink,
            hidden: attributes.hidden,
            overline: attributes.overline,
            protected: attributes.protected,
            double_width: attributes.double_width,
            double_height_top: attributes.double_height_top,
            double_height_bottom: attributes.double_height_bottom,
        }
    }
}

impl From<torvox_core::cell::Cell> for BridgeCell {
    fn from(cell: torvox_core::cell::Cell) -> Self {
        Self {
            char_code: cell.char as u32,
            fg: ((cell.foreground.r as u32) << 24)
                | ((cell.foreground.g as u32) << 16)
                | ((cell.foreground.b as u32) << 8)
                | (cell.foreground.a as u32),
            bg: ((cell.background.r as u32) << 24)
                | ((cell.background.g as u32) << 16)
                | ((cell.background.b as u32) << 8)
                | (cell.background.a as u32),
            attrs: cell.attrs.into(),
        }
    }
}

impl From<BridgeCell> for torvox_core::cell::Cell {
    fn from(bridge_cell: BridgeCell) -> Self {
        Self {
            char: char::from_u32(bridge_cell.char_code).unwrap_or(' '),
            foreground: torvox_core::cell::Color {
                r: ((bridge_cell.fg >> 24) & 0xFF) as u8,
                g: ((bridge_cell.fg >> 16) & 0xFF) as u8,
                b: ((bridge_cell.fg >> 8) & 0xFF) as u8,
                a: (bridge_cell.fg & 0xFF) as u8,
            },
            background: torvox_core::cell::Color {
                r: ((bridge_cell.bg >> 24) & 0xFF) as u8,
                g: ((bridge_cell.bg >> 16) & 0xFF) as u8,
                b: ((bridge_cell.bg >> 8) & 0xFF) as u8,
                a: (bridge_cell.bg & 0xFF) as u8,
            },
            attrs: bridge_cell.attrs.into(),
            width: 1,
        }
    }
}

/// Shell configuration: system default or custom path.
/// Maps to/from `torvox_core::config::Shell`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[boltffi::data]
pub enum Shell {
    #[default]
    SystemDefault,
    Custom {
        path: String,
    },
}

impl From<Shell> for torvox_core::config::Shell {
    fn from(shell: Shell) -> Self {
        match shell {
            Shell::SystemDefault => torvox_core::config::Shell::SystemDefault,
            Shell::Custom { path } => torvox_core::config::Shell::Custom(path),
        }
    }
}

impl From<torvox_core::config::Shell> for Shell {
    fn from(core_shell: torvox_core::config::Shell) -> Self {
        match core_shell {
            torvox_core::config::Shell::SystemDefault => Shell::SystemDefault,
            torvox_core::config::Shell::Custom(path) => Shell::Custom { path },
        }
    }
}

/// FFI-safe bridge type for terminal color scheme.
/// Maps to/from `torvox_core::config::Theme`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub struct BridgeTheme {
    pub name: String,
    pub bg: u32,
    pub fg: u32,
    pub cursor: u32,
    pub selection_bg: u32,
    pub ansi0: u32,
    pub ansi1: u32,
    pub ansi2: u32,
    pub ansi3: u32,
    pub ansi4: u32,
    pub ansi5: u32,
    pub ansi6: u32,
    pub ansi7: u32,
    pub ansi8: u32,
    pub ansi9: u32,
    pub ansi10: u32,
    pub ansi11: u32,
    pub ansi12: u32,
    pub ansi13: u32,
    pub ansi14: u32,
    pub ansi15: u32,
}

fn rgb_to_u32(color: [u8; 3]) -> u32 {
    ((color[0] as u32) << 16) | ((color[1] as u32) << 8) | (color[2] as u32)
}

fn u32_to_rgb(value: u32) -> [u8; 3] {
    [
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    ]
}

impl From<torvox_core::config::Theme> for BridgeTheme {
    fn from(theme: torvox_core::config::Theme) -> Self {
        Self {
            name: theme.name,
            bg: rgb_to_u32(theme.background),
            fg: rgb_to_u32(theme.foreground),
            cursor: rgb_to_u32(theme.cursor),
            selection_bg: rgb_to_u32(theme.selection_bg),
            ansi0: rgb_to_u32(theme.ansi[0]),
            ansi1: rgb_to_u32(theme.ansi[1]),
            ansi2: rgb_to_u32(theme.ansi[2]),
            ansi3: rgb_to_u32(theme.ansi[3]),
            ansi4: rgb_to_u32(theme.ansi[4]),
            ansi5: rgb_to_u32(theme.ansi[5]),
            ansi6: rgb_to_u32(theme.ansi[6]),
            ansi7: rgb_to_u32(theme.ansi[7]),
            ansi8: rgb_to_u32(theme.ansi[8]),
            ansi9: rgb_to_u32(theme.ansi[9]),
            ansi10: rgb_to_u32(theme.ansi[10]),
            ansi11: rgb_to_u32(theme.ansi[11]),
            ansi12: rgb_to_u32(theme.ansi[12]),
            ansi13: rgb_to_u32(theme.ansi[13]),
            ansi14: rgb_to_u32(theme.ansi[14]),
            ansi15: rgb_to_u32(theme.ansi[15]),
        }
    }
}

impl From<BridgeTheme> for torvox_core::config::Theme {
    fn from(bridge_theme: BridgeTheme) -> Self {
        Self {
            name: bridge_theme.name,
            background: u32_to_rgb(bridge_theme.bg),
            foreground: u32_to_rgb(bridge_theme.fg),
            cursor: u32_to_rgb(bridge_theme.cursor),
            selection_bg: u32_to_rgb(bridge_theme.selection_bg),
            ansi: [
                u32_to_rgb(bridge_theme.ansi0),
                u32_to_rgb(bridge_theme.ansi1),
                u32_to_rgb(bridge_theme.ansi2),
                u32_to_rgb(bridge_theme.ansi3),
                u32_to_rgb(bridge_theme.ansi4),
                u32_to_rgb(bridge_theme.ansi5),
                u32_to_rgb(bridge_theme.ansi6),
                u32_to_rgb(bridge_theme.ansi7),
                u32_to_rgb(bridge_theme.ansi8),
                u32_to_rgb(bridge_theme.ansi9),
                u32_to_rgb(bridge_theme.ansi10),
                u32_to_rgb(bridge_theme.ansi11),
                u32_to_rgb(bridge_theme.ansi12),
                u32_to_rgb(bridge_theme.ansi13),
                u32_to_rgb(bridge_theme.ansi14),
                u32_to_rgb(bridge_theme.ansi15),
            ],
        }
    }
}

/// Terminal configuration sent from Kotlin to the Rust bridge at startup.
/// Maps to/from `torvox_core::config::TerminalConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub struct TerminalConfig {
    pub shell: Shell,
    pub rows: u32,
    pub cols: u32,
    pub scrollback_lines: u32,
    pub font_size_tenths: u32,
    pub theme: BridgeTheme,
    pub home: String,
    pub user: String,
    pub path: String,
    pub working_directory: String,
    pub prefix: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: Shell::SystemDefault,
            rows: 24,
            cols: 80,
            scrollback_lines: 50_000,
            font_size_tenths: 140,
            theme: torvox_core::config::Theme::catppuccin_mocha().into(),
            home: String::new(),
            user: String::new(),
            path: String::new(),
            working_directory: String::new(),
            prefix: String::new(),
        }
    }
}

impl TerminalConfig {
    /// Explicit, non-lossy conversion to the core `TerminalConfig`.
    ///
    /// The bridge `TerminalConfig` carries fields (home, user, path,
    /// working_directory, prefix, theme) that have no equivalent on the core
    /// type. Those fields are intentionally NOT represented on the core type
    /// (its wire format is stable and must not change), so they are dropped
    /// here. Callers that need them must read them from this bridge config
    /// directly. Every field that *does* exist on the core type is copied
    /// exactly — unlike a `From` impl, nothing is silently defaulted.
    pub fn to_core_config(&self) -> torvox_core::config::TerminalConfig {
        torvox_core::config::TerminalConfig {
            rows: self.rows,
            cols: self.cols,
            scrollback_lines: self.scrollback_lines,
            shell: self.shell.clone().into(),
            font_size_tenths: self.font_size_tenths,
            backspace_mode: torvox_core::config::BackspaceMode::default(),
            right_alt_mode: torvox_core::config::RightAltMode::default(),
        }
    }

    /// Explicit, non-lossy conversion from the core `TerminalConfig`.
    ///
    /// Bridge-only fields (home, user, path, working_directory, prefix) are
    /// left empty, and `theme` is reset to the default catppuccin-mocha, because
    /// the core type carries neither of those. Every field that exists on the
    /// core type is copied exactly.
    pub fn from_core_config(core_config: &torvox_core::config::TerminalConfig) -> Self {
        Self {
            shell: core_config.shell.clone().into(),
            rows: core_config.rows,
            cols: core_config.cols,
            scrollback_lines: core_config.scrollback_lines,
            font_size_tenths: core_config.font_size_tenths,
            theme: torvox_core::config::Theme::catppuccin_mocha().into(),
            home: String::new(),
            user: String::new(),
            path: String::new(),
            working_directory: String::new(),
            prefix: String::new(),
        }
    }
}

/// Events from the terminal processed by the bridge and polled from Kotlin.
/// Maps to/from `torvox_core::event::TerminalEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub enum TerminalEvent {
    Bell,
    TitleChanged {
        title: String,
    },
    ClipboardRequest {
        text: String,
    },
    HyperlinkHover {
        url: Option<String>,
    },
    ProcessExited {
        exit_code: i32,
    },
    DirtyRegion {
        start_row: u32,
        end_row: u32,
    },
    CursorChanged {
        row: u32,
        col: u32,
    },
    SelectionChanged {
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
        mode: u8,
    },
}

impl From<torvox_core::event::TerminalEvent> for TerminalEvent {
    fn from(event: torvox_core::event::TerminalEvent) -> Self {
        match event {
            torvox_core::event::TerminalEvent::OutputReady => TerminalEvent::DirtyRegion {
                start_row: 0,
                end_row: 0,
            },
            torvox_core::event::TerminalEvent::Bell => TerminalEvent::Bell,
            torvox_core::event::TerminalEvent::TitleChanged(title) => {
                TerminalEvent::TitleChanged { title }
            }
            torvox_core::event::TerminalEvent::ClipboardRequest(text) => {
                TerminalEvent::ClipboardRequest { text }
            }
            torvox_core::event::TerminalEvent::HyperlinkHover(url) => {
                TerminalEvent::HyperlinkHover { url }
            }
            torvox_core::event::TerminalEvent::ProcessExited(exit_code) => {
                TerminalEvent::ProcessExited { exit_code }
            }
            torvox_core::event::TerminalEvent::CursorChanged(cursor) => {
                TerminalEvent::CursorChanged {
                    row: cursor.row,
                    col: cursor.col,
                }
            }
            torvox_core::event::TerminalEvent::SelectionChanged(sel) => match sel {
                Some(selection) => {
                    let (start, end) = selection.ordered();
                    TerminalEvent::SelectionChanged {
                        start_row: start.row,
                        start_col: start.col,
                        end_row: end.row,
                        end_col: end.col,
                        mode: match selection.mode {
                            torvox_core::selection::SelectionMode::Char => 0,
                            torvox_core::selection::SelectionMode::Word => 1,
                            torvox_core::selection::SelectionMode::Line => 2,
                            torvox_core::selection::SelectionMode::Block => 3,
                        },
                    }
                }
                None => TerminalEvent::SelectionChanged {
                    start_row: 0,
                    start_col: 0,
                    end_row: 0,
                    end_col: 0,
                    mode: 0,
                },
            },
            torvox_core::event::TerminalEvent::DirtyRegion(dirty) => TerminalEvent::DirtyRegion {
                start_row: dirty.start_row,
                end_row: dirty.end_row,
            },
        }
    }
}

const LIST_SEPARATOR: &str = "\x1f";
const TEXT_PREVIEW_MAX_CHARS: usize = 80;

/// Errors returned across the FFI boundary.
#[boltffi::error]
#[derive(Debug, Clone, thiserror::Error)]
pub enum TerminalError {
    #[error("PTY error: {detail}")]
    PtyError { detail: String },
    #[error("invalid config: {detail}")]
    InvalidConfig { detail: String },
    #[error("session unavailable: {detail}")]
    SessionUnavailable { detail: String },
}

/// Bridge between Kotlin and the Rust terminal stack.
/// Owns the render surface, session, and configuration.
pub struct TorvoxBridge {
    config: TerminalConfig,
    surface: std::sync::Mutex<Option<crate::surface::AndroidSurface>>,
    session: std::sync::Mutex<
        Option<std::sync::Arc<std::sync::Mutex<torvox_terminal::session::Session>>>,
    >,
    scroll_offset: std::sync::atomic::AtomicU32,
    surface_ready: std::sync::atomic::AtomicBool,
    cell_width: std::sync::atomic::AtomicU32,
    cell_height: std::sync::atomic::AtomicU32,
    scrollback_length: std::sync::atomic::AtomicU32,
    /// Lock-free sender for user-initiated PTY writes (keyboard/IME/paste).
    /// Captured once at session spawn so `write_to_pty`/`process_key_event`
    /// never block on the session mutex — avoiding UI-thread stalls while the
    /// render thread holds that lock during `process_output`.
    user_write_tx: std::sync::Mutex<Option<flume::Sender<Vec<u8>>>>,
}

impl TorvoxBridge {
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
}

/// Reconstruct a `&TorvoxBridge` from an i64 handle, call `f`, and catch panics.
/// SAFETY: `handle` must be a valid pointer to a `TorvoxBridge` created by
/// `torvox_bridge_new`. The caller serializes FFI calls so the bridge lives
/// for the duration of the call.
fn with_bridge<F, T>(handle: i64, f: F) -> Result<T, TerminalError>
where
    F: FnOnce(&TorvoxBridge) -> Result<T, TerminalError>,
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
        Ok(result) => result,
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
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("TorvoxRust"),
        );
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
        }
    }

    pub fn ping(&self) -> Result<String, TerminalError> {
        let ptr = self as *const TorvoxBridge;
        log::info!(
            "ping: self={:p}, aligned={}",
            ptr,
            (ptr as usize).is_multiple_of(8)
        );
        Ok("pong".to_string())
    }

    pub fn spawn_terminal(&self, _rows: u32, _cols: u32) -> Result<i32, TerminalError> {
        let shell: torvox_core::config::Shell = self.config.shell.clone().into();
        let shell_path = match &shell {
            torvox_core::config::Shell::SystemDefault => "/system/bin/sh",
            torvox_core::config::Shell::Custom(path) => path.as_str(),
        };
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        let surface = surface_guard.as_mut().ok_or(TerminalError::InvalidConfig {
            detail: "no surface — call set_native_window first".to_string(),
        })?;
        let env = self.shell_env();
        let session_arc =
            surface
                .spawn_session(shell_path, &env)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })?;
        match self.session.lock() {
            Ok(mut session_guard) => *session_guard = Some(session_arc.clone()),
            Err(poisoned) => {
                let mut session_guard = poisoned.into_inner();
                *session_guard = Some(session_arc.clone());
                log::warn!("spawn_terminal: session mutex was poisoned, recovered");
            }
        }
        // Capture the lock-free user-write sender once so subsequent
        // write_to_pty / process_key_event calls never touch the session mutex.
        if let Ok(session_guard) = session_arc.lock() {
            let sender = session_guard.user_write_sender();
            if let Ok(mut tx_guard) = self.user_write_tx.lock() {
                *tx_guard = Some(sender);
            }
        }
        Ok(0)
    }

    pub fn set_native_window(
        &self,
        window_ptr: i64,
        width: u32,
        height: u32,
    ) -> Result<(), TerminalError> {
        log::debug!(
            "set_native_window: window_ptr={:#x}, width={}, height={}",
            window_ptr,
            width,
            height
        );
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .update_native_window(window_ptr as *mut std::ffi::c_void, width, height)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })?;
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
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })?;
            surface.set_theme(self.config.theme.clone().into());
            *surface_guard = Some(surface);
        }
        // refresh cell metrics now that the font pipeline is set up
        if let Some(surface) = surface_guard.as_ref() {
            self.store_cell_metrics(surface);
        }
        self.surface_ready
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    fn store_cell_metrics(&self, surface: &crate::surface::AndroidSurface) {
        let (cell_width, cell_height) = surface.font_pipeline().cell_metrics();
        // Only log on change to avoid per-frame log spam
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

    /// Returns `Ok(true)` if new data was rendered, `Ok(false)` if idle.
    /// Process ghostty PTY output and take a cell snapshot.
    /// Called BEFORE the surface lock so ghostty C code never blocks the
    /// surface mutex.  Returns `(had_output, snapshot)`.
    fn process_session_for_render(
        &self,
    ) -> Result<(bool, torvox_terminal::ghostty_terminal::GridSnapshot), TerminalError> {
        let session_arc = self
            .session
            .lock()
            .map_err(|e| TerminalError::PtyError {
                detail: format!("session lock poisoned: {e}"),
            })?
            .as_ref()
            .cloned()
            .ok_or(TerminalError::PtyError {
                detail: "no active session for render".to_string(),
            })?;
        let mut session_guard = session_arc.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("session inner lock poisoned: {e}"),
        })?;
        let scroll_offset = self
            .scroll_offset
            .load(std::sync::atomic::Ordering::Relaxed);
        let had_output = session_guard.process_output();
        let snapshot = session_guard
            .terminal()
            .try_take_snapshot_with_scroll(scroll_offset)
            .ok_or(TerminalError::PtyError {
                detail: "snapshot unavailable — VT thread busy".to_string(),
            })?;
        Ok((had_output, snapshot))
    }

    pub fn render(&self) -> Result<bool, TerminalError> {
        // Step 1 – process ghostty output / take snapshot OUTSIDE the surface
        // lock so that ghostty C code (potentially 100+ ms) never stalls the
        // surface mutex.  The main thread can still apply settings / resize /
        // IME input while ghostty is working.
        let session_out = self.process_session_for_render()?;

        // Step 2 – lock surface only for the fast GPU path
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            let scroll_offset = self
                .scroll_offset
                .load(std::sync::atomic::Ordering::Relaxed);
            let result = surface
                .render_frame(scroll_offset, session_out.0, session_out.1)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                });
            self.store_cell_metrics(surface);
            // Cache scrollback length so the main thread can read it lock-free
            // via `scrollback_length()` without contending on the session lock.
            if let Some(session_arc) = self.session.lock().ok().and_then(|g| g.as_ref().cloned())
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

    pub fn save_test_frame(&self, data_dir: &str) -> Result<String, TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .save_test_frame(data_dir)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })
        } else {
            Err(TerminalError::PtyError {
                detail: "No surface".to_string(),
            })
        }
    }

    /// Same as `save_test_frame` but sets the selection first, all within a single
    /// surface lock acquisition. This avoids a race between `bridge.set_selection()`
    /// and `bridge.save_test_frame()` where the render thread can overwrite the
    /// selection in between.
    pub fn save_test_frame_with_selection(
        &self,
        data_dir: &str,
        start_row: i32,
        start_col: i32,
        end_row: i32,
        end_col: i32,
        active: bool,
        mode: u8,
    ) -> Result<String, TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            if active && start_row >= 0 && end_row >= 0 {
                surface.set_selection(Some(torvox_renderer::gpu::SelectionRange {
                    start_row,
                    start_col,
                    end_row,
                    end_col,
                    active: true,
                    mode: match mode {
                        0 => torvox_core::selection::SelectionMode::Char,
                        1 => torvox_core::selection::SelectionMode::Word,
                        2 => torvox_core::selection::SelectionMode::Line,
                        3 => torvox_core::selection::SelectionMode::Block,
                        _ => torvox_core::selection::SelectionMode::Char,
                    },
                    origin: None,
                }));
            } else {
                surface.set_selection(None);
            }
            surface
                .save_test_frame(data_dir)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })
        } else {
            Err(TerminalError::PtyError {
                detail: "No surface".to_string(),
            })
        }
    }

    pub fn poll_bel(&self) -> bool {
        let mut surface_guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in poll_bel");
                poisoned.into_inner()
            }
        };
        surface_guard
            .as_mut()
            .map(|s| s.poll_bel())
            .unwrap_or(false)
    }

    pub fn poll_clipboard(&self) -> Option<String> {
        let mut surface_guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in poll_clipboard");
                poisoned.into_inner()
            }
        };
        surface_guard.as_mut()?.poll_clipboard()
    }

    pub(crate) fn poll_notification_raw(&self) -> Option<(String, String)> {
        let mut surface_guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in poll_notification_raw");
                poisoned.into_inner()
            }
        };
        surface_guard.as_mut()?.poll_notification()
    }

    pub fn poll_shell_integration(&self) -> u8 {
        let mut surface_guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in poll_shell_integration");
                poisoned.into_inner()
            }
        };
        surface_guard
            .as_mut()
            .map(|s| s.poll_shell_integration())
            .unwrap_or(0)
    }

    /// Poll all deferred events (BEL, clipboard, notification, sync mode, shell
    /// integration) in a single surface-lock acquisition. This replaces the 5
    /// separate `poll_*` calls that each acquired the surface mutex on their
    /// own, eliminating ~4 extra JNI round-trips and lock acquisitions per
    /// render frame and reducing surface-lock contention with the main thread.
    pub fn poll_all(&self) -> (bool, Option<String>, Option<(String, String)>, bool, u8) {
        // Keep the C symbol alive for JNA — LTO strips unreferenced extern functions.
        #[cfg(target_os = "android")]
        unsafe {
            std::hint::black_box(torvox_bridge_poll_all(0));
        }
        let mut surface_guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in poll_all");
                poisoned.into_inner()
            }
        };
        surface_guard
            .as_mut()
            .map(|s| s.poll_all())
            .unwrap_or_default()
    }

    pub fn poll_sync_active(&self) -> bool {
        let mut surface_guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in poll_sync_active");
                poisoned.into_inner()
            }
        };
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

    pub fn resize(&self, rows: u32, cols: u32) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
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

    pub fn recompute_grid(&self, width: u32, height: u32) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
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
    ) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            // Re-apply the theme background before updating the native window
            // so the wgpu swapchain clear color matches this session's theme
            // rather than the default deep-blue (catppuccin mocha bg) or
            // the previous session's background.
            surface.set_theme(self.config.theme.clone().into());
            surface
                .update_native_window(window_ptr as *mut std::ffi::c_void, width, height)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })?;
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
        // Wake the render thread and force one present so the scrolled rows
        // become visible. `render()` skips idle frames, so without this the
        // viewport never moves on scroll/fling/jump-to-line.
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

    pub fn set_font_size(&self, size_tenths: u32) -> Result<(), TerminalError> {
        let size = size_tenths as f32 / 10.0;
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
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

    pub fn set_font_size_in_place(&self, size_tenths: u32) -> Result<(), TerminalError> {
        let size = size_tenths as f32 / 10.0;
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
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
    ) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            if active && start_row >= 0 && end_row >= 0 {
                surface.set_selection(Some(torvox_renderer::gpu::SelectionRange {
                    start_row,
                    start_col,
                    end_row,
                    end_col,
                    active: true,
                    mode: match mode {
                        0 => torvox_core::selection::SelectionMode::Char,
                        1 => torvox_core::selection::SelectionMode::Word,
                        2 => torvox_core::selection::SelectionMode::Line,
                        3 => torvox_core::selection::SelectionMode::Block,
                        _ => torvox_core::selection::SelectionMode::Char,
                    },
                    origin: None,
                }));
            } else {
                surface.set_selection(None);
            }
        }
        Ok(())
    }

    /// Get the text of a selected region as a single String. Reads from
    /// scrollback cache lines on the Rust side in one FFI call, instead of
    /// N+1 calls from Kotlin. Returns an empty string if no session.
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

    /// Expand the selection anchor (row, col) according to mode, then set the
    /// expanded bounds on the surface. Uses snapshot data from the session
    /// thread. Single FFI call — no two-way round trip needed.
    pub fn expand_and_set_selection(
        &self,
        row: u32,
        col: u32,
        mode: u8,
    ) -> Result<(u32, u32, u32, u32), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            let mode_enum = match mode {
                0 => torvox_core::selection::SelectionMode::Char,
                1 => torvox_core::selection::SelectionMode::Word,
                2 => torvox_core::selection::SelectionMode::Line,
                3 => torvox_core::selection::SelectionMode::Block,
                _ => torvox_core::selection::SelectionMode::Char,
            };

            // Get snapshot data for grid access
            if let Ok(guard) = self.session.lock()
                && let Some(session_arc) = guard.as_ref()
                && let Ok(session) = session_arc.lock()
            {
                let snapshot = session.terminal().take_snapshot();
                let cols = snapshot.cols;
                let cell_at = |r: u32, c: u32| -> Option<char> {
                    let idx = (r * cols + c) as usize;
                    snapshot
                        .cells
                        .get(idx)
                        .and_then(|s| char::from_u32(s.codepoint))
                };

                let selection = torvox_core::selection::Selection::new(
                    torvox_core::selection::SelectionAnchor { row, col },
                    torvox_core::selection::SelectionAnchor { row, col },
                    mode_enum,
                );
                let expanded = selection.expand(cell_at);
                let (start, end) = expanded.ordered();

                surface.set_selection(Some(torvox_renderer::gpu::SelectionRange {
                    start_row: start.row as i32,
                    start_col: start.col as i32,
                    end_row: end.row as i32,
                    end_col: end.col as i32,
                    active: true,
                    mode: mode_enum,
                    origin: Some((row as i32, col as i32)),
                }));
                return Ok((start.row, start.col, end.row, end.col));
            }
            // No session: clear selection
            surface.set_selection(None);
        }
        Ok((row, col, row, col))
    }

    /// Deserialize search highlights from wire format and set them on the surface.
    ///
    /// Wire format: [count: i32 LE] then for each highlight:
    ///   [row: i32 LE][start_col: i32 LE][end_col_exclusive: i32 LE][r: u8][g: u8][b: u8][a: u8]
    pub fn set_search_highlights(&self, serialized: Vec<u8>) -> Result<(), TerminalError> {
        let data = serialized;
        if data.len() < 4 {
            let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
                detail: format!("lock failed: {}", e),
            })?;
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
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_search_highlights(highlights);
        }
        Ok(())
    }

    pub fn set_font_family(&self, family_name: String) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        let surface = surface_guard.as_mut().ok_or(TerminalError::InvalidConfig {
            detail: "no surface available".to_string(),
        })?;
        if !surface.set_font_family(&family_name) {
            return Err(TerminalError::InvalidConfig {
                detail: format!("font family '{}' not found", family_name),
            });
        }
        Ok(())
    }

    pub fn set_theme(&self, theme: BridgeTheme) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
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
        let guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in list_fonts");
                poisoned.into_inner()
            }
        };
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
        let guard = match self.surface.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("surface mutex poisoned in list_font_families");
                poisoned.into_inner()
            }
        };
        guard
            .as_ref()
            .map(|s| s.font_pipeline().list_all_font_families().join("\x1f"))
            .unwrap_or_default()
    }

    pub fn load_font_file(&self, path: String) -> Option<String> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut surface_guard = match self.surface.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    log::error!("surface mutex poisoned in load_font_file");
                    poisoned.into_inner()
                }
            };
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

    pub fn set_save_path(&self, path: String) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_save_path(path);
        }
        Ok(())
    }

    pub fn save_session(&self, path: String) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .save_session(&path)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })?;
        }
        Ok(())
    }

    pub fn restore_session(&self, path: String) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface
                .restore_session(&path)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })?;
        }
        Ok(())
    }

    pub fn has_saved_session(&self, path: String) -> bool {
        crate::surface::AndroidSurface::has_saved_session(&path)
    }

    pub fn set_mouse_position(&self, row: u32, col: u32) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
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

    pub fn get_cell_width(&self) -> f32 {
        f32::from_bits(self.cell_width.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn get_cell_height(&self) -> f32 {
        f32::from_bits(self.cell_height.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn write_to_pty(&self, data: Vec<u8>) -> Result<(), TerminalError> {
        let guard = self
            .user_write_tx
            .lock()
            .map_err(|_| TerminalError::SessionUnavailable {
                detail: "user-write channel mutex poisoned".to_string(),
            })?;
        match guard.as_ref() {
            Some(sender) => sender.send(data).map_err(|error| {
                log::error!("bridge: user PTY write channel closed: {error}");
                TerminalError::PtyError {
                    detail: format!("user PTY write channel closed: {error}"),
                }
            }),
            None => Err(TerminalError::SessionUnavailable {
                detail: "no active session — user-write channel not initialized".to_string(),
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
    ) -> Result<(), TerminalError> {
        // Encode the key under the session lock (terminal state is mutated),
        // then enqueue the bytes on the lock-free channel so the actual PTY
        // write happens on the render thread. This keeps the UI thread off the
        // session mutex's write path and avoids stalls while the render thread
        // holds that lock during process_output.
        let encoded = {
            let session_arc = {
                let guard = self
                    .session
                    .lock()
                    .map_err(|_| TerminalError::SessionUnavailable {
                        detail: "session mutex poisoned".to_string(),
                    })?;
                match guard.as_ref() {
                    Some(session_arc) => session_arc.clone(),
                    None => {
                        return Err(TerminalError::SessionUnavailable {
                            detail: "no active session".to_string(),
                        });
                    }
                }
            };
            let session = session_arc
                .lock()
                .map_err(|_| TerminalError::SessionUnavailable {
                    detail: "session inner mutex poisoned".to_string(),
                })?;
            session.key_encode(
                key_code,
                modifiers as u16,
                action,
                unicode_char,
                unshifted_char,
            )
        };
        if let Some(encoded) = encoded
            && !encoded.is_empty()
        {
            log::trace!(
                "bridge: process_key_event key_code={key_code} modifiers={modifiers} action={action} unicode={unicode_char} encoded_len={}",
                encoded.len()
            );
            let guard =
                self.user_write_tx
                    .lock()
                    .map_err(|_| TerminalError::SessionUnavailable {
                        detail: "user-write channel mutex poisoned".to_string(),
                    })?;
            match guard.as_ref() {
                Some(sender) => sender.send(encoded).map_err(|error| {
                    log::error!("bridge: key PTY write channel closed: {error}");
                    TerminalError::PtyError {
                        detail: format!("key PTY write channel closed: {error}"),
                    }
                }),
                None => Err(TerminalError::SessionUnavailable {
                    detail: "no active session — user-write channel not initialized".to_string(),
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
    ) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_background_image(&rgba_data, width, height);
        }
        Ok(())
    }

    pub fn clear_background_image(&self) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.clear_background_image();
        }
        Ok(())
    }

    pub fn set_background_params(
        &self,
        blur_radius: i32,
        alpha_tenths: i32,
    ) -> Result<(), TerminalError> {
        let blur = blur_radius as f32;
        let alpha = alpha_tenths as f32 / 10.0;
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_background_params(blur, alpha);
        }
        Ok(())
    }

    pub fn set_cursor_blink_enabled(&self, enabled: bool) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_blink_enabled(enabled);
        }
        Ok(())
    }

    pub fn set_cursor_blink_speed_ms(&self, speed_ms: u32) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_blink_speed_ms(speed_ms);
        }
        Ok(())
    }

    pub fn reset_cursor_blink(&self) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.reset_blink();
        }
        Ok(())
    }

    pub fn set_cursor_style(&self, style: String) -> Result<(), TerminalError> {
        let cursor_style = match style.as_str() {
            "bar" => torvox_core::cursor::CursorStyle::Bar,
            "underline" => torvox_core::cursor::CursorStyle::Underline,
            _ => torvox_core::cursor::CursorStyle::Block,
        };
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.set_cursor_style(cursor_style);
        }
        Ok(())
    }
}

/// # Safety
/// `handle` must be a valid pointer to a TorvoxBridge previously
/// returned by `torvox_bridge_new`, or zero.
unsafe fn bridge_from_handle(handle: i64) -> Option<&'static TorvoxBridge> {
    let ptr = handle as *const TorvoxBridge;
    if ptr.is_null() {
        None
    } else {
        // SAFETY: The pointer is non-null and must be valid per the
        // Safety doc on this function. Callers guarantee the handle
        // came from torvox_bridge_new and the bridge is still alive.
        Some(unsafe { &*ptr })
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// The ANativeWindow pointer reconstructed from `window_ptr_low` and `window_ptr_high` must be a valid, non-null pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_native_window(
    handle: i64,
    window_ptr_low: u32,
    window_ptr_high: u32,
    width: u32,
    height: u32,
) -> i32 {
    log::debug!(
        "set_native_window_ffi: handle={handle}, low={window_ptr_low:#x}, high={window_ptr_high:#x}, width={width}, height={height}"
    );
    let window_ptr = ((window_ptr_high as i64) << 32) | (window_ptr_low as i64);
    log::debug!(
        "set_native_window_ffi: reconstructed window_ptr={:#x}",
        window_ptr
    );
    with_bridge(handle, |bridge| {
        bridge.set_native_window(window_ptr, width, height)
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_resize(handle: i64, rows: u32, cols: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.resize(rows, cols))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_update_native_window(
    handle: i64,
    window_ptr_low: u32,
    window_ptr_high: u32,
    width: u32,
    height: u32,
) -> i32 {
    let window_ptr = ((window_ptr_high as i64) << 32) | (window_ptr_low as i64);
    with_bridge(handle, |bridge| {
        bridge.update_native_window(window_ptr, width, height)
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_recompute_grid(handle: i64, width: u32, height: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.recompute_grid(width, height))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid bridge handle. Immediately updates the renderer's
/// viewport dimensions without triggering a grid resize — prevents texture
/// stretch/squash during IME show/hide animation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_surface_size(handle: i64, width: u32, height: u32) {
    with_bridge(handle, |bridge| {
        bridge.set_surface_size(width, height);
        Ok::<_, TerminalError>(())
    })
    .ok();
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_spawn_terminal(handle: i64, rows: u32, cols: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.spawn_terminal(rows, cols))
        .map(|_| 0)
        .unwrap_or(-1)
}

// SAFETY: callers must ensure `ptr` is valid for reads of `len` bytes.
fn read_string(ptr: *const u8, len: i32) -> String {
    if ptr.is_null() || len <= 0 {
        String::new()
    } else {
        // SAFETY: The caller guarantees ptr is valid for reads of len bytes
        // and is properly aligned for u8 access. The returned slice is
        // immediately converted to an owned String, so no aliasing issues.
        let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        String::from_utf8_lossy(slice).to_string()
    }
}

/// Create a CString from a String, stripping any interior NUL bytes.
/// Returns None if the resulting string is empty.
/// Ownership: caller receives a CString; use `.into_raw()` to pass to
/// Kotlin (Rust leaks ownership), then Kotlin must free via
/// `torvox_bridge_free_string` (which calls `CString::from_raw`).
fn safe_cstring(string: String) -> Option<std::ffi::CString> {
    if string.contains('\0') {
        log::warn!(
            "bridge: safe_cstring encountered interior NUL(s) — data truncated. Original length: {}",
            string.len()
        );
    }
    let stripped: String = string
        .chars()
        .filter(|&character| character != '\0')
        .collect();
    if stripped.is_empty() {
        None
    } else {
        std::ffi::CString::new(stripped).ok()
    }
}

/// Read a u32 from bytes at a given position with bounds checking.
fn read_u32_le(bytes: &[u8], pos: usize) -> Option<u32> {
    if pos + 4 > bytes.len() {
        log::error!(
            "wire deserialization: buffer too short at pos={pos}, len={}",
            bytes.len()
        );
        return None;
    }
    Some(u32::from_le_bytes(bytes[pos..pos + 4].try_into().ok()?))
}

/// Read a length-prefixed string from bytes at a given position with bounds checking.
fn read_wire_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
    let len = read_u32_le(bytes, *pos)? as usize;
    *pos += 4;
    if *pos + len > bytes.len() {
        log::error!(
            "wire deserialization: string length {len} exceeds buffer at pos={}",
            *pos
        );
        return None;
    }
    let string_value = String::from_utf8_lossy(&bytes[*pos..*pos + len]).to_string();
    *pos += len;
    Some(string_value)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_release_gpu_surface(handle: i64) {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return,
    };
    if let Err(panic_info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.release_gpu_surface();
    })) {
        let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic in FFI call".to_string()
        };
        log::error!(
            "FFI panic in torvox_bridge_release_gpu_surface: {}",
            message
        );
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_release_surface(handle: i64) {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return,
    };
    if let Err(panic_info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.release_surface();
    })) {
        let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic in FFI call".to_string()
        };
        log::error!("FFI panic in torvox_bridge_release_surface: {}", message);
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_save_path(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> i32 {
    let path = read_string(path_ptr, path_len);
    with_bridge(handle, |bridge| bridge.set_save_path(path))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_has_saved_session(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> bool {
    let path = read_string(path_ptr, path_len);
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return false,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.has_saved_session(path)
    })) {
        Ok(result) => result,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_has_saved_session: {}", message);
            false
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_save_session(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> i32 {
    let path = read_string(path_ptr, path_len);
    with_bridge(handle, |bridge| bridge.save_session(path))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `path_ptr` must be valid for reads of `path_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_restore_session(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> i32 {
    let path = read_string(path_ptr, path_len);
    with_bridge(handle, |bridge| bridge.restore_session(path))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `data_ptr` must be valid for reads of `data_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_write_to_pty(
    handle: i64,
    data_ptr: *const u8,
    data_len: i32,
) -> i32 {
    let data = if data_ptr.is_null() || data_len <= 0 {
        Vec::new()
    } else {
        // SAFETY: The caller guarantees data_ptr is valid for reads of data_len bytes.
        // The slice is immediately copied to an owned Vec, so no aliasing issues.
        unsafe { std::slice::from_raw_parts(data_ptr, data_len as usize) }.to_vec()
    };
    with_bridge(handle, |bridge| bridge.write_to_pty(data))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// Process a key event through the ghostty key encoder and write the resulting
/// escape sequence directly to the PTY.
///
/// # Parameters
/// - `handle`: bridge handle from `torvox_bridge_new`
/// - `key_code`: Android `KeyEvent` key code (e.g. `KeyEvent.KEYCODE_A` = 29)
/// - `modifiers`: bitmask — bit 0 = SHIFT, bit 1 = ALT, bit 2 = CTRL, bit 3 = META
/// - `action`: 0 = press, 1 = release, 2 = repeat
/// - `unicode_char`: the Unicode codepoint from `KeyEvent.getUnicodeChar()`, or 0
///
/// # Returns
/// 0 on success, -1 on error.
/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_process_key_event(
    handle: i64,
    key_code: u32,
    modifiers: u8,
    action: u8,
    unicode_char: u32,
    unshifted_char: u32,
) -> i32 {
    with_bridge(handle, |bridge| {
        bridge.process_key_event(key_code, modifiers, action, unicode_char, unshifted_char)
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_terminal_text(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let text =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.get_terminal_text()))
        {
            Ok(t) => t,
            Err(panic_info) => {
                let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "panic in FFI call".to_string()
                };
                log::error!("FFI panic in torvox_bridge_get_terminal_text: {}", message);
                return 0;
            }
        };
    match safe_cstring(text) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_active_session_title(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let title = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.get_active_session_title()
    })) {
        Ok(t) => t,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!(
                "FFI panic in torvox_bridge_get_active_session_title: {}",
                message
            );
            return 0;
        }
    };
    match safe_cstring(title) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_default_font_name(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let name = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.get_default_font_name()
    })) {
        Ok(n) => n,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!(
                "FFI panic in torvox_bridge_get_default_font_name: {}",
                message
            );
            return 0;
        }
    };
    match safe_cstring(name) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// Returns detailed font info string: active font name, type (vector/bitmap),
/// CJK fallback name, cell metrics, font size.
/// Caller must free with `torvox_bridge_free_string`.
/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_font_info(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let info =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.get_font_info())) {
            Ok(i) => i,
            Err(panic_info) => {
                let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "panic in FFI call".to_string()
                };
                log::error!("FFI panic in torvox_bridge_get_font_info: {}", message);
                return 0;
            }
        };
    match safe_cstring(info) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle, or zero.
/// `locale_ptr` must be a valid null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_system_locale(
    handle: i64,
    locale_ptr: *const std::os::raw::c_char,
) {
    if locale_ptr.is_null() {
        return;
    }
    // SAFETY: The caller guarantees locale_ptr is a valid null-terminated C string.
    let locale = match unsafe { std::ffi::CStr::from_ptr(locale_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_system_locale(locale);
        Ok(())
    }) {
        log::error!("bridge: torvox_bridge_set_system_locale failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// Returns a C string with font family names separated by \x1f (unit separator).
/// Caller must free with `torvox_bridge_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_list_font_families(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let families = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.list_font_families()
    })) {
        Ok(f) => f,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_list_font_families: {}", message);
            return 0;
        }
    };
    match safe_cstring(families) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_grid_rows(handle: i64) -> u32 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 24,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.get_grid_rows())) {
        Ok(rows) => rows,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_get_grid_rows: {}", message);
            24
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_grid_cols(handle: i64) -> u32 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 80,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.get_grid_cols())) {
        Ok(cols) => cols,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_get_grid_cols: {}", message);
            80
        }
    }
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_cell_width(handle: i64) -> f32 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0.0,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.get_cell_width())) {
        Ok(w) => w,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_get_cell_width: {}", message);
            0.0
        }
    }
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_cell_height(handle: i64) -> f32 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0.0,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.get_cell_height())) {
        Ok(h) => h,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_get_cell_height: {}", message);
            0.0
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_font_size(handle: i64, size_tenths: u32) -> i32 {
    with_bridge(handle, |bridge| bridge.set_font_size(size_tenths))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_font_size_in_place(
    handle: i64,
    size_tenths: u32,
) -> i32 {
    with_bridge(handle, |bridge| bridge.set_font_size_in_place(size_tenths))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// Each path_ptr/path_len pair must be valid for reads of path_len bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_extra_font_paths(
    handle: i64,
    paths_ptr: *const *const u8,
    lens_ptr: *const i32,
    count: i32,
) -> i32 {
    if paths_ptr.is_null() || lens_ptr.is_null() || count <= 0 {
        return -1;
    }
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        // SAFETY: Both pointers are checked non-null above, count is verified
        // positive, and the caller guarantees the arrays are valid for count
        // elements. Each element pointer is checked by read_string.
        let path_ptr = unsafe { *paths_ptr.add(i) };
        let path_len = unsafe { *lens_ptr.add(i) };
        paths.push(read_string(path_ptr, path_len));
    }
    with_bridge(handle, |bridge| {
        bridge.set_extra_font_paths(paths);
        Ok(())
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_selection(
    handle: i64,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
    active: i32,
    mode: i32,
) -> i32 {
    with_bridge(handle, |bridge| {
        bridge.set_selection(
            start_row,
            start_col,
            end_row,
            end_col,
            active != 0,
            mode as u8,
        )
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_expand_and_set_selection(
    handle: i64,
    row: u32,
    col: u32,
    mode: i32,
) -> i64 {
    with_bridge(handle, |bridge| {
        bridge.expand_and_set_selection(row, col, mode as u8)
    })
    .map(|(sr, sc, er, ec)| {
        (sr as i64 & 0xFFFF)
            | ((sc as i64 & 0xFFFF) << 16)
            | ((er as i64 & 0xFFFF) << 32)
            | ((ec as i64 & 0xFFFF) << 48)
    })
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `data_ptr` must be valid for reads of `data_len` bytes, and must not be aliased.
/// Wire format: [count: i32 LE] then for each:
///   [row: i32 LE][start_col: i32 LE][end_col_exclusive: i32 LE][r: u8][g: u8][b: u8][a: u8]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_search_highlights(
    handle: i64,
    data_ptr: *const u8,
    data_len: i32,
) -> i32 {
    if data_ptr.is_null() || data_len <= 0 {
        return with_bridge(handle, |bridge| bridge.set_search_highlights(Vec::new()))
            .map(|_| 0)
            .unwrap_or(-1);
    }
    // SAFETY: The caller guarantees data_ptr is valid for reads of data_len bytes.
    // The slice is immediately copied to an owned Vec, so no aliasing issues.
    let bytes = unsafe { std::slice::from_raw_parts(data_ptr, data_len as usize) };
    with_bridge(handle, |bridge| {
        bridge.set_search_highlights(bytes.to_vec())
    })
    .map(|_| 0)
    .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `family_ptr` must be valid for reads of `family_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_font_family(
    handle: i64,
    family_ptr: *const u8,
    family_len: i32,
) -> i32 {
    let family = read_string(family_ptr, family_len);
    with_bridge(handle, |bridge| bridge.set_font_family(family))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
/// `theme_ptr` must be valid for reads of `theme_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_theme(
    handle: i64,
    theme_ptr: *const u8,
    theme_len: i32,
) -> i32 {
    if theme_ptr.is_null() || theme_len <= 0 {
        return -1;
    }
    // SAFETY: The caller guarantees theme_ptr is valid for reads of theme_len bytes.
    // The slice is only used within this call for wire format deserialization.
    let bytes = unsafe { std::slice::from_raw_parts(theme_ptr, theme_len as usize) };
    // Deserialize BridgeTheme from boltffi wire format:
    // 1 string (name) + 20 u32s (bg, fg, cursor, selection_bg, ansi0..ansi15)
    let mut pos = 0usize;
    let name = match read_wire_string(bytes, &mut pos) {
        Some(n) => n,
        None => {
            log::error!(
                "torvox_bridge_set_theme: truncated theme buffer ({} bytes) — could not read name",
                bytes.len()
            );
            return -1;
        }
    };
    let read_color = |bytes: &[u8], pos: &mut usize| -> u32 {
        let color_value = read_u32_le(bytes, *pos).unwrap_or_else(|| {
            log::error!("torvox_bridge_set_theme: truncated theme buffer at pos={pos}");
            0
        });
        *pos += 4;
        color_value
    };
    let theme = BridgeTheme {
        name,
        bg: read_color(bytes, &mut pos),
        fg: read_color(bytes, &mut pos),
        cursor: read_color(bytes, &mut pos),
        selection_bg: read_color(bytes, &mut pos),
        ansi0: read_color(bytes, &mut pos),
        ansi1: read_color(bytes, &mut pos),
        ansi2: read_color(bytes, &mut pos),
        ansi3: read_color(bytes, &mut pos),
        ansi4: read_color(bytes, &mut pos),
        ansi5: read_color(bytes, &mut pos),
        ansi6: read_color(bytes, &mut pos),
        ansi7: read_color(bytes, &mut pos),
        ansi8: read_color(bytes, &mut pos),
        ansi9: read_color(bytes, &mut pos),
        ansi10: read_color(bytes, &mut pos),
        ansi11: read_color(bytes, &mut pos),
        ansi12: read_color(bytes, &mut pos),
        ansi13: read_color(bytes, &mut pos),
        ansi14: read_color(bytes, &mut pos),
        ansi15: read_color(bytes, &mut pos),
    };
    with_bridge(handle, |bridge| bridge.set_theme(theme))
        .map(|_| 0)
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_scrollback_line(handle: i64, index: u32) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let line = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.scrollback_line(index)
    })) {
        Ok(l) => l,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_scrollback_line: {}", message);
            return 0;
        }
    };
    match line {
        Some(s) => match safe_cstring(s) {
            Some(c_str) => c_str.into_raw() as i64,
            None => 0,
        },
        None => 0,
    }
}

/// # Safety
/// `s` must be a valid C string pointer previously returned by
/// `torvox_bridge_scrollback_line` or `torvox_bridge_search_in_scrollback`, or zero.
/// Ownership: takes back ownership of the CString allocated by `safe_cstring`
/// via `into_raw()`, then drops it immediately.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_string(s: i64) {
    if s != 0 {
        // SAFETY: The caller guarantees s is a pointer previously returned
        // by safe_cstring(...).into_raw(), i.e. a valid CString that was
        // leaked into raw pointer ownership. This call takes back ownership
        // and drops it immediately. s is validated non-null above.
        std::mem::drop(unsafe { std::ffi::CString::from_raw(s as *mut std::ffi::c_char) });
    }
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_ping(handle: i64) -> i32 {
    log::debug!("torvox_bridge_ping: handle={handle:#x}");
    match with_bridge(handle, |bridge| bridge.ping()) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("torvox_bridge_ping: error: {e}");
            -1
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_render(handle: i64) -> i32 {
    with_bridge(handle, |bridge| bridge.render())
        .map(|had_output| if had_output { 1 } else { 0 })
        .unwrap_or(-1)
}

/// # Safety
/// `handle` must be a valid surface handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_bel(handle: i64) -> i32 {
    let ptr = handle as *mut TorvoxBridge;
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: ptr is non-null (checked above) and the caller guarantees
    // the handle came from torvox_bridge_new. The bridge is still alive
    // for the duration of this call because the caller serializes FFI
    // calls and torvox_bridge_free is only called after all concurrent
    // calls complete.
    let bridge = unsafe { &*ptr };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.poll_bel())) {
        Ok(bel) => {
            if bel {
                1
            } else {
                0
            }
        }
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_poll_bel: {}", message);
            0
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_shell_integration(handle: i64) -> i32 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.poll_shell_integration()
    })) {
        Ok(val) => val as i32,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!(
                "FFI panic in torvox_bridge_poll_shell_integration: {}",
                message
            );
            0
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_sync_active(handle: i64) -> i32 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.poll_sync_active())) {
        Ok(active) => {
            if active {
                1
            } else {
                0
            }
        }
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_poll_sync_active: {}", message);
            0
        }
    }
}

/// Save a GPU test frame to disk.
/// `data_dir` must be a valid C string pointing to a writable directory.
/// Returns 0 on success, -1 on error.
/// # Safety
/// `handle` must be a valid surface handle. `data_dir` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_save_test_frame(
    handle: i64,
    data_dir: *const std::ffi::c_char,
) -> i32 {
    if data_dir.is_null() {
        return -1;
    }
    let dir = unsafe { std::ffi::CStr::from_ptr(data_dir) }
        .to_string_lossy()
        .into_owned();
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return -1,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.save_test_frame(&dir)
    })) {
        Ok(result) => match result {
            Ok(_) => 0,
            Err(e) => {
                log::error!("save_test_frame failed: {e}");
                -1
            }
        },
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_save_test_frame: {}", message);
            -1
        }
    }
}

/// # Safety
/// Same as `torvox_bridge_save_test_frame` but sets selection first, all within
/// one surface lock acquisition. Pass -1 for any row/col to clear selection.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_save_test_frame_with_selection(
    handle: i64,
    data_dir: *const std::ffi::c_char,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
    active: i32,
    mode: i32,
) -> i32 {
    if data_dir.is_null() {
        return -1;
    }
    let dir = unsafe { std::ffi::CStr::from_ptr(data_dir) }
        .to_str()
        .unwrap_or_default()
        .to_string();
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return -1,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.save_test_frame_with_selection(
            &dir,
            start_row,
            start_col,
            end_row,
            end_col,
            active != 0,
            mode as u8,
        )
    })) {
        Ok(result) => match result {
            Ok(_) => 0,
            Err(e) => {
                log::error!("save_test_frame_with_selection failed: {e}");
                -1
            }
        },
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!(
                "FFI panic in torvox_bridge_save_test_frame_with_selection: {}",
                message
            );
            -1
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// Returns a C string pointer that must be freed with `torvox_bridge_free_string`, or 0 if no clipboard text.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_clipboard(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let clipboard =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.poll_clipboard())) {
            Ok(c) => c,
            Err(panic_info) => {
                let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "panic in FFI call".to_string()
                };
                log::error!("FFI panic in torvox_bridge_poll_clipboard: {}", message);
                return 0;
            }
        };
    match clipboard {
        Some(text) => match safe_cstring(text) {
            Some(c_str) => c_str.into_raw() as i64,
            None => 0,
        },
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// Returns a pointer to `[title_ptr, body_ptr]` (two consecutive C string pointers) that must be
/// freed with `torvox_bridge_free_notification`, or 0 if no notification.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_notification(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let notification = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.poll_notification_raw()
    })) {
        Ok(n) => n,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_poll_notification: {}", message);
            return 0;
        }
    };
    match notification {
        Some((title, body)) => {
            let title_c = match safe_cstring(title) {
                Some(c) => c,
                None => return 0,
            };
            let body_c = match safe_cstring(body) {
                Some(c) => c,
                None => {
                    // Body is empty but title is valid; use empty body
                    std::ffi::CString::new("").expect("empty string has no null bytes")
                }
            };
            // Allocate a buffer holding both pointers: [title_ptr, body_ptr]
            let title_ptr = title_c.into_raw();
            let body_ptr = body_c.into_raw();
            let buf = Box::new([title_ptr, body_ptr]);
            Box::into_raw(buf) as i64
        }
        None => 0,
    }
}

/// # Safety
/// `ptr` must be a valid pointer previously returned by `torvox_bridge_poll_notification`.
/// Frees the two C strings and the pointer buffer allocated by `torvox_bridge_poll_notification`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_notification(ptr: i64) {
    if ptr != 0 {
        // SAFETY: The caller guarantees ptr was returned by
        // torvox_bridge_poll_notification, which allocates a Box<[*mut c_char; 2]>
        // and two CStrings via into_raw(). This reconstruction is the inverse:
        // Box::from_raw reclaims the Box, and CString::from_raw reclaims each CString
        // so they can be dropped. The pointer is non-null (checked above) and
        // is used exactly once.
        unsafe {
            let buf = Box::from_raw(ptr as *mut [*const std::ffi::c_char; 2]);
            drop(std::ffi::CString::from_raw(buf[0].cast_mut()));
            drop(std::ffi::CString::from_raw(buf[1].cast_mut()));
        }
    }
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`.
/// Returns a heap-allocated `PollAllFFI` pointer (free with `torvox_bridge_free_poll_all`).
/// Aggregates every deferred event the render thread drains each frame into a single
/// surface-lock acquisition, eliminating the per-poll lock churn of `poll_bel` /
/// `poll_clipboard` / `poll_notification` / `poll_sync_active` / `poll_shell_integration`.
#[repr(C)]
pub struct PollAllFFI {
    pub bel: u8,
    pub sync_active: u8,
    pub shell_integration: u8,
    pub clipboard_ptr: i64,
    pub notification_ptr: i64,
}

/// # Safety
/// `handle` must be a valid bridge handle previously returned by `torvox_bridge_new`.
/// Returns a heap-allocated `PollAllFFI` pointer (free with `torvox_bridge_free_poll_all`)
/// that batches the per-frame event polls into a single surface-lock acquisition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_poll_all(handle: i64) -> i64 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.poll_all()))
    {
        Ok(r) => r,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_poll_all: {}", message);
            return 0;
        }
    };
    let clipboard_ptr = match result.1 {
        Some(s) => match safe_cstring(s) {
            Some(c) => c.into_raw() as i64,
            None => 0,
        },
        None => 0,
    };
    let notification_ptr = match result.2 {
        Some((title, body)) => {
            let title_c = match safe_cstring(title) {
                Some(c) => c,
                None => return 0,
            };
            let body_c = match safe_cstring(body) {
                Some(c) => c,
                None => {
                    // SAFETY: title_c was created from safe_cstring above and is valid here.
                    unsafe {
                        std::mem::drop(std::ffi::CString::from_raw(title_c.into_raw()));
                    }
                    return 0;
                }
            };
            let buf = Box::new([title_c.into_raw(), body_c.into_raw()]);
            Box::into_raw(buf) as i64
        }
        None => 0,
    };
    let ffi = PollAllFFI {
        bel: if result.0 { 1 } else { 0 },
        sync_active: if result.3 { 1 } else { 0 },
        shell_integration: result.4,
        clipboard_ptr,
        notification_ptr,
    };
    Box::into_raw(Box::new(ffi)) as i64
}

/// # Safety
/// `ptr` must be a valid pointer previously returned by `torvox_bridge_poll_all`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_poll_all(ptr: i64) {
    if ptr != 0 {
        // SAFETY: ptr was returned by torvox_bridge_poll_all, which allocates a
        // Box<PollAllFFI> plus (optionally) a clipboard CString and a notification
        // pointer buffer. This reconstruction is the inverse.
        unsafe {
            let ffi = Box::from_raw(ptr as *mut PollAllFFI);
            if ffi.clipboard_ptr != 0 {
                let _ = std::ffi::CString::from_raw(ffi.clipboard_ptr as *mut std::ffi::c_char);
            }
            if ffi.notification_ptr != 0 {
                let buf = Box::from_raw(ffi.notification_ptr as *mut [*const std::ffi::c_char; 2]);
                drop(std::ffi::CString::from_raw(buf[0].cast_mut()));
                drop(std::ffi::CString::from_raw(buf[1].cast_mut()));
            }
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_cwd(handle: i64) -> *mut std::ffi::c_char {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return std::ptr::null_mut(),
    };
    let cwd = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.cwd())) {
        Ok(c) => c,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_cwd: {}", message);
            return std::ptr::null_mut();
        }
    };
    let cwd = if cwd.is_empty() { "unknown" } else { &cwd };
    match safe_cstring(cwd.to_string()) {
        Some(c_cwd) => c_cwd.into_raw(),
        None => std::ffi::CString::new("unknown")
            .expect("literal string has no null bytes")
            .into_raw(),
    }
}

/// # Safety
/// `s` must be a valid surface handle pointer previously returned by `torvox_bridge_cwd`.
/// Ownership: takes back ownership of the CString allocated via `into_raw()`,
/// then drops it immediately. Must only be called once per pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_free_cstring(s: *mut std::ffi::c_char) {
    if !s.is_null() {
        // SAFETY: The caller guarantees s was returned by torvox_bridge_cwd,
        // which allocates via safe_cstring(...).into_raw(). This is the
        // inverse: CString::from_raw reclaims ownership so it can be dropped.
        unsafe { drop(std::ffi::CString::from_raw(s)) };
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_focus_event(handle: i64, focused: i32) {
    let ptr = handle as *mut TorvoxBridge;
    if ptr.is_null() {
        return;
    }
    // SAFETY: ptr is non-null (checked above) and the caller guarantees
    // the handle came from torvox_bridge_new. The bridge is still alive
    // for the duration of this call because the caller serializes FFI
    // calls and torvox_bridge_free is only called after all concurrent
    // calls complete.
    let bridge = unsafe { &*ptr };
    if let Err(panic_info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.focus_event(focused != 0);
    })) {
        let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic in FFI call".to_string()
        };
        log::error!("FFI panic in torvox_bridge_focus_event: {}", message);
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_scrollback_len(handle: i64) -> u32 {
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.scrollback_length())) {
        Ok(len) => len,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!("FFI panic in torvox_bridge_scrollback_len: {}", message);
            0
        }
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// `query_ptr` must be valid for reads of `query_len` bytes, and must not be aliased.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_search_in_scrollback(
    handle: i64,
    query_ptr: *const u8,
    query_len: i32,
) -> i64 {
    let query = read_string(query_ptr, query_len);
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.search_in_scrollback(query)
    })) {
        Ok(r) => r,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!(
                "FFI panic in torvox_bridge_search_in_scrollback: {}",
                message
            );
            return 0;
        }
    };
    match result {
        Some(s) => match safe_cstring(s) {
            Some(c_str) => c_str.into_raw() as i64,
            None => 0,
        },
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`, or zero.
/// `query_ptr` must be valid for reads of `query_len` bytes, and must not be aliased.
/// Returns a C string (semicolon-separated "row,col,end" triples) that must be freed
/// with `torvox_bridge_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_search_all_in_scrollback(
    handle: i64,
    query_ptr: *const u8,
    query_len: i32,
    case_sensitive: u8,
    fuzzy: u8,
) -> i64 {
    let query = read_string(query_ptr, query_len);
    let case_sensitive = case_sensitive != 0;
    let fuzzy = fuzzy != 0;
    let bridge = match unsafe { bridge_from_handle(handle) } {
        Some(b) => b,
        None => return 0,
    };
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.search_all_in_scrollback(query, case_sensitive, fuzzy)
    })) {
        Ok(r) => r,
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic in FFI call".to_string()
            };
            log::error!(
                "FFI panic in torvox_bridge_search_all_in_scrollback: {}",
                message
            );
            return 0;
        }
    };
    if result.is_empty() {
        // Return null when no matches found (Kotlin interprets as null/empty)
        return 0;
    }
    match safe_cstring(result) {
        Some(c_str) => c_str.into_raw() as i64,
        None => 0,
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_scroll_offset(handle: i64, offset: i32) {
    let ptr = handle as *mut TorvoxBridge;
    if ptr.is_null() {
        return;
    }
    // SAFETY: ptr is non-null (checked above) and the caller guarantees
    // the handle came from torvox_bridge_new. The bridge is still alive
    // for the duration of this call because the caller serializes FFI
    // calls and torvox_bridge_free is only called after all concurrent
    // calls complete.
    let bridge = unsafe { &*ptr };
    if let Err(panic_info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.set_scroll_offset(offset as u32);
    })) {
        let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic in FFI call".to_string()
        };
        log::error!("FFI panic in torvox_bridge_set_scroll_offset: {}", message);
    }
}

/// # Safety
/// `handle` must be a valid surface handle previously returned by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_wait_until_ready_for_render(handle: i64) {
    let ptr = handle as *mut TorvoxBridge;
    if ptr.is_null() {
        return;
    }
    // SAFETY: ptr is non-null (checked above) and the caller guarantees
    // the handle came from torvox_bridge_new. The bridge is still alive
    // for the duration of this call because the caller serializes FFI
    // calls and torvox_bridge_free is only called after all concurrent
    // calls complete.
    let bridge = unsafe { &*ptr };
    if let Err(panic_info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.wait_until_ready_for_render();
    })) {
        let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic in FFI call".to_string()
        };
        log::error!(
            "FFI panic in torvox_bridge_wait_until_ready_for_render: {}",
            message
        );
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
/// `data` must point to valid RGBA pixel data of at least `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_background_image(
    handle: i64,
    data: *const u8,
    len: i32,
    width: i32,
    height: i32,
) {
    if data.is_null() || len <= 0 || width <= 0 || height <= 0 {
        log::warn!(
            "set_background_image: invalid args data={data:?} len={len} w={width} h={height}"
        );
        return;
    }
    // SAFETY: The caller guarantees data is valid for reads of len bytes.
    // The slice is immediately copied to an owned Vec, so no aliasing issues.
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_background_image(bytes.to_vec(), width as u32, height as u32)
    }) {
        log::error!("bridge: torvox_bridge_set_background_image failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_background_params(
    handle: i64,
    blur_radius: i32,
    alpha_tenths: i32,
) {
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_background_params(blur_radius, alpha_tenths)
    }) {
        log::error!("bridge: torvox_bridge_set_background_params failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_clear_background_image(handle: i64) {
    if let Err(error) = with_bridge(handle, |bridge| bridge.clear_background_image()) {
        log::error!("bridge: torvox_bridge_clear_background_image failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_cursor_blink_enabled(handle: i64, enabled: i32) {
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_cursor_blink_enabled(enabled != 0)
    }) {
        log::error!("bridge: torvox_bridge_set_cursor_blink_enabled failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_cursor_blink_speed_ms(handle: i64, speed_ms: i32) {
    if let Err(error) = with_bridge(handle, |bridge| {
        bridge.set_cursor_blink_speed_ms(speed_ms as u32)
    }) {
        log::error!("bridge: torvox_bridge_set_cursor_blink_speed_ms failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_reset_cursor_blink(handle: i64) {
    if let Err(error) = with_bridge(handle, |bridge| bridge.reset_cursor_blink()) {
        log::error!("bridge: torvox_bridge_reset_cursor_blink failed: {error}");
    }
}

/// # Safety
/// `handle` must be a valid pointer to a `TorvoxBridge` created by `torvox_bridge_new`.
/// `style_ptr` must point to a valid UTF-8 byte array of length `style_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_set_cursor_style(
    handle: i64,
    style_ptr: *const u8,
    style_len: i32,
) {
    let style = read_string(style_ptr, style_len);
    if let Err(error) = with_bridge(handle, |bridge| bridge.set_cursor_style(style)) {
        log::error!("bridge: torvox_bridge_set_cursor_style failed: {error}");
    }
}

/// Convert [f32; 4] (RGBA 0..1) to u32 ARGB.
#[inline]
fn to_argb(color: &[f32; 4]) -> u32 {
    let r = (color[0].clamp(0.0, 1.0) * 255.0) as u32;
    let g = (color[1].clamp(0.0, 1.0) * 255.0) as u32;
    let b = (color[2].clamp(0.0, 1.0) * 255.0) as u32;
    let a = (color[3].clamp(0.0, 1.0) * 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_get_snapshot(
    handle: i64,
    scroll_offset: u32,
    buf: *mut u8,
    buf_len: u32,
) -> i32 {
    let bridge = match (handle as *mut TorvoxBridge).as_mut() {
        Some(b) => b,
        None => return -1,
    };
    let session_guard = match bridge.session.lock() {
        Ok(g) => g,
        Err(_) => return -1,
    };
    let session = match session_guard.as_ref() {
        Some(s) => s,
        None => return 0,
    };
    let session_inner = match session.lock() {
        Ok(g) => g,
        Err(_) => return -1,
    };
    let snapshot = match session_inner
        .terminal()
        .try_take_snapshot_with_scroll(scroll_offset)
    {
        Some(s) => s,
        None => return 0,
    };
    let rows = snapshot.rows;
    let cols = snapshot.cols;
    let total = (rows * cols) as usize;
    let needed = 20 + total * 12;
    if (buf_len as usize) < needed {
        return -1;
    }
    *(buf as *mut u32) = rows;
    *(buf.add(4) as *mut u32) = cols;
    *(buf.add(8) as *mut u32) = snapshot.cursor_row;
    *(buf.add(12) as *mut u32) = snapshot.cursor_col;
    *(buf.add(16) as *mut u8) = if snapshot.cursor_visible { 1 } else { 0 };
    for i in 0..total {
        let cell = &snapshot.cells[i];
        let off = buf.add(20 + i * 12);
        *(off as *mut u32) = cell.codepoint;
        *(off.add(4) as *mut u32) = to_argb(&cell.foreground);
        *(off.add(8) as *mut u32) = to_argb(&cell.background);
    }
    total as i32
}

/// # Safety
///
/// `handle` must be a valid bridge handle. `path_ptr` must point to `path_len` valid bytes
/// or be null when `path_len` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_bridge_load_font_file(
    handle: i64,
    path_ptr: *const u8,
    path_len: i32,
) -> *mut core::ffi::c_char {
    let bridge = match (handle as *mut TorvoxBridge).as_mut() {
        Some(b) => b,
        None => return std::ptr::null_mut(),
    };
    let path = {
        let slice = if path_len >= 0 && !path_ptr.is_null() {
            unsafe { std::slice::from_raw_parts(path_ptr, path_len as usize) }
        } else {
            return std::ptr::null_mut();
        };
        String::from_utf8_lossy(slice).into_owned()
    };
    match bridge.load_font_file(path) {
        Some(family) => std::ffi::CString::new(family)
            .unwrap_or_default()
            .into_raw(),
        None => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_ping() {
        let config = TerminalConfig {
            shell: Shell::Custom {
                path: "/bin/sh".to_string(),
            },
            rows: 24,
            cols: 80,
            scrollback_lines: 50_000,
            font_size_tenths: 140,
            theme: torvox_core::config::Theme::catppuccin_mocha().into(),
            home: String::new(),
            user: String::new(),
            path: String::new(),
            working_directory: String::new(),
            prefix: String::new(),
        };
        let bridge = TorvoxBridge::new(config);
        assert_eq!(bridge.ping().unwrap(), "pong");
    }

    #[test]
    fn bridge_get_config() {
        let config = TerminalConfig {
            shell: Shell::Custom {
                path: "/bin/bash".to_string(),
            },
            rows: 40,
            cols: 120,
            scrollback_lines: 10000,
            font_size_tenths: 160,
            theme: torvox_core::config::Theme::dracula_plus().into(),
            home: String::new(),
            user: String::new(),
            path: String::new(),
            working_directory: String::new(),
            prefix: String::new(),
        };
        let bridge = TorvoxBridge::new(config.clone());
        let got = bridge.get_config();
        assert_eq!(got.shell, config.shell);
        assert_eq!(got.rows, config.rows);
    }

    #[test]
    fn shell_enum_default_is_system() {
        let s = Shell::default();
        assert!(matches!(s, Shell::SystemDefault));
    }

    #[test]
    fn shell_roundtrip_with_core() {
        let core_shell = torvox_core::config::Shell::Custom("/bin/zsh".to_string());
        let bridge_shell: Shell = core_shell.clone().into();
        assert!(matches!(bridge_shell, Shell::Custom { .. }));
        let back: torvox_core::config::Shell = bridge_shell.into();
        assert_eq!(core_shell, back);
    }

    #[test]
    fn terminal_config_roundtrip_with_core() {
        let core_config = torvox_core::config::TerminalConfig::default();
        let bridge_config = TerminalConfig::from_core_config(&core_config);
        assert!(matches!(bridge_config.shell, Shell::SystemDefault));
        assert_eq!(bridge_config.rows, core_config.rows);
        assert_eq!(bridge_config.cols, core_config.cols);
        assert_eq!(bridge_config.scrollback_lines, core_config.scrollback_lines);
        assert_eq!(bridge_config.font_size_tenths, core_config.font_size_tenths);
        let back = bridge_config.to_core_config();
        assert_eq!(core_config, back);
    }

    // ── R4: explicit `TerminalConfig` builder (lossy `From` deleted) ──

    /// `to_core_config` must copy EVERY shared field exactly
    /// (rows, cols, scrollback_lines, shell, font_size_tenths) —
    /// unlike the deleted `From` impl it never silently defaults them.
    #[test]
    fn terminal_config_to_core_copies_every_shared_field() {
        let bridge = TerminalConfig {
            shell: Shell::Custom {
                path: "/bin/zsh".to_string(),
            },
            rows: 48,
            cols: 160,
            scrollback_lines: 12_000,
            font_size_tenths: 200,
            theme: torvox_core::config::Theme::dracula_plus().into(),
            home: "/data/home".to_string(),
            user: "alice".to_string(),
            path: "/opt/bin".to_string(),
            working_directory: "/data/home/proj".to_string(),
            prefix: "/data/usr".to_string(),
        };
        let core = bridge.to_core_config();
        assert_eq!(core.rows, 48, "rows must be copied");
        assert_eq!(core.cols, 160, "cols must be copied");
        assert_eq!(
            core.scrollback_lines, 12_000,
            "scrollback_lines must be copied"
        );
        assert_eq!(
            core.font_size_tenths, 200,
            "font_size_tenths must be copied"
        );
        assert!(matches!(
            core.shell,
            torvox_core::config::Shell::Custom(path) if path == "/bin/zsh"
        ));
    }

    /// `from_core_config` copies the shared fields, leaves the bridge-only
    /// fields (home, user, path, working_directory, prefix) empty, and
    /// resets `theme` to the default catppuccin-mocha — the documented
    /// contract of the explicit builder.
    #[test]
    fn terminal_config_from_core_leaves_bridge_only_empty_and_theme_default() {
        let core_config = torvox_core::config::TerminalConfig {
            rows: 30,
            cols: 90,
            scrollback_lines: 7_000,
            shell: torvox_core::config::Shell::Custom("/bin/fish".to_string()),
            font_size_tenths: 180,
            backspace_mode: torvox_core::config::BackspaceMode::BS,
            right_alt_mode: torvox_core::config::RightAltMode::Meta,
        };
        let bridge = TerminalConfig::from_core_config(&core_config);
        // Shared fields copied exactly.
        assert_eq!(bridge.rows, 30);
        assert_eq!(bridge.cols, 90);
        assert_eq!(bridge.scrollback_lines, 7_000);
        assert_eq!(bridge.font_size_tenths, 180);
        assert!(matches!(
            bridge.shell,
            Shell::Custom { path } if path == "/bin/fish"
        ));
        // Bridge-only fields are intentionally NOT carried by this builder.
        assert_eq!(bridge.home, "", "home must be left empty");
        assert_eq!(bridge.user, "", "user must be left empty");
        assert_eq!(bridge.path, "", "path must be left empty");
        assert_eq!(
            bridge.working_directory, "",
            "working_directory must be left empty"
        );
        assert_eq!(bridge.prefix, "", "prefix must be left empty");
        // Theme resets to the documented default.
        assert_eq!(
            bridge.theme,
            torvox_core::config::Theme::catppuccin_mocha().into(),
            "theme must reset to catppuccin-mocha"
        );
    }

    #[test]
    fn bridge_attrs_roundtrip() {
        let core_attrs = torvox_core::cell::Attrs {
            bold: true,
            dim: true,
            italic: false,
            underline: true,
            double_underline: false,
            reverse: false,
            strikethrough: true,
            blink: false,
            hidden: false,
            overline: false,
            protected: true,
            double_width: false,
            double_height_top: false,
            double_height_bottom: false,
        };
        let bridge_attrs: BridgeAttrs = core_attrs.into();
        let back: torvox_core::cell::Attrs = bridge_attrs.into();
        assert_eq!(core_attrs, back);
    }

    #[test]
    fn grid_getters_return_defaults_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert!(bridge.get_grid_rows() > 0);
        assert!(bridge.get_grid_cols() > 0);
        assert_eq!(bridge.get_grid_rows(), 24);
        assert_eq!(bridge.get_grid_cols(), 80);
    }

    #[test]
    fn poll_bel_returns_false_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert!(!bridge.poll_bel(), "bell should be false without surface");
    }

    #[test]
    fn poll_bel_idempotent() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert!(!bridge.poll_bel());
        assert!(
            !bridge.poll_bel(),
            "repeated poll_bel should still be false"
        );
    }

    #[test]
    fn poll_clipboard_returns_none_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert_eq!(
            bridge.poll_clipboard(),
            None,
            "clipboard should be None without surface"
        );
    }

    #[test]
    fn poll_clipboard_idempotent() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert_eq!(bridge.poll_clipboard(), None);
        assert_eq!(
            bridge.poll_clipboard(),
            None,
            "repeated poll_clipboard should still be None"
        );
    }

    #[test]
    fn poll_shell_integration_returns_zero_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert_eq!(
            bridge.poll_shell_integration(),
            0,
            "shell integration should be 0 without surface"
        );
    }

    #[test]
    fn poll_sync_active_returns_false_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert!(
            !bridge.poll_sync_active(),
            "sync_active should be false without surface"
        );
    }

    #[test]
    fn cwd_returns_empty_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert_eq!(bridge.cwd(), "", "cwd should be empty without surface");
    }

    #[test]
    fn focus_event_does_not_panic_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        bridge.focus_event(true);
        bridge.focus_event(false);
    }

    #[test]
    fn scrollback_length_zero_without_surface() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert_eq!(
            bridge.scrollback_length(),
            0,
            "scrollback should be 0 without surface"
        );
    }

    #[test]
    fn set_save_path_succeeds() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        let temp_dir = std::env::temp_dir().join("torvox_test_save");
        let result = bridge.set_save_path(temp_dir.to_string_lossy().to_string());
        assert!(
            result.is_ok(),
            "set_save_path should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn has_saved_session_false_for_nonexistent() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        assert!(!bridge.has_saved_session("/nonexistent/path/session.bin".to_string()));
    }

    #[test]
    fn null_handle_poll_bel_returns_zero() {
        unsafe {
            let result = torvox_bridge_poll_bel(0);
            assert_eq!(result, 0, "null handle poll_bel should return 0");
        }
    }

    #[test]
    fn null_handle_poll_clipboard_returns_zero() {
        unsafe {
            let result = torvox_bridge_poll_clipboard(0);
            assert_eq!(result, 0, "null handle poll_clipboard should return 0");
        }
    }

    #[test]
    fn null_handle_poll_shell_integration_returns_zero() {
        unsafe {
            let result = torvox_bridge_poll_shell_integration(0);
            assert_eq!(
                result, 0,
                "null handle poll_shell_integration should return 0"
            );
        }
    }

    #[test]
    fn null_handle_poll_sync_active_returns_zero() {
        unsafe {
            let result = torvox_bridge_poll_sync_active(0);
            assert_eq!(result, 0, "null handle poll_sync_active should return 0");
        }
    }

    #[test]
    fn null_handle_cwd_returns_null() {
        unsafe {
            let result = torvox_bridge_cwd(0);
            assert!(result.is_null(), "null handle cwd should return null");
        }
    }

    #[test]
    fn null_handle_scrollback_length_returns_zero() {
        unsafe {
            let result = torvox_bridge_scrollback_len(0);
            assert_eq!(result, 0, "null handle scrollback_length should return 0");
        }
    }

    #[test]
    fn null_handle_focus_event_does_not_panic() {
        unsafe {
            torvox_bridge_focus_event(0, 1);
            torvox_bridge_focus_event(0, 0);
        }
    }

    #[test]
    fn free_cstring_null_does_not_panic() {
        unsafe {
            torvox_bridge_free_cstring(std::ptr::null_mut());
        }
    }

    #[test]
    fn free_string_null_does_not_panic() {
        unsafe {
            torvox_bridge_free_string(0);
        }
    }

    #[test]
    fn get_theme_names_returns_all_built_in() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        let names = bridge.get_theme_names();
        assert!(
            names.len() >= 10,
            "should have at least 10 built-in themes, got {}",
            names.len()
        );
        assert!(
            names.contains(&"Catppuccin Mocha".to_string()),
            "must include Catppuccin Mocha"
        );
    }

    #[test]
    fn get_theme_returns_known_theme() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        let theme = bridge.get_theme("Catppuccin Mocha".to_string());
        assert!(theme.is_some(), "Catppuccin Mocha should exist");
    }

    #[test]
    fn get_theme_returns_none_for_unknown() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        let theme = bridge.get_theme("Nonexistent Theme".to_string());
        assert!(theme.is_none(), "unknown theme should return None");
    }

    #[test]
    fn all_poll_apis_are_idempotent() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        for _ in 0..10 {
            assert!(!bridge.poll_bel());
            assert_eq!(bridge.poll_clipboard(), None);
            assert_eq!(bridge.poll_shell_integration(), 0);
            assert!(!bridge.poll_sync_active());
        }
    }

    #[test]
    fn concurrent_poll_apis_no_deadlock() {
        let bridge = TorvoxBridge::new(TerminalConfig::default());
        let bridge = &bridge;
        std::thread::scope(|scope| {
            let h1 = scope.spawn(|| {
                for _ in 0..100 {
                    let _ = bridge.poll_bel();
                }
            });
            let h2 = scope.spawn(|| {
                for _ in 0..100 {
                    let _ = bridge.poll_clipboard();
                }
            });
            let h3 = scope.spawn(|| {
                for _ in 0..100 {
                    let _ = bridge.poll_shell_integration();
                }
            });
            let h4 = scope.spawn(|| {
                for _ in 0..100 {
                    let _ = bridge.poll_sync_active();
                }
            });
            let h5 = scope.spawn(|| {
                for _ in 0..100 {
                    let _ = bridge.cwd();
                }
            });
            let h6 = scope.spawn(|| {
                for _ in 0..100 {
                    let _ = bridge.scrollback_length();
                }
            });
            h1.join().unwrap();
            h2.join().unwrap();
            h3.join().unwrap();
            h4.join().unwrap();
            h5.join().unwrap();
            h6.join().unwrap();
        });
    }

    #[test]
    fn bridge_new_and_free_roundtrip() {
        let config = TerminalConfig::default();
        let bridge = Box::new(TorvoxBridge::new(config));
        let ptr = Box::into_raw(bridge);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn get_config_preserves_all_fields() {
        let config = TerminalConfig {
            shell: Shell::Custom {
                path: "/bin/fish".to_string(),
            },
            rows: 50,
            cols: 160,
            scrollback_lines: 200000,
            font_size_tenths: 200,
            theme: torvox_core::config::Theme::dracula_plus().into(),
            home: "/home/test".to_string(),
            user: "testuser".to_string(),
            path: "/usr/bin:/usr/local/bin".to_string(),
            working_directory: "/tmp".to_string(),
            prefix: "myterm".to_string(),
        };
        let bridge = TorvoxBridge::new(config.clone());
        let got = bridge.get_config();
        assert_eq!(got.shell, config.shell);
        assert_eq!(got.rows, 50);
        assert_eq!(got.cols, 160);
        assert_eq!(got.scrollback_lines, 200000);
        assert_eq!(got.font_size_tenths, 200);
        assert_eq!(got.home, "/home/test");
        assert_eq!(got.user, "testuser");
        assert_eq!(got.path, "/usr/bin:/usr/local/bin");
        assert_eq!(got.working_directory, "/tmp");
        assert_eq!(got.prefix, "myterm");
    }

    // ═══════════════════════════════════════════════
    // safe_cstring unit tests
    // ═══════════════════════════════════════════════

    #[test]
    fn safe_cstring_normal_string() {
        let result = super::safe_cstring("hello world".to_string());
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_str().unwrap(), "hello world");
    }

    #[test]
    fn safe_cstring_strips_interior_nul() {
        let result = super::safe_cstring("hel\0lo".to_string());
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_str().unwrap(), "hello");
    }

    #[test]
    fn safe_cstring_all_nul_returns_none() {
        let result = super::safe_cstring("\0\0\0".to_string());
        assert!(result.is_none(), "all-NUL string should return None");
    }

    #[test]
    fn safe_cstring_empty_returns_none() {
        let result = super::safe_cstring(String::new());
        assert!(result.is_none(), "empty string should return None");
    }

    #[test]
    fn safe_cstring_single_char() {
        let result = super::safe_cstring("X".to_string());
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_str().unwrap(), "X");
    }

    #[test]
    fn safe_cstring_leading_trailing_nul_stripped() {
        let result = super::safe_cstring("\0abc\0".to_string());
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_str().unwrap(), "abc");
    }

    // ═══════════════════════════════════════════════
    // read_u32_le unit tests
    // ═══════════════════════════════════════════════

    #[test]
    fn read_u32_le_valid() {
        let bytes = 0x04030201u32.to_le_bytes();
        let result = super::read_u32_le(&bytes, 0);
        assert_eq!(result, Some(0x04030201));
    }

    #[test]
    fn read_u32_le_offset() {
        let mut bytes = vec![0u8; 12];
        bytes[4..8].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        let result = super::read_u32_le(&bytes, 4);
        assert_eq!(result, Some(0xDEADBEEF));
    }

    #[test]
    fn read_u32_le_truncated_buffer_returns_none() {
        let bytes = [0x01u8, 0x02, 0x03];
        let result = super::read_u32_le(&bytes, 0);
        assert!(result.is_none(), "should return None for truncated buffer");
    }

    #[test]
    fn read_u32_le_exact_boundary_returns_none() {
        let bytes = [0u8; 8];
        let result = super::read_u32_le(&bytes, 5);
        assert!(result.is_none(), "should return None at exact boundary");
    }

    #[test]
    fn read_u32_le_exactly_fits() {
        let bytes = [0u8; 8];
        let result = super::read_u32_le(&bytes, 4);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn read_u32_le_zero_value() {
        let bytes = [0u8; 4];
        let result = super::read_u32_le(&bytes, 0);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn read_u32_le_max_value() {
        let bytes = 0xFFFFFFFFu32.to_le_bytes();
        let result = super::read_u32_le(&bytes, 0);
        assert_eq!(result, Some(0xFFFFFFFF));
    }

    // ═══════════════════════════════════════════════
    // read_wire_string unit tests
    // ═══════════════════════════════════════════════

    #[test]
    fn read_wire_string_valid() {
        let s = "hello";
        let len_bytes = (s.len() as u32).to_le_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&len_bytes);
        bytes.extend_from_slice(s.as_bytes());
        let mut pos = 0usize;
        let result = super::read_wire_string(&bytes, &mut pos);
        assert_eq!(result, Some("hello".to_string()));
        assert_eq!(pos, 9);
    }

    #[test]
    fn read_wire_string_empty_string() {
        let len_bytes = 0u32.to_le_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&len_bytes);
        let mut pos = 0usize;
        let result = super::read_wire_string(&bytes, &mut pos);
        assert_eq!(result, Some(String::new()));
        assert_eq!(pos, 4);
    }

    #[test]
    fn read_wire_string_truncated_length_returns_none() {
        let bytes = [0x00, 0x00];
        let mut pos = 0usize;
        let result = super::read_wire_string(&bytes, &mut pos);
        assert!(result.is_none());
    }

    #[test]
    fn read_wire_string_length_exceeds_buffer_returns_none() {
        let len_bytes = 100u32.to_le_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&len_bytes);
        bytes.extend_from_slice(b"short");
        let mut pos = 0usize;
        let result = super::read_wire_string(&bytes, &mut pos);
        assert!(result.is_none());
    }

    #[test]
    fn read_wire_string_unicode() {
        let s = "你好世界";
        let len_bytes = (s.len() as u32).to_le_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&len_bytes);
        bytes.extend_from_slice(s.as_bytes());
        let mut pos = 0usize;
        let result = super::read_wire_string(&bytes, &mut pos);
        assert_eq!(result, Some(s.to_string()));
        assert_eq!(pos, 4 + s.len());
    }

    #[test]
    fn read_wire_string_chained() {
        let s1 = "hello";
        let s2 = "world";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(s1.len() as u32).to_le_bytes());
        bytes.extend_from_slice(s1.as_bytes());
        bytes.extend_from_slice(&(s2.len() as u32).to_le_bytes());
        bytes.extend_from_slice(s2.as_bytes());
        let mut pos = 0usize;
        let r1 = super::read_wire_string(&bytes, &mut pos);
        assert_eq!(r1, Some("hello".to_string()));
        let r2 = super::read_wire_string(&bytes, &mut pos);
        assert_eq!(r2, Some("world".to_string()));
        assert_eq!(pos, bytes.len());
    }

    // ═══════════════════════════════════════════════
    // Theme wire deserialization tests
    // ═══════════════════════════════════════════════

    #[test]
    fn theme_wire_deserialization_read_color_advances_pos() {
        let name = "TestTheme";
        let name_bytes = name.as_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name_bytes);
        for i in 0u32..20 {
            bytes.extend_from_slice(&(i * 0x01010101).to_le_bytes());
        }

        let mut pos = 0usize;
        let read_name = super::read_wire_string(&bytes, &mut pos).unwrap();
        assert_eq!(read_name, "TestTheme");

        let read_color = |bytes: &[u8], pos: &mut usize| -> u32 {
            let color_value = super::read_u32_le(bytes, *pos).unwrap();
            *pos += 4;
            color_value
        };

        assert_eq!(read_color(&bytes, &mut pos), 0x00000000);
        assert_eq!(read_color(&bytes, &mut pos), 0x01010101);
        assert_eq!(read_color(&bytes, &mut pos), 0x02020202);
        assert_eq!(read_color(&bytes, &mut pos), 0x03030303);
        assert_eq!(read_color(&bytes, &mut pos), 0x04040404);
        assert_eq!(read_color(&bytes, &mut pos), 0x05050505);
        assert_eq!(read_color(&bytes, &mut pos), 0x06060606);
        assert_eq!(read_color(&bytes, &mut pos), 0x07070707);
        assert_eq!(read_color(&bytes, &mut pos), 0x08080808);
        assert_eq!(read_color(&bytes, &mut pos), 0x09090909);
        assert_eq!(read_color(&bytes, &mut pos), 0x0A0A0A0A);
        assert_eq!(read_color(&bytes, &mut pos), 0x0B0B0B0B);
        assert_eq!(read_color(&bytes, &mut pos), 0x0C0C0C0C);
        assert_eq!(read_color(&bytes, &mut pos), 0x0D0D0D0D);
        assert_eq!(read_color(&bytes, &mut pos), 0x0E0E0E0E);
        assert_eq!(read_color(&bytes, &mut pos), 0x0F0F0F0F);
        assert_eq!(read_color(&bytes, &mut pos), 0x10101010);
        assert_eq!(read_color(&bytes, &mut pos), 0x11111111);
        assert_eq!(read_color(&bytes, &mut pos), 0x12121212);
        assert_eq!(read_color(&bytes, &mut pos), 0x13131313);
        assert_eq!(pos, bytes.len(), "should consume all bytes");
    }

    #[test]
    fn theme_wire_deserialization_truncated_colors_graceful() {
        let name = "Short";
        let name_bytes = name.as_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name_bytes);
        for i in 0u32..3 {
            bytes.extend_from_slice(&(i | 0xFF).to_le_bytes());
        }

        let mut pos = 0usize;
        let _read_name = super::read_wire_string(&bytes, &mut pos).unwrap();

        let read_color = |bytes: &[u8], pos: &mut usize| -> u32 {
            let color_value = super::read_u32_le(bytes, *pos).unwrap_or(0);
            *pos += 4;
            color_value
        };

        assert_eq!(read_color(&bytes, &mut pos), 0xFF);
        assert_eq!(read_color(&bytes, &mut pos), 0xFF);
        assert_eq!(read_color(&bytes, &mut pos), 0xFF);
        assert_eq!(read_color(&bytes, &mut pos), 0);
        assert_eq!(read_color(&bytes, &mut pos), 0);
    }

    // ═══════════════════════════════════════════════
    // Notification FFI safety tests
    // ═══════════════════════════════════════════════

    #[test]
    fn notification_free_null_ptr_is_safe() {
        unsafe {
            super::torvox_bridge_free_notification(0);
        }
    }

    #[test]
    fn notification_alloc_free_roundtrip() {
        let title = std::ffi::CString::new("Test Title").unwrap();
        let body = std::ffi::CString::new("Test Body").unwrap();
        let title_ptr = title.into_raw();
        let body_ptr = body.into_raw();
        let buf = Box::new([title_ptr, body_ptr]);
        let ptr = Box::into_raw(buf) as i64;
        unsafe {
            super::torvox_bridge_free_notification(ptr);
        }
    }

    #[test]
    fn notification_alloc_free_with_nul_in_body() {
        let title = std::ffi::CString::new("Title").unwrap();
        let title_ptr = title.into_raw();
        // Manually create a pointer to a C string containing interior NULs
        let body_raw: &[u8] = b"Body\0with\0nul\0";
        let body_ptr =
            unsafe { std::ffi::CString::from_vec_unchecked(body_raw.to_vec()) }.into_raw();
        let buf = Box::new([title_ptr, body_ptr]);
        let ptr = Box::into_raw(buf) as i64;
        unsafe {
            super::torvox_bridge_free_notification(ptr);
        }
    }

    #[test]
    fn safe_cstring_with_emoji() {
        let result = super::safe_cstring("\u{1F680} Hello 世界".to_string());
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_str().unwrap(), "\u{1F680} Hello 世界");
    }

    #[test]
    fn safe_cstring_with_newlines() {
        let result = super::safe_cstring("line1\nline2\rline3".to_string());
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_str().unwrap(), "line1\nline2\rline3");
    }
}
