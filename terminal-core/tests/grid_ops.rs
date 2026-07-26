use terminal_core::grid::Grid;

fn make_grid() -> Grid {
    Grid::new(24, 80)
}

fn fill_grid(g: &mut Grid, rows: u32, cols: u32) -> Vec<String> {
    let mut texts = Vec::new();
    for r in 0..rows {
        let mut row_text = String::new();
        for c in 0..cols {
            // Use letter 'a' + row as cell content
            let ch = (b'a' + r as u8) as char;
            if let Some(cell) = g.cell_mut(r, c) {
                cell.char = ch;
            }
            row_text.push(ch);
        }
        texts.push(row_text);
    }
    texts
}

/// Return true if the cell is considered blank (null or space).
fn is_blank(c: char) -> bool {
    c == '\0' || c == ' '
}

/// Assert that the entire row at `row` consists of blank cells.
fn assert_row_blank(g: &Grid, row: u32, cols: u32) {
    for col in 0..cols {
        if let Some(cell) = g.cell(row, col) {
            assert!(
                is_blank(cell.char),
                "cell ({row},{col}) should be blank, got {:?}",
                cell.char
            );
        }
    }
}

fn get_text(g: &Grid, rows: u32, cols: u32) -> Vec<String> {
    let mut result = Vec::new();
    for r in 0..rows {
        let mut row_text = String::new();
        for c in 0..cols {
            if let Some(cell) = g.cell(r, c) {
                row_text.push(cell.char);
            } else {
                row_text.push('?');
            }
        }
        result.push(row_text);
    }
    result
}

#[test]
fn scroll_down_inserts_blank_row_at_top() {
    let mut g = make_grid();
    let rows = 10;
    let cols = 5;
    let before = fill_grid(&mut g, rows, cols);
    // Scroll region from 0..rows down by 1
    g.scroll_down(0, rows, cols);
    let after = get_text(&g, rows, cols);
    // Top row should be blank (space or null char)
    assert_row_blank(&g, 0, cols);

    assert_eq!(
        after[1], before[0],
        "row 1 after scroll_down should match original row 0"
    );
}

#[test]
fn scroll_up_inserts_blank_row_at_bottom() {
    let mut g = make_grid();
    let rows = 10;
    let cols = 5;
    let before = fill_grid(&mut g, rows, cols);
    g.scroll_up(0, rows, cols);
    let after = get_text(&g, rows, cols);
    assert_eq!(
        after[0], before[1],
        "row 0 after scroll_up should match original row 1"
    );
    assert_row_blank(&g, rows - 1, cols);
}

#[test]
fn scroll_region_respected_scroll_up() {
    let mut g = make_grid();
    let rows = 10;
    let cols = 5;
    let before = fill_grid(&mut g, rows, cols);
    // Scroll region from 4 to 7 up by 1
    g.scroll_up(4, 7, cols);
    let after = get_text(&g, rows, cols);
    // Rows 0-3 should be unchanged
    for r in 0..4 {
        assert_eq!(after[r], before[r], "row {} should be unchanged", r);
    }
    // Row 4 should now have original row 5's content
    assert_eq!(
        after[4], before[5],
        "row 4 after scroll_up(4,7) should have been row 5"
    );
    // Row 6 should now be blank (bottom of region)
    assert_row_blank(&g, 6, cols);
    // Row 7+ should be unchanged
    assert_eq!(after[7], before[7], "row 7 should be unchanged");
}

#[test]
fn insert_lines_pushes_content_down() {
    let mut g = make_grid();
    let rows = 8;
    let cols = 5;
    let before = fill_grid(&mut g, rows, cols);
    // Insert 1 line at position 2 within region [2, 6)
    g.insert_lines(2, 1, 6, cols);
    let after = get_text(&g, rows, cols);
    // Row 2 should be blank
    assert_row_blank(&g, 2, cols);
    // Original row 2 content should shift to row 3
    assert_eq!(after[3], before[2], "original row 2 should now be at row 3");
    // Row 5 should have original row 4 content
    assert_eq!(after[5], before[4], "original row 4 should now be at row 5");
    // Rows 0-1 unchanged
    assert_eq!(after[0], before[0], "row 0 should be unchanged");
    assert_eq!(after[1], before[1], "row 1 should be unchanged");
    // Row 6+ unchanged (outside region)
    assert_eq!(after[6], before[6], "row 6 should be unchanged");
}

#[test]
fn delete_lines_pulls_content_up() {
    let mut g = make_grid();
    let rows = 8;
    let cols = 5;
    let before = fill_grid(&mut g, rows, cols);
    // Delete 1 line at position 2 within region [2, 6)
    g.delete_lines(2, 1, 6, cols);
    let after = get_text(&g, rows, cols);
    // Row 2 should now have original row 3 content
    assert_eq!(after[2], before[3], "original row 3 should now be at row 2");
    // Row 3 should have original row 4 content
    assert_eq!(after[3], before[4], "original row 4 should now be at row 3");
    // Row 5 should be blank (bottom of region after shift)
    assert_row_blank(&g, 5, cols);
    // Rows 0-1 and 6+ unchanged
    assert_eq!(after[0], before[0], "row 0 should be unchanged");
    assert_eq!(after[6], before[6], "row 6 should be unchanged");
}

#[test]
fn copy_rect_copies_content() {
    let mut g = make_grid();
    let rows = 10;
    let cols = 5;
    let before = fill_grid(&mut g, rows, cols);
    // Copy rows 0-1 to rows 4-5
    g.copy_rect(0, 0, 4, 0, cols, 2);
    let after = get_text(&g, rows, cols);
    assert_eq!(
        after[4], before[0],
        "copied row 4 should match original row 0"
    );
    assert_eq!(
        after[5], before[1],
        "copied row 5 should match original row 1"
    );
}

#[test]
fn erase_rect_clears_specified_area() {
    let mut g = make_grid();
    let rows = 5;
    let cols = 5;
    fill_grid(&mut g, rows, cols);
    g.erase_rect(1, 1, 3, 2, ' ');
    let after = get_text(&g, rows, cols);
    // Row 0 should be unchanged: aaaaa
    assert_eq!(after[0], "aaaaa", "row 0 should be unchanged");
    // Row 1 cols 1-3 should be blank
    for col in 1..4 {
        assert!(
            is_blank(after[1].as_bytes()[col] as char),
            "col {col} of row 1 should be blank"
        );
    }
    // Row 2 cols 1-3 should be blank
    for col in 1..4 {
        assert!(
            is_blank(after[2].as_bytes()[col] as char),
            "col {col} of row 2 should be blank"
        );
    }
}

/// erase_rect fully resets cell fields (fg, bg, attrs) not just char
#[test]
fn erase_rect_resets_all_cell_fields() {
    let mut g = make_grid();
    // Fill a single cell with non-default values
    let cell = g.cell_mut(2, 3).unwrap();
    cell.char = 'X';
    cell.foreground = terminal_core::cell::Color::new(100, 150, 200);
    cell.background = terminal_core::cell::Color::new(10, 20, 30);
    cell.attrs.bold = true;
    cell.attrs.italic = true;
    cell.width = 2;

    // Erase just that cell
    g.erase_rect(2, 3, 1, 1, ' ');

    let erased = g.cell(2, 3).unwrap();
    assert!(is_blank(erased.char), "char should be blank after erase");
    assert_eq!(
        erased.foreground,
        terminal_core::cell::Color::default(),
        "fg should reset to default"
    );
    assert_eq!(
        erased.background,
        terminal_core::cell::Color::default(),
        "bg should reset to default"
    );
    assert_eq!(
        erased.attrs,
        terminal_core::cell::Attrs::default(),
        "attrs should reset to default"
    );
    assert_eq!(erased.width, 1, "width should reset to 1");
}

#[test]
fn fill_rect_fills_content() {
    let mut g = make_grid();
    let rows = 5;
    let cols = 5;
    fill_grid(&mut g, rows, cols);
    g.fill_rect(1, 1, 3, 2, 'X');
    let after = get_text(&g, rows, cols);
    assert_eq!(&after[1][1..4], "XXX", "cols 1-3 of row 1 should be X");
    assert_eq!(&after[2][1..4], "XXX", "cols 1-3 of row 2 should be X");
}

#[test]
fn grid_new_initializes_all_cells() {
    let g = make_grid();
    assert_eq!(g.rows(), 24, "should have 24 rows");
    assert_eq!(g.cols(), 80, "should have 80 cols");
    // Every cell should be accessible
    for r in 0..24 {
        for c in 0..80 {
            assert!(g.cell(r, c).is_some(), "cell ({},{}) should exist", r, c);
        }
    }
}

#[test]
fn scroll_up_moves_top_line_to_scrollback() {
    let mut g = make_grid();
    let rows = 24;
    let cols = 80;
    fill_grid(&mut g, 1, cols); // Fill just row 0
    let scrollback_length_before = g.scrollback_length();
    g.scroll_up(0, rows, cols);
    assert_eq!(
        g.scrollback_length(),
        scrollback_length_before + 1,
        "scroll_up should add 1 line to scrollback"
    );
}
