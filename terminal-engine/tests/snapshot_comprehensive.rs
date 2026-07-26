use terminal_engine::ghostty_terminal::GhosttyTerminal;

fn make_terminal(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 1000).expect("terminal")
}

#[test]
fn snapshot_roundtrip_basic_write() {
    let mut t = make_terminal(2, 20);
    t.vt_write(b"Hello");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(s.rows, 2);
    assert_eq!(s.cols, 20);
    assert_eq!(s.cells[0].codepoint, 'H' as u32);
    assert_eq!(s.cells[1].codepoint, 'e' as u32);
    assert_eq!(s.cells[4].codepoint, 'o' as u32);
}

#[test]
fn snapshot_roundtrip_cursor_position() {
    let mut t = make_terminal(2, 20);
    t.vt_write(b"AB");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(s.cursor_col, 2, "cursor should be after 'AB'");
    assert_eq!(s.cursor_row, 0);
}

#[test]
fn snapshot_roundtrip_newline_moves_cursor() {
    let mut t = make_terminal(2, 20);
    t.pty_write(b"A\nB");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'A' as u32);
    assert_eq!(s.cells[20].codepoint, 'B' as u32);
}

#[test]
fn snapshot_roundtrip_sgr_bold() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[1mBold");
    t.flush();
    let s = t.take_snapshot();
    assert!(s.cells[0].bold, "first char should be bold");
    assert!(s.cells[1].bold, "second char should be bold");
}

#[test]
fn snapshot_roundtrip_sgr_reset() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[1mB\x1b[0mN");
    t.flush();
    let s = t.take_snapshot();
    assert!(s.cells[0].bold, "B should be bold");
    assert!(!s.cells[1].bold, "N after reset should not be bold");
}

#[test]
fn snapshot_roundtrip_scroll() {
    let mut t = make_terminal(3, 10);
    t.pty_write(b"A\nB\nC\nD");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(
        s.cells[0].codepoint, 'B' as u32,
        "after scroll, row 0 should be B"
    );
    assert_eq!(
        s.cells[10].codepoint, 'C' as u32,
        "after scroll, row 1 should be C"
    );
    assert_eq!(
        s.cells[20].codepoint, 'D' as u32,
        "after scroll, row 2 should be D"
    );
}

#[test]
fn snapshot_roundtrip_resize_preserves_content() {
    let mut t = make_terminal(2, 20);
    t.vt_write(b"PreserveMe");
    t.flush();
    t.resize(2, 40);
    let s = t.take_snapshot();
    assert_eq!(s.rows, 2);
    assert_eq!(s.cols, 40);
    assert_eq!(
        s.cells[0].codepoint, 'P' as u32,
        "content should survive resize"
    );
}

#[test]
fn snapshot_roundtrip_wide_char() {
    let mut t = make_terminal(1, 10);
    t.vt_write("你".as_bytes());
    t.flush();
    let s = t.take_snapshot();
    let codepoint = s.cells[0].codepoint;
    assert!(
        codepoint == '你' as u32 || codepoint > 0x4E00,
        "wide char should be preserved, got: {}",
        codepoint
    );
}

#[test]
fn snapshot_roundtrip_cursor_visibility() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[?25l");
    t.flush();
    let s = t.take_snapshot();
    assert!(!s.cursor_visible, "cursor should be hidden after DECSET 25");
}

#[test]
fn snapshot_roundtrip_cursor_show() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[?25h");
    t.flush();
    let s = t.take_snapshot();
    assert!(s.cursor_visible, "cursor should be visible after DECSET 25");
}

#[test]
fn snapshot_roundtrip_erase_line() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"HELLO\x1b[2K");
    t.flush();
    let s = t.take_snapshot();
    for i in 0..10 {
        assert_eq!(s.cells[i].codepoint, 0, "cell {} should be erased", i);
    }
}

#[test]
fn snapshot_roundtrip_insert_line() {
    let mut t = make_terminal(3, 10);
    t.pty_write(b"A\nB\nC");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[L");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'A' as u32, "row 0 should still be A");
    assert_eq!(s.cells[10].codepoint, 0, "row 1 should be empty (inserted)");
    assert_eq!(
        s.cells[20].codepoint, 'B' as u32,
        "row 2 should be B (pushed down)"
    );
}

#[test]
fn snapshot_roundtrip_delete_line() {
    let mut t = make_terminal(3, 10);
    t.pty_write(b"A\nB\nC");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[M");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'A' as u32, "row 0 should still be A");
    assert_eq!(
        s.cells[10].codepoint, 'C' as u32,
        "row 1 should be C (B deleted)"
    );
}

#[test]
fn snapshot_roundtrip_tab() {
    let mut t = make_terminal(1, 20);
    t.vt_write(b"A\tB");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'A' as u32);
    let b_col = s.cells.iter().position(|c| c.codepoint == 'B' as u32);
    assert!(
        b_col.is_some() && b_col.unwrap() >= 8,
        "B should be at tab stop (col >= 8)"
    );
}

#[test]
fn snapshot_roundtrip_carriage_return() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"ABC\rXYZ");
    t.flush();
    let s = t.take_snapshot();
    assert_eq!(
        s.cells[0].codepoint, 'X' as u32,
        "CR should return to col 0"
    );
    assert_eq!(s.cells[1].codepoint, 'Y' as u32);
    assert_eq!(s.cells[2].codepoint, 'Z' as u32);
}

#[test]
fn snapshot_roundtrip_sgr_italic() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[3mItalic");
    t.flush();
    let s = t.take_snapshot();
    assert!(s.cells[0].italic, "char should be italic");
}

#[test]
fn snapshot_roundtrip_sgr_underline() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[4mUnder");
    t.flush();
    let s = t.take_snapshot();
    assert!(s.cells[0].underline, "char should be underlined");
}

#[test]
fn snapshot_roundtrip_sgr_strikethrough() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[9mStrike");
    t.flush();
    let s = t.take_snapshot();
    assert!(s.cells[0].strikethrough, "char should have strikethrough");
}

#[test]
fn snapshot_roundtrip_sgr_color_fg() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[31mR");
    t.flush();
    let s = t.take_snapshot();
    assert!(
        s.cells[0].foreground[0] > 0.1,
        "red foreground should have red channel, got: {:?}",
        s.cells[0].foreground
    );
}

#[test]
fn snapshot_roundtrip_multi_sgr_accumulate() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[1m\x1b[3m\x1b[4mX");
    t.flush();
    let s = t.take_snapshot();
    assert!(s.cells[0].bold, "should be bold");
    assert!(s.cells[0].italic, "should be italic");
    assert!(s.cells[0].underline, "should be underlined");
}

#[test]
fn snapshot_roundtrip_256_color_fg() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[38;5;196mR");
    t.flush();
    let s = t.take_snapshot();
    assert!(
        s.cells[0].foreground[0] > 0.9,
        "256-color 196 should be bright red, fg={:?}",
        s.cells[0].foreground
    );
}

#[test]
#[allow(clippy::many_single_char_names)]
fn snapshot_roundtrip_truecolor_fg() {
    let mut t = make_terminal(1, 10);
    t.vt_write(b"\x1b[38;2;100;200;50mG");
    t.flush();
    let s = t.take_snapshot();
    let r = (s.cells[0].foreground[0] * 255.0) as u8;
    let g = (s.cells[0].foreground[1] * 255.0) as u8;
    let b = (s.cells[0].foreground[2] * 255.0) as u8;
    assert_eq!(r, 100, "truecolor R channel should be 100");
    assert_eq!(g, 200, "truecolor G channel should be 200");
    assert_eq!(b, 50, "truecolor B channel should be 50");
}
