/// Cursor movement interpreter — validates Ghostty cursor positioning.
///
/// Each test writes a positional sequence and reads back the cursor state,
/// exactly like WezTerm's semantic interpreter layer but driven through
/// Ghostty's engine.
use crate::ghostty_terminal::GhosttyTerminal;

/// A verified cursor operation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: u32,
    pub col: u32,
}

impl CursorPosition {
    pub fn read(t: &GhosttyTerminal) -> Self {
        let snap = t.take_snapshot();
        CursorPosition {
            row: snap.cursor_row,
            col: snap.cursor_col,
        }
    }
}

/// Move cursor absolutely and verify position.
pub fn assert_cup(t: &mut GhosttyTerminal, row: u32, col: u32, exp_row: u32, exp_col: u32) {
    let seq = format!("\x1b[{};{}H", row, col);
    t.vt_write(seq.as_bytes());
    t.flush();
    let position = CursorPosition::read(t);
    assert_eq!(
        position.row, exp_row,
        "CUP {};{}: expected row {}, got {}",
        row, col, exp_row, position.row
    );
    assert_eq!(
        position.col, exp_col,
        "CUP {};{}: expected col {}, got {}",
        row, col, exp_col, position.col
    );
}

/// Move cursor relatively and verify the delta.
pub fn assert_relative(
    t: &mut GhosttyTerminal,
    seq: &[u8],
    delta_row: i32,
    delta_col: i32,
) -> CursorPosition {
    let before = CursorPosition::read(t);
    t.vt_write(seq);
    t.flush();
    let after = CursorPosition::read(t);
    let exp_row = (before.row as i32 + delta_row).max(0) as u32;
    let exp_col = (before.col as i32 + delta_col).max(0) as u32;
    assert_eq!(
        after.row,
        exp_row,
        "{}: expected row {} (was {} + {}), got {}",
        String::from_utf8_lossy(seq),
        exp_row,
        before.row,
        delta_row,
        after.row
    );
    assert_eq!(
        after.col,
        exp_col,
        "{}: expected col {} (was {} + {}), got {}",
        String::from_utf8_lossy(seq),
        exp_col,
        before.col,
        delta_col,
        after.col
    );
    after
}

/// Write text at the current position and assert it appears.
pub fn assert_write_appears(t: &mut GhosttyTerminal, text: &str, exp_row: u32, exp_col: u32) {
    t.vt_write(text.as_bytes());
    t.flush();
    let snap = t.take_snapshot();
    let index = (exp_row * snap.cols + exp_col) as usize;
    let first_char = text.chars().next().unwrap() as u32;
    assert_eq!(
        snap.cells[index].codepoint, first_char,
        "Text '{}' should appear at ({}, {})",
        text, exp_row, exp_col
    );
}

/// Assert that a scroll region restricts cursor movement.
pub fn assert_cursor_in_region(t: &mut GhosttyTerminal, top: u32, bottom: u32) {
    let position = CursorPosition::read(t);
    assert!(
        position.row >= top && position.row <= bottom,
        "Cursor row {} should be in region [{}, {}]",
        position.row,
        top,
        bottom
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty_terminal::GhosttyTerminal;

    fn t() -> GhosttyTerminal {
        GhosttyTerminal::new(10, 30, 100).expect("terminal")
    }

    #[test]
    fn cup_home() {
        assert_cup(&mut t(), 1, 1, 0, 0);
    }

    #[test]
    fn cup_5_10() {
        assert_cup(&mut t(), 5, 10, 4, 9);
    }

    #[test]
    fn cup_clamp_max() {
        assert_cup(&mut t(), 99, 99, 9, 29);
    }

    #[test]
    fn cud_default() {
        let mut terminal = t();
        assert_relative(&mut terminal, b"\x1b[B", 1, 0);
    }

    #[test]
    fn cuu_default() {
        let mut terminal = t();
        terminal.vt_write(b"\x1b[5;1H"); // move to row 5
        terminal.flush();
        assert_relative(&mut terminal, b"\x1b[A", -1, 0);
    }

    #[test]
    fn cuf_5() {
        let mut terminal = t();
        assert_relative(&mut terminal, b"\x1b[5C", 0, 5);
    }

    #[test]
    fn cub_3() {
        let mut terminal = t();
        terminal.vt_write(b"\x1b[10C"); // go to col 10
        terminal.flush();
        assert_relative(&mut terminal, b"\x1b[3D", 0, -3);
    }
}
