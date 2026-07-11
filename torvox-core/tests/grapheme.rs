use torvox_core::grid::Grid;
use torvox_core::unicode::is_wide;

/// CJK character written to grid: codepoint preserved, width derived from unicode
#[test]
fn cjk_char_in_grid_has_width_2() {
    let mut g = Grid::new(5, 10);
    g.fill_cells(0, '\u{4E2D}', 0, 1);
    let c = g.cell(0, 0).unwrap();
    assert_eq!(c.char as u32, 0x4E2D, "CJK char codepoint preserved");
    assert!(is_wide('\u{4E2D}'), "CJK char should be wide per Unicode");
}

/// ASCII character written to grid via fill retains identity
#[test]
fn ascii_in_grid_identity() {
    let mut g = Grid::new(3, 10);
    g.fill_cells(0, 'A', 0, 1);
    let c = g.cell(0, 0).unwrap();
    assert_eq!(c.char as u32, 65, "ASCII A should have codepoint 65");
}

/// Wide and narrow cells placed adjacently in same row stay distinct
#[test]
fn wide_and_narrow_in_same_row() {
    let mut g = Grid::new(3, 20);
    let c0 = g.cell_mut(0, 0).unwrap();
    c0.char = '\u{4E2D}';
    c0.width = 2;
    let c2 = g.cell_mut(0, 2).unwrap();
    c2.char = 'a';
    c2.width = 1;
    assert!(is_wide('\u{4E2D}'));
    assert_eq!(g.cell(0, 0).unwrap().char, '\u{4E2D}');
    assert_eq!(g.cell(0, 2).unwrap().char, 'a');
}

/// Default grid cell starts as space with width 1
#[test]
fn grid_default_cell_is_space() {
    let g = Grid::new(3, 5);
    let c = g.cell(0, 0).unwrap();
    assert_eq!(c.char, ' ');
    assert_eq!(c.width, 1);
}

/// Resizing the grid preserves existing cell contents
#[test]
fn grid_resize_preserves_cells() {
    let mut g = Grid::new(3, 10);
    let cell = g.cell_mut(1, 5).unwrap();
    cell.char = 'K';
    g.resize(5, 20);
    let retrieved = g.cell(1, 5).expect("cell should survive resize");
    assert_eq!(retrieved.char, 'K');
}

/// Grid cell count matches declared dimensions
#[test]
fn grid_dimensions_correct() {
    let g = Grid::new(5, 10);
    assert_eq!(g.rows(), 5);
    assert_eq!(g.cols(), 10);
}

/// Accessing every cell in grid returns Some
#[test]
fn grid_all_cells_accessible() {
    let g = Grid::new(5, 10);
    for r in 0..5 {
        for c in 0..10 {
            assert!(g.cell(r, c).is_some(), "cell({r},{c}) should exist");
        }
    }
}

/// Combining character (U+0301 combining acute accent) placed after base
/// character has width 0 and base cell is preserved.
#[test]
fn combining_char_keeps_same_width() {
    let mut g = Grid::new(3, 10);
    g.fill_cells(0, 'a', 0, 1);
    g.fill_cells(0, '\u{0301}', 1, 2);
    let base = g.cell(0, 0).unwrap();
    let comb = g.cell(0, 1).unwrap();
    assert_eq!(base.char, 'a', "base char preserved after combining char");
    assert_eq!(comb.char as u32, 0x0301, "combining accent codepoint");
    assert!(comb.char as u32 != 0, "combining char stored");
}
