//! Torvox-only VT backend tests.
//!
//! These tests validate GhosttyTerminal behavior as used by Torvox.
//! They are named `torvox_only_*` to clearly indicate they test Torvox's
//! specific backend, not a cross-backend conformance suite.

use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn term(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 500).expect("terminal create")
}

fn get_line(t: &GhosttyTerminal, row: u32) -> String {
    let snap = t.take_snapshot();
    let mut s = String::new();
    for c in 0..snap.cols {
        let idx = (row * snap.cols + c) as usize;
        if idx < snap.cells.len() {
            let cell = &snap.cells[idx];
            if cell.codepoint == 0 {
                s.push(' ');
            } else if let Some(ch) = char::from_u32(cell.codepoint) {
                s.push(ch);
            }
        }
    }
    s
}

fn get_line_padded(t: &GhosttyTerminal, row: u32, width: u32) -> String {
    let snap = t.take_snapshot();
    let mut s = String::new();
    for c in 0..width {
        let idx = (row * snap.cols + c) as usize;
        if idx < snap.cells.len() {
            let cell = &snap.cells[idx];
            if cell.codepoint == 0 {
                s.push(' ');
            } else if let Some(ch) = char::from_u32(cell.codepoint) {
                s.push(ch);
            }
        }
    }
    s
}

#[test]
fn torvox_only_simple_text() {
    let mut t = term(3, 20);
    t.vt_write(b"Hello, world!");
    t.flush();
    let text = get_line(&t, 0);
    assert!(text.starts_with("Hello, world!"));
}

#[test]
fn torvox_only_cursor_positioning_cup() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[3;5HX");
    t.flush();
    let snap = t.take_snapshot();
    let idx = (2 * snap.cols + 4) as usize;
    if idx < snap.cells.len() {
        let ch = char::from_u32(snap.cells[idx].codepoint).unwrap_or('?');
        assert_eq!(ch, 'X', "CUP 3;5 should place X at row 2 col 4");
    }
}

#[test]
fn torvox_only_sgr_31_red_inverts_after_sgr_0() {
    let mut t = term(3, 30);
    t.vt_write(b"\x1b[31mRed\x1b[0mNorm");
    t.flush();
    let text = get_line(&t, 0);
    assert!(text.contains("Red"), "SGR 31 text 'Red' should be visible");
    assert!(text.contains("Norm"), "SGR 0 text 'Norm' should be visible");
}

#[test]
fn torvox_only_sgr_attributes_distinct() {
    let mut t = term(3, 30);
    t.vt_write(b"\x1b[1mBold\x1b[0m \x1b[3mItal\x1b[0m \x1b[4mUndr\x1b[0m");
    t.flush();
    let text = get_line(&t, 0);
    assert!(text.contains("Bold"));
    assert!(text.contains("Ital"));
    assert!(text.contains("Undr"));
}

#[test]
fn torvox_only_lf_crlf_position() {
    let mut t = term(4, 10);
    t.vt_write(b"ABC\n");
    t.flush();
    let snap = t.take_snapshot();
    let ch0 = char::from_u32(snap.cells[0].codepoint).unwrap_or('?');
    assert_eq!(ch0, 'A', "After \\n, A should remain at 0,0");
}

#[test]
fn torvox_only_scroll_region_insert() {
    let mut t = term(5, 20);
    for i in 0..5 {
        let bytes = format!("{}\n", i);
        t.vt_write(bytes.as_bytes());
    }
    t.flush();
    let snap = t.take_snapshot();
    for col_idx in 0..snap.cols.min(20) {
        let cell_idx = (3 * snap.cols + col_idx) as usize;
        if cell_idx < snap.cells.len() {
            let ch = char::from_u32(snap.cells[cell_idx].codepoint);
            if let Some('4') = ch {
                return;
            }
        }
    }
    panic!("row 3 (index 3) should contain '4' after the insert+scroll sequence");
}

#[test]
fn torvox_only_scroll_reverse() {
    let mut t = term(3, 10);
    t.vt_write(b"1\n2\n3");
    t.flush();
    t.vt_write(b"\x1bM");
    t.flush();
    let text0 = get_line_padded(&t, 0, 10);
    assert_eq!(
        text0.trim_end(),
        "1",
        "row 0 should contain '1' after non-top RI"
    );
}

#[test]
fn torvox_only_origin_mode() {
    let mut t = term(5, 20);
    t.vt_write(b"\x1b[?6h\x1b[5;1HX");
    t.flush();
    let snap = t.take_snapshot();
    let idx = (4 * snap.cols) as usize;
    if idx < snap.cells.len() {
        let ch = char::from_u32(snap.cells[idx].codepoint).unwrap_or('?');
        assert_eq!(ch, 'X', "origin mode cursor at row 5");
    }
}

#[test]
fn torvox_only_tab_stops() {
    let mut t = term(3, 30);
    t.vt_write(b"1\t2");
    t.flush();
    let text = get_line_padded(&t, 0, 30);
    assert_eq!(text.chars().next().unwrap(), '1');
    let second = text.chars().position(|c| c == '2').unwrap_or(99);
    assert_eq!(second, 8, "tab should advance to column 8");
}
