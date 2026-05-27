use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::cell::Cell;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    cells: Vec<Cell>,
}

impl Line {
    pub fn new(cols: u32) -> Self {
        Self {
            cells: alloc::vec![Cell::default(); cols as usize],
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
        self.cells.resize(new_cols as usize, Cell::default());
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
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
}
