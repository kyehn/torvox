#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub struct BridgeCell {
    pub char_code: u32,
    pub fg: u32,
    pub bg: u32,
    pub attrs: BridgeAttrs,
}

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
}

impl From<torvox_core::cell::Attrs> for BridgeAttrs {
    fn from(a: torvox_core::cell::Attrs) -> Self {
        Self {
            bold: a.bold,
            dim: a.dim,
            italic: a.italic,
            underline: a.underline,
            double_underline: a.double_underline,
            reverse: a.reverse,
            strikethrough: a.strikethrough,
            blink: a.blink,
            hidden: a.hidden,
            overline: a.overline,
        }
    }
}

impl From<BridgeAttrs> for torvox_core::cell::Attrs {
    fn from(a: BridgeAttrs) -> Self {
        Self {
            bold: a.bold,
            dim: a.dim,
            italic: a.italic,
            underline: a.underline,
            double_underline: a.double_underline,
            reverse: a.reverse,
            strikethrough: a.strikethrough,
            blink: a.blink,
            hidden: a.hidden,
            overline: a.overline,
        }
    }
}

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
    fn from(s: Shell) -> Self {
        match s {
            Shell::SystemDefault => torvox_core::config::Shell::SystemDefault,
            Shell::Custom { path } => torvox_core::config::Shell::Custom(path),
        }
    }
}

impl From<torvox_core::config::Shell> for Shell {
    fn from(s: torvox_core::config::Shell) -> Self {
        match s {
            torvox_core::config::Shell::SystemDefault => Shell::SystemDefault,
            torvox_core::config::Shell::Custom(path) => Shell::Custom { path },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[boltffi::data]
pub struct TerminalConfig {
    pub shell: Shell,
    pub rows: u32,
    pub cols: u32,
    pub scrollback_lines: u32,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: Shell::SystemDefault,
            rows: 24,
            cols: 80,
            scrollback_lines: 50_000,
        }
    }
}

impl From<TerminalConfig> for torvox_core::config::TerminalConfig {
    fn from(c: TerminalConfig) -> Self {
        Self {
            shell: c.shell.into(),
            rows: c.rows,
            cols: c.cols,
            scrollback_lines: c.scrollback_lines,
        }
    }
}

impl From<torvox_core::config::TerminalConfig> for TerminalConfig {
    fn from(c: torvox_core::config::TerminalConfig) -> Self {
        Self {
            shell: c.shell.into(),
            rows: c.rows,
            cols: c.cols,
            scrollback_lines: c.scrollback_lines,
        }
    }
}

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
    },
}

impl From<torvox_core::event::TerminalEvent> for TerminalEvent {
    fn from(e: torvox_core::event::TerminalEvent) -> Self {
        match e {
            torvox_core::event::TerminalEvent::OutputReady => TerminalEvent::DirtyRegion {
                start_row: 0,
                end_row: 0,
            },
            torvox_core::event::TerminalEvent::Bell => TerminalEvent::Bell,
            torvox_core::event::TerminalEvent::TitleChanged(t) => {
                TerminalEvent::TitleChanged { title: t }
            }
            torvox_core::event::TerminalEvent::ClipboardRequest(t) => {
                TerminalEvent::ClipboardRequest { text: t }
            }
            torvox_core::event::TerminalEvent::HyperlinkHover(u) => {
                TerminalEvent::HyperlinkHover { url: u }
            }
            torvox_core::event::TerminalEvent::ProcessExited(c) => {
                TerminalEvent::ProcessExited { exit_code: c }
            }
            torvox_core::event::TerminalEvent::CursorChanged(cursor) => {
                TerminalEvent::CursorChanged {
                    row: cursor.row,
                    col: cursor.col,
                }
            }
            torvox_core::event::TerminalEvent::SelectionChanged(sel) => match sel {
                Some(s) => {
                    let (lo, hi) = s.ordered();
                    TerminalEvent::SelectionChanged {
                        start_row: lo.row,
                        start_col: lo.col,
                        end_row: hi.row,
                        end_col: hi.col,
                    }
                }
                None => TerminalEvent::SelectionChanged {
                    start_row: 0,
                    start_col: 0,
                    end_row: 0,
                    end_col: 0,
                },
            },
            torvox_core::event::TerminalEvent::DirtyRegion(dr) => TerminalEvent::DirtyRegion {
                start_row: dr.start_row,
                end_row: dr.end_row,
            },
        }
    }
}

#[boltffi::error]
#[derive(Debug, Clone, thiserror::Error)]
pub enum TerminalError {
    #[error("PTY error: {detail}")]
    PtyError { detail: String },
    #[error("invalid config: {detail}")]
    InvalidConfig { detail: String },
}

#[allow(dead_code)]
pub struct TorvoxBridge {
    config: TerminalConfig,
    surface: std::sync::Mutex<Option<crate::surface::AndroidSurface>>,
}

#[allow(dead_code)]
#[boltffi::export]
impl TorvoxBridge {
    fn new(config: TerminalConfig) -> Self {
        Self {
            config,
            surface: std::sync::Mutex::new(None),
        }
    }

    fn ping(&self) -> String {
        "pong".to_string()
    }

    fn spawn_terminal(&self, rows: u32, cols: u32) -> Result<i32, TerminalError> {
        let shell: torvox_core::config::Shell = self.config.shell.clone().into();
        let shell_path = match &shell {
            torvox_core::config::Shell::SystemDefault => "/system/bin/sh",
            torvox_core::config::Shell::Custom(path) => path.as_str(),
        };
        let pty =
            torvox_terminal::PtyPair::spawn(shell_path, rows as u16, cols as u16).map_err(|e| {
                TerminalError::PtyError {
                    detail: e.to_string(),
                }
            })?;
        Ok(pty.child_pid().as_raw())
    }

    fn set_native_window(&self, window_ptr: i64) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if surface_guard.is_none() {
            let mut surface =
                crate::surface::AndroidSurface::new(self.config.rows, self.config.cols);
            surface
                .set_native_window(window_ptr as *mut std::ffi::c_void)
                .map_err(|e| TerminalError::PtyError {
                    detail: e.to_string(),
                })?;
            *surface_guard = Some(surface);
        }
        Ok(())
    }

    fn render(&self) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.render().map_err(|e| TerminalError::PtyError {
                detail: e.to_string(),
            })?;
        }
        Ok(())
    }

    fn resize(&self, rows: u32, cols: u32) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.resize(rows, cols);
        }
        Ok(())
    }

    fn release_surface(&self) {
        if let Ok(mut guard) = self.surface.lock() {
            *guard = None;
        }
    }

    fn scrollback_len(&self) -> u32 {
        self.surface
            .lock()
            .ok()
            .and_then(|g| {
                g.as_ref()
                    .map(|s| s.terminal().grid.scrollback_len() as u32)
            })
            .unwrap_or(0)
    }

    fn scrollback_line(&self, index: u32) -> Option<String> {
        self.surface.lock().ok().and_then(|g| {
            g.as_ref().and_then(|s| {
                s.terminal()
                    .grid
                    .scrollback_line(index as usize)
                    .map(|line| {
                        let mut text = String::new();
                        for col in 0..line.len() {
                            if let Some(cell) = line.get(col) {
                                text.push(cell.char);
                            }
                        }
                        text.trim_end().to_string()
                    })
            })
        })
    }

    fn get_config(&self) -> TerminalConfig {
        self.config.clone()
    }

    fn write_to_pty(&self, data: Vec<u8>) -> Result<(), TerminalError> {
        let mut surface_guard = self.surface.lock().map_err(|e| TerminalError::PtyError {
            detail: format!("lock failed: {}", e),
        })?;
        if let Some(surface) = surface_guard.as_mut() {
            surface.write_to_pty(&data);
        }
        Ok(())
    }

    fn echo_cells(&self, cells: Vec<BridgeCell>) -> Vec<BridgeCell> {
        cells
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
        };
        let bridge = TorvoxBridge::new(config);
        assert_eq!(bridge.ping(), "pong");
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
        };
        let bridge = TorvoxBridge::new(config.clone());
        let got = bridge.get_config();
        assert_eq!(got.shell, config.shell);
        assert_eq!(got.rows, config.rows);
    }

    #[test]
    fn bridge_echo_cells() {
        let config = TerminalConfig::default();
        let bridge = TorvoxBridge::new(config);
        let cells = vec![BridgeCell {
            char_code: 'A' as u32,
            fg: 0xFFFFFF,
            bg: 0x000000,
            attrs: BridgeAttrs::default(),
        }];
        let result = bridge.echo_cells(cells.clone());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].char_code, 'A' as u32);
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
        let bridge_config: TerminalConfig = core_config.clone().into();
        assert!(matches!(bridge_config.shell, Shell::SystemDefault));
        let back: torvox_core::config::TerminalConfig = bridge_config.into();
        assert_eq!(core_config, back);
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
        };
        let bridge_attrs: BridgeAttrs = core_attrs.into();
        let back: torvox_core::cell::Attrs = bridge_attrs.into();
        assert_eq!(core_attrs, back);
    }
}
