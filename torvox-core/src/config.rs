use alloc::string::String;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub rows: u32,
    pub cols: u32,
    pub scrollback_lines: u32,
    pub shell: Shell,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            scrollback_lines: 50_000,
            shell: Shell::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Shell {
    #[default]
    SystemDefault,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderConfig {
    pub font: FontConfig,
    pub bg_color: [u8; 3],
    pub fg_color: [u8; 3],
    pub cursor_style: crate::cursor::CursorStyle,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            bg_color: [30, 30, 46],
            fg_color: [205, 214, 244],
            cursor_style: crate::cursor::CursorStyle::Block,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontConfig {
    pub family: String,
    pub size: u16,
    pub line_spacing: i16,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: String::from("JetBrains Mono Nerd Font"),
            size: 14,
            line_spacing: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_config_default() {
        let c = TerminalConfig::default();
        assert_eq!(c.rows, 24);
        assert_eq!(c.cols, 80);
        assert_eq!(c.scrollback_lines, 50_000);
    }

    #[test]
    fn render_config_default() {
        let c = RenderConfig::default();
        assert_eq!(c.font.family, "JetBrains Mono Nerd Font");
        assert_eq!(c.font.size, 14);
    }
}
