use torvox_core::cell::Color;
use torvox_core::grid::Grid;
use torvox_core::selection::{ExpansionOptions, Selection, SelectionAnchor, SelectionMode};

fn make_grid(lines: &[&str]) -> Grid {
    let rows = lines.len() as u32;
    let cols = lines.iter().map(|l| l.len()).max().unwrap_or(1) as u32;
    let mut grid = Grid::new(rows, cols);
    for (row_idx, line) in lines.iter().enumerate() {
        for (col_idx, ch) in line.chars().enumerate() {
            if let Some(cell) = grid.cell_mut(row_idx as u32, col_idx as u32) {
                cell.char = ch;
            }
        }
    }
    grid
}

fn make_grid_with_nulls(lines: &[&[char]]) -> Grid {
    let rows = lines.len() as u32;
    let cols = lines.iter().map(|l| l.len()).max().unwrap_or(1) as u32;
    let mut grid = Grid::new(rows, cols);
    for (row_idx, line) in lines.iter().enumerate() {
        for (col_idx, ch) in line.iter().enumerate() {
            if let Some(cell) = grid.cell_mut(row_idx as u32, col_idx as u32) {
                cell.char = *ch;
            }
        }
    }
    grid
}

#[test]
fn grid_creation_with_selection_single_line() {
    let grid = make_grid(&["Hello World"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 4 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "Hello");
}

#[test]
fn char_selection_multiple_words() {
    let grid = make_grid(&["foo bar baz"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 4 },
        SelectionAnchor { row: 0, col: 10 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "bar baz");
}

#[test]
fn word_selection_span_equal_char_on_same_line() {
    let grid = make_grid(&["foo bar baz"]);
    let sel_word = Selection::new(
        SelectionAnchor { row: 0, col: 4 },
        SelectionAnchor { row: 0, col: 6 },
        SelectionMode::Word,
    );
    let sel_char = Selection::new(
        SelectionAnchor { row: 0, col: 4 },
        SelectionAnchor { row: 0, col: 6 },
        SelectionMode::Char,
    );
    assert_eq!(sel_word.text(&grid), sel_char.text(&grid));
}

#[test]
fn line_selection_trailing_spaces_trimmed() {
    let grid = make_grid_with_nulls(&[&['H', 'e', 'y', ' ', ' ', '\0', '\0']]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 6 },
        SelectionMode::Line,
    );
    assert_eq!(sel.text(&grid), "Hey");
}

#[test]
fn block_selection_3x3_rectangle() {
    let grid = make_grid(&["ABCDEF", "GHIJKL", "MNOPQR"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 1 },
        SelectionAnchor { row: 2, col: 3 },
        SelectionMode::Block,
    );
    assert_eq!(sel.text(&grid), "BCD\nHIJ\nNOP");
}

#[test]
fn block_selection_nulls_become_spaces() {
    let grid = make_grid_with_nulls(&[&['A', '\0', 'C'], &['D', '\0', 'F']]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 2 },
        SelectionMode::Block,
    );
    assert_eq!(sel.text(&grid), "A C\nD F");
}

#[test]
fn block_selection_single_column() {
    let grid = make_grid(&["ABC", "DEF", "GHI"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 1 },
        SelectionAnchor { row: 2, col: 1 },
        SelectionMode::Block,
    );
    assert_eq!(sel.text(&grid), "B\nE\nH");
}

#[test]
fn char_selection_narrow_multi_line() {
    let grid = make_grid(&["ABC", "DEF"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 1 },
        SelectionMode::Char,
    );
    let result = sel.text(&grid);
    assert!(result.starts_with("ABC"), "should include 'ABC' from row 0");
    assert!(result.contains('D'), "should include 'D' from row 1");
}

#[test]
fn all_four_modes_on_three_line_grid() {
    let grid = make_grid(&["AAAAA", "BBBBB", "CCCCC"]);
    let char_sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 2, col: 4 },
        SelectionMode::Char,
    );
    let line_sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 2, col: 4 },
        SelectionMode::Line,
    );
    let block_sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 2, col: 4 },
        SelectionMode::Block,
    );
    assert_eq!(char_sel.text(&grid), "AAAAA\nBBBBB\nCCCCC");
    assert_eq!(line_sel.text(&grid), "AAAAA\nBBBBB\nCCCCC");
    assert_eq!(block_sel.text(&grid), "AAAAA\nBBBBB\nCCCCC");
}

#[test]
fn char_selection_preserves_internal_spaces() {
    let grid = make_grid(&["A B C"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 4 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "A B C");
}

#[test]
fn char_selection_empty_range_returns_single_char() {
    let grid = make_grid(&["Hello"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 2 },
        SelectionAnchor { row: 0, col: 2 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "l");
}

#[test]
fn contains_method_exclusive_edges() {
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 3 },
        SelectionAnchor { row: 2, col: 7 },
        SelectionMode::Char,
    );
    assert!(sel.contains(0, 3));
    assert!(sel.contains(0, 79));
    assert!(!sel.contains(0, 2));
    assert!(sel.contains(1, 0));
    assert!(sel.contains(2, 7));
    assert!(!sel.contains(2, 8));
}

#[test]
fn contains_block_strict_bounds() {
    let sel = Selection::new(
        SelectionAnchor { row: 1, col: 2 },
        SelectionAnchor { row: 3, col: 5 },
        SelectionMode::Block,
    );
    assert!(sel.contains(1, 2));
    assert!(sel.contains(1, 5));
    assert!(sel.contains(3, 2));
    assert!(sel.contains(3, 5));
    assert!(!sel.contains(1, 1));
    assert!(!sel.contains(1, 6));
    assert!(!sel.contains(0, 3));
    assert!(!sel.contains(4, 3));
}

#[test]
fn contains_line_mode_ignores_col() {
    let sel = Selection::new(
        SelectionAnchor { row: 1, col: 0 },
        SelectionAnchor { row: 3, col: 0 },
        SelectionMode::Line,
    );
    assert!(sel.contains(1, 50));
    assert!(sel.contains(2, 99));
    assert!(sel.contains(3, 0));
    assert!(!sel.contains(0, 0));
    assert!(!sel.contains(4, 0));
}

#[test]
fn reversed_selection_text_equals_normal() {
    let grid = make_grid(&["First", "Second", "Third"]);
    let normal = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 5 },
        SelectionMode::Char,
    );
    let reversed = Selection::new(
        SelectionAnchor { row: 1, col: 5 },
        SelectionAnchor { row: 0, col: 0 },
        SelectionMode::Char,
    );
    assert_eq!(normal.text(&grid), reversed.text(&grid));
}

#[test]
fn selection_out_of_bounds_returns_available_content() {
    let grid = make_grid(&["AB", "CD"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 5, col: 10 },
        SelectionMode::Char,
    );
    let result = sel.text(&grid);
    assert!(result.contains("AB"));
    assert!(result.contains("CD"));
}

#[test]
fn selection_zero_row_grid_returns_empty() {
    let grid = Grid::new(0, 0);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 0 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "");
}

#[test]
fn cell_attributes_affect_selection_bounds_only() {
    let grid = make_grid(&["Hello World"]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 6 },
        SelectionAnchor { row: 0, col: 10 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "World");
}

#[test]
fn selection_does_not_depend_on_attrs() {
    let mut grid = Grid::new(1, 10);
    for col in 0..5 {
        let cell = grid.cell_mut(0, col).unwrap();
        cell.char = (b'A' + col as u8) as char;
        cell.attrs.bold = col % 2 == 0;
        cell.foreground = Color::new(255, 0, 0);
    }
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 4 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "ABCDE");
}

#[test]
fn selection_mixed_width_chars_char_mode() {
    let mut grid = Grid::new(1, 10);
    grid.cell_mut(0, 0).unwrap().char = 'a';
    grid.cell_mut(0, 1).unwrap().char = 'b';
    grid.cell_mut(0, 2).unwrap().char = '\0';
    grid.cell_mut(0, 3).unwrap().char = 'c';
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 3 },
        SelectionMode::Char,
    );
    assert_eq!(sel.text(&grid), "abc");
}

#[test]
fn line_selection_ignores_cell_colors() {
    let mut grid = Grid::new(2, 5);
    for col in 0..5 {
        let cell = grid.cell_mut(0, col).unwrap();
        cell.char = (b'0' + col as u8) as char;
        cell.background = Color::new(100, 100, 100);
    }
    for col in 0..5 {
        let cell = grid.cell_mut(1, col).unwrap();
        cell.char = (b'5' + col as u8) as char;
    }
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 0 },
        SelectionMode::Line,
    );
    assert_eq!(sel.text(&grid), "01234\n56789");
}

#[test]
fn block_selection_non_rectangular_anchor_ordering() {
    let grid = make_grid(&["ABCDEF", "GHIJKL"]);
    let normal = Selection::new(
        SelectionAnchor { row: 0, col: 1 },
        SelectionAnchor { row: 1, col: 4 },
        SelectionMode::Block,
    );
    let reversed = Selection::new(
        SelectionAnchor { row: 1, col: 4 },
        SelectionAnchor { row: 0, col: 1 },
        SelectionMode::Block,
    );
    assert_eq!(normal.text(&grid), reversed.text(&grid));
    assert_eq!(normal.text(&grid), "BCDE\nHIJK");
}

#[test]
fn line_selection_reversed_end_before_start() {
    let grid = make_grid(&["Alpha", "Bravo", "Charlie"]);
    let sel = Selection::new(
        SelectionAnchor { row: 1, col: 0 },
        SelectionAnchor { row: 0, col: 5 },
        SelectionMode::Line,
    );
    assert_eq!(sel.text(&grid), "Alpha\nBravo");
}

#[test]
fn block_selection_all_null_row() {
    let grid = make_grid_with_nulls(&[&['\0', '\0', '\0'], &['\0', '\0', '\0']]);
    let sel = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 2 },
        SelectionMode::Block,
    );
    assert_eq!(sel.text(&grid), "   \n   ");
}

#[test]
fn semantic_selection_single_line_url() {
    let grid = make_grid(&["visit https://example.com/path for info"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 15 },
        SelectionAnchor { row: 0, col: 15 },
        SelectionMode::Semantic,
    );
    let expanded = s.expand(
        |r, c| grid.cell(r, c).map(|c| c.char),
        ExpansionOptions::default(),
    );
    assert_eq!(expanded.start.col, 6, "url prefix start");
    assert_eq!(expanded.end.col, 29, "url path end");
    assert_eq!(expanded.text(&grid), "https://example.com/path");
}

#[test]
fn semantic_selection_cross_row_url() {
    let mut grid = Grid::new(2, 50);
    let line0 = "prefix https://example.com/long-";
    let line1 = "url-continuation-more more suffix";
    for (col, ch) in line0.chars().enumerate() {
        grid.cell_mut(0, col as u32).unwrap().char = ch;
    }
    for (col, ch) in line1.chars().enumerate() {
        grid.cell_mut(1, col as u32).unwrap().char = ch;
    }
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 10 },
        SelectionAnchor { row: 0, col: 10 },
        SelectionMode::Semantic,
    );
    let expanded = s.expand(
        |r, c| grid.cell(r, c).map(|c| c.char),
        ExpansionOptions::default(),
    );
    assert_eq!(expanded.start.row, 0);
    assert_eq!(expanded.start.col, 7);
    assert_eq!(expanded.end.row, 1);
    assert_eq!(expanded.end.col, 20);
    let text = expanded.text(&grid);
    assert!(
        text.contains("https://example.com/long-"),
        "url should start on row 0, got '{text}'"
    );
    assert!(
        text.contains("url-continuation-more"),
        "url should continue on row 1, got '{text}'"
    );
}

#[test]
fn semantic_selection_single_row_text() {
    let grid = make_grid(&["select just this word"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 7 },
        SelectionAnchor { row: 0, col: 7 },
        SelectionMode::Semantic,
    );
    let expanded = s.expand(
        |r, c| grid.cell(r, c).map(|c| c.char),
        ExpansionOptions::default(),
    );
    assert_eq!(expanded.text(&grid), "just");
}
