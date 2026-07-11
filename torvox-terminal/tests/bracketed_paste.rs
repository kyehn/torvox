// bracketed_paste.rs – Test paste enable/disable
use torvox_terminal::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn bp1_paste_mode_default() {
    let g = t();
    assert!(!g.mode_get(2004, 0));
}
#[test]
fn bp2_paste_mode_set() {
    let mut g = t();
    g.vt_write(b"\x1b[?2004h");
    g.flush();
    assert!(g.mode_get(2004, 0));
}
#[test]
fn bp3_paste_mode_reset() {
    let mut g = t();
    g.vt_write(b"\x1b[?2004h\x1b[?2004l");
    g.flush();
    assert!(!g.mode_get(2004, 0));
}
#[test]
fn bp4_bracketed_paste_no_crash() {
    let mut g = t();
    g.vt_write(b"\x1b[?2004hHello\x1b[?2004l");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'H' as u32);
}
#[test]
fn bp5_bracketed_paste_many_chars() {
    let mut g = t();
    g.vt_write(b"\x1b[?2004h");
    g.vt_write(&[b'A'; 100]);
    g.vt_write(b"\x1b[?2004l");
    g.flush();
    let s = g.take_snapshot();
    assert!(s.cells[0].codepoint > 0);
}
