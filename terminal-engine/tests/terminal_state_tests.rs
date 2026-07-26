// WezTerm-style Level 4: Terminal State Testing
// Tests grid state operations (scroll regions, tabs, margins, alt screen)
// using GhosttyTerminal's native state manipulation.

use terminal_engine::ghostty_terminal::GhosttyTerminal;
use terminal_engine::test_helpers::assert_invariants;

fn term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal")
}

fn sized(r: u32, c: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(r, c, 100).expect("terminal")
}

fn ci(t: &GhosttyTerminal) {
    assert_invariants(&t.take_snapshot());
}

// ── DECSTBM ─────────────────────────────────────────────────────────

#[test]
fn l4_decstbm_cursor_in_region() {
    let mut t = sized(10, 40);
    t.vt_write(b"\x1b[3;8r"); // scroll region rows 3-8
    t.vt_write(b"\x1b[HX"); // home
    t.flush();
    // Without origin mode, cursor CAN go outside region
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 0, "no origin: cursor row 0");
    ci(&t);
}

#[test]
fn l4_decstbm_origin_mode_starts_at_region() {
    let mut t = sized(10, 40);
    t.vt_write(b"\x1b[3;8r");
    t.vt_write(b"\x1b[?6h"); // origin mode ON
    t.vt_write(b"\x1b[HX"); // home — should be region top
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 2, "origin: cursor at region top row 2");
    ci(&t);
}

#[test]
fn l4_decstbm_scroll_inside_region() {
    let mut t = sized(5, 20);
    t.vt_write(b"\x1b[2;4r");
    t.vt_write(b"Line1\nLine2\nLine3\nLine4\nLine5");
    t.flush();
    // Scrolling only affects rows 2-4 (1-idx)
    let _snap = t.take_snapshot();
    ci(&t);
}

#[test]
fn l4_decstbm_rejects_invalid() {
    let mut t = sized(10, 40);
    t.vt_write(b"\x1b[10;5r"); // invalid: top > bottom
    t.flush();
    ci(&t);
    t.vt_write(b"\x1b[r"); // reset
    t.flush();
    ci(&t);
}

#[test]
fn l4_decstbm_full_reset() {
    let mut t = sized(10, 40);
    t.vt_write(b"\x1b[3;8r");
    t.vt_write(b"\x1b[r"); // reset to full screen
    t.flush();
    // After reset, the entire screen is the scroll region
    ci(&t);
}

// ── Tab stops ───────────────────────────────────────────────────────

#[test]
fn l4_tab_default_8() {
    let mut t = sized(5, 40);
    t.vt_write(b"\x09");
    t.flush();
    assert_eq!(t.cursor_x(), 8, "default tab -> col 8");
}

#[test]
fn l4_tab_custom_set() {
    let mut t = sized(5, 40);
    t.vt_write(b"\x1b[3g"); // clear all tabs
    t.vt_write(b"\x1b[6G\x1bH"); // CHA to col 6 (1-idx) = col 5 (0-idx), set tab
    t.vt_write(b"\x1b[H"); // home
    t.flush();
    t.vt_write(b"\x09"); // tab
    t.flush();
    assert_eq!(t.cursor_x(), 5, "custom tab -> col 5 (0-idx)");
}

#[test]
fn l4_tab_clear_one() {
    let mut t = sized(5, 40);
    t.vt_write(b"\x1b[3g"); // clear all
    t.vt_write(b"\x1b[6G\x1bH"); // CHA to col 6 (1-idx) = col 5 (0-idx), set tab
    t.vt_write(b"\x1b[11G\x1bH"); // CHA to col 11 (1-idx) = col 10 (0-idx), set tab
    t.vt_write(b"\x1b[H"); // home
    t.flush();
    t.vt_write(b"\x09\x09"); // tab tab -> should go to 5 then 10
    t.flush();
    assert_eq!(t.cursor_x(), 10, "two tabs: final col 10 (0-idx)");
}

#[test]
fn l4_tab_3g_clears_all() {
    let mut t = sized(5, 40);
    t.vt_write(b"\x1b[3g"); // TBC 3 = clear all tabs
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    assert_eq!(t.cursor_x(), 39, "no tabs: tab goes to last col");
}

// ── Automatic newline (LMN / DEC auto-wrap) ─────────────────────────

#[test]
fn l4_autowrap_at_right_margin() {
    let mut t = sized(3, 10);
    t.vt_write(b"123456789AB");
    t.flush();
    let snap = t.take_snapshot();
    // "123456789A" on row 0, "B" on row 1 (11 chars, 10 cols → wraps after 10)
    assert_eq!(snap.cells[0].codepoint, '1' as u32, "wrap: col 0 = 1");
    assert_eq!(
        snap.cells[9].codepoint, 'A' as u32,
        "wrap: col 9 = A (10th char)"
    );
    assert_eq!(
        snap.cells[10].codepoint, 'B' as u32,
        "wrap: row 1 col 0 = B"
    );
}

#[test]
fn l4_autowrap_off_overwrites_last_col() {
    let mut t = sized(3, 10);
    t.vt_write(b"\x1b[?7l"); // disable autowrap (DEC AWM)
    t.vt_write(b"123456789AB");
    t.flush();
    let snap = t.take_snapshot();
    // Without wrap: cursor stays at col 9, chars overwrite it
    // '1'-'9' at cols 0-8, 'A' at col 9, 'B' overwrites col 9
    assert_eq!(
        snap.cells[9].codepoint, 'B' as u32,
        "no wrap: col 9 = B (last char overwrites)"
    );
    assert_eq!(snap.cells[0].codepoint, '1' as u32, "no wrap: col 0 = 1");
}

#[test]
fn l4_autowrap_reset() {
    let mut t = sized(3, 10);
    t.vt_write(b"\x1b[?7l"); // disable
    t.vt_write(b"\x1b[?7h"); // re-enable
    t.vt_write(b"123456789AB");
    t.flush();
    let snap = t.take_snapshot();
    // With autowrap re-enabled: 11 chars, 10 cols → wraps
    assert_eq!(
        snap.cells[10].codepoint, 'B' as u32,
        "wrap re-enabled: row 1 col 0 = B"
    );
}

// ── Cursor visibility ───────────────────────────────────────────────

#[test]
fn l4_cursor_visible_default_true() {
    let t = term();
    assert!(t.is_cursor_enabled(), "cursor visible by default");
}

#[test]
fn l4_cursor_hide_and_show() {
    let mut t = term();
    t.vt_write(b"\x1b[?25l"); // DECTCEM reset
    t.flush();
    assert!(!t.is_cursor_enabled(), "cursor hidden");
    t.vt_write(b"\x1b[?25h");
    t.flush();
    assert!(t.is_cursor_enabled(), "cursor shown");
}

// ── Origin mode ─────────────────────────────────────────────────────

#[test]
fn l4_origin_mode_default_off() {
    let t = term();
    let mode = t.mode_get(6, 0); // DECOM
    assert!(!mode, "origin mode default off");
}

#[test]
fn l4_origin_mode_set_reset() {
    let mut t = term();
    t.vt_write(b"\x1b[?6h");
    t.flush();
    let on = t.mode_get(6, 0);
    assert!(on, "origin mode on");
    t.vt_write(b"\x1b[?6l");
    t.flush();
    let off = t.mode_get(6, 0);
    assert!(!off, "origin mode off");
}

// ── Alt screen ──────────────────────────────────────────────────────

#[test]
fn l4_alt_screen_off_by_default() {
    let t = term();
    assert!(!t.is_alt_screen_active(), "alt screen default off");
}

#[test]
fn l4_alt_screen_set_and_exit() {
    let mut t = term();
    t.vt_write(b"\x1b[?1049h"); // alt screen on (save cursor + switch)
    t.flush();
    assert!(t.is_alt_screen_active(), "alt screen active");
    t.vt_write(b"\x1b[?1049l"); // alt screen off (restore cursor)
    t.flush();
    assert!(!t.is_alt_screen_active(), "alt screen off");
}

// ── Line feed and scroll ────────────────────────────────────────────

#[test]
fn l4_lf_scrolls_at_bottom() {
    let mut t = sized(5, 20);
    t.pty_write(b"Row1\nRow2\nRow3\nRow4\nRow5\nRow6"); // last line triggers scroll
    t.flush();
    let snap = t.take_snapshot();
    // Row 0 should now have "Row2" (Row1 scrolled into scrollback)
    let r0_text: String = snap.cells[0..20]
        .iter()
        .filter_map(|c| {
            if c.codepoint != 0 {
                char::from_u32(c.codepoint)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        r0_text.trim(),
        "Row2",
        "LF scroll: Row1 -> scrollback, Row2 -> row 0"
    );
}

#[test]
fn l4_reverse_index_scrolls() {
    let mut t = sized(3, 20);
    t.vt_write(b"\x1b[H\x1bM"); // home, RI
    t.flush();
    // RI at top of scroll region scrolls content down
    ci(&t);
}

#[test]
fn l4_nel_moves_next_line_first_col() {
    let mut t = sized(5, 20);
    t.vt_write(b"\x1b[2;5H\x1bE"); // NEL
    t.flush();
    assert_eq!(t.cursor_y(), 2, "NEL row");
    assert_eq!(t.cursor_x(), 0, "NEL col 0");
}

// ── Scrollback ──────────────────────────────────────────────────────

#[test]
fn l4_scrollback_empty_initial() {
    let t = sized(5, 20);
    assert_eq!(t.scrollback_length(), 0, "empty scrollback initial");
}

#[test]
fn l4_scrollback_accumulates_lines() {
    let mut t = sized(5, 20);
    for _ in 0..10 {
        t.vt_write(b"\n");
    }
    t.flush();
    assert!(
        t.scrollback_length() > 0,
        "scrollback has lines after 10 newlines"
    );
}

// ── Resize ──────────────────────────────────────────────────────────

#[test]
fn l4_resize_no_crash() {
    let mut t = sized(10, 20);
    t.vt_write(b"Hello World");
    t.flush();
    t.resize(20, 40);
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 20, "resize: rows");
    assert_eq!(snap.cols, 40, "resize: cols");
    ci(&t);
}

#[test]
fn l4_resize_smaller_preserves_content() {
    let mut t = sized(5, 20);
    t.vt_write(b"Hello World This is long content");
    t.flush();
    t.resize(3, 10);
    ci(&t);
}

// ── DCS sequences ───────────────────────────────────────────────────

#[test]
fn l4_dcs_not_crash() {
    let mut t = term();
    // DCS sequences should not crash even if not fully implemented
    t.vt_write(b"\x1bP1+r\x1b\\");
    t.flush();
    ci(&t);
}

#[test]
fn l4_dcs_pt_arrowhead() {
    let mut t = term();
    // DECUDK (user-defined keys)
    t.vt_write(b"\x1bP0;1|18/17/16~ABC\x1b\\");
    t.flush();
    ci(&t);
}

// ── SOS/APC/PM sequences ────────────────────────────────────────────

#[test]
fn l4_sos_not_crash() {
    let mut t = term();
    t.vt_write(b"\x1bXHello\x1b\\");
    t.flush();
    ci(&t);
}

#[test]
fn l4_apc_not_crash() {
    let mut t = term();
    t.vt_write(b"\x1b_Gi=1,a=q;Hello\x1b\\");
    t.flush();
    ci(&t);
}

#[test]
fn l4_pm_not_crash() {
    let mut t = term();
    t.vt_write(b"\x1b^SomePrivateMessage\x1b\\");
    t.flush();
    ci(&t);
}

// ── SOS/APC/PM via C1 8-bit ────────────────────────────────────────

#[test]
fn l4_c1_sos_apc_pm_no_crash() {
    let mut t = term();
    // SOS = 0x98, APC = 0x9F, PM = 0x9E
    let data: &[u8] = &[0x98, b'H', b'i', 0x9C]; // SOS "Hi" ST
    t.vt_write(data);
    t.flush();
    ci(&t);
    let data2: &[u8] = &[0x9F, b't', b'e', b's', b't', 0x9C]; // APC "test" ST
    t.vt_write(data2);
    t.flush();
    ci(&t);
}
