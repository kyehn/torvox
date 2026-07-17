//! Cursor style and state definitions.
//!
//! # Requirements
//! - [FR-006](crate) — Cursor: DECTCEM visible/hidden, style
use serde::{Deserialize, Serialize};

/// Visual style of the text cursor in the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum CursorStyle {
    /// Full-cell block cursor that highlights the current character.
    #[default]
    Block,
    /// Underline cursor drawn below the current character.
    Underline,
    /// Vertical bar cursor drawn at the left edge of the current character.
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct CursorState {
    /// Current cursor row position (0-indexed).
    pub row: u32,
    /// Current cursor column position (0-indexed).
    pub col: u32,
    /// Visual style of the cursor (Block, Underline, Bar, etc.).
    pub style: CursorStyle,
    /// Whether the cursor is visible.
    pub visible: bool,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            style: CursorStyle::default(),
            visible: true,
        }
    }
}

/// Terminal cursor position and style.
///
/// ```
/// use torvox_core::cursor::{CursorState, CursorStyle};
///
/// let mut c = CursorState::new(10, 20);
/// assert_eq!(c.row, 10);
/// assert_eq!(c.col, 20);
/// assert_eq!(c.style, CursorStyle::Block);
/// assert!(c.visible);
///
/// c.move_up(3);
/// assert_eq!(c.row, 7);
///
/// c.move_down(5, 24);
/// assert_eq!(c.row, 12);
///
/// c.move_left(4);
/// assert_eq!(c.col, 16);
///
/// c.move_right(10, 80);
/// assert_eq!(c.col, 26);
///
/// c.carriage_return();
/// assert_eq!(c.col, 0);
///
/// c.move_to(50, 100);
/// c.clamp(24, 80);
/// assert_eq!(c.row, 23);
/// assert_eq!(c.col, 79);
/// ```
impl CursorState {
    /// Create a new cursor at the given row and column with default style and visibility.
    pub fn new(row: u32, col: u32) -> Self {
        Self {
            row,
            col,
            ..Default::default()
        }
    }

    /// Move the cursor to the specified row and column.
    pub fn move_to(&mut self, row: u32, col: u32) {
        self.row = row;
        self.col = col;
    }

    /// Move the cursor up by `n` rows, clamping at row 0.
    pub fn move_up(&mut self, n: u32) {
        self.row = self.row.saturating_sub(n);
    }

    /// Move the cursor down by `n` rows, clamping at `max_rows - 1`.
    pub fn move_down(&mut self, n: u32, max_rows: u32) {
        self.row = self.row.saturating_add(n).min(max_rows.saturating_sub(1));
    }

    /// Move the cursor left by `n` columns, clamping at column 0.
    pub fn move_left(&mut self, n: u32) {
        self.col = self.col.saturating_sub(n);
    }

    /// Move the cursor right by `n` columns, clamping at `max_cols - 1`.
    pub fn move_right(&mut self, n: u32, max_cols: u32) {
        self.col = self.col.saturating_add(n).min(max_cols.saturating_sub(1));
    }

    /// Reset the cursor column to 0 (carriage return).
    pub fn carriage_return(&mut self) {
        self.col = 0;
    }

    /// Clamp the cursor position so it does not exceed the grid bounds.
    pub fn clamp(&mut self, max_rows: u32, max_cols: u32) {
        self.row = self.row.min(max_rows.saturating_sub(1));
        self.col = self.col.min(max_cols.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn cursor_new_matches_default_except_position() {
        let from_new = CursorState::new(10, 20);
        let from_default = CursorState {
            row: 10,
            col: 20,
            ..CursorState::default()
        };
        assert_eq!(from_new, from_default);
        assert_eq!(from_new.style, CursorStyle::Block);
        assert!(from_new.visible);
    }

    #[test]
    fn move_cursor() {
        let mut c = CursorState::new(5, 10);
        c.move_to(3, 7);
        assert_eq!(c.row, 3);
        assert_eq!(c.col, 7);
    }

    #[test]
    fn carriage_return() {
        let mut c = CursorState::new(5, 42);
        c.carriage_return();
        assert_eq!(c.col, 0);
        assert_eq!(c.row, 5);
    }

    #[test]
    fn cursor_style_affects_equality() {
        let a = CursorState {
            style: CursorStyle::Block,
            ..CursorState::default()
        };
        let b = CursorState {
            style: CursorStyle::Underline,
            ..CursorState::default()
        };
        assert_ne!(a, b, "different styles should not be equal");
    }

    #[test]
    fn clamp_reduces_position_but_preserves_style() {
        let mut c = CursorState {
            row: 100,
            col: 200,
            style: CursorStyle::Bar,
            visible: false,
        };
        c.clamp(10, 20);
        assert_eq!(c.row, 9, "row should clamp to max_rows-1");
        assert_eq!(c.col, 19, "col should clamp to max_cols-1");
        assert_eq!(c.style, CursorStyle::Bar, "style must survive clamp");
        assert!(!c.visible, "visibility must survive clamp");
    }

    #[test]
    fn cursor_move_to_same_position() {
        let mut c = CursorState::new(5, 10);
        c.move_to(5, 10);
        assert_eq!(c.row, 5);
        assert_eq!(c.col, 10);
    }

    #[test]
    fn cursor_move_up_test() {
        let mut c;

        c = CursorState::new(5, 0);
        c.move_up(0);
        assert_eq!(c.row, 5, "zero");
        c = CursorState::new(10, 0);
        c.move_up(3);
        assert_eq!(c.row, 7, "partial");
        c = CursorState::new(2, 0);
        c.move_up(5);
        assert_eq!(c.row, 0, "clamp");
    }

    #[test]
    fn cursor_move_down_test() {
        let mut c;

        c = CursorState::new(5, 0);
        c.move_down(5, 0);
        assert_eq!(c.row, 0, "zero_max");
        c = CursorState::new(22, 0);
        c.move_down(5, 24);
        assert_eq!(c.row, 23, "clamp_max");
        c = CursorState::new(0, 0);
        c.move_down(100, 24);
        assert_eq!(c.row, 23, "overflow");
    }

    #[test]
    fn cursor_move_left_test() {
        let mut c;

        c = CursorState::new(0, 5);
        c.move_left(0);
        assert_eq!(c.col, 5, "zero");
        c = CursorState::new(0, 10);
        c.move_left(3);
        assert_eq!(c.col, 7, "partial");
        c = CursorState::new(0, 3);
        c.move_left(10);
        assert_eq!(c.col, 0, "clamp");
    }

    #[test]
    fn cursor_move_right_test() {
        let mut c;

        c = CursorState::new(0, 5);
        c.move_right(5, 0);
        assert_eq!(c.col, 0, "zero_max");
        c = CursorState::new(0, 76);
        c.move_right(10, 80);
        assert_eq!(c.col, 79, "clamp_max");
        c = CursorState::new(0, 0);
        c.move_right(100, 24);
        assert_eq!(c.col, 23, "overflow");
    }

    #[test]
    fn cursor_equality_compares_all_fields() {
        let a = CursorState {
            row: 1,
            col: 2,
            style: CursorStyle::Block,
            visible: true,
        };
        let b = CursorState {
            row: 1,
            col: 2,
            style: CursorStyle::Bar,
            visible: false,
        };
        assert_ne!(a, b, "different style+visible should make cursors unequal");
        let c = CursorState {
            row: 1,
            col: 2,
            style: CursorStyle::Block,
            visible: true,
        };
        assert_eq!(a, c, "identical cursors should be equal");
    }

    #[test]
    fn cursor_inequality_by_position() {
        let a = CursorState::new(1, 2);
        let b = CursorState::new(1, 3);
        assert_ne!(a, b);
    }

    #[test]
    fn clamp_at_zero_row_is_idempotent() {
        let mut c = CursorState::new(0, 0);
        c.clamp(10, 10);
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn clamp_saturates_on_max_rows_zero() {
        let mut c = CursorState::new(5, 5);
        c.clamp(0, 0);
        assert_eq!(c.row, 0, "max_rows=0 should saturate to 0");
        assert_eq!(c.col, 0, "max_cols=0 should saturate to 0");
    }

    #[test]
    fn cursor_style_serde() {
        for style in [CursorStyle::Block, CursorStyle::Underline, CursorStyle::Bar] {
            let json = serde_json::to_string(&style).unwrap();
            let back: CursorStyle = serde_json::from_str(&json).unwrap();
            assert_eq!(style, back);
        }
    }

    #[test]
    fn cursor_state_serde_roundtrip() {
        let c = CursorState {
            row: 5,
            col: 10,
            style: CursorStyle::Bar,
            visible: false,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CursorState = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn cursor_state_clone_independence() {
        let mut original = CursorState::new(1, 2);
        let cloned = original;
        original.move_to(99, 99);
        assert_eq!(cloned.row, 1, "Copy should snapshot position");
        assert_eq!(cloned.col, 2);
    }

    #[test]
    fn move_down_saturating_no_panic() {
        let mut pos = CursorState::new(u32::MAX - 5, 0);
        pos.move_down(u32::MAX, 24);
        assert_eq!(pos.row, 23, "saturating_add prevents overflow panic");
    }

    #[test]
    fn move_right_saturating_no_panic() {
        let mut pos = CursorState::new(0, u32::MAX - 5);
        pos.move_right(u32::MAX, 80);
        assert_eq!(pos.col, 79, "saturating_add prevents overflow panic");
    }

    #[test]
    fn cursor_style_default_is_block() {
        let style = CursorStyle::default();
        assert_eq!(style, CursorStyle::Block);
    }

    #[test]
    fn cursor_style_debug_not_empty() {
        let styles = [CursorStyle::Block, CursorStyle::Underline, CursorStyle::Bar];
        for style in &styles {
            let debug = format!("{:?}", style);
            assert!(
                !debug.is_empty(),
                "CursorStyle {:?} should have non-empty debug",
                style
            );
        }
    }

    #[test]
    fn move_to_preserves_style_and_visibility() {
        let mut c = CursorState {
            row: 5,
            col: 10,
            style: CursorStyle::Bar,
            visible: false,
        };
        c.move_to(20, 30);
        assert_eq!(c.row, 20, "row should update");
        assert_eq!(c.col, 30, "col should update");
        assert_eq!(c.style, CursorStyle::Bar, "style must survive move_to");
        assert!(!c.visible, "visibility must survive move_to");
    }

    #[test]
    fn move_up_preserves_col() {
        let mut c = CursorState::new(10, 42);
        c.move_up(3);
        assert_eq!(c.row, 7, "row should decrease");
        assert_eq!(c.col, 42, "col must survive move_up");
        assert!(c.visible, "visible must survive move_up");
        assert_eq!(c.style, CursorStyle::Block, "style must survive move_up");
    }

    #[test]
    fn move_down_preserves_style_and_visibility() {
        let mut c = CursorState {
            row: 0,
            col: 15,
            style: CursorStyle::Underline,
            visible: false,
        };
        c.move_down(5, 24);
        assert_eq!(c.row, 5, "row should increase");
        assert_eq!(c.col, 15, "col must survive move_down");
        assert_eq!(
            c.style,
            CursorStyle::Underline,
            "style must survive move_down"
        );
        assert!(!c.visible, "visibility must survive move_down");
    }

    #[test]
    fn carriage_return_preserves_style_and_visibility() {
        let mut c = CursorState {
            row: 7,
            col: 42,
            style: CursorStyle::Bar,
            visible: false,
        };
        c.carriage_return();
        assert_eq!(c.row, 7, "carriage_return should not change row");
        assert_eq!(c.col, 0, "carriage_return should reset col to 0");
        assert_eq!(
            c.style,
            CursorStyle::Bar,
            "style must survive carriage_return"
        );
        assert!(!c.visible, "visibility must survive carriage_return");
    }
}
