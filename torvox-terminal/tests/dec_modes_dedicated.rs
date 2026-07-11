// dec_modes_dedicated.rs – Test DECSET/DECRST/DECRPM for modes 1-2026
use torvox_terminal::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn dm1_decset_1() {
    let mut g = t();
    g.vt_write(b"\x1b[?1h");
    g.flush();
    assert!(g.mode_get(1, 0));
}
#[test]
fn dm2_decset_2() {
    let mut g = t();
    g.vt_write(b"\x1b[?2h\x1b[?2l");
    g.flush();
    assert!(!g.mode_get(2, 0));
}
#[test]
fn dm3_decset_3() {
    let mut g = t();
    g.vt_write(b"\x1b[?3h\x1b[?3l");
    g.flush();
    assert!(!g.mode_get(3, 0));
}
#[test]
fn dm4_decset_4() {
    let mut g = t();
    g.vt_write(b"\x1b[?4h");
    g.flush();
    assert!(g.mode_get(4, 0));
}
#[test]
fn dm5_decset_5() {
    let mut g = t();
    g.vt_write(b"\x1b[?5h");
    g.flush();
    assert!(g.mode_get(5, 0));
}
#[test]
fn dm6_decset_6() {
    let mut g = t();
    g.vt_write(b"\x1b[?6h");
    g.flush();
    assert!(g.mode_get(6, 0));
}
#[test]
fn dm7_decset_7() {
    let mut g = t();
    g.vt_write(b"\x1b[?7h");
    g.flush();
    assert!(g.mode_get(7, 0));
}
#[test]
fn dm8_decset_12() {
    let mut g = t();
    g.vt_write(b"\x1b[?12h");
    g.flush();
    assert!(g.mode_get(12, 0));
}
#[test]
fn dm9_decset_25() {
    let mut g = t();
    g.vt_write(b"\x1b[?25h\x1b[?25l");
    g.flush();
    assert!(!g.cursor_visible());
}
#[test]
fn dm10_decrst_6() {
    let mut g = t();
    g.vt_write(b"\x1b[?6h\x1b[?6l");
    g.flush();
    assert!(!g.mode_get(6, 0));
}
