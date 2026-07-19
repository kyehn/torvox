//! Terminal grid buffer — rows, columns, scrolling, dirty tracking.
//!
//! # Requirements
//! - [FR-005](crate) — Alt-screen buffer
//! - [FR-007](crate) — Scrollback buffer with configurable limit
//! - [FR-029](crate) — Scrollback: scroll up
//! - [FR-030](crate) — Scrollback: scroll down
//! - [FR-032](crate) — Scrollback: search
//! - [NFR-008](crate) — Memory: resize with O(1) cell moves
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::cell::{Attrs, Cell, Color, DirtyMask};
use crate::line::Line;

const DEFAULT_MAX_SCROLLBACK: usize = 50_000;

/// Read-only snapshot interface for the terminal grid.
/// Provides a restricted view (rows, cols, cell access, dirty mask)
/// so consumers read through a stable API rather than depending on Grid internals.
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

/// Terminal grid buffer — rows, columns, scrolling, and dirty tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct Grid {
    lines: Vec<Line>,
    dirty: DirtyMask,
    rows: u32,
    cols: u32,
    scrollback: VecDeque<Line>,
    max_scrollback: usize,
    alt_screen: bool,
}

impl Grid {
    /// Create a new grid with the given dimensions.
    ///
    /// ```
    /// use torvox_core::grid::Grid;
    ///
    /// let grid = Grid::new(24, 80);
    /// assert_eq!(grid.rows(), 24);
    /// assert_eq!(grid.cols(), 80);
    /// assert!(grid.dirty().any_dirty());
    /// assert_eq!(grid.scrollback_length(), 0);
    /// ```
    pub fn new(rows: u32, cols: u32) -> Self {
        let lines = (0..rows).map(|_| Line::new(cols)).collect();
        let mut dirty = DirtyMask::new(rows);
        dirty.mark_all(rows);
        Self {
            lines,
            dirty,
            rows,
            cols,
            scrollback: VecDeque::new(),
            max_scrollback: DEFAULT_MAX_SCROLLBACK,
            alt_screen: false,
        }
    }

    /// Create a new grid with explicit max scrollback size.
    pub fn with_scrollback(rows: u32, cols: u32, max_scrollback: usize) -> Self {
        let lines = (0..rows).map(|_| Line::new(cols)).collect();
        let mut dirty = DirtyMask::new(rows);
        dirty.mark_all(rows);
        Self {
            lines,
            dirty,
            rows,
            cols,
            scrollback: VecDeque::new(),
            max_scrollback,
            alt_screen: false,
        }
    }

    /// Return the number of rows in the grid.
    pub fn rows(&self) -> u32 {
        self.rows
    }

    /// Return the number of columns in the grid.
    pub fn cols(&self) -> u32 {
        self.cols
    }

    /// Borrow a line by row index.
    pub fn get(&self, row: u32) -> Option<&Line> {
        self.lines.get(row as usize)
    }

    /// Returns a cell slice for `row` (zero allocation, one Option layer of indirection).
    /// Faster than `get(row).map(|l| l.cells())`, suitable for render hot paths.
    pub fn row_cells(&self, row: u32) -> Option<&[Cell]> {
        self.lines.get(row as usize).map(|l| l.cells())
    }

    /// Mutably borrow a line by row index; marks the row dirty.
    pub fn get_mut(&mut self, row: u32) -> Option<&mut Line> {
        let line = self.lines.get_mut(row as usize)?;
        self.dirty.mark(row);
        Some(line)
    }

    /// Return a reference to the dirty mask.
    pub fn dirty(&self) -> &DirtyMask {
        &self.dirty
    }

    /// Clear all dirty flags.
    pub fn mark_clean(&mut self) {
        self.dirty.clear();
    }

    /// Mark every row dirty.
    pub fn mark_all_dirty(&mut self) {
        self.dirty.mark_all(self.rows);
    }

    /// Resize the grid, adding or removing rows/columns.
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

    /// Mutably borrow a cell at (row, col); marks the row dirty.
    pub fn cell_mut(&mut self, row: u32, col: u32) -> Option<&mut crate::cell::Cell> {
        let result = self
            .lines
            .get_mut(row as usize)
            .and_then(|line| line.get_mut(col));
        if result.is_some() {
            self.dirty.mark(row);
        }
        result
    }

    /// Borrow a cell at (row, col).
    pub fn cell(&self, row: u32, col: u32) -> Option<&crate::cell::Cell> {
        self.lines.get(row as usize).and_then(|line| line.get(col))
    }

    /// Return text content within a range of cells.
    pub fn text_in_range(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> alloc::string::String {
        let mut result = alloc::string::String::new();
        for r in start_row..=end_row {
            let cstart = if r == start_row { start_col } else { 0 };
            let cend = if r == end_row { end_col } else { u32::MAX };
            for c in cstart..=cend {
                if let Some(cell) = self.cell(r, c) {
                    result.push(cell.char);
                }
            }
        }
        result
    }

    /// Mark a single row dirty.
    pub fn mark_row_dirty(&mut self, row: u32) {
        self.dirty.mark(row);
    }

    /// Mark a range of rows dirty [start, end).
    pub fn mark_rows_dirty(&mut self, start: u32, end: u32) {
        for row in start..end {
            self.dirty.mark(row);
        }
    }

    /// Scroll the region [top, bottom) up by one line.
    pub fn scroll_up(&mut self, top: u32, bottom: u32, cols: u32) {
        if top >= bottom || bottom > self.rows {
            return;
        }
        let top_index = top as usize;
        let bottom_index = bottom as usize;
        if top == 0 {
            let removed = self.lines.remove(top_index);
            self.scrollback.push_back(removed);
            while self.scrollback.len() > self.max_scrollback {
                self.scrollback.pop_front();
            }
            self.lines.insert(bottom_index - 1, Line::new(cols));
        } else {
            self.lines[top_index..bottom_index].rotate_left(1);
            if let Some(line) = self.lines.get_mut(bottom_index - 1) {
                *line = Line::new(cols);
            }
        }
        for row in top..bottom {
            self.dirty.mark(row);
        }
    }

    /// Scroll the region [top, bottom) down by one line.
    pub fn scroll_down(&mut self, top: u32, bottom: u32, cols: u32) {
        if top >= bottom || bottom > self.rows {
            return;
        }
        let top_index = top as usize;
        let bottom_index = bottom as usize;
        self.lines[top_index..bottom_index].rotate_right(1);
        if let Some(line) = self.lines.get_mut(top_index) {
            *line = Line::new(cols);
        }
        for row in top..bottom {
            self.dirty.mark(row);
        }
    }

    /// Insert `count` blank lines at `at` within region [at, bottom).
    pub fn insert_lines(&mut self, at: u32, count: u32, bottom: u32, cols: u32) {
        if at >= bottom || count == 0 {
            return;
        }
        let actual = count.min(bottom - at);
        let at_index = at as usize;
        let bottom_index = bottom as usize;
        self.lines[at_index..bottom_index].rotate_right(actual as usize);
        for i in at_index..at_index + actual as usize {
            *self
                .lines
                .get_mut(i)
                .expect("grid invariant: i < lines.len() after rotate_right") = Line::new(cols);
        }
        for row in at..bottom {
            self.dirty.mark(row);
        }
    }

    /// Delete `count` lines at `at` within region [at, bottom).
    pub fn delete_lines(&mut self, at: u32, count: u32, bottom: u32, cols: u32) {
        if at >= bottom || count == 0 {
            return;
        }
        let actual = count.min(bottom - at);
        let at_index = at as usize;
        let bottom_index = bottom as usize;
        self.lines[at_index..bottom_index].rotate_left(actual as usize);
        for i in bottom_index - actual as usize..bottom_index {
            *self
                .lines
                .get_mut(i)
                .expect("grid invariant: i < lines.len() after rotate_left") = Line::new(cols);
        }
        for row in at..bottom {
            self.dirty.mark(row);
        }
    }

    /// Clear cells in [start_col, end_col) on `row`.
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

    /// Fill cells in [start_col, end_col) on `row` with `character`.
    pub fn fill_cells(&mut self, row: u32, character: char, start_col: u32, end_col: u32) {
        if let Some(line) = self.lines.get_mut(row as usize) {
            for col in start_col..end_col.min(line.len()) {
                if let Some(cell) = line.get_mut(col) {
                    cell.char = character;
                }
            }
            self.dirty.mark(row);
        }
    }

    /// Copy a rectangular region from (src_top, src_left) to (dest_top, dest_left).
    pub fn copy_rect(
        &mut self,
        src_top: u32,
        src_left: u32,
        dest_top: u32,
        dest_left: u32,
        width: u32,
        height: u32,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let copy_end_col = (src_left + width).min(self.cols);
        if copy_end_col <= src_left {
            return;
        }
        for row_off in 0..height {
            let src_row = src_top + row_off;
            let dst_row = dest_top + row_off;
            if src_row >= self.rows || dst_row >= self.rows {
                continue;
            }
            if src_row == dst_row {
                let cells_copy = self.lines[src_row as usize].cells().to_vec();
                for col in src_left..copy_end_col {
                    let src_idx = (col - src_left) as usize;
                    if src_idx >= cells_copy.len() {
                        break;
                    }
                    let cell = cells_copy[src_idx];
                    let dst_col = dest_left + col - src_left;
                    if dst_col < self.cols
                        && let Some(dst_cell) = self.lines[dst_row as usize].get_mut(dst_col)
                    {
                        *dst_cell = cell;
                    }
                }
            } else {
                for col in src_left..copy_end_col {
                    let src_val = self.cell(src_row, col).copied();
                    let dst_col = dest_left + col - src_left;
                    if dst_col < self.cols
                        && let Some(cell) = src_val
                        && let Some(dst_cell) = self.cell_mut(dst_row, dst_col)
                    {
                        *dst_cell = cell;
                    }
                }
            }
            self.dirty.mark(dst_row);
        }
    }

    /// Erase a rectangular region, resetting cells to defaults.
    pub fn erase_rect(&mut self, top: u32, left: u32, width: u32, height: u32, erase_char: char) {
        for row in top..(top + height).min(self.rows) {
            if let Some(line) = self.lines.get_mut(row as usize) {
                for col in left..(left + width).min(line.len()) {
                    if let Some(cell) = line.get_mut(col) {
                        cell.char = erase_char;
                        cell.foreground = Color::default();
                        cell.background = Color::default();
                        cell.attrs = Attrs::default();
                        cell.width = 1;
                    }
                }
                self.dirty.mark(row);
            }
        }
    }

    /// Fill a rectangular region with `character`.
    pub fn fill_rect(&mut self, top: u32, left: u32, width: u32, height: u32, character: char) {
        for row in top..(top + height).min(self.rows) {
            if let Some(line) = self.lines.get_mut(row as usize) {
                for col in left..(left + width).min(line.len()) {
                    if let Some(cell) = line.get_mut(col) {
                        cell.char = character;
                    }
                }
                self.dirty.mark(row);
            }
        }
    }

    /// Erase a line, optionally preserving protected cells.
    pub fn selective_erase_line(&mut self, row: u32, protect: bool) {
        if let Some(line) = self.lines.get_mut(row as usize) {
            for col in 0..line.len() {
                if let Some(cell) = line.get_mut(col) {
                    if protect && cell.attrs.protected {
                        continue;
                    }
                    *cell = Cell::default();
                }
            }
            self.dirty.mark(row);
        }
    }

    /// Erase rows [top, bottom), optionally preserving protected cells.
    pub fn selective_erase_display(&mut self, top: u32, bottom: u32, protect: bool) {
        for row in top..bottom.min(self.rows) {
            self.selective_erase_line(row, protect);
        }
    }

    /// Number of lines in the scrollback buffer.
    pub fn scrollback_length(&self) -> usize {
        self.scrollback.len()
    }

    /// Borrow a scrollback line by index.
    pub fn scrollback_line(&self, index: usize) -> Option<&Line> {
        self.scrollback.get(index)
    }

    /// Clear all scrollback history.
    pub fn clear_scrollback(&mut self) {
        self.scrollback.clear();
    }

    /// Maximum number of scrollback lines.
    pub fn max_scrollback(&self) -> usize {
        self.max_scrollback
    }

    /// Whether the alternate screen buffer is active.
    pub fn alt_screen(&self) -> bool {
        self.alt_screen
    }

    /// Enable or disable the alternate screen buffer.
    pub fn set_alt_screen(&mut self, enabled: bool) {
        self.alt_screen = enabled;
    }

    /// Push a line onto the scrollback buffer.
    pub fn push_scrollback(&mut self, line: Line) {
        self.scrollback.push_back(line);
        while self.scrollback.len() > self.max_scrollback {
            self.scrollback.pop_front();
        }
    }

    /// Validates basic invariants. For debugging, checks internal consistency.
    pub fn assert_invariants(&self) {
        assert!(
            self.lines.len() == self.rows as usize,
            "lines.len()={} != rows={}",
            self.lines.len(),
            self.rows
        );
        assert!(
            self.scrollback.len() <= self.max_scrollback,
            "scrollback {} > max {}",
            self.scrollback.len(),
            self.max_scrollback
        );
        for (i, line) in self.lines.iter().enumerate() {
            assert!(
                line.len() == self.cols,
                "line {i} len={} != cols={}",
                line.len(),
                self.cols
            );
        }
        for i in 1..self.lines.len() {
            let prev_ptr = self.lines[i - 1].cells().as_ptr();
            let curr_ptr = self.lines[i].cells().as_ptr();
            assert!(
                prev_ptr != curr_ptr,
                "adjacent duplicate line pointer at rows {}-{}",
                i - 1,
                i
            );
        }
        if self.alt_screen {
            assert!(
                self.scrollback.is_empty(),
                "alt screen must have no scrollback history ({} rows)",
                self.scrollback.len()
            );
        }
        for (row, line) in self.lines.iter().enumerate() {
            if let Some(cell) = line.get(0) {
                use unicode_width::UnicodeWidthChar;
                let width = cell.char.width();
                assert!(
                    width != Some(0),
                    "zero-width character at col 0 row {}: U+{:04X}",
                    row,
                    cell.char as u32
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

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
    fn grid_row_cells_returns_full_slice() {
        let mut g = Grid::new(3, 5);
        g.fill_cells(1, 'X', 0, 5);
        let row1 = g.row_cells(1).unwrap();
        assert_eq!(row1.len(), 5);
        assert!(row1.iter().all(|c| c.char == 'X'));
    }

    #[test]
    fn grid_row_cells_out_of_bounds() {
        let g = Grid::new(3, 5);
        assert!(g.row_cells(3).is_none());
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
        assert_eq!(g.scrollback_length(), 1);
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
        assert_eq!(g.scrollback_length(), 0);
    }

    #[test]
    fn grid_scrollback_max_limit() {
        let mut g = Grid::with_scrollback(2, 5, 3);
        for i in 0..5 {
            g.fill_cells(0, (b'A' + i) as char, 0, 5);
            g.scroll_up(0, 2, 5);
        }
        assert!(g.scrollback_length() <= 3);
    }

    #[test]
    fn grid_scrollback_clear() {
        let mut g = Grid::new(2, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.scroll_up(0, 2, 5);
        assert_eq!(g.scrollback_length(), 1);
        g.clear_scrollback();
        assert_eq!(g.scrollback_length(), 0);
    }

    #[quickcheck]
    fn prop_grid_resize_preserves_cols(rows: u32, cols: u32, new_rows: u32, new_cols: u32) -> bool {
        let rows = rows.clamp(1, 30);
        let cols = cols.clamp(1, 30);
        let new_rows = new_rows.clamp(1, 50);
        let new_cols = new_cols.clamp(1, 50);
        let mut g = Grid::new(rows, cols);
        g.mark_clean();
        g.resize(new_rows, new_cols);
        g.assert_invariants();
        g.rows() == new_rows && g.cols() == new_cols
    }

    #[quickcheck]
    fn prop_grid_invariant_after_scroll(
        rows: u32,
        cols: u32,
        top: u32,
        bottom: u32,
        scroll_count: u8,
    ) -> bool {
        let rows = rows.clamp(3, 50);
        let cols = cols.clamp(1, 80);
        let bottom = bottom.clamp(1, rows);
        let top = top.min(bottom - 1);
        let mut g = Grid::new(rows, cols);
        for _ in 0..(scroll_count % 20) {
            g.scroll_up(top, bottom, cols);
            g.assert_invariants();
            g.scroll_down(top, bottom, cols);
            g.assert_invariants();
        }
        true
    }

    #[test]
    fn grid_invariant_fresh() {
        let g = Grid::new(24, 80);
        g.assert_invariants();
    }

    #[test]
    fn grid_invariant_after_scroll_up() {
        let mut g = Grid::new(5, 10);
        g.fill_cells(0, 'A', 0, 10);
        g.scroll_up(0, 5, 10);
        g.assert_invariants();
        assert_eq!(g.scrollback_length(), 1);
    }

    #[test]
    fn grid_invariant_after_scroll_down() {
        let mut g = Grid::new(5, 10);
        g.fill_cells(4, 'Z', 0, 10);
        g.scroll_down(0, 5, 10);
        g.assert_invariants();
    }

    #[test]
    fn grid_invariant_after_insert_lines() {
        let mut g = Grid::new(10, 20);
        g.fill_cells(0, 'X', 0, 20);
        g.insert_lines(3, 2, 10, 20);
        g.assert_invariants();
    }

    #[test]
    fn grid_invariant_after_delete_lines() {
        let mut g = Grid::new(10, 20);
        g.fill_cells(0, 'X', 0, 20);
        g.delete_lines(2, 3, 10, 20);
        g.assert_invariants();
    }

    #[test]
    fn grid_invariant_after_resize_grow() {
        let mut g = Grid::new(5, 10);
        g.fill_cells(0, 'A', 0, 10);
        g.resize(20, 40);
        g.assert_invariants();
    }

    #[test]
    fn grid_invariant_after_resize_shrink() {
        let mut g = Grid::new(20, 40);
        g.fill_cells(0, 'A', 0, 40);
        g.fill_cells(19, 'Z', 0, 40);
        g.resize(5, 10);
        g.assert_invariants();
    }

    #[test]
    fn grid_invariant_after_clear_and_fill() {
        let mut g = Grid::new(5, 10);
        g.fill_cells(2, 'X', 0, 10);
        g.assert_invariants();
        g.clear_cells(2, 0, 10);
        g.assert_invariants();
    }

    #[test]
    fn grid_invariant_after_multiple_scrolls_with_scrollback() {
        let mut g = Grid::with_scrollback(5, 10, 100);
        for i in 0..25 {
            g.fill_cells(0, (b'A' + (i % 26) as u8) as char, 0, 10);
            g.scroll_up(0, 5, 10);
            g.assert_invariants();
        }
    }

    #[quickcheck]
    fn prop_dirty_mask_mark_then_clear(rows: u8) -> bool {
        let rows = rows.clamp(1, 200) as u32;
        let mut m = DirtyMask::new(rows);
        m.clear();
        for r in 0..rows {
            m.mark(r);
            if !m.is_dirty(r) {
                return false;
            }
        }
        m.clear();
        !m.any_dirty()
    }

    #[quickcheck]
    fn prop_scrollback_bounded(max_lines: u8, scrolls: u8) -> bool {
        let max_lines = (max_lines.clamp(1, 100)) as u32;
        let scrolls = scrolls.clamp(0, 200);
        let mut g = Grid::with_scrollback(2, 5, max_lines as usize);
        for _ in 0..scrolls {
            g.fill_cells(0, 'A', 0, 5);
            g.scroll_up(0, 2, 5);
        }
        g.scrollback_length() <= max_lines as usize
    }

    #[test]
    fn grid_with_scrollback_dimensions() {
        let g = Grid::with_scrollback(24, 80, 1000);
        assert_eq!(g.rows(), 24);
        assert_eq!(g.cols(), 80);
        assert_eq!(g.max_scrollback(), 1000);
    }

    #[test]
    fn grid_default_max_scrollback_50k() {
        let g = Grid::new(24, 80);
        assert_eq!(g.max_scrollback(), 50_000);
    }

    #[test]
    fn grid_scroll_up_invalid_top_equals_bottom() {
        let mut g = Grid::new(5, 5);
        g.mark_clean();
        g.scroll_up(3, 3, 5);
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_scroll_up_invalid_bottom_above_rows() {
        let mut g = Grid::new(5, 5);
        g.mark_clean();
        g.scroll_up(0, 10, 5);
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_scroll_up_region_size_one() {
        let mut g = Grid::new(5, 5);
        g.mark_clean();
        g.scroll_up(0, 1, 5);
        assert!(
            g.dirty().any_dirty(),
            "single-row scroll_up must mark dirty"
        );
    }

    #[test]
    fn grid_scroll_up_region_marks_dirty() {
        let mut g = Grid::new(5, 5);
        g.mark_clean();
        g.scroll_up(0, 5, 5);
        assert!(g.dirty().any_dirty());
    }

    #[test]
    fn grid_scroll_up_top_zero_appends_to_scrollback() {
        let mut g = Grid::new(2, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.scroll_up(0, 2, 5);
        assert_eq!(g.scrollback_length(), 1);
        let first = g.scrollback_line(0).unwrap();
        assert_eq!(first.get(0).unwrap().char, 'A');
    }

    #[test]
    fn grid_scroll_up_top_nonzero_does_not_push_scrollback() {
        let mut g = Grid::new(4, 5);
        g.scroll_up(1, 4, 5);
        assert_eq!(g.scrollback_length(), 0);
    }

    #[test]
    fn grid_scroll_down_marks_dirty() {
        let mut g = Grid::new(5, 5);
        g.mark_clean();
        g.scroll_down(0, 5, 5);
        assert!(g.dirty().any_dirty());
    }

    #[test]
    fn grid_scroll_down_top_equals_bottom_no_op() {
        let mut g = Grid::new(5, 5);
        g.mark_clean();
        g.scroll_down(2, 2, 5);
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_insert_lines_basic() {
        let mut g = Grid::new(4, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.fill_cells(3, 'D', 0, 3);
        g.insert_lines(1, 1, 4, 3);
        // After insert: row at index 1 is blank, B pushed to 2, C to 3, D discarded
        assert_eq!(g.get(1).unwrap().get(0).unwrap().char, ' ');
        assert_eq!(g.get(2).unwrap().get(0).unwrap().char, 'B');
        assert_eq!(g.get(3).unwrap().get(0).unwrap().char, 'C');
    }

    #[test]
    fn grid_insert_lines_at_equals_bottom_no_op() {
        let mut g = Grid::new(3, 3);
        g.mark_clean();
        g.insert_lines(3, 1, 3, 3);
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_insert_lines_zero_count_no_op() {
        let mut g = Grid::new(3, 3);
        g.mark_clean();
        g.insert_lines(1, 0, 3, 3);
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_insert_lines_count_clamped_to_region() {
        let mut g = Grid::new(4, 3);
        g.insert_lines(0, 100, 4, 3);
        // Should insert min(100, 4) = 4 blank lines, so all are blank
        for r in 0..4 {
            assert_eq!(g.get(r).unwrap().get(0).unwrap().char, ' ');
        }
    }

    #[test]
    fn grid_delete_lines_basic() {
        let mut g = Grid::new(4, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.fill_cells(3, 'D', 0, 3);
        g.delete_lines(1, 1, 4, 3);
        // After delete: A stays at 0, C moves to 1, D moves to 2, last row is blank
        assert_eq!(g.get(0).unwrap().get(0).unwrap().char, 'A');
        assert_eq!(g.get(1).unwrap().get(0).unwrap().char, 'C');
        assert_eq!(g.get(2).unwrap().get(0).unwrap().char, 'D');
        assert_eq!(g.get(3).unwrap().get(0).unwrap().char, ' ');
    }

    #[test]
    fn grid_delete_lines_at_equals_bottom_no_op() {
        let mut g = Grid::new(3, 3);
        g.mark_clean();
        g.delete_lines(3, 1, 3, 3);
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_delete_lines_zero_count_no_op() {
        let mut g = Grid::new(3, 3);
        g.mark_clean();
        g.delete_lines(0, 0, 3, 3);
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_clear_cells_range() {
        let mut g = Grid::new(1, 10);
        g.fill_cells(0, 'X', 0, 10);
        g.clear_cells(0, 2, 5);
        assert_eq!(g.get(0).unwrap().get(0).unwrap().char, 'X');
        assert_eq!(g.get(0).unwrap().get(1).unwrap().char, 'X');
        assert_eq!(g.get(0).unwrap().get(2).unwrap().char, ' ');
        assert_eq!(g.get(0).unwrap().get(4).unwrap().char, ' ');
        assert_eq!(g.get(0).unwrap().get(5).unwrap().char, 'X');
    }

    #[test]
    fn grid_clear_cells_out_of_range_clamps() {
        let mut g = Grid::new(1, 5);
        g.fill_cells(0, 'X', 0, 5);
        g.clear_cells(0, 3, 100);
        for c in 3..5 {
            assert_eq!(g.get(0).unwrap().get(c).unwrap().char, ' ');
        }
    }

    #[test]
    fn grid_clear_cells_invalid_row_no_panic() {
        let mut g = Grid::new(3, 5);
        g.fill_cells(0, 'X', 0, 5);
        g.clear_cells(100, 0, 5);
        assert_eq!(g.get(0).unwrap().get(0).unwrap().char, 'X');
        assert_eq!(g.rows(), 3);
    }

    #[test]
    fn grid_fill_cells_partial_range() {
        let mut g = Grid::new(1, 10);
        g.fill_cells(0, 'X', 3, 7);
        for c in 0..3 {
            assert_eq!(g.get(0).unwrap().get(c).unwrap().char, ' ');
        }
        for c in 3..7 {
            assert_eq!(g.get(0).unwrap().get(c).unwrap().char, 'X');
        }
        for c in 7..10 {
            assert_eq!(g.get(0).unwrap().get(c).unwrap().char, ' ');
        }
    }

    #[test]
    fn grid_cell_mut_marks_dirty() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        let cell = g.cell_mut(1, 2).unwrap();
        cell.char = 'Z';
        assert!(g.dirty().is_dirty(1));
    }

    #[test]
    fn grid_cell_mut_out_of_bounds_returns_none() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        assert!(g.cell_mut(3, 0).is_none());
        assert!(g.cell_mut(0, 5).is_none());
        assert!(!g.dirty().any_dirty());
    }

    #[test]
    fn grid_cell_read() {
        let mut g = Grid::new(2, 3);
        g.get_mut(0).unwrap().get_mut(1).unwrap().char = 'Y';
        assert_eq!(g.cell(0, 1).unwrap().char, 'Y');
        assert!(g.cell(5, 0).is_none());
    }

    #[test]
    fn grid_mark_row_dirty() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        g.mark_row_dirty(2);
        assert!(g.dirty().is_dirty(2));
        assert!(!g.dirty().is_dirty(1));
    }

    #[test]
    fn grid_mark_rows_dirty_range() {
        let mut g = Grid::new(10, 5);
        g.mark_clean();
        g.mark_rows_dirty(3, 7);
        for r in 3..7 {
            assert!(g.dirty().is_dirty(r));
        }
        assert!(!g.dirty().is_dirty(2));
        assert!(!g.dirty().is_dirty(7));
    }

    #[test]
    fn grid_mark_all_dirty() {
        let mut g = Grid::new(5, 5);
        g.mark_clean();
        g.mark_all_dirty();
        for r in 0..5 {
            assert!(g.dirty().is_dirty(r));
        }
    }

    #[test]
    fn grid_push_scrollback_within_limit() {
        let mut g = Grid::with_scrollback(2, 3, 5);
        g.push_scrollback(Line::new(3));
        g.push_scrollback(Line::new(3));
        assert_eq!(g.scrollback_length(), 2);
    }

    #[test]
    fn grid_push_scrollback_evicts_oldest() {
        let mut g = Grid::with_scrollback(2, 3, 3);
        for _ in 0..10 {
            g.push_scrollback(Line::new(3));
        }
        assert_eq!(g.scrollback_length(), 3);
    }

    #[test]
    fn grid_scrollback_line_out_of_bounds() {
        let g = Grid::new(2, 5);
        assert!(g.scrollback_line(0).is_none());
    }

    #[test]
    fn grid_scrollback_clear_via_method() {
        let mut g = Grid::new(2, 5);
        g.fill_cells(0, 'X', 0, 5);
        g.scroll_up(0, 2, 5);
        assert!(g.scrollback_length() > 0);
        g.clear_scrollback();
        assert_eq!(g.scrollback_length(), 0);
    }

    #[test]
    fn grid_scrollback_line_index_returns_correct() {
        let mut g = Grid::with_scrollback(2, 5, 10);
        for i in 0..3 {
            g.fill_cells(0, (b'A' + i) as char, 0, 5);
            g.scroll_up(0, 2, 5);
        }
        // Oldest is at index 0
        assert_eq!(g.scrollback_line(0).unwrap().get(0).unwrap().char, 'A');
        assert_eq!(g.scrollback_line(1).unwrap().get(0).unwrap().char, 'B');
        assert_eq!(g.scrollback_line(2).unwrap().get(0).unwrap().char, 'C');
    }

    #[test]
    fn grid_resize_shrinks_preserves_existing_data() {
        let mut g = Grid::new(10, 20);
        g.fill_cells(0, 'X', 0, 20);
        g.resize(5, 10);
        assert_eq!(g.cell(0, 0).unwrap().char, 'X');
        assert_eq!(g.cell(0, 9).unwrap().char, 'X');
    }

    #[test]
    fn grid_resize_grows_preserves_existing_data() {
        let mut g = Grid::new(5, 10);
        g.fill_cells(0, 'A', 0, 10);
        g.resize(10, 20);
        assert_eq!(g.cell(0, 0).unwrap().char, 'A');
        assert_eq!(g.cell(0, 9).unwrap().char, 'A');
    }

    #[test]
    fn grid_scroll_up_preserves_top_zero_only() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.scroll_up(0, 3, 3);
        // 'A' goes to scrollback, 'B' moves to 0, 'C' moves to 1, last is blank
        assert_eq!(g.cell(0, 0).unwrap().char, 'B');
        assert_eq!(g.cell(1, 0).unwrap().char, 'C');
        assert_eq!(g.cell(2, 0).unwrap().char, ' ');
        assert_eq!(g.scrollback_line(0).unwrap().get(0).unwrap().char, 'A');
    }

    #[test]
    fn grid_resize_cols_changes_line_lengths() {
        let mut g = Grid::new(2, 5);
        g.fill_cells(0, 'X', 0, 5);
        g.resize(2, 10);
        assert_eq!(g.row_cells(0).unwrap().len(), 10);
    }

    #[test]
    fn grid_zero_zero_creates_empty() {
        let g = Grid::new(0, 0);
        assert_eq!(g.rows(), 0);
        assert_eq!(g.cols(), 0);
    }

    #[test]
    fn grid_alt_screen_default_false() {
        let g = Grid::new(5, 5);
        assert!(!g.alt_screen());
    }

    #[test]
    fn grid_set_alt_screen_toggle() {
        let mut g = Grid::new(5, 5);
        g.set_alt_screen(true);
        assert!(g.alt_screen());
        g.set_alt_screen(false);
        assert!(!g.alt_screen());
    }

    #[test]
    fn grid_selective_erase_line_clears_all() {
        let mut g = Grid::new(2, 5);
        g.fill_cells(0, 'X', 0, 5);
        g.mark_clean();
        g.selective_erase_line(0, false);
        for c in 0..5 {
            assert_eq!(g.get(0).unwrap().get(c).unwrap().char, ' ');
        }
        assert!(g.dirty().is_dirty(0));
    }

    #[test]
    fn grid_selective_erase_line_protects_flagged() {
        let mut g = Grid::new(1, 3);
        g.get_mut(0).unwrap().get_mut(0).unwrap().char = 'A';
        g.get_mut(0).unwrap().get_mut(1).unwrap().char = 'B';
        g.get_mut(0).unwrap().get_mut(1).unwrap().attrs.protected = true;
        g.get_mut(0).unwrap().get_mut(2).unwrap().char = 'C';
        g.mark_clean();
        g.selective_erase_line(0, true);
        assert_eq!(g.get(0).unwrap().get(0).unwrap().char, ' ');
        assert_eq!(g.get(0).unwrap().get(1).unwrap().char, 'B');
        assert_eq!(g.get(0).unwrap().get(2).unwrap().char, ' ');
    }

    #[test]
    fn grid_selective_erase_line_no_protect_clears_protected() {
        let mut g = Grid::new(1, 2);
        g.get_mut(0).unwrap().get_mut(0).unwrap().char = 'A';
        g.get_mut(0).unwrap().get_mut(0).unwrap().attrs.protected = true;
        g.get_mut(0).unwrap().get_mut(1).unwrap().char = 'B';
        g.mark_clean();
        g.selective_erase_line(0, false);
        assert_eq!(g.get(0).unwrap().get(0).unwrap().char, ' ');
        assert_eq!(g.get(0).unwrap().get(1).unwrap().char, ' ');
    }

    #[test]
    fn grid_selective_erase_display_range() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.mark_clean();
        g.selective_erase_display(1, 3, false);
        for c in 0..3 {
            assert_eq!(g.get(0).unwrap().get(c).unwrap().char, 'A');
            assert_eq!(g.get(1).unwrap().get(c).unwrap().char, ' ');
            assert_eq!(g.get(2).unwrap().get(c).unwrap().char, ' ');
        }
    }

    #[test]
    fn grid_selective_erase_line_out_of_bounds_no_panic() {
        let mut g = Grid::new(2, 3);
        g.selective_erase_line(100, false);
    }

    #[test]
    fn grid_cell_ref_returns_correct_cell() {
        let mut g = Grid::new(3, 5);
        g.fill_cells(1, 'X', 0, 5);
        let cell = g.cell(1, 2).unwrap();
        assert_eq!(cell.char, 'X');
    }

    #[test]
    fn grid_cell_ref_out_of_bounds_returns_none() {
        let g = Grid::new(3, 5);
        assert!(g.cell(10, 0).is_none());
        assert!(g.cell(0, 10).is_none());
    }

    #[test]
    fn grid_cell_mut_modifies_cell() {
        let mut g = Grid::new(3, 5);
        let cell = g.cell_mut(0, 0).unwrap();
        cell.char = 'Z';
        cell.foreground = Color::new(100, 200, 50);
        assert_eq!(g.cell(0, 0).unwrap().char, 'Z');
        assert_eq!(g.cell(0, 0).unwrap().foreground, Color::new(100, 200, 50));
    }

    #[test]
    fn grid_copy_rect_basic() {
        let mut g = Grid::new(4, 4);
        g.fill_cells(0, 'A', 0, 4);
        g.fill_cells(1, 'B', 0, 4);
        g.copy_rect(0, 0, 2, 0, 4, 2);
        assert_eq!(g.cell(0, 0).unwrap().char, 'A');
        assert_eq!(g.cell(1, 0).unwrap().char, 'B');
        assert_eq!(g.cell(2, 0).unwrap().char, 'A');
        assert_eq!(g.cell(3, 0).unwrap().char, 'B');
    }

    #[test]
    fn grid_alt_screen_toggle() {
        let mut g = Grid::new(3, 3);
        assert!(!g.alt_screen());
        g.set_alt_screen(true);
        assert!(g.alt_screen());
        g.set_alt_screen(false);
        assert!(!g.alt_screen());
    }

    #[test]
    fn grid_assert_invariants_no_panic_on_valid_grid() {
        let g = Grid::new(5, 10);
        g.assert_invariants();
    }

    #[test]
    fn grid_scrollback_operations() {
        let mut g = Grid::new(3, 5);
        assert_eq!(g.scrollback_length(), 0);
        assert!(g.scrollback_line(0).is_none());
        g.push_scrollback(Line::new(5));
        assert_eq!(g.scrollback_length(), 1);
        assert!(g.scrollback_line(0).is_some());
        g.clear_scrollback();
        assert_eq!(g.scrollback_length(), 0);
    }

    #[test]
    fn grid_max_scrollback_stored() {
        let g = Grid::with_scrollback(3, 5, 500);
        assert_eq!(g.max_scrollback(), 500);
    }

    #[test]
    fn grid_dirty_mask_access() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        assert!(!g.dirty().any_dirty());
        g.mark_all_dirty();
        assert!(g.dirty().any_dirty());
    }

    #[test]
    fn grid_get_and_get_mut_consistent() {
        let mut g = Grid::new(2, 3);
        g.fill_cells(0, 'P', 0, 3);
        assert_eq!(g.get(0).unwrap().get(1).unwrap().char, 'P');
        g.get_mut(0).unwrap().get_mut(1).unwrap().char = 'Q';
        assert_eq!(g.get(0).unwrap().get(1).unwrap().char, 'Q');
    }

    #[test]
    fn grid_copy_rect_same_row() {
        let mut g = Grid::new(2, 10);
        g.fill_cells(0, 'A', 0, 4);
        g.copy_rect(0, 0, 0, 5, 4, 1);
        assert_eq!(g.cell(0, 0).unwrap().char, 'A', "source unchanged");
        assert_eq!(g.cell(0, 5).unwrap().char, 'A', "copied to dest col 5");
        assert_eq!(g.cell(0, 8).unwrap().char, 'A', "copied to dest col 8");
        assert_eq!(g.cell(0, 9).unwrap().char, ' ', "col 9 untouched");
    }

    #[test]
    fn grid_scroll_up_single_row_clears() {
        let mut g = Grid::new(3, 5);
        g.fill_cells(1, 'X', 0, 5);
        assert_eq!(g.cell(1, 0).unwrap().char, 'X');
        g.scroll_up(1, 2, 5);
        assert_eq!(
            g.cell(1, 0).unwrap().char,
            ' ',
            "single-row scroll_up must clear the row"
        );
    }

    #[test]
    fn grid_scroll_up_multi_row_works() {
        let mut g = Grid::new(4, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.fill_cells(1, 'B', 0, 5);
        g.fill_cells(2, 'C', 0, 5);
        g.scroll_up(0, 3, 5);
        assert_eq!(g.cell(0, 0).unwrap().char, 'B');
        assert_eq!(g.cell(1, 0).unwrap().char, 'C');
        assert_eq!(g.cell(2, 0).unwrap().char, ' ');
    }

    // ── Property tests for resize content preservation and dirty marking ──

    #[quickcheck]
    fn prop_grid_resize_preserves_overlap_content(rows: u32, cols: u32) -> bool {
        let rows = rows.clamp(1, 30);
        let cols = cols.clamp(1, 30);
        let mut g = Grid::new(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                if let Some(cell) = g.cell_mut(r, c) {
                    cell.char = 'X';
                }
            }
        }
        g.mark_clean();
        let new_rows = rows + 5;
        let new_cols = cols + 5;
        g.resize(new_rows, new_cols);
        g.assert_invariants();
        for r in 0..rows {
            for c in 0..cols {
                if g.cell(r, c).unwrap().char != 'X' {
                    return false;
                }
            }
        }
        true
    }

    #[quickcheck]
    fn prop_grid_mark_clean_after_mark_all_dirty(rows: u32) -> bool {
        if rows == 0 || rows > 200 {
            return true;
        }
        let mut g = Grid::new(rows, rows.max(1));
        g.mark_all_dirty();
        if !g.dirty().any_dirty() {
            return false;
        }
        g.mark_clean();
        !g.dirty().any_dirty()
    }

    #[quickcheck]
    fn prop_grid_expand_retains_content(
        rows: u16,
        cols: u16,
        expand_rows: u16,
        expand_cols: u16,
    ) -> bool {
        let r = (rows % 50).max(3) as u32;
        let c = (cols % 80).max(10) as u32;
        let er = r.saturating_add((expand_rows % 20) as u32);
        let ec = c.saturating_add((expand_cols % 20) as u32);
        let mut grid = Grid::new(r, c);
        if let Some(cell) = grid.cell_mut(0, 0) {
            cell.char = 'X';
        }
        grid.resize(er, ec);
        grid.cell(0, 0).is_some_and(|cell| cell.char == 'X')
    }

    #[quickcheck]
    fn prop_grid_row_col_contract(rows: u16, cols: u16) -> bool {
        let r = (rows % 30).max(5) as u32;
        let c = (cols % 50).max(10) as u32;
        let mut grid = Grid::new(r, c);
        if let Some(cell) = grid.cell_mut(r - 1, c - 1) {
            cell.char = 'Z';
        }
        grid.resize(r / 2 + 1, c / 2 + 1);
        grid.rows() == r / 2 + 1 && grid.cols() == c / 2 + 1
    }

    #[test]
    fn grid_copy_rect_out_of_bounds_src() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.copy_rect(10, 0, 0, 0, 3, 3);
        g.assert_invariants();
    }

    #[test]
    fn grid_copy_rect_out_of_bounds_dst() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.copy_rect(0, 0, 10, 0, 3, 3);
        g.assert_invariants();
    }

    #[test]
    fn grid_copy_rect_width_exceeds_cols() {
        let mut g = Grid::new(2, 3);
        g.fill_cells(0, 'X', 0, 3);
        g.copy_rect(0, 0, 1, 0, 10, 1);
        assert_eq!(g.cell(1, 0).unwrap().char, 'X');
        assert_eq!(g.cell(1, 1).unwrap().char, 'X');
        assert_eq!(g.cell(1, 2).unwrap().char, 'X');
    }

    #[test]
    fn grid_fill_rect_basic() {
        let mut g = Grid::new(3, 4);
        g.fill_rect(0, 0, 2, 2, 'Z');
        for r in 0..2 {
            for c in 0..2 {
                assert_eq!(g.cell(r, c).unwrap().char, 'Z');
            }
        }
        assert_eq!(g.cell(0, 2).unwrap().char, ' ');
        assert_eq!(g.cell(2, 0).unwrap().char, ' ');
    }

    #[test]
    fn grid_fill_rect_full_grid() {
        let mut g = Grid::new(2, 3);
        g.fill_rect(0, 0, 3, 2, 'Y');
        for r in 0..2 {
            for c in 0..3 {
                assert_eq!(g.cell(r, c).unwrap().char, 'Y');
            }
        }
    }

    #[test]
    fn grid_erase_rect_basic() {
        let mut g = Grid::new(3, 4);
        g.fill_cells(0, 'A', 0, 4);
        g.fill_cells(1, 'B', 0, 4);
        g.erase_rect(0, 0, 2, 2, ' ');
        assert_eq!(g.cell(0, 0).unwrap().char, ' ');
        assert_eq!(g.cell(0, 2).unwrap().char, 'A');
        assert_eq!(g.cell(1, 0).unwrap().char, ' ');
        assert_eq!(g.cell(1, 2).unwrap().char, 'B');
    }

    #[test]
    fn grid_scroll_down_full_region() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.scroll_down(0, 3, 3);
        assert_eq!(g.cell(0, 0).unwrap().char, ' ');
        assert_eq!(g.cell(1, 0).unwrap().char, 'A');
        assert_eq!(g.cell(2, 0).unwrap().char, 'B');
    }

    #[test]
    fn grid_scroll_down_subregion() {
        let mut g = Grid::new(4, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.fill_cells(3, 'D', 0, 3);
        g.scroll_down(1, 4, 3);
        // row 0 unchanged, rows 1-3 scroll down
        assert_eq!(g.cell(0, 0).unwrap().char, 'A');
        assert_eq!(g.cell(1, 0).unwrap().char, ' ');
        assert_eq!(g.cell(2, 0).unwrap().char, 'B');
        assert_eq!(g.cell(3, 0).unwrap().char, 'C');
    }

    #[test]
    fn grid_fill_rect_at_boundary() {
        let mut g = Grid::new(2, 3);
        g.fill_rect(1, 2, 1, 1, 'Z');
        assert_eq!(g.cell(1, 2).unwrap().char, 'Z');
        assert_eq!(g.cell(1, 0).unwrap().char, ' ');
    }

    #[test]
    fn grid_erase_rect_out_of_bounds_no_panic() {
        let mut g = Grid::new(2, 3);
        g.erase_rect(10, 10, 5, 5, ' ');
        g.assert_invariants();
    }

    #[test]
    fn grid_erase_rect_resets_attrs() {
        let mut g = Grid::new(1, 3);
        g.cell_mut(0, 0).unwrap().attrs.bold = true;
        g.cell_mut(0, 0).unwrap().foreground = Color::new(100, 100, 100);
        g.erase_rect(0, 0, 1, 1, ' ');
        assert_eq!(g.cell(0, 0).unwrap().attrs, Attrs::default());
        assert_eq!(g.cell(0, 0).unwrap().foreground, Color::default());
    }

    #[test]
    fn grid_insert_lines_at_zero() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.insert_lines(0, 1, 3, 3);
        assert_eq!(g.cell(0, 0).unwrap().char, ' ');
        assert_eq!(g.cell(1, 0).unwrap().char, 'A');
        assert_eq!(g.cell(2, 0).unwrap().char, 'B');
    }

    #[test]
    fn grid_insert_lines_at_last_line_region_blanks_it() {
        let mut g = Grid::new(4, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.fill_cells(3, 'D', 0, 3);
        g.insert_lines(3, 1, 4, 3);
        // Insert at position bottom-1: the single-element rotate is a no-op,
        // then the line at that position gets blanked
        assert_eq!(g.cell(0, 0).unwrap().char, 'A');
        assert_eq!(g.cell(1, 0).unwrap().char, 'B');
        assert_eq!(g.cell(2, 0).unwrap().char, 'C');
        assert_eq!(g.cell(3, 0).unwrap().char, ' ');
    }

    #[test]
    fn grid_delete_lines_at_zero() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.delete_lines(0, 1, 3, 3);
        assert_eq!(g.cell(0, 0).unwrap().char, 'B');
        assert_eq!(g.cell(1, 0).unwrap().char, 'C');
        assert_eq!(g.cell(2, 0).unwrap().char, ' ');
    }

    #[test]
    fn grid_delete_lines_at_last_line_region() {
        let mut g = Grid::new(4, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.fill_cells(2, 'C', 0, 3);
        g.fill_cells(3, 'D', 0, 3);
        g.delete_lines(2, 1, 4, 3);
        assert_eq!(g.cell(2, 0).unwrap().char, 'D');
        assert_eq!(g.cell(3, 0).unwrap().char, ' ');
    }

    #[test]
    fn grid_scrollback_eviction_pushes_oldest_out() {
        let mut g = Grid::with_scrollback(2, 3, 2);
        for i in 0..5 {
            g.fill_cells(0, (b'0' + i) as char, 0, 3);
            g.scroll_up(0, 2, 3);
        }
        assert_eq!(g.scrollback_length(), 2);
        // The two most recent should be kept
        assert_eq!(g.scrollback_line(0).unwrap().get(0).unwrap().char, '3');
        assert_eq!(g.scrollback_line(1).unwrap().get(0).unwrap().char, '4');
    }

    #[test]
    fn grid_scrollback_line_after_clear_returns_none() {
        let mut g = Grid::new(2, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.scroll_up(0, 2, 3);
        assert!(g.scrollback_line(0).is_some());
        g.clear_scrollback();
        assert!(g.scrollback_line(0).is_none());
    }

    #[test]
    fn grid_copy_rect_self_overlap_no_panic() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'A', 0, 3);
        g.fill_cells(1, 'B', 0, 3);
        g.copy_rect(0, 0, 0, 0, 3, 3);
        g.assert_invariants();
    }

    #[test]
    fn grid_copy_rect_partial_overlap() {
        let mut g = Grid::new(3, 3);
        g.fill_cells(0, 'X', 0, 3);
        g.copy_rect(0, 0, 1, 0, 2, 2);
        assert_eq!(g.cell(1, 0).unwrap().char, 'X');
        assert_eq!(g.cell(1, 1).unwrap().char, 'X');
        assert_eq!(g.cell(2, 0).unwrap().char, 'X');
        assert_eq!(g.cell(2, 1).unwrap().char, 'X');
    }

    #[quickcheck]
    fn prop_grid_scroll_down_preserves_rows(rows: u32, cols: u32) -> bool {
        let rows = rows.clamp(1, 50);
        let cols = cols.clamp(1, 50);
        let mut g = Grid::new(rows, cols);
        g.scroll_down(0, rows, cols);
        g.rows() == rows
    }

    #[quickcheck]
    fn prop_grid_insert_lines_preserves_rows(at: u32, rows: u32) -> bool {
        let rows = rows.clamp(3, 50);
        let at = at % rows;
        let mut g = Grid::new(rows, 10);
        g.insert_lines(at, 1, rows, 10);
        g.assert_invariants();
        g.rows() == rows
    }

    #[quickcheck]
    fn prop_grid_delete_lines_preserves_rows(at: u32, rows: u32) -> bool {
        let rows = rows.clamp(3, 50);
        let at = at % rows;
        let mut g = Grid::new(rows, 10);
        g.delete_lines(at, 1, rows, 10);
        g.assert_invariants();
        g.rows() == rows
    }

    #[quickcheck]
    fn prop_grid_scrollback_length_bounded(scrolls: u8) -> bool {
        let scrolls = scrolls.clamp(0, 100);
        let mut g = Grid::with_scrollback(2, 5, 5);
        for i in 0..scrolls {
            g.fill_cells(0, (b'a' + (i % 26) as u8) as char, 0, 5);
            g.scroll_up(0, 2, 5);
        }
        g.scrollback_length() <= 5
    }

    #[quickcheck]
    fn prop_grid_fill_rect_marks_dirty(row: u32, rows: u32) -> bool {
        let rows = rows.clamp(1, 20);
        let r = row % rows;
        let mut g = Grid::new(rows, 10);
        g.mark_clean();
        g.fill_rect(r, 0, 10, 1, 'X');
        g.dirty().is_dirty(r)
    }

    #[quickcheck]
    fn prop_grid_erase_rect_marks_dirty(row: u32, rows: u32) -> bool {
        let rows = rows.clamp(1, 20);
        let r = row % rows;
        let mut g = Grid::new(rows, 10);
        g.mark_clean();
        g.erase_rect(r, 0, 10, 1, ' ');
        g.dirty().is_dirty(r)
    }
}
