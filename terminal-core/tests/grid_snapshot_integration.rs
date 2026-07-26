use terminal_core::cell::{Cell, Color};
use terminal_core::grid::Grid;
use terminal_core::snapshot::SessionSnapshot;

fn make_grid_with_content(rows: u32, cols: u32, content: &[(&str, u32, u32)]) -> Grid {
    let mut grid = Grid::new(rows, cols);
    for (text, row, col) in content {
        for (i, ch) in text.chars().enumerate() {
            let c = *col + i as u32;
            if c < cols
                && let Some(cell) = grid.cell_mut(*row, c)
            {
                *cell = Cell {
                    char: ch,
                    ..Default::default()
                };
            }
        }
    }
    grid
}

fn snapshot_to_text(snap: &SessionSnapshot) -> Vec<String> {
    snap.visible_lines
        .iter()
        .map(|line| {
            line.cells()
                .iter()
                .map(|c| if c.char == '\0' { ' ' } else { c.char })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn snapshot_from_grid_preserves_content() {
    let grid = make_grid_with_content(3, 10, &[("Hello", 0, 0), ("World", 1, 0), ("!", 2, 0)]);
    let snap = SessionSnapshot::from_grid(&grid);
    assert_eq!(snap.rows, 3);
    assert_eq!(snap.cols, 10);
    assert_eq!(snap.visible_lines.len(), 3);
    let text = snapshot_to_text(&snap);
    assert_eq!(text[0], "Hello");
    assert_eq!(text[1], "World");
    assert_eq!(text[2], "!");
}

#[test]
fn snapshot_serde_json_roundtrip_preserves_content() {
    let grid = make_grid_with_content(
        3,
        20,
        &[
            ("First line content", 0, 0),
            ("Second line content", 1, 0),
            ("Third line content", 2, 0),
        ],
    );
    let snap = SessionSnapshot::from_grid(&grid);
    let json = serde_json::to_string(&snap).expect("serialize");
    let restored: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(snap, restored);
    assert_eq!(restored.rows, 3);
    assert_eq!(restored.cols, 20);
}

#[test]
fn snapshot_from_grid_matches_cell_by_cell() {
    let mut grid = Grid::new(3, 10);
    for row in 0..3 {
        for col in 0..10 {
            if let Some(cell) = grid.cell_mut(row, col) {
                cell.char = (b'A' + (row * 10 + col) as u8 % 26) as char;
                cell.foreground = Color {
                    r: (row * 50) as u8,
                    g: (col * 25) as u8,
                    b: 100,
                    a: 255,
                };
                cell.attrs.bold = col % 2 == 0;
            }
        }
    }
    let snap = SessionSnapshot::from_grid(&grid);
    for row in 0..3 {
        for col in 0..10 {
            let orig = grid.cell(row, col).unwrap();
            let restored = &snap.visible_lines[row as usize].cells()[col as usize];
            assert_eq!(orig.char, restored.char, "char mismatch at [{row},{col}]");
            assert_eq!(
                orig.foreground, restored.foreground,
                "fg mismatch at [{row},{col}]"
            );
            assert_eq!(
                orig.background, restored.background,
                "bg mismatch at [{row},{col}]"
            );
            assert_eq!(
                orig.attrs.bold, restored.attrs.bold,
                "bold mismatch at [{row},{col}]"
            );
        }
    }
}

#[test]
fn snapshot_preserves_sgr_attributes() {
    let mut grid = Grid::new(1, 10);
    {
        let cell = grid.cell_mut(0, 0).unwrap();
        cell.char = 'B';
        cell.attrs.bold = true;
        cell.foreground = Color::new(255, 0, 0);
    }
    {
        let cell = grid.cell_mut(0, 1).unwrap();
        cell.char = 'I';
        cell.attrs.italic = true;
    }
    {
        let cell = grid.cell_mut(0, 2).unwrap();
        cell.char = 'U';
        cell.attrs.underline = true;
    }
    let snap = SessionSnapshot::from_grid(&grid);
    let cell0 = &snap.visible_lines[0].cells()[0];
    assert_eq!(cell0.char, 'B');
    assert!(cell0.attrs.bold, "bold attribute should be preserved");
    assert_eq!(cell0.foreground.r, 255);
    let cell1 = &snap.visible_lines[0].cells()[1];
    assert!(cell1.attrs.italic, "italic attribute should be preserved");
    let cell2 = &snap.visible_lines[0].cells()[2];
    assert!(
        cell2.attrs.underline,
        "underline attribute should be preserved"
    );
}

#[test]
fn snapshot_preserves_scrollback_content() {
    let mut grid = Grid::new(2, 10);
    grid.fill_cells(0, 'A', 0, 10);
    grid.scroll_up(0, 2, 10);
    grid.fill_cells(0, 'B', 0, 10);
    grid.scroll_up(0, 2, 10);
    grid.fill_cells(0, 'C', 0, 10);
    let snap = SessionSnapshot::from_grid(&grid);
    assert!(
        snap.scrollback_lines.len() >= 2,
        "should have at least 2 scrollback lines"
    );
}

#[test]
fn snapshot_apply_to_scrollback_roundtrip() {
    let mut grid = Grid::new(2, 5);
    grid.fill_cells(0, 'A', 0, 5);
    grid.scroll_up(0, 2, 5);
    grid.fill_cells(0, 'B', 0, 5);
    grid.scroll_up(0, 2, 5);
    let snap = SessionSnapshot::from_grid(&grid);
    let mut restored = Grid::new(2, 5);
    snap.apply_to_scrollback(&mut restored, 1000);
    assert!(
        restored.scrollback_length() >= 2,
        "restored grid should have scrollback"
    );
}

#[test]
fn snapshot_equality_for_same_content() {
    let mut grid1 = Grid::new(3, 5);
    grid1.cell_mut(0, 0).unwrap().char = 'X';
    let snap1 = SessionSnapshot::from_grid(&grid1);
    let snap2 = SessionSnapshot::from_grid(&grid1);
    assert_eq!(
        snap1, snap2,
        "same grid content should produce equal snapshots"
    );
}

#[test]
fn snapshot_inequality_for_different_content() {
    let mut grid1 = Grid::new(3, 5);
    grid1.cell_mut(0, 0).unwrap().char = 'X';
    let mut grid2 = Grid::new(3, 5);
    grid2.cell_mut(0, 0).unwrap().char = 'Y';
    let snap1 = SessionSnapshot::from_grid(&grid1);
    let snap2 = SessionSnapshot::from_grid(&grid2);
    assert_ne!(
        snap1, snap2,
        "different content should produce different snapshots"
    );
}

#[test]
fn snapshot_serde_json_preserves_max_scrollback() {
    let grid = Grid::with_scrollback(5, 10, 5000);
    let snap = SessionSnapshot::from_grid(&grid);
    let json = serde_json::to_string(&snap).expect("serialize");
    let restored: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.max_scrollback, 5000);
}

#[test]
fn snapshot_apply_to_scrollback_respects_max_lines() {
    let mut grid = Grid::new(2, 5);
    for i in 0..10 {
        grid.fill_cells(0, (b'A' + i) as char, 0, 5);
        grid.scroll_up(0, 2, 5);
    }
    let snap = SessionSnapshot::from_grid(&grid);
    let mut restored = Grid::with_scrollback(2, 5, 3);
    snap.apply_to_scrollback(&mut restored, 3);
    assert!(restored.scrollback_length() <= 3);
}

#[test]
fn snapshot_text_conversion_full_content() {
    let mut grid = Grid::new(2, 5);
    grid.cell_mut(0, 0).unwrap().char = 'H';
    grid.cell_mut(0, 1).unwrap().char = 'e';
    grid.cell_mut(0, 2).unwrap().char = 'l';
    grid.cell_mut(0, 3).unwrap().char = 'l';
    grid.cell_mut(0, 4).unwrap().char = 'o';
    grid.cell_mut(1, 0).unwrap().char = '!';
    let snap = SessionSnapshot::from_grid(&grid);
    let text = snapshot_to_text(&snap);
    assert_eq!(text[0], "Hello");
    assert_eq!(text[1], "!");
}

#[test]
fn snapshot_empty_grid_returns_empty_lines() {
    let grid = Grid::new(3, 5);
    let snap = SessionSnapshot::from_grid(&grid);
    let text = snapshot_to_text(&snap);
    assert_eq!(text.len(), 3);
    for line in &text {
        assert_eq!(line, &"", "each line should be empty");
    }
}

#[test]
fn snapshot_preserves_cell_width_attribute() {
    let mut grid = Grid::new(1, 5);
    grid.cell_mut(0, 0).unwrap().char = 'W';
    grid.cell_mut(0, 0).unwrap().width = 2;
    let snap = SessionSnapshot::from_grid(&grid);
    assert_eq!(snap.visible_lines[0].cells()[0].width, 2);
}

#[test]
fn snapshot_from_grid_preserves_all_cell_styles() {
    let mut grid = Grid::new(1, 5);
    grid.cell_mut(0, 0).unwrap().char = 'S';
    grid.cell_mut(0, 0).unwrap().attrs.strikethrough = true;
    grid.cell_mut(0, 1).unwrap().char = 'H';
    grid.cell_mut(0, 1).unwrap().attrs.hidden = true;
    let snap = SessionSnapshot::from_grid(&grid);
    assert!(snap.visible_lines[0].cells()[0].attrs.strikethrough);
    assert!(snap.visible_lines[0].cells()[1].attrs.hidden);
}

#[test]
fn snapshot_serde_json_with_scrollback() {
    let mut grid = Grid::new(3, 10);
    grid.fill_cells(0, 'X', 0, 10);
    grid.scroll_up(0, 3, 10);
    grid.fill_cells(0, 'Y', 0, 10);
    let snap = SessionSnapshot::from_grid(&grid);
    let json = serde_json::to_string(&snap).expect("serialize");
    let restored: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.scrollback_lines.len(), snap.scrollback_lines.len());
    assert_eq!(restored.visible_lines.len(), snap.visible_lines.len());
}
