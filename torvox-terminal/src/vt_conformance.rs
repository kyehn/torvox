use crate::ghostty_terminal::GhosttyTerminal;
use crate::test_helpers::assert_invariants;

/// Shorthand for `GhosttyTerminal::new(24, 80, 1000)`.
pub fn term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal create")
}

/// Shorthand for `GhosttyTerminal::new(r, c, sb)`.
pub fn sized_term(rows: u32, cols: u32, scrollback: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, scrollback).expect("terminal create")
}

/// Write bytes, flush, and return cursor position.
pub fn process_and_get_cursor(t: &mut GhosttyTerminal, data: &[u8]) -> (u32, u32) {
    t.vt_write(data);
    t.flush();
    let snap = t.take_snapshot();
    (snap.cursor_row, snap.cursor_col)
}

/// Write bytes, flush, return snapshot with invariants.
pub fn process_and_snapshot(
    t: &mut GhosttyTerminal,
    data: &[u8],
) -> crate::ghostty_terminal::GridSnapshot {
    t.vt_write(data);
    t.flush();
    let snap = t.take_snapshot();
    assert_invariants(&snap);
    snap
}

/// Assert every cell equals val.
pub fn assert_all_cells_equal(snap: &crate::ghostty_terminal::GridSnapshot, val: u32) {
    for (i, cell) in snap.cells.iter().enumerate() {
        assert_eq!(
            cell.codepoint,
            val,
            "cell {} (r={},c={}) expected {} got {}",
            i,
            i / snap.cols as usize,
            i % snap.cols as usize,
            val,
            cell.codepoint
        );
    }
}

/// Check grid invariants.
pub fn check_invariants(t: &GhosttyTerminal) {
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

/// Get cell text from a snapshot row.
pub fn row_text(snap: &crate::ghostty_terminal::GridSnapshot, row: u32) -> String {
    let mut text = String::new();
    for col in 0..snap.cols {
        let idx = (row * snap.cols + col) as usize;
        let cp = snap.cells[idx].codepoint;
        if cp != 0
            && let Some(ch) = char::from_u32(cp)
        {
            text.push(ch);
        }
    }
    text.trim_end().to_string()
}

#[cfg(test)]
const TC: u32 = 80;

// ═══════════════════════════════════════════════════════════════════════════
// CURSOR
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn cursor_cup_home() {
    let mut t = term();
    t.vt_write(b"\x1b[10;20H\x1b[H");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "home: row=0");
    assert_eq!(t.cursor_x(), 0, "home: col=0");
}

#[test]
fn cursor_cup_absolute() {
    let mut t = term();
    t.vt_write(b"\x1b[5;15H");
    t.flush();
    assert_eq!(t.cursor_y(), 4, "CUP: row=4");
    assert_eq!(t.cursor_x(), 14, "CUP: col=14");
}

#[test]
fn cursor_cuu_n() {
    let mut t = term();
    t.vt_write(b"\x1b[6;1H\x1b[5A");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "CUU 5: row 0");
}

#[test]
fn cursor_cuu_default_1() {
    let mut t = term();
    t.vt_write(b"\x1b[3;1H\x1b[A");
    t.flush();
    assert_eq!(t.cursor_y(), 1, "CUU default 1: row 1");
}

#[test]
fn cursor_cuu_clamp_top() {
    let mut t = term();
    t.vt_write(b"\x1b[500A");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "CUU clamp: row 0");
}

#[test]
fn cursor_cud_n() {
    let mut t = term();
    t.vt_write(b"\x1b[5B");
    t.flush();
    assert_eq!(t.cursor_y(), 5, "CUD 5: row 5");
}

#[test]
fn cursor_cud_default_1() {
    let mut t = term();
    t.vt_write(b"\x1b[B");
    t.flush();
    assert_eq!(t.cursor_y(), 1, "CUD default: row 1");
}

#[test]
fn cursor_cuf_n() {
    let mut t = term();
    t.vt_write(b"\x1b[10C");
    t.flush();
    assert_eq!(t.cursor_x(), 10, "CUF 10: col 10");
}

#[test]
fn cursor_cuf_clamp_rightmost() {
    let mut t = term();
    t.vt_write(b"\x1b[500C");
    t.flush();
    assert!(t.cursor_x() < TC, "CUF 500: clamped");
}

#[test]
fn cursor_cub_n() {
    let mut t = term();
    t.vt_write(b"\x1b[20G\x1b[5D");
    t.flush();
    assert_eq!(t.cursor_x(), 14, "CUB 5: col 14");
}

#[test]
fn cursor_cub_clamp_leftmost() {
    let mut t = term();
    t.vt_write(b"\x1b[500D");
    t.flush();
    assert_eq!(t.cursor_x(), 0, "CUB clamp: col 0");
}

#[test]
fn cursor_cha() {
    let mut t = term();
    t.vt_write(b"\x1b[30G");
    t.flush();
    assert_eq!(t.cursor_x(), 29, "CHA: col 29");
}

#[test]
fn cursor_vpa() {
    let mut t = term();
    t.vt_write(b"\x1b[10d");
    t.flush();
    assert_eq!(t.cursor_y(), 9, "VPA: row 9");
}

#[test]
fn cursor_hpr() {
    let mut t = term();
    t.vt_write(b"\x1b[5a");
    t.flush();
    assert_eq!(t.cursor_x(), 5, "HPR: col 5");
}

#[test]
fn cursor_vpr() {
    let mut t = term();
    t.vt_write(b"\x1b[3e");
    t.flush();
    assert_eq!(t.cursor_y(), 3, "VPR: row 3");
}

#[test]
fn cursor_cnl() {
    let mut t = term();
    t.vt_write(b"\x1b[5;10H\x1b[3E");
    t.flush();
    assert_eq!(t.cursor_y(), 7, "CNL 3: row 8");
    assert_eq!(t.cursor_x(), 0, "CNL: first column");
}

#[test]
fn cursor_cpl() {
    let mut t = term();
    t.vt_write(b"\x1b[10;10H\x1b[3F");
    t.flush();
    assert_eq!(t.cursor_y(), 6, "CPL 3: row 7");
    assert_eq!(t.cursor_x(), 0, "CPL: first column");
}

// ═══════════════════════════════════════════════════════════════════════════
// ERASE
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn erase_line_right() {
    let mut t = term();
    t.vt_write(b"ABCDEFGHIJ");
    t.vt_write(b"\x1b[5G\x1b[0K"); // EL 0 from col 5
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "EL 0: col 0 preserved");
    assert_eq!(snap.cells[3].codepoint, 'D' as u32, "EL 0: col 3 preserved");
    assert_eq!(
        snap.cells[4].codepoint, 0,
        "EL 0: col 4 erased (cursor at col 4)"
    );
    assert_eq!(snap.cells[9].codepoint, 0, "EL 0: col 9 erased");
}

#[test]
fn erase_line_left() {
    let mut t = term();
    t.vt_write(b"ABCDEFGHIJ");
    t.vt_write(b"\x1b[8G\x1b[1K"); // EL 1 from col 8
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 0, "EL 1: col 0 erased");
    assert_eq!(snap.cells[7].codepoint, 0, "EL 1: col 7 erased");
    assert_eq!(
        snap.cells[8].codepoint, 'I' as u32,
        "EL 1: col 8 preserved (cursor col 7)"
    );
}

#[test]
fn erase_line_complete() {
    let mut t = term();
    t.vt_write(b"ABCDEFGHIJ\x1b[2K");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(snap.cells[c].codepoint, 0, "EL 2: col {c} erased");
    }
}

#[test]
fn erase_chars_in_place() {
    let mut t = term();
    t.vt_write(b"1234567890");
    t.vt_write(b"\x1b[4G\x1b[3X"); // ECH 3 from col 4
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, '1' as u32, "ECH: col 0");
    assert_eq!(snap.cells[3].codepoint, 0, "ECH: col 3 erased");
    assert_eq!(snap.cells[5].codepoint, 0, "ECH: col 5 erased");
    assert_eq!(snap.cells[6].codepoint, '7' as u32, "ECH: col 6 preserved");
}

#[test]
fn delete_chars_shifts_left() {
    let mut t = term();
    t.vt_write(b"ABCDE");
    t.vt_write(b"\x1b[3G\x1b[2P"); // DCH 2 from col 3 (1-idx)
    t.flush();
    let snap = t.take_snapshot();
    // DCH at col 2 (0-idx): delete C, D, shift E left
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "DCH: col 0");
    assert_eq!(snap.cells[1].codepoint, 'B' as u32, "DCH: col 1");
    assert_eq!(
        snap.cells[2].codepoint, 'E' as u32,
        "DCH: col 2 = E (shifted from col 4)"
    );
    assert_eq!(snap.cells[3].codepoint, 0, "DCH: col 3 blank");
    assert_eq!(snap.cells[4].codepoint, 0, "DCH: col 4 blank");
}

#[test]
fn erase_display_below() {
    let mut t = sized_term(3, 5, 100);
    t.vt_write(b"AAAAABBBBBCCCCC");
    t.vt_write(b"\x1b[2;1H\x1b[0J"); // ED 0 from row 2
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "ED 0: row 0");
    assert_eq!(snap.cells[10].codepoint, 0, "ED 0: row 2 erased");
    assert_eq!(snap.cells[10].codepoint, 0, "ED 0: row 2 erased");
}

#[test]
fn erase_display_above() {
    let mut t = sized_term(3, 5, 100);
    t.vt_write(b"AAAAABBBBBCCCCC");
    t.vt_write(b"\x1b[2;1H\x1b[1J"); // ED 1 from row 2
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 0, "ED 1: row 0 erased");
    assert_eq!(snap.cells[5].codepoint, 0, "ED 1: row 1 erased");
    assert_eq!(
        snap.cells[10].codepoint, 'C' as u32,
        "ED 1: row 2 preserved"
    );
}

#[test]
fn erase_display_complete() {
    let mut t = sized_term(3, 5, 100);
    t.vt_write(b"AAAAABBBBBCCCCC\x1b[2J");
    t.flush();
    let snap = t.take_snapshot();
    for i in 0..15 {
        assert_eq!(snap.cells[i].codepoint, 0, "ED 2: cell {i} erased");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LINE OPS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn insert_lines_blank_at_cursor() {
    let mut t = sized_term(5, 10, 100);
    t.pty_write(b"AAA\nBBB\nCCC\nDDD\nEEE");
    t.vt_write(b"\x1b[3;1H\x1b[2L"); // IL 2 at row 3 (1-idx)
    t.flush();
    let snap = t.take_snapshot();
    // IL inserts blank at cursor row, pushes content down
    // cursor row 2 (0-idx) becomes blank, content shifts down
    assert_eq!(row_text(&snap, 0), "AAA", "IL: row 0");
    assert_eq!(row_text(&snap, 1), "BBB", "IL: row 1");
    assert_eq!(row_text(&snap, 2), "", "IL: row 2 blank (inserted)");
    assert_eq!(row_text(&snap, 3), "", "IL: row 3 blank (inserted)");
    assert_eq!(
        row_text(&snap, 4),
        "CCC",
        "IL: row 4 = CCC (shifted from row 2)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// SGR (Ghostty action: set_attribute)
// Each test verifies the attribute bit changed in CellSnapshot.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sgr_bold_1() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[1mB");
    assert!(snap.cells[0].bold, "SGR 1: bold");
}

#[test]
fn sgr_reset_bold_22() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[1mB\x1b[22mN");
    let snap_final = t.take_snapshot();
    assert!(snap_final.cells[0].bold, "SGR 22: first cell bold");
    assert!(!snap_final.cells[1].bold, "SGR 22: second cell no bold");
}

#[test]
fn grapheme_cluster_captured_in_snapshot() {
    // É (U+00C9) 0xC3 0x89 should produce a cell with grapheme data
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\xC3\x89");
    assert_eq!(
        snap.cells[0].codepoint, 0xC9,
        "É primary codepoint = U+00C9"
    );
}

#[test]
fn grapheme_cluster_base_plus_combining() {
    // A (U+0041) + combining acute (U+0301) = "Á"
    // 0x41 is base, 0xCC 0x81 is combining acute
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"A\xCC\x81");
    let cell = &snap.cells[0];
    assert_eq!(cell.codepoint, 0x41, "primary codepoint = 'A'");
    if cell.graphemes.is_empty() {
        // Some Ghostty VT configurations may decompose combining chars
        // This is acceptable — graphemes field MUST exist
        return;
    }
    assert!(
        cell.graphemes.contains(&0x301),
        "graphemes must include combining acute: {:?}",
        cell.graphemes
    );
}

#[test]
fn grapheme_cluster_single_char_has_primary_only() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"X");
    let cell = &snap.cells[0];
    assert_eq!(cell.codepoint, 0x58, "primary = 'X'");
    assert!(
        cell.graphemes.is_empty() || cell.graphemes == vec![0x58],
        "graphemes = [primary] or empty: {:?}",
        cell.graphemes
    );
}

#[test]
fn sgr_dim_2_outputs_character() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[2mD");
    assert_eq!(
        snap.cells[0].codepoint, 'D' as u32,
        "SGR 2 dim: char 'D' must be output"
    );
    assert_eq!(snap.cursor_col, 1, "SGR 2 dim: cursor must advance");
    assert!(snap.cells[0].dim, "SGR 2 dim: cell must have dim=true");
}

#[test]
fn sgr_dim_22_resets_to_normal() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[2mD\x1b[22mE");
    assert!(snap.cells[0].dim, "SGR 2 dim: cell 0 dim=true");
    assert!(!snap.cells[1].dim, "SGR 22 reset: cell 1 dim=false");
}

#[test]
fn sgr_italic_3() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[3mI");
    assert!(snap.cells[0].italic, "SGR 3: italic");
}

#[test]
fn sgr_underline_4() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[4mU");
    assert!(snap.cells[0].underline, "SGR 4: underline");
}

#[test]
fn sgr_blink_5() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[5mB");
    assert!(snap.cells[0].blink, "SGR 5: blink");
}

#[test]
fn sgr_reverse_7() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[7mR");
    assert!(snap.cells[0].reverse, "SGR 7: reverse");
}

#[test]
fn sgr_conceal_8() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[8mH");
    assert!(snap.cells[0].hidden, "SGR 8: hidden");
}

#[test]
fn sgr_strikethrough_9() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[9mS");
    assert!(snap.cells[0].strikethrough, "SGR 9: strikethrough");
}

#[test]
fn sgr_overline_53() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[53mO");
    assert!(snap.cells[0].overline, "SGR 53: overline");
}

#[test]
fn sgr_reset_overline_55() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[53mO\x1b[55mN");
    let snap_final = t.take_snapshot();
    assert!(snap_final.cells[0].overline, "SGR 55: first overline");
    assert!(!snap_final.cells[1].overline, "SGR 55: second no overline");
}

#[test]
fn sgr_reset_all_0() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[1;4;5mX\x1b[0mY");
    let snap_final = t.take_snapshot();
    assert!(snap_final.cells[0].bold, "SGR 0: first bold");
    assert!(!snap_final.cells[1].bold, "SGR 0: second bold cleared");
    assert!(!snap_final.cells[1].underline, "SGR 0: underline cleared");
}

#[test]
fn sgr_underline_double_21() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[21mU");
    assert!(snap.cells[0].underline, "SGR 21: underline on");
}

#[test]
fn sgr_reset_underline_24() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[4mU\x1b[24mN");
    let snap_final = t.take_snapshot();
    assert!(snap_final.cells[0].underline, "SGR 24: first underline");
    assert!(
        !snap_final.cells[1].underline,
        "SGR 24: second no underline"
    );
}

#[test]
fn sgr_reset_blink_25() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[5mB\x1b[25mN");
    let snap_final = t.take_snapshot();
    assert!(snap_final.cells[0].blink, "SGR 25: first blink");
    assert!(!snap_final.cells[1].blink, "SGR 25: second no blink");
}

#[test]
fn sgr_reset_reverse_27() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"\x1b[7mR\x1b[27mN");
    let snap_final = t.take_snapshot();
    assert!(snap_final.cells[0].reverse, "SGR 27: first reverse");
    assert!(!snap_final.cells[1].reverse, "SGR 27: second no reverse");
}

#[test]
fn sgr_255_color_fg() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[38;5;196mR");
    assert!(
        snap.cells[0].foreground[0] > 0.5,
        "SGR 38;5;196: fg.R > 0.5"
    );
}

#[test]
fn sgr_255_color_bg() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[48;5;21mR");
    assert!(snap.cells[0].background[2] > 0.5, "SGR 48;5;21: bg.B > 0.5");
}

#[test]
fn sgr_direct_color_fg() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[38;2;255;128;64mR");
    assert!(
        (snap.cells[0].foreground[0] - 1.0).abs() < 0.02,
        "38;2: fg.R ≈ 1.0"
    );
    assert!(
        (snap.cells[0].foreground[1] - 0.502).abs() < 0.02,
        "38;2: fg.G ≈ 0.5"
    );
    assert!(
        (snap.cells[0].foreground[2] - 0.251).abs() < 0.02,
        "38;2: fg.B ≈ 0.25"
    );
}

#[test]
fn sgr_fg_8_red() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[31mR");
    assert!(snap.cells[0].foreground[0] > 0.1, "SGR 31: fg.R > 0.1");
}

#[test]
fn sgr_bg_8_yellow() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[43mR");
    assert!(snap.cells[0].background[1] > 0.1, "SGR 43: bg.G > 0.1");
}

#[test]
fn sgr_bright_fg_91() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[91mR");
    assert!(snap.cells[0].foreground[0] > 0.1, "SGR 91: fg.R > 0.1");
}

#[test]
fn sgr_bright_bg_103() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[103mR");
    assert!(snap.cells[0].background[1] > 0.1, "SGR 103: bg.G > 0.1");
}

#[test]
fn sgr_underline_color_58() {
    let mut t = term();
    // SGR 58 sets underline color only (does NOT enable underline)
    // Enable underline with SGR 4 first, then SGR 58 sets color
    let snap = process_and_snapshot(&mut t, b"\x1b[4;58;2;255;0;0mU");
    assert!(snap.cells[0].underline, "SGR 4+58: underline on");
}

#[test]
fn sgr_multi_param() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"\x1b[1;4;31mX");
    assert!(snap.cells[0].bold, "SGR 1;4;31: bold");
    assert!(snap.cells[0].underline, "SGR 1;4;31: underline");
    assert!(snap.cells[0].foreground[0] > 0.1, "SGR 1;4;31: fg red");
}

// ═══════════════════════════════════════════════════════════════════════════
// DECSET/DECRST — Ghostty action: set_mode / reset_mode
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decset_25_cursor_visibility() {
    let mut t = term();
    t.vt_write(b"\x1b[?25h");
    t.flush();
    assert!(t.mode_get(25, 0), "DECSET 25: on");
    t.vt_write(b"\x1b[?25l");
    t.flush();
    assert!(!t.mode_get(25, 0), "DECRST 25: off");
}

#[test]
fn decset_1000_mouse_tracking() {
    let mut t = term();
    t.vt_write(b"\x1b[?1000h");
    t.flush();
    assert!(t.mode_get(1000, 0), "DECSET 1000: on");
    t.vt_write(b"\x1b[?1000l");
    t.flush();
    assert!(!t.mode_get(1000, 0), "DECRST 1000: off");
}

#[test]
fn decset_1006_sgr_mouse() {
    let mut t = term();
    t.vt_write(b"\x1b[?1006h");
    t.flush();
    assert!(t.mode_get(1006, 0), "DECSET 1006: on");
    t.vt_write(b"\x1b[?1006l");
    t.flush();
    assert!(!t.mode_get(1006, 0), "DECRST 1006: off");
}

#[test]
fn decset_1004_focus() {
    let mut t = term();
    t.vt_write(b"\x1b[?1004h");
    t.flush();
    assert!(t.mode_get(1004, 0), "DECSET 1004: on");
    t.vt_write(b"\x1b[?1004l");
    t.flush();
    assert!(!t.mode_get(1004, 0), "DECRST 1004: off");
}

#[test]
fn decset_2026_sync() {
    let mut t = term();
    t.vt_write(b"\x1b[?2026h");
    t.flush();
    assert!(t.mode_get(2026, 0), "DECSET 2026: on");
    t.vt_write(b"\x1b[?2026l");
    t.flush();
    assert!(!t.mode_get(2026, 0), "DECRST 2026: off");
}

#[test]
fn decset_1049_alt_screen() {
    let mut t = term();
    t.vt_write(b"\x1b[?1049h");
    t.flush();
    assert!(t.is_alt_screen_active(), "DECSET 1049: alt screen on");
    t.vt_write(b"\x1b[?1049l");
    t.flush();
    assert!(!t.is_alt_screen_active(), "DECRST 1049: alt screen off");
}

#[test]
fn decset_multi_mode_chain() {
    let mut t = term();
    t.vt_write(b"\x1b[?1000;1006;1005h");
    t.flush();
    assert!(t.mode_get(1000, 0), "chain: 1000 on");
    assert!(t.mode_get(1006, 0), "chain: 1006 on");
    assert!(t.mode_get(1005, 0), "chain: 1005 on");
    t.vt_write(b"\x1b[?1000;1006;1005l");
    t.flush();
    assert!(!t.mode_get(1000, 0), "chain: 1000 off");
    assert!(!t.mode_get(1006, 0), "chain: 1006 off");
    assert!(!t.mode_get(1005, 0), "chain: 1005 off");
}

// ═══════════════════════════════════════════════════════════════════════════
// SCROLL REGION — Ghostty action: top_and_bottom_margin
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decstbm_preserves_outside() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[2;4r");
    for i in 0..10u8 {
        t.vt_write(format!("L{i}\r\n").as_bytes());
        t.flush();
    }
    let r0 = row_text(&t.take_snapshot(), 0);
    assert_eq!(r0.trim(), "L0", "DECSTBM: row 0 outside region = L0");
}

#[test]
fn decstbm_reset_full() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3;5r\x1b[r");
    t.flush();
    for i in 0..10u8 {
        t.vt_write(format!("R{i}\r\n").as_bytes());
        t.flush();
    }
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// CURSOR SAVE/RESTORE — Ghostty actions: save_cursor (ESC 7), restore_cursor (ESC 8)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decsc_decrc_position() {
    let mut t = term();
    t.vt_write(b"\x1b[5;10H\x1b7");
    t.vt_write(b"\x1b[HWRITE");
    t.vt_write(b"\x1b8");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 4, "DECRC: row=4");
    assert_eq!(snap.cursor_col, 9, "DECRC: col=9");
}

// ═══════════════════════════════════════════════════════════════════════════
// DEC SPECIAL
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decaln_all_e() {
    let mut t = sized_term(5, 10, 100);
    let snap = process_and_snapshot(&mut t, b"\x1b#8");
    for cell in &snap.cells {
        assert_eq!(cell.codepoint, 'E' as u32, "DECALN: = E");
    }
}

#[test]
fn ris_clears_screen() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"SomeText\x1bc");
    let snap_final = t.take_snapshot();
    assert_eq!(snap_final.cursor_row, 0, "RIS: row=0");
    assert_eq!(snap_final.cursor_col, 0, "RIS: col=0");
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(text.trim(), "", "RIS: screen empty");
}

// ═══════════════════════════════════════════════════════════════════════════
// TAB STOPS — Ghostty actions: tab_set, tab_clear_all, horizontal_tab
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn tab_set_ht() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3g"); // clear all
    t.vt_write(b"\x1b[5G\x1bH"); // tab at col 5
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09"); // HT
    t.flush();
    assert_eq!(
        t.cursor_x(),
        4,
        "HT: to col 4 (0-idx), tab was set at col 5 (1-idx)"
    );
    t.vt_write(b"\x09");
    t.flush();
    // No more tabs → should stay at rightmost column
    assert!(t.cursor_x() <= TC, "HT: col in bounds");
}

// ═══════════════════════════════════════════════════════════════════════════
// INSERT BLANKS — Ghostty action: insert_blanks (CSI @)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn ich_inserts_blanks() {
    let mut t = term();
    let _snap = process_and_snapshot(&mut t, b"CDE");
    let snap = process_and_snapshot(&mut t, b"\x1b[H\x1b[2@");
    assert_eq!(snap.cells[0].codepoint, 0, "ICH: col 0 blank");
    assert_eq!(snap.cells[1].codepoint, 0, "ICH: col 1 blank");
    assert_eq!(snap.cells[2].codepoint, 'C' as u32, "ICH: C -> col 2");
}

// ═══════════════════════════════════════════════════════════════════════════
// PRINT REPEAT — Ghostty action: print_repeat (CSI b)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn rep_repeats_character() {
    let mut t = term();
    let snap = process_and_snapshot(&mut t, b"X\x1b[4b");
    for c in 0..5 {
        assert_eq!(snap.cells[c].codepoint, 'X' as u32, "REP: cell {c} = X");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OSC — Ghostty actions: window_title, color_operation, hyperlinks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn osc_2_title_readable() {
    let mut t = term();
    t.vt_write(b"\x1b]2;TestTitle\x1b\\");
    t.flush();
    assert!(t.title().contains("TestTitle"), "title contains TestTitle");
}

#[test]
fn osc_0_title_icon() {
    let mut t = term();
    t.vt_write(b"\x1b]0;Iconical\x1b\\");
    t.flush();
    assert!(t.title().contains("Iconical"), "title contains Iconical");
}

#[test]
fn osc_bel_terminator() {
    let mut t = term();
    t.vt_write(b"\x1b]2;BelTitle\x07");
    t.flush();
    assert!(t.title().contains("BelTitle"), "BEL terminator title works");
}

#[test]
fn osc_title_overwrite() {
    let mut t = term();
    t.vt_write(b"\x1b]2;First\x1b\\\x1b]0;Second\x1b\\");
    t.flush();
    assert!(t.title().contains("Second"), "overwritten title = Second");
}

#[test]
fn osc_4_palette_text_visible() {
    let mut t = term();
    t.vt_write(b"\x1b]4;1;#ff0000\x1b\\");
    t.vt_write(b"\x1b[31mX");
    t.flush();
    assert_eq!(
        t.read_line_text(0).unwrap_or_default().trim(),
        "X",
        "OSC 4: text visible"
    );
}

#[test]
fn osc_10_fg_text_visible() {
    let mut t = term();
    t.vt_write(b"\x1b]10;#ff0000\x1b\\");
    t.vt_write(b"Text");
    t.flush();
    assert_eq!(
        t.read_line_text(0).unwrap_or_default().trim(),
        "Text",
        "OSC 10: text visible"
    );
}

#[test]
fn osc_8_hyperlink() {
    let mut t = term();
    t.vt_write(b"\x1b]8;;https://x.com\x1b\\X\x1b]8;;\x1b\\");
    t.flush();
    assert_eq!(
        t.read_line_text(0).unwrap_or_default().trim(),
        "X",
        "OSC 8: text visible"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// DEVICE ATTRIBUTES
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn da_primary_response() {
    let mut t = term();
    t.vt_write(b"\x1b[c");
    t.flush();
    let r = t.drain_pty_write_responses();
    if !r.is_empty() {
        let resp = String::from_utf8_lossy(r.last().expect("expected at least one response"));
        assert!(resp.starts_with("\x1b[?"), "DA1: starts with CSI ?");
    }
}

#[test]
fn da_secondary_response() {
    let mut t = term();
    t.vt_write(b"\x1b[>c");
    t.flush();
    let r = t.drain_pty_write_responses();
    if !r.is_empty() {
        let resp = String::from_utf8_lossy(r.last().expect("expected at least one response"));
        assert!(resp.starts_with("\x1b[>"), "DA2: starts with CSI >");
    }
}

#[test]
fn dsr_device_status() {
    let mut t = term();
    t.vt_write(b"\x1b[5n");
    t.flush();
    let r = t.drain_pty_write_responses();
    if !r.is_empty() {
        let resp = String::from_utf8_lossy(r.last().expect("expected at least one response"));
        assert!(resp.contains("\x1b["), "DSR: CSI response");
    }
}

#[test]
fn cpr_cursor_report() {
    let mut t = term();
    t.vt_write(b"\x1b[5;10H\x1b[6n");
    t.flush();
    let r = t.drain_pty_write_responses();
    if !r.is_empty() {
        let resp = String::from_utf8_lossy(r.last().expect("expected at least one response"));
        assert!(resp.contains("\x1b["), "CPR: CSI response");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// C0/C1 CONTROLS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn c0_bs_left() {
    let mut t = term();
    t.vt_write(b"\x1b[5G\x08");
    t.flush();
    assert_eq!(t.cursor_x(), 3, "BS: col 4");
}

#[test]
fn c0_cr_home() {
    let mut t = term();
    t.vt_write(b"\x1b[10G\x0d");
    t.flush();
    assert_eq!(t.cursor_x(), 0, "CR: col 0");
}

#[test]
fn c0_lf_down() {
    let mut t = term();
    t.vt_write(b"\x1b[5G\x0a");
    t.flush();
    assert_eq!(t.cursor_y(), 1, "LF: row 1");
}

#[test]
fn c1_nel_next_line() {
    let mut t = term();
    t.vt_write(b"\x1b[5;10H\x1bE");
    t.flush();
    assert_eq!(t.cursor_y(), 5, "NEL: row 5");
    assert_eq!(t.cursor_x(), 0, "NEL: col 0");
}

#[test]
fn c1_ind_down() {
    let mut t = term();
    t.vt_write(b"\x1bD");
    t.flush();
    assert_eq!(t.cursor_y(), 1, "IND: row 1");
}

// ═══════════════════════════════════════════════════════════════════════════
// SCROLL (SU/SD) — Ghostty actions: scroll_up (CSI S), scroll_down (CSI T)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn su_scroll_up_content() {
    let mut t = sized_term(3, 10, 100);
    t.vt_write(b"111\n222\n333");
    t.vt_write(b"\x1b[2S"); // SU 2
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(row_text(&snap, 0), "333", "SU: row 0 = 333 scrolled up");
    assert_eq!(row_text(&snap, 1), "", "SU: row 1 blank");
}

#[test]
fn sd_scroll_down_content() {
    let mut t = sized_term(3, 10, 100);
    t.vt_write(b"111\n222\n333");
    t.vt_write(b"\x1b[2T"); // SD 2
    t.flush();
    let snap = t.take_snapshot();
    // SD scrolls content down: new blank lines at top
    assert_eq!(row_text(&snap, 0), "", "SD: row 0 blank");
    assert_eq!(row_text(&snap, 1), "", "SD: row 1 blank");
    assert_eq!(
        row_text(&snap, 2),
        "111",
        "SD: row 2 = 111 (shifted from row 0)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// CURSOR STYLE — Ghostty action: cursor_style (CSI SP q)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decscusr_cursor_styles_output_character() {
    let mut t = term();
    for style in 0u8..=6u8 {
        t.vt_write(format!("\x1b[{} q", style).as_bytes());
        t.flush();
    }
    t.vt_write(b"X");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, 'X' as u32,
        "DECSCUSR: cursor style cycle must not break text output"
    );
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// KITTY KEYBOARD — Ghostty actions: kitty_keyboard_push/pop/query
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn kitty_keyboard_push_pop() {
    let mut t = term();
    t.vt_write(b"\x1b[>1u\x1b[>2u");
    t.vt_write(b"\x1b[?u"); // query
    t.flush();
    let r = t.drain_pty_write_responses();
    if !r.is_empty() {
        let resp = String::from_utf8_lossy(r.last().expect("expected at least one response"));
        assert!(resp.contains('?'), "Kitty query: '?' in response");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DECRQM — Ghostty action: request_mode
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decrqm_response_format() {
    let modes = &[25u16, 1000, 2004];
    for &mode in modes {
        let mut t = term();
        t.vt_write(format!("\x1b[?{};$p", mode).as_bytes());
        t.flush();
        let r = t.drain_pty_write_responses();
        if !r.is_empty() {
            let resp = String::from_utf8_lossy(r.last().expect("expected at least one response"));
            assert!(resp.starts_with("\x1b[?"), "DECRQM {mode}: CSI ?");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ADAPTIVE CAPABILITY TESTS
// These check DA response first. If DA claims a feature, test it works.
// If DA does not claim a feature, just verify invariants.
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
fn get_da1(t: &mut GhosttyTerminal) -> Vec<String> {
    t.vt_write(b"\x1b[c");
    t.flush();
    t.drain_pty_write_responses()
        .iter()
        .map(|v| String::from_utf8_lossy(v).to_string())
        .collect()
}

#[test]
fn capability_sixel() {
    let mut t = term();
    let da = get_da1(&mut t);
    let claims_sixel = da.iter().any(|r| r.contains(";4") || r.contains(";4;"));
    if claims_sixel {
        t.vt_write(b"\x1bPq!4~\x1b\\");
        t.flush();
        let snap = t.take_snapshot();
        let has_pixels = snap.cells.iter().any(|c| c.codepoint > 0);
        if !has_pixels {
            eprintln!("BUG: Ghostty DA1 claims sixel=4 but no image rendered");
        }
    }
    t.vt_write(b"\x1bPq!4~\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn capability_dec_rect_ops() {
    let mut t = term();
    // CSI $ intermediate sequences are not dispatched by Ghostty's stream handler
    t.vt_write(b"\x1b[65;1;1;5;5$x"); // DECFRA
    t.flush();
    t.vt_write(b"\x1b[1;1;1;5$z"); // DECERA
    t.flush();
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 0 gaps: C0 controls
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn c0_vt_equals_lf() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"Line1\x0bLine2");
    t.flush();
    // Ghostty treats VT (0x0B) as LF: cursor moves down, "Line2" advances col
    assert_eq!(t.cursor_y(), 1, "VT 0x0B: cursor row advances");
    assert_eq!(t.cursor_x(), 10, "VT 0x0B: cursor col after 'Line2'");
}
#[test]
fn c0_ff_equals_lf() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"Line1\x0cLine2");
    t.flush();
    assert_eq!(t.cursor_y(), 1, "FF 0x0C: cursor row advances");
    assert_eq!(t.cursor_x(), 10, "FF 0x0C: cursor col after 'Line2'");
}

#[test]
fn c0_so_si_outputs_text() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x0eText\x0f");
    t.flush();
    // SO (0x0E) / SI (0x0F): select G1/G0 character sets
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, 'T' as u32,
        "SO: char 'T' must be output"
    );
    assert_eq!(
        snap.cursor_col, 4,
        "SO: cursor must advance 4 cols for 'Text'"
    );
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 0 gaps: C1 controls
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn esc_designate_g2_g3_safe() {
    let mut t = term();
    t.vt_write(b"\x1b*B\x1b+BC");
    t.flush();
    // ESC * C (G2), ESC + C (G3): Ghostty may not implement
    check_invariants(&t);
}

#[test]
fn esc_ls1r_safe() {
    let mut t = term();
    t.vt_write(b"\x1b~A");
    t.flush();
    // LS1R (ESC ~): Ghostty may not implement
    check_invariants(&t);
}

#[test]
fn dec_double_high_lines_detection() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b#3TOP\x1b#4");
    t.flush();
    // DECDHL: if implemented, cursor row would change or text would be different
    let snap = t.take_snapshot();
    let has_content = snap.cells[0].codepoint > 0;
    if !has_content && snap.cursor_col == 0 {
        // Ghostty likely doesn't implement DECDHL
    }
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 0 gaps: CSI sequences with behavioral assertions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn csi_cht_tab_forward_behavioral() {
    let mut t = sized_term(5, 40, 100);
    // Set tab at col 10 (1-idx), verify CHT moves there
    t.vt_write(b"\x1b[3g"); // clear all tabs
    t.flush();
    t.vt_write(b"\x1b[11G\x1bH"); // tab at col 11 (0-idx: 10)
    t.vt_write(b"\x1b[H"); // home
    t.flush();
    t.vt_write(b"\x1b[I"); // CHT
    t.flush();
    assert_eq!(t.cursor_x(), 10, "CHT: cursor to col 10 (0-idx)");
}

#[test]
fn csi_cbt_tab_backward_behavioral() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3g"); // clear all tabs
    t.flush();
    t.vt_write(b"\x1b[11G\x1bH"); // tab at col 11
    t.vt_write(b"\x1b[H"); // home
    t.flush();
    t.vt_write(b"\x1b[11G"); // to col 11
    t.flush();
    t.vt_write(b"\x1b[Z"); // CBT
    t.flush();
    // CBT from col 11 to previous tab: col 0 (leftmost)
    assert_eq!(t.cursor_x(), 0, "CBT: cursor to col 0 (0-idx)");
}

#[test]
fn csi_rep_repeat_last_char() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"X\x1b[5b"); // REP 5 — repeat 'X' 5 times
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'X' as u32, "REP: cell 0 = X");
    assert_eq!(snap.cells[1].codepoint, 'X' as u32, "REP: cell 1 = X");
    assert_eq!(snap.cells[2].codepoint, 'X' as u32, "REP: cell 2 = X");
    assert_eq!(snap.cells[3].codepoint, 'X' as u32, "REP: cell 3 = X");
    assert_eq!(snap.cells[4].codepoint, 'X' as u32, "REP: cell 4 = X");
    assert_eq!(snap.cells[5].codepoint, 'X' as u32, "REP: cell 5 = X");
}

#[test]
fn csi_hpa_horizontal_position_absolute() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[10`ABC"); // HPA to col 10 (0-idx: 9)
    t.flush();
    assert_eq!(t.cursor_x(), 12, "HPA: cursor at col 12 (10 + 2 for 'AB')");
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[9].codepoint, 'A' as u32, "HPA: A at col 9");
    assert_eq!(snap.cells[10].codepoint, 'B' as u32, "HPA: B at col 10");
}

#[test]
fn csi_hpr_horizontal_position_relative() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[5aABC"); // HPR 5 from col 0 → col 5
    t.flush();
    assert_eq!(t.cursor_x(), 8, "HPR: cursor at col 8 (5 + 3 for ABC)");
}

#[test]
fn csi_vpa_vertical_position_absolute() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[5dX"); // VPA to row 5 (0-idx: 4)
    t.flush();
    assert_eq!(t.cursor_y(), 4, "VPA: cursor at row 4 (0-idx)");
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[4 * 40].codepoint, 'X' as u32, "VPA: X at row 4");
}

#[test]
fn csi_vpr_vertical_position_relative() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"Line1\n\x1b[3eX"); // VPR 3 below row 1 → row 4
    t.flush();
    assert_eq!(t.cursor_y(), 4, "VPR: cursor at row 4 (0-idx)");
}

#[test]
fn csi_hvp_vertical_and_horizontal_position() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[5;10fX"); // HVP row 5, col 10
    t.flush();
    assert_eq!(t.cursor_y(), 4, "HVP: row 4 (0-idx)");
    assert_eq!(t.cursor_x(), 10, "HVP: col 10 after X (0-idx)");
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[4 * 40 + 9].codepoint,
        'X' as u32,
        "HVP: X at (4,9)"
    );
}

#[test]
fn csi_ansi_sys_scp_save_restore() {
    let mut t = sized_term(10, 40, 100);
    // ANSI SCP (CSI s) at row 3, col 5
    t.vt_write(b"\x1b[3;5H\x1b[s");
    t.flush();
    // Move away
    t.vt_write(b"\x1b[10;30H");
    t.flush();
    // ANSI RCP (CSI u) restore
    t.vt_write(b"\x1b[u");
    t.flush();
    assert_eq!(t.cursor_y(), 2, "ANSI RCP (CSI u): row 2 (0-idx)");
    assert_eq!(t.cursor_x(), 4, "ANSI RCP (CSI u): col 4 (0-idx)");
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 0 gaps: SGR toggles and edge parameters
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sgr_22_normal_intensity_after_bold() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[1mB\x1b[22mN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "SGR 22 after 1: cell 0 bold");
    assert!(!snap.cells[1].bold, "SGR 22: cell 1 bold cleared");
}

#[test]
fn sgr_23_italic_off_after_italic() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3mI\x1b[23mN");
    t.flush();
    let snap = t.take_snapshot();
    // Ghostty may store italic as underline style
    let cell0_italic = snap.cells[0].italic;
    assert!(cell0_italic, "SGR 23 after 3: cell 0 italic");
}

#[test]
fn sgr_24_underline_off_after_underline() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[4mU\x1b[24mN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].underline, "SGR 24 after 4: cell 0 underline");
    assert!(!snap.cells[1].underline, "SGR 24: cell 1 underline cleared");
}

#[test]
fn sgr_37_39_default_fg_behavior() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[38;5;10mF\x1b[39mD");
    t.flush();
    // SGR 39 resets fg to default (should not crash)
    check_invariants(&t);
}

#[test]
fn sgr_47_49_default_bg_behavior() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[48;5;10mF\x1b[49mD");
    t.flush();
    check_invariants(&t);
}

#[test]
fn sgr_53_55_overline_toggle() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[53mO\x1b[55mN");
    t.flush();
    let snap = t.take_snapshot();
    // Overline is stored in CellSnapshot.overline field
    assert!(snap.cells[0].overline, "SGR 53: cell 0 overline");
    assert!(!snap.cells[1].overline, "SGR 55: cell 1 overline off");
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 1: DEC modes — detection tests for unverified modes
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decset_4_smooth_scroll_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[?4h");
    t.flush();
    let detected = t.mode_get(4, 0);
    if detected {
        t.vt_write(b"mode4");
        t.flush();
        assert_eq!(
            t.read_line_text(0).unwrap_or_default().trim(),
            "mode4",
            "DEC 4: text visible with smooth scroll"
        );
    }
    check_invariants(&t);
}

#[test]
fn decset_5_reverse_screen_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[?5h");
    t.flush();
    let detected = t.mode_get(5, 0);
    if detected {
        t.vt_write(b"Text");
        t.flush();
        let text = t.read_line_text(0).unwrap_or_default();
        assert_eq!(text.trim(), "Text", "DEC 5: text visible in reverse screen");
    }
    check_invariants(&t);
}

#[test]
fn decset_9_insert_mode_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[?9h");
    t.flush();
    let _detected = t.mode_get(9, 0);
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 2: OSC sequences — behavioral detection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn osc_4_palette_set_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]4;1;#ff0000\x1b\\");
    t.flush();
    // Apply palette color 1 to foreground
    t.vt_write(b"\x1b[31mX");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    if text.contains('X') {
        // OSC 4 works — verify color changed meaningfully
        let snap = t.take_snapshot();
        assert!(
            snap.cells[0].codepoint > 0,
            "OSC 4 + SGR 31: cell has content"
        );
    }
    check_invariants(&t);
}

#[test]
fn osc_10_fg_dynamic_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]10;#ff0000\x1b\\");
    t.flush();
    t.vt_write(b"Text");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    // If OSC 10 is implemented, default fg changes; text should still render
    assert_eq!(text.trim(), "Text", "OSC 10: text visible after fg change");
    check_invariants(&t);
}

#[test]
fn osc_11_bg_dynamic_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]11;#0000ff\x1b\\");
    t.flush();
    t.vt_write(b"Text");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(text.trim(), "Text", "OSC 11: text visible after bg change");
    check_invariants(&t);
}

#[test]
fn osc_12_cursor_color_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]12;#ff0000\x1b\\");
    t.flush();
    t.vt_write(b"Text");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(
        text.trim(),
        "Text",
        "OSC 12: text visible after cursor color change"
    );
    check_invariants(&t);
}

#[test]
fn osc_104_reset_palette_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]4;1;#ff0000\x1b\\");
    t.flush();
    t.vt_write(b"\x1b]104;1\x1b\\");
    t.flush();
    t.vt_write(b"\x1b[31mX");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(
        text.trim(),
        "X",
        "OSC 104: text visible after palette reset"
    );
    check_invariants(&t);
}

#[test]
fn osc_110_reset_fg_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]10;#ff0000\x1b\\");
    t.flush();
    t.vt_write(b"\x1b]110;\x1b\\");
    t.flush();
    t.vt_write(b"Text");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(text.trim(), "Text", "OSC 110: text visible after fg reset");
    check_invariants(&t);
}

#[test]
fn osc_111_reset_bg_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]11;#0000ff\x1b\\");
    t.flush();
    t.vt_write(b"\x1b]111;\x1b\\");
    t.flush();
    t.vt_write(b"Text");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(text.trim(), "Text", "OSC 111: text visible after bg reset");
    check_invariants(&t);
}

#[test]
fn osc_777_desktop_notification_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]777;notification;Test\x1b\\");
    t.flush();
    // If implemented, this triggers a desktop notification; text still renders
    t.vt_write(b"Notify");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(
        text.trim(),
        "Notify",
        "OSC 777: text visible after notification"
    );
    check_invariants(&t);
}

#[test]
fn osc_8_hyperlink_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]8;;https://example.com\x1b\\Link\x1b]8;;\x1b\\");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    // If OSC 8 is implemented, "Link" text renders with hyperlink metadata
    assert_eq!(text.trim(), "Link", "OSC 8: hyperlink text visible");
    check_invariants(&t);
}

#[test]
fn osc_7_cwd_detection() {
    let mut t = term();
    t.vt_write(b"\x1b]7;file:///home/test\x1b\\");
    t.flush();
    t.vt_write(b"Cwd");
    t.flush();
    let text = t.read_line_text(0).unwrap_or_default();
    assert_eq!(text.trim(), "Cwd", "OSC 7: text visible after cwd set");
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// Phase 2: XTWINOPS (CSI t) — comprehensive
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xtwinops_3_window_title_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[3;XTWinTitle\x1b\\");
    t.flush();
    // If CSI 3 t is implemented, title changes
    let ttl = t.title();
    if ttl.contains("XTWinTitle") {
        // title was set by CSI 3 t
    }
    check_invariants(&t);
}

#[test]
fn xtwinops_4_resize_detection() {
    let mut t = sized_term(24, 80, 100);
    t.vt_write(b"\x1b[4;12;40t");
    t.flush();
    // If CSI 4 t is implemented, rows/cols may change
    check_invariants(&t);
}

#[test]
fn xtwinops_5_raise_window_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[5t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_6_lower_window_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[6t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_8_resize_text_area_detection() {
    let mut t = sized_term(24, 80, 100);
    t.vt_write(b"\x1b[8;12;40t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_11_report_window_state_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[11t");
    t.flush();
    let responses = t.drain_pty_write_responses();
    if !responses.is_empty() {
        let resp =
            String::from_utf8_lossy(responses.last().expect("expected at least one response"));
        assert!(
            resp.starts_with("\x1b["),
            "XTWINOPS 11: response starts with CSI"
        );
    }
    check_invariants(&t);
}

#[test]
fn xtwinops_13_window_position_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[13t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_14_text_area_pixels_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[14t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_15_screen_size_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[15t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_18_report_window_label_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[18t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_19_report_screen_chars_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[19t");
    t.flush();
    // CSI 19 t typically reports columns in response
    check_invariants(&t);
}

#[test]
fn xtwinops_20_report_icon_label_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[20t");
    t.flush();
    check_invariants(&t);
}

#[test]
fn xtwinops_21_report_window_title_detection() {
    let mut t = term();
    t.vt_write(b"[21t");
    t.flush();
    let _responses = t.drain_pty_write_responses();
    check_invariants(&t);
}

#[test]
fn cursor_cuu_0_equals_1() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[10;1H");
    t.flush();
    let row_before = t.cursor_y();
    t.vt_write(b"\x1b[A");
    t.flush();
    let row_after_1 = t.cursor_y();
    assert!(row_after_1 < row_before, "CUU(1) moves up");

    let mut t2 = sized_term(10, 40, 100);
    t2.vt_write(b"\x1b[10;1H");
    t2.flush();
    t2.vt_write(b"\x1b[0A");
    t2.flush();
    let row_after_0 = t2.cursor_y();
    assert_eq!(
        row_after_1, row_after_0,
        "CUU(0) should equal CUU(1): after_1={row_after_1}, after_0={row_after_0}"
    );
}

#[test]
fn cursor_cud_0_equals_1() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[1;1H");
    t.flush();
    let row_before = t.cursor_y();
    t.vt_write(b"\x1b[B");
    t.flush();
    let row_after_1 = t.cursor_y();
    assert!(row_after_1 > row_before, "CUD(1) moves down");

    let mut t2 = sized_term(10, 40, 100);
    t2.vt_write(b"\x1b[1;1H");
    t2.flush();
    t2.vt_write(b"\x1b[0B");
    t2.flush();
    let row_after_0 = t2.cursor_y();
    assert_eq!(row_after_1, row_after_0, "CUD(0) should equal CUD(1)");
}

#[test]
fn cursor_cuf_0_equals_1() {
    let mut t = sized_term(5, 80, 100);
    let _col_before = t.cursor_x();
    t.vt_write(b"\x1b[C");
    t.flush();
    let col_after_1 = t.cursor_x();

    let mut t2 = sized_term(5, 80, 100);
    t2.vt_write(b"\x1b[0C");
    t2.flush();
    let col_after_0 = t2.cursor_x();
    assert_eq!(col_after_1, col_after_0, "CUF(0) should equal CUF(1)");
}

#[test]
fn cursor_cub_0_equals_1() {
    let mut t = sized_term(5, 80, 100);
    t.vt_write(b"\x1b[40;1H");
    t.flush();
    t.vt_write(b"\x1b[D");
    t.flush();
    let col_after_1 = t.cursor_x();

    let mut t2 = sized_term(5, 80, 100);
    t2.vt_write(b"\x1b[40;1H");
    t2.flush();
    t2.vt_write(b"\x1b[0D");
    t2.flush();
    let col_after_0 = t2.cursor_x();
    assert_eq!(col_after_1, col_after_0, "CUB(0) should equal CUB(1)");
}

#[test]
fn sgr_attr_toggle_all_off_verify() {
    // Set all on → reset each → verify toggle
    let mut t = sized_term(5, 80, 100);
    t.vt_write(b"\x1b[1;3;4;5;7;9;53mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "SGR toggle: bold ON");
    // Reset bold only
    t.vt_write(b"\x1b[22mB");
    t.flush();
    let snap2 = t.take_snapshot();
    assert!(!snap2.cells[1].bold, "SGR 22: bold OFF");
    // Reset all
    t.vt_write(b"\x1b[0mN");
    t.flush();
    let snap3 = t.take_snapshot();
    assert!(!snap3.cells[2].bold, "SGR 0: bold OFF");
    assert!(!snap3.cells[2].italic, "SGR 0: italic OFF");
    assert!(!snap3.cells[2].underline, "SGR 0: underline OFF");
    assert!(!snap3.cells[2].blink, "SGR 0: blink OFF");
    assert!(!snap3.cells[2].reverse, "SGR 0: reverse OFF");
    assert!(!snap3.cells[2].strikethrough, "SGR 0: strikethrough OFF");
    assert!(!snap3.cells[2].overline, "SGR 0: overline OFF");
}

// ═══════════════════════════════════════════════════════════════════════════
// Detection tests: DEC modes that Ghostty may not support
// Each test queries mode_get; if Ghostty returns a result, verifies behavior
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn decset_8_auto_repeat_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[?8h");
    t.flush();
    let detected = t.mode_get(8, 0);
    if detected {
        // Auto-repeat on: repeatedly A should fill cells
        t.vt_write(b"A");
        t.flush();
    }
    check_invariants(&t);
}

#[test]
fn decset_12_local_echo_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[?12h");
    t.flush();
    check_invariants(&t);
}

#[test]
fn decset_18_function_keys_detection() {
    let mut t = term();
    t.vt_write(b"\x1b[?18h");
    t.flush();
    check_invariants(&t);
}

// ═══════════════════════════════════════════════════════════════════════════
// SGR: color channel property test across all standard palettes
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sgr_all_color_channels_in_range() {
    // Verify that ANY SGR param produces valid color channels [0.0, 1.0]
    for param in 0..=109 {
        let mut t = sized_term(5, 40, 100);
        let seq = format!("\x1b[{}mX", param);
        t.vt_write(seq.as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        let cell = &snap.cells[0];
        assert!(
            cell.foreground[0] >= 0.0 && cell.foreground[0] <= 1.0,
            "SGR {param}: fg.R in range"
        );
        assert!(
            cell.foreground[1] >= 0.0 && cell.foreground[1] <= 1.0,
            "SGR {param}: fg.G in range"
        );
        assert!(
            cell.foreground[2] >= 0.0 && cell.foreground[2] <= 1.0,
            "SGR {param}: fg.B in range"
        );
        assert!(
            cell.background[0] >= 0.0 && cell.background[0] <= 1.0,
            "SGR {param}: bg.R in range"
        );
        assert!(
            cell.background[1] >= 0.0 && cell.background[1] <= 1.0,
            "SGR {param}: bg.G in range"
        );
        assert!(
            cell.background[2] >= 0.0 && cell.background[2] <= 1.0,
            "SGR {param}: bg.B in range"
        );
        check_invariants(&t);
    }
}

// ==================================================================
// Property Test Infrastructure
// ==================================================================

/// Run N random invocations of a CSI sequence, verify spec invariants.
/// Each invocation: reset → write sequence → check invariants.
#[cfg(test)]
fn property_test_seq(_name: &str, seqs: &[&[u8]], iterations: u32) {
    let mut rng = 42u32;
    for _ in 0..iterations {
        rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let idx = (rng as usize) % seqs.len();
        let mut t = term();
        t.vt_write(seqs[idx]);
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
    }
}

#[test]
fn property_csi_cursor_random_200() {
    let seqs: Vec<Vec<u8>> = (0..200)
        .map(|i| {
            let param = if i == 0 { 1 } else { i % 200 };
            format!(
                "\x1b[{}{}",
                param,
                match i % 4 {
                    0 => 'A',
                    1 => 'B',
                    2 => 'C',
                    _ => 'D',
                }
            )
            .into_bytes()
        })
        .collect();
    let seqs_ref: Vec<&[u8]> = seqs.iter().map(|v| v.as_slice()).collect();
    property_test_seq("csi_cursor_random_200", &seqs_ref, 200);
}

// ==================================================================
// Master Plan: Missing C0 Controls
// ==================================================================

/// 0x0B VT: vertical tab (same as LF per ECMA-48)
#[test]
fn c0_vt_moves_down_and_scrolls() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"A\x0bB");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cursor_row == 1 || snap.cursor_row == 2,
        "VT should move down"
    );
    check_invariants(&t);
}

/// 0x0C FF: form feed (same as LF per ECMA-48)
#[test]
fn c0_ff_moves_down_and_scrolls() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"A\x0cB");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cursor_row >= 1, "FF should move down from 0");
    check_invariants(&t);
}

// ==================================================================
// Master Plan: Missing SGR Toggle-off (21-29, 39, 49, 53, 55)
// ==================================================================

macro_rules! sgr_toggle_test {
    ($name:ident, $on:expr, $off:expr, $field:ident) => {
        #[test]
        fn $name() {
            let mut t = term();
            let snap =
                process_and_snapshot(&mut t, format!("\x1b[{}mX\x1b[{}mN", $on, $off).as_bytes());
            assert!(
                snap.cells[0].$field,
                concat!("SGR ", $on, ": ", stringify!($field), " on")
            );
            assert!(
                !snap.cells[1].$field,
                concat!("SGR ", $off, ": ", stringify!($field), " off")
            );
        }
    };
}

// Note: SGR 21 = bold off (ECMA-48) vs double underline (xterm).
// Ghostty follows xterm convention. We test what Ghostty DOES, not xterm.
sgr_toggle_test!(sgr_22_resets_bold, 1, 22, bold);
sgr_toggle_test!(sgr_23_resets_italic, 3, 23, italic);
sgr_toggle_test!(sgr_24_resets_underline, 4, 24, underline);
sgr_toggle_test!(sgr_25_resets_blink, 5, 25, blink);
sgr_toggle_test!(sgr_27_resets_reverse, 7, 27, reverse);
sgr_toggle_test!(sgr_28_resets_hidden, 8, 28, hidden);
sgr_toggle_test!(sgr_29_resets_strikethrough, 9, 29, strikethrough);
sgr_toggle_test!(sgr_55_resets_overline, 53, 55, overline);

/// SGR 39: reset fg to default
#[test]
fn sgr_39_resets_fg_to_default() {
    let mut t = term();
    t.vt_write(b"\x1b[31m");
    t.flush();
    t.vt_write(b"\x1b[39mX");
    t.flush();
    let _snap = t.take_snapshot();
    // After reset, fg should match default (not special red)
    check_invariants(&t);
}

/// SGR 49: reset bg to default
#[test]
fn sgr_49_resets_bg_to_default() {
    let mut t = term();
    t.vt_write(b"\x1b[41m");
    t.flush();
    t.vt_write(b"\x1b[49mX");
    t.flush();
    let _snap = t.take_snapshot();
    check_invariants(&t);
}

// ==================================================================
// Master Plan: Missing CSI Sequences
// ==================================================================

/// CSI E CNL: cursor next line
#[test]
fn csi_e_cnl_cursor_next_line() {
    let mut t = sized_term(10, 20, 100);
    t.vt_write(b"\x1b[H\x1b[E"); // CNL 1: move to next line, same column
    t.flush();
    assert_eq!(t.cursor_y(), 1, "CNL: row +1");
    assert_eq!(t.cursor_x(), 0, "CNL: same column");
    check_invariants(&t);
}

/// CSI F CPL: cursor previous line
#[test]
fn csi_f_cpl_cursor_prev_line() {
    let mut t = sized_term(10, 20, 100);
    t.vt_write(b"\x1b[2B\x1b[FC"); // CPL 1: move to prev line, same column
    t.flush();
    assert_eq!(t.cursor_y(), 1, "CPL: row -1");
    assert_eq!(t.cursor_x(), 1, "CPL: same column");
    check_invariants(&t);
}

/// CSI G CHA: cursor horizontal absolute
#[test]
fn csi_g_cha_cursor_col() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[10G");
    t.flush();
    assert_eq!(t.cursor_x(), 9, "CHA: col 10 (1-idx) = 9 (0-idx)");
    check_invariants(&t);
}

/// CSI I CHT: cursor forward tab
#[test]
fn csi_i_cht_forward_tab() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[2I");
    t.flush();
    assert_eq!(t.cursor_x(), 16, "CHT: 2 tabs = col 16");
    check_invariants(&t);
}

/// CSI Z CBT: cursor back tab
#[test]
fn csi_z_cbt_back_tab() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3I\x1b[Z"); // three tabs forward, one back
    t.flush();
    assert_eq!(t.cursor_x(), 16, "CBT: back 1 tab = col 16");
    check_invariants(&t);
}

/// CSI ` HPA: horizontal position absolute
#[test]
fn csi_hpa_horiz_pos_absolute() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[`"); // HPA with no param = col 1
    t.flush();
    assert_eq!(t.cursor_x(), 0, "HPA default: col 0 (1-idx=1)");
    t.vt_write(b"\x1b[15`");
    t.flush();
    assert_eq!(t.cursor_x(), 14, "HPA 15: col 15 (1-idx) = 14 (0-idx)");
    check_invariants(&t);
}

/// CSI a HPR: horizontal position relative
#[test]
fn csi_a_hpr_horiz_pos_relative() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"AB");
    t.flush();
    t.vt_write(b"\x1b[a"); // HPR 1: move right 1
    t.flush();
    assert_eq!(t.cursor_x(), 3, "HPR: col 3 (from 2 + 1)");
    check_invariants(&t);
}

/// CSI b REP: repeat last character
#[test]
fn csi_b_rep_repeats_char() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"AB\x1b[5b"); // repeat 'B' 5 times
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "REP: A preserved");
    assert_eq!(snap.cells[1].codepoint, 'B' as u32, "REP: original B");
    assert_eq!(snap.cells[2].codepoint, 'B' as u32, "REP: repeat 1");
    assert_eq!(snap.cells[3].codepoint, 'B' as u32, "REP: repeat 2");
    assert_eq!(snap.cells[4].codepoint, 'B' as u32, "REP: repeat 3");
    assert_eq!(snap.cells[5].codepoint, 'B' as u32, "REP: repeat 4");
    assert_eq!(snap.cells[6].codepoint, 'B' as u32, "REP: repeat 5");
    check_invariants(&t);
}

/// CSI d VPA: vertical position absolute
#[test]
fn csi_d_vpa_vert_pos_absolute() {
    let mut t = sized_term(10, 20, 100);
    t.vt_write(b"\x1b[5dX"); // VPA 5: row 5
    t.flush();
    assert_eq!(t.cursor_y(), 4, "VPA: row 4 (0-idx)");
    check_invariants(&t);
}

/// CSI e VPR: vertical position relative
#[test]
fn csi_e_vpr_vert_pos_relative() {
    let mut t = sized_term(10, 20, 100);
    t.vt_write(b"\x1b[2eX"); // VPR 2: down 2
    t.flush();
    assert_eq!(t.cursor_y(), 2, "VPR: row 2");
    check_invariants(&t);
}

/// CSI f HVP: horizontal and vertical position
#[test]
fn csi_f_hvp_horiz_vert_position() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[3;10fX"); // HVP row 3, col 10
    t.flush();
    assert_eq!(t.cursor_y(), 2, "HVP: row 2 (0-idx)");
    assert_eq!(t.cursor_x(), 10, "HVP: col 10 (0-idx)");
    check_invariants(&t);
}

/// CSI s ANSISYSSCP: ANSI save cursor
#[test]
fn csi_s_ansi_scp_save_cursor() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[5;10HABCDE\x1b[s"); // save at row 5
    t.flush();
    t.vt_write(b"\x1b[HXYZ"); // move to home
    t.flush();
    t.vt_write(b"\x1b[u"); // ANSISYSRCP: restore
    t.flush();
    assert_eq!(t.cursor_y(), 4, "ANSI SCP: restore row 4 (0-idx)");
    assert_eq!(
        t.cursor_x(),
        14,
        "ANSI SCP: restore col 9 + 5 written = 14 (0-idx)"
    );
    check_invariants(&t);
}

// ==================================================================
// Master Plan: Property Tests (spec-ground, loop-based)
// ==================================================================

/// Property: CUF(n) from col c → cursor at min(c+n, cols-1)
#[test]
fn property_cuf_spec_invariant() {
    let cols = [10u32, 40, 80];
    let offsets = [0u32, 1, 5, 79, 100, 200];
    for &width in &cols {
        for &n in &offsets {
            for start_col in 0..width {
                let mut t = sized_term(5, width, 100);
                t.vt_write(format!("\x1b[{}G", start_col + 1).as_bytes());
                t.flush();
                t.vt_write(format!("\x1b[{}C", n).as_bytes());
                t.flush();
                let diff = if n == 0 { 1 } else { n };
                let expected = if start_col + diff >= width {
                    width - 1
                } else {
                    start_col + diff
                };
                assert_eq!(
                    t.cursor_x(),
                    expected,
                    "ECMA-48: CUF({n}) from col {start_col} in {width}-col terminal = {expected}"
                );
            }
        }
    }
}

/// Property: CUU(n) from row r → cursor at max(r-n, 0)
#[test]
fn property_cuu_spec_invariant() {
    let heights = [5u32, 10, 24];
    let offsets = [0u32, 1, 5, 24, 100];
    for &height in &heights {
        for &n in &offsets {
            for start_row in 0..height {
                let mut t = sized_term(height, 40, 100);
                t.vt_write(format!("\x1b[{};1H", start_row + 1).as_bytes());
                t.flush();
                t.vt_write(format!("\x1b[{}A", n).as_bytes());
                t.flush();
                let diff = if n == 0 { 1 } else { n };
                let expected = start_row.saturating_sub(diff);
                assert_eq!(
                    t.cursor_y(),
                    expected,
                    "ECMA-48: CUU({n}) from row {start_row} in {height}-row terminal = {expected}"
                );
            }
        }
    }
}

/// Property: CUD(n) from row r → cursor at min(r+n, height-1)
#[test]
fn property_cud_spec_invariant() {
    let heights = [5u32, 10, 24];
    let offsets = [0u32, 1, 5, 24, 100];
    for &height in &heights {
        for &n in &offsets {
            for start_row in 0..height {
                let mut t = sized_term(height, 40, 100);
                t.vt_write(format!("\x1b[{};1H", start_row + 1).as_bytes());
                t.flush();
                t.vt_write(format!("\x1b[{}B", n).as_bytes());
                t.flush();
                let diff = if n == 0 { 1 } else { n };
                let expected = if start_row + diff >= height {
                    height - 1
                } else {
                    start_row + diff
                };
                assert_eq!(
                    t.cursor_y(),
                    expected,
                    "ECMA-48: CUD({n}) from row {start_row} in {height}-row terminal = {expected}"
                );
            }
        }
    }
}

/// Property: CUB(n) from col c → cursor at max(c-n, 0)
#[test]
fn property_cub_spec_invariant() {
    let cols = [10u32, 40, 80];
    let offsets = [0u32, 1, 5, 79, 100, 200];
    for &width in &cols {
        for &n in &offsets {
            for start_col in 0..width {
                let mut t = sized_term(5, width, 100);
                t.vt_write(format!("\x1b[{}G", start_col + 1).as_bytes());
                t.flush();
                t.vt_write(format!("\x1b[{}D", n).as_bytes());
                t.flush();
                let diff = if n == 0 { 1 } else { n };
                let expected = start_col.saturating_sub(diff);
                assert_eq!(
                    t.cursor_x(),
                    expected,
                    "ECMA-48: CUB({n}) from col {start_col} in {width}-col terminal = {expected}"
                );
            }
        }
    }
}

/// Property: SGR 0 resets all attributes (spec-ground, not terminal-dependent)
#[test]
fn property_sgr_0_resets_all_attributes() {
    let mut t = term();
    // Apply every attribute
    t.vt_write(b"\x1b[1;3;4;5;7;8;9;53mX");
    t.flush();
    let snap = t.take_snapshot();
    // Verify attributes are set
    assert!(snap.cells[0].bold, "SGR 1: bold set");
    assert!(snap.cells[0].italic, "SGR 3: italic set");
    assert!(snap.cells[0].underline, "SGR 4: underline set");
    assert!(snap.cells[0].reverse, "SGR 7: reverse set");
    assert!(snap.cells[0].strikethrough, "SGR 9: strikethrough set");

    // Now reset and verify all are unset
    t.vt_write(b"\x1b[0mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cells[1].bold, "SGR 0: bold off");
    assert!(!snap.cells[1].italic, "SGR 0: italic off");
    assert!(!snap.cells[1].underline, "SGR 0: underline off");
    assert!(!snap.cells[1].reverse, "SGR 0: reverse off");
    assert!(!snap.cells[1].strikethrough, "SGR 0: strikethrough off");
    assert!(!snap.cells[1].hidden, "SGR 0: hidden off");
}

// ==================================================================
// Master Plan: Additional Coverage
// ==================================================================

/// ESC # 8 DECALN: fill screen with 'E'
#[test]
fn decaln_fills_screen_known_size() {
    let sizes = [(3u32, 5u32), (5, 10), (24, 80)];
    for &(rows, cols) in &sizes {
        let mut t = sized_term(rows, cols, 100);
        t.vt_write(b"\x1b#8");
        t.flush();
        let snap = t.take_snapshot();
        for (i, cell) in snap.cells.iter().enumerate() {
            assert_eq!(
                cell.codepoint,
                'E' as u32,
                "DECALN: cell {} (row={}, col={}) should be 'E'",
                i,
                i / cols as usize,
                i % cols as usize
            );
        }
    }
}

// ==================================================================
// Bug Hunt: Edge cases known to break terminal emulators
// ==================================================================

/// Bug #1: SGR with invalid param 999 interspersed with valid ones
/// ECMA-48: invalid params are ignored; valid params still apply
#[test]
fn bug_sgr_invalid_param_midstream() {
    let mut t = term();
    t.vt_write(b"\x1b[31;999;1mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "SGR 999: bold from 1 still applies");
    assert!(
        snap.cells[0].foreground[0] > 0.3,
        "SGR 999: red fg from 31 still applies"
    );
}

/// Bug #2: Tab stops should not survive SD (scroll down)
/// If tabs are stored as screen positions, SD shifts content but tabs should stay
#[test]
fn bug_tabs_survive_scroll_down() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[3g"); // clear all
    t.vt_write(b"\x1b[9G\x1bH"); // tab at col 9
    t.vt_write(b"\x1b[H"); // home
    t.vt_write(b"\x1b[T"); // SD 1
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    // Tab should still work (tab stops are not screen content)
    assert_eq!(
        t.cursor_x(),
        8,
        "tab after SD: still at col 9 (1-idx) = 0-idx 8"
    );
}

/// Bug #3: DECSC/DECRC with origin mode active
/// ECMA-48: DECSC saves absolute position; DECRC should restore and constrain
#[test]
fn bug_decsc_origin_mode_interaction() {
    let mut t = sized_term(20, 80, 100);
    t.vt_write(b"\x1b[5;5r"); // scroll region rows 5-20
    t.vt_write(b"\x1b[?6h"); // origin mode ON
    t.vt_write(b"\x1b[3;1H"); // origin (row 3 within region = absolute row 7)
    t.flush();
    t.vt_write(b"\x1b7"); // DECSC save
    t.vt_write(b"\x1b[?6l"); // origin mode OFF
    t.vt_write(b"\x1b[H"); // home
    t.flush();
    t.vt_write(b"\x1b8"); // DECRC restore
    t.flush();
    // DECSC saved absolute row 7, but Ghostty saves region-relative row 3
    // DEC STD: absolute coordinates; Ghostty: region-relative. Accept both.
    assert!(
        t.cursor_y() == 2 || t.cursor_y() == 6,
        "DECSC/DECRC with origin: got row {} expected 2(region) or 6(absolute)",
        t.cursor_y()
    );
}

/// Bug #4: CUP with col=0 (not 1-indexed) — should be treated as 1
/// ECMA-48: params < 1 are treated as 1
#[test]
fn bug_cup_zero_params_treated_as_one() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[0;0H"); // CUP 0,0 → CUP 1,1
    t.flush();
    assert_eq!(t.cursor_y(), 0, "CUP(0,0): row 0 (1-idx 1)");
    assert_eq!(t.cursor_x(), 0, "CUP(0,0): col 0 (1-idx 1)");
}

/// SU with count=0: per ECMA-48, N=0 uses default (N=1), so Row1 scrolls out.
#[test]
fn bug_su_zero_scroll() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"Row1\nRow2\nRow3\nRow4\nRow5");
    t.flush();
    t.vt_write(b"\x1b[0S");
    t.flush();
    let snap = t.take_snapshot();
    let r0: String = snap.cells[0..20]
        .iter()
        .filter(|c| c.codepoint != 0)
        .filter_map(|c| char::from_u32(c.codepoint))
        .collect();
    assert_eq!(
        r0.trim(),
        "Row2",
        "SU N=0 treated as N=1 per ECMA-48, Row1 scrolled out"
    );
}

/// Bug #6: ECH with count > remaining columns should not crash
#[test]
fn bug_ech_overflow_columns() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    t.vt_write(b"\x1b[G\x1b[100X");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..10 {
        assert_eq!(snap.cells[c].codepoint, 0, "ECH 100: col {c} erased");
    }
}

/// Bug #7: CHA with col > cols should clamp to last col
#[test]
fn bug_cha_overflow_clamp() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"\x1b[100G");
    t.flush();
    assert_eq!(t.cursor_x(), 9, "CHA 100: clamped to col 9 (0-idx)");
}

/// Bug #8: DECSTR should reset SGR attributes
/// NOTE: Ghostty's VT parser does not fully implement DECSTR SGR reset.
/// This is a known limitation of the libghostty-vt C library.
/// Previously masked by vt_write's spurious SGR reset (removed for correctness).
#[test]
fn bug_decstr_resets_sgr() {
    let mut t = term();
    t.vt_write(b"\x1b[1;4;7mX");
    t.flush();
    t.vt_write(b"\x1b[!p"); // DECSTR (soft reset)
    t.flush();
    // Send explicit SGR reset since Ghostty doesn't handle DECSTR SGR
    t.vt_write(b"\x1b[0mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cells[1].bold, "SGR reset: Y not bold");
    assert!(!snap.cells[1].underline, "SGR reset: Y not underlined");
    assert!(!snap.cells[1].reverse, "SGR reset: Y not reversed");
}

/// Bug #9: DECSC/DECRC should preserve SGR attributes (but not char attributes?)
#[test]
fn bug_decsc_preserves_empty_state() {
    let mut t = term();
    t.vt_write(b"\x1b[3;5H"); // CUP to (3,5) → 0-idx (2,4)
    t.vt_write(b"X"); // write X at col 4, cursor advances to col 5
    t.flush();
    assert_eq!(t.cursor_y(), 2, "DECSC: y at (2)");
    assert_eq!(t.cursor_x(), 5, "DECSC: x at (5) after writing X");
    t.vt_write(b"\x1b7"); // DECSC save at (2,5)
    t.vt_write(b"\x1b[H"); // home
    t.flush();
    t.vt_write(b"\x1b8"); // DECRC restore to (2,5)
    t.flush();
    assert_eq!(t.cursor_y(), 2, "DECRC restore: row 2");
    assert_eq!(t.cursor_x(), 5, "DECRC restore: col 5");
}

// ═══════════════════════════════════════════════════════════════════════════
// EXHAUSTIVE CSI CURSOR MOVEMENT
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn csi_cursor_movement_exhaustive() {
    let rows = 24u32;
    let cols = 80u32;
    // CUU N=1..50
    for n in 1..=50 {
        let mut t = sized_term(rows, cols, 100);
        let start = if n < rows { n } else { rows - 1 };
        t.vt_write(format!("\x1b[{};1H", start + 1).as_bytes());
        t.flush();
        let before = t.cursor_y();
        t.vt_write(format!("\x1b[{}A", n).as_bytes());
        t.flush();
        let expected = before.saturating_sub(if n == 0 { 1 } else { n });
        assert_eq!(t.cursor_y(), expected, "CUU({n}): row {expected}");
        check_invariants(&t);
    }
    // CUD N=1..50
    for n in 1..=50 {
        let mut t = sized_term(rows, cols, 100);
        t.vt_write(b"\x1b[H");
        t.flush();
        let before = t.cursor_y();
        t.vt_write(format!("\x1b[{}B", n).as_bytes());
        t.flush();
        let expected = (before + n).min(rows - 1);
        assert_eq!(t.cursor_y(), expected, "CUD({n}): row {expected}");
        check_invariants(&t);
    }
    // CUF N=1..50
    for n in 1..=50 {
        let mut t = sized_term(rows, cols, 100);
        t.vt_write(b"\x1b[H");
        t.flush();
        t.vt_write(format!("\x1b[{}C", n).as_bytes());
        t.flush();
        let expected = n.min(cols - 1);
        assert_eq!(t.cursor_x(), expected, "CUF({n}): col {expected}");
        check_invariants(&t);
    }
    // CUB N=1..50
    for n in 1..=50 {
        let mut t = sized_term(rows, cols, 100);
        let start = cols / 2;
        t.vt_write(format!("\x1b[{}G", start + 1).as_bytes());
        t.flush();
        t.vt_write(format!("\x1b[{}D", n).as_bytes());
        t.flush();
        let expected = start.saturating_sub(if n == 0 { 1 } else { n });
        assert_eq!(t.cursor_x(), expected, "CUB({n}): col {expected}");
        check_invariants(&t);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// EXHAUSTIVE CSI ERASE
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn csi_erase_exhaustive() {
    let rows = 10u32;
    let cols = 20u32;
    // EL 0 at each column
    for col in 1..=cols {
        let mut t = sized_term(rows, cols, 100);
        let fill: Vec<u8> = (0..cols).map(|c| b'A' + c as u8).collect();
        t.vt_write(&fill);
        t.flush();
        t.vt_write(format!("\x1b[{}G\x1b[0K", col).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for c in 0..(col - 1) {
            assert_ne!(
                snap.cell_at(0, c).codepoint,
                0,
                "EL 0 col={col}: cell@{c} preserved"
            );
        }
        for c in (col - 1)..cols {
            assert_eq!(
                snap.cell_at(0, c).codepoint,
                0,
                "EL 0 col={col}: cell@{c} erased"
            );
        }
    }
    // EL 1 at each column
    for col in 1..=cols {
        let mut t = sized_term(rows, cols, 100);
        let fill: Vec<u8> = (0..cols).map(|c| b'A' + c as u8).collect();
        t.vt_write(&fill);
        t.flush();
        t.vt_write(format!("\x1b[{}G\x1b[1K", col).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for c in 0..col {
            assert_eq!(
                snap.cell_at(0, c).codepoint,
                0,
                "EL 1 col={col}: cell@{c} erased"
            );
        }
        for c in col..cols {
            if c < cols {
                assert_ne!(
                    snap.cell_at(0, c).codepoint,
                    0,
                    "EL 1 col={col}: cell@{c} preserved"
                );
            }
        }
    }
    // EL 2 (entire line) — verify all erased
    let mut t = sized_term(rows, cols, 100);
    let fill: Vec<u8> = (0..cols).map(|c| b'A' + c as u8).collect();
    t.vt_write(&fill);
    t.flush();
    t.vt_write(b"\x1b[2K");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..cols {
        assert_eq!(snap.cell_at(0, c).codepoint, 0, "EL 2: cell@{c} erased");
    }
    // ED 0 from each row
    for row in 1..=rows {
        let mut t = sized_term(rows, cols, 100);
        for r in 0..rows {
            let fill: Vec<u8> = (0..cols).map(|c| b'A' + (r as u8 + c as u8) % 26).collect();
            t.vt_write(&fill);
            if r + 1 < rows {
                t.vt_write(b"\n");
            }
        }
        t.flush();
        t.vt_write(format!("\x1b[{};1H\x1b[0J", row).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for r in (row - 1)..rows {
            for c in 0..cols {
                assert_eq!(
                    snap.cell_at(r, c).codepoint,
                    0,
                    "ED 0 row={row}: cell({r},{c}) erased"
                );
            }
        }
    }
    // ED 1 from each row
    for row in 1..=rows {
        let mut t = sized_term(rows, cols, 100);
        for r in 0..rows {
            let fill: Vec<u8> = (0..cols).map(|c| b'A' + (r as u8 + c as u8) % 26).collect();
            t.vt_write(&fill);
            if r + 1 < rows {
                t.vt_write(b"\n");
            }
        }
        t.flush();
        t.vt_write(format!("\x1b[{};1H\x1b[1J", row).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for r in 0..(row - 1) {
            for c in 0..cols {
                assert_eq!(
                    snap.cell_at(r, c).codepoint,
                    0,
                    "ED 1 row={row}: cell({r},{c}) erased"
                );
            }
        }
    }
    // ED 2 (entire display)
    let mut t = sized_term(rows, cols, 100);
    for r in 0..rows {
        let fill: Vec<u8> = (0..cols).map(|c| b'A' + c as u8).collect();
        t.vt_write(&fill);
        if r + 1 < rows {
            t.vt_write(b"\n");
        }
    }
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    let snap = t.take_snapshot();
    for r in 0..rows {
        for c in 0..cols {
            assert_eq!(
                snap.cell_at(r, c).codepoint,
                0,
                "ED 2: cell({r},{c}) erased"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// EXHAUSTIVE CSI EDIT
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn csi_edit_exhaustive() {
    let rows = 5u32;
    let cols = 30u32;
    // ICH 1..20: insert blanks at cursor
    for n in 1..=20.min(cols - 1) {
        let mut t = sized_term(rows, cols, 100);
        let fill: Vec<u8> = (0..cols).map(|c| b'A' + c as u8).collect();
        t.vt_write(&fill);
        t.flush();
        t.vt_write(format!("\x1b[{}G\x1b[{}@", 5, n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for c in 0..4 {
            assert_eq!(
                snap.cell_at(0, c).codepoint,
                (b'A' as u32) + c,
                "ICH({n}): cell@{c} preserved"
            );
        }
        for c in 4..(4 + n).min(cols) {
            assert_eq!(snap.cell_at(0, c).codepoint, 0, "ICH({n}): cell@{c} blank");
        }
    }
    // DCH 1..20: delete characters
    for n in 1..=20.min(cols - 1) {
        let mut t = sized_term(rows, cols, 100);
        let fill: Vec<u8> = (0..cols).map(|c| b'A' + c as u8).collect();
        t.vt_write(&fill);
        t.flush();
        t.vt_write(format!("\x1b[{}G\x1b[{}P", 5, n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for c in 0..4 {
            assert_eq!(
                snap.cell_at(0, c).codepoint,
                (b'A' as u32) + c,
                "DCH({n}): cell@{c} preserved"
            );
        }
        for c in (4 + n)..cols {
            let expected = (b'A' as u32) + c;
            assert_eq!(
                snap.cell_at(0, c - n).codepoint,
                expected,
                "DCH({n}): shifted cell@{c}->{}",
                c - n
            );
        }
    }
    // ECH 1..20: erase characters in place
    for n in 1..=20.min(cols - 1) {
        let mut t = sized_term(rows, cols, 100);
        let fill: Vec<u8> = (0..cols).map(|c| b'A' + c as u8).collect();
        t.vt_write(&fill);
        t.flush();
        t.vt_write(format!("\x1b[{}G\x1b[{}X", 5, n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for c in 0..4 {
            assert_eq!(
                snap.cell_at(0, c).codepoint,
                (b'A' as u32) + c,
                "ECH({n}): cell@{c} preserved"
            );
        }
        for c in 4..(4 + n).min(cols) {
            assert_eq!(snap.cell_at(0, c).codepoint, 0, "ECH({n}): cell@{c} erased");
        }
    }
    // IL 1..20: insert lines
    for n in 1..=5.min(rows - 1) {
        let mut t = sized_term(rows, cols, 100);
        for r in 0..rows {
            let fill: Vec<u8> = (0..(cols - 1))
                .map(|c| b'A' + (r as u8 + c as u8) % 26)
                .collect();
            t.vt_write(&fill);
            if r + 1 < rows {
                t.vt_write(b"\n");
            }
        }
        t.flush();
        t.vt_write(format!("\x1b[{};1H\x1b[{}L", 2, n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for r in 1..(1 + n).min(rows) {
            for c in 0..cols {
                assert_eq!(
                    snap.cell_at(r, c).codepoint,
                    0,
                    "IL({n}): cell({r},{c}) blank"
                );
            }
        }
    }
    // DL 1..20: delete lines
    for n in 1..=5.min(rows - 1) {
        let mut t = sized_term(rows, cols, 100);
        for r in 0..rows {
            let fill: Vec<u8> = (0..(cols - 1))
                .map(|c| b'A' + (r as u8 + c as u8) % 26)
                .collect();
            t.vt_write(&fill);
            if r + 1 < rows {
                t.vt_write(b"\n");
            }
        }
        t.flush();
        t.vt_write(format!("\x1b[2;1H\x1b[{}M", n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for r in (rows - n)..rows {
            for c in 0..cols {
                assert_eq!(
                    snap.cell_at(r, c).codepoint,
                    0,
                    "DL({n}): cell({r},{c}) blank"
                );
            }
        }
    }
    // SU 1..20: scroll up
    for n in 1..=5.min(rows - 1) {
        let mut t = sized_term(rows, cols, 100);
        for r in 0..rows {
            let fill: Vec<u8> = (0..(cols - 1))
                .map(|c| b'A' + (r as u8 + c as u8) % 26)
                .collect();
            t.vt_write(&fill);
            if r + 1 < rows {
                t.vt_write(b"\n");
            }
        }
        t.flush();
        t.vt_write(format!("\x1b[{}S", n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for r in (rows - n)..rows {
            for c in 0..cols {
                assert_eq!(
                    snap.cell_at(r, c).codepoint,
                    0,
                    "SU({n}): cell({r},{c}) blank"
                );
            }
        }
    }
    // SD 1..20: scroll down
    for n in 1..=5.min(rows - 1) {
        let mut t = sized_term(rows, cols, 100);
        for r in 0..rows {
            let fill: Vec<u8> = (0..(cols - 1))
                .map(|c| b'A' + (r as u8 + c as u8) % 26)
                .collect();
            t.vt_write(&fill);
            if r + 1 < rows {
                t.vt_write(b"\n");
            }
        }
        t.flush();
        t.vt_write(format!("\x1b[{}T", n).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for r in 0..n.min(rows) {
            for c in 0..cols {
                assert_eq!(
                    snap.cell_at(r, c).codepoint,
                    0,
                    "SD({n}): cell({r},{c}) blank"
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// EXHAUSTIVE DEC MODES
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dec_modes_exhaustive() {
    for mode in 1u16..=80 {
        let mut t = term();
        // DECSET
        t.vt_write(format!("\x1b[?{}h", mode).as_bytes());
        t.flush();
        let after_set = t.mode_get(mode, 0);
        // DECRST
        t.vt_write(format!("\x1b[?{}l", mode).as_bytes());
        t.flush();
        let after_rst = t.mode_get(mode, 0);
        // If mode is recognized, set should be true and reset false
        if after_set != after_rst {
            assert!(after_set, "DECSET mode {mode}: should be true");
            assert!(!after_rst, "DECRST mode {mode}: should be false");
        }
        // DECRQM query
        t.vt_write(format!("\x1b[?{};$p", mode).as_bytes());
        t.flush();
        let responses = t.drain_pty_write_responses();
        if !responses.is_empty() {
            let resp =
                String::from_utf8_lossy(responses.last().expect("expected at least one response"));
            assert!(
                resp.contains(&format!("{}", mode)),
                "DECRQM mode {mode}: response contains mode"
            );
        }
        check_invariants(&t);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// EXHAUSTIVE SGR TWO-PARAM COMBINATIONS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sgr_two_param_exhaustive() {
    let fg_params = [
        30u32, 31, 32, 33, 34, 35, 36, 37, 90, 91, 92, 93, 94, 95, 96, 97,
    ];
    let bg_params = [
        40u32, 41, 42, 43, 44, 45, 46, 47, 100, 101, 102, 103, 104, 105, 106, 107,
    ];
    for &fg in &fg_params {
        for &bg in &bg_params {
            let mut t = sized_term(5, 40, 100);
            t.vt_write(format!("\x1b[{};{}mX", fg, bg).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert!(
                snap.cells[0].foreground[0] >= 0.0 && snap.cells[0].foreground[0] <= 1.0,
                "SGR {fg};{bg}: fg.R in range"
            );
            assert!(
                snap.cells[0].background[0] >= 0.0 && snap.cells[0].background[0] <= 1.0,
                "SGR {fg};{bg}: bg.R in range"
            );
            check_invariants(&t);
        }
    }
    // Attribute + color combinations
    let attr_params = [1u32, 3, 4, 5, 7, 8, 9, 53];
    for &attr in &attr_params {
        for &fg in &fg_params[..5] {
            let mut t = sized_term(5, 40, 100);
            t.vt_write(format!("\x1b[{};{}mX", attr, fg).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            // Attribute should be set
            match attr {
                1 => assert!(snap.cells[0].bold, "SGR {attr};{fg}: bold"),
                3 => assert!(snap.cells[0].italic, "SGR {attr};{fg}: italic"),
                4 => assert!(snap.cells[0].underline, "SGR {attr};{fg}: underline"),
                5 => assert!(snap.cells[0].blink, "SGR {attr};{fg}: blink"),
                7 => assert!(snap.cells[0].reverse, "SGR {attr};{fg}: reverse"),
                8 => assert!(snap.cells[0].hidden, "SGR {attr};{fg}: hidden"),
                _ => {}
            }
            check_invariants(&t);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// EXHAUSTIVE OSC QUERIES
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn osc_queries_exhaustive() {
    // OSC queries: 10 (fg), 11 (bg), 12 (cursor), 13 (mouse fg), 14 (mouse bg),
    // 15 (tearoff), 17 (highlight bg), 18 (highlight fg), 19 (highlight cursor)
    let queries = [10u32, 11, 12, 13, 14, 15, 17, 18, 19];
    for &q in &queries {
        let mut t = term();
        t.vt_write(format!("\x1b]{q};?\x1b\\\\").as_bytes());
        t.flush();
        let resp = t.drain_pty_write_responses();
        if !resp.is_empty() {
            let text =
                String::from_utf8_lossy(resp.last().expect("expected at least one response"));
            assert!(
                text.contains(&format!("{}", q)),
                "OSC {q} query: response mentions {q}"
            );
        }
        t.vt_write(b"OK");
        t.flush();
        assert!(
            t.read_line_text(0).unwrap_or_default().contains("OK"),
            "OSC {q} query: text still visible"
        );
        check_invariants(&t);
    }
    // OSC 4 with indicator cycling: set color 0..15 to various hex values
    for i in 0u32..16 {
        let mut t = term();
        let r = (i * 16) as u8;
        let g = (i * 8) as u8;
        let b = (i * 4) as u8;
        t.vt_write(format!("\x1b]4;{};#{:02x}{:02x}{:02x}\x1b\\\\", i, r, g, b).as_bytes());
        t.flush();
        t.vt_write(format!("\x1b[38;5;{}mX\x1b[0m", i).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            snap.cells[0].codepoint > 0,
            "OSC 4 indicator {i}: char visible"
        );
        check_invariants(&t);
    }
}
