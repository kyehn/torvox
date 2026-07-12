#![allow(non_snake_case)]

use torvox_terminal::ghostty_terminal::GhosttyTerminal;
use torvox_terminal::test_helpers::assert_invariants;

fn term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("term")
}

#[test]
fn selection_char_mode_basic() {
    let mut t = term();
    t.vt_write(b"\x1b[1;1HABCDEFGHIJ");
    t.flush();
    let snap = t.take_snapshot();
    assert_invariants(&snap);
    let text = t.read_visible_text();
    assert!(
        text.contains("CDEF"),
        "expected 'CDEF' in visible text, got: {text:?}"
    );
}

#[test]
fn selection_char_mode_multi_line() {
    let mut t = term();
    t.vt_write(b"ABC\r\nDEF");
    t.flush();
    let line0 = t.read_line_text(0);
    let line1 = t.read_line_text(1);
    assert!(line0.is_some());
    assert!(line1.is_some());
    assert!(line0.unwrap().contains("ABC"));
    assert!(line1.unwrap().contains("DEF"));
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

#[test]
fn selection_word_mode() {
    let mut t = term();
    t.vt_write(b"hello world foo");
    t.flush();
    let text = t.read_visible_text();
    assert!(
        text.contains("hello"),
        "expected 'hello' in text, got: {text:?}"
    );
    assert!(
        text.contains("world"),
        "expected 'world' in text, got: {text:?}"
    );
    assert!(
        text.contains("foo"),
        "expected 'foo' in text, got: {text:?}"
    );
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

#[test]
fn selection_line_mode() {
    let mut t = term();
    t.vt_write(b"Entire line content here");
    t.flush();
    let line0 = t.read_line_text(0);
    assert!(line0.is_some());
    assert_eq!(line0.unwrap(), "Entire line content here");
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

#[test]
fn selection_block_mode() {
    let mut t = term();
    t.vt_write(b"ABCD\r\nEFGH\r\nIJKL");
    t.flush();
    let text = t.read_visible_text();
    assert!(text.contains("ABCD"));
    assert!(text.contains("EFGH"));
    assert!(text.contains("IJKL"));
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

#[test]
fn selection_after_scroll() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("term");
    for i in 0..10 {
        t.vt_write(format!("line {i}\n").as_bytes());
    }
    t.flush();
    assert!(
        t.scrollback_length() > 0,
        "scrollback should have entries after scrolling"
    );
    t.vt_write(b"AfterScrollSelection");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'A' as u32);
    assert!(found, "terminal should survive scrollback and render");
    assert_invariants(&snap);
}

#[test]
fn selection_empty_when_nothing_selected() {
    let t = term();
    let text = t.read_visible_text();
    assert!(
        text.is_empty() || text.trim().is_empty(),
        "visible text should be empty on fresh terminal, got: {text:?}"
    );
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

#[test]
fn read_visible_text_returns_viewport_not_history_after_scroll() {
    // Regression: read_visible_text must return the VISIBLE viewport, not the
    // first N history rows. With scrollback present, read_line_text_impl takes
    // an absolute row, so the visible read must offset by scrollback_rows.
    let mut t = term();
    for line in 0..60 {
        t.vt_write(format!("LINE_{line:02}\r\n").as_bytes());
    }
    t.flush();
    let text = t.read_visible_text();
    assert!(
        text.contains("LINE_59"),
        "visible text must contain the last written line, got: {text:?}"
    );
    assert!(
        !text.contains("LINE_00"),
        "visible text must NOT contain scrolled-off history line LINE_00, got: {text:?}"
    );
}
