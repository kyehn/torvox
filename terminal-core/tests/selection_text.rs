use terminal_core::cell::Cell;
use terminal_core::grid::Grid;
use terminal_core::selection::{ExpansionOptions, Selection, SelectionAnchor, SelectionMode};

fn make_grid(lines: &[&str]) -> Grid {
    let rows = lines.len() as u32;
    let cols = lines.iter().map(|l| l.len()).max().unwrap_or(1) as u32;
    let mut grid = Grid::new(rows, cols);
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

fn make_empty_grid(rows: u32, cols: u32) -> Grid {
    Grid::new(rows, cols)
}

fn make_grid_with_nulls(lines: &[&[char]]) -> Grid {
    let rows = lines.len() as u32;
    let cols = lines.iter().map(|l| l.len()).max().unwrap_or(1) as u32;
    let mut grid = Grid::new(rows, cols);
    for (row_idx, line) in lines.iter().enumerate() {
        for (col_idx, ch) in line.iter().enumerate() {
            if let Some(cell) = grid.cell_mut(row_idx as u32, col_idx as u32) {
                *cell = Cell {
                    char: *ch,
                    ..Default::default()
                };
            }
        }
    }
    grid
}

#[test]
fn char_selection_preserves_trailing_spaces() {
    let grid = make_grid(&["Hello     "]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 9 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert!(
        result.starts_with("Hello"),
        "should start with 'Hello', got: {result:?}"
    );
    assert!(
        result.starts_with("Hello     "),
        "Char mode preserves trailing spaces, got: {result:?}"
    );
}

#[test]
fn char_selection_single_word_middle() {
    let grid = make_grid(&["Hello World"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 6 },
        SelectionAnchor { row: 0, col: 10 },
        SelectionMode::Char,
    );
    assert_eq!(s.text(&grid), "World");
}

#[test]
fn char_selection_reversed_single_line() {
    let grid = make_grid(&["Hello World"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 10 },
        SelectionAnchor { row: 0, col: 0 },
        SelectionMode::Char,
    );
    assert_eq!(s.text(&grid), "Hello World");
}

#[test]
fn char_selection_multi_line() {
    let grid = make_grid(&["First", "Second", "Third"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 2 },
        SelectionAnchor { row: 2, col: 3 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert!(
        result.contains("rst"),
        "should contain 'rst', got: {result:?}"
    );
    assert!(
        result.contains("Secon"),
        "should contain 'Secon' (end col exclusive), got: {result:?}"
    );
    assert!(
        result.contains("Thir"),
        "should contain 'Thir', got: {result:?}"
    );
    let lines: Vec<&str> = result.split('\n').collect();
    assert_eq!(lines.len(), 3, "should have 3 lines, got: {result:?}");
}

#[test]
fn char_selection_entire_single_line() {
    let grid = make_grid(&["ABCDE"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 4 },
        SelectionMode::Char,
    );
    assert_eq!(s.text(&grid), "ABCDE");
}

#[test]
fn char_selection_single_cell() {
    let grid = make_grid(&["ABCDE"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 2 },
        SelectionAnchor { row: 0, col: 2 },
        SelectionMode::Char,
    );
    assert_eq!(s.text(&grid), "C");
}

#[test]
fn char_selection_preserves_internal_spaces() {
    let grid = make_grid(&["A B C D"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 6 },
        SelectionMode::Char,
    );
    assert_eq!(s.text(&grid), "A B C D");
}

#[test]
fn char_selection_null_chars_dropped() {
    let grid = make_grid_with_nulls(&[&['A', '\0', 'B', '\0', 'C']]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 4 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert_eq!(result, "ABC", "null chars should be dropped");
}

#[test]
fn line_selection_single_line() {
    let grid = make_grid(&["Hello World"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 5 },
        SelectionAnchor { row: 0, col: 10 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "Hello World");
}

#[test]
fn line_selection_multi_line() {
    let grid = make_grid(&["First", "Second", "Third"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 5 },
        SelectionAnchor { row: 1, col: 0 },
        SelectionMode::Line,
    );
    let result = s.text(&grid);
    assert_eq!(result, "First\nSecond");
}

#[test]
fn line_selection_all_three() {
    let grid = make_grid(&["AA", "BB", "CC"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 2, col: 0 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "AA\nBB\nCC");
}

#[test]
fn line_selection_trims_trailing_spaces() {
    let grid = make_grid_with_nulls(&[&['A', 'B', ' ', ' ', '\0', '\0']]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 5 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "AB");
}

#[test]
fn block_selection_single_column() {
    let grid = make_grid(&["ABC", "DEF", "GHI"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 1 },
        SelectionAnchor { row: 2, col: 1 },
        SelectionMode::Block,
    );
    assert_eq!(s.text(&grid), "B\nE\nH");
}

#[test]
fn block_selection_rectangle() {
    let grid = make_grid(&["ABCDEFGHIJ", "0123456789", "abcdefghij"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 2 },
        SelectionAnchor { row: 1, col: 5 },
        SelectionMode::Block,
    );
    assert_eq!(s.text(&grid), "CDEF\n2345");
}

#[test]
fn block_selection_3x3() {
    let grid = make_grid(&["ABCDEF", "GHIJKL", "MNOPQR"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 1 },
        SelectionAnchor { row: 2, col: 3 },
        SelectionMode::Block,
    );
    let result = s.text(&grid);
    assert_eq!(result, "BCD\nHIJ\nNOP", "got: {:?}", result);
}

#[test]
fn block_selection_single_cell() {
    let grid = make_grid(&["ABCD"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 2 },
        SelectionAnchor { row: 0, col: 2 },
        SelectionMode::Block,
    );
    assert_eq!(s.text(&grid), "C");
}

#[test]
fn block_selection_null_chars_become_spaces() {
    let grid = make_grid_with_nulls(&[&['A', '\0', 'C'], &['D', 'E', 'F']]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 1 },
        SelectionMode::Block,
    );
    assert_eq!(s.text(&grid), "A \nDE");
}

#[test]
fn word_selection_same_as_char() {
    let grid = make_grid(&["Hello World"]);
    let s_word = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 4 },
        SelectionMode::Word,
    );
    let s_char = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 4 },
        SelectionMode::Char,
    );
    assert_eq!(s_word.text(&grid), s_char.text(&grid));
}

#[test]
fn selection_reversed_multi_line() {
    let grid = make_grid(&["First", "Second"]);
    let s = Selection::new(
        SelectionAnchor { row: 1, col: 4 },
        SelectionAnchor { row: 0, col: 2 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert!(
        result.contains("rst"),
        "should contain 'rst', got: {result:?}"
    );
    assert!(
        result.contains("Secon"),
        "should contain 'Secon' (col 4 = 'n', not 'd'), got: {result:?}"
    );
}

#[test]
fn selection_out_of_bounds_row_returns_empty() {
    let grid = make_grid(&["AB"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 1 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert_eq!(result, "AB");
}

#[test]
fn selection_col_beyond_line_length() {
    let grid = make_grid(&["AB"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 100 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert_eq!(result, "AB");
}

#[test]
fn contains_boundary_left_edge() {
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 5 },
        SelectionAnchor { row: 0, col: 10 },
        SelectionMode::Char,
    );
    assert!(s.contains(0, 5));
    assert!(!s.contains(0, 4));
}

#[test]
fn contains_boundary_right_edge() {
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 5 },
        SelectionAnchor { row: 0, col: 10 },
        SelectionMode::Char,
    );
    assert!(s.contains(0, 10));
    assert!(!s.contains(0, 11));
}

#[test]
fn contains_multi_row_first_row_only_start() {
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 5 },
        SelectionAnchor { row: 2, col: 3 },
        SelectionMode::Char,
    );
    assert!(s.contains(0, 5));
    assert!(s.contains(0, 79));
    assert!(!s.contains(0, 4));
    assert!(s.contains(1, 0));
    assert!(s.contains(2, 3));
    assert!(!s.contains(2, 4));
}

#[test]
fn block_contains_strict_bounds() {
    let s = Selection::new(
        SelectionAnchor { row: 1, col: 2 },
        SelectionAnchor { row: 3, col: 5 },
        SelectionMode::Block,
    );
    assert!(s.contains(1, 2));
    assert!(s.contains(1, 5));
    assert!(s.contains(3, 2));
    assert!(s.contains(3, 5));
    assert!(!s.contains(1, 1));
    assert!(!s.contains(1, 6));
    assert!(!s.contains(0, 3));
    assert!(!s.contains(4, 3));
}

fn word_selection_with_expansion(grid: &Grid, row: u32, col: u32) -> String {
    let s = Selection::new(
        SelectionAnchor { row, col },
        SelectionAnchor { row, col },
        SelectionMode::Word,
    );
    let expanded = s.expand(
        |r, c| grid.cell(r, c).map(|cell| cell.char),
        ExpansionOptions::default(),
    );
    // Word mode's expand() only sets start/end; text() still uses the original mode
    let char_s = Selection::new(
        SelectionAnchor {
            row: expanded.start.row,
            col: expanded.start.col,
        },
        SelectionAnchor {
            row: expanded.end.row,
            col: expanded.end.col,
        },
        SelectionMode::Char,
    );
    char_s.text(grid)
}

// ── New selection tests ──

#[test]
fn block_selection_reversed_anchor() {
    let grid = make_grid(&["ABCDEF", "GHIJKL", "MNOPQR"]);
    let s = Selection::new(
        SelectionAnchor { row: 2, col: 3 },
        SelectionAnchor { row: 0, col: 1 },
        SelectionMode::Block,
    );
    let result = s.text(&grid);
    assert_eq!(result, "BCD\nHIJ\nNOP", "got: {:?}", result);
}

#[test]
fn block_selection_single_row_multi_column() {
    let grid = make_grid(&["ABCDEFGHIJ"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 2 },
        SelectionAnchor { row: 0, col: 7 },
        SelectionMode::Block,
    );
    assert_eq!(s.text(&grid), "CDEFGH");
}

#[test]
fn block_selection_out_of_bounds_clamped() {
    let grid = make_grid(&["AB", "CD"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 5, col: 10 },
        SelectionMode::Block,
    );
    // Grid has only 2 rows × 2 cols, available rows are read
    let result = s.text(&grid);
    assert!(result.contains("AB"));
    assert!(result.contains("CD"));
}

#[test]
fn block_selection_out_of_bounds_truncates_whitespace() {
    let grid = make_grid(&["AB", "CD"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 10 },
        SelectionMode::Block,
    );
    let result = s.text(&grid);
    // Block mode includes trailing whitespace for null chars, trimmed by line mode
    assert!(result.starts_with("AB\nC"));
}

#[test]
fn selection_empty_returns_empty_string() {
    let grid = make_grid(&["Hello"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 0 },
        SelectionMode::Char,
    );
    assert_eq!(s.text(&grid), "H");
}

#[test]
fn selection_all_null_row_char_mode() {
    let grid = make_grid_with_nulls(&[&['\0', '\0', '\0']]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 2 },
        SelectionMode::Char,
    );
    assert_eq!(s.text(&grid), "");
}

#[test]
fn line_selection_reversed_rows() {
    let grid = make_grid(&["First", "Second", "Third"]);
    let s = Selection::new(
        SelectionAnchor { row: 2, col: 0 },
        SelectionAnchor { row: 0, col: 0 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "First\nSecond\nThird");
}

#[test]
fn line_selection_single_middle_row() {
    let grid = make_grid(&["Alpha", "Bravo", "Charlie"]);
    let s = Selection::new(
        SelectionAnchor { row: 1, col: 3 },
        SelectionAnchor { row: 1, col: 0 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "Bravo");
}

#[test]
fn line_selection_trailing_blanks_in_middle_row() {
    let grid = make_grid_with_nulls(&[&['H', 'i', '\0', '\0'], &['B', 'y', 'e', ' ']]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 1, col: 3 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "Hi\nBye");
}

#[test]
fn word_selection_via_expand_picks_full_word() {
    let grid = make_grid(&["the quick brown fox"]);
    let result = word_selection_with_expansion(&grid, 0, 6); // middle of "quick"
    assert_eq!(result, "quick", "got: {:?}", result);
}

#[test]
fn word_selection_via_expand_number() {
    let grid = make_grid(&["abc 123 xyz"]);
    let result = word_selection_with_expansion(&grid, 0, 5); // middle of "123"
    assert_eq!(result, "123");
}

#[test]
fn word_selection_via_expand_underscore() {
    let grid = make_grid(&["my_var_name"]);
    let result = word_selection_with_expansion(&grid, 0, 4); // middle of "my_var_name"
    assert_eq!(result, "my_var_name");
}

#[test]
fn word_selection_via_expand_punctuation_boundary() {
    let grid = make_grid(&["hello(world)test"]);
    let result = word_selection_with_expansion(&grid, 0, 7); // middle of "world"
    assert_eq!(result, "world");
}

#[test]
fn word_selection_via_expand_hyphenated() {
    let grid = make_grid(&["well-known"]);
    let result = word_selection_with_expansion(&grid, 0, 3); // middle of "well-known"
    // Hyphen is a word boundary, so expand finds "well" as the word
    assert_eq!(result, "well");
}

#[test]
fn word_selection_via_expand_second_hyphen_part() {
    let grid = make_grid(&["well-known"]);
    let result = word_selection_with_expansion(&grid, 0, 6); // middle of "known"
    // Hyphen is a word boundary, so expand finds "known" as the word
    assert_eq!(result, "known");
}

#[test]
fn word_selection_via_expand_first_word() {
    let grid = make_grid(&["cat dog mouse"]);
    let result = word_selection_with_expansion(&grid, 0, 1); // middle of "cat"
    assert_eq!(result, "cat");
}

#[test]
fn line_selection_single_line_trailing_blanks() {
    let grid = make_grid_with_nulls(&[&['A', 'B', ' ', ' ', '\0', '\0', '\0']]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 6 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "AB");
}

#[test]
fn line_selection_reversed_single_line() {
    let grid = make_grid(&["ReverseSelectionTest"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 15 },
        SelectionAnchor { row: 0, col: 0 },
        SelectionMode::Line,
    );
    assert_eq!(s.text(&grid), "ReverseSelectionTest");
}

#[test]
fn block_selection_span_multiple_rows_with_nulls() {
    let grid = make_grid_with_nulls(&[
        &['A', 'B', '\0', 'D'],
        &['E', '\0', 'G', 'H'],
        &['I', 'J', 'K', '\0'],
    ]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 2, col: 2 },
        SelectionMode::Block,
    );
    let result = s.text(&grid);
    let lines: Vec<&str> = result.split('\n').collect();
    assert_eq!(lines.len(), 3);
    // Block mode: null becomes space (default char), each row gets cols 0-2
    assert_eq!(lines[0], "AB ");
    assert_eq!(lines[1], "E G");
    assert_eq!(lines[2], "IJK");
}

#[test]
fn selection_grid_edge_row_truncated_to_available() {
    let grid = make_grid(&["AB", "CD"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 10, col: 0 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    // Char mode iterates up to end row but grid only has 2 rows
    assert!(result.contains("AB"));
    assert!(result.contains("CD"));
}

#[test]
fn selection_grid_edge_row_single_available_line() {
    let grid = make_grid(&["Hello"]);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 5, col: 0 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert!(result.starts_with("Hello"));
}

#[test]
fn selection_zero_row_grid() {
    let grid = make_empty_grid(0, 0);
    let s = Selection::new(
        SelectionAnchor { row: 0, col: 0 },
        SelectionAnchor { row: 0, col: 0 },
        SelectionMode::Char,
    );
    let result = s.text(&grid);
    assert_eq!(result, "", "empty grid should yield empty selection");
}
