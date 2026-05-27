use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CursorStyle {
    #[default]
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    pub row: u32,
    pub col: u32,
    pub style: CursorStyle,
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

impl CursorState {
    pub fn new(row: u32, col: u32) -> Self {
        Self {
            row,
            col,
            ..Default::default()
        }
    }

    pub fn move_to(&mut self, row: u32, col: u32) {
        self.row = row;
        self.col = col;
    }

    pub fn move_up(&mut self, n: u32) {
        self.row = self.row.saturating_sub(n);
    }

    pub fn move_down(&mut self, n: u32, max_rows: u32) {
        self.row = (self.row + n).min(max_rows.saturating_sub(1));
    }

    pub fn move_left(&mut self, n: u32) {
        self.col = self.col.saturating_sub(n);
    }

    pub fn move_right(&mut self, n: u32, max_cols: u32) {
        self.col = (self.col + n).min(max_cols.saturating_sub(1));
    }

    pub fn carriage_return(&mut self) {
        self.col = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cursor() {
        let c = CursorState::default();
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
        assert_eq!(c.style, CursorStyle::Block);
        assert!(c.visible);
    }

    #[test]
    fn move_cursor() {
        let mut c = CursorState::new(5, 10);
        c.move_to(3, 7);
        assert_eq!(c.row, 3);
        assert_eq!(c.col, 7);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let mut c = CursorState::new(2, 0);
        c.move_up(5);
        assert_eq!(c.row, 0);
    }

    #[test]
    fn move_down_clamps_at_max() {
        let mut c = CursorState::new(22, 0);
        c.move_down(5, 24);
        assert_eq!(c.row, 23);
    }

    #[test]
    fn move_left_clamps_at_zero() {
        let mut c = CursorState::new(0, 3);
        c.move_left(10);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn move_right_clamps_at_max() {
        let mut c = CursorState::new(0, 76);
        c.move_right(10, 80);
        assert_eq!(c.col, 79);
    }

    #[test]
    fn carriage_return() {
        let mut c = CursorState::new(5, 42);
        c.carriage_return();
        assert_eq!(c.col, 0);
        assert_eq!(c.row, 5);
    }
}
