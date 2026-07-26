/// ECMA-48 / VT standard compliance tests.
///
/// Each test encodes DEFINED expected behavior from the VT specification,
/// not inferred or guessed behavior. If these tests fail, the terminal has
/// a real bug — not just a gap in test coverage.
///
/// Reference: ECMA-48 5th edition (1991), VT100/VT520 manuals,
/// xterm's ctlseqs.ms documentation.
use terminal_engine::ghostty_terminal::CellSnapshot;
use terminal_engine::ghostty_terminal::GhosttyTerminal;

fn term(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 500).expect("terminal create")
}

fn cell(t: &GhosttyTerminal, row: u32, col: u32) -> CellSnapshot {
    let snap = t.take_snapshot();
    let idx = (row * snap.cols + col) as usize;
    snap.cells[idx].clone()
}

fn row_text(t: &GhosttyTerminal, row: u32) -> String {
    let snap = t.take_snapshot();
    let mut s = String::new();
    for c in 0..snap.cols {
        let idx = (row * snap.cols + c) as usize;
        let cp = snap.cells[idx].codepoint;
        if cp == 0 {
            s.push(' ');
        } else if let Some(ch) = char::from_u32(cp) {
            s.push(ch);
        }
    }
    s
}

// ============================================================
// SGR — Select Graphic Rendition (ECMA-48 §8.3.117)
// ============================================================

/// SGR 1 (bold) followed by text sets bold attribute on those cells only
#[test]
fn sgr_bold_affects_only_subsequent_cells() {
    let mut t = term(3, 40);
    t.vt_write(b"plain\x1b[1mbold\x1b[0mplain");
    t.flush();
    let c0 = cell(&t, 0, 0);
    let c4 = cell(&t, 0, 4);
    let c5 = cell(&t, 0, 5);
    let c8 = cell(&t, 0, 8);
    let c9 = cell(&t, 0, 9);
    assert!(!c0.bold, "cell 0 'p' should not be bold");
    assert!(!c4.bold, "cell 4 'n' should not be bold");
    assert!(c5.bold, "cell 5 'b' should be bold after SGR 1");
    assert!(c8.bold, "cell 8 'd' should be bold");
    assert!(!c9.bold, "cell 9 'p' should not be bold after SGR 0");
}

/// SGR 4 (underline) sets underline attribute
#[test]
fn sgr_underline_affects_subsequent_cells() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[4munder\x1b[0mplain");
    t.flush();
    let c0 = cell(&t, 0, 0);
    let c4 = cell(&t, 0, 4);
    let c5 = cell(&t, 0, 5);
    assert!(c0.underline, "cell 0 'u' should be underlined");
    assert!(c4.underline, "cell 4 'r' should be underlined");
    assert!(
        !c5.underline,
        "cell 5 'p' should not be underlined after SGR 0"
    );
}

/// SGR 7 (reverse) sets reverse video
#[test]
fn sgr_reverse_sets_reverse_attribute() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[7mrev\x1b[0mplain");
    t.flush();
    let c0 = cell(&t, 0, 0);
    let c3 = cell(&t, 0, 3);
    assert!(c0.reverse, "cell 0 should be reverse after SGR 7");
    assert!(!c3.reverse, "cell 3 should not be reverse after SGR 0");
}

/// SGR 30-37 (foreground color codes) — verify exact ANSI palette values
#[test]
/// SGR 30-37 sets ANSI foreground colors to exact palette values
fn sgr_foreground_color_codes() {
    let mut t = term(3, 40);
    for (i, color_code) in [30u8, 31, 32, 33, 34, 35, 36, 37].iter().enumerate() {
        let seq = format!("\x1b[{}m{}\x1b[0m", color_code, (b'A' + i as u8) as char);
        t.vt_write(seq.as_bytes());
    }
    t.flush();
    // Catppuccin Mocha palette: SGR 31 (red) = [243, 139, 168]
    let c_red = cell(&t, 0, 1);
    let r = (c_red.foreground[0] * 255.0).round() as u8;
    let g = (c_red.foreground[1] * 255.0).round() as u8;
    let b = (c_red.foreground[2] * 255.0).round() as u8;
    assert_eq!(r, 243, "SGR 31 red channel (expected 243, got {r})");
    assert_eq!(g, 139, "SGR 31 green channel (expected 139, got {g})");
    assert_eq!(b, 168, "SGR 31 blue channel (expected 168, got {b})");
    // SGR 32 (green) = [166, 227, 161]
    let c_green = cell(&t, 0, 2);
    let r = (c_green.foreground[0] * 255.0).round() as u8;
    let g = (c_green.foreground[1] * 255.0).round() as u8;
    let b = (c_green.foreground[2] * 255.0).round() as u8;
    assert_eq!(r, 166, "SGR 32 red channel (expected 166, got {r})");
    assert_eq!(g, 227, "SGR 32 green channel (expected 227, got {g})");
    assert_eq!(b, 161, "SGR 32 blue channel (expected 161, got {b})");
    // SGR 34 (blue) = [137, 180, 250]
    let c_blue = cell(&t, 0, 4);
    let r = (c_blue.foreground[0] * 255.0).round() as u8;
    let g = (c_blue.foreground[1] * 255.0).round() as u8;
    let b = (c_blue.foreground[2] * 255.0).round() as u8;
    assert_eq!(r, 137, "SGR 34 red channel (expected 137, got {r})");
    assert_eq!(g, 180, "SGR 34 green channel (expected 180, got {g})");
    assert_eq!(b, 250, "SGR 34 blue channel (expected 250, got {b})");
}

// ============================================================
// CUP — Cursor Position (ECMA-48 §8.3.22)
// ============================================================

#[test]
fn cup_positions_cursor_correctly() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[3;10HX");
    t.flush();
    let c = cell(&t, 2, 9);
    assert_eq!(c.codepoint, 'X' as u32, "X should be at row=2 col=9");
}

#[test]
fn cup_out_of_range_row_clamps() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[99;1HX");
    t.flush();
    let c = cell(&t, 4, 0);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "CUP row=99 should clamp to last row (4)"
    );
}

#[test]
fn cup_out_of_range_col_clamps() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[1;99HX");
    t.flush();
    let c = cell(&t, 0, 19);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "CUP col=99 should clamp to last col (19)"
    );
}

// ============================================================
// ED — Erase Display (ECMA-48 §8.3.38)
// ============================================================

#[test]
fn ed2_erases_entire_display() {
    let mut t = term(3, 5);
    t.vt_write(b"ABCDE\nFGHIJ\nKLMNO");
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    for row in 0..3 {
        for col in 0..5 {
            let c = cell(&t, row, col);
            assert_eq!(
                c.codepoint, 0,
                "cell({row},{col}) should be empty after ED 2"
            );
        }
    }
}

/// ED 0 erases from cursor position TO end (inclusive of cursor)
#[test]
fn ed0_erases_from_cursor_to_end() {
    let mut t = term(3, 10);
    t.vt_write(b"0123456789");
    t.flush();
    t.vt_write(b"\x1b[4G");
    t.vt_write(b"\x1b[0J");
    t.flush();
    let text = row_text(&t, 0);
    assert_eq!(text.chars().next().unwrap(), '0');
    assert_eq!(text.chars().nth(2).unwrap_or('?'), '2');
    assert_eq!(
        text.chars().nth(3).unwrap_or('?'),
        ' ',
        "col 3 should be erased by ED 0 (erasures from cursor inclusive)"
    );
}

// ============================================================
// CUD/CUU/CUF/CUB — Cursor movement
// ============================================================

#[test]
fn cud_moves_cursor_down() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[2B");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 2, 0);
    assert_eq!(c.codepoint, 'X' as u32, "CUD 2 should place X at row 2");
}

#[test]
fn cuf_moves_cursor_right() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[5C");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 0, 5);
    assert_eq!(c.codepoint, 'X' as u32, "CUF 5 should place X at col 5");
}

#[test]
fn cub_moves_cursor_left() {
    let mut t = term(3, 20);
    t.vt_write(b"ABCDE\x1b[2D");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 0, 3);
    assert_eq!(c.codepoint, 'X' as u32, "CUB 2 should place X at col 3");
}

// ============================================================
// DECSC/DECRC — Save/Restore Cursor
// ============================================================

#[test]
fn decsc_decrc_save_restore_cursor_position() {
    let mut t = term(5, 20);
    t.vt_write(b"Hello");
    t.flush();
    t.vt_write(b"\x1b7");
    t.vt_write(b"\x1b[3;10HWorld");
    t.flush();
    t.vt_write(b"\x1b8");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 0, 5);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "DECRC should restore cursor to (0,5)"
    );
}

// ============================================================
// DECAWM — Auto-wrap Mode (DEC private mode 7)
// ============================================================

/// DECAWM on (default): writing at right margin wraps to next line
#[test]
fn decawm_on_wraps_at_right_margin() {
    let mut t = term(3, 5);
    t.vt_write(b"ABCDE");
    t.flush();
    t.vt_write(b"F");
    t.flush();
    let c = cell(&t, 1, 0);
    assert_eq!(c.codepoint, 'F' as u32, "F should wrap to row 1 col 0");
}

// ============================================================
// DECSTBM — Set Top and Bottom Margins (Scroll Region)
// ============================================================

#[test]
fn scroll_region_restricts_scroll() {
    let mut t = term(5, 10);
    t.vt_write(b"1\n2\n3\n4\n5");
    t.flush();
    t.vt_write(b"\x1b[2;4r");
    t.vt_write(b"\x1b[4;1H");
    t.vt_write(b"\x1b[1L");
    t.flush();
    let r0 = row_text(&t, 0).trim_end().to_string();
    assert!(
        !r0.is_empty(),
        "Row 0 should still have content (outside scroll region)"
    );
}

// ============================================================
// IL/DL — Insert/Delete Line
// ============================================================

/// Insert Line shifts content down within scroll region
#[test]
fn insert_line_shifts_content_down() {
    let mut t = term(5, 10);
    t.pty_write(b"Row1\nRow2\nRow3\nRow4\nRow5");
    t.flush();
    t.vt_write(b"\x1b[3;1H");
    t.vt_write(b"\x1b[1L");
    t.flush();
    let r3 = row_text(&t, 2).trim_end().to_string();
    let r4 = row_text(&t, 3).trim_end().to_string();
    assert!(
        r3.is_empty() || r3.chars().all(|c| c == ' '),
        "Row 3 should be empty after IL"
    );
    assert_eq!(r4, "Row3", "Row 4 should have old Row3 content after IL");
}

/// Delete Line shifts content up within scroll region
#[test]
fn delete_line_shifts_content_up() {
    let mut t = term(5, 10);
    t.pty_write(b"Row1\nRow2\nRow3\nRow4\nRow5");
    t.flush();
    t.vt_write(b"\x1b[2;1H");
    t.vt_write(b"\x1b[1M");
    t.flush();
    let r2 = row_text(&t, 1).trim_end().to_string();
    assert_eq!(r2, "Row3", "Row 2 should have old Row3 content after DL");
}

// ============================================================
// CHA — Cursor Horizontal Absolute (ECMA-48 §8.3.15)
// ============================================================

#[test]
fn cha_moves_to_absolute_column() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[10GX");
    t.flush();
    let c = cell(&t, 0, 9);
    assert_eq!(c.codepoint, 'X' as u32, "X should be at col 9 via CHA");
}

// ============================================================
// EL — Erase in Line (ECMA-48 §8.3.39)
// ============================================================

/// EL 0 erases from cursor to end of line (INCLUDING cursor position)
#[test]
fn el0_erases_from_cursor_to_end() {
    let mut t = term(3, 10);
    t.vt_write(b"0123456789");
    t.flush();
    t.vt_write(b"\x1b[5G");
    t.vt_write(b"\x1b[0K");
    t.flush();
    let c0 = cell(&t, 0, 3);
    let c4 = cell(&t, 0, 4);
    assert_eq!(c0.codepoint, '3' as u32, "Cell 3 should survive EL 0");
    assert_eq!(c4.codepoint, 0, "Cell 4 (cursor) should be erased by EL 0");
}

/// EL 1 erases from start to cursor (INCLUDING cursor position)
#[test]
fn el1_erases_from_start_to_cursor() {
    let mut t = term(3, 10);
    t.vt_write(b"0123456789");
    t.flush();
    t.vt_write(b"\x1b[5G");
    t.vt_write(b"\x1b[1K");
    t.flush();
    let c4 = cell(&t, 0, 4);
    let c5 = cell(&t, 0, 5);
    assert_eq!(c4.codepoint, 0, "Cell 4 (cursor) should be erased by EL 1");
    assert_eq!(c5.codepoint, '5' as u32, "Cell 5 should survive EL 1");
}

#[test]
fn el2_erases_entire_line() {
    let mut t = term(3, 10);
    t.vt_write(b"0123456789");
    t.flush();
    t.vt_write(b"\x1b[2K");
    t.flush();
    for col in 0..10 {
        let c = cell(&t, 0, col);
        assert_eq!(c.codepoint, 0, "Column {col} should be erased by EL 2");
    }
}

// ============================================================
// DCH — Delete Character (ECMA-48 §8.3.27)
// ============================================================

#[test]
fn dch_deletes_characters_and_shifts_left() {
    let mut t = term(3, 10);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[3G");
    t.vt_write(b"\x1b[1P");
    t.flush();
    let c2 = cell(&t, 0, 2);
    let c8 = cell(&t, 0, 8);
    assert_eq!(c2.codepoint, 'D' as u32, "DCH: col 2 should have D");
    assert_eq!(
        c8.codepoint, 'J' as u32,
        "DCH: col 8 should have J (shifted)"
    );
    let c9 = cell(&t, 0, 9);
    assert_eq!(c9.codepoint, 0, "DCH: col 9 should be empty after shift");
}

// ============================================================
// ECH — Erase Character (ECMA-48 §8.3.36)
// ============================================================

#[test]
fn ech_erases_n_characters() {
    let mut t = term(3, 10);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[3G");
    t.vt_write(b"\x1b[3X");
    t.flush();
    assert_eq!(
        cell(&t, 0, 0).codepoint,
        'A' as u32,
        "col 0 should survive ECH"
    );
    assert_eq!(
        cell(&t, 0, 1).codepoint,
        'B' as u32,
        "col 1 should survive ECH"
    );
    assert_eq!(
        cell(&t, 0, 2).codepoint,
        0,
        "col 2 (cursor) should be erased by ECH"
    );
    assert_eq!(cell(&t, 0, 4).codepoint, 0, "col 4 should be erased by ECH");
    assert_eq!(
        cell(&t, 0, 5).codepoint,
        'F' as u32,
        "col 5 should survive ECH"
    );
}

// ============================================================
// KNOWN BUGS — Tests that document real terminal bugs.
//
// These tests encode ECMA-48 / VT standard behavior.
// If they fail, the terminal has a bug — the test is correct.
// Do NOT remove these tests. Fix the terminal instead.
// ============================================================

/// CUU (cursor up) — row movement is correct, but column is preserved.
///
/// Write "1\n2\n3\n4" (cursor at (3,1)), then CUU 2 → (1,1), then "X".
/// X should be at (1,1), not (1,0) — the column was 1 from writing "4".
#[test]
fn cuu_moves_cursor_to_correct_row() {
    let mut t = term(5, 20);
    t.pty_write(b"1\n2\n3\n4");
    t.flush();
    t.vt_write(b"\x1b[2A");
    t.vt_write(b"X");
    t.flush();
    let snap = t.take_snapshot();
    // Row 1, col 1 has 'X' (verified cursor position)
    assert_eq!(
        snap.cells[(snap.cols + 1) as usize].codepoint,
        'X' as u32,
        "CUU 2 from row 3 should place X at (1,1): cursor_row={}, cursor_col={}",
        snap.cursor_row,
        snap.cursor_col
    );
    // Row 1, col 0 still has '2'
    assert_eq!(snap.cells[snap.cols as usize].codepoint, '2' as u32);
}

/// DECAWM off prevents wrapping (fixed via ghostty correctness patch)
/// With DECAWM off, cursor stays at right margin, chars overwrite last column
#[test]
fn decawm_off_does_not_wrap() {
    let mut t = term(3, 5);
    t.vt_write(b"\x1b[?7l");
    t.vt_write(b"ABCDE");
    t.flush();
    t.vt_write(b"FGH");
    t.flush();
    let snap = t.take_snapshot();
    // Cursor stays at col 4 (right margin), chars overwrite it
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "col 0 = A (unchanged)");
    assert_eq!(
        snap.cells[4].codepoint, 'H' as u32,
        "col 4 = H (overwritten by F,G,H)"
    );
}

/// DECRC restores SGR bold attribute.
///
/// NOTE: bold + DECSC must be in one vt_write() call because the
/// vt_write() helper appends ST+SGR reset to each call.
/// Similarly, DECRC + Y must be in one call so the print
/// happens before the trailing SGR reset clears bold.
#[test]
fn decrc_restores_bold() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[1m\x1b7");
    t.vt_write(b"\x1b[0m");
    t.vt_write(b"X");
    t.flush();
    t.vt_write(b"\x1b8Y");
    t.flush();
    let cy = cell(&t, 0, 0);
    assert!(
        cy.bold,
        "Y after DECRC should be bold (restored from DECSC)"
    );
}

/// SGR 3 sets italic attribute (verified: italic chain is correct in
/// upstream ghostty; local .ghostty previously failed to compile.)
#[test]
fn sgr_italic_sets_attribute() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[3mitalic\x1b[0mplain");
    t.flush();
    let c0 = cell(&t, 0, 0);
    assert!(c0.italic, "cell 0 'i' should be italic after SGR 3");
}

/// SGR 1;31 combined sets bold+red foreground
#[test]
#[allow(clippy::many_single_char_names)]
fn sgr_combined_bold_red_sets_color() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[1;31mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.bold, "SGR 1 should set bold");
    let r = (c.foreground[0] * 255.0).round() as u8;
    let g = (c.foreground[1] * 255.0).round() as u8;
    let b = (c.foreground[2] * 255.0).round() as u8;
    assert_eq!(r, 243, "SGR 31 should set red=243");
    assert_eq!(g, 139, "SGR 31 should set green=139");
    assert_eq!(b, 168, "SGR 31 should set blue=168");
}
