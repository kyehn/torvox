use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SelectionMode {
    #[default]
    Char,
    Word,
    Line,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionAnchor {
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub start: SelectionAnchor,
    pub end: SelectionAnchor,
    pub mode: SelectionMode,
}

impl Selection {
    pub fn new(start: SelectionAnchor, end: SelectionAnchor, mode: SelectionMode) -> Self {
        Self { start, end, mode }
    }

    pub fn is_ordered(&self) -> bool {
        if self.start.row < self.end.row {
            return true;
        }
        self.start.row == self.end.row && self.start.col <= self.end.col
    }

    pub fn ordered(&self) -> (SelectionAnchor, SelectionAnchor) {
        if self.is_ordered() {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    pub fn contains(&self, row: u32, col: u32) -> bool {
        let (lo, hi) = self.ordered();
        match self.mode {
            SelectionMode::Char | SelectionMode::Word => {
                if row < lo.row || row > hi.row {
                    return false;
                }
                if row == lo.row && row == hi.row {
                    col >= lo.col && col <= hi.col
                } else if row == lo.row {
                    col >= lo.col
                } else if row == hi.row {
                    col <= hi.col
                } else {
                    true
                }
            }
            SelectionMode::Line => row >= lo.row && row <= hi.row,
            SelectionMode::Block => {
                row >= lo.row && row <= hi.row && col >= lo.col && col <= hi.col
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_ordered_same_row() {
        let s = Selection::new(
            SelectionAnchor { row: 5, col: 10 },
            SelectionAnchor { row: 5, col: 3 },
            SelectionMode::Char,
        );
        assert!(!s.is_ordered());
        let (lo, hi) = s.ordered();
        assert_eq!(lo.col, 3);
        assert_eq!(hi.col, 10);
    }

    #[test]
    fn selection_ordered_different_rows() {
        let s = Selection::new(
            SelectionAnchor { row: 3, col: 0 },
            SelectionAnchor { row: 5, col: 10 },
            SelectionMode::Char,
        );
        assert!(s.is_ordered());
    }

    #[test]
    fn char_selection_contains() {
        let s = Selection::new(
            SelectionAnchor { row: 2, col: 5 },
            SelectionAnchor { row: 4, col: 10 },
            SelectionMode::Char,
        );
        assert!(s.contains(2, 5));
        assert!(s.contains(2, 6));
        assert!(s.contains(3, 0));
        assert!(s.contains(4, 10));
        assert!(!s.contains(2, 4));
        assert!(!s.contains(4, 11));
        assert!(!s.contains(1, 0));
        assert!(!s.contains(5, 0));
    }

    #[test]
    fn line_selection_contains() {
        let s = Selection::new(
            SelectionAnchor { row: 2, col: 5 },
            SelectionAnchor { row: 4, col: 10 },
            SelectionMode::Line,
        );
        assert!(s.contains(2, 0));
        assert!(s.contains(3, 0));
        assert!(s.contains(4, 79));
        assert!(!s.contains(1, 0));
        assert!(!s.contains(5, 0));
    }

    #[test]
    fn block_selection_contains() {
        let s = Selection::new(
            SelectionAnchor { row: 2, col: 5 },
            SelectionAnchor { row: 4, col: 10 },
            SelectionMode::Block,
        );
        assert!(s.contains(2, 5));
        assert!(s.contains(3, 7));
        assert!(s.contains(4, 10));
        assert!(!s.contains(2, 4));
        assert!(!s.contains(3, 11));
    }
}
