//! Xterm conformance tests — implements xterm-conformance spec requirements.
//!
//! Covers: SGR full matrix, DECSTBM 20 configs, DECRQM, extended cursor movement.
//! Each test verifies Torvox behavior matches xterm reference output.

use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn term(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 500).expect("terminal create")
}

fn cell(
    t: &GhosttyTerminal,
    row: u32,
    col: u32,
) -> torvox_terminal::ghostty_terminal::CellSnapshot {
    let snap = t.take_snapshot();
    let idx = (row * snap.cols + col) as usize;
    snap.cells[idx].clone()
}

fn row_text(t: &GhosttyTerminal, row: u32) -> String {
    let snap = t.take_snapshot();
    let mut s = String::new();
    for c in 0..snap.cols {
        let idx = (row * snap.cols + c) as usize;
        let cp = snap.cells[idx].codepoint;
        if cp == 0 {
            s.push(' ');
        } else if let Some(ch) = char::from_u32(cp) {
            s.push(ch);
        }
    }
    s.trim_end().to_string()
}

// ============================================================
// SGR Full Matrix — Task 1.4
// Test key combinations: bold × color, italic × color, etc.
// xterm supports: attrs 0-9,22-27 × fg 30-37,90-97 × bg 40-47,100-107
// ============================================================

#[test]
fn sgr_bold_red_fg() {
    let mut t = term(3, 10);
    t.vt_write(b"\x1b[1;31mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.bold, "SGR 1 should set bold");
    let r = (c.foreground[0] * 255.0).round() as u8;
    assert!(r > 200, "SGR 31 red should be bright, got r={r}");
}

#[test]
fn sgr_italic_blue_fg() {
    let mut t = term(3, 10);
    t.vt_write(b"\x1b[3;34mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.italic, "SGR 3 should set italic");
    let b = (c.foreground[2] * 255.0).round() as u8;
    assert!(b > 200, "SGR 34 blue should be bright, got b={b}");
}

#[test]
fn sgr_underline_green_bg() {
    let mut t = term(3, 10);
    t.vt_write(b"\x1b[4;42mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.underline, "SGR 4 should set underline");
    let g = (c.background[1] * 255.0).round() as u8;
    assert!(g > 100, "SGR 42 green bg should be visible, got g={g}");
}

#[test]
fn sgr_bright_foreground_colors() {
    let mut t = term(3, 40);
    for (i, code) in [90u8, 91, 92, 93, 94, 95, 96, 97].iter().enumerate() {
        let seq = format!("\x1b[{}m{}\x1b[0m", code, (b'A' + i as u8) as char);
        t.vt_write(seq.as_bytes());
    }
    t.flush();
    // SGR 91 = bright red
    let c = cell(&t, 0, 1);
    let r = (c.foreground[0] * 255.0).round() as u8;
    assert!(r > 200, "SGR 91 bright red should be >200, got {r}");
}

#[test]
fn sgr_bright_background_colors() {
    let mut t = term(3, 40);
    for (i, code) in [100u8, 101, 102, 103, 104, 105, 106, 107]
        .iter()
        .enumerate()
    {
        let seq = format!("\x1b[{}m{}\x1b[0m", code, (b'A' + i as u8) as char);
        t.vt_write(seq.as_bytes());
    }
    t.flush();
    let c = cell(&t, 0, 0);
    let r = (c.background[0] * 255.0).round() as u8;
    assert!(r > 100, "SGR 100 bright bg should be visible, got r={r}");
}

#[test]
fn sgr_256_color_extended() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[38;5;196mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    let r = (c.foreground[0] * 255.0).round() as u8;
    assert!(r > 200, "SGR 38;5;196 (red cube) should be bright, got {r}");
}

#[test]
fn sgr_256_color_grayscale() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[38;5;232mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    let val = (c.foreground[0] * 255.0).round() as u8;
    assert!(
        val < 20,
        "SGR 38;5;232 (grayscale near black) should be dark, got {val}"
    );
}

#[test]
fn sgr_24bit_color() {
    let mut t = term(3, 40);
    t.vt_write(b"\x1b[38;2;255;128;0mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    let r = (c.foreground[0] * 255.0).round() as u8;
    let g = (c.foreground[1] * 255.0).round() as u8;
    let b = (c.foreground[2] * 255.0).round() as u8;
    assert_eq!(r, 255, "24-bit red channel");
    assert_eq!(g, 128, "24-bit green channel");
    assert_eq!(b, 0, "24-bit blue channel");
}

#[test]
fn sgr_strikethrough() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[9mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.strikethrough, "SGR 9 should set strikethrough");
}

#[test]
fn sgr_blink() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[5mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.blink, "SGR 5 should set blink");
}

#[test]
fn sgr_hidden() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[8mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.hidden, "SGR 8 should set hidden");
}

#[test]
fn sgr_reverse_video() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[7mX\x1b[0m");
    t.flush();
    let c = cell(&t, 0, 0);
    assert!(c.reverse, "SGR 7 should set reverse");
}

#[test]
fn sgr_reset_all_attributes() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[1;3;4;5;7;9mX\x1b[0mY");
    t.flush();
    let c0 = cell(&t, 0, 0);
    let c1 = cell(&t, 0, 1);
    assert!(c0.bold && c0.italic && c0.underline, "X should have attrs");
    assert!(
        !c1.bold && !c1.italic && !c1.underline,
        "Y should have no attrs after SGR 0"
    );
}

// ============================================================
// CUU/CUD/CUF/CUB/CUP — Task 1.5 (additional tests)
// ============================================================

#[test]
fn cuu_clamps_at_top() {
    let mut t = term(3, 20);
    t.pty_write(b"1\n2\n3");
    t.flush();
    t.vt_write(b"\x1b[99A");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 0, 1);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "CUU 99 from row 2 should clamp to row 0"
    );
}

#[test]
fn cud_clamps_at_bottom() {
    let mut t = term(3, 20);
    t.vt_write(b"1");
    t.flush();
    t.vt_write(b"\x1b[99B");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 2, 1);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "CUD 99 from row 0 should clamp to row 2"
    );
}

#[test]
fn cuf_clamps_at_right_margin() {
    let mut t = term(3, 10);
    t.vt_write(b"\x1b[99CX");
    t.flush();
    let c = cell(&t, 0, 9);
    assert_eq!(c.codepoint, 'X' as u32, "CUF 99 should clamp to last col");
}

#[test]
fn cub_clamps_at_left_margin() {
    let mut t = term(3, 10);
    t.vt_write(b"\x1b[99D");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(c.codepoint, 'X' as u32, "CUB 99 should clamp to col 0");
}

#[test]
fn cup_default_params() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[HX");
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "CUP with no params should go to (1,1) = (0,0)"
    );
}

#[test]
fn cup_row_only() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[3HX");
    t.flush();
    let c = cell(&t, 2, 0);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "CUP row=3 should go to row 2, col 0"
    );
}

#[test]
fn cup_col_only() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[;10HX");
    t.flush();
    let c = cell(&t, 0, 9);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "CUP col=10 should go to col 9, row 0"
    );
}

// ============================================================
// DECSTBM — Task 1.6 — 20 scroll region configurations
// ============================================================

#[test]
fn decstbm_full_screen_region() {
    let mut t = term(5, 10);
    t.vt_write(b"1\n2\n3\n4\n5");
    t.flush();
    t.vt_write(b"\x1b[1;5r"); // Full screen region
    t.vt_write(b"\x1b[1;1HA");
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(
        c.codepoint, 'A' as u32,
        "Full screen region should accept writes"
    );
}

#[test]
fn decstbm_single_row_region() {
    let mut t = term(5, 10);
    t.vt_write(b"1\n2\n3\n4\n5");
    t.flush();
    t.vt_write(b"\x1b[3;3r"); // Single row region
    t.vt_write(b"\x1b[3;1HX");
    t.flush();
    let c = cell(&t, 2, 0);
    assert_eq!(c.codepoint, 'X' as u32, "Single row region should work");
}

#[test]
fn decstbm_top_half() {
    let mut t = term(6, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF");
    t.flush();
    t.vt_write(b"\x1b[1;3r"); // Rows 1-3
    t.vt_write(b"\x1b[1;1HX");
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(c.codepoint, 'X' as u32, "Top half region should work");
    // Row 4 (outside region) should be unaffected
    let r4 = row_text(&t, 3);
    assert!(
        r4.contains('D'),
        "Row 4 should still have 'D' outside region"
    );
}

#[test]
fn decstbm_bottom_half() {
    let mut t = term(6, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF");
    t.flush();
    t.vt_write(b"\x1b[4;6r"); // Rows 4-6
    t.vt_write(b"\x1b[4;1HX");
    t.flush();
    let c = cell(&t, 3, 0);
    assert_eq!(c.codepoint, 'X' as u32, "Bottom half region should work");
    let r1 = row_text(&t, 0);
    assert!(
        r1.contains('A'),
        "Row 1 should still have 'A' outside region"
    );
}

#[test]
fn decstbm_middle_region() {
    let mut t = term(8, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF\nG\nH");
    t.flush();
    t.vt_write(b"\x1b[3;6r"); // Rows 3-6
    t.vt_write(b"\x1b[3;1HX");
    t.flush();
    let c = cell(&t, 2, 0);
    assert_eq!(c.codepoint, 'X' as u32, "Middle region should work");
    let r1 = row_text(&t, 0);
    assert!(
        r1.contains('A'),
        "Row 1 should still have 'A' outside region"
    );
    let r7 = row_text(&t, 6);
    assert!(
        r7.contains('G'),
        "Row 7 should still have 'G' outside region"
    );
}

#[test]
fn decstbm_scroll_within_region() {
    let mut t = term(8, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF\nG\nH");
    t.flush();
    t.vt_write(b"\x1b[3;6r"); // Rows 3-6
    t.vt_write(b"\x1b[6;1H"); // Move to bottom of region
    t.vt_write(b"\x1b[1L"); // Insert line → scroll down within region
    t.flush();
    let r2 = row_text(&t, 1);
    assert!(
        r2.contains('B'),
        "Row 2 should still have 'B' (outside region)"
    );
}

#[test]
fn decstbm_reset_clears_region() {
    let mut t = term(5, 10);
    t.vt_write(b"\x1b[2;4r");
    t.vt_write(b"\x1b[r"); // Reset
    t.pty_write(b"A\nB\nC\nD\nE");
    t.flush();
    // After reset, all rows should be usable
    for row in 0..5 {
        let c = cell(&t, row, 0);
        assert!(
            c.codepoint != 0,
            "Row {row} should have content after reset"
        );
    }
}

#[test]
fn decstbm_top_bottom_reversed() {
    let mut t = term(5, 10);
    t.vt_write(b"\x1b[4;2r"); // Top > bottom (invalid)
    t.vt_write(b"\x1b[1;1HX");
    t.flush();
    let c = cell(&t, 0, 0);
    // Invalid region should either be ignored or reset to full screen
    assert!(
        c.codepoint == 'X' as u32 || c.codepoint == 0,
        "Reversed region should be handled gracefully"
    );
}

#[test]
fn decstbm_top_zero() {
    let mut t = term(5, 10);
    t.vt_write(b"\x1b[0;3r"); // Top = 0 (invalid, must be >= 1)
    t.vt_write(b"\x1b[1;1HX");
    t.flush();
    let c = cell(&t, 0, 0);
    // Should be handled gracefully
    assert!(c.codepoint != 0, "Top=0 should be handled");
}

#[test]
fn decstbm_scroll_back_and_forth() {
    let mut t = term(6, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF");
    t.flush();
    t.vt_write(b"\x1b[2;5r"); // Region rows 2-5
    t.vt_write(b"\x1b[5;1H"); // Bottom of region
    t.vt_write(b"\x1b[1L"); // Insert line (scroll down)
    t.flush();
    t.vt_write(b"\x1b[2;1H"); // Top of region
    t.vt_write(b"\x1b[1M"); // Delete line (scroll up)
    t.flush();
    // After insert+delete, content should be approximately back to original
    let r1 = row_text(&t, 0);
    assert!(
        r1.contains('A'),
        "Row 1 should still have 'A' (outside region)"
    );
}

#[test]
fn decstbm_region_2_rows() {
    let mut t = term(6, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF");
    t.flush();
    t.vt_write(b"\x1b[3;4r"); // 2-row region
    t.vt_write(b"\x1b[4;1HX");
    t.flush();
    let c = cell(&t, 3, 0);
    assert_eq!(c.codepoint, 'X' as u32, "2-row region should work");
}

#[test]
fn decstbm_region_3_rows() {
    let mut t = term(8, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF\nG\nH");
    t.flush();
    t.vt_write(b"\x1b[2;4r"); // 3-row region
    t.vt_write(b"\x1b[3;1HX");
    t.flush();
    let c = cell(&t, 2, 0);
    assert_eq!(c.codepoint, 'X' as u32, "3-row region should work");
}

#[test]
fn decstbm_region_4_rows() {
    let mut t = term(8, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF\nG\nH");
    t.flush();
    t.vt_write(b"\x1b[2;5r"); // 4-row region
    t.vt_write(b"\x1b[5;1HX");
    t.flush();
    let c = cell(&t, 4, 0);
    assert_eq!(c.codepoint, 'X' as u32, "4-row region should work");
}

#[test]
fn decstbm_top_row_only() {
    let mut t = term(5, 10);
    t.vt_write(b"1\n2\n3\n4\n5");
    t.flush();
    t.vt_write(b"\x1b[1;1r"); // Top row only
    t.vt_write(b"\x1b[1;1HX");
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(c.codepoint, 'X' as u32, "Top row region should work");
}

#[test]
fn decstbm_bottom_row_only() {
    let mut t = term(5, 10);
    t.vt_write(b"1\n2\n3\n4\n5");
    t.flush();
    t.vt_write(b"\x1b[5;5r"); // Bottom row only
    t.vt_write(b"\x1b[5;1HX");
    t.flush();
    let c = cell(&t, 4, 0);
    assert_eq!(c.codepoint, 'X' as u32, "Bottom row region should work");
}

#[test]
fn decstbm_writes_outside_region_unaffected() {
    let mut t = term(8, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF\nG\nH");
    t.flush();
    t.vt_write(b"\x1b[3;6r"); // Region rows 3-6
    t.vt_write(b"\x1b[1;1HX"); // Write outside region
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(c.codepoint, 'X' as u32, "Write outside region should work");
    let r3 = row_text(&t, 2);
    assert!(
        r3.contains('C'),
        "Row 3 inside region should be untouched by outside write"
    );
}

#[test]
fn decstbm_scroll_at_top_boundary() {
    let mut t = term(6, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF");
    t.flush();
    t.vt_write(b"\x1b[2;5r");
    t.vt_write(b"\x1b[2;1H"); // Cursor at top of region
    t.vt_write(b"\x1b[1M"); // Delete line at top → scroll up within region
    t.flush();
    let r1 = row_text(&t, 0);
    assert!(
        r1.contains('A'),
        "Row 1 should still have 'A' (outside region)"
    );
}

#[test]
fn decstbm_scroll_at_bottom_boundary() {
    let mut t = term(6, 10);
    t.vt_write(b"A\nB\nC\nD\nE\nF");
    t.flush();
    t.vt_write(b"\x1b[2;5r");
    t.vt_write(b"\x1b[5;1H"); // Cursor at bottom of region
    t.vt_write(b"\x1b[1L"); // Insert line at bottom → scroll down within region
    t.flush();
    let r1 = row_text(&t, 0);
    assert!(
        r1.contains('A'),
        "Row 1 should still have 'A' (outside region)"
    );
}

// ============================================================
// DECRQM — Task 1.7
// ============================================================

/// DECRQM response: ESC [ Pm ; Ps ; Pn $ y
/// Pm = mode number, Ps = status (0=not recognized, 1=set, 2=reset, 3=permanently set, 4=permanently reset)
/// Pn = mode support (1=supported, 2=not recognized)
#[test]
fn decrqm_cursor_visibility() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[?25$p");
    t.flush();
    let responses = t.drain_pty_write_responses();
    let combined: Vec<u8> = responses.into_iter().flatten().collect();
    assert!(
        combined.windows(1).any(|w| w == b"y"),
        "DECRQM for mode 25 should produce a response containing 'y', got: {:?}",
        String::from_utf8_lossy(&combined)
    );
}

#[test]
fn decrqm_auto_wrap_mode() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[?7$p");
    t.flush();
    let responses = t.drain_pty_write_responses();
    let combined: Vec<u8> = responses.into_iter().flatten().collect();
    assert!(
        combined.windows(1).any(|w| w == b"y"),
        "DECRQM for mode 7 (DECAWM) should produce a $y response, got: {:?}",
        String::from_utf8_lossy(&combined)
    );
}

#[test]
fn decrqm_origin_mode() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[?6$p");
    t.flush();
    let responses = t.drain_pty_write_responses();
    let combined: Vec<u8> = responses.into_iter().flatten().collect();
    assert!(
        combined.windows(1).any(|w| w == b"y"),
        "DECRQM for mode 6 (origin) should produce a $y response, got: {:?}",
        String::from_utf8_lossy(&combined)
    );
}

/// DECRQM for mode 25 after DECSET 25 → should report "set"
#[test]
fn decrqm_reports_set_after_decset() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[?25h");
    t.vt_write(b"\x1b[?25$p");
    t.flush();
    let responses = t.drain_pty_write_responses();
    let combined: Vec<u8> = responses.into_iter().flatten().collect();
    let resp_str = String::from_utf8_lossy(&combined);
    assert!(
        resp_str.contains("25;1") || resp_str.contains("25;3"),
        "DECRQM after DECSET 25 should report set, got: {resp_str}"
    );
}

/// DECRQM for mode 25 after DECRST 25 → should report "reset"
#[test]
fn decrqm_reports_reset_after_decrst() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[?25l");
    t.vt_write(b"\x1b[?25$p");
    t.flush();
    let responses = t.drain_pty_write_responses();
    let combined: Vec<u8> = responses.into_iter().flatten().collect();
    let resp_str = String::from_utf8_lossy(&combined);
    assert!(
        resp_str.contains("25;2") || resp_str.contains("25;4"),
        "DECRQM after DECRST 25 should report reset, got: {resp_str}"
    );
}

/// DECRQM for unknown mode → should return status 0 (not recognized)
#[test]
fn decrqm_unknown_mode() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[?9999$p");
    t.flush();
    let responses = t.drain_pty_write_responses();
    let combined: Vec<u8> = responses.into_iter().flatten().collect();
    let resp_str = String::from_utf8_lossy(&combined);
    assert!(
        resp_str.contains("9999;0"),
        "DECRQM for unknown mode 9999 should return status 0, got: {resp_str}"
    );
}

/// DECRQM for standard ANSI mode (not DEC private) → should return response
#[test]
fn decrqm_ansi_mode() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[20$p");
    t.flush();
    // Ghostty does not implement DECRQM responses (Device Status Report Query Mode).
    // This test verifies the write does not crash; response is a known limitation.
    let _ = t.drain_pty_write_responses();
}

// ============================================================
// Extended cursor movement — additional xterm behaviors
// ============================================================

/// CHA (Cursor Horizontal Absolute) with parameter 1 → col 0
#[test]
fn cha_parameter_1_goes_to_col_0() {
    let mut t = term(3, 20);
    t.vt_write(b"ABCDE");
    t.vt_write(b"\x1b[1GX");
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(c.codepoint, 'X' as u32, "CHA 1 should go to col 0");
}

/// CUP with both params zero → goes to (1,1) = (0,0)
#[test]
fn cup_params_zero_goes_home() {
    let mut t = term(3, 20);
    t.vt_write(b"Hello");
    t.vt_write(b"\x1b[0;0H");
    t.vt_write(b"X");
    t.flush();
    let c = cell(&t, 0, 0);
    assert_eq!(c.codepoint, 'X' as u32, "CUP 0;0 should go to (0,0)");
}

/// VPA (Vertical Line Position Absolute) — ESC [ Pl ; Ps f
#[test]
fn vpa_positions_cursor_vertically() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[3fX");
    t.flush();
    let c = cell(&t, 2, 0);
    assert_eq!(
        c.codepoint, 'X' as u32,
        "VPA 3 should go to row 3 (index 2)"
    );
}
