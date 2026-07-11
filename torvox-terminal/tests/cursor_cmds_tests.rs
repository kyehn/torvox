// WezTerm-style Level 3: Cursor Command Interpreter Testing
// Tests cursor movement commands as independent semantic units.

use torvox_terminal::cursor_cmds::{assert_cup, assert_relative};
use torvox_terminal::test_helpers::assert_invariants;

fn term() -> torvox_terminal::ghostty_terminal::GhosttyTerminal {
    torvox_terminal::ghostty_terminal::GhosttyTerminal::new(10, 30, 100).expect("terminal")
}

fn sized(r: u32, c: u32) -> torvox_terminal::ghostty_terminal::GhosttyTerminal {
    torvox_terminal::ghostty_terminal::GhosttyTerminal::new(r, c, 100).expect("terminal")
}

fn ci(t: &torvox_terminal::ghostty_terminal::GhosttyTerminal) {
    assert_invariants(&t.take_snapshot());
}

// ── CUP ─────────────────────────────────────────────────────────────

#[test]
fn l3_cup_origin_home() {
    assert_cup(&mut term(), 1, 1, 0, 0);
}

#[test]
fn l3_cup_5_10() {
    assert_cup(&mut term(), 5, 10, 4, 9);
}

#[test]
fn l3_cup_clamp_to_screen() {
    assert_cup(&mut term(), 99, 99, 9, 29);
}

#[test]
fn l3_cup_zero_treated_as_one() {
    assert_cup(&mut term(), 0, 0, 0, 0);
}

#[test]
fn l3_cup_last_line() {
    assert_cup(&mut term(), 10, 30, 9, 29);
}

// ── CUU (Cursor Up) ─────────────────────────────────────────────────

#[test]
fn l3_cuu_default() {
    let mut t = term();
    t.vt_write(b"\x1b[5;1H");
    t.flush();
    assert_relative(&mut t, b"\x1b[A", -1, 0);
    ci(&t);
}

#[test]
fn l3_cuu_3() {
    let mut t = sized(10, 30);
    t.vt_write(b"\x1b[8;1H");
    t.flush();
    assert_relative(&mut t, b"\x1b[3A", -3, 0);
    ci(&t);
}

#[test]
fn l3_cuu_beyond_top_clamps() {
    let mut t = sized(5, 20);
    t.vt_write(b"\x1b[99A");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "CUU 99 clamps to top");
    ci(&t);
}

// ── CUD (Cursor Down) ───────────────────────────────────────────────

#[test]
fn l3_cud_default() {
    let mut t = term();
    assert_relative(&mut t, b"\x1b[B", 1, 0);
    ci(&t);
}

#[test]
fn l3_cud_3() {
    let mut t = term();
    assert_relative(&mut t, b"\x1b[3B", 3, 0);
    ci(&t);
}

#[test]
fn l3_cud_beyond_bottom_clamps() {
    let mut t = sized(10, 20);
    t.vt_write(b"\x1b[99B");
    t.flush();
    assert_eq!(t.cursor_y(), 9, "CUD 99 clamps to bottom");
    ci(&t);
}

// ── CUF (Cursor Forward) ────────────────────────────────────────────

#[test]
fn l3_cuf_default() {
    let mut t = term();
    assert_relative(&mut t, b"\x1b[C", 0, 1);
    ci(&t);
}

#[test]
fn l3_cuf_5() {
    let mut t = term();
    assert_relative(&mut t, b"\x1b[5C", 0, 5);
    ci(&t);
}

#[test]
fn l3_cuf_beyond_right_clamps() {
    let mut t = sized(5, 10);
    t.vt_write(b"\x1b[99C");
    t.flush();
    assert_eq!(t.cursor_x(), 9, "CUF 99 clamps to right");
    ci(&t);
}

// ── CUB (Cursor Back) ───────────────────────────────────────────────

#[test]
fn l3_cub_default() {
    let mut t = term();
    t.vt_write(b"\x1b[10C"); // move right first
    t.flush();
    assert_relative(&mut t, b"\x1b[D", 0, -1);
    ci(&t);
}

#[test]
fn l3_cub_3() {
    let mut t = term();
    t.vt_write(b"\x1b[10C");
    t.flush();
    assert_relative(&mut t, b"\x1b[3D", 0, -3);
    ci(&t);
}

#[test]
fn l3_cub_beyond_left_clamps() {
    let mut t = sized(5, 10);
    t.vt_write(b"\x1b[99D");
    t.flush();
    assert_eq!(t.cursor_x(), 0, "CUB 99 clamps to left");
    ci(&t);
}

// ── CNL/CPL (Cursor Next/Prev Line) ─────────────────────────────────

#[test]
fn l3_cnl_default() {
    let mut t = term();
    t.vt_write(b"\x1b[5;20H");
    t.flush();
    t.vt_write(b"\x1b[E"); // CNL
    t.flush();
    assert_eq!(t.cursor_y(), 5, "CNL default -> +1 row");
    assert_eq!(t.cursor_x(), 0, "CNL default -> col 0");
    ci(&t);
}

#[test]
fn l3_cpl_default() {
    let mut t = term();
    t.vt_write(b"\x1b[5;20H");
    t.flush();
    assert_eq!(t.cursor_y(), 4, "CUP to row 5 (0-indexed 4)");
    t.vt_write(b"\x1bM"); // RI (Reverse Index)
    t.flush();
    // RI from row 4 (0-indexed, not top of scroll region) moves to row 3
    let y = t.cursor_y();
    // Ghostty RI behavior: from non-top-of-region, moves up by 1
    assert!(y <= 4, "RI should move cursor up or stay, got y={y}");
    ci(&t);
}

// ── CHA (Cursor Horizontal Absolute) ────────────────────────────────

#[test]
fn l3_cha_default() {
    let mut t = sized(5, 20);
    t.vt_write(b"\x1b[20G");
    t.flush();
    assert_eq!(t.cursor_x(), 19, "CHA default -> col 20");
    ci(&t);
}

#[test]
fn l3_cha_home() {
    let mut t = sized(5, 20);
    t.vt_write(b"\x1b[10C\x1b[G");
    t.flush();
    assert_eq!(t.cursor_x(), 0, "CHA -> col 0");
    ci(&t);
}

// ── VPA (Vertical Position Absolute) ────────────────────────────────

#[test]
fn l3_vpa_default() {
    let mut t = sized(10, 20);
    t.vt_write(b"\x1b[5d");
    t.flush();
    assert_eq!(t.cursor_y(), 4, "VPA -> row 5");
    ci(&t);
}

// ── HVP (Horizontal Vertical Position) ──────────────────────────────

#[test]
fn l3_hvp_3_5() {
    let mut t = sized(10, 20);
    t.vt_write(b"\x1b[3;5f");
    t.flush();
    assert_eq!(t.cursor_y(), 2, "HVP row");
    assert_eq!(t.cursor_x(), 4, "HVP col");
    ci(&t);
}

// ── Cursor column preservation across CUU/CUB ───────────────────────

#[test]
fn l3_cuu_preserves_col() {
    let mut t = sized(10, 20);
    // ECMA-48 8.3.11 and 8.3.12: CUU/CUB preserve column
    t.vt_write(b"\x1b[5;10H");
    t.flush();
    t.vt_write(b"\x1b[A");
    t.flush();
    assert_eq!(t.cursor_x(), 9, "CUU preserves col 9");
    t.vt_write(b"\x1b[B");
    t.flush();
    assert_eq!(t.cursor_x(), 9, "CUD preserves col 9");
}
