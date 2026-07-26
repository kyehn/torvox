use terminal_engine::vt_conformance::{process_and_snapshot, row_text, sized_term, term};

#[test]
fn grid_cell_operations_produce_expected_content() {
    let mut t = sized_term(5, 20, 100);
    let snap = process_and_snapshot(&mut t, b"Hello\nWorld\n!");
    assert_eq!(row_text(&snap, 0), "Hello");
    assert_eq!(row_text(&snap, 1), "World");
    assert_eq!(row_text(&snap, 2), "!");
}

#[test]
fn cursor_home_after_text() {
    let mut t = term();
    t.vt_write(b"abc");
    t.flush();
    assert_eq!(t.cursor_x(), 3);
    t.vt_write(b"\r\n");
    t.flush();
    assert_eq!(t.cursor_y(), 1);
    assert_eq!(t.cursor_x(), 0);
}

#[test]
fn cursor_cup_absolute_position() {
    let mut t = term();
    t.vt_write(b"\x1b[10;20H");
    t.flush();
    assert_eq!(t.cursor_y(), 9, "CUP row should be 9");
    assert_eq!(t.cursor_x(), 19, "CUP col should be 19");
}

#[test]
fn cursor_cuu_cud_consistent() {
    let mut t = term();
    t.vt_write(b"\x1b[5;1H");
    t.flush();
    assert_eq!(t.cursor_y(), 4);
    t.vt_write(b"\x1b[2A");
    t.flush();
    assert_eq!(t.cursor_y(), 2, "CUU 2 from row 4 -> row 2");
    t.vt_write(b"\x1b[3B");
    t.flush();
    assert_eq!(t.cursor_y(), 5, "CUD 3 from row 2 -> row 5");
}

#[test]
fn cursor_cuf_cub_consistent() {
    let mut t = term();
    t.vt_write(b"\x1b[15G");
    t.flush();
    assert_eq!(t.cursor_x(), 14);
    t.vt_write(b"\x1b[5D");
    t.flush();
    assert_eq!(t.cursor_x(), 9, "CUB 5 from col 14 -> col 9");
    t.vt_write(b"\x1b[3C");
    t.flush();
    assert_eq!(t.cursor_x(), 12, "CUF 3 from col 9 -> col 12");
}

#[test]
fn sgr_bold_applies_to_cell() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[1mBold");
    assert!(snap.cells[0].bold, "first cell should be bold");
    let text = t.read_line_text(0).unwrap_or_default();
    assert!(text.starts_with("Bold"), "text should be output");
}

#[test]
fn sgr_italic_applies_to_cell() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[3mItalic");
    assert!(snap.cells[0].italic, "first cell should be italic");
}

#[test]
fn sgr_underline_applies_to_cell() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[4mUnderline");
    assert!(snap.cells[0].underline, "first cell should be underlined");
}

#[test]
fn sgr_reverse_applies_to_cell() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[7mReverse");
    assert!(
        snap.cells[0].reverse,
        "first cell should have reverse video"
    );
}

#[test]
fn sgr_strikethrough_applies_to_cell() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[9mStrike");
    assert!(
        snap.cells[0].strikethrough,
        "first cell should have strikethrough"
    );
}

#[test]
fn sgr_foreground_color_8bit() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[38;5;196mR");
    assert!(
        snap.cells[0].foreground[0] > 0.5,
        "foreground red should be > 0.5 for ANSI 196"
    );
}

#[test]
fn sgr_background_color_8bit() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[48;5;21mR");
    assert!(
        snap.cells[0].background[2] > 0.5,
        "background blue should be > 0.5 for ANSI 21"
    );
}

#[test]
fn sgr_reset_clears_all_attributes() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[1;4;31mX\x1b[0mY");
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "first cell should be bold");
    assert!(snap.cells[0].underline, "first cell should be underlined");
    assert!(
        !snap.cells[1].bold,
        "second cell should not be bold after reset"
    );
    assert!(
        !snap.cells[1].underline,
        "second cell should not be underlined after reset"
    );
}

#[test]
fn sgr_bold_off_22() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[1mB\x1b[22mN");
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "first cell should be bold");
    assert!(
        !snap.cells[1].bold,
        "second cell should not be bold after SGR 22"
    );
}

#[test]
fn sgr_underline_off_24() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[4mU\x1b[24mN");
    let snap = t.take_snapshot();
    assert!(snap.cells[0].underline, "first cell should be underlined");
    assert!(
        !snap.cells[1].underline,
        "second cell should not be underlined after SGR 24"
    );
}

#[test]
fn multiple_sgr_params() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[1;4;31mX");
    assert!(snap.cells[0].bold, "bold from multi-param SGR");
    assert!(snap.cells[0].underline, "underline from multi-param SGR");
    assert!(
        snap.cells[0].foreground[0] > 0.1,
        "red foreground from multi-param SGR"
    );
}

#[test]
fn text_output_cursor_advances() {
    let mut t = term();
    t.vt_write(b"Hello");
    t.flush();
    assert_eq!(
        t.cursor_x(),
        5,
        "cursor should advance 5 cols after 'Hello'"
    );
}

#[test]
fn newline_moves_cursor_down() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"Line1\nLine2\nLine3");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 2, "cursor should be on row 2");
    assert_eq!(row_text(&snap, 0), "Line1");
    assert_eq!(row_text(&snap, 1), "Line2");
    assert_eq!(row_text(&snap, 2), "Line3");
}

#[test]
fn carriage_return_goes_to_col_0() {
    let mut t = term();
    t.vt_write(b"Hello\rX");
    t.flush();
    assert_eq!(t.cursor_x(), 1, "cursor should be at col 1 after CR + X");
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, 'X' as u32,
        "CR should overwrite first char"
    );
}

#[test]
fn scroll_up_moves_content_to_scrollback() {
    let mut t = sized_term(3, 10, 100);
    t.vt_write(b"111\n222\n333");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 3);
    let dumped = t.dump_grid();
    assert_eq!(dumped.rows, 3);
    assert!(!dumped.visible.is_empty());
}

#[test]
fn erase_display_clears_screen() {
    let mut t = sized_term(3, 10, 100);
    let _snap = process_and_snapshot(&mut t, b"AAAAABBBBBCCCCC\x1b[2J");
    let snap = t.take_snapshot();
    for cell in &snap.cells {
        assert_eq!(cell.codepoint, 0, "all cells should be empty after ED 2");
    }
}

#[test]
fn erase_line_clears_row() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"ABCDEFGHIJ\x1b[2K");
    let snap = t.take_snapshot();
    for col in 0..10 {
        assert_eq!(
            snap.cells[col].codepoint, 0,
            "col {col} should be empty after EL 2"
        );
    }
}

#[test]
fn delete_chars_shifts_remaining_left() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"ABCDE\x1b[3G\x1b[2P");
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "DCH: col 0");
    assert_eq!(snap.cells[1].codepoint, 'B' as u32, "DCH: col 1");
    assert_eq!(
        snap.cells[2].codepoint, 'E' as u32,
        "DCH: col 2 = E (shifted)"
    );
    assert_eq!(snap.cells[3].codepoint, 0, "DCH: col 3 blank");
    assert_eq!(snap.cells[4].codepoint, 0, "DCH: col 4 blank");
}

#[test]
fn insert_blanks_shifts_content_right() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"CDE");
    let snap = process_and_snapshot(&mut t, b"\x1b[H\x1b[2@");
    assert_eq!(snap.cells[0].codepoint, 0, "ICH: col 0 blank");
    assert_eq!(snap.cells[1].codepoint, 0, "ICH: col 1 blank");
    assert_eq!(snap.cells[2].codepoint, 'C' as u32, "ICH: C -> col 2");
}

#[test]
fn repeat_character() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"X\x1b[4b");
    for col in 0..5 {
        assert_eq!(snap.cells[col].codepoint, 'X' as u32, "REP: cell {col} = X");
    }
}
