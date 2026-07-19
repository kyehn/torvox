//! Selection computation for the bridge layer.
//!
//! The bridge's selection boltffi methods all share the same shape: take a
//! terminal snapshot, read cell codepoints through a `cell_at` closure, expand a
//! `Selection` to its full extent, and build a GPU `SelectionRange`. This module
//! owns that pure computation so it can be unit-tested without a `TorvoxBridge`,
//! an `AndroidSurface`, or a GPU context — improving locality (selection math in
//! one place) and leverage (snapshot-driven tests).

use torvox_core::selection::{ExpansionOptions, Selection, SelectionAnchor, SelectionMode};
use torvox_renderer::gpu::SelectionRange;
use torvox_terminal::ghostty_terminal::CellSnapshot;

/// Build a closure that reads the primary codepoint of a grid cell.
///
/// Lives here so the two call sites (`expand_and_set_selection`,
/// `set_selection_endpoint`) share one definition instead of re-deriving it.
pub(crate) fn cell_at_fn(
    cells: &[CellSnapshot],
    cols: u32,
) -> impl Fn(u32, u32) -> Option<char> + '_ {
    move |row, col| {
        let idx = (row * cols + col) as usize;
        cells.get(idx).and_then(|c| char::from_u32(c.codepoint))
    }
}

/// Expand a selection anchored at `anchor` to its full extent for `mode`.
///
/// Pure: given a cell buffer and a starting anchor, returns the ordered
/// (start, end) anchors. No surface, session, or GPU access required.
pub(crate) fn expand_at(
    cells: &[CellSnapshot],
    cols: u32,
    anchor: SelectionAnchor,
    mode: SelectionMode,
    options: ExpansionOptions,
) -> (SelectionAnchor, SelectionAnchor) {
    let cell_at = cell_at_fn(cells, cols);
    Selection::new(anchor, anchor, mode)
        .expand(cell_at, options)
        .ordered()
}

/// Construct a GPU `SelectionRange` from two ordered anchors.
pub(crate) fn range_from(
    start: SelectionAnchor,
    end: SelectionAnchor,
    mode: SelectionMode,
    origin: Option<(i32, i32)>,
    is_empty: bool,
) -> SelectionRange {
    SelectionRange {
        start_row: start.row as i32,
        start_col: start.col as i32,
        end_row: end.row as i32,
        end_col: end.col as i32,
        active: true,
        mode,
        origin,
        is_empty,
    }
}

/// Check if a cell position contains text (non-null, non-whitespace).
pub(crate) fn contains_text(cells: &[CellSnapshot], cols: u32, row: u32, col: u32) -> bool {
    let cell_at = cell_at_fn(cells, cols);
    matches!(cell_at(row, col), Some(ch) if ch != '\0' && !ch.is_whitespace())
}

/// Check if a single cell position is empty (null char or absent).
pub(crate) fn is_position_empty(cells: &[CellSnapshot], cols: u32, row: u32, col: u32) -> bool {
    let cell_at = cell_at_fn(cells, cols);
    matches!(cell_at(row, col), None | Some('\0'))
}

/// Expand a single selection endpoint in Word mode.
pub(crate) fn expand_endpoint(
    cells: &[CellSnapshot],
    cols: u32,
    anchor: SelectionAnchor,
    mode: SelectionMode,
    options: ExpansionOptions,
) -> (SelectionAnchor, SelectionAnchor) {
    let cell_at = cell_at_fn(cells, cols);
    Selection::new(anchor, anchor, mode)
        .expand_word(&cell_at, options)
        .ordered()
}

#[cfg(test)]
mod tests {
    use super::*;
    use torvox_core::selection::SelectionMode;

    fn snapshot_with(text: &str) -> (Vec<CellSnapshot>, u32) {
        let cols = text.chars().count() as u32;
        let cells = text
            .chars()
            .map(|c| CellSnapshot {
                codepoint: c as u32,
                ..Default::default()
            })
            .collect();
        (cells, cols)
    }

    #[test]
    fn expand_word_stops_at_space() {
        let (cells, cols) = snapshot_with("hello world");
        let anchor = SelectionAnchor { row: 0, col: 2 };
        let (start, end) = expand_at(
            &cells,
            cols,
            anchor,
            SelectionMode::Word,
            ExpansionOptions::default(),
        );
        assert_eq!(start, SelectionAnchor { row: 0, col: 0 });
        assert_eq!(end, SelectionAnchor { row: 0, col: 4 });
    }

    #[test]
    fn expand_char_is_single_cell() {
        let (cells, cols) = snapshot_with("abc");
        let anchor = SelectionAnchor { row: 0, col: 1 };
        let (start, end) = expand_at(
            &cells,
            cols,
            anchor,
            SelectionMode::Char,
            ExpansionOptions::default(),
        );
        assert_eq!(start, anchor);
        assert_eq!(end, anchor);
    }

    #[test]
    fn range_from_maps_anchors_to_i32() {
        let range = range_from(
            SelectionAnchor { row: 1, col: 2 },
            SelectionAnchor { row: 3, col: 4 },
            SelectionMode::Line,
            Some((0, 0)),
            false,
        );
        assert_eq!(range.start_row, 1);
        assert_eq!(range.start_col, 2);
        assert_eq!(range.end_row, 3);
        assert_eq!(range.end_col, 4);
        assert_eq!(range.mode, SelectionMode::Line);
        assert_eq!(range.origin, Some((0, 0)));
        assert!(!range.is_empty);
    }

    #[test]
    fn contains_text_with_text_char_returns_true() {
        let (cells, cols) = snapshot_with("abc");
        assert!(contains_text(&cells, cols, 0, 1));
    }

    #[test]
    fn contains_text_with_empty_char_returns_false() {
        let cells = vec![CellSnapshot::default()];
        assert!(!contains_text(&cells, 1, 0, 0));
    }

    #[test]
    fn is_position_empty_with_null_char_returns_true() {
        let cells = vec![CellSnapshot::default()];
        assert!(is_position_empty(&cells, 1, 0, 0));
    }

    #[test]
    fn expand_at_with_word_mode_uses_options() {
        let (cells, cols) = snapshot_with("hello world");
        let anchor = SelectionAnchor { row: 0, col: 3 };
        let (start, end) = expand_at(
            &cells,
            cols,
            anchor,
            SelectionMode::Word,
            ExpansionOptions {
                bridge_whitespace: false,
                ..ExpansionOptions::default()
            },
        );
        assert_eq!(start, SelectionAnchor { row: 0, col: 0 });
        assert_eq!(end, SelectionAnchor { row: 0, col: 4 });
    }

    #[test]
    fn expand_endpoint_on_word_expands_both_sides() {
        let (cells, cols) = snapshot_with("hello");
        let anchor = SelectionAnchor { row: 0, col: 2 };
        let (s, e) = expand_endpoint(
            &cells,
            cols,
            anchor,
            SelectionMode::Word,
            ExpansionOptions::default(),
        );
        assert_eq!(s, SelectionAnchor { row: 0, col: 0 });
        assert_eq!(e, SelectionAnchor { row: 0, col: 4 });
    }
}
