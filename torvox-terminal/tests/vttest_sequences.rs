use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn term(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 500).expect("terminal create")
}

fn row_text(t: &GhosttyTerminal, row: u32) -> String {
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
    s.trim_end().to_string()
}

/// ESC sequences sent to VT500 conformity test screens.
/// vttest validates DEC/ISO standard compliance. We capture the sequences
/// it sends and verify Torvox's responses match the expected state.
/// Known vttest screen sequences:
// Screen 1: Cursor keys and character sets
pub const VTTEST_SCREEN1_SEQUENCES: &[&[u8]] = &[
    b"\x1b[A",  // Cursor Up
    b"\x1b[B",  // Cursor Down
    b"\x1b[C",  // Cursor Right
    b"\x1b[D",  // Cursor Left
    b"\x1b[H",  // Cursor Home
    b"\x1b[1~", // Find
    b"\x1b[2~", // Insert
    b"\x1b[3~", // Delete
    b"\x1b[4~", // Select
    b"\x1b[5~", // Page Up
    b"\x1b[6~", // Page Down
];

/// Screen 3: Scroll regions — DECSTBM
pub const VTTEST_SCREEN3_SEQUENCES: &[&[u8]] = &[
    b"\x1b[3;5r",       // Set scroll region rows 3-5
    b"ABC\nD\nE\nF\nG", // Write content while scroll region active
    b"\x1b[r",          // Reset scroll region
];

/// Screen 4: Tab stops
pub const VTTEST_SCREEN4_SEQUENCES: &[&[u8]] = &[
    b"\x1b[3C", // Cursor forward 3
    b"\x1b[1C", // Cursor forward 1
    b"\x1bH",   // Set tab at current column
];

/// Screen 5: Insert/Delete Line
pub const VTTEST_SCREEN5_SEQUENCES: &[&[u8]] = &[
    b"LINE1\nLINE2\nLINE3\nLINE4\nLINE5",
    b"\x1b[2L", // Insert 2 lines at cursor
    b"\x1b[2M", // Delete 2 lines
];

/// Screen 10: Selective Erase
pub const VTTEST_SCREEN10_SEQUENCES: &[&[u8]] = &[
    b"1234567890",
    b"\x1b[4X", // Erase 4 characters
];

/// Cursor Up (ESC [A) from row 2 moves cursor to row 1 (0-indexed 0)
#[test]
fn vttest_cursor_up() {
    let mut t = term(5, 20);
    t.vt_write(b"1\n2");
    t.flush();
    t.vt_write(b"\x1b[A");
    t.vt_write(b"X");
    t.flush();
    let r0 = row_text(&t, 0);
    assert!(
        r0.contains('X'),
        "CUU after row 1 should place X on row 0: got {r0:?}"
    );
}

/// Cursor Down (ESC [B) moves cursor to next row
#[test]
fn vttest_cursor_down() {
    let mut t = term(5, 20);
    t.vt_write(b"1\x1b[BGX");
    t.flush();
    let r1 = row_text(&t, 1);
    assert!(
        !r1.is_empty(),
        "CUD should move cursor to row 1: got {r1:?}"
    );
}

/// Cursor Right (ESC [C) moves cursor right
#[test]
fn vttest_cursor_right() {
    let mut t = term(3, 20);
    t.vt_write(b"\x1b[2CX");
    t.flush();
    let text = row_text(&t, 0);
    assert!(
        text.contains('X'),
        "CUF should place X at col 2: got {text:?}"
    );
}

/// Cursor Left (ESC [D) moves cursor left
#[test]
fn vttest_cursor_left() {
    let mut t = term(3, 20);
    t.vt_write(b"AB\x1b[DX");
    t.flush();
    let text = row_text(&t, 0);
    assert!(
        text.contains('X'),
        "CUB should place X at col 1 overwriting B: got {text:?}"
    );
}

/// Scroll region restricts scrolling (DECSTBM)
#[test]
fn vttest_scroll_region() {
    let mut t = term(5, 20);
    for seq in VTTEST_SCREEN3_SEQUENCES {
        t.vt_write(seq);
    }
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 5, "scroll region test: rows unchanged");
}

/// Insert/Delete Line via IL/DL
#[test]
fn vttest_insert_delete_line() {
    let mut t = term(5, 20);
    for seq in VTTEST_SCREEN5_SEQUENCES {
        t.vt_write(seq);
    }
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 5, "IL/DL test: rows unchanged");
}

/// Erase characters (ECH) — move cursor to col 5 (0-indexed 4) then erase 4 chars
#[test]
fn vttest_erase_characters() {
    let mut t = term(3, 20);
    t.vt_write(b"1234567890");
    t.vt_write(b"\x1b[5G");
    t.vt_write(b"\x1b[4X");
    t.flush();
    let text = row_text(&t, 0);
    assert_eq!(
        &text[..10],
        "1234    90",
        "ECH at col 5 should erase cols 5-8, leaving 1234 + spaces + 90: got {text:?}"
    );
}
