/// Terminal-specific invalidation levels (ordered by cost).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InvalidationLevel {
    /// Only cursor blinked/moved — re-render cursor area only
    CursorOnly = 0,
    /// Cell content changed — rebuild cell instances for dirty rows
    Cells = 1,
    /// Grid geometry changed (resize, scroll) — full rebuild
    Geometry = 2,
    /// Font/theme/atlas changed — rebuild everything including atlas
    Full = 3,
}

impl InvalidationLevel {
    /// Combine two levels, taking the more severe one.
    pub fn max(self, other: Self) -> Self {
        if other > self { other } else { self }
    }

    /// Whether this level requires atlas re-upload.
    pub fn needs_atlas_rebuild(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Whether this level requires full instance buffer rebuild.
    pub fn needs_full_rebuild(self) -> bool {
        matches!(self, Self::Geometry | Self::Full)
    }

    /// Whether only cursor area needs redraw.
    pub fn is_cursor_only(self) -> bool {
        matches!(self, Self::CursorOnly)
    }
}

/// Tracks what changed between frames to skip redundant rendering work.
pub struct FrameInvalidation {
    level: InvalidationLevel,
    dirty_rows: Vec<bool>,
    rows: u32,
    cursor_moved: bool,
    selection_changed: bool,
}

impl FrameInvalidation {
    pub fn new(rows: u32) -> Self {
        Self {
            level: InvalidationLevel::Full,
            dirty_rows: vec![true; rows as usize],
            rows,
            cursor_moved: false,
            selection_changed: false,
        }
    }

    /// Record that the cursor moved (at minimum CursorOnly invalidation).
    pub fn mark_cursor_moved(&mut self) {
        self.level = self.level.max(InvalidationLevel::CursorOnly);
        self.cursor_moved = true;
    }

    /// Record that specific rows have changed content.
    pub fn mark_rows_dirty(&mut self, start: u32, end: u32) {
        self.level = self.level.max(InvalidationLevel::Cells);
        let start = start.min(self.rows) as usize;
        let end = end.min(self.rows) as usize;
        for i in start..end {
            self.dirty_rows[i] = true;
        }
    }

    /// Record that all rows are dirty (e.g., after scroll or resize).
    pub fn mark_all_dirty(&mut self) {
        self.level = self.level.max(InvalidationLevel::Geometry);
        self.dirty_rows.iter_mut().for_each(|d| *d = true);
    }

    /// Record that selection changed (needs visual update but not full rebuild).
    pub fn mark_selection_changed(&mut self) {
        self.level = self.level.max(InvalidationLevel::Cells);
        self.selection_changed = true;
    }

    /// Record that a full rebuild is needed (font/theme change).
    pub fn mark_full_rebuild(&mut self) {
        self.level = InvalidationLevel::Full;
        self.dirty_rows.iter_mut().for_each(|d| *d = true);
    }

    /// Get the current invalidation level.
    pub fn level(&self) -> InvalidationLevel {
        self.level
    }

    /// Check if a specific row is dirty.
    pub fn is_row_dirty(&self, row: u32) -> bool {
        (row as usize) < self.dirty_rows.len() && self.dirty_rows[row as usize]
    }

    /// Get all dirty row indices.
    pub fn dirty_rows(&self) -> Vec<u32> {
        self.dirty_rows
            .iter()
            .enumerate()
            .filter(|(_, &d)| d)
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// Check if cursor moved.
    pub fn cursor_moved(&self) -> bool {
        self.cursor_moved
    }

    /// Check if selection changed.
    pub fn selection_changed(&self) -> bool {
        self.selection_changed
    }

    /// Reset for next frame, keeping only the current level.
    pub fn reset(&mut self) {
        self.level = InvalidationLevel::CursorOnly;
        self.dirty_rows.iter_mut().for_each(|d| *d = false);
        self.cursor_moved = false;
        self.selection_changed = false;
    }

    /// Force full reset (e.g., after a full rebuild completes).
    pub fn clear(&mut self) {
        self.level = InvalidationLevel::CursorOnly;
        self.dirty_rows.iter_mut().for_each(|d| *d = false);
        self.cursor_moved = false;
        self.selection_changed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ordering() {
        assert!(InvalidationLevel::CursorOnly < InvalidationLevel::Cells);
        assert!(InvalidationLevel::Cells < InvalidationLevel::Geometry);
        assert!(InvalidationLevel::Geometry < InvalidationLevel::Full);
    }

    #[test]
    fn level_max_takes_higher() {
        assert_eq!(
            InvalidationLevel::CursorOnly.max(InvalidationLevel::Cells),
            InvalidationLevel::Cells
        );
        assert_eq!(
            InvalidationLevel::Geometry.max(InvalidationLevel::CursorOnly),
            InvalidationLevel::Geometry
        );
        assert_eq!(
            InvalidationLevel::Full.max(InvalidationLevel::Cells),
            InvalidationLevel::Full
        );
    }

    #[test]
    fn level_properties() {
        assert!(!InvalidationLevel::CursorOnly.needs_full_rebuild());
        assert!(InvalidationLevel::Geometry.needs_full_rebuild());
        assert!(InvalidationLevel::Full.needs_atlas_rebuild());
        assert!(!InvalidationLevel::Cells.needs_atlas_rebuild());
        assert!(InvalidationLevel::CursorOnly.is_cursor_only());
        assert!(!InvalidationLevel::Cells.is_cursor_only());
    }

    #[test]
    fn invalidation_starts_at_full() {
        let inv = FrameInvalidation::new(24);
        assert_eq!(inv.level(), InvalidationLevel::Full);
    }

    #[test]
    fn mark_cursor_moved() {
        let mut inv = FrameInvalidation::new(24);
        inv.mark_cursor_moved();
        assert_eq!(inv.level(), InvalidationLevel::CursorOnly);
        assert!(inv.cursor_moved());
    }

    #[test]
    fn mark_rows_dirty() {
        let mut inv = FrameInvalidation::new(24);
        inv.mark_rows_dirty(5, 10);
        assert_eq!(inv.level(), InvalidationLevel::Cells);
        assert!(inv.is_row_dirty(5));
        assert!(inv.is_row_dirty(9));
        assert!(!inv.is_row_dirty(4));
        assert!(!inv.is_row_dirty(10));
    }

    #[test]
    fn mark_all_dirty() {
        let mut inv = FrameInvalidation::new(24);
        inv.mark_all_dirty();
        assert_eq!(inv.level(), InvalidationLevel::Geometry);
        for i in 0..24 {
            assert!(inv.is_row_dirty(i));
        }
    }

    #[test]
    fn mark_full_rebuild() {
        let mut inv = FrameInvalidation::new(24);
        inv.mark_cursor_moved();
        inv.mark_full_rebuild();
        assert_eq!(inv.level(), InvalidationLevel::Full);
    }

    #[test]
    fn reset_keeps_cursor_level() {
        let mut inv = FrameInvalidation::new(24);
        inv.mark_full_rebuild();
        inv.reset();
        assert_eq!(inv.level(), InvalidationLevel::CursorOnly);
        assert!(!inv.cursor_moved());
    }

    #[test]
    fn dirty_rows_collected() {
        let mut inv = FrameInvalidation::new(10);
        inv.mark_rows_dirty(2, 5);
        inv.mark_rows_dirty(7, 8);
        let dirty = inv.dirty_rows();
        assert_eq!(dirty, vec![2, 3, 4, 7]);
    }

    #[test]
    fn row_out_of_range_ignored() {
        let mut inv = FrameInvalidation::new(10);
        inv.mark_rows_dirty(8, 20); // end > rows
        assert!(inv.is_row_dirty(8));
        assert!(inv.is_row_dirty(9));
        assert!(!inv.is_row_dirty(10));
    }

    #[test]
    fn selection_change_uses_cells_level() {
        let mut inv = FrameInvalidation::new(24);
        inv.mark_selection_changed();
        assert_eq!(inv.level(), InvalidationLevel::Cells);
        assert!(inv.selection_changed());
    }
}
