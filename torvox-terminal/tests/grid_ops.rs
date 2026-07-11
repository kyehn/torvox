use torvox_core::grid::Grid;

fn small_grid(rows: u32, cols: u32) -> Grid {
    Grid::new(rows, cols)
}

fn set_char(g: &mut Grid, row: u32, col: u32, c: char) {
    if let Some(cell) = g.cell_mut(row, col) {
        cell.char = c;
    }
}

fn get_text(g: &Grid, rows: u32, cols: u32) -> Vec<String> {
    (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| {
                    let cell_ref = g.cell(r, c);
                    if let Some(cell) = cell_ref {
                        cell.char
                    } else {
                        ' '
                    }
                })
                .collect()
        })
        .collect()
}

fn fill_grid(g: &mut Grid, rows: u32, cols: u32) {
    for row in 0..rows {
        for col in 0..cols {
            let c = (b'a' + (row as u8 * cols as u8 + col as u8) % 26) as char;
            set_char(g, row, col, c);
        }
    }
}

#[test]
fn scroll_down_inserts_blank_row_at_top() {
    let rows = 5u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    g.scroll_down(0, rows, cols);
    let text = get_text(&g, rows, cols);
    for c in text[0].chars() {
        assert!(
            c == '\0' || c == ' ',
            "first row after scroll_down should be blank"
        );
    }
}

#[test]
fn scroll_up_inserts_blank_row_at_bottom() {
    let rows = 5u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    g.scroll_up(0, rows, cols);
    let text = get_text(&g, rows, cols);
    for c in text[(rows - 1) as usize].chars() {
        assert!(
            c == '\0' || c == ' ',
            "last row after scroll_up should be blank"
        );
    }
}

#[test]
fn scroll_region_respected_scroll_down() {
    let rows = 8u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    let before = get_text(&g, rows, cols);
    g.scroll_down(4, 7, cols); // scroll region rows 5-7 (0-indexed: 4-6)
    let after = get_text(&g, rows, cols);
    for row in 0..4 {
        assert_eq!(
            after[row as usize], before[row as usize],
            "row {row} should be unchanged"
        );
    }
}

#[test]
fn scroll_region_respected_scroll_up() {
    let rows = 8u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    let before = get_text(&g, rows, cols);
    g.scroll_up(4, 7, cols);
    let after = get_text(&g, rows, cols);
    for row in 0..4 {
        assert_eq!(
            after[row as usize], before[row as usize],
            "row {row} should be unchanged after scroll_up outside region"
        );
    }
    for row in 7..rows {
        assert_eq!(
            after[row as usize], before[row as usize],
            "row {row} should be unchanged after scroll_up outside region"
        );
    }
}

#[test]
fn delete_lines_pulls_content_up() {
    let rows = 6u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    let before = get_text(&g, rows, cols);
    g.delete_lines(1, 1, rows - 1, cols);
    let after = get_text(&g, rows, cols);
    assert_eq!(
        after[1], before[2],
        "after delete_lines at 1, old row 2 content should appear at row 1"
    );
}

#[test]
fn erase_rect_clears_specified_area() {
    let rows = 5u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    g.erase_rect(1, 1, 2, 2, ' ');
    let text = get_text(&g, rows, cols);
    for cell in text[1].chars().skip(1).take(2) {
        assert!(
            cell == '\0' || cell == ' ',
            "erase_rect should clear cells, got {cell:?}"
        );
    }
}

#[test]
fn fill_rect_writes_character_to_area() {
    let rows = 5u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    g.fill_rect(1, 1, 2, 2, 'X');
    let text = get_text(&g, rows, cols);
    assert_eq!(
        text[1].as_bytes()[1] as char,
        'X',
        "fill_rect should set cell to 'X'"
    );
    assert_eq!(
        text[2].as_bytes()[2] as char,
        'X',
        "fill_rect should set cell to 'X'"
    );
}

#[test]
fn clear_cells_resets_range() {
    let rows = 3u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    g.clear_cells(1, 1, 4);
    let text = get_text(&g, rows, cols);
    for (col, cell_char) in text[1].chars().enumerate() {
        let col_u32 = col as u32;
        if (1..4).contains(&col_u32) {
            assert!(
                cell_char == '\0' || cell_char == ' ',
                "clear_cells range should be blank, got {cell_char:?} at col {col_u32}"
            );
        }
    }
}

#[test]
fn copy_rect_moves_content() {
    let rows = 5u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    let before = get_text(&g, rows, cols);
    g.copy_rect(0, 0, 3, 0, cols, 2);
    let after = get_text(&g, rows, cols);
    assert_eq!(
        after[3], before[0],
        "copy_rect should copy row 0 content to row 3"
    );
    assert_eq!(
        after[4], before[1],
        "copy_rect should copy row 1 content to row 4"
    );
}

#[test]
fn selective_erase_display_clears_unprotected() {
    let rows = 5u32;
    let cols = 5u32;
    let mut g = small_grid(rows, cols);
    fill_grid(&mut g, rows, cols);
    g.selective_erase_display(0, rows, false);
    let text = get_text(&g, rows, cols);
    for row_text in &text {
        for ch in row_text.chars() {
            assert!(
                ch == '\0' || ch == ' ',
                "selective_erase_display should clear unprotected cells, got {ch:?}"
            );
        }
    }
}

#[test]
fn scroll_down_multiple_lines() {
    let rows = 5u32;
    let cols = 3u32;
    let mut g = small_grid(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            let c = (b'A' + row as u8) as char;
            set_char(&mut g, row, col, c);
        }
    }
    g.scroll_down(0, rows, cols);
    g.scroll_down(0, rows, cols);
    let text = get_text(&g, rows, cols);
    assert_eq!(
        text[0].chars().next().unwrap(),
        ' ',
        "two scroll downs should leave first row blank"
    );
}

#[test]
fn insert_lines_at_top_in_region() {
    let rows = 5u32;
    let cols = 3u32;
    let mut g = small_grid(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            let c = (b'0' + row as u8) as char;
            set_char(&mut g, row, col, c);
        }
    }
    let before = get_text(&g, rows, cols);
    g.insert_lines(0, 1, rows - 1, cols);
    let after = get_text(&g, rows, cols);
    assert_eq!(
        after[1], before[0],
        "insert_lines at 0 should move previous first row down"
    );
}

#[test]
fn delete_lines_at_bottom_clears_last() {
    let rows = 5u32;
    let cols = 3u32;
    let mut g = small_grid(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            let c = (b'A' + row as u8) as char;
            set_char(&mut g, row, col, c);
        }
    }
    g.delete_lines(rows - 2, 1, rows - 1, cols);
    let text = get_text(&g, rows, cols);
    for ch in text[(rows - 2) as usize].chars() {
        assert_eq!(
            ch, ' ',
            "delete_lines bottom should be space characters, got '{}'",
            ch
        );
    }
}
