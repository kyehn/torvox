use terminal_engine::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn r1_roundtrip_hello() {
    let mut g = t();
    g.vt_write(b"hello");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'h' as u32);
}
#[test]
fn r2_roundtrip_multi_write() {
    let mut g = t();
    g.vt_write(b"A");
    g.flush();
    g.vt_write(b"B");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[1].codepoint, 'B' as u32);
}
#[test]
fn r3_roundtrip_newline() {
    let mut g = t();
    g.pty_write(b"X\nY");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[80].codepoint, 'Y' as u32);
}
#[test]
fn r4_roundtrip_cup_write() {
    let mut g = t();
    g.vt_write(b"\x1b[5;10HZ");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[4 * 80 + 9].codepoint, 'Z' as u32);
}
#[test]
fn r5_roundtrip_sgr_bold() {
    let mut g = t();
    g.vt_write(b"\x1b[1mBold");
    g.flush();
    let s = g.take_snapshot();
    assert!(s.cells[0].bold);
}
#[test]
fn r6_roundtrip_sgr_color() {
    let mut g = t();
    g.vt_write(b"\x1b[31mR");
    g.flush();
    let s = g.take_snapshot();
    assert!(s.cells[0].foreground[0] > 0.1);
}
#[test]
fn r7_roundtrip_erase_line() {
    let mut g = t();
    g.vt_write(b"ABCDE\x1b[2K");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 0);
}
#[test]
fn r8_roundtrip_scroll_up() {
    let mut g = GhosttyTerminal::new(3, 10, 100).expect("t");
    g.pty_write(b"A\nB\nC\nD");
    g.flush();
    let s = g.take_snapshot();
    // After A LF B LF C LF (scroll), D: row 0=B, row 1=C, row 2=D
    assert_eq!(
        s.cells[0].codepoint, 'B' as u32,
        "After scrolling with 3 rows, row 0 should be 'B'"
    );
    assert_eq!(
        s.cells[10].codepoint, 'C' as u32,
        "After scrolling with 3 rows, row 1 should be 'C'"
    );
    assert_eq!(
        s.cells[20].codepoint, 'D' as u32,
        "After scrolling with 3 rows, row 2 should be 'D'"
    );
}
#[test]
fn r9_roundtrip_resize() {
    let mut g = GhosttyTerminal::new(10, 20, 100).expect("t");
    g.vt_write(b"Hello");
    g.flush();
    g.resize(20, 40);
    let s = g.take_snapshot();
    assert_eq!(s.rows, 20);
}
#[test]
fn r10_roundtrip_scrollback() {
    let mut g = GhosttyTerminal::new(3, 10, 100).expect("t");
    for i in 0..10 {
        g.vt_write(format!("{}\n", i).as_bytes());
    }
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.rows, 3);
}
#[test]
fn r11_roundtrip_alt_screen() {
    let mut g = t();
    g.vt_write(b"Primary");
    g.flush();
    g.vt_write(b"\x1b[?1049h"); // alt screen
    g.flush();
    g.vt_write(b"\x1b[HAlt"); // home then write
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'A' as u32, "alt shows A");
    g.vt_write(b"\x1b[?1049l"); // exit alt
    g.flush();
    let s2 = g.take_snapshot();
    assert_eq!(s2.cells[0].codepoint, 'P' as u32, "primary restored P");
}
#[test]
fn r12_roundtrip_cursor_save() {
    let mut g = t();
    g.vt_write(b"\x1b7\x1b[5;10H\x1b8"); // save, move, restore
    g.flush();
    assert_eq!(g.cursor_x(), 0, "cursor restored to col 0");
    g.vt_write(b"X");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'X' as u32, "X written at col 0");
}
#[test]
fn r13_roundtrip_insert_lines() {
    let mut g = GhosttyTerminal::new(5, 10, 100).expect("t");
    g.vt_write(b"AB\nCD\nEF");
    g.vt_write(b"\x1b[2;1H\x1b[2L");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[20].codepoint, 0);
}
#[test]
fn r14_roundtrip_delete_lines() {
    let mut g = GhosttyTerminal::new(3, 5, 100).expect("t");
    g.vt_write(b"AAAAABBBBBCCCCC");
    g.vt_write(b"\x1b[2;1H\x1b[M"); // delete line at row 2
    g.flush();
    let s = g.take_snapshot();
    // After DL: row 1 = CCCCC (shifted up), row 2 = blank
    assert!(s.cells[0].codepoint > 0, "row 0 has content");
}
#[test]
fn r15_roundtrip_decset_decresets() {
    let mut g = t();
    g.vt_write(b"\x1b[?6h");
    g.flush();
    assert!(g.mode_get(6, 0));
    g.vt_write(b"\x1b[?6l");
    g.flush();
    assert!(!g.mode_get(6, 0));
}
#[test]
fn r16_roundtrip_decset_cursor_keys() {
    let mut g = t();
    g.vt_write(b"\x1b[?1h");
    g.flush();
    assert!(g.mode_get(1, 0));
}
#[test]
fn r17_roundtrip_decset_insert() {
    let mut g = t();
    g.vt_write(b"\x1b[?4h");
    g.flush();
    assert!(g.mode_get(4, 0));
}
#[test]
fn r18_roundtrip_decset_srm() {
    let mut g = t();
    g.vt_write(b"\x1b[?12h");
    g.flush();
    assert!(g.mode_get(12, 0));
}
#[test]
fn r19_roundtrip_tab_set() {
    let mut g = t();
    g.vt_write(b"\x1b[3g\x1b[10G\x1bH\x1b[H\x09");
    g.flush();
    assert_eq!(g.cursor_x(), 9);
}
#[test]
fn r20_roundtrip_ri() {
    let mut g = GhosttyTerminal::new(3, 10, 100).expect("t");
    g.vt_write(b"A\n\x1bM");
    g.flush();
    assert_eq!(g.cursor_y(), 0);
}
#[test]
fn r21_roundtrip_nel() {
    let mut g = t();
    g.vt_write(b"X\x1bE");
    g.flush();
    assert_eq!(g.cursor_y(), 1);
}
#[test]
fn r22_roundtrip_scroll_region() {
    let mut g = GhosttyTerminal::new(5, 10, 100).expect("t");
    g.vt_write(b"\x1b[2;4r\x1b[2;1HA");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[10].codepoint, 'A' as u32);
}
#[test]
fn r23_roundtrip_origin_mode_cursor() {
    let mut g = t();
    g.vt_write(b"\x1b[?6h\x1b[1;1HX");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'X' as u32);
}
#[test]
fn r24_roundtrip_decom() {
    let mut g = t();
    g.vt_write(b"\x1b[?6h");
    g.flush();
    assert!(g.mode_get(6, 0));
    g.vt_write(b"\x1b[?6l");
    g.flush();
    assert!(!g.mode_get(6, 0));
}
#[test]
fn r25_roundtrip_write_wide() {
    let mut g = t();
    g.vt_write(b"\xef\xbc\x81");
    g.flush();
    let s = g.take_snapshot();
    assert!(s.cells[0].codepoint > 0x80);
}
#[test]
fn r26_roundtrip_write_crlf() {
    let mut g = t();
    g.vt_write(b"AB\r\nC");
    g.flush();
    let s = g.take_snapshot();
    // AB, CR, LF, C → C at row 1 col 0 = cells[80]
    assert_eq!(s.cells[80].codepoint, 'C' as u32, "C after CRLF at row 1");
}
#[test]
fn r27_roundtrip_write_tab() {
    let mut g = t();
    g.vt_write(b"A\x09B");
    g.flush();
    assert!(g.cursor_x() > 1);
}
#[test]
fn r28_roundtrip_csq_skip_unknown() {
    let mut g = t();
    g.vt_write(b"\x1b[?9999h\x1b[?9999l");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.rows, 24);
}
#[test]
fn r29_roundtrip_osc_title() {
    let mut g = t();
    g.vt_write(b"\x1b]0;TestTitle\x07");
    g.flush();
    let snap = g.take_snapshot();
    assert_eq!(
        g.title(),
        "TestTitle".to_string(),
        "OSC 0 should set terminal title"
    );
    assert_eq!(
        snap.cells[0].codepoint, 0,
        "OSC 0 does not write to cells, cell should be empty"
    );
}
#[test]
fn r30_roundtrip_dsr() {
    let mut g = t();
    g.vt_write(b"\x1b[5n");
    g.flush();
    let r = g.drain_pty_write_responses();
    assert!(!r.is_empty(), "DSR response should not be empty");
    assert!(
        r.iter().any(|b| b.contains(&b'n')),
        "DSR response should contain 'n' (device status): {:?}",
        r
    );
}
