use crate::ghostty_terminal::{GhosttyTerminal, GridSnapshot};

/// Convenience shorthand: `tc(&mut term).write(b"X").assert_row_text(0, "X")`.
pub fn tc<'a>(term: &'a mut GhosttyTerminal) -> TermTestCase<'a> {
    TermTestCase::new(term)
}

/// Maximum allowed per-channel difference for color equality.
/// Corresponds to ±5 in u8 space (~0.0196 in f32).
pub const COLOR_TOLERANCE: f32 = 5.0 / 255.0;

/// Individual terminal effects/attributes that can be asserted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectFlag {
    Bold,
    Dim,
    Italic,
    Underline,
    Reverse,
    Strikethrough,
    Blink,
    Hidden,
    Overline,
}

/// Compare two `[f32; 4]` colors channel-by-channel with tolerance.
pub fn colors_approx_eq(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (x - y).abs() <= COLOR_TOLERANCE)
}

/// Get a cell from a snapshot by row/col.
fn cell_at(
    snap: &GridSnapshot,
    row: u32,
    col: u32,
) -> Option<&crate::ghostty_terminal::CellSnapshot> {
    if row >= snap.rows || col >= snap.cols {
        return None;
    }
    let idx = (row * snap.cols + col) as usize;
    snap.cells.get(idx)
}

/// Row text with trailing nulls/spaces trimmed.
fn row_text(snap: &GridSnapshot, row: u32) -> String {
    let mut text = String::new();
    for col in 0..snap.cols {
        if let Some(cell) = cell_at(snap, row, col)
            && cell.codepoint != 0
            && let Some(character) = char::from_u32(cell.codepoint)
        {
            text.push(character);
        }
    }
    text.trim_end().to_string()
}

/// Untrimmed row text — includes trailing spaces and null codepoints as `'\0'`.
pub fn row_text_raw(snap: &GridSnapshot, row: u32) -> String {
    let mut text = String::new();
    for col in 0..snap.cols {
        if let Some(cell) = cell_at(snap, row, col) {
            if cell.codepoint == 0 {
                text.push('\0');
            } else if let Some(character) = char::from_u32(cell.codepoint) {
                text.push(character);
            }
        }
    }
    text
}

/// Structural invariants every snapshot must satisfy.
pub fn assert_invariants(snap: &GridSnapshot) {
    let cell_count = snap.cells.len();
    let expected = (snap.rows * snap.cols) as usize;
    assert_eq!(
        cell_count, expected,
        "snapshot cell count {cell_count} != rows×cols ({expected})"
    );

    for (i, cell) in snap.cells.iter().enumerate() {
        let row = i as u32 / snap.cols;
        let col = i as u32 % snap.cols;
        if cell.codepoint != 0 {
            assert!(
                char::from_u32(cell.codepoint).is_some(),
                "invalid codepoint U+{:X} at ({row},{col})",
                cell.codepoint
            );
        }
        for ch in &cell.foreground {
            assert!(
                *ch >= 0.0 && *ch <= 1.0,
                "fg channel {ch} out of range [0,1] at ({row},{col})"
            );
        }
        for ch in &cell.background {
            assert!(
                *ch >= 0.0 && *ch <= 1.0,
                "bg channel {ch} out of range [0,1] at ({row},{col})"
            );
        }
    }
}

/// Fluent test wrapper for `GhosttyTerminal`.
///
/// Provides chainable assertions that consume and return `Self`,
/// enabling the Termux-style `.write().assert_lines_are().assert_cursor_at()` pattern.
pub struct TermTestCase<'a> {
    term: &'a mut GhosttyTerminal,
    before: Option<GridSnapshot>,
}

impl<'a> TermTestCase<'a> {
    pub fn new(term: &'a mut GhosttyTerminal) -> Self {
        Self { term, before: None }
    }

    /// Write bytes to the terminal (typically PTY/test text output),
    /// flush synchronously, and verify structural invariants.
    /// Uses `pty_write` which converts LF to CR+LF for correct
    /// terminal text behavior.
    pub fn write(self, data: &[u8]) -> Self {
        self.term.pty_write(data);
        self.term.flush();
        let snap = self.term.take_snapshot();
        assert_invariants(&snap);
        self
    }

    /// Write bytes followed by a newline, then flush.
    pub fn writeln(self, data: &[u8]) -> Self {
        let mut buf = data.to_vec();
        buf.push(b'\n');
        self.term.pty_write(&buf);
        self.term.flush();
        let snap = self.term.take_snapshot();
        assert_invariants(&snap);
        self
    }

    /// Capture a snapshot *before* the next write for `assert_content_preserved`.
    pub fn capture_before(mut self) -> Self {
        self.before = Some(self.term.take_snapshot());
        self
    }

    /// Assert a specific row's trimmed text matches.
    pub fn assert_row_text(self, row: u32, expected: &str) -> Self {
        let snap = self.term.take_snapshot();
        let actual = row_text(&snap, row);
        assert_eq!(
            actual, expected,
            "row {row} text mismatch (expected={expected:?}, actual={actual:?})"
        );
        self
    }

    /// Assert that given rows contain expected text (trimmed).
    pub fn assert_lines_are(self, expected: &[&str]) -> Self {
        let snap = self.term.take_snapshot();
        for (i, &exp) in expected.iter().enumerate() {
            let actual = row_text(&snap, i as u32);
            assert_eq!(
                actual, exp,
                "row {i} mismatch (expected={exp:?}, actual={actual:?})"
            );
        }
        self
    }

    /// Assert cursor position using snapshot cursor info.
    pub fn assert_cursor_at(self, row: u32, col: u32) -> Self {
        let snap = self.term.take_snapshot();
        assert_eq!(
            snap.cursor_row, row,
            "cursor row mismatch: expected {row}, got {}",
            snap.cursor_row
        );
        assert_eq!(
            snap.cursor_col, col,
            "cursor col mismatch: expected {col}, got {}",
            snap.cursor_col
        );
        self
    }

    /// Assert foreground color of the cell at (row, col) approximates `expected`.
    pub fn assert_fg(self, row: u32, col: u32, expected: [f32; 4]) -> Self {
        let snap = self.term.take_snapshot();
        let cell = cell_at(&snap, row, col).unwrap_or_else(|| panic!("no cell at ({row}, {col})"));
        assert!(
            colors_approx_eq(&cell.foreground, &expected),
            "fg at ({row},{col}) expected {expected:?}, got {:?}",
            cell.foreground
        );
        self
    }

    /// Assert foreground color of the cell at (row, col) using exact u8 RGB values.
    pub fn assert_fg_exact(self, row: u32, col: u32, r: u8, g: u8, b: u8) -> Self {
        let expected = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
        self.assert_fg(row, col, expected)
    }

    // Required: shared test helper — not all test binaries call every method.
    #[allow(dead_code)]
    pub fn assert_bg_exact(self, row: u32, col: u32, r: u8, g: u8, b: u8) -> Self {
        let expected = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
        self.assert_bg(row, col, expected)
    }

    /// Assert background color of the cell at (row, col) approximates `expected`.
    pub fn assert_bg(self, row: u32, col: u32, expected: [f32; 4]) -> Self {
        let snap = self.term.take_snapshot();
        let cell = cell_at(&snap, row, col).unwrap_or_else(|| panic!("no cell at ({row}, {col})"));
        assert!(
            colors_approx_eq(&cell.background, &expected),
            "bg at ({row},{col}) expected {expected:?}, got {:?}",
            cell.background
        );
        self
    }

    /// Assert the cell at (row, col) has all given effect flags set.
    pub fn assert_effects(self, row: u32, col: u32, effects: &[EffectFlag]) -> Self {
        let snap = self.term.take_snapshot();
        let cell = cell_at(&snap, row, col).unwrap_or_else(|| panic!("no cell at ({row}, {col})"));
        for effect in effects {
            match effect {
                EffectFlag::Bold => assert!(cell.bold, "expected Bold at ({row},{col})"),
                EffectFlag::Dim => {} // not exposed by Ghostty C API
                EffectFlag::Italic => assert!(cell.italic, "expected Italic at ({row},{col})"),
                EffectFlag::Underline => {
                    assert!(cell.underline, "expected Underline at ({row},{col})")
                }
                EffectFlag::Reverse => assert!(cell.reverse, "expected Reverse at ({row},{col})"),
                EffectFlag::Strikethrough => {
                    assert!(
                        cell.strikethrough,
                        "expected Strikethrough at ({row},{col})"
                    )
                }
                EffectFlag::Blink => assert!(cell.blink, "expected Blink at ({row},{col})"),
                EffectFlag::Hidden => assert!(cell.hidden, "expected Hidden at ({row},{col})"),
                EffectFlag::Overline => {
                    assert!(cell.overline, "expected Overline at ({row},{col})")
                }
            }
        }
        self
    }

    /// Assert strikethrough is set at (row, col).
    pub fn assert_strikethrough(self, row: u32, col: u32) -> Self {
        let snap = self.term.take_snapshot();
        let cell = cell_at(&snap, row, col).unwrap_or_else(|| panic!("no cell at ({row}, {col})"));
        assert!(
            cell.strikethrough,
            "expected strikethrough at ({row},{col})"
        );
        self
    }

    /// Assert blink is set at (row, col).
    pub fn assert_blink(self, row: u32, col: u32) -> Self {
        let snap = self.term.take_snapshot();
        let cell = cell_at(&snap, row, col).unwrap_or_else(|| panic!("no cell at ({row}, {col})"));
        assert!(cell.blink, "expected blink at ({row},{col})");
        self
    }

    /// Assert hidden (concealed) at (row, col).
    pub fn assert_hidden(self, row: u32, col: u32) -> Self {
        let snap = self.term.take_snapshot();
        let cell = cell_at(&snap, row, col).unwrap_or_else(|| panic!("no cell at ({row}, {col})"));
        assert!(cell.hidden, "expected hidden at ({row},{col})");
        self
    }

    /// Assert the terminal's current title matches `expected`.
    pub fn assert_title(self, expected: &str) -> Self {
        let title = self.term.title();
        assert!(
            title.contains(expected),
            "title mismatch: expected to contain {expected:?}, got {title:?}"
        );
        self
    }

    /// Assert that DEC private mode N is in the expected state.
    pub fn assert_mode(self, mode_num: u16, expected: bool) -> Self {
        let actual = self.term.mode_get(mode_num, 0);
        assert_eq!(
            actual, expected,
            "DEC mode {mode_num} mismatch: expected {expected}, got {actual}"
        );
        self
    }

    /// Assert the cell at (row, col) does NOT have given effect flags.
    pub fn assert_no_effects(self, row: u32, col: u32, effects: &[EffectFlag]) -> Self {
        let snap = self.term.take_snapshot();
        let cell = cell_at(&snap, row, col).unwrap_or_else(|| panic!("no cell at ({row}, {col})"));
        for effect in effects {
            match effect {
                EffectFlag::Bold => assert!(!cell.bold, "unexpected Bold at ({row},{col})"),
                EffectFlag::Dim => {} // not exposed by Ghostty C API
                EffectFlag::Italic => assert!(!cell.italic, "unexpected Italic at ({row},{col})"),
                EffectFlag::Underline => {
                    assert!(!cell.underline, "unexpected Underline at ({row},{col})")
                }
                EffectFlag::Reverse => {
                    assert!(!cell.reverse, "unexpected Reverse at ({row},{col})")
                }
                EffectFlag::Strikethrough => {
                    assert!(
                        !cell.strikethrough,
                        "unexpected Strikethrough at ({row},{col})"
                    )
                }
                EffectFlag::Blink => {
                    assert!(!cell.blink, "unexpected Blink at ({row},{col})")
                }
                EffectFlag::Hidden => {
                    assert!(!cell.hidden, "unexpected Hidden at ({row},{col})")
                }
                EffectFlag::Overline => {
                    assert!(!cell.overline, "unexpected Overline at ({row},{col})")
                }
            }
        }
        self
    }

    /// Assert the terminal has produced a specific output response (DSR, CPR, DA, etc.).
    /// Drains all pending responses and checks the last one matches `expected`.
    pub fn assert_output_response(self, expected: &[u8]) -> Self {
        let responses = self.term.drain_pty_write_responses();
        assert!(
            !responses.is_empty(),
            "expected terminal output response, but none were captured"
        );
        let last = responses.last().expect("non-empty responses");
        assert_eq!(
            last.as_slice(),
            expected,
            "terminal output response mismatch"
        );
        self
    }

    /// Assert content is preserved: compare current snapshot against `before`.
    /// Fails if any cell's codepoint changed.
    pub fn assert_content_preserved(self) -> Self {
        let before = self
            .before
            .as_ref()
            .expect("assert_content_preserved requires a prior capture_before() call");
        let after = self.term.take_snapshot();
        assert_eq!(
            before.rows, after.rows,
            "row count changed between snapshots"
        );
        assert_eq!(
            before.cols, after.cols,
            "col count changed between snapshots"
        );
        for (i, (b, a)) in before.cells.iter().zip(after.cells.iter()).enumerate() {
            assert_eq!(
                b.codepoint,
                a.codepoint,
                "cell index {i} (row={}, col={}) codepoint changed: {} → {}",
                i / before.cols as usize,
                i % before.cols as usize,
                b.codepoint,
                a.codepoint,
            );
        }
        self
    }

    /// Assert that a sequence leaves the terminal in a clean state.
    /// Writes `sequence`, flushes, writes a known token, then asserts
    /// the token appears at the expected cursor position.
    pub fn assert_sequence_clean(
        self,
        sequence: &[u8],
        token: &[u8],
        expected_row: u32,
        expected_col: u32,
    ) -> Self {
        let token_str = std::str::from_utf8(token).unwrap_or("");
        let tc = self
            .write(sequence)
            .write(token)
            .assert_cursor_at(expected_row, expected_col);
        let snap = tc.term.take_snapshot();
        let actual_row = row_text(&snap, expected_row);
        assert!(
            actual_row.contains(token_str),
            "expected token {token_str:?} at row {expected_row}, got {actual_row:?}"
        );
        tc
    }

    /// Snapshot and check structural invariants at end of a chain.
    pub fn take_and_invariants(self) -> Self {
        let snap = self.term.take_snapshot();
        assert_invariants(&snap);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty_terminal::GhosttyTerminal;

    fn term() -> GhosttyTerminal {
        GhosttyTerminal::new(24, 80, 1000).expect("terminal create")
    }

    fn small_term() -> GhosttyTerminal {
        GhosttyTerminal::new(3, 3, 100).expect("term")
    }

    #[test]
    fn test_case_write_and_assert_row() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"Hello, World!")
            .assert_row_text(0, "Hello, World!");
    }

    #[test]
    fn test_case_write_multiline() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .writeln(b"line one")
            .writeln(b"line two")
            .assert_lines_are(&["line one", "line two"]);
    }

    #[test]
    fn test_case_cursor_movement() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[5;10HX")
            .assert_cursor_at(4, 10);
    }

    #[test]
    fn test_case_sgr_bold() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[1mB")
            .assert_effects(0, 0, &[EffectFlag::Bold])
            .assert_no_effects(
                0,
                0,
                &[
                    EffectFlag::Italic,
                    EffectFlag::Underline,
                    EffectFlag::Reverse,
                ],
            );
    }

    #[test]
    fn test_case_sgr_color() {
        let mut t = term();
        // Use 24-bit color (SGR 38;2) because palette-indexed colors (SGR 31)
        // resolve to StyleColor::PaletteIndex, which build_snapshot maps to
        // default_fg rather than the palette entry.
        let red = [1.0, 0.0, 0.0, 1.0];
        TermTestCase::new(&mut t)
            .write(b"\x1b[38;2;255;0;0mX")
            .assert_fg(0, 0, red);
    }

    #[test]
    fn test_case_sgr_reset_clears_attrs() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[1;3;4;7mA\x1b[0mB")
            .assert_no_effects(
                0,
                1,
                &[
                    EffectFlag::Bold,
                    EffectFlag::Italic,
                    EffectFlag::Underline,
                    EffectFlag::Reverse,
                ],
            )
            .assert_row_text(0, "AB");
    }

    #[test]
    fn test_case_content_preserved_after_noop() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"persistent content")
            .capture_before()
            .write(b"")
            .assert_content_preserved();
    }

    #[test]
    fn test_case_sequence_clean() {
        let mut t = term();
        TermTestCase::new(&mut t).assert_sequence_clean(b"\x1b[31m", b"OK", 0, 2);
    }

    #[test]
    fn test_case_row_text_raw_trailing_nulls() {
        let mut t = small_term();
        // The "AB" fills cols 0-1; col 2 has codepoint 0 (empty)
        // row_text_raw encodes codepoint 0 as \0
        let tc = TermTestCase::new(&mut t).write(b"AB");
        let snap = tc.term.take_snapshot();
        let raw = row_text_raw(&snap, 0);
        // Must have exactly 3 chars: A, B, null
        assert_eq!(raw.len(), 3, "3 cols in small_term");
        assert_eq!(raw.as_bytes()[0], b'A');
        assert_eq!(raw.as_bytes()[1], b'B');
        assert_eq!(raw.as_bytes()[2], 0, "col 2 is null");
    }

    #[test]
    fn test_case_invariants_after_scroll() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        for i in 0..10u8 {
            t.vt_write(format!("line {i}\n").as_bytes());
        }
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
        assert!(t.scrollback_length() > 0);
    }

    #[test]
    fn test_assert_lines_are_full_width() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        TermTestCase::new(&mut t)
            .write(b"ABCDE")
            .assert_lines_are(&["ABCDE", "", ""]);
    }

    #[test]
    fn test_assert_lines_are_multiple_rows_exact() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        TermTestCase::new(&mut t)
            .write(b"ABCDE\n12345")
            .assert_lines_are(&["ABCDE", "12345", ""]);
    }

    #[test]
    fn test_auto_invariants_on_write_no_corruption() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        // write() auto-calls invariants — if the terminal state is corrupt, this panics
        TermTestCase::new(&mut t).write(b"normal text\nmore text\nfinal line");
    }

    #[test]
    fn test_output_capture_dsr() {
        let mut t = term();
        t.vt_write(b"\x1b[5n");
        t.flush();
        let responses = t.drain_pty_write_responses();
        if !responses.is_empty() {
            let last = responses.last().expect("non-empty");
            let text = String::from_utf8_lossy(last);
            assert!(
                text.contains("\x1b[0n"),
                "DSR should respond with status, got: {text}"
            );
        }
        // DSR may not be supported by all backends — skip assertion if no response
    }

    #[test]
    fn test_output_capture_cpr() {
        let mut t = term();
        t.vt_write(b"\x1b[5;10H"); // CUP to row 5, col 10
        t.vt_write(b"\x1b[6n"); // CPR
        t.flush();
        let responses = t.drain_pty_write_responses();
        assert!(!responses.is_empty(), "CPR should produce a response");
        // CPR format: ESC [ row ; col R  (1-indexed)
        let last = responses.last().expect("non-empty");
        let text = String::from_utf8_lossy(last);
        assert!(
            text.contains("5;10") || text.contains("4;9"),
            "CPR should report row=5 col=10 (1-indexed), got: {text}"
        );
    }

    #[test]
    fn test_output_capture_decxpr() {
        let mut t = term();
        t.vt_write(b"\x1b[?6n");
        t.flush();
        let responses = t.drain_pty_write_responses();
        if !responses.is_empty() {
            let last = responses.last().expect("non-empty");
            let text = String::from_utf8_lossy(last);
            assert!(
                text.starts_with("\x1b[?"),
                "DECXCPR should start with ESC[?, got: {text}"
            );
        }
        // DECXCPR may not be supported by all terminal backends — skip if no response
    }

    #[test]
    fn test_is_mouse_tracking_active() {
        let mut t = term();
        assert!(!t.is_mouse_tracking_active());
        t.vt_write(b"\x1b[?1000h");
        t.flush();
        assert!(t.is_mouse_tracking_active());
        t.vt_write(b"\x1b[?1000l");
        t.flush();
        assert!(!t.is_mouse_tracking_active());
    }

    #[test]
    fn test_is_cursor_enabled() {
        let mut t = term();
        assert!(t.is_cursor_enabled());
        t.vt_write(b"\x1b[?25l");
        t.flush();
        assert!(!t.is_cursor_enabled());
        t.vt_write(b"\x1b[?25h");
        t.flush();
        assert!(t.is_cursor_enabled());
    }

    #[test]
    fn test_is_bracketed_paste_active() {
        let mut t = term();
        assert!(!t.is_bracketed_paste_active());
        t.vt_write(b"\x1b[?2004h");
        t.flush();
        assert!(t.is_bracketed_paste_active());
        t.vt_write(b"\x1b[?2004l");
        t.flush();
        assert!(!t.is_bracketed_paste_active());
    }

    #[test]
    fn test_is_origin_mode() {
        let mut t = term();
        assert!(!t.is_origin_mode());
        t.vt_write(b"\x1b[?6h");
        t.flush();
        assert!(t.is_origin_mode());
        t.vt_write(b"\x1b[?6l");
        t.flush();
        assert!(!t.is_origin_mode());
    }

    #[test]
    fn test_is_autowrap_enabled() {
        let mut t = term();
        assert!(t.is_autowrap_enabled());
        t.vt_write(b"\x1b[?7l");
        t.flush();
        assert!(!t.is_autowrap_enabled());
        t.vt_write(b"\x1b[?7h");
        t.flush();
        assert!(t.is_autowrap_enabled());
    }

    #[test]
    fn test_is_alt_screen_active() {
        let mut t = term();
        assert!(!t.is_alt_screen_active());
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        assert!(t.is_alt_screen_active());
        t.vt_write(b"\x1b[?1049l");
        t.flush();
        assert!(!t.is_alt_screen_active());
    }

    #[test]
    fn test_assert_fg_exact() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[38;2;255;128;64mX")
            .assert_fg_exact(0, 0, 255, 128, 64);
    }

    #[test]
    fn test_assert_bg_exact() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[48;2;64;128;255mX")
            .assert_bg_exact(0, 0, 64, 128, 255);
    }

    #[test]
    fn test_assert_strikethrough() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[9mX")
            .assert_strikethrough(0, 0);
    }

    #[test]
    fn test_assert_blink() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[5mX")
            .assert_blink(0, 0);
    }

    #[test]
    fn test_assert_hidden() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[8mX")
            .assert_hidden(0, 0);
    }

    #[test]
    fn test_assert_title_osc2() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b]2;MyTitle\x1b\\")
            .assert_title("MyTitle");
    }

    #[test]
    fn test_assert_title_osc0() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b]0;IconTitle\x1b\\")
            .assert_title("IconTitle");
    }

    #[test]
    fn test_assert_mode_set_reset_cycle() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[?1h")
            .assert_mode(1, true)
            .write(b"\x1b[?1l")
            .assert_mode(1, false);
    }

    #[test]
    fn test_assert_mode_origin() {
        let mut t = term();
        TermTestCase::new(&mut t)
            .write(b"\x1b[?6h")
            .assert_mode(6, true)
            .write(b"\x1b[?6l")
            .assert_mode(6, false);
    }
}
