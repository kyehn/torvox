// WezTerm-style Level 2: Sequence Parser Testing
// Tests CSI parsing through GhosttyTerminal — verifying that Ghostty processes
// each CSI shape (with/without params, private markers, subparams) correctly.

use terminal_engine::action_parser::CsiSeq;
use terminal_engine::test_helpers::assert_invariants;

fn term() -> terminal_engine::ghostty_terminal::GhosttyTerminal {
    terminal_engine::ghostty_terminal::GhosttyTerminal::new(24, 80, 1000).expect("terminal")
}

fn sized(r: u32, c: u32) -> terminal_engine::ghostty_terminal::GhosttyTerminal {
    terminal_engine::ghostty_terminal::GhosttyTerminal::new(r, c, 1000).expect("terminal")
}

fn ci(t: &terminal_engine::ghostty_terminal::GhosttyTerminal) {
    assert_invariants(&t.take_snapshot());
}

// ── CSI parsing verification ────────────────────────────────────────

#[test]
fn l2_csi_cursor_up() {
    let parsed = CsiSeq::parse(b"\x1b[5A").unwrap();
    assert_eq!(parsed.final_byte, b'A');
    assert_eq!(parsed.params, &[5]);
    assert!(parsed.private_marker.is_none());
    // Behavioral verif: cursor goes up 5
    let mut t = sized(10, 30);
    t.vt_write(b"\x1b[6;1H\x1b[5A");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "CUU 5 from row 6 -> row 0-ish");
    ci(&t);
}

#[test]
fn l2_csi_cursor_down_default() {
    let parsed = CsiSeq::parse(b"\x1b[B").unwrap();
    assert_eq!(parsed.final_byte, b'B');
    assert!(parsed.params.is_empty());
    // N=0 treated as N=1
    let parsed0 = CsiSeq::parse(b"\x1b[0B").unwrap();
    assert_eq!(parsed0.params, &[0]);
    // Ghostty: CUD with no param behaves as CUD 1
    let mut t = sized(5, 20);
    t.vt_write(b"\x1b[B");
    t.flush();
    assert_eq!(t.cursor_y(), 1, "CUD default -> row 1");
}

#[test]
fn l2_csi_sgr_multi_params() {
    let parsed = CsiSeq::parse(b"\x1b[1;31;42m").unwrap();
    assert_eq!(parsed.final_byte, b'm');
    assert_eq!(parsed.params, &[1, 31, 42]);
    // Behavioral: bold + red fg + green bg
    let mut t = term();
    t.vt_write(b"\x1b[1;31;42mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.cells[0].bold, "bold set");
    assert!(snap.cells[0].foreground[0] > 0.0, "red fg");
    assert!(snap.cells[0].background[1] > 0.0, "green bg");
}

#[test]
fn l2_csi_sgr_truecolor() {
    let parsed = CsiSeq::parse(b"\x1b[38;2;100;150;200m").unwrap();
    assert_eq!(parsed.final_byte, b'm');
    assert_eq!(parsed.params, &[38, 2, 100, 150, 200]);
    let mut t = term();
    t.vt_write(b"\x1b[38;2;100;150;200mX");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        (snap.cells[0].foreground[0] - 0.392).abs() < 0.05,
        "fg R ~100/255=0.392"
    );
}

#[test]
fn l2_csi_private_decset() {
    let parsed = CsiSeq::parse(b"\x1b[?25h").unwrap();
    assert_eq!(parsed.final_byte, b'h');
    assert_eq!(parsed.private_marker, Some(b'?'));
    assert_eq!(parsed.params, &[25]);
    // Behavioral: DECSET 25 shows cursor (default: visible)
    let mut t = term();
    t.vt_write(b"\x1b[?25l"); // hide
    t.flush();
    assert!(!t.is_cursor_enabled(), "cursor hidden");
    t.vt_write(b"\x1b[?25h"); // show
    t.flush();
    assert!(t.is_cursor_enabled(), "cursor visible");
}

#[test]
fn l2_csi_cup_5_10() {
    let parsed = CsiSeq::parse(b"\x1b[5;10H").unwrap();
    assert_eq!(parsed.final_byte, b'H');
    assert_eq!(parsed.params, &[5, 10]);
    let mut t = sized(10, 30);
    t.vt_write(b"\x1b[5;10H");
    t.flush();
    assert_eq!(t.cursor_y(), 4, "CUP row 5 -> idx 4");
    assert_eq!(t.cursor_x(), 9, "CUP col 10 -> idx 9");
}

#[test]
fn l2_csi_ed_params() {
    for (seq, exp_p) in [
        ("\x1b[J", 0),
        ("\x1b[0J", 0),
        ("\x1b[1J", 1),
        ("\x1b[2J", 2),
    ] {
        let parsed = CsiSeq::parse(seq.as_bytes()).unwrap();
        assert_eq!(parsed.final_byte, b'J');
        if parsed.params.is_empty() {
            let effective = parsed.param_or_default(0, 0);
            assert_eq!(effective, exp_p, "ED {}: default->0", seq);
        } else {
            assert_eq!(parsed.params[0], exp_p, "ED {}: param", seq);
        }
    }
}

#[test]
fn l2_csi_scroll_region() {
    let parsed = CsiSeq::parse(b"\x1b[2;4r").unwrap();
    assert_eq!(parsed.final_byte, b'r');
    assert_eq!(parsed.params, &[2, 4]);
}

#[test]
fn l2_csi_empty_params_home() {
    let mut t = sized(5, 20);
    t.vt_write(b"\x1b[5;10H\x1b[H");
    t.flush();
    assert_eq!(t.cursor_y(), 0, "CUP empty -> home");
    assert_eq!(t.cursor_x(), 0, "CUP empty -> home");
}

#[test]
fn l2_csi_missing_params() {
    for seq in &[b"\x1b[;H" as &[u8], b"\x1b[;;H", b"\x1b[0;0H"] {
        let mut t = sized(5, 20);
        t.vt_write(seq);
        t.flush();
        // Missing/0 params -> home (1,1) = (0,0)
        assert_eq!(
            t.cursor_y(),
            0,
            "CSI {:?} -> home row",
            String::from_utf8_lossy(seq)
        );
        assert_eq!(
            t.cursor_x(),
            0,
            "CSI {:?} -> home col",
            String::from_utf8_lossy(seq)
        );
        ci(&t);
    }
}

#[test]
fn l2_csi_param_0_vs_1() {
    // CSI with param 0 should behave identically to param 1 (for cursor moves)
    let mut t = sized(10, 30);
    t.vt_write(b"\x1b[5;1H\x1b[0B"); // CUD with param 0
    t.flush();
    let pos0 = (t.cursor_y(), t.cursor_x());
    let mut t2 = sized(10, 30);
    t2.vt_write(b"\x1b[5;1H\x1b[1B"); // CUD with param 1
    t2.flush();
    let pos1 = (t2.cursor_y(), t2.cursor_x());
    assert_eq!(pos0, pos1, "CUD 0 and CUD 1 should behave identically");
}

#[test]
fn l2_csi_leading_zeros() {
    // CSI 001;003;004m should be equivalent to CSI 1;3;4m
    let parsed1 = CsiSeq::parse(b"\x1b[001;003;004m").unwrap();
    let parsed2 = CsiSeq::parse(b"\x1b[1;3;4m").unwrap();
    assert_eq!(
        parsed1.params, parsed2.params,
        "leading zeros should be ignored"
    );
}

#[test]
fn l2_csi_subparameter_colons_safe() {
    // CSI with colon subparams should not crash
    let mut t = term();
    t.vt_write(b"\x1b[1:2H"); // CUP with colon subparams
    t.flush();
    ci(&t);
    t.vt_write(b"\x1b[38:2:100:150:200mX");
    t.flush();
    ci(&t);
}

#[test]
fn l2_csi_unrecognized_final_bytes_safe() {
    for seq in &[
        b"\x1b[5k" as &[u8],
        b"\x1b[5n",
        b"\x1b[?5n",
        b"\x1b[5o",
        b"\x1b[5q",
        b"\x1b[5t",
    ] {
        let mut t = term();
        t.vt_write(seq);
        t.flush();
        ci(&t);
    }
}

#[test]
fn l2_csi_decrqm_all_modes() {
    for mode in &[
        1u16, 2, 3, 6, 7, 12, 25, 40, 42, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1015, 1016,
        1034, 1035, 1036, 1037, 1039, 1040, 1041, 1042, 1048, 1049, 2004, 2026,
    ] {
        let mut t = term();
        let q = format!("\x1b[?{};$p", mode);
        t.vt_write(q.as_bytes());
        t.flush();
        ci(&t);
    }
}
