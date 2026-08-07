//! Output processor — extracts OSC events, BEL, shell integration markers
//! from raw PTY output before passing to the VT emulator.
//!
//! # Requirements
//! - FR-033 — Scrollback: terminal content
//!
//! # Responsibilities
//! - Decode OSC 52/7/8/9/777 events into structured types
//! - Detect BEL (0x07) characters for audible bell
//! - Extract OSC 133 shell integration markers
//! - Produce filtered output for the VT parser
//!
//! Extracted from `Session::process_output` to improve locality and testability.

use crate::terminal::osc_handler::{OscEvent, OscHandler};

/// Shell integration markers (OSC 133).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ShellIntegration {
    #[default]
    None = 0,
    PromptStart = 1,
    PromptEnd = 2,
    CommandStart = 3,
    CommandExecuted = 4,
}

impl From<u8> for ShellIntegration {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::PromptStart,
            2 => Self::PromptEnd,
            3 => Self::CommandStart,
            4 => Self::CommandExecuted,
            _ => Self::None,
        }
    }
}

/// State snapshot produced by processing one chunk of PTY output.
#[derive(Debug, Default)]
pub struct OutputSnapshot {
    /// Clipboard text set by OSC 52.
    pub clipboard: Option<String>,
    /// OSC 52 clipboard read request: the selection name to answer.
    pub clipboard_read: Option<String>,
    /// Working directory reported by OSC 7.
    pub cwd: Option<String>,
    /// Hyperlink URL set by OSC 8.
    pub hyperlink: Option<String>,
    /// Notification (title, body) from OSC 9 or OSC 777.
    pub notification: Option<(String, String)>,
    /// BEL character detected in this chunk.
    pub bel: bool,
    /// Shell integration marker detected in this chunk.
    pub shell_integration: ShellIntegration,
    /// Exit code from OSC 133;D[;exit_code] (None unless D carried one).
    pub shell_exit_code: Option<i32>,
    /// Filtered output bytes for the VT parser.
    pub filtered: Vec<u8>,
}

/// Processes raw PTY output, extracting events and producing filtered bytes.
pub struct OutputProcessor {
    osc_handler: OscHandler,
}

impl Default for OutputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputProcessor {
    pub fn new() -> Self {
        Self {
            osc_handler: OscHandler::new(),
        }
    }

    /// Process a raw output chunk and return a snapshot of decoded events.
    pub fn process(&mut self, data: &[u8]) -> OutputSnapshot {
        self.osc_handler.process(data);

        let mut snapshot = OutputSnapshot::default();

        for event in self.osc_handler.events() {
            match event {
                OscEvent::Clipboard(ce) => {
                    snapshot.clipboard = Some(ce.text.clone());
                }
                OscEvent::ClipboardRead(ce) => {
                    snapshot.clipboard_read = Some(ce.selection.clone());
                }
                OscEvent::Cwd(ce) => {
                    snapshot.cwd = Some(ce.path.clone());
                }
                OscEvent::Hyperlink(he) => {
                    snapshot.hyperlink = he.url.clone();
                }
                OscEvent::Notification(ne) => {
                    snapshot.notification = Some((ne.title.clone(), ne.body.clone()));
                }
                OscEvent::ShellIntegration(se) => {
                    snapshot.shell_integration = match se.marker {
                        b'A' => ShellIntegration::PromptStart,
                        b'B' => ShellIntegration::PromptEnd,
                        b'C' => ShellIntegration::CommandStart,
                        b'D' => ShellIntegration::CommandExecuted,
                        _ => ShellIntegration::None,
                    };
                    snapshot.shell_exit_code = se.exit_code;
                }
            }
        }

        let filtered = self.osc_handler.output();
        if snapshot.shell_integration == ShellIntegration::None && filtered.contains(&0x07) {
            snapshot.bel = true;
        }
        snapshot.filtered = filtered.to_vec();
        snapshot
    }

    /// Access the most recently processed filtered output.
    pub fn output(&self) -> &[u8] {
        self.osc_handler.output()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bel_detection() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"hello\x07world");
        assert!(snap.bel);
        assert_eq!(snap.filtered, b"hello\x07world");
    }

    #[test]
    fn no_bel() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"no bell here");
        assert!(!snap.bel);
    }

    #[test]
    fn shell_integration_prompt_start() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]133;A\x07");
        assert_eq!(snap.shell_integration, ShellIntegration::PromptStart);
    }

    #[test]
    fn shell_integration_command_start() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]133;C\x07");
        assert_eq!(snap.shell_integration, ShellIntegration::CommandStart);
    }

    #[test]
    fn shell_integration_st_terminator() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]133;B\x1b\\");
        assert_eq!(snap.shell_integration, ShellIntegration::PromptEnd);
    }

    #[test]
    fn osc52_clipboard_read_forwarded() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]52;c;?\x07");
        assert_eq!(snap.clipboard_read.as_deref(), Some("c"));
        assert!(
            snap.filtered.is_empty(),
            "read request must not reach the VT parser"
        );
    }

    #[test]
    fn no_shell_integration() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"plain text");
        assert_eq!(snap.shell_integration, ShellIntegration::None);
    }
}
