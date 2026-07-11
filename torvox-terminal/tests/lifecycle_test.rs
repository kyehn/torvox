// lifecycle_test.rs – Session start-stop-restart lifecycle
use torvox_terminal::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn lc1_create_ok() {
    let g = t();
    assert_eq!(g.cursor_x(), 0);
}
#[test]
fn lc2_create_write_read() {
    let mut g = t();
    g.vt_write(b"test");
    g.flush();
    assert_eq!(g.cursor_x(), 4);
}
#[test]
fn lc3_create_multiple_instances() {
    let g1 = t();
    let g2 = t();
    assert_eq!(g1.cursor_x(), g2.cursor_x());
}
#[test]
fn lc4_create_flush_empty() {
    let g = t();
    g.flush();
    assert_eq!(g.cursor_x(), 0);
}
#[test]
fn lc5_reset_sgr() {
    let mut g = t();
    g.vt_write(b"\x1b[1;31m\x1bc");
    g.flush();
    let s = g.take_snapshot();
    assert!(!s.cells[0].bold);
}
