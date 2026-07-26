use terminal_engine::GhosttyTerminal;
use terminal_engine::vt_conformance::{check_invariants, sized_term, term};

// ====================================================================
// P1.1: Kitty termtests — Window Title OSCs
// ====================================================================

#[test]
fn kitty_osc_0_set_window_icon_title() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]0;KittyTestTitle\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_osc_1_set_icon_name() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]1;KittyIcon\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_osc_2_set_window_title() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]2;WindowTitle\x1b\\");
    t.flush();
    let title = t.title();
    assert!(
        title.contains("WindowTitle"),
        "Kitty OSC 2: title should contain WindowTitle, got: {:?}",
        title
    );
    check_invariants(&t);
}

#[test]
fn kitty_osc_0_with_unicode() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]0;\xc3\xa9\xf0\x9f\x92\xbbTitle\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_osc_2_with_special_chars() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]2;Test [2024] Foo & Bar <3>\x1b\\");
    t.flush();
    let title = t.title();
    assert!(title.contains("Test"), "Kitty OSC 2 special: {:?}", title);
    check_invariants(&t);
}

#[test]
fn kitty_osc_2_multiple_set_title() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]2;First\x1b\\");
    t.flush();
    t.vt_write(b"\x1b]2;Second\x1b\\");
    t.flush();
    let title = t.title();
    assert!(
        title.contains("Second"),
        "Kitty OSC 2 overwrite: {:?}",
        title
    );
    check_invariants(&t);
}

#[test]
fn kitty_osc_0_bel_terminator() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]0;TestTitle\x07");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_osc_2_empty_title() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]2;\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_osc_0_2_combined() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]0;CombinedTitle\x1b\\");
    t.flush();
    t.vt_write(b"\x1b]2;SpecificTitle\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_osc_0_1_2_all_titles() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"\x1b]0;All:Icon+Title\x1b\\");
    t.flush();
    t.vt_write(b"\x1b]1;IconOnly\x1b\\");
    t.flush();
    t.vt_write(b"\x1b]2;TitleOnly\x1b\\");
    t.flush();
    check_invariants(&t);
}

#[test]
fn kitty_osc_0_1_2_clear_after_text() {
    let mut t = sized_term(5, 20, 500);
    t.vt_write(b"Some text\x1b]2;AfterText\x1b\\");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cell_at(0, 0).codepoint,
        'S' as u32,
        "Kitty OSC 2 after text: codepoint unchanged"
    );
    check_invariants(&t);
}

#[test]
fn kitty_osc_0_chain_several_times() {
    for _ in 0..10 {
        let mut t = sized_term(5, 20, 500);
        t.vt_write(b"\x1b]2;ChainTestTitle\x1b\\");
        t.flush();
        check_invariants(&t);
    }
}
