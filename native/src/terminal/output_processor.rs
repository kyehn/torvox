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
use std::sync::atomic::{AtomicBool, Ordering};

/// Maximum bytes to capture between OSC 133 B and C markers.
/// Prevents unbounded memory growth from runaway output (e.g., `cat /dev/urandom | xxd`).
const MAX_CAPTURE_BYTES: usize = 64 * 1024;

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
    /// ConEmu progress (state, value) from OSC 9;4.
    /// state: 0=remove, 1=indeterminate, 2=normal, 3=error, 4=indeterminate(error).
    /// value: 0–100 clamped.
    pub progress: Option<(u8, u8)>,
    /// BEL character detected in this chunk.
    pub bel: bool,
    /// Shell integration marker detected in this chunk.
    pub shell_integration: ShellIntegration,
    /// Exit code from OSC 133;D `[;exit_code]` (None unless D carried one).
    pub shell_exit_code: Option<i32>,
    /// Filtered output bytes for the VT parser.
    pub filtered: Vec<u8>,
}

/// Byte prefix of an OSC 133 shell-integration marker (`ESC ] 133 ; <letter>`),
/// matched byte-by-byte so marker sequences split across chunk boundaries
/// are still recognised.
const OSC133_MARKER_PREFIX: &[u8] = b"\x1b]133;";

/// Processes raw PTY output, extracting events and producing filtered bytes.
pub struct OutputProcessor {
    osc_handler: OscHandler,
    /// OSC 133 `last_command_output`: text captured between the B (prompt
    /// end) and C (command output start) markers. Consumed and cleared by
    /// [`Self::take_last_command_output`].
    last_command_output: String,
    /// True while bytes are being captured (between B and C markers).
    capturing_output: bool,
    /// In-progress capture buffer (raw bytes, lossy-converted on finalise).
    capture_buf: Vec<u8>,
    /// Number of `OSC133_MARKER_PREFIX` bytes matched so far (carried
    /// across chunk boundaries).
    prefix_match: usize,
    /// Consumed-but-unconfirmed prefix bytes; flushed into the capture on a
    /// mismatch so non-133 escape sequences are preserved verbatim.
    pending: Vec<u8>,
    /// True while consuming the marker terminator (BEL or ST `ESC \`).
    skip_terminator: bool,
    /// True after the `ESC` of an ST (`ESC \`) terminator.
    st_esc: bool,
    /// P1-1 `new_output` flag (dual-flag protocol, see
    /// docs/reference/dual-flag-protocol.md): set when a non-empty PTY
    /// chunk is ingested ([`Self::process`]), read-and-cleared by the
    /// render thread via [`Self::take_new_output`]. Independent from the
    /// P2-1 `dirty` flag: new output may reset the viewport to the
    /// bottom; dirty (selection/highlight/font-size changes) must only
    /// trigger a repaint, never a scroll reset.
    new_output: AtomicBool,
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
            last_command_output: String::new(),
            capturing_output: false,
            capture_buf: Vec::with_capacity(1024),
            prefix_match: 0,
            pending: Vec::with_capacity(8),
            skip_terminator: false,
            st_esc: false,
            new_output: AtomicBool::new(false),
        }
    }

    /// Take and clear the `new_output` flag (single-consumer read-clear;
    /// the render thread is the only reader). Returns true if any PTY
    /// output was ingested since the last take.
    pub fn take_new_output(&self) -> bool {
        self.new_output.swap(false, Ordering::AcqRel)
    }

    /// Take the OSC 133 last-command-output text (captured between the B
    /// and C markers), clearing it so the next capture starts fresh.
    pub fn take_last_command_output(&mut self) -> String {
        std::mem::take(&mut self.last_command_output)
    }

    /// Scan raw output for OSC 133 A/B/C/D markers and capture the text
    /// between the B (prompt end) and C (command output start) markers
    /// into [`Self::last_command_output`]. A new A (prompt start) resets
    /// any in-progress capture.
    fn scan_osc133(&mut self, data: &[u8]) {
        for &byte in data {
            self.scan_osc133_byte(byte);
        }
    }

    fn scan_osc133_byte(&mut self, byte: u8) {
        // Marker terminator (BEL or ST `ESC \`) — never part of the text.
        if self.skip_terminator {
            if byte == 0x07 {
                self.skip_terminator = false;
                return;
            }
            if byte == 0x1B {
                self.skip_terminator = false;
                self.st_esc = true;
                return;
            }
            self.skip_terminator = false;
        }
        if self.st_esc {
            self.st_esc = false;
            if byte == b'\\' {
                return;
            }
            // Not a valid ST terminator; reprocess the byte below.
        }
        if self.prefix_match == OSC133_MARKER_PREFIX.len() {
            match byte {
                // A: prompt start — reset the capture for the new cycle.
                b'A' => {
                    self.capturing_output = false;
                    self.capture_buf.clear();
                }
                // B: prompt end — begin capturing the command output region.
                b'B' => {
                    self.capturing_output = true;
                    self.capture_buf.clear();
                }
                // C: command output start — end capture and store the result.
                b'C' => {
                    self.capturing_output = false;
                    self.last_command_output =
                        String::from_utf8_lossy(&std::mem::take(&mut self.capture_buf))
                            .into_owned();
                }
                _ => {}
            }
            self.pending.clear();
            self.prefix_match = 0;
            self.skip_terminator = true;
            return;
        }
        if byte == OSC133_MARKER_PREFIX[self.prefix_match] {
            self.pending.push(byte);
            self.prefix_match += 1;
            return;
        }
        // Prefix mismatch: the pending bytes were ordinary text (e.g. a
        // different escape sequence) — flush them, then reprocess `byte`.
        if self.capturing_output {
            let remaining = MAX_CAPTURE_BYTES.saturating_sub(self.capture_buf.len());
            let flush_len = self.pending.len().min(remaining);
            self.capture_buf
                .extend_from_slice(&self.pending[..flush_len]);
        }
        self.pending.clear();
        self.prefix_match = 0;
        if byte == OSC133_MARKER_PREFIX[0] {
            self.pending.push(byte);
            self.prefix_match = 1;
        } else if self.capturing_output && self.capture_buf.len() < MAX_CAPTURE_BYTES {
            self.capture_buf.push(byte);
        }
    }

    /// Process a raw output chunk and return a snapshot of decoded events.
    pub fn process(&mut self, data: &[u8]) -> OutputSnapshot {
        // P1-1: PTY ingest → raise `new_output` (bypass flag, not a queued
        // event — see docs/reference/dual-flag-protocol.md). Empty chunks
        // carry no output and do not count.
        if !data.is_empty() {
            self.new_output.store(true, Ordering::Release);
        }
        self.scan_osc133(data);
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
                OscEvent::Progress(pe) => {
                    snapshot.progress = Some((pe.state, pe.value));
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

    // ── P1-1 new_output flag (dual-flag protocol) ────────────────────

    #[test]
    fn pty_write_raises_new_output_flag() {
        let mut proc = OutputProcessor::new();
        // Idle before any ingest: flag must be clear.
        assert!(!proc.take_new_output());
        // PTY write → flag true.
        let _ = proc.process(b"echo hi\r\n");
        assert!(proc.take_new_output());
        // Single-consumer read-clear: a second take sees false.
        assert!(!proc.take_new_output());
    }

    #[test]
    fn idle_keeps_new_output_flag_clear() {
        let mut proc = OutputProcessor::new();
        let _ = proc.process(b"first chunk");
        assert!(proc.take_new_output());
        // No further PTY writes: the flag stays clear across takes.
        assert!(!proc.take_new_output());
        assert!(!proc.take_new_output());
    }

    #[test]
    fn empty_chunk_does_not_raise_new_output_flag() {
        let mut proc = OutputProcessor::new();
        let _ = proc.process(b"");
        assert!(!proc.take_new_output());
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

    #[test]
    fn shell_integration_command_executed() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]133;D\x1b\\");
        assert_eq!(snap.shell_integration, ShellIntegration::CommandExecuted);
    }

    #[test]
    fn shell_integration_exit_code() {
        // `D;exit_code` carries the command exit code (termlib
        // OscParser handleOsc133 COMMAND_FINISHED marker).
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]133;D;0\x1b\\");
        assert_eq!(snap.shell_integration, ShellIntegration::CommandExecuted);
        assert_eq!(snap.shell_exit_code, Some(0));
        let snap = proc.process(b"\x1b]133;D;42\x1b\\");
        assert_eq!(snap.shell_exit_code, Some(42));
        // Plain D has no exit code.
        let snap = proc.process(b"\x1b]133;D\x1b\\");
        assert_eq!(snap.shell_exit_code, None);
        // A/B/C never carry exit codes.
        let snap = proc.process(b"\x1b]133;C\x1b\\");
        assert_eq!(snap.shell_exit_code, None);
    }

    #[test]
    fn shell_integration_empty_osc() {
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;\x07").shell_integration,
            ShellIntegration::None
        );
        assert_eq!(
            proc.process(b"\x1b]133;\x1b\\").shell_integration,
            ShellIntegration::None
        );
    }

    #[test]
    fn shell_integration_unknown_marker() {
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;X\x07").shell_integration,
            ShellIntegration::None
        );
    }

    #[test]
    fn shell_integration_incomplete_sequence() {
        // Truncated OSC (no terminator yet) must not fire a marker.
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;C").shell_integration,
            ShellIntegration::None
        );
        assert_eq!(
            proc.process(b"\x1b]133;").shell_integration,
            ShellIntegration::None
        );
    }

    #[test]
    fn shell_integration_mixed_terminators() {
        // Both BEL and ST terminators are accepted for the same marker.
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;A\x07").shell_integration,
            ShellIntegration::PromptStart
        );
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;A\x1b\\").shell_integration,
            ShellIntegration::PromptStart
        );
    }

    #[test]
    fn shell_integration_detects_marker_in_surrounding_text() {
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"$ \x1b]133;C\x07 echo hello")
                .shell_integration,
            ShellIntegration::CommandStart
        );
    }

    #[test]
    fn last_command_output_captures_between_b_and_c() {
        let mut proc = OutputProcessor::new();
        proc.process(b"\x1b]133;A\x07$ \x1b]133;B\x07echo hello\x1b]133;C\x07hello");
        assert_eq!(proc.take_last_command_output(), "echo hello");
    }

    #[test]
    fn last_command_output_cross_chunk_and_st_terminator() {
        let mut proc = OutputProcessor::new();
        proc.process(b"\x1b]133;A\x1b\\$ \x1b]133;B\x1b\\ech");
        assert_eq!(
            proc.take_last_command_output(),
            "",
            "capture must stay open across chunk boundaries"
        );
        proc.process(b"o hello\x1b]133;C\x1b\\hello");
        assert_eq!(proc.take_last_command_output(), "echo hello");
    }

    #[test]
    fn last_command_output_take_clears() {
        let mut proc = OutputProcessor::new();
        proc.process(b"\x1b]133;B\x07abc\x1b]133;C\x07");
        assert_eq!(proc.take_last_command_output(), "abc");
        assert_eq!(
            proc.take_last_command_output(),
            "",
            "take must clear the capture"
        );
    }

    #[test]
    fn last_command_output_reset_on_new_prompt() {
        let mut proc = OutputProcessor::new();
        proc.process(b"\x1b]133;B\x07abc\x1b]133;A\x07$ ");
        assert_eq!(
            proc.take_last_command_output(),
            "",
            "a new prompt marker must reset an in-progress capture"
        );
    }

    #[test]
    fn last_command_output_preserves_non_osc133_escapes() {
        let mut proc = OutputProcessor::new();
        proc.process(b"\x1b]133;B\x07\x1b[31mred\x1b]133;C\x07");
        assert_eq!(proc.take_last_command_output(), "\x1b[31mred");
    }

    /// OSC 133 capture is capped at MAX_CAPTURE_BYTES to prevent OOM.
    #[test]
    fn last_command_output_respects_capture_cap() {
        use super::MAX_CAPTURE_BYTES;
        let mut proc = OutputProcessor::new();
        // Start capture with B marker
        proc.process(b"\x1b]133;B");
        // Fill capture_buf with raw text (no OSC133 prefix) past the cap.
        // Between B and C, any non-ESC byte goes directly into capture_buf.
        let payload = vec![b'x'; MAX_CAPTURE_BYTES * 2];
        proc.process(&payload);
        // End capture with C marker
        proc.process(b"\x1b]133;C");
        let output = proc.take_last_command_output();
        assert!(
            output.len() <= MAX_CAPTURE_BYTES,
            "capture_buf exceeded cap: {} > {MAX_CAPTURE_BYTES}",
            output.len()
        );
    }
}
