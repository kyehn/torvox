//! Selection modes — character, word, line, and block selection.
//!
//! # Requirements
//! - [FR-015](crate) — Selection: render highlight
//! - [FR-022](crate) — Selection: word boundary
//! - [FR-023](crate) — Selection: line-at-a-time
use serde::{Deserialize, Serialize};

/// Returns true when `character` is a CJK ideograph or kana/hangul syllable.
///
/// CJK text has no inter-word spaces, so each glyph must be treated as part
/// of a "word" for selection purposes — otherwise long-press on a single CJK
/// glyph would only ever select that one character. Covers the common Unified
/// CJK, extension-A, and the Hiragana/Katakana/Hangul syllable ranges.
fn char_is_cjk(character: char) -> bool {
    // Unified CJK Ideographs
    ('\u{3400}'..='\u{4dbf}').contains(&character)
        || ('\u{4e00}'..='\u{9fff}').contains(&character)
        // CJK Unified Ideographs Extension A
        || ('\u{f900}'..='\u{faff}').contains(&character)
        // Hiragana + Katakana
        || ('\u{3040}'..='\u{30ff}').contains(&character)
        // Hangul Syllables
        || ('\u{ac00}'..='\u{d7a3}').contains(&character)
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || char_is_cjk(character)
}

fn is_url_safe(character: char) -> bool {
    character.is_alphanumeric()
        || matches!(
            character,
            '/' | ':'
                | '?'
                | '#'
                | '@'
                | '!'
                | '$'
                | '&'
                | '('
                | ')'
                | '*'
                | '+'
                | ','
                | ';'
                | '='
                | '.'
                | '_'
                | '~'
                | '%'
                | '-'
                | '['
                | ']'
        )
        // CJK glyphs inside a URL (e.g. IDN / punycode-adjacent text) must not
        // break the URL scan.
        || char_is_cjk(character)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum SelectionMode {
    #[default]
    Char,
    Word,
    Line,
    Block,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
/// A single endpoint of a selection (row, col).
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct SelectionAnchor {
    /// Anchor row position (0-indexed).
    pub row: u32,
    /// Anchor column position (0-indexed).
    pub col: u32,
}

/// A selection with start, end anchors and mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
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

    /// Extract selected text from a grid.
    pub fn text(&self, grid: &crate::grid::Grid) -> alloc::string::String {
        let (lo, hi) = self.ordered();
        let mut result = alloc::string::String::new();
        match self.mode {
            SelectionMode::Char | SelectionMode::Word => {
                for row in lo.row..=hi.row {
                    if let Some(cells) = grid.row_cells(row) {
                        if cells.is_empty() {
                            continue;
                        }
                        let start_col = if row == lo.row { lo.col } else { 0 };
                        let end_col = if row == hi.row {
                            hi.col
                        } else {
                            cells.len() as u32 - 1
                        };
                        let mut row_str = alloc::string::String::new();
                        for col in start_col..=end_col {
                            if let Some(cell) = cells.get(col as usize)
                                && cell.char != '\0'
                            {
                                row_str.push(cell.char);
                            }
                        }
                        if row < hi.row {
                            row_str.push('\n');
                        }
                        result.push_str(&row_str);
                    }
                }
            }
            SelectionMode::Line => {
                for row in lo.row..=hi.row {
                    if let Some(cells) = grid.row_cells(row) {
                        let text: alloc::string::String = cells
                            .iter()
                            .map(|c| if c.char == '\0' { ' ' } else { c.char })
                            .collect();
                        result.push_str(text.trim_end());
                        if row < hi.row {
                            result.push('\n');
                        }
                    }
                }
            }
            SelectionMode::Block => {
                for row in lo.row..=hi.row {
                    if let Some(cells) = grid.row_cells(row) {
                        for col in lo.col..=hi.col {
                            if let Some(cell) = cells.get(col as usize) {
                                result.push(if cell.char == '\0' { ' ' } else { cell.char });
                            }
                        }
                        if row < hi.row {
                            result.push('\n');
                        }
                    }
                }
            }
        }
        result
    }
}

impl Selection {
    /// Expand word boundaries around the start anchor.
    ///
    /// Word characters are `[A-Za-z0-9_]` plus CJK ideographs/kana/hangul. To
    /// avoid merging a CJK glyph with an adjacent Latin word (e.g. selecting
    /// `本` in `abc日本語def` should only grab `日本語`, not `abc日本語def`), the
    /// expansion keeps the run within a single script class: once the seed
    /// character is CJK, only other CJK glyphs extend the word, and vice-versa.
    pub fn expand_word(mut self, cell_at: impl Fn(u32, u32) -> Option<char>) -> Self {
        let seed_is_cjk = cell_at(self.start.row, self.start.col)
            .map(char_is_cjk)
            .unwrap_or(false);
        let same_script = |ch: char| is_word_char(ch) && char_is_cjk(ch) == seed_is_cjk;
        let mut left = self.start.col;
        while left > 0 {
            if let Some(ch) = cell_at(self.start.row, left - 1) {
                if same_script(ch) {
                    left -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        let mut right = self.end.col;
        while let Some(ch) = cell_at(self.end.row, right + 1) {
            if same_script(ch) {
                right += 1;
            } else {
                break;
            }
        }
        self.start.col = left;
        self.end.col = right;
        self
    }

    /// Expand to cover a full URL at the selection start, with cross-row wrap detection.
    pub fn expand_url(mut self, cell_at: impl Fn(u32, u32) -> Option<char>) -> Self {
        let start = self.start;
        let max_lookback = 50u32.min(start.col);
        let lookback_start = start.col.saturating_sub(max_lookback);
        let lookahead = 10u32; // enough to capture "https://" + one char
        let scan_end = start.col + lookahead;
        let mut buf = alloc::vec::Vec::new();
        for c in lookback_start..=scan_end {
            if let Some(ch) = cell_at(start.row, c) {
                buf.push(ch);
            }
        }
        let text: alloc::string::String = buf.iter().collect();
        let prefix_pos = text
            .rfind("https://")
            .or_else(|| text.rfind("http://"))
            .or_else(|| text.rfind("www."))
            .or_else(|| text.rfind("ftp://"));
        if let Some(pos) = prefix_pos {
            self.start.col = lookback_start + pos as u32;
        } else {
            return self;
        }

        let mut row = self.end.row;
        let mut col = self.end.col;
        const URL_SCAN_LIMIT: u32 = 200;
        for _ in 0..URL_SCAN_LIMIT {
            match cell_at(row, col + 1) {
                Some(ch) if is_url_safe(ch) => {
                    col += 1;
                }
                Some(ch) if ch == '\0' || ch == ' ' => {
                    if let Some(next_ch) = cell_at(row + 1, 0) {
                        if is_url_safe(next_ch) {
                            row += 1;
                            col = 0;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        self.end.row = row;
        self.end.col = col;

        // Strip trailing sentence punctuation that belongs to prose surrounding
        // the URL (e.g. the trailing '.' in "see https://example.com."). We only
        // drop a trailing '.', ',' or ';' when the character immediately before
        // it is itself URL-safe (i.e. it is genuinely trailing, not an internal
        // separator like the '.' in "example.com"). We never shrink past the
        // selected start column.
        loop {
            if self.end.col == 0 && self.end.row == self.start.row {
                break;
            }
            let (prev_row, prev_col) = if self.end.col > 0 {
                (self.end.row, self.end.col - 1)
            } else if self.end.row > self.start.row {
                (self.end.row - 1, self.end.col)
            } else {
                break;
            };
            let trailing = cell_at(self.end.row, self.end.col);
            let is_trailing_punct = matches!(trailing, Some('.' | ',' | ';'));
            if !is_trailing_punct {
                break;
            }
            let prev_is_safe = cell_at(prev_row, prev_col)
                .map(is_url_safe)
                .unwrap_or(false);
            if !prev_is_safe {
                break;
            }
            if self.end.row == self.start.row && self.end.col <= self.start.col {
                break;
            }
            if self.end.col > 0 {
                self.end.col -= 1;
            } else {
                self.end.row -= 1;
            }
        }
        self
    }

    /// Expand the selection according to its mode.
    /// For Word mode, expands to word boundaries then tries URL detection.
    /// For other modes, returns self unchanged.
    pub fn expand(self, cell_at: impl Fn(u32, u32) -> Option<char>) -> Self {
        match self.mode {
            SelectionMode::Word => {
                let expanded = self.expand_word(&cell_at);
                expanded.expand_url(cell_at)
            }
            SelectionMode::Char | SelectionMode::Line | SelectionMode::Block => self,
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

    #[test]
    fn selection_mode_default_is_char() {
        assert_eq!(SelectionMode::default(), SelectionMode::Char);
    }

    #[test]
    fn selection_all_modes_distinct() {
        assert_ne!(SelectionMode::Char, SelectionMode::Word);
        assert_ne!(SelectionMode::Char, SelectionMode::Line);
        assert_ne!(SelectionMode::Char, SelectionMode::Block);
        assert_ne!(SelectionMode::Word, SelectionMode::Line);
        assert_ne!(SelectionMode::Word, SelectionMode::Block);
        assert_ne!(SelectionMode::Line, SelectionMode::Block);
    }

    #[test]
    fn selection_word_mode_same_as_char() {
        // Word mode uses the same containment logic as char mode
        let s = Selection::new(
            SelectionAnchor { row: 1, col: 2 },
            SelectionAnchor { row: 3, col: 4 },
            SelectionMode::Word,
        );
        assert!(s.contains(2, 0));
        assert!(!s.contains(4, 0));
    }

    #[test]
    fn selection_line_mode_full_row() {
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 50 },
            SelectionAnchor { row: 0, col: 10 },
            SelectionMode::Line,
        );
        let (lo, hi) = s.ordered();
        assert_eq!(lo.row, 0);
        assert_eq!(hi.row, 0);
        assert!(s.contains(0, 0));
        assert!(s.contains(0, 1000));
    }

    #[test]
    fn selection_block_outside_rows() {
        let s = Selection::new(
            SelectionAnchor { row: 2, col: 5 },
            SelectionAnchor { row: 4, col: 10 },
            SelectionMode::Block,
        );
        assert!(!s.contains(1, 7));
        assert!(!s.contains(5, 7));
    }

    #[test]
    fn selection_block_inside_cols_outside_rows() {
        let s = Selection::new(
            SelectionAnchor { row: 2, col: 5 },
            SelectionAnchor { row: 4, col: 10 },
            SelectionMode::Block,
        );
        assert!(!s.contains(0, 7));
        assert!(!s.contains(10, 7));
    }

    #[test]
    fn selection_anchor_equality() {
        let a = SelectionAnchor { row: 1, col: 2 };
        let b = SelectionAnchor { row: 1, col: 2 };
        assert_eq!(a, b);
        let c = SelectionAnchor { row: 1, col: 3 };
        assert_ne!(a, c);
    }

    #[test]
    fn selection_anchor_copy() {
        let a = SelectionAnchor { row: 1, col: 2 };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn selection_serde_json_roundtrip() {
        let s = Selection::new(
            SelectionAnchor { row: 1, col: 2 },
            SelectionAnchor { row: 3, col: 4 },
            SelectionMode::Line,
        );
        let json = serde_json::to_string(&s).unwrap();
        let back: Selection = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn selection_ordered_when_start_equals_end() {
        let s = Selection::new(
            SelectionAnchor { row: 5, col: 5 },
            SelectionAnchor { row: 5, col: 5 },
            SelectionMode::Char,
        );
        assert!(s.is_ordered());
        let (lo, hi) = s.ordered();
        assert_eq!(lo, hi);
    }

    #[test]
    fn selection_char_contains_single_cell() {
        let s = Selection::new(
            SelectionAnchor { row: 5, col: 5 },
            SelectionAnchor { row: 5, col: 5 },
            SelectionMode::Char,
        );
        assert!(s.contains(5, 5));
        assert!(!s.contains(5, 4));
        assert!(!s.contains(5, 6));
    }

    #[test]
    fn selection_mode_serde() {
        for mode in [
            SelectionMode::Char,
            SelectionMode::Word,
            SelectionMode::Line,
            SelectionMode::Block,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: SelectionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn selection_is_ordered_just_equals_col() {
        // same row, end col equal to start col
        let s = Selection::new(
            SelectionAnchor { row: 5, col: 10 },
            SelectionAnchor { row: 5, col: 10 },
            SelectionMode::Char,
        );
        assert!(s.is_ordered());
    }

    #[test]
    fn selection_ordered_end_before_start_swaps() {
        let s = Selection::new(
            SelectionAnchor { row: 5, col: 10 },
            SelectionAnchor { row: 3, col: 5 },
            SelectionMode::Char,
        );
        let (lo, hi) = s.ordered();
        assert_eq!(lo.row, 3);
        assert_eq!(hi.row, 5);
    }

    #[test]
    fn selection_block_contains_middle_row() {
        let s = Selection::new(
            SelectionAnchor { row: 2, col: 5 },
            SelectionAnchor { row: 4, col: 10 },
            SelectionMode::Block,
        );
        assert!(s.contains(3, 7));
        assert!(!s.contains(3, 3));
    }

    fn make_grid_with_text(lines: &[&str]) -> crate::grid::Grid {
        use crate::cell::Cell;
        let rows = lines.len() as u32;
        let cols = lines.iter().map(|l| l.len()).max().unwrap_or(1) as u32;
        let mut grid = crate::grid::Grid::new(rows, cols);
        for (row_idx, line) in lines.iter().enumerate() {
            for (col_idx, ch) in line.chars().enumerate() {
                if let Some(cell) = grid.cell_mut(row_idx as u32, col_idx as u32) {
                    *cell = Cell {
                        char: ch,
                        ..Default::default()
                    };
                }
            }
        }
        grid
    }

    #[test]
    fn char_text_extraction_single_line() {
        let grid = make_grid_with_text(&["Hello, World!"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 4 },
            SelectionMode::Char,
        );
        assert_eq!(s.text(&grid), "Hello");
    }

    #[test]
    fn char_text_extraction_multi_line() {
        let grid = make_grid_with_text(&["First line", "Second line"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 6 },
            SelectionAnchor { row: 1, col: 5 },
            SelectionMode::Char,
        );
        let result = s.text(&grid);
        // Space at end of "line " is part of the selected area (col 9 <= hi.col=9 on row 0)
        assert!(
            result.starts_with("line"),
            "should extract 'line' from first row, got: {result:?}"
        );
        assert!(
            result.ends_with("Second"),
            "should end with 'Second' from second row, got: {result:?}"
        );
        assert!(
            result.contains('\n'),
            "should have newline between rows, got: {result:?}"
        );
    }

    #[test]
    fn word_text_extraction() {
        let grid = make_grid_with_text(&["Hello World"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 4 },
            SelectionMode::Word,
        );
        assert_eq!(s.text(&grid), "Hello");
    }

    #[test]
    fn line_text_extraction() {
        let grid = make_grid_with_text(&["Hello", "World", "Test"]);
        let s = Selection::new(
            SelectionAnchor { row: 1, col: 2 },
            SelectionAnchor { row: 2, col: 1 },
            SelectionMode::Line,
        );
        let result = s.text(&grid);
        assert_eq!(result, "World\nTest");
    }

    #[test]
    fn block_text_extraction() {
        let grid = make_grid_with_text(&["ABCDEFGHIJ", "0123456789"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 2 },
            SelectionAnchor { row: 1, col: 5 },
            SelectionMode::Block,
        );
        assert_eq!(s.text(&grid), "CDEF\n2345");
    }

    #[test]
    fn block_text_extraction_single_cell() {
        let grid = make_grid_with_text(&["ABCD"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 },
            SelectionAnchor { row: 0, col: 1 },
            SelectionMode::Block,
        );
        assert_eq!(s.text(&grid), "B");
    }

    #[test]
    fn text_extraction_reversed_selection() {
        let grid = make_grid_with_text(&["ABC"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 3 },
            SelectionAnchor { row: 0, col: 0 },
            SelectionMode::Char,
        );
        assert_eq!(s.text(&grid), "ABC");
    }

    #[test]
    fn char_text_extraction_empty_row() {
        let grid = crate::grid::Grid::new(2, 10);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 1, col: 5 },
            SelectionMode::Char,
        );
        let result = s.text(&grid);
        // Grid default cells have char=' ' (0x20). With trailing-space preservation
        // and null filtering, spaces pass through. Contains newline between rows.
        assert!(
            result.contains('\n'),
            "empty rows should have newline: {result:?}"
        );
    }

    // ── Expansion tests ──

    #[test]
    fn expand_word_left_boundary() {
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 6 }, // 'w' in "world"
            SelectionAnchor { row: 0, col: 6 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 6);
        assert_eq!(expanded.end.col, 10);
    }

    #[test]
    fn expand_word_multi_word() {
        let grid = make_grid_with_text(&["abc def ghi"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 4 }, // 'e' in "def"
            SelectionAnchor { row: 0, col: 4 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 4);
        assert_eq!(expanded.end.col, 6);
    }

    #[test]
    fn expand_word_underscore_contiguous() {
        let grid = make_grid_with_text(&["foo_bar_baz"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 4 }, // 'b' in "bar"
            SelectionAnchor { row: 0, col: 4 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.text(&grid), "foo_bar_baz");
    }

    #[test]
    fn expand_url_basic() {
        let grid = make_grid_with_text(&["visit https://example.com/path now"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 13 }, // ':' in https://
            SelectionAnchor { row: 0, col: 13 },
            SelectionMode::Word,
        );
        let expanded = s.expand_url(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 6); // 'h' in https
        assert_eq!(expanded.end.col, 29); // last char of /path (space at 30 stops)
    }

    #[test]
    fn expand_url_cross_row_wrap() {
        let grid =
            make_grid_with_text(&["https://example.com/long-", "url-continuation more text"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 10 },
            SelectionAnchor { row: 0, col: 10 },
            SelectionMode::Word,
        );
        let expanded = s.expand_url(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.end.row, 1);
        assert_eq!(expanded.end.col, 15); // 'n' at col 15 is last URL char, col 16 is space
    }

    #[test]
    fn expand_url_no_prefix_noop() {
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 6 },
            SelectionAnchor { row: 0, col: 6 },
            SelectionMode::Word,
        );
        let expanded = s.expand_url(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 6);
        assert_eq!(expanded.end.col, 6);
    }

    #[test]
    fn expand_dispatcher_word_mode() {
        let grid = make_grid_with_text(&["select this word"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 9 }, // middle of "this"
            SelectionAnchor { row: 0, col: 9 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 7);
        assert_eq!(expanded.end.col, 10);
    }

    #[test]
    fn expand_dispatcher_url_mode() {
        let grid = make_grid_with_text(&["click https://rust-lang.org now"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 8 }, // 'h' in https
            SelectionAnchor { row: 0, col: 8 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        // expand_word gives "https", then expand_url scans left and right
        assert_eq!(expanded.start.col, 6); // 'h' in https
        assert_eq!(expanded.end.col, 26); // end of "rust-lang.org" (space at 27 stops)
    }

    #[test]
    fn expand_char_mode_noop() {
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 6 },
            SelectionAnchor { row: 0, col: 6 },
            SelectionMode::Char,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 6);
        assert_eq!(expanded.end.col, 6);
    }

    #[test]
    fn expand_line_mode_noop() {
        let grid = make_grid_with_text(&["hello", "world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 2 },
            SelectionAnchor { row: 1, col: 3 },
            SelectionMode::Line,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 2);
        assert_eq!(expanded.end.col, 3);
    }

    #[test]
    fn expand_block_mode_noop() {
        let grid = make_grid_with_text(&["hello"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 },
            SelectionAnchor { row: 0, col: 3 },
            SelectionMode::Block,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 1);
        assert_eq!(expanded.end.col, 3);
    }

    // ── text() trailing space preservation ──

    #[test]
    fn char_text_preserves_trailing_spaces() {
        let grid = make_grid_with_text(&["hello     "]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 9 },
            SelectionMode::Char,
        );
        assert_eq!(s.text(&grid), "hello     ");
    }

    #[test]
    fn word_text_preserves_trailing_spaces() {
        let grid = make_grid_with_text(&["hello world   "]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 13 },
            SelectionMode::Word,
        );
        let result = s.text(&grid);
        assert!(
            result.contains("   "),
            "trailing spaces should be preserved: {result:?}"
        );
    }

    // ── Edge cases: empty grid, full grid, word boundaries ──

    #[test]
    fn selection_all_modes_empty_grid() {
        let grid = crate::grid::Grid::new(0, 0);
        for mode in [
            SelectionMode::Char,
            SelectionMode::Word,
            SelectionMode::Line,
            SelectionMode::Block,
        ] {
            let s = Selection::new(
                SelectionAnchor { row: 0, col: 0 },
                SelectionAnchor { row: 0, col: 0 },
                mode,
            );
            assert_eq!(
                s.text(&grid),
                "",
                "mode {mode:?} should return empty text on empty grid"
            );
        }
    }

    #[test]
    fn selection_char_full_grid() {
        let grid = make_grid_with_text(&["ABC", "DEF"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 1, col: 2 },
            SelectionMode::Char,
        );
        assert_eq!(s.text(&grid), "ABC\nDEF");
    }

    #[test]
    fn selection_line_full_grid() {
        let grid = make_grid_with_text(&["Hello", "World"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 1, col: 0 },
            SelectionMode::Line,
        );
        assert_eq!(s.text(&grid), "Hello\nWorld");
    }

    #[test]
    fn selection_block_full_grid() {
        let grid = make_grid_with_text(&["AB", "CD"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 1, col: 1 },
            SelectionMode::Block,
        );
        assert_eq!(s.text(&grid), "AB\nCD");
    }

    #[test]
    fn selection_word_start_of_line() {
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 4 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.text(&grid), "hello");
    }

    #[test]
    fn selection_word_end_of_line() {
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 6 },
            SelectionAnchor { row: 0, col: 10 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.text(&grid), "world");
    }

    #[test]
    fn is_word_char_alphanumeric() {
        assert!(is_word_char('a'));
        assert!(is_word_char('Z'));
        assert!(is_word_char('5'));
        assert!(is_word_char('_'));
    }

    #[test]
    fn is_word_char_non_alphanumeric() {
        assert!(!is_word_char(' '));
        assert!(!is_word_char('-'));
        assert!(!is_word_char('.'));
        assert!(!is_word_char('/'));
        assert!(!is_word_char('('));
    }

    #[test]
    fn is_word_char_all_digits() {
        for d in '0'..='9' {
            assert!(is_word_char(d), "digit {d:?} should be word char");
        }
    }

    #[test]
    fn is_word_char_all_lowercase() {
        for c in 'a'..='z' {
            assert!(is_word_char(c), "lowercase {c:?} should be word char");
        }
    }

    #[test]
    fn is_word_char_all_uppercase() {
        for c in 'A'..='Z' {
            assert!(is_word_char(c), "uppercase {c:?} should be word char");
        }
    }

    #[test]
    fn is_url_safe_common_chars() {
        assert!(is_url_safe('/'));
        assert!(is_url_safe(':'));
        assert!(is_url_safe('.'));
        assert!(is_url_safe('_'));
        assert!(is_url_safe('~'));
        assert!(is_url_safe('%'));
        assert!(!is_url_safe(' '));
        assert!(!is_url_safe('<'));
        assert!(!is_url_safe('>'));
        assert!(!is_url_safe('"'));
        assert!(!is_url_safe('\''));
    }

    #[test]
    fn block_selection_variable_line_width() {
        let grid = make_grid_with_text(&["ABCDE", "012"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 },
            SelectionAnchor { row: 1, col: 3 },
            SelectionMode::Block,
        );
        // Grid pads shorter rows with spaces, so row 1 col 3 is a space
        assert_eq!(s.text(&grid), "BCD\n12 ");
    }

    #[test]
    fn block_selection_single_column() {
        let grid = make_grid_with_text(&["ABC", "DEF", "GHI"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 },
            SelectionAnchor { row: 2, col: 1 },
            SelectionMode::Block,
        );
        assert_eq!(s.text(&grid), "B\nE\nH");
    }

    #[test]
    fn char_selection_cjk() {
        let grid = make_grid_with_text(&["日本語"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 2 },
            SelectionMode::Char,
        );
        assert_eq!(s.text(&grid), "日本語");
    }

    #[test]
    fn char_selection_mixed_ascii_cjk() {
        let grid = make_grid_with_text(&["Hello日本語World"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 5 },
            SelectionAnchor { row: 0, col: 7 },
            SelectionMode::Char,
        );
        assert_eq!(s.text(&grid), "日本語");
    }

    #[test]
    fn line_selection_reversed() {
        let grid = make_grid_with_text(&["line1", "line2", "line3"]);
        let s = Selection::new(
            SelectionAnchor { row: 2, col: 0 },
            SelectionAnchor { row: 0, col: 0 },
            SelectionMode::Line,
        );
        assert_eq!(s.text(&grid), "line1\nline2\nline3");
    }

    #[test]
    fn is_url_safe_all_valid_chars() {
        let safe_chars = "/:?#@!$&()*+,;=._~%-[]";
        for ch in safe_chars.chars() {
            assert!(is_url_safe(ch), "{ch:?} should be URL safe");
        }
    }

    #[test]
    fn is_url_safe_unsafe_chars() {
        let unsafe_chars = " <>\"'{}|\\^`";
        for ch in unsafe_chars.chars() {
            assert!(!is_url_safe(ch), "{ch:?} should NOT be URL safe");
        }
    }

    #[test]
    fn expand_word_at_line_start() {
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 0 },
            SelectionAnchor { row: 0, col: 0 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 4);
    }

    #[test]
    fn expand_word_single_word_line() {
        let grid = make_grid_with_text(&["entireline"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 3 },
            SelectionAnchor { row: 0, col: 3 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 9);
    }

    #[test]
    fn expand_word_at_end_of_line() {
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 10 },
            SelectionAnchor { row: 0, col: 10 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 6);
        assert_eq!(expanded.end.col, 10);
    }

    #[test]
    fn expand_url_no_prefix_leaves_unchanged() {
        let grid = make_grid_with_text(&["no url here"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 5 },
            SelectionAnchor { row: 0, col: 5 },
            SelectionMode::Word,
        );
        let expanded = s.expand_url(|r, c| grid.cell(r, c).map(|cell| cell.char));
        // expand_url only looks for URL prefix, if not found returns self
        // But expand dispatcher calls expand_word first then expand_url
        // For expand_word: "rl" is not a word boundary context
        assert_eq!(expanded.start.col, 5);
        assert_eq!(expanded.end.col, 5);
    }

    #[test]
    fn expand_url_http_variants() {
        let grid = make_grid_with_text(&["see http://example.com for info"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 5 },
            SelectionAnchor { row: 0, col: 5 },
            SelectionMode::Word,
        );
        let expanded = s.expand_url(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 4);
        assert_eq!(expanded.end.col, 21);
    }

    #[test]
    fn expand_url_www_prefix() {
        let grid = make_grid_with_text(&["www.example.com/path"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 },
            SelectionAnchor { row: 0, col: 1 },
            SelectionMode::Word,
        );
        let expanded = s.expand_url(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 0);
    }

    #[test]
    fn is_word_char_cjk() {
        // CJK glyphs must be treated as word characters so a long-press selects
        // the whole ideograph run instead of a single cell.
        assert!(is_word_char('日'));
        assert!(is_word_char('本'));
        assert!(is_word_char('語'));
        // Latin + CJK mixed must still be recognized as word chars.
        assert!(is_word_char('a'));
        assert!(is_word_char('5'));
    }

    #[test]
    fn is_word_char_not_whitespace() {
        assert!(!is_word_char(' '));
        assert!(!is_word_char('\t'));
    }

    #[test]
    fn is_url_safe_cjk() {
        assert!(is_url_safe('本'));
        assert!(is_url_safe('語'));
    }

    #[test]
    fn expand_word_cjk_selects_run() {
        let grid = make_grid_with_text(&["日本語テスト"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 }, // '本'
            SelectionAnchor { row: 0, col: 1 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        // Whole ideograph run should be selected, not a single glyph.
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 5);
        assert_eq!(expanded.text(&grid), "日本語テスト");
    }

    #[test]
    fn expand_word_cjk_then_ascii_boundary() {
        let grid = make_grid_with_text(&["abc日本語def"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 4 }, // '本'
            SelectionAnchor { row: 0, col: 4 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        // CJK run only; ASCII words on either side are separate boundaries.
        assert_eq!(expanded.start.col, 3);
        assert_eq!(expanded.end.col, 5);
        assert_eq!(expanded.text(&grid), "日本語");
    }

    #[test]
    fn expand_url_trailing_punctuation_stripped() {
        let grid = make_grid_with_text(&["see https://example.com. now"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 8 }, // 'h' in https
            SelectionAnchor { row: 0, col: 8 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 4);
        // Trailing '.' is stripped; selection ends at 'm'.
        assert_eq!(expanded.end.col, 22);
        assert_eq!(expanded.text(&grid), "https://example.com");
    }

    #[test]
    fn expand_url_trailing_comma_stripped() {
        let grid = make_grid_with_text(&["open https://example.com, please"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 5 },
            SelectionAnchor { row: 0, col: 5 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        // The comma is not URL-safe, so the forward scan already stops at 'm'.
        assert_eq!(expanded.end.col, 23);
        assert_eq!(expanded.text(&grid), "https://example.com");
    }

    #[test]
    fn expand_url_keeps_internal_dot() {
        let grid = make_grid_with_text(&["goto https://example.com/path"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 5 },
            SelectionAnchor { row: 0, col: 5 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        // Internal dots are preserved.
        assert_eq!(expanded.text(&grid), "https://example.com/path");
    }

    #[test]
    fn expand_url_trailing_semicolon_stripped() {
        let grid = make_grid_with_text(&["visit https://example.com; done"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 6 },
            SelectionAnchor { row: 0, col: 6 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.end.col, 24);
        assert_eq!(expanded.text(&grid), "https://example.com");
    }

    #[test]
    fn selection_contains_char_same_row() {
        let s = Selection::new(
            SelectionAnchor { row: 3, col: 5 },
            SelectionAnchor { row: 3, col: 10 },
            SelectionMode::Char,
        );
        assert!(s.contains(3, 5));
        assert!(s.contains(3, 10));
        assert!(!s.contains(3, 4));
        assert!(!s.contains(3, 11));
    }

    #[test]
    fn selection_contains_word_same_row() {
        let s = Selection::new(
            SelectionAnchor { row: 1, col: 2 },
            SelectionAnchor { row: 1, col: 5 },
            SelectionMode::Word,
        );
        assert!(s.contains(1, 2));
        assert!(s.contains(1, 5));
        assert!(!s.contains(1, 1));
        assert!(!s.contains(1, 6));
    }

    #[test]
    fn selection_contains_line_multi_row() {
        let s = Selection::new(
            SelectionAnchor { row: 1, col: 0 },
            SelectionAnchor { row: 3, col: 0 },
            SelectionMode::Line,
        );
        assert!(s.contains(1, 100));
        assert!(s.contains(2, 0));
        assert!(s.contains(3, 50));
        assert!(!s.contains(0, 0));
        assert!(!s.contains(4, 0));
    }

    #[test]
    fn selection_contains_block_multi_row_multi_col() {
        let s = Selection::new(
            SelectionAnchor { row: 1, col: 2 },
            SelectionAnchor { row: 3, col: 5 },
            SelectionMode::Block,
        );
        assert!(s.contains(1, 2));
        assert!(s.contains(1, 5));
        assert!(s.contains(3, 2));
        assert!(s.contains(3, 5));
        assert!(s.contains(2, 3));
        assert!(!s.contains(0, 3));
        assert!(!s.contains(4, 3));
        assert!(!s.contains(2, 1));
        assert!(!s.contains(2, 6));
    }

    #[test]
    fn block_selection_same_row() {
        let grid = make_grid_with_text(&["ABCD"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 },
            SelectionAnchor { row: 0, col: 2 },
            SelectionMode::Block,
        );
        assert_eq!(s.text(&grid), "BC");
    }

    // ── Whitespace / mixed expansion ──

    #[test]
    fn expand_word_on_whitespace_expands_to_adjacent_words() {
        // Long-pressing the whitespace between two words seeds a non-word char,
        // but expand_word walks outward to the word chars on each side, so the
        // whole "hello world" run is selected.
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 5 }, // the space between the words
            SelectionAnchor { row: 0, col: 5 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 10);
        assert_eq!(expanded.text(&grid), "hello world");
    }

    #[test]
    fn expand_word_on_leading_whitespace() {
        // A whitespace seed with no word char to its left expands rightward only.
        let grid = make_grid_with_text(&["  hello"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 }, // second leading space
            SelectionAnchor { row: 0, col: 1 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 1);
        assert_eq!(expanded.end.col, 6);
        assert_eq!(expanded.text(&grid), " hello");
    }

    #[test]
    fn expand_word_url_trailing_slash_preserved() {
        // A trailing slash is URL-safe and must be included in the selection.
        let grid = make_grid_with_text(&["open https://example.com/ now"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 5 },
            SelectionAnchor { row: 0, col: 5 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.text(&grid), "https://example.com/");
    }

    #[test]
    fn expand_word_ascii_run_inside_cjk_does_not_merge() {
        // "abc日本語def" — long-press on the ASCII "abc" must NOT absorb the
        // adjacent CJK run (script-class isolation).
        let grid = make_grid_with_text(&["abc日本語def"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 },
            SelectionAnchor { row: 0, col: 1 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 2);
        assert_eq!(expanded.text(&grid), "abc");
    }

    #[test]
    fn expand_word_cjk_run_with_number() {
        // Numbers are word chars; a CJK run adjacent to a digit of different
        // script class must remain separate.
        let grid = make_grid_with_text(&["版本2发布"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 1 }, // '本'
            SelectionAnchor { row: 0, col: 1 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        // Only the CJK run "版本" is selected (col 0..1), the ASCII '2' is a
        // separate script class.
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 1);
        assert_eq!(expanded.text(&grid), "版本");
    }

    #[test]
    fn char_is_cjk_covers_common_ranges() {
        // Directly exercise the CJK detection ranges used by expand_word.
        assert!(char_is_cjk('中')); // CJK Unified Ideographs
        assert!(char_is_cjk('本'));
        assert!(char_is_cjk('㐀')); // Extension A
        assert!(char_is_cjk('\u{F900}')); // Compatibility ideograph
        assert!(char_is_cjk('あ')); // Hiragana
        assert!(char_is_cjk('ア')); // Katakana
        assert!(char_is_cjk('가')); // Hangul
        assert!(!char_is_cjk('a'));
        assert!(!char_is_cjk('1'));
        assert!(!char_is_cjk(' '));
    }

    #[test]
    fn expand_word_pure_ascii_word() {
        // Long-press inside a plain ASCII identifier selects the whole word.
        let grid = make_grid_with_text(&["hello world"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 2 },
            SelectionAnchor { row: 0, col: 2 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 4);
        assert_eq!(expanded.text(&grid), "hello");
    }

    #[test]
    fn expand_word_long_press_on_whitespace_bridges_adjacent_words() {
        // A long-press on the space between words expands left/right across the
        // blank to include both adjacent word runs (the seed is non-word so the
        // run bridges the whitespace rather than stopping at it).
        let grid = make_grid_with_text(&["foo bar"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 3 },
            SelectionAnchor { row: 0, col: 3 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.start.col, 0);
        assert_eq!(expanded.end.col, 6);
        assert_eq!(expanded.text(&grid), "foo bar");
    }

    #[test]
    fn expand_word_url_trailing_punctuation_stripped() {
        // Trailing sentence punctuation after a URL must NOT be included in the
        // selection (the URL extends to the period, then the period is stripped).
        let grid = make_grid_with_text(&["see https://example.com. now"]);
        let s = Selection::new(
            SelectionAnchor { row: 0, col: 6 },
            SelectionAnchor { row: 0, col: 6 },
            SelectionMode::Word,
        );
        let expanded = s.expand(|r, c| grid.cell(r, c).map(|cell| cell.char));
        assert_eq!(expanded.text(&grid), "https://example.com");
    }
}
