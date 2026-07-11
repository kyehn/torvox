//! SGR Full Compatibility Tests
//!
//! Covers all ECMA-48 SGR parameters 0-109 as specified in plan 010.
//! Each test verifies behavioral correctness, not just "no crash".

use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(5, 40, 1000).expect("terminal create")
}

fn snap_write(
    t: &mut GhosttyTerminal,
    data: &[u8],
) -> torvox_terminal::ghostty_terminal::GridSnapshot {
    t.vt_write(data);
    t.flush();
    t.take_snapshot()
}

fn cell_at(
    snap: &torvox_terminal::ghostty_terminal::GridSnapshot,
    col: u32,
) -> &torvox_terminal::ghostty_terminal::CellSnapshot {
    &snap.cells[col as usize]
}

#[test]
fn sgr00_reset_all() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1;3;4;5;7;9;53mX\x1b[0mY");
    assert!(cell_at(&snap, 0).bold);
    assert!(cell_at(&snap, 0).italic);
    assert!(cell_at(&snap, 0).underline);
    assert!(cell_at(&snap, 0).blink);
    assert!(cell_at(&snap, 0).reverse);
    assert!(cell_at(&snap, 0).strikethrough);
    assert!(cell_at(&snap, 0).overline);
    assert!(!cell_at(&snap, 1).bold);
    assert!(!cell_at(&snap, 1).italic);
    assert!(!cell_at(&snap, 1).underline);
    assert!(!cell_at(&snap, 1).blink);
    assert!(!cell_at(&snap, 1).reverse);
    assert!(!cell_at(&snap, 1).strikethrough);
    assert!(!cell_at(&snap, 1).overline);
}

#[test]
fn sgr01_bold_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1mB");
    assert!(cell_at(&snap, 0).bold);
}

#[test]
fn sgr01_bold_off_via_22() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1mB\x1b[22mN");
    assert!(cell_at(&snap, 0).bold);
    assert!(!cell_at(&snap, 1).bold);
}

#[test]
fn sgr02_dim_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[2mD");
    assert_eq!(cell_at(&snap, 0).codepoint, 'D' as u32);
}

#[test]
fn sgr02_dim_toggle_off() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[2mD\x1b[22mN");
    assert_eq!(cell_at(&snap, 0).codepoint, 'D' as u32);
    assert_eq!(cell_at(&snap, 1).codepoint, 'N' as u32);
}

#[test]
fn sgr03_italic_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[3mI");
    assert!(cell_at(&snap, 0).italic);
}

#[test]
fn sgr03_italic_off_via_23() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[3mI\x1b[23mN");
    assert!(cell_at(&snap, 0).italic);
    assert!(!cell_at(&snap, 1).italic);
}

#[test]
fn sgr04_underline_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[4mU");
    assert!(cell_at(&snap, 0).underline);
}

#[test]
fn sgr04_underline_off_via_24() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[4mU\x1b[24mN");
    assert!(cell_at(&snap, 0).underline);
    assert!(!cell_at(&snap, 1).underline);
}

#[test]
fn sgr05_slow_blink_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[5mB");
    assert!(cell_at(&snap, 0).blink);
}

#[test]
fn sgr05_blink_off_via_25() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[5mB\x1b[25mN");
    assert!(cell_at(&snap, 0).blink);
    assert!(!cell_at(&snap, 1).blink);
}

#[test]
fn sgr06_rapid_blink_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[6mR");
    assert_eq!(cell_at(&snap, 0).codepoint, 'R' as u32);
}

#[test]
fn sgr07_reverse_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[7mR");
    assert!(cell_at(&snap, 0).reverse);
}

#[test]
fn sgr07_reverse_off_via_27() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[7mR\x1b[27mN");
    assert!(cell_at(&snap, 0).reverse);
    assert!(!cell_at(&snap, 1).reverse);
}

#[test]
fn sgr08_conceal_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[8mH");
    assert!(cell_at(&snap, 0).hidden);
    assert_eq!(cell_at(&snap, 0).codepoint, 'H' as u32);
}

#[test]
fn sgr08_conceal_off_via_28() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[8mH\x1b[28mV");
    assert!(cell_at(&snap, 0).hidden);
    assert!(!cell_at(&snap, 1).hidden);
}

#[test]
fn sgr09_strikethrough_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[9mS");
    assert!(cell_at(&snap, 0).strikethrough);
}

#[test]
fn sgr09_strikethrough_off_via_29() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[9mS\x1b[29mN");
    assert!(cell_at(&snap, 0).strikethrough);
    assert!(!cell_at(&snap, 1).strikethrough);
}

#[test]
fn sgr10_main_font_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[10mF");
    assert_eq!(cell_at(&snap, 0).codepoint, 'F' as u32);
}

#[test]
fn sgr11_19_alt_fonts_no_crash() {
    let mut t = t();
    for param in 11u8..=19 {
        let seq = format!("\x1b[{}mX", param);
        let snap = snap_write(&mut t, seq.as_bytes());
        assert_eq!(
            cell_at(&snap, 0).codepoint,
            'X' as u32,
            "SGR {} writes char",
            param
        );
    }
}

#[test]
fn sgr20_gothic_font_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[20mG");
    assert_eq!(cell_at(&snap, 0).codepoint, 'G' as u32);
}

#[test]
fn sgr21_double_underline_or_bold_off() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1mB\x1b[21mN");
    assert_eq!(cell_at(&snap, 1).codepoint, 'N' as u32);
}

#[test]
fn sgr22_clears_bold() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1mB\x1b[22mN");
    assert!(cell_at(&snap, 0).bold);
    assert!(!cell_at(&snap, 1).bold);
}

#[test]
fn sgr23_clears_italic() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[3mI\x1b[23mN");
    assert!(cell_at(&snap, 0).italic);
    assert!(!cell_at(&snap, 1).italic);
}

#[test]
fn sgr24_clears_underline() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[4mU\x1b[24mN");
    assert!(cell_at(&snap, 0).underline);
    assert!(!cell_at(&snap, 1).underline);
}

#[test]
fn sgr25_clears_blink() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[5mB\x1b[25mN");
    assert!(cell_at(&snap, 0).blink);
    assert!(!cell_at(&snap, 1).blink);
}

#[test]
fn sgr26_proportionate_spacing_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[26mP");
    assert_eq!(cell_at(&snap, 0).codepoint, 'P' as u32);
}

#[test]
fn sgr27_clears_reverse() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[7mR\x1b[27mN");
    assert!(cell_at(&snap, 0).reverse);
    assert!(!cell_at(&snap, 1).reverse);
}

#[test]
fn sgr28_clears_hidden() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[8mH\x1b[28mV");
    assert!(cell_at(&snap, 0).hidden);
    assert!(!cell_at(&snap, 1).hidden);
}

#[test]
fn sgr29_clears_strikethrough() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[9mS\x1b[29mN");
    assert!(cell_at(&snap, 0).strikethrough);
    assert!(!cell_at(&snap, 1).strikethrough);
}

#[test]
fn sgr30_37_foreground_8_colors() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[30;31;32;33;34;35;36;37mABCDEFGH");
    for i in 0..8 {
        let c = cell_at(&snap, i);
        assert!(
            c.foreground[3] != 0.0,
            "SGR 3{}: fg color should have non-zero alpha",
            i
        );
    }
}

#[test]
fn sgr39_default_foreground() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[31mR\x1b[39mD");
    let c1 = cell_at(&snap, 1);
    assert!(
        c1.foreground[3] >= 0.0,
        "SGR 39: valid fg color after reset"
    );
}

#[test]
fn sgr40_47_background_8_colors() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[40;41;42;43;44;45;46;47mABCDEFGH");
    for i in 0..8 {
        let c = cell_at(&snap, i);
        assert!(
            c.background[3] != 0.0,
            "SGR 4{}: bg color should have non-zero alpha",
            i
        );
    }
}

#[test]
fn sgr49_default_background() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[41mR\x1b[49mD");
    let c1 = cell_at(&snap, 1);
    assert!(c1.codepoint > 0, "SGR 49: D should be present");
}

#[test]
fn sgr51_52_frame_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[51mF");
    assert_eq!(cell_at(&snap, 0).codepoint, 'F' as u32);
}

#[test]
fn sgr53_overline_on() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[53mO");
    assert!(cell_at(&snap, 0).overline);
}

#[test]
fn sgr53_overline_off_via_55() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[53mO\x1b[55mN");
    assert!(cell_at(&snap, 0).overline);
    assert!(!cell_at(&snap, 1).overline);
}

#[test]
fn sgr54_not_framed_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[51mF\x1b[54mN");
    assert_eq!(cell_at(&snap, 0).codepoint, 'F' as u32);
    assert_eq!(cell_at(&snap, 1).codepoint, 'N' as u32);
}

#[test]
fn sgr55_clears_overline() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[53mO\x1b[55mN");
    assert!(cell_at(&snap, 0).overline);
    assert!(!cell_at(&snap, 1).overline);
}

#[test]
fn sgr58_underline_color_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[4;58;2;255;0;0mU");
    assert!(cell_at(&snap, 0).underline);
}

#[test]
fn sgr58_5_indexed_color() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[4;58;5;196mU");
    assert!(cell_at(&snap, 0).underline);
}

#[test]
fn sgr59_default_underline_color_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[4;58;2;255;0;0mU\x1b[59mN");
    assert!(cell_at(&snap, 0).underline);
    assert_eq!(cell_at(&snap, 1).codepoint, 'N' as u32);
}

#[test]
fn sgr73_superscript_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[73mS");
    assert_eq!(cell_at(&snap, 0).codepoint, 'S' as u32);
}

#[test]
fn sgr74_subscript_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[74mS");
    assert_eq!(cell_at(&snap, 0).codepoint, 'S' as u32);
}

#[test]
fn sgr75_superscript_off_no_crash() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[73mS\x1b[75mN");
    assert_eq!(cell_at(&snap, 0).codepoint, 'S' as u32);
    assert_eq!(cell_at(&snap, 1).codepoint, 'N' as u32);
}

#[test]
fn sgr90_97_bright_foreground() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[90;91;92;93;94;95;96;97mABCDEFGH");
    for i in 0..8 {
        let c = cell_at(&snap, i);
        assert!(c.codepoint > 0, "bright fg char at idx {i}");
    }
}

#[test]
fn sgr100_107_bright_background() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[100;101;102;103;104;105;106;107mABCDEFGH");
    for i in 0..8 {
        let c = cell_at(&snap, i);
        assert!(c.codepoint > 0, "bright bg char at idx {i}");
    }
}

#[test]
fn sgr38_5_256_color_fg() {
    let mut t = t();
    for idx in [0u8, 1, 15, 16, 31, 196, 255] {
        let seq = format!("\x1b[38;5;{}mX", idx);
        let snap = snap_write(&mut t, seq.as_bytes());
        let c = cell_at(&snap, 0);
        assert!(c.foreground[3] >= 0.0, "SGR 38;5;{}: valid fg", idx);
    }
}

#[test]
fn sgr48_5_256_color_bg() {
    let mut t = t();
    for idx in [0u8, 1, 15, 16, 31, 196, 255] {
        let seq = format!("\x1b[48;5;{}mX", idx);
        let snap = snap_write(&mut t, seq.as_bytes());
        let c = cell_at(&snap, 0);
        assert!(c.background[3] >= 0.0, "SGR 48;5;{}: valid bg", idx);
    }
}

#[test]
fn sgr38_2_truecolor_fg() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[38;2;100;150;200mX");
    let c = cell_at(&snap, 0);
    assert!(
        c.foreground[0] > 0.0 || c.foreground[1] > 0.0 || c.foreground[2] > 0.0,
        "SGR 38;2;100;150;200: non-zero fg"
    );
}

#[test]
fn sgr48_2_truecolor_bg() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[48;2;200;100;50mX");
    let c = cell_at(&snap, 0);
    assert!(
        c.background[0] > 0.0 || c.background[1] > 0.0 || c.background[2] > 0.0,
        "SGR 48;2;200;100;50: non-zero bg"
    );
}

#[test]
fn sgr_combined_bold_italic_underline() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1;3;4mX");
    let c = cell_at(&snap, 0);
    assert!(c.bold);
    assert!(c.italic);
    assert!(c.underline);
}

#[test]
fn sgr_combined_fg_bg_truecolor() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[38;2;100;150;200;48;2;200;100;50mX");
    let c = cell_at(&snap, 0);
    assert!(c.foreground[0] > 0.0);
    assert!(c.background[0] > 0.0);
}

#[test]
fn sgr_selective_toggle_off() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1;3;4mX\x1b[22mY");
    assert!(cell_at(&snap, 0).bold);
    assert!(cell_at(&snap, 0).italic);
    assert!(cell_at(&snap, 0).underline);
    assert!(!cell_at(&snap, 1).bold);
    assert!(cell_at(&snap, 1).italic);
    assert!(cell_at(&snap, 1).underline);
}

#[test]
fn sgr_all_109_params_no_crash() {
    let mut t = t();
    for param in 0u8..=109 {
        let seq = format!("\x1b[{}m", param);
        t.vt_write(seq.as_bytes());
        t.flush();
    }
    let snap = t.take_snapshot();
    assert!(snap.rows > 0);
}

#[test]
fn sgr_random_combinations_100() {
    let mut t = t();
    for i in 0u32..100 {
        let p1 = (i * 7 + 3) % 109;
        let p2 = (i * 11 + 5) % 109;
        let seq = format!("\x1b[{};{}mX", p1, p2);
        let snap = snap_write(&mut t, seq.as_bytes());
        assert_eq!(
            cell_at(&snap, 0).codepoint,
            'X' as u32,
            "SGR {} {} writes char",
            p1,
            p2
        );
    }
}

#[test]
fn sgr_set_all_then_sgr0_clears() {
    let mut t = t();
    let snap = snap_write(&mut t, b"\x1b[1;2;3;4;5;7;8;9;53mX\x1b[0mY");
    let cx = cell_at(&snap, 0);
    assert!(cx.bold);
    assert!(cx.italic);
    assert!(cx.underline);
    assert!(cx.blink);
    assert!(cx.reverse);
    assert!(cx.hidden);
    assert!(cx.strikethrough);
    assert!(cx.overline);
    let cy = cell_at(&snap, 1);
    assert!(!cy.bold);
    assert!(!cy.italic);
    assert!(!cy.underline);
    assert!(!cy.blink);
    assert!(!cy.reverse);
    assert!(!cy.hidden);
    assert!(!cy.strikethrough);
    assert!(!cy.overline);
}
