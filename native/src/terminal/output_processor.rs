//! Output processor — extracts OSC events from raw PTY output before
//! passing to the VT emulator.
//!
//! # Requirements
//! - FR-033 — Scrollback: terminal content
//!
//! # Responsibilities
//! - Decode OSC 52/7/8 events into structured types
//! - Produce filtered output for the VT parser
//!
//! Extracted from `Session::process_output` to improve locality and testability.

use crate::terminal::osc_handler::{OscEvent, OscHandler};
use std::sync::atomic::{AtomicBool, Ordering};

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
    /// Filtered output bytes for the VT parser.
    pub filtered: Vec<u8>,
}

/// Processes raw PTY output, extracting events and producing filtered bytes.
pub struct OutputProcessor {
    osc_handler: OscHandler,
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
            new_output: AtomicBool::new(false),
        }
    }

    /// Take and clear the `new_output` flag (single-consumer read-clear;
    /// the render thread is the only reader). Returns true if any PTY
    /// output was ingested since the last take.
    pub fn take_new_output(&self) -> bool {
        self.new_output.swap(false, Ordering::AcqRel)
    }

    /// Process a raw output chunk and return a snapshot of decoded events.
    pub fn process(&mut self, data: &[u8]) -> OutputSnapshot {
        // P1-1: PTY ingest → raise `new_output` (bypass flag, not a queued
        // event — see docs/reference/dual-flag-protocol.md). Empty chunks
        // carry no output and do not count.
        if !data.is_empty() {
            self.new_output.store(true, Ordering::Release);
        }
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
            }
        }

        let filtered = self.osc_handler.output();
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
    fn osc52_clipboard_read_forwarded() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]52;c;?\x07");
        assert_eq!(snap.clipboard_read.as_deref(), Some("c"));
        assert!(
            snap.filtered.is_empty(),
            "read request must not reach the VT parser"
        );
    }
}
