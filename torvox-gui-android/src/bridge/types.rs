pub(crate) const DEFAULT_GRID_ROWS: u32 = 24;
pub(crate) const DEFAULT_GRID_COLS: u32 = 80;
pub(crate) const LIST_SEPARATOR: &str = "\x1f";
pub(crate) const TEXT_PREVIEW_MAX_CHARS: usize = 80;

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

pub(crate) fn rgb_to_u32(color: [u8; 3]) -> u32 {
    ((color[0] as u32) << 16) | ((color[1] as u32) << 8) | (color[2] as u32)
}

pub(crate) fn u32_to_rgb(value: u32) -> [u8; 3] {
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
                        mode: selection.mode.to_u8(),
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

#[boltffi::data]
#[derive(Debug, Default)]
pub struct PollAllResult {
    pub bel: bool,
    pub clipboard: Option<String>,
    pub notification_title: Option<String>,
    pub notification_body: Option<String>,
    pub sync_active: bool,
    pub shell_integration: u8,
}

#[boltffi::data]
#[derive(Debug, Clone, Copy)]
pub struct SelectionEndpointParams {
    pub handle_side: u8,
    pub anchor_row: i32,
    pub anchor_col: i32,
    pub other_row: i32,
    pub other_col: i32,
    pub mode: u8,
    pub origin_row: i32,
    pub origin_col: i32,
}

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

#[boltffi::error]
#[derive(Debug, Clone, thiserror::Error)]
pub enum BridgeError {
    #[error("PTY error: {0}")]
    Pty(String),
    #[error("render error: {0}")]
    Render(String),
    #[error("lock contention: {context}")]
    Lock { context: String },
    #[error("session unavailable: {detail}")]
    SessionUnavailable { detail: String },
    #[error("invalid config: {detail}")]
    InvalidConfig { detail: String },
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl From<BridgeError> for TerminalError {
    fn from(e: BridgeError) -> Self {
        match e {
            BridgeError::Pty(d) => TerminalError::PtyError { detail: d },
            BridgeError::Render(d) => TerminalError::PtyError { detail: d },
            BridgeError::Lock { context } => TerminalError::PtyError {
                detail: format!("lock contention: {context}"),
            },
            BridgeError::SessionUnavailable { detail } => TerminalError::SessionUnavailable {
                detail: detail.to_string(),
            },
            BridgeError::InvalidConfig { detail } => TerminalError::InvalidConfig {
                detail: detail.to_string(),
            },
            BridgeError::Unsupported(d) => TerminalError::PtyError { detail: d },
        }
    }
}

macro_rules! lock_surface {
    ($bridge:expr) => {
        $bridge
            .surface
            .lock()
            .map_err(|_| $crate::bridge::BridgeError::Lock {
                context: "surface".into(),
            })?
    };
}

macro_rules! lock_session {
    ($bridge:expr) => {
        $bridge
            .session
            .lock()
            .map_err(|_| $crate::bridge::BridgeError::Lock {
                context: "session".into(),
            })?
    };
}
