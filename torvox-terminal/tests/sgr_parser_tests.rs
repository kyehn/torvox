// WezTerm-style Level 3: SGR Semantic Interpreter Testing
// Tests SGR parameter → Attrs semantics through GhosttyTerminal.

use torvox_terminal::sgr_parser::{SgrEffects, apply_sgr_and_read, assert_sgr0_clears_all};
use torvox_terminal::test_helpers::assert_invariants;

fn term() -> torvox_terminal::ghostty_terminal::GhosttyTerminal {
    torvox_terminal::ghostty_terminal::GhosttyTerminal::new(24, 80, 1000).expect("terminal")
}

fn ci(t: &torvox_terminal::ghostty_terminal::GhosttyTerminal) {
    assert_invariants(&t.take_snapshot());
}

// ── Individual attributes ───────────────────────────────────────────

#[test]
fn l3_sgr_parse_bold() {
    let fx = apply_sgr_and_read(&mut term(), &[1]);
    assert!(fx.bold, "SGR 1 bold");
    assert!(!fx.italic, "SGR 1 should not set italic");
}

#[test]
fn l3_sgr_parse_bold_off_sgr22() {
    let mut t = term();
    t.vt_write(b"\x1b[1m\x1b[22mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(!fx.bold, "SGR 22 should clear bold");
}

#[test]
fn l3_sgr_parse_italic() {
    let fx = apply_sgr_and_read(&mut term(), &[3]);
    assert!(fx.italic, "SGR 3 italic");
}

#[test]
fn l3_sgr_parse_underline() {
    let fx = apply_sgr_and_read(&mut term(), &[4]);
    assert!(fx.underline, "SGR 4 underline");
}

#[test]
fn l3_sgr_parse_blink() {
    let fx = apply_sgr_and_read(&mut term(), &[5]);
    assert!(fx.blink, "SGR 5 blink");
}

#[test]
fn l3_sgr_parse_reverse() {
    let fx = apply_sgr_and_read(&mut term(), &[7]);
    assert!(fx.reverse, "SGR 7 reverse");
}

#[test]
fn l3_sgr_parse_hidden() {
    let fx = apply_sgr_and_read(&mut term(), &[8]);
    assert!(fx.hidden, "SGR 8 hidden");
}

#[test]
fn l3_sgr_parse_strikethrough() {
    let fx = apply_sgr_and_read(&mut term(), &[9]);
    assert!(fx.strikethrough, "SGR 9 strikethrough");
}

#[test]
fn l3_sgr_parse_overline() {
    let fx = apply_sgr_and_read(&mut term(), &[53]);
    assert!(fx.overline, "SGR 53 overline");
}

// ── Toggle-offs ─────────────────────────────────────────────────────

#[test]
fn l3_sgr_23_italic_off_idempotent() {
    let fx = apply_sgr_and_read(&mut term(), &[23]);
    assert!(!fx.italic, "SGR 23 should clear italic");
}

#[test]
fn l3_sgr_24_underline_off() {
    let mut t = term();
    t.vt_write(b"\x1b[4m\x1b[24mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(!fx.underline, "SGR 24 should clear underline");
}

#[test]
fn l3_sgr_25_blink_off() {
    let mut t = term();
    t.vt_write(b"\x1b[5m\x1b[25mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(!fx.blink, "SGR 25 should clear blink");
}

#[test]
fn l3_sgr_27_reverse_off() {
    let mut t = term();
    t.vt_write(b"\x1b[7m\x1b[27mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(!fx.reverse, "SGR 27 should clear reverse");
}

#[test]
fn l3_sgr_28_hidden_off() {
    let mut t = term();
    t.vt_write(b"\x1b[8m\x1b[28mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(!fx.hidden, "SGR 28 should clear hidden");
}

#[test]
fn l3_sgr_29_strikethrough_off() {
    let mut t = term();
    t.vt_write(b"\x1b[9m\x1b[29mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(!fx.strikethrough, "SGR 29 should clear strikethrough");
}

#[test]
fn l3_sgr_55_overline_off() {
    let mut t = term();
    t.vt_write(b"\x1b[53m\x1b[55mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(!fx.overline, "SGR 55 should clear overline");
}

// ── Foreground colors ───────────────────────────────────────────────

#[test]
fn l3_sgr_fg_8color() {
    let tests = [
        (30u8, false),
        (31, false),
        (32, false),
        (33, false),
        (34, false),
        (35, false),
        (36, false),
        (37, false),
    ];
    for (code, _is_default) in &tests {
        let mut t = term();
        t.vt_write(format!("\x1b[{}mX", code).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        // Standard colors (30-37) should have non-zero fg
        let rgb_sum =
            snap.cells[0].foreground[0] + snap.cells[0].foreground[1] + snap.cells[0].foreground[2];
        if rgb_sum < 0.01 {
            // 30 = black may have very low values
            assert_eq!(
                *code, 30,
                "fg color {} should have non-zero fg sum {:.3}",
                code, rgb_sum
            );
        }
        ci(&t);
    }
}

#[test]
fn l3_sgr_bg_8color() {
    for code in 40u8..=47 {
        let mut t = term();
        t.vt_write(format!("\x1b[{}mX", code).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        let rgb_sum =
            snap.cells[0].background[0] + snap.cells[0].background[1] + snap.cells[0].background[2];
        if rgb_sum < 0.01 && code != 40 {
            // Only black (40) may have zero bg
            panic!(
                "bg color {} should have non-zero bg sum {:.3}",
                code, rgb_sum
            );
        }
    }
}

#[test]
fn l3_sgr_fg_256color_specific() {
    // Index 9 = bright red -> should have strong R
    let mut t = term();
    t.vt_write(b"\x1b[38;5;196mX"); // bright red
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cells[0].foreground[0] > 0.5,
        "256 red index 196: fg R > 0.5"
    );
}

#[test]
fn l3_sgr_bg_256color() {
    let mut t = term();
    t.vt_write(b"\x1b[48;5;34mX"); // green
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cells[0].background[1] > 0.3,
        "256 green index 34: bg G > 0.3"
    );
}

#[test]
fn l3_sgr_truecolor_fg() {
    let mut t = term();
    t.vt_write(b"\x1b[38;2;100;150;200mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        (snap.cells[0].foreground[0] - 0.392).abs() < 0.05,
        "truecolor fg R ~0.392"
    );
    assert!(
        (snap.cells[0].foreground[1] - 0.588).abs() < 0.05,
        "truecolor fg G ~0.588"
    );
    assert!(
        (snap.cells[0].foreground[2] - 0.784).abs() < 0.05,
        "truecolor fg B ~0.784"
    );
}

#[test]
fn l3_sgr_truecolor_bg() {
    let mut t = term();
    t.vt_write(b"\x1b[48;2;10;20;30mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        (snap.cells[0].background[0] - 0.039).abs() < 0.05,
        "truecolor bg R ~0.039"
    );
    assert!(
        (snap.cells[0].background[1] - 0.078).abs() < 0.05,
        "truecolor bg G ~0.078"
    );
    assert!(
        (snap.cells[0].background[2] - 0.118).abs() < 0.05,
        "truecolor bg B ~0.118"
    );
}

// ── SGR 0 reset ─────────────────────────────────────────────────────

#[test]
fn l3_sgr0_resets_all() {
    assert_sgr0_clears_all(&mut term());
}

#[test]
fn l3_sgr0_clears_underline() {
    let mut t = term();
    t.vt_write(b"\x1b[4;58;2;255;0;0mU\x1b[0mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 1);
    assert!(!fx.underline, "SGR 0 clears underline");
    assert!(!fx.underline_set, "SGR 0 clears underline color");
}

// ── Multi-attribute combination ──────────────────────────────────────

#[test]
fn l3_sgr_1_31_42_bold_red_on_green() {
    let mut t = term();
    t.vt_write(b"\x1b[1;31;42mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(fx.bold, "bold");
    assert!(fx.fg_set, "fg set");
    assert!(fx.bg_set, "bg set");
}

#[test]
fn l3_sgr_1_3_4_7_9_all() {
    let mut t = term();
    t.vt_write(b"\x1b[1;3;4;7;9mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(fx.bold, "bold in 1;3;4;7;9");
    assert!(fx.italic, "italic in 1;3;4;7;9");
    assert!(fx.underline, "underline in 1;3;4;7;9");
    assert!(fx.reverse, "reverse in 1;3;4;7;9");
    assert!(fx.strikethrough, "strikethrough in 1;3;4;7;9");
}

#[test]
fn l3_sgr_0_1_31_sgr0_then_bold_red() {
    let mut t = term();
    t.vt_write(b"\x1b[0;1;31mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(fx.bold, "SGR 0;1;31: bold should be set");
    assert!(fx.fg_set, "SGR 0;1;31: fg should be set");
}

// ── SGR 21 bug detection ────────────────────────────────────────────

#[test]
fn l3_sgr_21_known_ghostty_bug() {
    // SGR 21 should clear bold (ECMA-48 5th ed) but Ghostty treats it as
    // double underline.  Document this known deviation.
    let mut t = term();
    t.vt_write(b"\x1b[1m\x1b[21mX");
    t.flush();
    let fx = SgrEffects::read_from(&t, 0);
    assert!(
        fx.bold,
        "KNOWN GHOSTTY BUG: SGR 21 should clear bold (ECMA-48) but Ghostty treats as double-underline"
    );
    // Also verify we see the workaround: SGR 22 does clear bold
    let mut t2 = term();
    t2.vt_write(b"\x1b[1m\x1b[22mX");
    t2.flush();
    let fx2 = SgrEffects::read_from(&t2, 0);
    assert!(
        !fx2.bold,
        "SGR 22 correctly clears bold (workaround for SGR 21 bug)"
    );
}

// ── Font selectors ──────────────────────────────────────────────────

#[test]
fn l3_sgr_font_selectors_10_19() {
    for code in 10u8..=19 {
        let mut t = term();
        t.vt_write(format!("\x1b[{}mX", code).as_bytes());
        t.flush();
        ci(&t);
    }
}

// ── Unknown extender ranges ─────────────────────────────────────────

#[test]
fn l3_sgr_unknown_ranges_safe() {
    for code in [73u8, 74, 75, 76, 77, 78, 79, 83, 84, 85, 86, 87, 88, 89] {
        let mut t = term();
        t.vt_write(format!("\x1b[{}mX", code).as_bytes());
        t.flush();
        ci(&t);
    }
}
