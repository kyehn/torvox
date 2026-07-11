//! Terminal display line with cell storage and scrollback semantics.
use alloc::boxed::Box;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::cell::Cell;

#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineAttr {
    #[default]
    Normal,
    DoubleWidth,
    DoubleHeightTop,
    DoubleHeightBottom,
}

/// Terminal line: fixed-capacity `Box<[Cell]>` providing stable addresses and small inline `attr`.
/// `Box<[Cell]>` avoids the capacity/length overhead of `Vec`, making it a natural choice
/// for lines whose size is known at construction time.
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
/// A fixed-capacity row of cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    cells: Box<[Cell]>,
    pub attr: LineAttr,
}

/// A fixed-capacity row of cells with a line attribute.
///
/// ```
/// use torvox_core::line::{Line, LineAttr};
/// use torvox_core::cell::Cell;
///
/// let mut line = Line::new(80);
/// assert_eq!(line.len(), 80);
/// assert!(!line.is_empty());
/// assert_eq!(line.attr, LineAttr::Normal);
///
/// line.get_mut(5).unwrap().char = 'H';
/// assert_eq!(line.get(5).unwrap().char, 'H');
///
/// line.resize(100);
/// assert_eq!(line.len(), 100);
/// assert_eq!(line.get(5).unwrap().char, 'H');
/// assert_eq!(line.get(99).unwrap().char, ' ');
/// ```
impl Line {
    pub fn new(cols: u32) -> Self {
        let cells: Box<[Cell]> = (0..cols).map(|_| Cell::default()).collect();
        Self {
            cells,
            attr: LineAttr::Normal,
        }
    }

    pub fn len(&self) -> u32 {
        self.cells.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn get(&self, col: u32) -> Option<&Cell> {
        self.cells.get(col as usize)
    }

    pub fn get_mut(&mut self, col: u32) -> Option<&mut Cell> {
        self.cells.get_mut(col as usize)
    }

    pub fn resize(&mut self, new_cols: u32) {
        let new_len = new_cols as usize;
        if new_len == self.cells.len() {
            return;
        }
        let mut new_cells = Vec::with_capacity(new_len);
        new_cells.resize(new_len, Cell::default());
        let copy_len = new_len.min(self.cells.len());
        new_cells[..copy_len].clone_from_slice(&self.cells[..copy_len]);
        self.cells = new_cells.into_boxed_slice();
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_new_len() {
        let l = Line::new(80);
        assert_eq!(l.len(), 80);
        assert!(!l.is_empty());
    }

    #[test]
    fn line_new_empty() {
        let l = Line::new(0);
        assert_eq!(l.len(), 0);
        assert!(l.is_empty());
    }

    #[test]
    fn line_get_valid() {
        let l = Line::new(5);
        assert!(l.get(0).is_some());
        assert!(l.get(4).is_some());
    }

    #[test]
    fn line_get_out_of_bounds() {
        let l = Line::new(5);
        assert!(l.get(5).is_none());
    }

    #[test]
    fn line_default_cells_are_spaces() {
        let l = Line::new(10);
        for c in l.cells() {
            assert_eq!(c.char, ' ');
        }
    }

    #[test]
    fn line_set_cell() {
        let mut l = Line::new(5);
        l.get_mut(2).unwrap().char = 'A';
        assert_eq!(l.get(2).unwrap().char, 'A');
    }

    #[test]
    fn line_resize_grow() {
        let mut l = Line::new(5);
        l.resize(10);
        assert_eq!(l.len(), 10);
    }

    #[test]
    fn line_resize_shrink() {
        let mut l = Line::new(10);
        l.resize(5);
        assert_eq!(l.len(), 5);
    }

    #[test]
    fn line_resize_new_cells_default() {
        let mut l = Line::new(2);
        l.resize(3);
        assert_eq!(l.get(2).unwrap().char, ' ');
    }

    #[test]
    fn line_resize_preserves_existing_cells() {
        let mut l = Line::new(3);
        l.get_mut(0).unwrap().char = 'A';
        l.get_mut(1).unwrap().char = 'B';
        l.resize(5);
        assert_eq!(l.get(0).unwrap().char, 'A');
        assert_eq!(l.get(1).unwrap().char, 'B');
        assert_eq!(l.get(2).unwrap().char, ' ');
        assert_eq!(l.get(3).unwrap().char, ' ');
    }

    #[test]
    fn line_resize_shrink_keeps_prefix() {
        let mut l = Line::new(5);
        l.get_mut(0).unwrap().char = 'A';
        l.get_mut(1).unwrap().char = 'B';
        l.resize(2);
        assert_eq!(l.len(), 2);
        assert_eq!(l.get(0).unwrap().char, 'A');
        assert_eq!(l.get(1).unwrap().char, 'B');
    }

    #[test]
    fn line_default_attr_is_normal() {
        let l = Line::new(5);
        assert_eq!(l.attr, LineAttr::Normal);
    }

    #[test]
    fn line_set_attr() {
        let mut l = Line::new(5);
        l.attr = LineAttr::DoubleWidth;
        assert_eq!(l.attr, LineAttr::DoubleWidth);
    }

    #[test]
    fn line_resize_same_size_no_op() {
        let mut l = Line::new(5);
        l.get_mut(0).unwrap().char = 'A';
        l.resize(5);
        assert_eq!(l.len(), 5);
        assert_eq!(l.get(0).unwrap().char, 'A');
    }

    #[test]
    fn line_resize_to_zero() {
        let mut l = Line::new(5);
        l.resize(0);
        assert_eq!(l.len(), 0);
        assert!(l.is_empty());
    }

    #[test]
    fn line_resize_from_zero() {
        let mut l = Line::new(0);
        l.resize(5);
        assert_eq!(l.len(), 5);
    }

    #[test]
    fn line_get_mut_modifies() {
        let mut l = Line::new(3);
        let c = l.get_mut(1).unwrap();
        c.char = 'X';
        c.foreground = crate::cell::Color::new(1, 2, 3);
        assert_eq!(l.get(1).unwrap().char, 'X');
        assert_eq!(
            l.get(1).unwrap().foreground,
            crate::cell::Color::new(1, 2, 3)
        );
    }

    #[test]
    fn line_cells_returns_full_slice() {
        let l = Line::new(5);
        let cells: &[crate::cell::Cell] = l.cells();
        assert_eq!(cells.len(), 5);
    }

    #[test]
    fn line_resize_shrink_drops_suffix() {
        let mut l = Line::new(5);
        l.get_mut(3).unwrap().char = 'X';
        l.get_mut(4).unwrap().char = 'Y';
        l.resize(2);
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn line_serde_json_roundtrip() {
        let mut l = Line::new(3);
        l.get_mut(0).unwrap().char = 'A';
        l.get_mut(1).unwrap().char = 'B';
        l.attr = LineAttr::DoubleWidth;
        let json = serde_json::to_string(&l).unwrap();
        let back: Line = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn line_equality() {
        let l1 = Line::new(3);
        let l2 = Line::new(3);
        assert_eq!(l1, l2);
        let mut l3 = Line::new(3);
        l3.get_mut(0).unwrap().char = 'X';
        assert_ne!(l1, l3);
    }

    #[test]
    fn line_clone() {
        let mut l = Line::new(3);
        l.get_mut(0).unwrap().char = 'A';
        let l2 = l.clone();
        assert_eq!(l, l2);
    }

    #[test]
    fn line_attr_variants() {
        let mut l = Line::new(1);
        l.attr = LineAttr::Normal;
        assert_eq!(l.attr, LineAttr::Normal);
        l.attr = LineAttr::DoubleWidth;
        assert_eq!(l.attr, LineAttr::DoubleWidth);
        l.attr = LineAttr::DoubleHeightTop;
        assert_eq!(l.attr, LineAttr::DoubleHeightTop);
        l.attr = LineAttr::DoubleHeightBottom;
        assert_eq!(l.attr, LineAttr::DoubleHeightBottom);
    }

    #[test]
    fn line_new_from_zero_is_empty() {
        let l = Line::new(0);
        assert!(l.is_empty());
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn line_resize_to_zero_empties() {
        let mut l = Line::new(10);
        assert!(!l.is_empty());
        l.resize(0);
        assert!(l.is_empty());
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn line_resize_from_zero_grows() {
        let mut l = Line::new(0);
        l.resize(5);
        assert!(!l.is_empty());
        assert_eq!(l.len(), 5);
        for i in 0..5 {
            assert_eq!(l.get(i).unwrap().char, ' ');
        }
    }

    #[test]
    fn cells_mut_modifies_cells() {
        let mut line = Line::new(5);
        {
            let cells = line.cells_mut();
            cells[0] = Cell {
                char: 'H',
                ..Cell::default()
            };
            cells[1] = Cell {
                char: 'i',
                ..Cell::default()
            };
        }
        assert_eq!(line.cells()[0].char, 'H', "first cell should be H");
        assert_eq!(line.cells()[1].char, 'i', "second cell should be i");
        assert_eq!(
            line.cells()[2].char,
            ' ',
            "third cell should remain default"
        );
    }

    #[test]
    fn cells_mut_write_and_read_back() {
        let mut line = Line::new(3);
        line.cells_mut()[2] = Cell {
            char: 'Z',
            ..Cell::default()
        };
        assert_eq!(
            line.get(2).unwrap().char,
            'Z',
            "cell at index 2 should be Z"
        );
        assert_eq!(
            line.cells_mut().len(),
            3,
            "cells_mut should return full-length slice"
        );
    }

    #[test]
    fn cells_mut_all_cells_accessible() {
        let mut line = Line::new(10);
        for i in 0..10 {
            line.cells_mut()[i] = Cell {
                char: (b'A' + i as u8) as char,
                ..Cell::default()
            };
        }
        for i in 0..10 {
            let expected = (b'A' + i as u8) as char;
            assert_eq!(
                line.cells()[i].char,
                expected,
                "cell {} should be '{}'",
                i,
                expected
            );
        }
    }
}
