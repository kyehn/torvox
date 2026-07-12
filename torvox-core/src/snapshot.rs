//! Rkyv-serializable snapshot for the Android bridge.
//!
//! # Requirements
//! - [FR-050](crate) — Bridge: rkyv serialization
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::grid::Grid;
use crate::line::Line;

#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub visible_lines: Vec<Line>,
    pub scrollback_lines: Vec<Line>,
    pub rows: u32,
    pub cols: u32,
    pub max_scrollback: usize,
}

impl SessionSnapshot {
    pub fn from_grid(grid: &Grid) -> Self {
        let mut visible_lines = Vec::with_capacity(grid.rows() as usize);
        for r in 0..grid.rows() {
            visible_lines.push(
                grid.get(r)
                    .cloned()
                    .unwrap_or_else(|| Line::new(grid.cols())),
            );
        }
        let scrollback_length = grid.scrollback_length();
        let mut scrollback_lines = Vec::with_capacity(scrollback_length);
        for i in 0..scrollback_length {
            scrollback_lines.push(
                grid.scrollback_line(i)
                    .cloned()
                    .unwrap_or_else(|| Line::new(grid.cols())),
            );
        }
        Self {
            visible_lines,
            scrollback_lines,
            rows: grid.rows(),
            cols: grid.cols(),
            max_scrollback: grid.max_scrollback(),
        }
    }

    pub fn apply_to_scrollback(&self, grid: &mut Grid, max_lines: usize) {
        let total = self.scrollback_lines.len() + self.visible_lines.len();
        let keep = total.min(max_lines);
        let mut all: Vec<Line> = Vec::with_capacity(total);
        all.extend(self.scrollback_lines.iter().cloned());
        all.extend(self.visible_lines.iter().cloned());
        let start = total.saturating_sub(keep);
        for line in all.iter().skip(start) {
            grid.push_scrollback(line.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn session_snapshot_from_grid_roundtrip() {
        let mut grid = Grid::new(24, 80);
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'X';
        grid.get_mut(5).unwrap().get_mut(10).unwrap().char = 'Y';
        grid.fill_cells(0, 'A', 0, 80);
        grid.scroll_up(0, 24, 80);
        grid.scroll_up(0, 24, 80);

        let snapshot = SessionSnapshot::from_grid(&grid);
        assert_eq!(snapshot.rows, 24);
        assert_eq!(snapshot.cols, 80);
        assert!(!snapshot.visible_lines.is_empty());
        assert_eq!(snapshot.scrollback_lines.len(), 2);
    }

    #[test]
    fn session_snapshot_apply_to_scrollback() {
        let mut grid = Grid::new(24, 80);
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'Z';
        grid.scroll_up(0, 24, 80);

        let snapshot = SessionSnapshot::from_grid(&grid);
        let mut restored = Grid::new(24, 80);
        snapshot.apply_to_scrollback(&mut restored, 1000);

        assert!(restored.scrollback_length() > 0);
        let first = restored.scrollback_line(0).unwrap().get(0).unwrap();
        assert_eq!(first.char, 'Z');
    }

    #[cfg(feature = "rkyv")]
    #[test]
    fn session_snapshot_rkyv_roundtrip() {
        use rkyv::rancor::Error;

        let mut grid = Grid::new(24, 80);
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'R';
        grid.scroll_up(0, 24, 80);

        let snapshot = SessionSnapshot::from_grid(&grid);
        let bytes = rkyv::to_bytes::<Error>(&snapshot).expect("rkyv serialize failed");
        let archived = rkyv::access::<<SessionSnapshot as rkyv::Archive>::Archived, Error>(&bytes)
            .expect("rkyv access failed");
        let restored =
            rkyv::deserialize::<SessionSnapshot, Error>(archived).expect("rkyv deserialize failed");
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn snapshot_empty_grid() {
        let grid = Grid::new(5, 10);
        let snap = SessionSnapshot::from_grid(&grid);
        assert_eq!(snap.rows, 5);
        assert_eq!(snap.cols, 10);
        assert_eq!(snap.visible_lines.len(), 5);
        assert_eq!(snap.scrollback_lines.len(), 0);
    }

    #[test]
    fn snapshot_captures_visible_chars() {
        let mut grid = Grid::new(3, 5);
        grid.get_mut(1).unwrap().get_mut(2).unwrap().char = 'Q';
        let snap = SessionSnapshot::from_grid(&grid);
        assert_eq!(snap.visible_lines[1].get(2).unwrap().char, 'Q');
    }

    #[test]
    fn snapshot_captures_scrollback() {
        let mut grid = Grid::new(2, 5);
        for i in 0..5 {
            grid.fill_cells(0, (b'A' + i) as char, 0, 5);
            grid.scroll_up(0, 2, 5);
        }
        let snap = SessionSnapshot::from_grid(&grid);
        assert_eq!(snap.scrollback_lines.len(), 5);
    }

    #[test]
    fn snapshot_serde_json_roundtrip() {
        let mut grid = Grid::new(3, 5);
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'X';
        let snap = SessionSnapshot::from_grid(&grid);
        let json = serde_json::to_string(&snap).expect("ser");
        let back: SessionSnapshot = serde_json::from_str(&json).expect("de");
        assert_eq!(snap, back);
    }

    #[test]
    fn snapshot_preserves_max_scrollback() {
        let grid = Grid::with_scrollback(5, 10, 1234);
        let snap = SessionSnapshot::from_grid(&grid);
        assert_eq!(snap.max_scrollback, 1234);
    }

    #[test]
    fn snapshot_apply_to_scrollback_appends_visible_and_scrollback() {
        let mut grid = Grid::new(2, 3);
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'A';
        grid.scroll_up(0, 2, 3); // A goes into scrollback
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'B';
        grid.scroll_up(0, 2, 3); // B goes into scrollback
        // The grid top is now empty (newly allocated row); the two prior rows are
        // in scrollback as 'A' (oldest) and 'B' (newest).
        let snap = SessionSnapshot::from_grid(&grid);
        let mut restored = Grid::new(2, 3);
        snap.apply_to_scrollback(&mut restored, 1000);
        // 2 scrollback lines + 2 visible lines = 4 lines restored
        assert_eq!(restored.scrollback_length(), 4);
    }

    #[test]
    fn snapshot_apply_to_scrollback_respects_max() {
        let mut grid = Grid::new(2, 3);
        for i in 0..10 {
            grid.fill_cells(0, (b'A' + i) as char, 0, 3);
            grid.scroll_up(0, 2, 3);
        }
        let snap = SessionSnapshot::from_grid(&grid);
        let mut restored = Grid::with_scrollback(2, 3, 5);
        snap.apply_to_scrollback(&mut restored, 3);
        // Scrollback at most 3 lines
        assert!(restored.scrollback_length() <= 3);
    }

    #[test]
    fn snapshot_apply_to_scrollback_zero_max_keeps_nothing() {
        let mut grid = Grid::new(2, 3);
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'Z';
        let snap = SessionSnapshot::from_grid(&grid);
        let mut restored = Grid::new(2, 3);
        snap.apply_to_scrollback(&mut restored, 0);
        assert_eq!(restored.scrollback_length(), 0);
    }

    #[test]
    fn snapshot_apply_to_scrollback_evicts_oldest() {
        let mut grid = Grid::new(2, 3);
        for i in 0..5 {
            grid.fill_cells(0, (b'A' + i) as char, 0, 3);
            grid.scroll_up(0, 2, 3);
        }
        let snap = SessionSnapshot::from_grid(&grid);
        let mut restored = Grid::with_scrollback(2, 3, 3);
        snap.apply_to_scrollback(&mut restored, 3);
        // Should keep the 3 most recent lines
        // Snapshot has 3 scrollback lines + 2 visible lines = 5 total lines
        // Applied with max=3, keeps the last 3 lines
        assert_eq!(restored.scrollback_length(), 3);
    }

    #[test]
    fn snapshot_visible_lines_match_grid() {
        let mut grid = Grid::new(2, 3);
        grid.fill_cells(0, 'X', 0, 3);
        grid.fill_cells(1, 'Y', 0, 3);
        let snap = SessionSnapshot::from_grid(&grid);
        for r in 0..2 {
            for c in 0..3 {
                assert_eq!(
                    snap.visible_lines[r as usize].get(c).unwrap().char,
                    grid.cell(r, c).unwrap().char
                );
            }
        }
    }

    #[test]
    fn snapshot_equality_requires_same_grid_state() {
        let mut grid1 = Grid::new(3, 5);
        grid1.get_mut(0).unwrap().get_mut(0).unwrap().char = 'X';
        let snap1 = SessionSnapshot::from_grid(&grid1);

        let mut grid2 = Grid::new(3, 5);
        grid2.get_mut(0).unwrap().get_mut(0).unwrap().char = 'Y';
        let snap2 = SessionSnapshot::from_grid(&grid2);

        assert_ne!(
            snap1, snap2,
            "different grid content should produce different snapshots"
        );

        let snap3 = SessionSnapshot::from_grid(&grid1);
        assert_eq!(
            snap1, snap3,
            "same grid state should produce equal snapshots"
        );
    }

    #[test]
    fn snapshot_clone_independence() {
        let mut grid = Grid::new(3, 5);
        grid.get_mut(0).unwrap().get_mut(0).unwrap().char = 'X';
        let original = SessionSnapshot::from_grid(&grid);
        let cloned = original.clone();
        assert_eq!(original, cloned);

        // Verify clone is independent (can't modify original since it's Copy-safe via Clone)
        assert_eq!(cloned.visible_lines.len(), original.visible_lines.len());
    }
}
