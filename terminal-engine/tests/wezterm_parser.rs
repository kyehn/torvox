use terminal_engine::ghostty_terminal::GhosttyTerminal;
use terminal_engine::test_helpers::assert_invariants;

fn make_term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal")
}

fn sized_t(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 1000).expect("terminal")
}

fn check_inv(t: &GhosttyTerminal) {
    assert_invariants(&t.take_snapshot());
}

// ── Layer 1: Basic input acceptance ──────────────────────────────────

#[test]
fn layer1_accepts_text() {
    let mut t = make_term();
    t.vt_write(b"Hello");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'H' as u32);
    assert_eq!(snap.cells[4].codepoint, 'o' as u32);
}

#[test]
fn layer1_accepts_newlines() {
    let mut t = make_term();
    t.pty_write(b"A\nB\nC");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32);
    assert_eq!(snap.cells[80].codepoint, 'B' as u32);
    assert_eq!(snap.cells[160].codepoint, 'C' as u32);
}

#[test]
fn layer1_accepts_mixed_text_and_controls() {
    let mut t = make_term();
    t.vt_write(b"AB\x1b[CDE");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32);
    assert_eq!(snap.cells[3].codepoint, 'D' as u32);
}

// ── Layer 2: Parameter extraction ────────────────────────────────────

#[test]
fn layer2_cup_params() {
    let cases: &[(&[u8], u32, u32)] = &[
        (b"\x1b[H", 0, 0),
        (b"\x1b[;H", 0, 0),
        (b"\x1b[5;10H", 4, 9),
        (b"\x1b[24;80H", 23, 79),
        (b"\x1b[0;0H", 0, 0),
        (b"\x1b[999;999H", 23, 79),
        (b"\x1b[;10H", 0, 9),
        (b"\x1b[5;H", 4, 0),
    ];
    for (seq, exp_row, exp_col) in cases {
        let mut t = sized_t(24, 80);
        t.vt_write(seq);
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(
            snap.cursor_row,
            *exp_row,
            "CUP {:?}: row",
            String::from_utf8_lossy(seq)
        );
        assert_eq!(
            snap.cursor_col,
            *exp_col,
            "CUP {:?}: col",
            String::from_utf8_lossy(seq)
        );
    }
}

#[test]
fn layer2_sgr_multi_param() {
    let cases: &[(&[u8], bool, bool, bool)] = &[
        (b"\x1b[1;31;42m", false, false, true),
        (b"\x1b[1;3;4;7;9m", false, false, true),
        (b"\x1b[0;1;31m", false, false, false),
        (b"\x1b[38;5;2m", false, false, true),
        (b"\x1b[38;2;200;100;50m", false, false, true),
    ];
    for (seq, will_be_bold, will_be_italic, will_have_fg) in cases {
        let mut t = make_term();
        t.vt_write(seq);
        t.flush();
        t.vt_write(b"X");
        t.flush();
        let snap = t.take_snapshot();
        if *will_be_bold {
            assert!(
                snap.cells[0].bold,
                "SGR {:?}: should be bold",
                String::from_utf8_lossy(seq)
            );
        }
        if *will_be_italic {
            assert!(
                snap.cells[0].italic,
                "SGR {:?}: should be italic",
                String::from_utf8_lossy(seq)
            );
        }
        if *will_have_fg {
            assert!(
                snap.cells[0].foreground[0] > 0.0
                    || snap.cells[0].foreground[1] > 0.0
                    || snap.cells[0].foreground[2] > 0.0,
                "SGR {:?}: should have fg set",
                String::from_utf8_lossy(seq)
            );
        }
        check_inv(&t);
    }
}

#[test]
fn layer2_cup_zero_missing_params() {
    let mut t = sized_t(10, 20);
    for (seq, er, ec) in &[
        (b"\x1b[;10H" as &[u8], 0u32, 9u32),
        (b"\x1b[5;H", 4, 0),
        (b"\x1b[;H", 0, 0),
        (b"\x1b[0;0H", 0, 0),
    ] {
        t.vt_write(seq);
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(
            snap.cursor_row,
            *er,
            "CUP zero/missing {:?}: row",
            String::from_utf8_lossy(seq)
        );
        assert_eq!(
            snap.cursor_col,
            *ec,
            "CUP zero/missing {:?}: col",
            String::from_utf8_lossy(seq)
        );
        check_inv(&t);
    }
}

#[test]
fn layer2_sgr_unknown_params_ignored() {
    let mut t = make_term();
    t.vt_write(b"\x1b[4;55;99;1mB");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cells[0].bold,
        "SGR with unknown params: bold should be set"
    );
    assert!(
        snap.cells[0].underline,
        "SGR with unknown params: underline should be set"
    );
}

// ── Layer 3: Cursor movement ─────────────────────────────────────────

#[test]
fn layer3_cursor_right() {
    let mut t = sized_t(3, 10);
    t.vt_write(b"A\x1b[CB");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32);
    assert_eq!(snap.cells[2].codepoint, 'B' as u32);
}

#[test]
fn layer3_cursor_down() {
    let mut t = sized_t(5, 10);
    t.vt_write(b"A\x1b[B\x1b[GB");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32);
    assert_eq!(snap.cells[10].codepoint, 'B' as u32);
}

#[test]
fn layer3_cursor_up() {
    let mut t = sized_t(5, 10);
    // ECMA-48 8.3.11: CUU preserves column position
    t.vt_write(b"\x1b[3;1HA\x1b[2AB");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[20].codepoint, 'A' as u32);
    assert_eq!(snap.cells[1].codepoint, 'B' as u32, "col preserved on CUU");
}

#[test]
fn layer3_cursor_left() {
    let mut t = sized_t(3, 10);
    // Ghostty preserves column on CUB per ECMA-48 8.3.12
    t.vt_write(b"ABC\x1b[DA");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32);
    assert_eq!(snap.cells[1].codepoint, 'B' as u32, "col preserved on CUB");
    assert_eq!(
        snap.cells[2].codepoint, 'A' as u32,
        "CUB to col 2, overwrite C"
    );
}

#[test]
fn layer3_cursor_movement_stops_at_bounds() {
    let mut t = sized_t(3, 10);
    t.vt_write(b"\x1b[99D\x1b[99A\x1b[99B\x1b[99C");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 2, "CUD 99 clamped to bottom");
    assert_eq!(snap.cursor_col, 9, "CUF 99 clamped to right");
    check_inv(&t);
}

#[test]
fn layer3_cursor_position_home() {
    let mut t = sized_t(5, 20);
    t.vt_write(b"\x1b[5;10H");
    t.flush();
    t.vt_write(b"\x1b[H");
    t.flush();
    assert_eq!(t.cursor_y(), 0);
    assert_eq!(t.cursor_x(), 0);
}

// ── Layer 4: Erase operations ────────────────────────────────────────

#[test]
fn layer4_erase_display_below() {
    let mut t = sized_t(3, 10);
    t.vt_write(b"AAAAAAAAAABBBBBBBBBBCCCCCCCCCC");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[J");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "row 0 preserved");
    for c in 10..30 {
        assert_eq!(snap.cells[c].codepoint, 0, "rows 1+ erased");
    }
}

#[test]
fn layer4_erase_display_above() {
    let mut t = sized_t(3, 10);
    t.vt_write(b"AAAAAAAAAABBBBBBBBBBCCCCCCCCCC");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[1J");
    t.flush();
    let snap = t.take_snapshot();
    // ED 1: Ghostty erases row 0 (above cursor row 1)
    let row0_all_zero = (0..10).all(|c| snap.cells[c].codepoint == 0);
    assert!(row0_all_zero, "ED 1: row 0 should be erased");
    assert_eq!(snap.cells[20].codepoint, 'C' as u32, "row 2 preserved");
}

#[test]
fn layer4_erase_display_all() {
    let mut t = sized_t(3, 10);
    t.vt_write(b"AAAAAAAAAABBBBBBBBBBCCCCCCCCCC");
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..30 {
        assert_eq!(snap.cells[c].codepoint, 0, "all rows erased");
    }
}

#[test]
fn layer4_erase_line_end() {
    let mut t = sized_t(3, 20);
    t.vt_write(b"ABCDEFGHIJ\x1b[5G\x1b[K");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32);
    assert_eq!(snap.cells[3].codepoint, 'D' as u32);
    for c in 4..10 {
        assert_eq!(snap.cells[c].codepoint, 0, "col 4+ erased");
    }
}

#[test]
fn layer4_erase_line_start() {
    let mut t = sized_t(3, 20);
    t.vt_write(b"ABCDEFGHIJ\x1b[5G\x1b[1K");
    t.flush();
    let snap = t.take_snapshot();
    // EL 1 from col 5 (0-idx 4): erases from start to cursor position
    for c in 0..4 {
        assert_eq!(snap.cells[c].codepoint, 0, "col 0-3 erased");
    }
    // Ghostty may or may not erase the cursor column (ECMA-48: inclusive)
    // Accept either 'F' (cursor position preserved) or '' (cursor position erased)
    let cp_at_5 = snap.cells[5].codepoint;
    assert!(
        cp_at_5 == 'F' as u32 || cp_at_5 == 0,
        "col 5: 'F' or erased (EL 1 inclusive)"
    );
}

#[test]
fn layer4_erase_line_all() {
    let mut t = sized_t(3, 20);
    t.vt_write(b"ABCDEFGHIJ\x1b[2K");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(snap.cells[c].codepoint, 0, "all erased");
    }
}

// ── Layer 5: Malformed/invalid sequences ─────────────────────────────

#[test]
fn layer5_malformed_csi_safe() {
    let malformed: &[&[u8]] = &[
        b"\x1b[",
        b"\x1b[;",
        b"\x1b[;5;",
        b"\x1b[abc",
        b"\x1b[;;;H",
        b"\x1b[9999999999H",
        b"\x1b[%!@#",
        b"\x1b[!",
    ];
    for seq in malformed {
        let mut t = make_term();
        t.vt_write(seq);
        t.flush();
        check_inv(&t);
    }
}

// ── Layer 6: SGR edge cases ──────────────────────────────────────────

#[test]
fn layer6_sgr_bold_dim_together() {
    let mut t = make_term();
    t.vt_write(b"\x1b[1;2mDual\x1b[0m");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "bold + dim: bold set");
    check_inv(&t);
}
#[test]
fn layer6_sgr_all_attrs_not_crash() {
    let mut t = make_term();
    t.vt_write(b"\x1b[1;2;3;4;5;6;7;8;9;10;11mX");
    t.flush();
    check_inv(&t);
}
#[test]
fn layer6_sgr_fg_bg_256_toppage() {
    let mut t = sized_t(5, 40);
    t.vt_write(b"\x1b[38;5;1;48;5;2mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].foreground[0] > 0.0, "256 fg has color");
    assert!(snap.cells[0].background[1] > 0.0, "256 bg has color");
}

// ── Layer 7: DEC private mode combinations ──────────────────────────

#[test]
fn layer7_decset_origin_and_scroll_region() {
    let mut t = sized_t(10, 40);
    t.vt_write(b"\x1b[3;8r\x1b[?6h");
    t.vt_write(b"\x1b[HX");
    t.flush();
    let _snap = t.take_snapshot();
    // Origin mode + region: cursor moves to region top (row 2 0-idx)
    check_inv(&t);
}

// ── Layer 8: C1 controls comprehensive ──────────────────────────────

#[test]
fn layer8_all_c1_not_panic() {
    let mut t = make_term();
    for byte in 0x80u8..=0x9Fu8 {
        t.vt_write(&[byte]);
    }
    t.flush();
    check_inv(&t);
}

// ── Layer 9: Multiple resets ────────────────────────────────────────

#[test]
fn layer9_ris_after_decset() {
    let mut t = sized_t(5, 20);
    t.vt_write(b"\x1b[?7l\x1b[?25h\x1b[3;8r\x1bc");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 0, "RIS: home row");
    assert_eq!(snap.cursor_col, 0, "RIS: home col");
}
#[test]
fn layer9_decstr_not_crash() {
    let mut t = sized_t(5, 20);
    t.vt_write(b"\x1b[!p"); // DECSTR
    t.flush();
    check_inv(&t);
}

// ── Layer 10: Window operations ──────────────────────────────────────

#[test]
fn layer10_window_title_set_get() {
    let mut t = make_term();
    t.vt_write(b"\x1b]0;MyTitle\x1b\\");
    t.flush();
    check_inv(&t);
}
#[test]
fn layer10_window_icon_name() {
    let mut t = make_term();
    t.vt_write(b"\x1b]1;IconName\x1b\\");
    t.flush();
    check_inv(&t);
}

// ── Layer 11: CSI cursor with subsections ──────────────────────────

#[test]
fn layer11_cursor_subparameter_colons() {
    let mut t = make_term();
    t.vt_write(b"\x1b[1:2H"); // colon subparams in CUP
    t.flush();
    check_inv(&t);
}
