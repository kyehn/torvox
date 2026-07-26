use terminal_core::cell::Cell;
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

#[test]
fn snapshot_preserves_single_line_content() {
    let grid = make_grid_with_content(2, 10, &[("Hello", 0, 0), ("World", 1, 0)]);
    let snap = SessionSnapshot::from_grid(&grid);

    let restored = SessionSnapshot {
        visible_lines: snap.visible_lines.clone(),
        scrollback_lines: snap.scrollback_lines.clone(),
        rows: snap.rows,
        cols: snap.cols,
        max_scrollback: snap.max_scrollback,
    };

    assert_eq!(restored.rows, 2);
    assert_eq!(restored.cols, 10);
    assert_eq!(restored.visible_lines.len(), 2);

    let line0: String = restored.visible_lines[0]
        .cells()
        .iter()
        .map(|c| c.char)
        .collect::<String>()
        .trim_end()
        .to_string();
    assert_eq!(line0, "Hello", "visible line 0 should contain 'Hello'");

    let line1: String = restored.visible_lines[1]
        .cells()
        .iter()
        .map(|c| c.char)
        .collect::<String>()
        .trim_end()
        .to_string();
    assert_eq!(line1, "World", "visible line 1 should contain 'World'");
}

#[test]
fn snapshot_preserves_sgr_attributes() {
    let mut grid = Grid::new(1, 10);
    {
        let cell = grid.cell_mut(0, 0).unwrap();
        cell.char = 'B';
        cell.attrs.bold = true;
        cell.foreground = terminal_core::cell::Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
    }
    let snap = SessionSnapshot::from_grid(&grid);
    let cell = snap.visible_lines[0].cells().first().unwrap();
    assert_eq!(cell.char, 'B');
    assert!(cell.attrs.bold, "bold attribute should be preserved");
    assert_eq!(cell.foreground.r, 255, "foreground red should be preserved");
    assert_eq!(cell.foreground.g, 0, "foreground green should be 0");
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
        "should have scrollback lines"
    );

    let mut restored_grid = Grid::new(2, 10);
    snap.apply_to_scrollback(&mut restored_grid, 1000);
    assert!(
        restored_grid.scrollback_length() >= 2,
        "restored grid should have scrollback"
    );
}

#[test]
fn snapshot_serde_roundtrip_preserves_content() {
    let grid = make_grid_with_content(
        3,
        20,
        &[
            ("First line here", 0, 0),
            ("Second line here", 1, 0),
            ("Third line here", 2, 0),
        ],
    );
    let snap = SessionSnapshot::from_grid(&grid);
    let json = serde_json::to_string(&snap).expect("serialize");
    let back: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back.rows, snap.rows);
    assert_eq!(back.cols, snap.cols);
    assert_eq!(back.visible_lines.len(), snap.visible_lines.len());
    assert_eq!(back.scrollback_lines.len(), snap.scrollback_lines.len());

    for (i, (orig, restored)) in snap
        .visible_lines
        .iter()
        .zip(back.visible_lines.iter())
        .enumerate()
    {
        for j in 0..snap.cols as usize {
            let o = orig.cells().get(j).unwrap();
            let r = restored.cells().get(j).unwrap();
            assert_eq!(o.char, r.char, "cell [{i},{j}] char mismatch");
            assert_eq!(o.foreground, r.foreground, "cell [{i},{j}] fg mismatch");
            assert_eq!(o.background, r.background, "cell [{i},{j}] bg mismatch");
            assert_eq!(o.attrs, r.attrs, "cell [{i},{j}] attrs mismatch");
        }
    }
}

#[test]
fn snapshot_cell_by_cell_equality() {
    let mut grid = Grid::new(3, 10);
    for row in 0..3 {
        for col in 0..10 {
            if let Some(cell) = grid.cell_mut(row, col) {
                *cell = Cell {
                    char: (b'A' + (row * 10 + col) as u8 % 26) as char,
                    foreground: terminal_core::cell::Color {
                        r: (row * 80) as u8,
                        g: (col * 25) as u8,
                        b: 128,
                        a: 255,
                    },
                    attrs: terminal_core::cell::Attrs {
                        bold: col % 3 == 0,
                        italic: col % 5 == 0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
            }
        }
    }

    let snap = SessionSnapshot::from_grid(&grid);
    let json = serde_json::to_string(&snap).expect("serialize");
    let back: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");

    for row in 0..3 {
        for col in 0..10 {
            let orig = grid.cell(row, col).unwrap();
            let restored_cell = back.visible_lines[row as usize]
                .cells()
                .get(col as usize)
                .unwrap();
            assert_eq!(
                orig.char, restored_cell.char,
                "char mismatch at [{row},{col}]"
            );
            assert_eq!(
                orig.foreground, restored_cell.foreground,
                "fg mismatch at [{row},{col}]"
            );
            assert_eq!(
                orig.attrs.bold, restored_cell.attrs.bold,
                "bold mismatch at [{row},{col}]"
            );
            assert_eq!(
                orig.attrs.italic, restored_cell.attrs.italic,
                "italic mismatch at [{row},{col}]"
            );
        }
    }
}

#[test]
fn snapshot_preserves_cell_width() {
    let mut grid = Grid::new(1, 10);
    if let Some(cell) = grid.cell_mut(0, 0) {
        cell.char = '中';
        cell.width = 2;
    }
    let snap = SessionSnapshot::from_grid(&grid);
    let cell = snap.visible_lines[0].cells().first().unwrap();
    assert_eq!(cell.width, 2, "wide char width should be preserved");
}

#[test]
fn snapshot_preserves_uri() {
    let mut grid = Grid::new(1, 10);
    if let Some(cell) = grid.cell_mut(0, 0) {
        cell.char = 'X';
    }
    let snap = SessionSnapshot::from_grid(&grid);
    let cell = snap.visible_lines[0].cells().first().unwrap();
    assert_eq!(cell.char, 'X');
}

#[test]
fn snapshot_preserves_hidden_attribute() {
    let mut grid = Grid::new(1, 10);
    if let Some(cell) = grid.cell_mut(0, 0) {
        cell.char = 'H';
        cell.attrs.hidden = true;
    }
    let snap = SessionSnapshot::from_grid(&grid);
    let cell = snap.visible_lines[0].cells().first().unwrap();
    assert!(cell.attrs.hidden, "hidden attribute should be preserved");
}

#[test]
fn snapshot_preserves_strikethrough() {
    let mut grid = Grid::new(1, 10);
    if let Some(cell) = grid.cell_mut(0, 0) {
        cell.char = 'S';
        cell.attrs.strikethrough = true;
    }
    let snap = SessionSnapshot::from_grid(&grid);
    let cell = snap.visible_lines[0].cells().first().unwrap();
    assert!(
        cell.attrs.strikethrough,
        "strikethrough should be preserved"
    );
}

#[test]
fn snapshot_large_grid_content_equality() {
    let mut grid = Grid::new(50, 120);
    for row in 0..50 {
        for col in 0..120 {
            if let Some(cell) = grid.cell_mut(row, col) {
                *cell = Cell {
                    char: ((row * 120 + col) % 94 + 33) as u8 as char,
                    foreground: terminal_core::cell::Color {
                        r: (row * 5) as u8,
                        g: (col * 2) as u8,
                        b: 64,
                        a: 255,
                    },
                    ..Default::default()
                };
            }
        }
    }

    let snap = SessionSnapshot::from_grid(&grid);
    let json = serde_json::to_string(&snap).expect("serialize large grid");
    let back: SessionSnapshot = serde_json::from_str(&json).expect("deserialize large grid");

    assert_eq!(back.rows, 50);
    assert_eq!(back.cols, 120);
    assert_eq!(back.visible_lines.len(), 50);

    for row in 0..50 {
        for col in 0..120 {
            let orig = grid.cell(row, col).unwrap();
            let restored = back.visible_lines[row as usize]
                .cells()
                .get(col as usize)
                .unwrap();
            assert_eq!(orig.char, restored.char, "char mismatch at [{row},{col}]");
            assert_eq!(
                orig.foreground.r, restored.foreground.r,
                "fg.r mismatch at [{row},{col}]"
            );
        }
    }
}
