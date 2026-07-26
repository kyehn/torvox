#![allow(non_snake_case)]

use terminal_engine::ghostty_terminal::GhosttyTerminal;
use terminal_engine::test_helpers::assert_invariants;

fn term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn alt_screen_enter_exit_no_crash() {
    let mut t = term();
    t.vt_write(b"\x1b[?1049h");
    t.flush();
    assert!(t.alt_screen(), "should be in alt screen after enter");
    t.vt_write(b"\x1b[?1049l");
    t.flush();
    assert!(!t.alt_screen(), "should be in main screen after exit");
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

#[test]
fn alt_screen_content_preserved() {
    let mut t = term();
    t.vt_write(b"MainScreenText");
    t.flush();
    t.vt_write(b"\x1b[?1049h");
    t.flush();
    t.vt_write(b"AltScreenText");
    t.flush();
    t.vt_write(b"\x1b[?1049l");
    t.flush();
    let line0 = t.read_line_text(0);
    assert!(line0.is_some(), "main screen content should be preserved");
    let text = line0.unwrap();
    assert!(
        text.contains("MainScreenText"),
        "expected 'MainScreenText' after alt screen exit, got: {text:?}"
    );
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

#[test]
fn alt_screen_scroll_not_crash() {
    let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
    t.vt_write(b"\x1b[?1049h");
    t.flush();
    for i in 0..5 {
        t.vt_write(format!("alt line {i}\n").as_bytes());
    }
    t.flush();
    t.vt_write(b"AfterAltScroll");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'A' as u32);
    assert!(found, "should render after scroll in alt screen");
    assert_invariants(&snap);
}

#[test]
fn alt_screen_cursor_position() {
    let mut t = term();
    t.vt_write(b"\x1b[?1049h");
    t.flush();
    t.vt_write(b"\x1b[5;10HX");
    t.flush();
    let snap = t.take_snapshot();
    let idx = (4 * 80 + 9) as usize;
    assert_eq!(
        snap.cells[idx].codepoint, 'X' as u32,
        "cursor should be at row 5 col 10 (0-based 4,9) in alt screen"
    );
    assert_invariants(&snap);
}

#[test]
fn alt_screen_clear_before_switch() {
    let mut t = term();
    t.vt_write(b"PreserveThis");
    t.flush();
    t.vt_write(b"\x1b[?1049h");
    t.flush();
    t.vt_write(b"\x1b[2J");
    t.flush();
    t.vt_write(b"\x1b[?1049l");
    t.flush();
    let line0 = t.read_line_text(0);
    assert!(
        line0.is_some(),
        "main screen content should survive DECSET 1049"
    );
    let text = line0.unwrap();
    assert!(
        text.contains("PreserveThis"),
        "expected 'PreserveThis' preserved after alt screen clear+exit, got: {text:?}"
    );
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}
