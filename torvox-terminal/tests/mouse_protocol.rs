// mouse_protocol.rs – Test SGR, UTF-8, X10, SGR-PIXEL mode encoding
use torvox_terminal::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn mp1_mouse_x10_mode() {
    let mut g = t();
    g.vt_write(b"\x1b[?9h");
    g.flush();
    assert!(g.mode_get(9, 0));
}
#[test]
fn mp2_mouse_sgr_mode() {
    let mut g = t();
    g.vt_write(b"\x1b[?1000h");
    g.flush();
    assert!(g.mode_get(1000, 0));
}
#[test]
fn mp3_mouse_sgr_ext() {
    let mut g = t();
    g.vt_write(b"\x1b[?1006h");
    g.flush();
    assert!(g.mode_get(1006, 0));
}
#[test]
fn mp4_mouse_utf8() {
    let mut g = t();
    g.vt_write(b"\x1b[?1005h");
    g.flush();
    assert!(g.mode_get(1005, 0));
}
#[test]
fn mp5_mouse_sgr_pixels() {
    let mut g = t();
    g.vt_write(b"\x1b[?1016h");
    g.flush();
    assert!(g.mode_get(1016, 0));
}
#[test]
fn mp6_all_modes_off() {
    let g = t();
    assert!(!g.mode_get(9, 0));
    assert!(!g.mode_get(1000, 0));
}
