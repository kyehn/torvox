use terminal_engine::GhosttyTerminal;
use terminal_engine::vt_conformance::{check_invariants, process_and_snapshot, sized_term, term};

// ====================================================================
// P1.1: Kitty termtests — Cursor Movement
// Complete port of Kitty's cursor-movement test scenarios
// ====================================================================

/// Kitty termtest: cursor_up moves cursor up by specified lines
#[test]
fn kitty_cursor_up_default() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[5;1H\x1b[A");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 3, "Kitty CUU: up 1 from row 5");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_up_0_equals_1() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[5;1H\x1b[0A");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 3, "Kitty CUU 0: behaves as 1");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_up_beyond_top_stops() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[1;1H\x1b[999A");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 0, "Kitty CUU 999: clamps to row 0");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_down_default() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[H\x1b[B");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 1, "Kitty CUD: down 1 from home");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_down_beyond_bottom_stops() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[24;1H\x1b[999B");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 23, "Kitty CUD 999: clamps at last row");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_forward_default() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[H\x1b[C");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 1, "Kitty CUF: forward 1 from col 0");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_forward_beyond_right_stops() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[H\x1b[999C");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 79, "Kitty CUF 999: clamps to col 79");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_back_default() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[1;40H\x1b[D");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 38, "Kitty CUB: back 1 from col 39");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_back_beyond_left_stops() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[H\x1b[999D");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 0, "Kitty CUB 999: clamps to col 0");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_next_line_default() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[5;10H\x1b[E");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 5, "Kitty CNL: next line from row 5");
    assert_eq!(snap.cursor_col, 0, "Kitty CNL: col resets to 0");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_prev_line_from_mid() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[10;10H\x1b[F");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 8, "Kitty CPL: prev line from row 10");
    assert_eq!(snap.cursor_col, 0, "Kitty CPL: col resets to 0");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_position_absolute_row() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[15d");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 14, "Kitty VPA: absolute row 15");
    assert_eq!(snap.cursor_col, 0, "Kitty VPA: col stays 0");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_position_absolute_col() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[40G");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 39, "Kitty CHA: absolute col 40");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_position_hvp() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[10;20f");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 9, "Kitty HVP: row 10");
    assert_eq!(snap.cursor_col, 19, "Kitty HVP: col 20");
    check_invariants(&t);
}

#[test]
fn kitty_cursor_position_cup_all_corners() {
    let pairs = &[
        (1u32, 1u32, 0u32, 0u32),
        (1u32, 80u32, 0u32, 79u32),
        (24u32, 1u32, 23u32, 0u32),
        (24u32, 80u32, 23u32, 79u32),
        (12u32, 40u32, 11u32, 39u32),
        (500u32, 500u32, 23u32, 79u32), // clamped
    ];
    for &(r, c, exp_row, exp_col) in pairs {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(format!("\x1b[{};{}H", r, c).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(
            snap.cursor_row, exp_row,
            "Kitty CUP ({r},{c}): row = {exp_row}"
        );
        assert_eq!(
            snap.cursor_col, exp_col,
            "Kitty CUP ({r},{c}): col = {exp_col}"
        );
        check_invariants(&t);
    }
}

#[test]
fn kitty_cursor_up_5_then_write() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[6;1H\x1b[5AX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(1, 0).codepoint,
        'X' as u32,
        "Kitty CUU 5 + write 'X' at row 1"
    );
}

#[test]
fn kitty_cursor_down_5_then_write() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"X\x1b[5B");
    t.flush();
    t.vt_write(b"Y");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 0).codepoint,
        'X' as u32,
        "Kitty: X at (0,0)"
    );
    assert_eq!(
        snap.cell_at(5, 0).codepoint,
        'Y' as u32,
        "Kitty CUD 5: Y at (5,0)"
    );
}

#[test]
fn kitty_cursor_right_10_then_write() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[11CX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 10).codepoint,
        'X' as u32,
        "Kitty CUF 11: X at col 10"
    );
}

#[test]
fn kitty_cursor_left_10_from_mid() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[1;20H\x1b[10D");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 9, "Kitty CUB 10 from col 19: col 9");
}

#[test]
fn kitty_cursor_movement_carriage_return() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"Hello\r");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 0, "Kitty CR: col 0 after CR");
    assert_eq!(
        snap.cells[0].codepoint, 'H' as u32,
        "Kitty CR: H still at col 0"
    );
}

#[test]
fn kitty_cursor_movement_next_line_multiple() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[H\x1b[3E");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 2, "Kitty CNL 3: row 3");
    assert_eq!(snap.cursor_col, 0, "Kitty CNL 3: col 0");
}

#[test]
fn kitty_cursor_movement_prev_line_multiple() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[10;1H\x1b[3F");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 6, "Kitty CPL 3 from row 10: row 7");
}

#[test]
fn kitty_cursor_movement_hvp_relative() {
    let mut t = sized_term(24, 80, 500);
    t.vt_write(b"\x1b[10;10f\x1b[3e");
    t.flush();
    // HVP 10;10 → VPR 3 → row 12 (0-idx)
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 12, "Kitty HVP + VPR: row 13");
    assert_eq!(snap.cursor_col, 9, "Kitty HVP + VPR: col 10");
}

/// Combined cursor movement stress test — moves from 40 scenarios
#[test]
fn kitty_cursor_movement_all_sequences_combined_50() {
    for _ in 0..50 {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(b"\x1b[H");
        t.vt_write(b"\x1b[12;40H");
        t.vt_write(b"\x1b[5A");
        t.vt_write(b"\x1b[3B");
        t.vt_write(b"\x1b[10C");
        t.vt_write(b"\x1b[4D");
        t.vt_write(b"O");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cell_at(9, 45).codepoint, 'O' as u32);
        check_invariants(&t);
    }
}
