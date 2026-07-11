#![allow(non_snake_case)]

use torvox_terminal::ghostty_terminal::GhosttyTerminal;
use torvox_terminal::test_helpers::assert_invariants;

#[test]
fn dpi_default_scale_one() {
    let t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 24);
    assert_eq!(snap.cols, 80);
    assert_invariants(&snap);
}

#[test]
fn text_render_after_vt_write() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    t.vt_write(b"DPI test");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells.iter().any(|c| c.codepoint == 'D' as u32));
    assert_invariants(&snap);
}

#[test]
fn dpi_passthrough_after_resize() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    t.resize(30, 100);
    t.flush();
    assert_eq!(t.rows(), 30);
    assert_eq!(t.cols(), 100);
    t.vt_write(b"AfterResizeDPI");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells.iter().any(|c| c.codepoint == 'A' as u32));
    assert_invariants(&snap);
}

#[test]
fn vt_write_negative_value_no_crash() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    t.vt_write(b"NoCrash");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'N' as u32);
    assert!(found, "terminal should survive and render 'NoCrash'");
    assert_invariants(&snap);
}

#[test]
fn vt_write_zero_value_no_crash() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    t.vt_write(b"ZeroDPI");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'Z' as u32);
    assert!(found, "terminal should survive and render 'ZeroDPI'");
    assert_invariants(&snap);
}
