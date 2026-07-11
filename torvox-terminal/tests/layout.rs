use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn term(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 500).expect("terminal create")
}

fn get_line(t: &GhosttyTerminal, row: u32) -> String {
    let snap = t.take_snapshot();
    (0..snap.cols)
        .filter_map(|c| {
            let idx = (row * snap.cols + c) as usize;
            if idx < snap.cells.len() {
                let cell = &snap.cells[idx];
                char::from_u32(cell.codepoint)
            } else {
                None
            }
        })
        .collect()
}

fn get_line_padded(t: &GhosttyTerminal, row: u32, width: u32) -> String {
    let snap = t.take_snapshot();
    let mut s = String::new();
    for c in 0..width {
        let idx = (row * snap.cols + c) as usize;
        if idx < snap.cells.len() {
            let cell = &snap.cells[idx];
            if cell.codepoint == 0 {
                s.push(' ');
            } else if let Some(ch) = char::from_u32(cell.codepoint) {
                s.push(ch);
            }
        }
    }
    s
}

#[test]
fn text_writes_at_cursor() {
    let mut t = term(3, 20);
    t.vt_write(b"Hello, world!");
    t.flush();
    let text = get_line(&t, 0);
    assert!(text.starts_with("Hello, world!"));
}

#[test]
fn clear_screen_empties_visible() {
    let mut t = term(5, 20);
    t.vt_write(b"Hello");
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    let snap = t.take_snapshot();
    let all_empty = snap.cells.iter().all(|c| c.codepoint == 0);
    assert!(all_empty, "all cells should be 0 after clear");
}

#[test]
fn cursor_movement_positioned_text() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[3;10HX");
    t.flush();
    let text = get_line_padded(&t, 2, 20);
    assert_eq!(
        text.chars().nth(9).unwrap(),
        'X',
        "X should appear at row 3 (0-based=2) column 10 (0-based=9)"
    );
}

#[test]
fn carriage_return_at_end() {
    let mut t = term(3, 10);
    t.vt_write(b"ABCDEFGHIJ\rX");
    t.flush();
    let text = get_line_padded(&t, 0, 10);
    assert_eq!(text.chars().next().unwrap(), 'X');
    assert_eq!(text.chars().nth(1).unwrap(), 'B');
}

#[test]
fn newline_at_bottom_scrolls() {
    let mut t = term(3, 10);
    t.pty_write(b"1\n2\n3");
    t.flush();
    assert_eq!(get_line_padded(&t, 0, 10).trim_end(), "1");
    assert_eq!(get_line_padded(&t, 1, 10).trim_end(), "2");
    assert_eq!(get_line_padded(&t, 2, 10).trim_end(), "3");
    t.pty_write(b"\n4");
    t.flush();
    assert_eq!(
        get_line_padded(&t, 0, 10).trim_end(),
        "2",
        "line 0 should scroll"
    );
    assert_eq!(get_line_padded(&t, 1, 10).trim_end(), "3");
    assert_eq!(get_line_padded(&t, 2, 10).trim_end(), "4");
}

#[test]
fn tab_advances_cursor() {
    let mut t = term(3, 20);
    t.vt_write(b"A\tB");
    t.flush();
    let text = get_line_padded(&t, 0, 20);
    assert_eq!(text.chars().next().unwrap(), 'A');
    assert_eq!(
        text.chars().nth(8).unwrap(),
        'B',
        "Tab should advance to column 8"
    );
}

#[test]
fn cursor_up_moves_cursor() {
    let mut t = term(5, 20);
    t.pty_write(b"line1\n\n\nline4");
    t.flush();
    assert_eq!(get_line_padded(&t, 3, 20).trim_end(), "line4");
    t.vt_write(b"\x1b[4;1HX");
    t.flush();
    assert_eq!(get_line_padded(&t, 3, 20).chars().next().unwrap(), 'X');
}

/// CJK wide character (U+4E2D = 中) occupies 2 columns
#[test]
fn cjk_wide_char_advances_cursor_by_2() {
    let mut t = term(3, 20);
    t.vt_write("\u{4E2D}".as_bytes());
    t.flush();
    t.vt_write(b"x");
    t.flush();
    let text = get_line_padded(&t, 0, 20);
    assert_eq!(text.chars().next().unwrap(), '\u{4E2D}');
    assert_eq!(text.chars().nth(1).unwrap(), ' ');
    assert_eq!(text.chars().nth(2).unwrap(), 'x');
}

/// Fullwidth CJK at right margin wraps to next line
#[test]
fn cjk_at_right_margin_wraps() {
    let mut t = term(3, 5);
    t.vt_write("ABCD\u{4E2D}".as_bytes());
    t.flush();
    let text0 = get_line_padded(&t, 0, 5);
    let text1 = get_line_padded(&t, 1, 5);
    assert_eq!(
        text0.chars().nth(2).unwrap_or('\0'),
        'C',
        "ABCD should occupy first 4 columns of row 0"
    );
    // CJK char at position 4 (right margin) wraps: cursor stays at 4, char appears on next line
    let char_pos = text1.chars().position(|c| c == '\u{4E2D}').unwrap_or(99);
    assert!(
        char_pos < 5,
        "CJK right-margin char \u{4E2D} should wrap to row 1 at or near column 0"
    );
}

/// Mixed ASCII + CJK
#[test]
fn mixed_ascii_and_cjk() {
    let mut t = term(3, 30);
    t.vt_write("Hello\u{4E16}\u{754C}".as_bytes());
    t.flush();
    let text = get_line_padded(&t, 0, 30);
    assert!(text.contains("Hello"));
    assert!(
        text.contains('\u{4E16}'),
        "Row 0 should contain CJK char \u{4E16}"
    );
    assert!(
        text.contains('\u{754C}'),
        "Row 0 should contain CJK char \u{754C}"
    );
}

/// Cursor wraps at column 80 (standard terminal width default)
#[test]
fn cursor_wrap_to_next_line() {
    let mut t = term(3, 10);
    let long_line: Vec<u8> = (0..10).flat_map(|_| b"x").copied().collect();
    t.vt_write(&long_line);
    t.flush();
    t.vt_write(b"y");
    t.flush();
    let text0 = get_line_padded(&t, 0, 10);
    let text1 = get_line_padded(&t, 1, 10);
    assert_eq!(
        text0.trim_end().len(),
        10,
        "row 0 should be full after 10 chars"
    );
    assert_eq!(
        text1.chars().next().unwrap_or('?'),
        'y',
        "'y' should wrap to row 1 column 0"
    );
}

/// Reverse index at top scrolls content down
#[test]
fn reverse_index_at_top() {
    // Test 1: RI when cursor at top row scrolls content down
    let mut t = term(3, 10);
    t.pty_write(b"line1\nline2\nline3"); // cursor at (2,5)
    t.flush();
    // Move cursor to row 0, THEN issue RI
    // Using CUP (ESC [ row ; col H) to position cursor at top
    t.vt_write(b"\x1b[1;1H");
    t.flush();
    // Now RI with cursor at top should scroll down
    t.vt_write(b"\x1bM");
    t.flush();
    let text0 = get_line_padded(&t, 0, 10);
    let text1 = get_line_padded(&t, 1, 10);
    assert_eq!(
        text0.trim_end(),
        "",
        "RI at top scrolls — row 0 should be blank"
    );
    assert_eq!(
        text1.trim_end(),
        "line1",
        "row 1 should have previous top content"
    );
}

/// Scroll with wide chars in grid
#[test]
fn scroll_with_wide_chars() {
    let mut t = term(3, 10);
    t.pty_write("\u{4E2D}\u{56FD}\u{4EBA}\n".as_bytes());
    for _ in 0..3 {
        t.pty_write("123456789\n".as_bytes());
    }
    t.flush();
    let text0 = get_line_padded(&t, 0, 10);
    assert_eq!(text0.trim_end(), "123456789");
}

/// Combining character (U+0301 combining acute accent) after base
#[test]
fn combining_char_does_not_advance_cursor() {
    let mut t = term(3, 20);
    t.vt_write("a\u{0301}".as_bytes());
    t.flush();
    t.vt_write(b"x");
    t.flush();
    let text = get_line_padded(&t, 0, 20);
    assert_eq!(text.chars().next().unwrap(), 'a');
    // x should be at position 1 or 2 depending on combining char handling
    assert_eq!(text.chars().nth(1).unwrap_or('x'), 'x');
}
