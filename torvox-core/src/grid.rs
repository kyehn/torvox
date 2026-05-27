use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::cell::{Cell, DirtyMask};
use crate::line::Line;

/// Read-only snapshot of the terminal grid for rendering.
/// Implemented by Grid so renderer doesn't depend on terminal crate.
pub trait GridSnapshot {
    fn rows(&self) -> u32;
    fn cols(&self) -> u32;
    fn get(&self, row: u32) -> Option<&Line>;
    fn cell(&self, row: u32, col: u32) -> Option<&Cell>;
    fn dirty(&self) -> &DirtyMask;
}

impl GridSnapshot for Grid {
    fn rows(&self) -> u32 {
        self.rows
    }
    fn cols(&self) -> u32 {
        self.cols
    }
    fn get(&self, row: u32) -> Option<&Line> {
        self.lines.get(row as usize)
    }
    fn cell(&self, row: u32, col: u32) -> Option<&Cell> {
        self.lines.get(row as usize).and_then(|line| line.get(col))
    }
    fn dirty(&self) -> &DirtyMask {
        &self.dirty
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    lines: Vec<Line>,
    dirty: DirtyMask,
    rows: u32,
    cols: u32,
    scrollback: Vec<Line>,
    max_scrollback: usize,
}

impl Grid {
    pub fn new(rows: u32, cols: u32) -> Self {
        let lines = (0..rows).map(|_| Line::new(cols)).collect();
        let mut dirty = DirtyMask::new(rows);
        dirty.mark_all(rows);
        Self {
            lines,
            dirty,
            rows,
            cols,
            scrollback: Vec::new(),
            max_scrollback: 50_000,
        }
    }

    pub fn with_scrollback(rows: u32, cols: u32, max_scrollback: usize) -> Self {
        let lines = (0..rows).map(|_| Line::new(cols)).collect();
        let mut dirty = DirtyMask::new(rows);
        dirty.mark_all(rows);
        Self {
            lines,
            dirty,
            rows,
            cols,
            scrollback: Vec::new(),
            max_scrollback,
        }
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn cols(&self) -> u32 {
        self.cols
    }

    pub fn get(&self, row: u32) -> Option<&Line> {
        self.lines.get(row as usize)
    }

    pub fn get_mut(&mut self, row: u32) -> Option<&mut Line> {
        self.dirty.mark(row);
        self.lines.get_mut(row as usize)
    }

    pub fn dirty(&self) -> &DirtyMask {
        &self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty.clear();
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty.mark_all(self.rows);
    }

    pub fn resize(&mut self, new_rows: u32, new_cols: u32) {
        self.lines.resize(new_rows as usize, Line::new(new_cols));
        for line in &mut self.lines {
            line.resize(new_cols);
        }
        self.dirty.resize(new_rows);
        self.dirty.mark_all(new_rows);
        self.rows = new_rows;
        self.cols = new_cols;
    }

    pub fn cell_mut(&mut self, row: u32, col: u32) -> Option<&mut crate::cell::Cell> {
        self.dirty.mark(row);
        self.lines
            .get_mut(row as usize)
            .and_then(|line| line.get_mut(col))
    }

    pub fn cell(&self, row: u32, col: u32) -> Option<&crate::cell::Cell> {
        self.lines.get(row as usize).and_then(|line| line.get(col))
    }

    pub fn mark_row_dirty(&mut self, row: u32) {
        self.dirty.mark(row);
    }

    pub fn mark_rows_dirty(&mut self, start: u32, end: u32) {
        for row in start..end {
            self.dirty.mark(row);
        }
    }

    pub fn scroll_up(&mut self, top: u32, bottom: u32, cols: u32) {
        if top >= bottom || bottom > self.rows {
            return;
        }
        let region_size = bottom - top;
        if region_size <= 1 {
            return;
        }
        let t = top as usize;
        let b = bottom as usize;
        if top == 0 {
            let removed = self.lines.remove(t);
            self.scrollback.push(removed);
            if self.scrollback.len() > self.max_scrollback {
                let excess = self.scrollback.len() - self.max_scrollback;
                self.scrollback.drain(..excess);
            }
            self.lines.insert(b - 1, Line::new(cols));
        } else {
            self.lines[t..b].rotate_left(1);
            *self
                .lines
                .get_mut(b - 1)
                .expect("grid invariant: b-1 < lines.len() after rotate") = Line::new(cols);
        }
        for row in top..bottom {
            self.dirty.mark(row);
        }
    }

    pub fn scroll_down(&mut self, top: u32, bottom: u32, cols: u32) {
        if top >= bottom || bottom > self.rows {
            return;
        }
        let t = top as usize;
        let b = bottom as usize;
        self.lines[t..b].rotate_right(1);
        *self
            .lines
            .get_mut(t)
            .expect("grid invariant: t < lines.len() after rotate") = Line::new(cols);
        for row in top..bottom {
            self.dirty.mark(row);
        }
    }

    pub fn insert_lines(&mut self, at: u32, count: u32, bottom: u32, cols: u32) {
        if at >= bottom || count == 0 {
            return;
        }
        let actual = count.min(bottom - at);
        let a = at as usize;
        let b = bottom as usize;
        // Rotate left: insert blank lines at `at`, push existing lines down
        self.lines[a..b].rotate_right(actual as usize);
        // Fill the inserted lines (now at the beginning of the slice) with blank
        for i in a..a + actual as usize {
            *self
                .lines
                .get_mut(i)
                .expect("grid invariant: i < lines.len() after rotate_right") = Line::new(cols);
        }
        for row in at..bottom {
            self.dirty.mark(row);
        }
    }

    pub fn delete_lines(&mut self, at: u32, count: u32, bottom: u32, cols: u32) {
        if at >= bottom || count == 0 {
            return;
        }
        let actual = count.min(bottom - at);
        let a = at as usize;
        let b = bottom as usize;
        // Rotate left: delete lines at `at`, pull lines up from below
        self.lines[a..b].rotate_left(actual as usize);
        // Fill the deleted lines (now at the end of the slice) with blank
        for i in b - actual as usize..b {
            *self
                .lines
                .get_mut(i)
                .expect("grid invariant: i < lines.len() after rotate_left") = Line::new(cols);
        }
        for row in at..bottom {
            self.dirty.mark(row);
        }
    }

    pub fn clear_cells(&mut self, row: u32, start_col: u32, end_col: u32) {
        if let Some(line) = self.lines.get_mut(row as usize) {
            for col in start_col..end_col.min(line.len()) {
                if let Some(cell) = line.get_mut(col) {
                    *cell = crate::cell::Cell::default();
                }
            }
            self.dirty.mark(row);
        }
    }

    pub fn fill_cells(&mut self, row: u32, ch: char, start_col: u32, end_col: u32) {
        if let Some(line) = self.lines.get_mut(row as usize) {
            for col in start_col..end_col.min(line.len()) {
                if let Some(cell) = line.get_mut(col) {
                    cell.char = ch;
                }
            }
            self.dirty.mark(row);
        }
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub fn scrollback_line(&self, index: usize) -> Option<&Line> {
        self.scrollback.get(index)
    }

    pub fn clear_scrollback(&mut self) {
        self.scrollback.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_new_dimensions() {
        let g = Grid::new(24, 80);
        assert_eq!(g.rows(), 24);
        assert_eq!(g.cols(), 80);
    }

    #[test]
    fn grid_new_all_dirty() {
        let g = Grid::new(3, 5);
        for row in 0..3 {
            assert!(g.dirty().is_dirty(row));
        }
    }

    #[test]
    fn grid_mark_clean() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_get_valid_row() {
        let g = Grid::new(24, 80);
        assert!(g.get(0).is_some());
        assert!(g.get(23).is_some());
    }

    #[test]
    fn grid_get_out_of_bounds() {
        let g = Grid::new(24, 80);
        assert!(g.get(24).is_none());
    }

    #[test]
    fn grid_get_mut_marks_dirty() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        let _ = g.get_mut(1);
        assert!(g.dirty().is_dirty(1));
        assert!(!g.dirty().is_dirty(0));
    }

    #[test]
    fn grid_resize() {
        let mut g = Grid::new(24, 80);
        g.resize(30, 100);
        assert_eq!(g.rows(), 30);
        assert_eq!(g.cols(), 100);
    }

    #[test]
    fn grid_resize_marks_all_dirty() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        g.resize(5, 10);
        assert!(g.dirty().any_dirty());
    }

    #[test]
    fn grid_cell_default() {
        let g = Grid::new(1, 1);
        let cell = g.get(0).unwrap().get(0).unwrap();
        assert_eq!(cell.char, ' ');
    }

    #[test]
    fn grid_set_cell() {
        let mut g = Grid::new(1, 1);
        g.get_mut(0).unwrap().get_mut(0).unwrap().char = 'X';
        assert_eq!(g.get(0).unwrap().get(0).unwrap().char, 'X');
    }

    #[test]
    fn dirty_mask_bit_ops() {
        let mut m = DirtyMask::new(24);
        assert!(!m.any_dirty());
        m.mark(0);
        m.mark(3);
        assert!(m.is_dirty(0));
        assert!(m.is_dirty(3));
        assert!(!m.is_dirty(1));
        m.clear();
        assert!(!m.any_dirty());
        m.mark_all(5);
        for i in 0..5 {
            assert!(m.is_dirty(i));
        }
    }

    #[test]
    fn grid_scrollback_on_scroll_up() {
        let mut g = Grid::new(3, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.fill_cells(1, 'B', 0, 5);
        g.fill_cells(2, 'C', 0, 5);
        g.scroll_up(0, 3, 5);
        assert_eq!(g.scrollback_len(), 1);
        assert_eq!(g.scrollback_line(0).unwrap().get(0).unwrap().char, 'A');
    }

    #[test]
    fn grid_scrollback_no_save_when_not_top() {
        let mut g = Grid::new(4, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.fill_cells(1, 'B', 0, 5);
        g.fill_cells(2, 'C', 0, 5);
        g.fill_cells(3, 'D', 0, 5);
        g.scroll_up(1, 3, 5);
        assert_eq!(g.scrollback_len(), 0);
    }

    #[test]
    fn grid_scrollback_max_limit() {
        let mut g = Grid::with_scrollback(2, 5, 3);
        for i in 0..5 {
            g.fill_cells(0, (b'A' + i) as char, 0, 5);
            g.scroll_up(0, 2, 5);
        }
        assert!(g.scrollback_len() <= 3);
    }

    #[test]
    fn grid_scrollback_clear() {
        let mut g = Grid::new(2, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.scroll_up(0, 2, 5);
        assert_eq!(g.scrollback_len(), 1);
        g.clear_scrollback();
        assert_eq!(g.scrollback_len(), 0);
    }
}
