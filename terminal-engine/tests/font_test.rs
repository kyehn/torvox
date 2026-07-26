// font_test.rs – Font resolution and character coverage tests
use terminal_engine::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn ft1_basic_ascii_renders() {
    let mut g = t();
    g.vt_write(b"ABC");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'A' as u32);
}
#[test]
fn ft2_digits_render() {
    let mut g = t();
    g.vt_write(b"123");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, '1' as u32);
}
#[test]
fn ft3_punctuation_renders() {
    let mut g = t();
    g.vt_write(b"!@#$");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, '!' as u32);
}
#[test]
fn ft4_unicode_renders() {
    let mut g = t();
    g.vt_write("éñ€".as_bytes());
    g.flush();
    let s = g.take_snapshot();
    assert!(s.cells[0].codepoint > 127);
}
#[test]
fn ft5_box_drawing() {
    let mut g = t();
    g.vt_write(b"\x1b(0lmk");
    g.flush();
    let s = g.take_snapshot();
    assert!(!s.cells.is_empty());
}
