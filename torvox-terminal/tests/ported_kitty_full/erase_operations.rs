use torvox_terminal::GhosttyTerminal;
use torvox_terminal::vt_conformance::{check_invariants, sized_term, term};

// ====================================================================
// P1.1: Kitty termtests — Erase Operations
// Full port of Kitty's EL/ED/ECH/DCH/ICH erase test scenarios
// ====================================================================

fn rect_text(t: &GhosttyTerminal, rows: u32, cols: u32) -> Vec<String> {
    let snap = t.take_snapshot();
    let mut result = Vec::new();
    for r in 0..rows {
        let mut line = String::new();
        for c in 0..cols {
            let idx = (r * snap.cols + c) as usize;
            let cp = snap.cells[idx].codepoint;
            line.push(char::from_u32(cp).unwrap_or('?'));
        }
        result.push(line);
    }
    result
}

// ── EL 0-2 variants ─────────────────────────────────────────────

#[test]
fn kitty_el_0_erase_to_end() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[5G\x1b[0K");
    t.flush();
    let text = rect_text(&t, 1, 20)[0].clone();
    assert!(text.starts_with("ABCD"), "Kitty EL 0: ABCD preserved");
    // Cols 4+ should be erased
    for c in 4..20 {
        let snap = t.take_snapshot();
        assert_eq!(
            snap.cell_at(0, c).codepoint,
            0,
            "Kitty EL 0: col {c} erased"
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_el_1_erase_to_start() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[5G\x1b[1K");
    t.flush();
    let text = rect_text(&t, 1, 20)[0].clone();
    assert!(
        text[5..].starts_with("FGHIJ"),
        "Kitty EL 1: FGHIJ preserved"
    );
    for c in 0..4 {
        let snap = t.take_snapshot();
        assert_eq!(
            snap.cell_at(0, c).codepoint,
            0,
            "Kitty EL 1: col {c} erased"
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_el_2_erase_line() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[2K");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(
            snap.cell_at(0, c).codepoint,
            0,
            "Kitty EL 2: col {c} erased"
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_el_0_at_home_clears_screen_region() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"AAAAABBBBBCCCCCDDDDDEEEEE");
    t.flush();
    t.vt_write(b"\x1b[H\x1b[0K");
    t.flush();
    let snap = t.take_snapshot();
    // Row 0 should be empty
    for c in 0..20 {
        assert_eq!(
            snap.cell_at(0, c).codepoint,
            0,
            "Kitty EL 0 at home: cell(0,{c}) erased"
        );
    }
    // Row 1+ should be intact
    assert_eq!(
        snap.cell_at(1, 0).codepoint,
        'B' as u32,
        "Kitty EL 0 at home: row 1 B preserved"
    );
}

#[test]
fn kitty_el_1_at_end_clears_row() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[10G\x1b[1K");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..9 {
        assert_eq!(
            snap.cell_at(0, c).codepoint,
            0,
            "Kitty EL 1: col {c} erased from left"
        );
    }
}

#[test]
fn kitty_el_2_multiple_rows() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"AAAAABBBBBCCCCCDDDDDEEEEE");
    t.flush();
    for row in 0..5 {
        t.vt_write(format!("\x1b[{};1H\x1b[2K", row + 1).as_bytes());
        t.flush();
    }
    let snap = t.take_snapshot();
    for r in 0..5 {
        for c in 0..20 {
            assert_eq!(
                snap.cell_at(r, c).codepoint,
                0,
                "Kitty EL 2 all rows: cell({r},{c}) erased"
            );
        }
    }
    check_invariants(&t);
}

// ── ED 0-2 variants ─────────────────────────────────────────────

#[test]
fn kitty_ed_0_erase_to_end_of_display() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"AAAAABBBBBCCCCCDDDDDEEEEE");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[0J");
    t.flush();
    let snap = t.take_snapshot();
    // Row 0 preserved
    assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
    // Row 1+ erased from col 0 onward
    for r in 1..5 {
        for c in 0..20 {
            assert_eq!(
                snap.cell_at(r, c).codepoint,
                0,
                "Kitty ED 0: cell({r},{c}) erased"
            );
        }
    }
    check_invariants(&t);
}

#[test]
fn kitty_ed_1_erase_to_start_of_display() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"AAAAABBBBBCCCCCDDDDDEEEEE");
    t.flush();
    t.vt_write(b"\x1b[3;1H\x1b[1J");
    t.flush();
    let snap = t.take_snapshot();
    // Row 0-1 erased
    for r in 0..2 {
        for c in 0..20 {
            assert_eq!(
                snap.cell_at(r, c).codepoint,
                0,
                "Kitty ED 1: cell({r},{c}) erased"
            );
        }
    }
    // Row 2 preserved (cursor row)
    assert_eq!(
        snap.cell_at(2, 0).codepoint,
        'C' as u32,
        "Kitty ED 1: row 2 C preserved"
    );
    check_invariants(&t);
}

#[test]
fn kitty_ed_2_erase_display() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"AAAAABBBBBCCCCCDDDDDEEEEE");
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    let snap = t.take_snapshot();
    for r in 0..5 {
        for c in 0..20 {
            assert_eq!(
                snap.cell_at(r, c).codepoint,
                0,
                "Kitty ED 2: cell({r},{c}) erased"
            );
        }
    }
    check_invariants(&t);
}

// ── ECH variants ────────────────────────────────────────────────

#[test]
fn kitty_ech_erase_chars() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[3G\x1b[3X");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
    assert_eq!(snap.cell_at(0, 1).codepoint, 'B' as u32);
    assert_eq!(snap.cell_at(0, 2).codepoint, 0, "ECH: col 2");
    assert_eq!(snap.cell_at(0, 3).codepoint, 0, "ECH: col 3");
    assert_eq!(snap.cell_at(0, 4).codepoint, 0, "ECH: col 4");
    assert_eq!(snap.cell_at(0, 5).codepoint, 'F' as u32, "ECH: col 5");
    check_invariants(&t);
}

#[test]
fn kitty_ech_erase_all_at_home() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[H\x1b[10X");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(snap.cell_at(0, c).codepoint, 0, "ECH all: col {c}");
    }
    check_invariants(&t);
}

#[test]
fn kitty_ech_beyond_end_clamps() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[5G\x1b[999X");
    t.flush();
    let snap = t.take_snapshot();
    for c in 4..20 {
        assert_eq!(snap.cell_at(0, c).codepoint, 0, "ECH clamp: col {c}");
    }
    check_invariants(&t);
}

// ── DCH variants ────────────────────────────────────────────────

#[test]
fn kitty_dch_delete_chars_shift_left() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[3G\x1b[3P");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
    assert_eq!(snap.cell_at(0, 1).codepoint, 'B' as u32);
    assert_eq!(
        snap.cell_at(0, 2).codepoint,
        'F' as u32,
        "DCH: F shifts to col 2"
    );
    assert_eq!(snap.cell_at(0, 3).codepoint, 'G' as u32, "DCH: G to col 3");
    assert_eq!(snap.cell_at(0, 4).codepoint, 'H' as u32, "DCH: H to col 4");
    check_invariants(&t);
}

#[test]
fn kitty_dch_0_equals_1() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[3G\x1b[0P");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 2).codepoint,
        'D' as u32,
        "Kitty DCH 0 = DCH 1: D at col 2"
    );
}

#[test]
fn kitty_dch_all_chars_deletes_all() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[H\x1b[10P");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(snap.cell_at(0, c).codepoint, 0, "Kitty DCH 10: col {c}");
    }
}

// ── ICH variants ────────────────────────────────────────────────

#[test]
fn kitty_ich_insert_chars_shift_right() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"CD");
    t.flush();
    t.vt_write(b"\x1b[G\x1b[2@");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 0, "ICH: col 0 blank");
    assert_eq!(snap.cell_at(0, 1).codepoint, 0, "ICH: col 1 blank");
    assert_eq!(snap.cell_at(0, 2).codepoint, 'C' as u32, "ICH: C at col 2");
    assert_eq!(snap.cell_at(0, 3).codepoint, 'D' as u32, "ICH: D at col 3");
    check_invariants(&t);
}

#[test]
fn kitty_ich_0_equals_1() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"XYZ");
    t.flush();
    t.vt_write(b"\x1b[G\x1b[0@");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 0, "ICH 0: blank inserted");
    assert_eq!(
        snap.cell_at(0, 1).codepoint,
        'X' as u32,
        "ICH 0: X shifts right"
    );
}

#[test]
fn kitty_ich_at_end_noop() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"Hello");
    t.flush();
    t.vt_write(b"\x1b[10G\x1b[5@");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 9).codepoint, 0, "ICH at end: noop");
    check_invariants(&t);
}

// ── IL/DL variants ──────────────────────────────────────────────

#[test]
fn kitty_il_insert_lines() {
    let mut t = sized_term(5, 10, 500);
    t.vt_write(b"AAABBBCCCDDDEEE");
    t.flush();
    t.vt_write(b"\x1b[3;1H\x1b[2L");
    t.flush();
    let snap = t.take_snapshot();
    let text: String = snap.cells[0..30]
        .iter()
        .filter_map(|c| char::from_u32(c.codepoint))
        .collect();
    assert_eq!(text.trim_end(), "AAABBB", "Kitty IL: AAA+BBB on rows 0-1");
    check_invariants(&t);
}

#[test]
fn kitty_dl_delete_lines() {
    let mut t = sized_term(5, 10, 500);
    t.vt_write(b"AAABBBCCCDDDEEE");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[2M");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32, "DL: row 0");
    assert_eq!(snap.cell_at(20, 0).codepoint, 'D' as u32, "DL: row 2 D");
    assert_eq!(snap.cell_at(30, 0).codepoint, 'E' as u32, "DL: row 3 E");
    check_invariants(&t);
}

// ── SU/SD variants ──────────────────────────────────────────────

#[test]
fn kitty_su_scroll_up_bottom_empty() {
    let mut t = sized_term(3, 10, 500);
    t.vt_write(b"Row1\nRow2\nRow3");
    t.flush();
    t.vt_write(b"\x1b[S");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(
            snap.cell_at(2, c).codepoint,
            0,
            "Kitty SU: bottom row empty"
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_sd_scroll_down_top_empty() {
    let mut t = sized_term(3, 10, 500);
    t.vt_write(b"Row1\nRow2\nRow3");
    t.flush();
    t.vt_write(b"\x1b[T");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(snap.cell_at(0, c).codepoint, 0, "Kitty SD: top row empty");
    }
    check_invariants(&t);
}

/// All erase sequences in one stress test (50 iterations)
#[test]
fn kitty_erase_combined_50() {
    for _ in 0..50 {
        let mut t = sized_term(5, 20, 500);
        t.vt_write(b"AAAABBBBCCCCDDDDEEEE");
        t.vt_write(b"\x1b[2;1H\x1b[0K");
        t.vt_write(b"\x1b[3;1H\x1b[K");
        t.vt_write(b"\x1b[4;1H\x1b[1K");
        t.flush();
        let snap = t.take_snapshot();
        // Row 0 intact
        assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
        // Row 1 erased from start
        assert_eq!(snap.cell_at(10, 0).codepoint, 0);
        check_invariants(&t);
    }
}
