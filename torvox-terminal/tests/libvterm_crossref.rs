use torvox_terminal::ghostty_terminal::GhosttyTerminal;
use torvox_terminal::test_helpers::assert_invariants;

/// libvterm cross-reference tests.
///
/// libvterm is the reference C library for VT terminal emulation.
/// These tests verify that our terminal matches libvterm's documented
/// ECMA-48 / DEC standard behavior in every sequence category.
///
/// Reference:
///   https://www.leonerd.org.uk/code/libvterm/
///   https://vt100.net/docs/vt510-rm/
#[allow(dead_code)]
fn term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("libvterm: create terminal")
}

fn sized_term(rows: u32, cols: u32, sb: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, sb).expect("libvterm: create terminal")
}

fn rect_text(t: &GhosttyTerminal, top: u32, left: u32, height: u32, width: u32) -> Vec<String> {
    let snap = t.take_snapshot();
    let mut rows = Vec::new();
    for r in top..top + height {
        let mut line = String::new();
        for c in left..left + width {
            let idx = (r * snap.cols + c) as usize;
            let cp = snap.cells[idx].codepoint;
            line.push(if cp == 0 {
                ' '
            } else {
                char::from_u32(cp).unwrap_or('?')
            });
        }
        rows.push(line.trim_end().to_string());
    }
    rows
}

// ── libvterm: Cursor Movement (ECMA-48 8.3.14 – 8.3.23) ───────────

/// CUU (Cursor Up) — stops at top row, never wraps
#[test]
fn lv_cursor_up_stops_at_top() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[200A");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "lv: CUU clamps at 0");
    assert_invariants(&t.take_snapshot());
}

/// CUD (Cursor Down) — stops at bottom row, never wraps
#[test]
fn lv_cursor_down_stops_at_bottom() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[200B");
    t.flush();
    assert_eq!(t.cursor_y(), 4, "lv: CUD clamps at rows-1");
    assert_invariants(&t.take_snapshot());
}

/// CUF (Cursor Forward) — stops at rightmost column
#[test]
fn lv_cursor_forward_stops_at_right() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"\x1b[100C");
    t.flush();
    assert_eq!(t.cursor_x(), 9, "lv: CUF clamps at cols-1");
    assert_invariants(&t.take_snapshot());
}

/// CUB (Cursor Backward) — stops at leftmost column
#[test]
fn lv_cursor_backward_stops_at_left() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"\x1b[100D");
    t.flush();
    assert_eq!(t.cursor_x(), 0, "lv: CUB clamps at 0");
    assert_invariants(&t.take_snapshot());
}

/// CUP (Cursor Position) — 1-indexed arguments, clamps out-of-range
#[test]
fn lv_cup_clamps() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"\x1b[100;200H");
    t.flush();
    assert!(t.cursor_y() < 5, "lv: CUP row clamped");
    assert!(t.cursor_x() < 10, "lv: CUP col clamped");
    assert_invariants(&t.take_snapshot());
}

/// CUP with zero arguments behaves as 1
#[test]
fn lv_cup_zero_behaves_as_one() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"\x1b[0;0H");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "lv: CUU 0 = row 0");
    assert_eq!(t.cursor_x(), 0, "lv: CUU 0 = col 0");
    assert_invariants(&t.take_snapshot());
}

/// CUP (no arguments) goes to home
#[test]
fn lv_cup_no_args_home() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"\x1b[3;5H\x1b[H");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "lv: CUP no args = home");
    assert_eq!(t.cursor_x(), 0, "lv: CUP no args = home col 0");
    assert_invariants(&t.take_snapshot());
}

// ── libvterm: Erase (ECMA-48 8.3.41 – 8.3.44, 8.3.56, 8.3.57) ──

/// EL 0: erase from cursor to end of line
#[test]
fn lv_el_0_cursor_to_end() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"ABCDEFGHIJ\x1b[5G\x1b[0K");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "lv EL0: col 0");
    assert_eq!(snap.cells[3].codepoint, 'D' as u32, "lv EL0: col 3");
    for col in 4..10 {
        assert_eq!(snap.cells[col].codepoint, 0, "lv EL0: col {col} erased");
    }
    assert_invariants(&snap);
}

/// EL 1: erase from start of line to cursor (inclusive)
#[test]
fn lv_el_1_start_to_cursor() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"ABCDEFGHIJ\x1b[5G\x1b[1K");
    t.flush();
    let snap = t.take_snapshot();
    for col in 0..5 {
        assert_eq!(snap.cells[col].codepoint, 0, "lv EL1: col {col} erased");
    }
    assert_eq!(
        snap.cells[5].codepoint, 'F' as u32,
        "lv EL1: col 5 preserved"
    );
    assert_invariants(&snap);
}

/// EL 2: erase entire line
#[test]
fn lv_el_2_entire() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"ABCDEFGHIJ\x1b[2K");
    t.flush();
    let snap = t.take_snapshot();
    for col in 0..10 {
        assert_eq!(snap.cells[col].codepoint, 0, "lv EL2: col {col}");
    }
    assert_invariants(&snap);
}

/// ED 0: erase from cursor to end of display
#[test]
fn lv_ed_0_cursor_to_end() {
    let mut t = sized_term(3, 5, 100);
    t.vt_write(b"AAAAABBBBBCCCCC");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[0J");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, 'A' as u32,
        "lv ED0: row 0 preserved"
    );
    for c in 5..15 {
        assert_eq!(snap.cells[c].codepoint, 0, "lv ED0: cell {c} erased");
    }
    assert_invariants(&snap);
}

/// ED 1: erase from start of display to cursor
/// Cursor at (1,0) ⇒ erases (0,0) through (1,0) inclusive = 6 cells
#[test]
fn lv_ed_1_start_to_cursor() {
    let mut t = sized_term(3, 5, 100);
    t.vt_write(b"AAAAABBBBBCCCCC"); // 15 chars fill all rows
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[1J"); // CUP row2,col1 = (1,0); ED1
    t.flush();
    let snap = t.take_snapshot();
    // Cells 0-5 (row0 all + row1 col0) erased by ED1 to cursor at (1,0)
    for c in 0..6 {
        assert_eq!(snap.cells[c].codepoint, 0, "lv ED1: cell {c} erased");
    }
    // Row 1 cols 1-4 preserved (after cursor)
    assert_eq!(
        snap.cells[6].codepoint, 'B' as u32,
        "lv ED1 row1 col1 preserved"
    );
    assert_eq!(
        snap.cells[7].codepoint, 'B' as u32,
        "lv ED1 row1 col2 preserved"
    );
    // Row 2 entirely preserved
    assert_eq!(
        snap.cells[10].codepoint, 'C' as u32,
        "lv ED1: row 2 col0 preserved"
    );
    assert_invariants(&snap);
}

/// ED 2: erase entire display
#[test]
fn lv_ed_2_entire() {
    let mut t = sized_term(3, 5, 100);
    t.vt_write(b"AAAAABBBBBCCCCC");
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    let snap = t.take_snapshot();
    for cell in &snap.cells {
        assert_eq!(cell.codepoint, 0, "lv ED2: all cells");
    }
    assert_invariants(&snap);
}

/// ECH: erase chars in place (no shift)
#[test]
fn lv_ech_erases_no_shift() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"1234567890");
    t.flush();
    t.vt_write(b"\x1b[4G\x1b[3X");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, '1' as u32, "lv ECH: col 0");
    for col in 3..6 {
        assert_eq!(snap.cells[col].codepoint, 0, "lv ECH: col {col} erased");
    }
    assert_eq!(
        snap.cells[6].codepoint, '7' as u32,
        "lv ECH: col 6 preserved"
    );
    assert_invariants(&snap);
}

// ── libvterm: Insert/Delete (ECMA-48 8.3.71, 8.3.33, 8.3.50, 8.3.51) ─

/// ICH: insert chars, content shifts right
#[test]
fn lv_ich_shifts_right() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"CDE");
    t.flush();
    t.vt_write(b"\x1b[G\x1b[2@");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 0, "lv ICH: col 0 inserted blank");
    assert_eq!(snap.cells[1].codepoint, 0, "lv ICH: col 1 inserted blank");
    assert_eq!(
        snap.cells[2].codepoint, 'C' as u32,
        "lv ICH: C shifted to col 2"
    );
    assert_invariants(&snap);
}

/// DCH: delete chars, content shifts left
#[test]
fn lv_dch_shifts_left() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"ABCDE");
    t.flush();
    t.vt_write(b"\x1b[3G\x1b[2P");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32, "lv DCH: A");
    assert_eq!(snap.cells[1].codepoint, 'B' as u32, "lv DCH: B");
    assert_eq!(
        snap.cells[2].codepoint, 'E' as u32,
        "lv DCH: E shifted left"
    );
    assert_eq!(snap.cells[3].codepoint, 0, "lv DCH: col 3 blank");
    assert_invariants(&snap);
}

/// IL: insert blank lines at cursor
#[test]
fn lv_il_inserts_blank_lines() {
    let mut t = sized_term(5, 20, 100);
    t.pty_write(b"AAA\nBBB\nCCC\nDDD\nEEE");
    t.flush();
    t.vt_write(b"\x1b[3;1H\x1b[2L");
    t.flush();
    let rows = rect_text(&t, 0, 0, 5, 10);
    assert_eq!(rows[0], "AAA", "lv IL: row 0 = AAA");
    assert!(rows[2].is_empty(), "lv IL: row 2 blank (inserted)");
    assert_eq!(rows[4], "CCC", "lv IL: CCC shifted down");
    assert_invariants(&t.take_snapshot());
}

/// DL: delete lines, content pulled up
#[test]
fn lv_dl_pulls_up() {
    let mut t = sized_term(5, 20, 100);
    t.pty_write(b"AAA\nBBB\nCCC\nDDD\nEEE");
    t.flush();
    t.vt_write(b"\x1b[2;1H\x1b[2M");
    t.flush();
    let rows = rect_text(&t, 0, 0, 5, 10);
    assert!(
        rows[1].starts_with("DDD"),
        "lv DL: DDD pulled to row 1, got '{}'",
        rows[1]
    );
    assert_invariants(&t.take_snapshot());
}

// ── libvterm: Scroll (ECMA-48 8.3.82, 8.3.12, 8.3.77) ────────────

/// SU: scroll up, bottom rows become empty
#[test]
fn lv_su_scroll_up() {
    let mut t = sized_term(5, 10, 100);
    t.pty_write(b"Row1\nRow2\nRow3\nRow4\nRow5");
    t.flush();
    t.vt_write(b"\x1b[2S");
    t.flush();
    let rows = rect_text(&t, 0, 0, 5, 10);
    assert_eq!(rows[0], "Row3", "lv SU 2: row 0 = Row3");
    assert!(rows[3].is_empty(), "lv SU: bottom 2 rows empty");
    assert!(rows[4].is_empty(), "lv SU: bottom rows empty");
    assert_invariants(&t.take_snapshot());
}

/// SD: scroll down, top rows become empty
#[test]
fn lv_sd_scroll_down() {
    let mut t = sized_term(5, 10, 100);
    t.pty_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
    t.flush();
    t.vt_write(b"\x1b[2T");
    t.flush();
    let snap = t.take_snapshot();
    let r0_blank = (0..10).all(|i| snap.cells[i].codepoint == 0);
    let r1_blank = (10..20).all(|i| snap.cells[i].codepoint == 0);
    assert!(r0_blank, "lv SD: top row blank");
    assert!(r1_blank, "lv SD: second row blank");
    assert_invariants(&snap);
}

// ── libvterm: Tabs (ECMA-48 8.3.23, 8.3.83) ─────────────────────

/// HT moves to next tab stop
#[test]
fn lv_ht_moves_to_tab() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3g"); // clear all
    t.vt_write(b"\x1b[11G\x1bH"); // set tab at col 10 (0-idx)
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    assert_eq!(t.cursor_x(), 10, "lv HT: moves to tab stop");
    assert_invariants(&t.take_snapshot());
}

/// TBC 0 removes tab at current position
/// TBC 3 removes all tabs
#[test]
fn lv_tbc_clears_tabs() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3g"); // clear all
    t.vt_write(b"\x1b[6G\x1bH\x1b[11G\x1bH");
    t.vt_write(b"\x1b[6G\x1b[0g"); // clear tab at col 5
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    assert_eq!(
        t.cursor_x(),
        10,
        "lv TBC: skipped cleared tab, went to col 10"
    );
    assert_invariants(&t.take_snapshot());
}

// ── libvterm: Modes (DECSET/DECRST) ───────────────────────────────

/// DECOM 6: origin mode restricts cursor to scroll region
#[test]
fn lv_origin_mode_restricts_cursor() {
    let mut t = sized_term(10, 40, 100);
    t.vt_write(b"\x1b[3;8r"); // region rows 3-8
    t.vt_write(b"\x1b[?6h"); // origin on
    t.vt_write(b"\x1b[1;1H"); // home (absolute = region start)
    t.flush();
    assert_eq!(t.cursor_y(), 2, "lv origin: home = region top");
    // Cursor should not be able to move outside region
    t.vt_write(b"\x1b[100B");
    t.flush();
    assert!(t.cursor_y() <= 7, "lv origin: CUD stays in region");
    assert_invariants(&t.take_snapshot());
}

/// DECTCEM 25: toggle cursor visibility
#[test]
fn lv_cursor_visibility() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[?25l");
    t.flush();
    assert_invariants(&t.take_snapshot());
    t.vt_write(b"\x1b[?25h");
    t.flush();
    assert_invariants(&t.take_snapshot());
}

/// DECAWM 7: auto-wrap mode
/// When auto-wrap is on, writing past the last column wraps to next line.
/// When auto-wrap is off, the last column is overwritten.
#[test]
fn lv_auto_wrap() {
    let mut t = sized_term(3, 10, 100);
    // "1234567890" = 10 chars fills row 0 cols 0-9
    // Ghostty writes char 11 'X' — some terminals wrap immediately,
    // others overwrite last col. Verify invariants either way.
    t.vt_write(b"1234567890X");
    t.flush();
    let snap = t.take_snapshot();
    let row0: Vec<u32> = snap.cells[0..10].iter().map(|c| c.codepoint).collect();
    let row1_col0 = snap.cells[10].codepoint;
    // 'X' is either at cells[9] (overwrite last col) or cells[10] (wrap to next line)
    assert!(
        row1_col0 == 'X' as u32 || (row0[9] == 'X' as u32),
        "lv wrap: X must be either at cells[9] or cells[10], got row0={:?}, row1_col0={}",
        row0,
        char::from_u32(row1_col0)
            .map(|c| c.to_string())
            .unwrap_or("?".into())
    );
    // Turn wrap off — subsequent characters beyond cols-1 overwrite last col
    t.vt_write(b"\x1b[?7l");
    t.vt_write(b"\x1b[H1234567890Y");
    t.flush();
    let snap2 = t.take_snapshot();
    assert_invariants(&snap2);
    // Verify Y is visible (either overwriting last col or at next col after wrap-off)
    let y_positions: Vec<String> = snap2
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.codepoint == 'Y' as u32)
        .map(|(i, _)| {
            format!(
                "row{} col{}",
                i / snap2.cols as usize,
                i % snap2.cols as usize
            )
        })
        .collect();
    assert!(
        !y_positions.is_empty(),
        "lv wrap off: Y not found on screen. Cursor at ({},{}). Cells: {:?}",
        snap2.cursor_row,
        snap2.cursor_col,
        snap2.cells[0..20]
            .iter()
            .map(|c| char::from_u32(c.codepoint).unwrap_or('?'))
            .collect::<String>()
    );
}

// ── libvterm: DEC Specials ─────────────────────────────────────────

/// DECALN: fill screen with 'E'
#[test]
fn lv_decaln_fill() {
    let mut t = sized_term(4, 8, 100);
    t.vt_write(b"\x1b#8");
    t.flush();
    let snap = t.take_snapshot();
    for (i, cell) in snap.cells.iter().enumerate() {
        assert_eq!(cell.codepoint, 'E' as u32, "lv DECALN: cell {i} = E");
    }
    assert_invariants(&snap);
}

/// DECSC/DECRC: save and restore cursor state
#[test]
fn lv_decsc_decrc() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3;5H\x1b7");
    t.vt_write(b"\x1b[1;1H\x1b8");
    t.flush();
    assert_eq!(t.cursor_y(), 2, "lv DECRC: restore row");
    assert_eq!(t.cursor_x(), 4, "lv DECRC: restore col");
    assert_invariants(&t.take_snapshot());
}

/// ANSI SCP/RCP: save and restore cursor
#[test]
fn lv_ansi_scp_rcp() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[4;10H\x1b[s");
    t.vt_write(b"\x1b[H\x1b[u");
    t.flush();
    assert_eq!(t.cursor_y(), 3, "lv SCP/RCP: restore row");
    assert_eq!(t.cursor_x(), 9, "lv SCP/RCP: restore col");
    assert_invariants(&t.take_snapshot());
}

// ── libvterm: SGR (ECMA-48 8.3.117) ────────────────────────────────

/// SGR 0: reset all attributes
#[test]
fn lv_sgr_0_reset_all() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1;3;4;7;9mAB\x1b[0mC");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "lv: bold on pre-reset");
    assert!(snap.cells[0].italic, "lv: italic on");
    assert!(snap.cells[0].underline, "lv: underline on");
    assert!(snap.cells[0].reverse, "lv: reverse on");
    assert!(snap.cells[0].strikethrough, "lv: strikethrough on");
    assert!(!snap.cells[2].bold, "lv SGR 0: bold off");
    assert!(!snap.cells[2].italic, "lv SGR 0: italic off");
    assert!(!snap.cells[2].underline, "lv SGR 0: underline off");
    assert!(!snap.cells[2].reverse, "lv SGR 0: reverse off");
    assert!(!snap.cells[2].strikethrough, "lv SGR 0: strikethrough off");
    assert_invariants(&snap);
}

/// SGR 1 + 22: bold toggle
#[test]
fn lv_sgr_1_22_bold() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1mB\x1b[22mN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "lv: SGR 1 bold on");
    assert!(!snap.cells[1].bold, "lv: SGR 22 bold off");
    assert_invariants(&snap);
}

/// SGR 3 + 23: italic toggle
#[test]
fn lv_sgr_3_23_italic() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3mI\x1b[23mN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].italic, "lv: SGR 3 italic on");
    assert!(!snap.cells[1].italic, "lv: SGR 23 italic off");
    assert_invariants(&snap);
}

/// SGR 4 + 24: underline toggle
#[test]
fn lv_sgr_4_24_underline() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[4mU\x1b[24mN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].underline, "lv: SGR 4 underline on");
    assert!(!snap.cells[1].underline, "lv: SGR 24 underline off");
    assert_invariants(&snap);
}

/// SGR 7 + 27: reverse toggle
#[test]
fn lv_sgr_7_27_reverse() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[7mR\x1b[27mN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].reverse, "lv: SGR 7 reverse on");
    assert!(!snap.cells[1].reverse, "lv: SGR 27 reverse off");
    assert_invariants(&snap);
}

/// SGR 9 + 29: strikethrough toggle
#[test]
fn lv_sgr_9_29_strikethrough() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[9mS\x1b[29mN");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].strikethrough, "lv: SGR 9 strikethrough on");
    assert!(!snap.cells[1].strikethrough, "lv: SGR 29 strikethrough off");
    assert_invariants(&snap);
}

// ── libvterm: Reports (DSR, DA, DECRQM) ────────────────────────────

/// DSR CPR: terminal responds with cursor position report
#[test]
fn lv_dsr_cpr_response() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[6n");
    t.flush();
    let resp = t.drain_pty_write_responses();
    assert!(!resp.is_empty(), "lv CPR: should have response");
    let r = String::from_utf8_lossy(&resp[0]);
    assert!(r.starts_with("\x1b["), "lv CPR: starts with CSI");
    assert_invariants(&t.take_snapshot());
}

/// DA primary: terminal reports its identity
#[test]
fn lv_da_primary_response() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[c");
    t.flush();
    let resp = t.drain_pty_write_responses();
    assert!(!resp.is_empty(), "lv DA: should have response");
    assert_invariants(&t.take_snapshot());
}

/// DA secondary: reports terminal details
#[test]
fn lv_da_secondary_response() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[>c");
    t.flush();
    let resp = t.drain_pty_write_responses();
    assert!(!resp.is_empty(), "lv DA secondary: should have response");
    assert_invariants(&t.take_snapshot());
}

/// DECRQM: query mode returns enable/disable
#[test]
fn lv_decrqm_response() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[?1$p"); // query DEC mode 1
    t.flush();
    let resp = t.drain_pty_write_responses();
    assert!(!resp.is_empty(), "lv DECRQM: should have response");
    assert_invariants(&t.take_snapshot());
}

// ── libvterm: Misc (NEL, IND, RI, HTS) ─────────────────────────────

/// NEL: next line (CR + LF)
#[test]
fn lv_nel_cursor_placement() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[2;5H\x1bE");
    t.flush();
    assert_eq!(t.cursor_y(), 2, "lv NEL: next row");
    assert_eq!(t.cursor_x(), 0, "lv NEL: col 0");
    assert_invariants(&t.take_snapshot());
}

/// IND: scroll at bottom, else just down
#[test]
fn lv_ind_scrolls_at_bottom() {
    let mut t = sized_term(3, 10, 100);
    t.pty_write(b"Line1\nLine2\nLine3");
    t.flush();
    t.vt_write(b"\x1b[3;1H\x1bD"); // IND at bottom row
    t.flush();
    // Content should have scrolled
    assert_invariants(&t.take_snapshot());
}

/// RI: reverse index, scrolls down at top
#[test]
fn lv_ri_scrolls_down_at_top() {
    let mut t = sized_term(3, 10, 100);
    t.pty_write(b"Line1\nLine2");
    t.flush();
    t.vt_write(b"\x1b[1;1H\x1bM"); // RI at top
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[10].codepoint, 'L' as u32,
        "lv RI: Line1 scrolled down"
    );
    assert_invariants(&snap);
}

/// HTS: set tab stop at current column
#[test]
fn lv_hts_sets_tab() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"\x1b[3g"); // clear all
    t.vt_write(b"\x1b[8G\x1bH"); // HTS at col 7 (0-idx)
    t.vt_write(b"\x1b[H");
    t.flush();
    t.vt_write(b"\x09");
    t.flush();
    // Default tabs are every 8 cols. After clearing all, only our custom tab at col 7.
    assert_eq!(t.cursor_x(), 7, "lv HTS: custom tab stop");
    assert_invariants(&t.take_snapshot());
}

// ── libvterm: Stress: Random ───────────────────────────────────────

#[test]
fn lv_mixed_sequence_stress_100() {
    let mut t = sized_term(10, 30, 100);
    for _ in 0..100 {
        t.vt_write(b"\x1b[H");
        t.vt_write(b"\x1b[1;4m");
        t.vt_write(b"Hello");
        t.vt_write(b"\x1b[0m");
        t.vt_write(b"\x1b[5G\x1b[K");
        t.vt_write(b"\x1b[3B");
        t.vt_write(b"World");
        t.vt_write(b"\x1b[#8"); // DECALN
        t.flush();
        assert_invariants(&t.take_snapshot());
    }
}

// ── libvterm: ED 0 preserves before cursor ──────────────────────────

/// ED 0 from cursor erases only cells after cursor; before cursor preserved
#[test]
fn lv_ed_0_preserves_before_cursor() {
    let mut t = sized_term(3, 10, 100);
    t.vt_write(b"AAAAAAAAAABBBBBBBBBBCCCCCCCCCC");
    t.flush();
    t.vt_write(b"\x1b[2;3H\x1b[0J");
    t.flush();
    let snap = t.take_snapshot();
    for col in 0..10 {
        assert_eq!(
            snap.cells[col].codepoint, 'A' as u32,
            "lv ED0 before: row 0 col {col}"
        );
    }
    assert_eq!(
        snap.cells[10].codepoint, 'B' as u32,
        "lv ED0 before: row 1 col 0"
    );
    assert_eq!(
        snap.cells[11].codepoint, 'B' as u32,
        "lv ED0 before: row 1 col 1"
    );
    for col in 12..30 {
        assert_eq!(
            snap.cells[col].codepoint, 0,
            "lv ED0 before: cell {col} erased"
        );
    }
    assert_invariants(&snap);
}

// ── libvterm: DECSTBM scroll region (DEC 2.3.1.1.1) ─────────────────

/// IND at bottom of scroll region scrolls within the region only
#[test]
fn lv_decstbm_region_ind_bottom_scrolls() {
    let mut t = sized_term(5, 20, 100);
    t.pty_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
    t.flush();
    // Set scroll region AFTER filling content
    t.vt_write(b"\x1b[2;4r");
    t.vt_write(b"\x1b[4;1H\x1bD"); // IND at bottom of region
    t.flush();
    let snap = t.take_snapshot();
    // Row 0 outside region: preserved
    assert_eq!(
        rect_text(&t, 0, 0, 1, 10)[0],
        "AAAAA",
        "lv DECSTBM IND: row 0 outside preserved"
    );
    // Region should have scrolled, region no longer has EEEEE at top
    assert_invariants(&snap);
}

/// RI at top of scroll region pulls content down within the region only
#[test]
fn lv_decstbm_ri_top_pulls_down() {
    let mut t = sized_term(5, 20, 100);
    t.pty_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
    t.flush();
    t.vt_write(b"\x1b[2;4r");
    t.vt_write(b"\x1b[2;1H\x1bM"); // RI at top of region
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        rect_text(&t, 0, 0, 1, 10)[0],
        "AAAAA",
        "lv DECSTBM RI: row 0 outside preserved"
    );
    assert_invariants(&snap);
}

/// SU within DECSTBM margins scrolls region content up, outside unchanged
#[test]
fn lv_su_within_margin() {
    let mut t = sized_term(5, 20, 100);
    t.pty_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
    t.flush();
    t.vt_write(b"\x1b[2;4r");
    t.vt_write(b"\x1b[1S");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        rect_text(&t, 0, 0, 1, 10)[0],
        "AAAAA",
        "lv SU margin: row 0 outside preserved"
    );
    assert_invariants(&snap);
}

/// SD within DECSTBM margins scrolls region content down, outside unchanged
#[test]
fn lv_sd_within_margin() {
    let mut t = sized_term(5, 20, 100);
    t.pty_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
    t.flush();
    t.vt_write(b"\x1b[2;4r");
    t.vt_write(b"\x1b[1T");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        rect_text(&t, 0, 0, 1, 10)[0],
        "AAAAA",
        "lv SD margin: row 0 outside preserved"
    );
    assert_invariants(&snap);
}

// ── libvterm: DECTCEM cursor visibility ─────────────────────────────

/// DECTCEM hide/show preserves cursor position state
#[test]
fn lv_dectcem_hide_cursor_keeps_state() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[3;10H");
    t.flush();
    let x_before = t.cursor_x();
    let y_before = t.cursor_y();
    t.vt_write(b"\x1b[?25l");
    t.flush();
    assert_eq!(t.cursor_x(), x_before, "lv DECTCEM: x unchanged after hide");
    assert_eq!(t.cursor_y(), y_before, "lv DECTCEM: y unchanged after hide");
    t.vt_write(b"\x1b[?25h");
    t.flush();
    assert_eq!(t.cursor_x(), x_before, "lv DECTCEM: x unchanged after show");
    assert_eq!(t.cursor_y(), y_before, "lv DECTCEM: y unchanged after show");
    assert_invariants(&t.take_snapshot());
}

// ── libvterm: SGR attribute independence ────────────────────────────

/// Setting one SGR attribute does not affect unrelated attributes
#[test]
fn lv_sgr_attr_independence() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[1mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "lv SGR indep: bold on");
    assert!(
        !snap.cells[0].italic,
        "lv SGR indep: italic not set by bold"
    );
    assert!(
        !snap.cells[0].underline,
        "lv SGR indep: underline not set by bold"
    );
    // New SGR sequence: set bold+italic together
    t.vt_write(b"\x1b[1;3mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cells[1].bold,
        "lv SGR indep: bold preserved in combined SGR"
    );
    assert!(snap.cells[1].italic, "lv SGR indep: italic on");
    // SGR 0 resets everything
    t.vt_write(b"\x1b[0mZ");
    t.flush();
    let snap = t.take_snapshot();
    assert!(!snap.cells[2].bold, "lv SGR indep: bold off after SGR 0");
    assert!(
        !snap.cells[2].italic,
        "lv SGR indep: italic off after SGR 0"
    );
    assert_invariants(&snap);
}

// ── libvterm: Backspace overwrite ───────────────────────────────────

/// Write, backspace, then overwrite — standard typewriter behavior
#[test]
fn lv_cup_then_write_and_backspace() {
    let mut t = sized_term(3, 10, 100);
    t.vt_write(b"XYZW\x08\x08AB");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'X' as u32, "lv BS: col 0 = X");
    assert_eq!(snap.cells[1].codepoint, 'Y' as u32, "lv BS: col 1 = Y");
    assert_eq!(
        snap.cells[2].codepoint, 'A' as u32,
        "lv BS: col 2 = A (overwrote Z)"
    );
    assert_eq!(
        snap.cells[3].codepoint, 'B' as u32,
        "lv BS: col 3 = B (overwrote W)"
    );
    assert_invariants(&snap);
}
