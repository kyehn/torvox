use terminal_engine::GhosttyTerminal;
use terminal_engine::vt_conformance::{check_invariants, sized_term, term};

// ====================================================================
// P1.1: Kitty termtests — Tabs
// ====================================================================

#[test]
fn kitty_tab_set_and_clear() {
    let mut t = sized_term(5, 40, 500);
    // Clear all tabs
    t.vt_write(b"\x1b[3g");
    t.flush();
    // Set tab at col 10
    t.vt_write(b"\x1b[10G\x1bH");
    t.flush();
    // Tab from home
    t.vt_write(b"\x1b[H\x09");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 9, "Kitty tab: HT to col 10");
    check_invariants(&t);
}

#[test]
fn kitty_tab_multiple_stops() {
    let mut t = sized_term(5, 40, 500);
    t.vt_write(b"\x1b[3g"); // clear all
    for col in &[5u32, 15, 25, 35] {
        t.vt_write(format!("\x1b[{}G\x1bH", col).as_bytes());
        t.flush();
    }
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09X\x09Y\x09Z");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 4).codepoint,
        'X' as u32,
        "Kitty tab: X at col 5"
    );
    assert_eq!(
        snap.cell_at(0, 14).codepoint,
        'Y' as u32,
        "Kitty tab: Y at col 15"
    );
    assert_eq!(
        snap.cell_at(0, 24).codepoint,
        'Z' as u32,
        "Kitty tab: Z at col 25"
    );
    check_invariants(&t);
}

#[test]
fn kitty_tab_clear_one() {
    let mut t = sized_term(5, 40, 500);
    t.vt_write(b"\x1b[3g");
    t.vt_write(b"\x1b[10G\x1bH");
    t.vt_write(b"\x1b[20G\x1bH");
    t.vt_write(b"\x1b[10G\x1b[0g"); // clear tab at col 10
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cursor_col, 19,
        "Kitty tab: after clearing col 10, HT goes to col 20"
    );
    check_invariants(&t);
}

#[test]
fn kitty_tab_last_column_no_wrap() {
    let mut t = sized_term(5, 40, 500);
    t.vt_write(b"\x1b[3g");
    t.vt_write(b"\x1b[40G\x1bH"); // tab at last column
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 39, "Kitty tab: at last col");
    check_invariants(&t);
}

#[test]
fn kitty_tab_default_stops() {
    let mut t = sized_term(5, 40, 500);
    // Default: every 8 columns starting from 9
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 8, "Kitty default tab: col 9");
    t.vt_write(b"\x09");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 16, "Kitty default tab: col 17");
    check_invariants(&t);
}

// ── Tab + text ─────────────────────────────────────────────────

#[test]
fn kitty_tab_with_text_content() {
    let mut t = sized_term(5, 40, 500);
    t.vt_write(b"A\x09B\x09C");
    t.flush();
    let snap = t.take_snapshot();
    let text: String = snap.cells[0..30]
        .iter()
        .filter_map(|c| char::from_u32(c.codepoint))
        .collect();
    assert_eq!(text.chars().next(), Some('A'), "Kitty tab: A at col 0");
    assert_eq!(
        snap.cell_at(0, 8).codepoint,
        'B' as u32,
        "Kitty tab: B at col 8"
    );
    assert_eq!(
        snap.cell_at(0, 16).codepoint,
        'C' as u32,
        "Kitty tab: C at col 16"
    );
    check_invariants(&t);
}

#[test]
fn kitty_tab_clear_all_no_movement() {
    let mut t = sized_term(5, 40, 500);
    t.vt_write(b"A\x09B");
    t.flush();
    t.vt_write(b"\x1b[3g");
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    // After clearing all tabs, HT should go to rightmost col
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cursor_col, 39,
        "Kitty tab: no tabs, HT goes to last col"
    );
    check_invariants(&t);
}

#[test]
fn kitty_tab_chained_clear_set_cycle() {
    let mut t = sized_term(5, 40, 500);
    for _ in 0..5 {
        t.vt_write(b"\x1b[3g");
        t.flush();
        t.vt_write(b"\x1b[10G\x1bH");
        t.flush();
        t.vt_write(b"\x1b[H\x09");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cursor_col, 9, "Kitty tab cycle: col 10");
    }
    check_invariants(&t);
}
