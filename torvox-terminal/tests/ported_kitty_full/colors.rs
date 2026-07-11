use torvox_terminal::GhosttyTerminal;
use torvox_terminal::vt_conformance::{check_invariants, sized_term, term};

// ====================================================================
// P1.1: Kitty termtests — Colors (8/16/256/truecolor)
// ====================================================================

fn color_channel_range(c: &[f32; 3]) -> bool {
    c[0] >= 0.0 && c[0] <= 1.0 && c[1] >= 0.0 && c[1] <= 1.0 && c[2] >= 0.0 && c[2] <= 1.0
}

// ── 8 standard colors ───────────────────────────────────────────

#[test]
fn kitty_color_30_37_fg() {
    let mut t = sized_term(5, 20, 500);
    for c in 30..=37 {
        t.vt_write(format!("\x1b[{}mX\x1b[0m", c).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            color_channel_range(&snap.cell_at(0, 0).foreground),
            "Kitty color {}: fg in range",
            c
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_color_40_47_bg() {
    let mut t = sized_term(5, 20, 500);
    for c in 40..=47 {
        t.vt_write(format!("\x1b[{}mX\x1b[0m", c).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            color_channel_range(&snap.cell_at(0, 0).background),
            "Kitty color {}: bg in range",
            c
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_color_90_97_bright_fg() {
    let mut t = sized_term(5, 20, 500);
    for c in 90..=97 {
        t.vt_write(format!("\x1b[{}mX\x1b[0m", c).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn kitty_color_100_107_bright_bg() {
    let mut t = sized_term(5, 20, 500);
    for c in 100..=107 {
        t.vt_write(format!("\x1b[{}mX\x1b[0m", c).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

// ── 256 colors using 38:5 / 48:5 ─────────────────────────────────

#[test]
fn kitty_color_256_fg_all() {
    let mut t = sized_term(5, 20, 500);
    for idx in 0u8..=255u8 {
        t.vt_write(format!("\x1b[38;5;{}mX\x1b[0m", idx).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            color_channel_range(&snap.cell_at(0, 0).foreground),
            "Kitty 256 fg {}: fg in range",
            idx
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_color_256_bg_all() {
    let mut t = sized_term(5, 20, 500);
    for idx in 0u8..=255u8 {
        t.vt_write(format!("\x1b[48;5;{}mX\x1b[0m", idx).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            color_channel_range(&snap.cell_at(0, 0).background),
            "Kitty 256 bg {}: bg in range",
            idx
        );
    }
    check_invariants(&t);
}

#[test]
fn kitty_color_256_fg_bg_pair() {
    for fg in &[0u8, 1u8, 15u8, 16u8, 128u8, 231u8, 255u8] {
        for bg in &[0u8, 7u8, 8u8, 196u8, 232u8, 255u8] {
            let mut t = sized_term(5, 20, 500);
            t.vt_write(format!("\x1b[38;5;{};48;5;{}mX\x1b[0m", fg, bg).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert!(
                color_channel_range(&snap.cell_at(0, 0).foreground),
                "Kitty 256 fg {}/bg {}: fg in range",
                fg,
                bg
            );
            assert!(
                color_channel_range(&snap.cell_at(0, 0).background),
                "Kitty 256 fg {}/bg {}: bg in range",
                fg,
                bg
            );
            check_invariants(&t);
        }
    }
}

// ── Truecolor 38:2 / 48:2 ───────────────────────────────────────

#[test]
fn kitty_color_truecolor_fg() {
    let colors = &[
        (255, 0, 0),
        (0, 255, 0),
        (0, 0, 255),
        (128, 128, 128),
        (0, 0, 0),
        (255, 255, 255),
        (123, 45, 67),
        (200, 150, 100),
    ];
    for (r, g, b) in colors {
        let mut t = sized_term(5, 20, 500);
        t.vt_write(format!("\x1b[38;2;{};{};{}mX\x1b[0m", r, g, b).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn kitty_color_truecolor_bg() {
    let colors = &[(255, 0, 0), (0, 255, 0), (0, 0, 255)];
    for (r, g, b) in colors {
        let mut t = sized_term(5, 20, 500);
        t.vt_write(format!("\x1b[48;2;{};{};{}mX\x1b[0m", r, g, b).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn kitty_color_truecolor_fg_bg() {
    for (fr, fg, fb, br, bg, bb) in &[
        (255, 0, 0, 0, 0, 255),
        (0, 255, 0, 255, 0, 0),
        (255, 255, 255, 0, 0, 0),
    ] {
        let mut t = sized_term(5, 20, 500);
        t.vt_write(
            format!(
                "\x1b[38;2;{};{};{};48;2;{};{};{}mX\x1b[0m",
                fr, fg, fb, br, bg, bb
            )
            .as_bytes(),
        );
        t.flush();
        check_invariants(&t);
    }
}

// ── Default colors ──────────────────────────────────────────────

#[test]
fn kitty_color_default_fg() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"X");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        color_channel_range(&snap.cell_at(0, 0).foreground),
        "Kitty default fg: in range"
    );
}

#[test]
fn kitty_color_default_bg() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"X");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        color_channel_range(&snap.cell_at(0, 0).background),
        "Kitty default bg: in range"
    );
}

// ── Underline color (58:2) ──────────────────────────────────────

#[test]
fn kitty_color_underline_truecolor() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b[4;58;2;255;0;0mUnderline\x1b[0m");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_color_all_formats_no_crash() {
    let mut t = sized_term(5, 30, 500);
    let seqs = &[
        b"\x1b[31m",
        b"\x1b[38;5;128m",
        b"\x1b[38;2;128;64;32m",
        b"\x1b[48;2;32;64;128m",
        b"\x1b[91m",
        b"\x1b[38;5;255;48;5;128m",
        b"\x1b[4;58;2;0;255;0m",
    ];
    for seq in seqs {
        t.vt_write(seq);
        t.flush();
        t.vt_write(b"X");
        t.flush();
        check_invariants(&t);
    }
}
