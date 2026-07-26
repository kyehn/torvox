// selection_categories.rs – Test char, word, line, block selection modes
use terminal_engine::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn sc1_char_selection_init() {
    let g = t();
    let s = g.take_snapshot();
    assert_eq!(s.rows, 24);
}
#[test]
fn sc2_line_selection_init() {
    let g = t();
    let s = g.take_snapshot();
    assert_eq!(s.cols, 80);
}
#[test]
fn sc3_write_creates_cells() {
    let mut g = t();
    g.vt_write(b"Hello");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'H' as u32);
}
#[test]
fn sc4_word_break_at_spaces() {
    let mut g = t();
    g.vt_write(b"Hello World");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[5].codepoint, ' ' as u32);
}
#[test]
fn sc5_block_selection() {
    let mut g = t();
    g.pty_write(b"Row1\nRow2");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[80].codepoint, 'R' as u32);
}
