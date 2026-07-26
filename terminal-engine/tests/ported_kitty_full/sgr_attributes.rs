use terminal_engine::GhosttyTerminal;
use terminal_engine::vt_conformance::{check_invariants, sized_term, term};

// ====================================================================
// P1.1: Kitty termtests — SGR Attributes
// Full port of Kitty's SGR test scenarios
// ====================================================================

#[test]
fn kitty_sgr_bold() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1mBOLD\x1b[0m");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty SGR 1: bold");
    assert!(!snap.cell_at(0, 4).bold, "Kitty SGR 0: bold off");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_dim() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[2mDIM");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).dim, "Kitty SGR 2: dim");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_italic() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[3mITALIC");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).italic, "Kitty SGR 3: italic");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_underline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[4mUNDER");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).underline, "Kitty SGR 4: underline");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_blink() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[5mBLINK");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).blink, "Kitty SGR 5: blink");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_reverse() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[7mREV");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).reverse, "Kitty SGR 7: reverse");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_conceal() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[8mHIDDEN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).hidden, "Kitty SGR 8: hidden");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_strikethrough() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[9mSTRIKE");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cell_at(0, 0).strikethrough,
        "Kitty SGR 9: strikethrough"
    );
    check_invariants(&t);
}

#[test]
fn kitty_sgr_overline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[53mOVER");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).overline, "Kitty SGR 53: overline");
    check_invariants(&t);
}

// ── Attribute toggle-offs ────────────────────────────────────────

#[test]
fn kitty_sgr_22_clears_bold_dim() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;2mX\x1b[22mY");
    t.flush();
    let snap = t.take_snapshot();
    // X has bold+dim, Y should have both cleared
    assert!(!snap.cell_at(0, 1).bold, "Kitty SGR 22: bold off");
    assert!(!snap.cell_at(0, 1).dim, "Kitty SGR 22: dim off");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_23_clears_italic() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[3mX\x1b[23mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 1).italic, "Kitty SGR 23: italic off");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_24_clears_underline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[4mX\x1b[24mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 1).underline, "Kitty SGR 24: underline off");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_25_clears_blink() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[5mX\x1b[25mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 1).blink, "Kitty SGR 25: blink off");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_27_clears_reverse() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[7mX\x1b[27mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 1).reverse, "Kitty SGR 27: reverse off");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_28_clears_conceal() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[8mX\x1b[28mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 1).hidden, "Kitty SGR 28: hidden off");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_29_clears_strikethrough() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[9mX\x1b[29mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        !snap.cell_at(0, 1).strikethrough,
        "Kitty SGR 29: strikethrough off"
    );
    check_invariants(&t);
}

#[test]
fn kitty_sgr_55_clears_overline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[53mX\x1b[55mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 1).overline, "Kitty SGR 55: overline off");
    check_invariants(&t);
}

// ── SGR combined attributes ─────────────────────────────────────

#[test]
fn kitty_sgr_bold_italic() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;3mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty combo: bold");
    assert!(snap.cell_at(0, 0).italic, "Kitty combo: italic");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_bold_underline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;4mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty combo: bold+underline");
    assert!(snap.cell_at(0, 0).underline, "Kitty combo: underline");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_bold_reverse() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;7mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty combo: bold+reverse");
    assert!(snap.cell_at(0, 0).reverse, "Kitty combo: reverse");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_italic_underline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[3;4mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).italic, "Kitty combo: italic+underline");
    assert!(snap.cell_at(0, 0).underline, "Kitty combo: underline");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_three_attributes() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;3;4mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty combo 3: bold");
    assert!(snap.cell_at(0, 0).italic, "Kitty combo 3: italic");
    assert!(snap.cell_at(0, 0).underline, "Kitty combo 3: underline");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_four_attributes() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;3;4;7mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty combo 4: bold");
    assert!(snap.cell_at(0, 0).italic, "Kitty combo 4: italic");
    assert!(snap.cell_at(0, 0).underline, "Kitty combo 4: underline");
    assert!(snap.cell_at(0, 0).reverse, "Kitty combo 4: reverse");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_all_attributes() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;3;4;5;7;8;9;53mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty combo all: bold");
    assert!(snap.cell_at(0, 0).italic, "Kitty combo all: italic");
    assert!(snap.cell_at(0, 0).underline, "Kitty combo all: underline");
    assert!(snap.cell_at(0, 0).blink, "Kitty combo all: blink");
    assert!(snap.cell_at(0, 0).reverse, "Kitty combo all: reverse");
    assert!(snap.cell_at(0, 0).hidden, "Kitty combo all: hidden");
    assert!(
        snap.cell_at(0, 0).strikethrough,
        "Kitty combo all: strikethrough"
    );
    assert!(snap.cell_at(0, 0).overline, "Kitty combo all: overline");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_reset_chain() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;3;4mAB\x1b[0mC");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Kitty SGR 0: A has bold");
    assert!(snap.cell_at(0, 1).bold, "Kitty SGR 0: B has bold");
    assert!(!snap.cell_at(0, 2).bold, "Kitty SGR 0: C bold reset");
    assert!(!snap.cell_at(0, 2).italic, "Kitty SGR 0: C italic reset");
    assert!(
        !snap.cell_at(0, 2).underline,
        "Kitty SGR 0: C underline reset"
    );
    check_invariants(&t);
}

// ── SGR parameter combinations ──────────────────────────────────

#[test]
fn kitty_sgr_all_attributes_no_crash() {
    let mut t = sized_term(5, 20, 500);
    for attr in 0u8..=109u8 {
        t.vt_write(format!("\x1b[{}mX", attr).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn kitty_sgr_fg_colors_8() {
    let mut t = sized_term(5, 20, 500);
    for color in 30u8..=37u8 {
        t.vt_write(format!("\x1b[{}mX", color).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).foreground[0] >= 0.0 && snap.cell_at(0, 0).foreground[0] <= 1.0);
        t.vt_write(b"\x1b[0m");
        t.flush();
    }
    check_invariants(&t);
}

#[test]
fn kitty_sgr_bg_colors_8() {
    let mut t = sized_term(5, 20, 500);
    for color in 40u8..=47u8 {
        t.vt_write(format!("\x1b[{}mX", color).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).background[0] >= 0.0 && snap.cell_at(0, 0).background[0] <= 1.0);
        t.vt_write(b"\x1b[0m");
        t.flush();
    }
    check_invariants(&t);
}

#[test]
fn kitty_sgr_bright_fg_colors() {
    let mut t = sized_term(5, 20, 500);
    for color in 90u8..=97u8 {
        t.vt_write(format!("\x1b[{}mX", color).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn kitty_sgr_bright_bg_colors() {
    let mut t = sized_term(5, 20, 500);
    for color in 100u8..=107u8 {
        t.vt_write(format!("\x1b[{}mX", color).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

/// SGR + text correctness
#[test]
fn kitty_sgr_bold_does_not_change_codepoint() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 0).codepoint,
        'X' as u32,
        "Kitty SGR bold: codepoint unchanged"
    );
}

#[test]
fn kitty_sgr_color_does_not_change_codepoint() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[31mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 0).codepoint,
        'X' as u32,
        "Kitty SGR color: codepoint unchanged"
    );
}

// ── SGR font selectors ──────────────────────────────────────────

#[test]
fn kitty_sgr_font_selectors_safe() {
    let mut t = sized_term(5, 20, 500);
    for font in 10u8..=19u8 {
        t.vt_write(format!("\x1b[{}mX", font).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

// ── SGR underline style variants ────────────────────────────────

#[test]
fn kitty_sgr_4_1_single_underline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[4:1mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).underline, "Kitty SGR 4:1: underline");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_4_2_double_underline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[4:2mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).underline, "Kitty SGR 4:2: underline");
    check_invariants(&t);
}

#[test]
fn kitty_sgr_4_3_curly_underline() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[4:3mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).underline, "Kitty SGR 4:3: underline");
    check_invariants(&t);
}

/// SGR after reset: all should be off
#[test]
fn kitty_sgr_reset_clears_all_attributes() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[1;3;4;5;7;8;9;53mBEFORE");
    t.flush();
    t.vt_write(b"\x1b[0mAFTER");
    t.flush();
    let snap = t.take_snapshot();
    // "AFTER" starts at col 6
    let idx = 6;
    assert!(!snap.cell_at(0, idx).bold, "Kitty SGR 0: bold off");
    assert!(!snap.cell_at(0, idx).italic, "Kitty SGR 0: italic off");
    assert!(
        !snap.cell_at(0, idx).underline,
        "Kitty SGR 0: underline off"
    );
    assert!(!snap.cell_at(0, idx).blink, "Kitty SGR 0: blink off");
    assert!(!snap.cell_at(0, idx).reverse, "Kitty SGR 0: reverse off");
    assert!(!snap.cell_at(0, idx).hidden, "Kitty SGR 0: hidden off");
    assert!(
        !snap.cell_at(0, idx).strikethrough,
        "Kitty SGR 0: strikethrough off"
    );
    check_invariants(&t);
}
