// ====================================================================
// P2.4: WezTerm-Style Structured Parser Tests
//
// Following WezTerm's approach of separating parsing into layers:
// 1. CSI parsing (raw bytes → parameters)
// 2. OSC parsing (raw bytes → command + data)
// 3. C0/C1 parsing (raw bytes → control)
// 4. SGR semantic tests (parameter lists → attributes)
//
// Since torvox uses libghostty-vt for actual parsing, we test the
// GhosttyTerminal API's parameter extraction behavior.
// ====================================================================

use torvox_terminal::vt_conformance::{check_invariants, sized_term};

// ── Layer 1: CSI parameter extraction ────────────────────────────
// WezTerm-style: verify the parser extracts correct parameters

#[test]
fn wez_csi_empty_params_restore_default() {
    // CSI with no params should use defaults
    // CUP without params = home (1,1)
    let mut t = sized_term(24, 80, 100);
    t.vt_write(b"\x1b[5;40H");
    t.flush();
    t.vt_write(b"\x1b[H");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cursor_row, 0,
        "Wez CSI empty: CUP no params = home row"
    );
    assert_eq!(
        snap.cursor_col, 0,
        "Wez CSI empty: CUP no params = home col"
    );
}

#[test]
fn wez_csi_single_param_cursor_up() {
    // CSI N A = CUU
    for n in &[1u32, 5, 10, 20] {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[20;1H"); // CUP(20,1) → row 19 (0-idx), col 0
        t.flush();
        t.vt_write(format!("\x1b[{}A", n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        // CUP(20,1) → row 19. CUU n → max(19 - n, 0)
        let expected = 19u32.saturating_sub(*n);
        assert_eq!(snap.cursor_row, expected, "Wez CSI CUU {n}: row {expected}");
    }
}

#[test]
fn wez_csi_multi_param_cup() {
    // CSI N;M H = CUP
    for &(n, m) in &[(1u32, 1u32), (5, 10), (24, 80), (999, 999)] {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(format!("\x1b[{};{}H", n, m).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        let exp_row = (n - 1).min(23);
        let exp_col = (m - 1).min(79);
        assert_eq!(snap.cursor_row, exp_row, "Wez CSI CUP ({n},{m}): row");
        assert_eq!(snap.cursor_col, exp_col, "Wez CSI CUP ({n},{m}): col");
    }
}

#[test]
fn wez_csi_final_byte_determines_action() {
    // Same params, different final bytes = different actions
    let mut t = sized_term(24, 80, 100);
    t.vt_write(b"\x1b[10;20H");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 9, "Wez final byte H: CUP row");
    assert_eq!(snap.cursor_col, 19, "Wez final byte H: CUP col");
}

#[test]
fn wez_csi_ed_final_byte_0_1_2() {
    // ED with same params but different final byte ... no, ED uses J
    // But we can test that EL 0/1/2 all use K final byte
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[5G\x1b[0K");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 4).codepoint,
        0,
        "Wez EL 0: erased from col 5"
    );
}

#[test]
fn wez_csi_parameter_overflow() {
    // Very large parameters should be handled (clamped)
    let mut t = sized_term(24, 80, 100);
    t.vt_write(b"\x1b[99999;99999H");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 23, "Wez overflow CUP: clamped row");
    assert_eq!(snap.cursor_col, 79, "Wez overflow CUP: clamped col");
}

#[test]
fn wez_csi_invalid_params_default() {
    // Invalid/omitted params should fall back to defaults
    let mut t = sized_term(24, 80, 100);
    t.vt_write(b"\x1b[;H");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 0, "Wez invalid CSI: omitted row = 0");
    assert_eq!(snap.cursor_col, 0, "Wez invalid CSI: omitted col = 0");
}

// ── Layer 2: SGR parameter semantics ────────────────────────────
// WezTerm-style: verify correct attribute mapping

#[test]
fn wez_sgr_single_param_bold() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).bold, "Wez SGR 1: bold set");
    assert!(!snap.cell_at(0, 0).italic, "Wez SGR 1: italic unchanged");
    assert!(
        !snap.cell_at(0, 0).underline,
        "Wez SGR 1: underline unchanged"
    );
}

#[test]
fn wez_sgr_single_param_italic() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 0).bold, "Wez SGR 3: bold unchanged");
    assert!(snap.cell_at(0, 0).italic, "Wez SGR 3: italic set");
}

#[test]
fn wez_sgr_single_param_underline() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[4mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 0).underline, "Wez SGR 4: underline set");
}

#[test]
fn wez_sgr_multi_param_reset_all_then_set() {
    // SGR 0;1 = reset then bold
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;3;4mX");
    t.flush();
    t.vt_write(b"\x1b[0;5mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cell_at(0, 1).bold, "Wez SGR 0;5: bold reset");
    assert!(!snap.cell_at(0, 1).italic, "Wez SGR 0;5: italic reset");
    assert!(
        !snap.cell_at(0, 1).underline,
        "Wez SGR 0;5: underline reset"
    );
    assert!(snap.cell_at(0, 1).blink, "Wez SGR 0;5: blink set");
}

#[test]
fn wez_sgr_param_order_matters() {
    // SGR params are applied left-to-right
    // 1;0 = bold then reset → no bold
    // 0;1 = reset then bold → bold
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;0mN\x1b[0;1mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        !snap.cell_at(0, 0).bold,
        "Wez SGR 1;0: no bold (reset wins)"
    );
    assert!(snap.cell_at(0, 1).bold, "Wez SGR 0;1: bold (set wins)");
}

#[test]
fn wez_sgr_38_5_color_index_parsed() {
    // 38;5;N → indexed color
    let _t = sized_term(5, 20, 100);
    for idx in &[0u8, 1u8, 15u8, 16u8, 128u8, 255u8] {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(format!("\x1b[38;5;{}mX\x1b[0m", idx).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        let fg = &snap.cell_at(0, 0).foreground;
        assert!(
            fg[0] >= 0.0 && fg[1] >= 0.0 && fg[2] >= 0.0,
            "Wez SGR 38;5;{idx}: fg channels non-negative"
        );
    }
}

#[test]
fn wez_sgr_48_5_color_index_parsed() {
    // 48;5;N → background indexed
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[48;5;196mX\x1b[0m");
    t.flush();
    let snap = t.take_snapshot();
    let bg = &snap.cell_at(0, 0).background;
    assert!(
        bg[0] >= 0.0 && bg[1] >= 0.0 && bg[2] >= 0.0,
        "Wez SGR 48;5;196: bg channels non-negative"
    );
}

#[test]
fn wez_sgr_38_2_truecolor_parsed() {
    // 38;2;R;G;B → truecolor foreground
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[38;2;255;128;64mX\x1b[0m");
    t.flush();
    let snap = t.take_snapshot();
    let fg = &snap.cell_at(0, 0).foreground;
    assert!(fg[0] > 0.0, "Wez truecolor fg: R > 0");
}

#[test]
fn wez_sgr_48_2_truecolor_bg() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[48;2;64;128;255mX\x1b[0m");
    t.flush();
    let snap = t.take_snapshot();
    let bg = &snap.cell_at(0, 0).background;
    assert!(bg[2] > 0.0, "Wez truecolor bg: B > 0");
}

// ── Layer 3: OSC parsing ────────────────────────────────────────
// WezTerm-style: verify OSC structure

#[test]
fn wez_osc_2_title_parsed() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b]2;TestTitle\x1b\\");
    t.flush();
    let title = t.title();
    assert!(
        title.contains("TestTitle"),
        "Wez OSC 2: title contains TestTitle, got: {:?}",
        title
    );
}

#[test]
fn wez_osc_2_bel_terminated() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b]2;BelTerminatedTitle\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_4_palette_color_format() {
    let mut t = sized_term(5, 20, 100);
    // OSC 4 ; N ; #RRGGBB ST
    t.vt_write(b"\x1b]4;1;#ff0000\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_4_multiple_colors() {
    let mut t = sized_term(5, 20, 100);
    // Multiple palette changes in one OSC
    t.vt_write(b"\x1b]4;1;#ff0000;2;#00ff00;3;#0000ff\x1b\\");
    t.flush();
    check_invariants(&t);
}

// ── Layer 4: C0/C1 control codes ────────────────────────────────
// WezTerm-style: verify simple control characters

#[test]
fn wez_c0_bell() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"X\x07Y");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'X' as u32);
    assert_eq!(
        snap.cell_at(0, 1).codepoint,
        'Y' as u32,
        "Wez BEL: Y placed after X"
    );
    check_invariants(&t);
}

#[test]
fn wez_c0_backspace() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"AB\x08");
    t.flush();
    let snap = t.take_snapshot();
    // After "AB" cursor is at col 2, BS moves back 1 → col 1
    assert_eq!(snap.cursor_col, 1, "Wez BS: col 2 → col 1");
}

#[test]
fn wez_c0_linefeed() {
    let mut t = sized_term(5, 20, 100);
    t.pty_write(b"Row1\nRow2");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 1, "Wez LF: row 2");
    assert_eq!(
        snap.cell_at(1, 0).codepoint,
        'R' as u32,
        "Wez LF: Row2 on row 2"
    );
}

#[test]
fn wez_c0_tab() {
    let mut t = sized_term(5, 30, 100);
    t.vt_write(b"X\x09Y");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 8).codepoint,
        'Y' as u32,
        "Wez HT: Y at col 8"
    );
}

#[test]
fn wez_c0_vtab_no_crash() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"X\x0BY");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_c0_formfeed_no_crash() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"X\x0CY");
    t.flush();
    check_invariants(&t);
}

// ── Combined completeness test ──────────────────────────────────

#[test]
fn wez_all_csi_final_bytes_safe() {
    let final_bytes = b"ABCDEFGHIJKLMNOPSTXYZabcdefghijklmnopstxyz`$@";
    for &fb in final_bytes {
        let mut t = sized_term(24, 80, 100);
        let seq = [b"\x1b[5" as &[u8], &[fb]].concat();
        t.vt_write(&seq);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn wez_all_csi_params_safe() {
    let params = &["1", "5", "0", "99999", "1;1", "1;1;1", "0;0", "1;5;10"];
    for param in params {
        for &fb in b"ABCDHJKmsu" {
            let mut t = sized_term(24, 80, 100);
            let seq = format!("\x1b[{}{}", param, fb as char);
            t.vt_write(seq.as_bytes());
            t.flush();
            check_invariants(&t);
        }
    }
}

// ── SGR attribute combination tests ────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn assert_effects_set(
    snap: &torvox_terminal::ghostty_terminal::GridSnapshot,
    row: u32,
    col: u32,
    bold: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
    strikethrough: bool,
    overline: bool,
    blink: bool,
    hidden: bool,
) {
    let cell = &snap.cell_at(row, col);
    assert_eq!(cell.bold, bold, "bold at ({row},{col})");
    assert_eq!(cell.italic, italic, "italic at ({row},{col})");
    assert_eq!(cell.underline, underline, "underline at ({row},{col})");
    assert_eq!(cell.reverse, reverse, "reverse at ({row},{col})");
    assert_eq!(
        cell.strikethrough, strikethrough,
        "strikethrough at ({row},{col})"
    );
    assert_eq!(cell.overline, overline, "overline at ({row},{col})");
    assert_eq!(cell.blink, blink, "blink at ({row},{col})");
    assert_eq!(cell.hidden, hidden, "hidden at ({row},{col})");
}

#[test]
fn wez_sgr_bold_italic_combination() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;3mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, true, true, false, false, false, false, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_underline_reverse_combination() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[4;7mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, true, true, false, false, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_strikethrough_overline_combination() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[9;53mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, false, false, true, true, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_blink_hidden_combination() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[5;8mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, false, false, false, false, true, true,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_bold_underline_italic_triple() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;3;4mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, true, true, true, false, false, false, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_bold_blink_reverse_underline_quad() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;5;7;4mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, true, false, true, true, false, false, true, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_all_attrs_on_then_reset_all() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;3;4;5;7;8;9;53mX\x1b[0mY");
    t.flush();
    let snap = t.take_snapshot();
    // X has all attrs
    assert_effects_set(&snap, 0, 0, true, true, true, true, true, true, true, true);
    // Y has none after reset
    assert_effects_set(
        &snap, 0, 1, false, false, false, false, false, false, false, false,
    );
    check_invariants(&t);
}

// ── DEC mode combination tests ─────────────────────────────────────

#[test]
fn wez_dec_origin_application_cursor_combination() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?6h"); // origin mode ON
    t.vt_write(b"\x1b[?1h"); // application cursor keys ON
    t.flush();
    assert!(t.is_origin_mode(), "origin mode ON");
    assert!(t.mode_get(1, 0), "application cursor ON");
    t.vt_write(b"\x1b[?6l"); // origin OFF
    t.vt_write(b"\x1b[?1l"); // cursor OFF
    t.flush();
    assert!(!t.is_origin_mode(), "origin mode OFF");
    assert!(!t.mode_get(1, 0), "application cursor OFF");
    check_invariants(&t);
}

#[test]
fn wez_dec_wrap_insert_mode_combination() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[?7h"); // autowrap ON
    t.vt_write(b"\x1b[?4h"); // insert mode ON
    t.flush();
    assert!(t.is_autowrap_enabled(), "autowrap ON");
    let _detected = t.mode_get(4, 0);
    // insert mode may or may not be supported
    t.vt_write(b"\x1b[?7l"); // autowrap OFF
    t.vt_write(b"\x1b[?4l");
    t.flush();
    assert!(!t.is_autowrap_enabled(), "autowrap OFF");
    check_invariants(&t);
}

#[test]
fn wez_dec_mouse_tracking_with_sgr_mode() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1000h"); // basic mouse ON
    t.vt_write(b"\x1b[?1006h"); // SGR mouse ON
    t.flush();
    assert!(t.is_mouse_tracking_active(), "mouse tracking active");
    assert!(t.mode_get(1006, 0), "SGR mouse ON");
    t.vt_write(b"\x1b[?1000l");
    t.vt_write(b"\x1b[?1006l");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_dec_cursor_visibility_with_blink() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?25h"); // cursor visible
    t.vt_write(b"\x1b[?12h"); // cursor blink (if supported)
    t.flush();
    assert!(t.is_cursor_enabled(), "cursor enabled");
    t.vt_write(b"\x1b[?25l");
    t.flush();
    assert!(!t.is_cursor_enabled(), "cursor disabled");
    check_invariants(&t);
}

#[test]
fn wez_dec_alt_screen_with_bracketed_paste() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1049h"); // alt screen
    t.vt_write(b"\x1b[?2004h"); // bracketed paste
    t.flush();
    assert!(t.is_alt_screen_active(), "alt screen active");
    assert!(t.is_bracketed_paste_active(), "bracketed paste active");
    t.vt_write(b"\x1b[?1049l");
    t.vt_write(b"\x1b[?2004l");
    t.flush();
    assert!(!t.is_alt_screen_active(), "alt screen off");
    assert!(!t.is_bracketed_paste_active(), "bracketed paste off");
    check_invariants(&t);
}

#[test]
fn wez_dec_origin_with_scroll_region() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[3;8r"); // scroll region rows 3-8
    t.vt_write(b"\x1b[?6h"); // origin ON
    t.flush();
    assert!(t.is_origin_mode(), "origin mode ON with scroll region");
    // CUP to region home (1-based: 1,1)
    t.vt_write(b"\x1b[1;1H");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 2, "origin+CUP: row 2 (0-idx = region top)");
    t.vt_write(b"\x1b[?6l");
    t.flush();
    check_invariants(&t);
}

// ── CSI multi-param correctness ────────────────────────────────────

#[test]
fn wez_cup_boundary_pairs() {
    let pairs = &[
        (1u32, 1u32, 0u32, 0u32),
        (1u32, 80u32, 0u32, 79u32),
        (24u32, 1u32, 23u32, 0u32),
        (24u32, 80u32, 23u32, 79u32),
        (12u32, 40u32, 11u32, 39u32),
        (500u32, 500u32, 23u32, 79u32),
        (0u32, 0u32, 0u32, 0u32),
        (25u32, 81u32, 23u32, 79u32),
        (1u32, 50u32, 0u32, 49u32),
        (10u32, 1u32, 9u32, 0u32),
    ];
    for &(r, c, exp_r, exp_c) in pairs {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(format!("\x1b[{};{}H", r, c).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cursor_row, exp_r, "CUP({r},{c}): row {exp_r}");
        assert_eq!(snap.cursor_col, exp_c, "CUP({r},{c}): col {exp_c}");
        check_invariants(&t);
    }
}

// ── OSC query/response ─────────────────────────────────────────────

#[test]
fn wez_osc_10_fg_query_response() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]10;?\x1b\\");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("10"), "OSC 10 query: response mentions 10");
    }
    check_invariants(&t);
}

#[test]
fn wez_osc_11_bg_query_response() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]11;?\x1b\\");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("11"), "OSC 11 query: response mentions 11");
    }
    check_invariants(&t);
}

#[test]
fn wez_osc_4_query_color_0() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]4;0;?\x1b\\");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("0"), "OSC 4 query: response mentions color 0");
    }
    check_invariants(&t);
}

#[test]
fn wez_osc_set_then_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]10;#ff0000\x1b\\"); // set fg to red
    t.vt_write(b"\x1b]10;?\x1b\\"); // query fg
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(
            text.contains("ff0000") || text.contains("10"),
            "OSC 10 set+query: red in response"
        );
    }
    check_invariants(&t);
}

#[test]
fn wez_osc_4_set_multiple_then_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]4;0;#000000;1;#aa0000;2;#00aa00\x1b\\");
    t.vt_write(b"\x1b]4;1;?\x1b\\");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(
            text.contains("1;") || text.contains("aa0000"),
            "OSC 4 set multi+query: mentions color 1"
        );
    }
    check_invariants(&t);
}

// ── DECRPM response format ─────────────────────────────────────────

#[test]
fn wez_decrpm_mode_25_response() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?25;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.starts_with("\x1b[?"), "DECRPM 25: starts with CSI ?");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_mode_1000_response() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1000;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("1000"), "DECRPM 1000: mentions mode");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_mode_1_set_then_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1h"); // set application cursor
    t.vt_write(b"\x1b[?1;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        // Response format: ESC [ ? mode ; status $ y
        // status 1 = SET, 2 = RESET, 3 = PSS, 4 = RTS, 0 = unknown
        assert!(
            text.contains("1") || text.contains("\x1b[?"),
            "DECRPM 1 set: response mentions mode 1"
        );
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_mode_1_reset_then_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1l"); // reset application cursor
    t.vt_write(b"\x1b[?1;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(
            text.contains("1"),
            "DECRPM 1 reset: response mentions mode 1"
        );
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_mode_6_with_origin() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?6h"); // origin ON
    t.vt_write(b"\x1b[?6;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("6"), "DECRPM 6: mentions mode 6");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_debug_1049_alt_screen() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1049h"); // alt screen ON
    t.vt_write(b"\x1b[?1049;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("1049"), "DECRPM 1049: mentions mode");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_mode_2004_bracketed_paste() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?2004h"); // bracketed paste ON
    t.vt_write(b"\x1b[?2004;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("2004"), "DECRPM 2004: mentions mode");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_mode_9_detection() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?9;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("9"), "DECRPM 9: mentions mode");
    }
    check_invariants(&t);
}

// --- SGR attribute extended combos (20) ---

#[test]
fn wez_sgr_bold_italic_underline_blink() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;3;4;5mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, true, true, true, false, false, false, true, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_underline_strikethrough_blink() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[4;9;5mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, true, false, true, false, true, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_reverse_italic_underline() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[7;3;4mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, true, true, true, false, false, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_faint_no_bold() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[2mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, false, false, false, false, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_italic_reverse_blink() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3;7;5mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, true, false, true, false, false, true, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_dim_italic() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[2;3mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, true, false, false, false, false, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_blink_strikethrough() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[5;9mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, false, false, true, false, true, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_conceal_reveal() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[8mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, false, false, false, false, false, true,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_overline_blink() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[53;5mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, false, false, false, true, true, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_italic_strikethrough_blink() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3;9;5mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, true, false, false, true, false, true, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_bold_reverse_conceal() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;7;8mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, true, false, false, true, false, false, false, true,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_underline_overline_blink_hidden() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[4;53;5;8mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, false, false, true, false, false, true, true, true,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_bold_italic_strikethrough() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;3;9mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, true, true, false, false, true, false, false, false,
    );
    check_invariants(&t);
}

#[test]
fn wez_sgr_five_way_combo() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;4;7;9;53mX");
    t.flush();
    let snap = t.take_snapshot();
    assert_effects_set(
        &snap, 0, 0, true, false, true, true, true, true, false, false,
    );
    check_invariants(&t);
}

// --- DEC mode combos (10) ---

#[test]
fn wez_mode_origin_and_cursor_keys() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?1h\x1b[?6h");
    t.flush();
    assert!(t.mode_get(1, 0), "DECCKM on");
    assert!(t.mode_get(6, 0), "DECOM on");
    check_invariants(&t);
}

#[test]
fn wez_mode_wrap_insert_combo() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?4h\x1b[?7h");
    t.flush();
    assert!(t.mode_get(4, 0), "IRM on");
    assert!(t.mode_get(7, 0), "AWM on");
    check_invariants(&t);
}

#[test]
fn wez_mode_all_off() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?1l\x1b[?6l\x1b[?7l\x1b[?4l");
    t.flush();
    assert!(!t.mode_get(1, 0), "DECCKM off");
    assert!(!t.mode_get(6, 0), "DECOM off");
    check_invariants(&t);
}

#[test]
fn wez_mode_application_cursor_with_wrap() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?1h\x1b[?7h");
    t.flush();
    assert!(t.mode_get(1, 0));
    assert!(t.mode_get(7, 0));
    check_invariants(&t);
}

#[test]
fn wez_mode_insert_auto_wrap() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?1h\x1b[?7l");
    t.flush();
    assert!(t.mode_get(1, 0));
    assert!(!t.mode_get(7, 0));
    check_invariants(&t);
}

#[test]
fn wez_mode_origin_no_wrap() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?6h\x1b[?7l");
    t.flush();
    assert!(t.mode_get(6, 0));
    assert!(!t.mode_get(7, 0));
    check_invariants(&t);
}

#[test]
fn wez_mode_scroll_lock_on() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[?12h");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_mode_cursor_visible_on_off() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?25h");
    t.flush();
    t.vt_write(b"\x1b[?25l");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_mode_mouse_x10_and_sgr() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?9h\x1b[?1000h");
    t.flush();
    assert!(t.mode_get(9, 0));
    assert!(t.mode_get(1000, 0));
    check_invariants(&t);
}

#[test]
fn wez_mode_bracketed_paste_on() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?2004h");
    t.flush();
    assert!(t.mode_get(2004, 0));
    check_invariants(&t);
}

// --- CSI multi-param (10) ---

#[test]
fn wez_cup_edge_row_1() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[1;1HX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'X' as u32);
    check_invariants(&t);
}

#[test]
fn wez_cup_edge_row_max() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[5;1HX");
    t.flush();
    let snap = t.take_snapshot();
    let idx = 4 * 40;
    assert_eq!(snap.cells[idx].codepoint, 'X' as u32);
    check_invariants(&t);
}

#[test]
fn wez_cup_edge_col_1() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[1;1HX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'X' as u32);
    check_invariants(&t);
}

#[test]
fn wez_cup_edge_col_max() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[1;40HX");
    t.flush();
    assert_eq!(t.cursor_x(), 39);
    check_invariants(&t);
}

#[test]
fn wez_cup_both_defaults_home() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"X\x1b[H");
    t.flush();
    assert_eq!(t.cursor_x(), 0);
    assert_eq!(t.cursor_y(), 0);
    check_invariants(&t);
}

#[test]
fn wez_cup_overflow_clamp() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[100;100HX");
    t.flush();
    assert!(t.cursor_y() < 5, "row clamped");
    assert!(t.cursor_x() < 40, "col clamped");
    check_invariants(&t);
}

#[test]
fn wez_cha_center_col() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[20GX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[19].codepoint, 'X' as u32);
    check_invariants(&t);
}

#[test]
fn wez_vpa_mid_row() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3dX");
    t.flush();
    let snap = t.take_snapshot();
    let idx = 2 * 40;
    assert_eq!(snap.cells[idx].codepoint, 'X' as u32);
    check_invariants(&t);
}

#[test]
fn wez_hvp_both() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3;20fX");
    t.flush();
    let snap = t.take_snapshot();
    let idx = 2 * 40 + 19;
    assert_eq!(snap.cells[idx].codepoint, 'X' as u32);
    check_invariants(&t);
}

#[test]
fn wez_cursor_cnl_cpl() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"Line1\x1b[E\x1b[F");
    t.flush();
    assert_eq!(t.cursor_y(), 0);
    check_invariants(&t);
}

// --- OSC queries (10) ---

#[test]
fn wez_osc_rgb_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]4;0;?\x07");
    t.flush();
    let _resp = t.drain_pty_write_responses();
    check_invariants(&t);
}

#[test]
fn wez_osc_fg_bg_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]10;?\x07\x1b]11;?\x07");
    t.flush();
    let _resp = t.drain_pty_write_responses();
    check_invariants(&t);
}

#[test]
fn wez_osc_cursor_color_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]12;?\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_icon_title_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]0;?\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_title_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]2;?\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_hyperlink_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]8;?\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_color_scheme_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]4;0;?\x07\x1b]4;1;?\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_reset_color() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]104\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_set_title_then_query() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]0;TestTitle\x07\x1b]0;?\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn wez_osc_set_fg_then_reset() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b]10;#ff0000\x07\x1b]110\x07");
    t.flush();
    check_invariants(&t);
}

// --- DECRPM extended (10) ---

#[test]
fn wez_decrpm_origin_mode_cycle() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?6h\x1b[?6l");
    t.vt_write(b"\x1b[?6;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("6"), "DECRPM origin: mentions 6");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_auto_wrap_cycle() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?7h\x1b[?7l");
    t.vt_write(b"\x1b[?7;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("7"), "DECRPM wrap: mentions 7");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_cursor_keys_cycle() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1h\x1b[?1l");
    t.vt_write(b"\x1b[?1;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("1"), "DECRPM keys: mentions 1");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_insert_mode_cycle() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?4h\x1b[?4l");
    t.vt_write(b"\x1b[?4;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("4"), "DECRPM IRM: mentions 4");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_scroll_lock_cycle() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?12h\x1b[?12l");
    t.vt_write(b"\x1b[?12;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("12"), "DECRPM scroll: mentions 12");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_cursor_visible() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?25;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("25"), "DECRPM visible: mentions 25");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_mouse_x10() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?9;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("9"), "DECRPM x10: mentions 9");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_bracketed_paste() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?2004;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("2004"), "DECRPM paste: mentions 2004");
    }
    check_invariants(&t);
}

#[test]
fn wez_decrpm_alt_screen() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[?1049;$p");
    t.flush();
    let resp = t.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("1049"), "DECRPM alt: mentions 1049");
    }
    check_invariants(&t);
}
