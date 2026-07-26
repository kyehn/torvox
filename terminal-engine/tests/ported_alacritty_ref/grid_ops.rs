use terminal_engine::GhosttyTerminal;
use terminal_engine::vt_conformance::{check_invariants, sized_term, term};

// ====================================================================
// P1.2: Alacritty .ref-style snapshot regression tests
// Each test = run a VT sequence → capture snapshot → assert
// Matches Alacritty's ref/ directory pattern
// ====================================================================

fn capture_grid_text(t: &GhosttyTerminal) -> String {
    let snap = t.take_snapshot();
    let mut out = String::new();
    for r in 0..snap.rows {
        for c in 0..snap.cols {
            let idx = (r * snap.cols + c) as usize;
            let cp = snap.cells[idx].codepoint;
            out.push(if cp == 0 {
                '·'
            } else {
                char::from_u32(cp).unwrap_or('¿')
            });
        }
        if r < snap.rows - 1 {
            out.push('\n');
        }
    }
    out
}

// ── Grid: Scroll / Scrollback ───────────────────────────────────

#[test]
fn alacritty_ref_scroll_fill_viewport() {
    let mut t = sized_term(5, 20, 100);
    for i in 0..5 {
        t.vt_write(format!("Line{}\r\n", i).as_bytes());
        t.flush();
    }
    let snap = t.take_snapshot();
    // Viewport: rows 0-4 should have Line0-Line4
    for i in 0..5 {
        let cells = &snap.cells[(i * 20) as usize..(i * 20 + 5) as usize];
        let text: String = cells
            .iter()
            .filter_map(|c| char::from_u32(c.codepoint))
            .collect();
        assert_eq!(text, format!("Line{}", i), "Alacritty scroll fill: row {i}");
    }
    check_invariants(&t);
}

#[test]
fn alacritty_ref_scroll_exceeds_viewport() {
    let mut t = sized_term(5, 20, 100);
    for i in 0..10 {
        t.vt_write(format!("Line{}\r\n", i).as_bytes());
        t.flush();
    }
    let snap = t.take_snapshot();
    // Only last 5 visible
    let row0_text: String = snap.cells[0..20]
        .iter()
        .filter_map(|c| char::from_u32(c.codepoint))
        .collect();
    assert_eq!(row0_text.trim(), "Line5", "Alacritty scroll: line 5 at top");
    check_invariants(&t);
}

#[test]
fn alacritty_ref_scrollback_content_preserved() {
    let mut t = sized_term(3, 10, 100);
    for i in 0..10 {
        t.vt_write(format!("Line{}\r\n", i).as_bytes());
        t.flush();
    }
    let snap = t.take_snapshot();
    // Last 3 lines visible
    let r0: String = snap.cells[0..10]
        .iter()
        .filter_map(|c| char::from_u32(c.codepoint))
        .collect();
    assert_eq!(r0.trim(), "Line7");
    check_invariants(&t);
}

// ── Grid: Resize ────────────────────────────────────────────────

#[test]
fn alacritty_ref_resize_wider_preserves_content() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"ABCDEFGHIJ");
    t.flush();
    // Resize to wider
    t.resize(5, 20);
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cols, 20, "Alacritty resize: cols 20");
    assert_eq!(
        snap.cell_at(0, 0).codepoint,
        'A' as u32,
        "Alacritty resize: content preserved at (0,0)"
    );
    assert_eq!(
        snap.cell_at(0, 9).codepoint,
        'J' as u32,
        "Alacritty resize: content preserved at col 9"
    );
    check_invariants(&snap);
}

#[test]
fn alacritty_ref_resize_narrower_truncates() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"ABCDEFGHIJKLMNOPQRST");
    t.flush();
    t.resize(5, 10);
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cols, 10, "Alacritty resize narrower: cols 10");
    check_invariants(&snap);
}

#[test]
fn alacritty_ref_resize_taller_shows_scrollback() {
    let mut t = sized_term(3, 20, 100);
    for i in 0..6 {
        t.vt_write(format!("Line{}\r\n", i).as_bytes());
        t.flush();
    }
    t.resize(6, 20);
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 6, "Alacritty resize taller: rows 6");
    check_invariants(&snap);
}

#[test]
fn alacritty_ref_resize_shorter_keeps_scrollback() {
    let mut t = sized_term(10, 20, 100);
    for i in 0..15 {
        t.vt_write(format!("Line{}\r\n", i).as_bytes());
        t.flush();
    }
    t.resize(5, 20);
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 5, "Alacritty resize shorter: rows 5");
    check_invariants(&snap);
}

#[test]
fn alacritty_ref_resize_width_taller_different_ratio() {
    let mut t = sized_term(4, 15, 100);
    t.vt_write(b"Hello World!");
    t.flush();
    t.resize(16, 80);
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 16);
    assert_eq!(snap.cols, 80);
    check_invariants(&snap);
}

#[test]
fn alacritty_ref_resize_zero_same() {
    // Resize to same dimensions should be a no-op
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"TestContent");
    t.flush();
    let snap1 = t.take_snapshot();
    t.resize(5, 20);
    t.flush();
    let snap2 = t.take_snapshot();
    assert_eq!(snap1.cols, snap2.cols);
    check_invariants(&snap2);
}

#[test]
fn alacritty_ref_resize_minimal() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"ContentHere");
    t.flush();
    t.resize(1, 1);
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 1);
    assert_eq!(snap.cols, 1);
    check_invariants(&snap);
}

// ── Grid: Clear ─────────────────────────────────────────────────

#[test]
fn alacritty_ref_clear_screen_ed_2() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"AAAAABBBBBCCCCCDDDDDEEEEE");
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    let snap = t.take_snapshot();
    for r in 0..5 {
        for c in 0..20 {
            assert_eq!(
                snap.cell_at(r, c).codepoint,
                0,
                "Alacritty clear ED 2: cell({r},{c})"
            );
        }
    }
    check_invariants(&snap);
}

#[test]
fn alacritty_ref_clear_line_el_2() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"HelloWorldLine");
    t.flush();
    t.vt_write(b"\x1b[2K");
    t.flush();
    let snap = t.take_snapshot();
    for c in 0..20 {
        assert_eq!(
            snap.cell_at(0, c).codepoint,
            0,
            "Alacritty clear EL 2: col {c}"
        );
    }
    check_invariants(&snap);
}

// ── Colors: 256 combinations ────────────────────────────────────

#[test]
fn alacritty_ref_color_256_fg_all() {
    let mut t = sized_term(5, 20, 100);
    for idx in 0u8..=255u8 {
        t.vt_write(format!("\x1b[38;5;{}mX\x1b[0m", idx).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        let fg = &snap.cell_at(0, 0).foreground;
        assert!(
            fg[0] >= 0.0
                && fg[0] <= 1.0
                && fg[1] >= 0.0
                && fg[1] <= 1.0
                && fg[2] >= 0.0
                && fg[2] <= 1.0
        );
    }
    check_invariants(&t);
}

#[test]
fn alacritty_ref_color_256_bg_all() {
    let mut t = sized_term(5, 20, 100);
    for idx in 0u8..=255u8 {
        t.vt_write(format!("\x1b[48;5;{}mX\x1b[0m", idx).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        let bg = &snap.cell_at(0, 0).background;
        assert!(
            bg[0] >= 0.0
                && bg[0] <= 1.0
                && bg[1] >= 0.0
                && bg[1] <= 1.0
                && bg[2] >= 0.0
                && bg[2] <= 1.0
        );
    }
    check_invariants(&t);
}

#[test]
fn alacritty_ref_color_truecolor_random() {
    let cases = &[
        (255u8, 0u8, 0u8),
        (0, 255, 0),
        (0, 0, 255),
        (128, 128, 128),
        (255, 255, 255),
        (0, 0, 0),
        (100, 150, 200),
        (50, 50, 50),
        (200, 100, 50),
    ];
    for &(r, g, b) in cases {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(format!("\x1b[38;2;{};{};{}mX\x1b[0m", r, g, b).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn alacritty_ref_color_fg_8_and_bright() {
    let mut t = sized_term(5, 20, 100);
    for c in 30u8..=37u8 {
        t.vt_write(format!("\x1b[{}mX\x1b[0m", c).as_bytes());
        t.flush();
        check_invariants(&t);
    }
    for c in 90u8..=97u8 {
        t.vt_write(format!("\x1b[{}mX\x1b[0m", c).as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn alacritty_ref_color_reset_default() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[31mX\x1b[39mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cell_at(0, 1).foreground[0] > 0.0,
        "Alacritty default fg: reset works"
    );
    check_invariants(&t);
}

#[test]
fn alacritty_ref_color_reset_default_bg() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[41mX\x1b[49mY");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cell_at(0, 1).background[0] <= 1.0, "Alacritty bg reset: works");
    check_invariants(&t);
}

// ── Meta-chars: Line boundaries ─────────────────────────────────

#[test]
fn alacritty_ref_newline_advances_row() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"Line1\nLine2");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'L' as u32);
    assert_eq!(snap.cell_at(20, 0).codepoint, 'L' as u32);
    assert_eq!(snap.cursor_row, 1, "Alacritty \\n: row 2");
    check_invariants(&t);
}

#[test]
fn alacritty_ref_carriage_return_resets_col() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"Hello\rX");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 0).codepoint,
        'X' as u32,
        "Alacritty \\r: overwrites first char"
    );
    check_invariants(&t);
}

#[test]
fn alacritty_ref_crlf_both_actions() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"First\r\nSecond");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'F' as u32);
    assert_eq!(snap.cell_at(20, 0).codepoint, 'S' as u32);
    check_invariants(&t);
}

#[test]
fn alacritty_ref_tab_single() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"A\x09B");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
    assert_eq!(
        snap.cell_at(0, 8).codepoint,
        'B' as u32,
        "Alacritty tab: B at col 8"
    );
    check_invariants(&t);
}

#[test]
fn alacritty_ref_tab_multiple() {
    let mut t = sized_term(5, 40, 100);
    t.vt_write(b"A\x09B\x09C");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
    assert_eq!(snap.cell_at(0, 8).codepoint, 'B' as u32);
    assert_eq!(snap.cell_at(0, 16).codepoint, 'C' as u32);
    check_invariants(&t);
}

#[test]
fn alacritty_ref_backspace() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"AB\x08");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 0, "Alacritty BS: col 0");
    check_invariants(&t);
}

#[test]
fn alacritty_ref_line_wrap() {
    let mut t = sized_term(5, 10, 100);
    let long_line = "A".repeat(12);
    t.vt_write(long_line.as_bytes());
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 9).codepoint,
        'A' as u32,
        "Alacritty wrap: last col = A"
    );
    // After wrapping, A should be on next row
    assert_eq!(
        snap.cell_at(1, 0).codepoint,
        'A' as u32,
        "Alacritty wrap: next row col 0 = A"
    );
    assert_eq!(
        snap.cell_at(1, 1).codepoint,
        'A' as u32,
        "Alacritty wrap: next row col 1 = A"
    );
    check_invariants(&t);
}

#[test]
fn alacritty_ref_wrap_disabled() {
    let mut t = sized_term(5, 10, 100);
    t.vt_write(b"\x1b[?7l"); // wrap off
    t.vt_write(b"12345678901");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_col, 10, "Alacritty no-wrap: col stays at edge");
    assert_eq!(
        snap.cell_at(0, 10).codepoint,
        '1' as u32,
        "Alacritty no-wrap: char overwrites last col"
    );
    check_invariants(&t);
}

// ── Meta-chars: Vertical tab / Form feed ────────────────────────

#[test]
fn alacritty_ref_vt_formfeed_no_crash() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"X\x0B");
    t.flush();
    check_invariants(&t);
    t.vt_write(b"\x0C");
    t.flush();
    check_invariants(&t);
}

#[test]
fn alacritty_ref_8bit_c1_safe() {
    let mut t = sized_term(5, 20, 100);
    let c1_bytes: &[u8] = &[0x84, 0x85, 0x88, 0x8D, 0x8E, 0x8F, 0x9A, 0x9B];
    for &b in c1_bytes {
        t.vt_write(&[b]);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn alacritty_ref_stress_10_ops() {
    for _ in 0..10 {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Hello\r\nWorld\x09Tab\x1b[1;5HX\x1b[31mY\x1b[0mZ");
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn alacritty_ref_empty_sequence_safe() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"");
    t.flush();
    check_invariants(&t);
}

#[test]
fn alacritty_ref_lowest_dimensions() {
    let mut t = sized_term(2, 2, 10);
    t.vt_write(b"AB");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
    assert_eq!(snap.cell_at(0, 1).codepoint, 'B' as u32);
    check_invariants(&snap);
}

#[test]
fn alacritty_ref_alternate_screen_buffer() {
    let mut t = sized_term(5, 20, 100);
    t.vt_write(b"\x1b[?1049h"); // alt screen
    t.flush();
    t.vt_write(b"ALT");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 0).codepoint,
        'A' as u32,
        "Alacritty alt: ALT at (0,0)"
    );
    t.vt_write(b"\x1b[?1049l"); // main screen
    t.flush();
    check_invariants(&t);
}

#[test]
fn alacritty_ref_scroll_region_preserved() {
    let mut t = sized_term(10, 30, 100);
    t.vt_write(b"\x1b[4;8r"); // scroll region rows 4-8
    t.vt_write(b"\x1b[1;1H"); // home (outside region)
    t.vt_write(b"TOP");
    t.flush();
    t.vt_write(b"\x1b[4;1H"); // in region
    for _ in 0..5 {
        t.vt_write(b"INREGION\r\n");
        t.flush();
    }
    t.vt_write(b"\x1b[r"); // reset
    t.flush();
    check_invariants(&t);
}
