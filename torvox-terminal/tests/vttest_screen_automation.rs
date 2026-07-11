// vttest_screen_automation.rs – Remaining vttest screens
use torvox_terminal::ghostty_terminal::GhosttyTerminal;
fn t() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn vt1_cursor_movement_screen1() {
    let mut g = t();
    g.vt_write(b"Test1");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[0].codepoint, 'T' as u32);
}
#[test]
fn vt2_cursor_movement_screen2() {
    let mut g = t();
    g.vt_write(b"\x1b[5;10HX");
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.cells[4 * 80 + 9].codepoint, 'X' as u32);
}
#[test]
fn vt3_sgr_screen() {
    let mut g = t();
    g.vt_write(b"\x1b[1;31mText");
    g.flush();
    let s = g.take_snapshot();
    assert!(s.cells[0].bold);
}
#[test]
fn vt4_scroll_screen() {
    let mut g = GhosttyTerminal::new(5, 20, 100).expect("t");
    for i in 0..10 {
        g.vt_write(format!("{}\n", i).as_bytes());
    }
    g.flush();
    let s = g.take_snapshot();
    assert_eq!(s.rows, 5);
}
#[test]
fn vt5_tab_screen() {
    let mut g = t();
    g.vt_write(b"\x1b[10G\x1bH\x1b[H\x09X");
    g.flush();
    let col = g.cursor_x();
    assert!(
        col >= 8,
        "tab should advance cursor to at least col 8, got {}",
        col
    );
}
#[test]
fn vt6_erase_screen() {
    let mut g = t();
    g.vt_write(b"Data\x1b[2J");
    g.flush();
    let s = g.take_snapshot();
    assert!(!s.cells.iter().any(|c| c.codepoint == 'D' as u32));
}
