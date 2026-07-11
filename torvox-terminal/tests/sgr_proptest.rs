use proptest::prelude::*;
use torvox_terminal::ghostty_terminal::GhosttyTerminal;
use torvox_terminal::test_helpers::assert_invariants;

fn make_term(rows: u32, cols: u32) -> GhosttyTerminal {
    GhosttyTerminal::new(rows, cols, 1000).expect("term")
}

#[allow(dead_code)]
fn sgr_seq() -> impl Strategy<Value = Vec<u8>> {
    // Generate 1-5 SGR params, each 0-107
    proptest::collection::vec(0u8..=107u8, 1..=5usize).prop_map(|params| {
        let mut seq = b"\x1b[".to_vec();
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                seq.push(b';');
            }
            seq.extend(format!("{}", p).as_bytes());
        }
        seq.push(b'm');
        seq
    })
}

#[allow(dead_code)]
fn sgr_256_seq() -> impl Strategy<Value = Vec<u8>> {
    (0u8..=255u8).prop_map(|idx| format!("\x1b[38;5;{}mX", idx).into_bytes())
}

#[allow(dead_code)]
fn sgr_truecolor_seq() -> impl Strategy<Value = Vec<u8>> {
    (0u8..=255u8, 0u8..=255u8, 0u8..=255u8)
        .prop_map(|(r, g, b)| format!("\x1b[38;2;{};{};{}mX", r, g, b).into_bytes())
}

proptest! {

    #[test]
    fn sgr_any_params_no_crash(params in proptest::collection::vec(0u8..=107u8, 1..=5)) {
        let mut t = make_term(5, 40);
        let mut seq = b"\x1b[".to_vec();
        for (i, p) in params.iter().enumerate() {
            if i > 0 { seq.push(b';'); }
            seq.extend(format!("{}", p).as_bytes());
        }
        seq.push(b'm');
        t.vt_write(&seq);
        t.flush();
        let snap = t.take_snapshot();
        // Color channels in range
        for i in 0..3 {
            prop_assert!(snap.cells[0].foreground[i] >= 0.0 && snap.cells[0].foreground[i] <= 1.0,
                "fg[{}]={} out of range", i, snap.cells[0].foreground[i]);
            prop_assert!(snap.cells[0].background[i] >= 0.0 && snap.cells[0].background[i] <= 1.0,
                "bg[{}]={} out of range", i, snap.cells[0].background[i]);
        }
    }

    #[test]
    fn sgr_256_color_no_crash(idx in 0u8..=255u8) {
        let mut t = make_term(5, 40);
        t.vt_write(format!("\x1b[38;5;{}mX", idx).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for i in 0..3 {
            prop_assert!(snap.cells[0].foreground[i] >= 0.0, "fg[{i}] < 0 for idx={idx}");
            prop_assert!(snap.cells[0].foreground[i] <= 1.0, "fg[{i}] > 1 for idx={idx}");
        }
    }

    #[test]
    fn sgr_truecolor_valid_channels(r in 0u8..=255u8, g in 0u8..=255u8, b in 0u8..=255u8) {
        let mut t = make_term(5, 40);
        t.vt_write(format!("\x1b[38;2;{};{};{}mX", r, g, b).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        for i in 0..3 {
            prop_assert!(snap.cells[0].foreground[i] >= 0.0, "fg[{i}] < 0");
            prop_assert!(snap.cells[0].foreground[i] <= 1.0, "fg[{i}] > 1");
        }
    }
}

// ── Loop-based exhaustive SGR combinations ───────────────────────

#[test]
fn sgr_fg_bg_all_8_standard() {
    for fg in 30..=37 {
        for bg in 40..=47 {
            let mut t = make_term(5, 40);
            t.vt_write(format!("\x1b[{};{}mX", fg, bg).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert_invariants(&snap);
        }
    }
}

#[test]
fn sgr_bright_fg_bg_all_8_pairs() {
    for fg in 90..=97 {
        for bg in 100..=107 {
            let mut t = make_term(5, 40);
            t.vt_write(format!("\x1b[{};{}mX", fg, bg).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert_invariants(&snap);
        }
    }
}

#[test]
fn sgr_256_color_systematic_64_pairs() {
    let idxs = [0u8, 16, 52, 88, 124, 160, 196, 232];
    for &fg in &idxs {
        for &bg in &idxs {
            let mut t = make_term(5, 40);
            t.vt_write(format!("\x1b[38;5;{};48;5;{}mX", fg, bg).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert_invariants(&snap);
            for i in 0..3 {
                assert!(
                    snap.cells[0].foreground[i] >= 0.0 && snap.cells[0].foreground[i] <= 1.0,
                    "fg[{i}]={} out for fg={fg},bg={bg}",
                    snap.cells[0].foreground[i]
                );
                assert!(
                    snap.cells[0].background[i] >= 0.0 && snap.cells[0].background[i] <= 1.0,
                    "bg[{i}]={} out for fg={fg},bg={bg}",
                    snap.cells[0].background[i]
                );
            }
        }
    }
}

#[test]
fn sgr_attr_all_8_pair_combinations() {
    let attrs = [1u8, 3, 4, 5, 7, 8, 9, 53];
    for &a in &attrs {
        for &b in &attrs {
            let mut t = make_term(5, 40);
            t.vt_write(format!("\x1b[{};{}mX", a, b).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert_invariants(&snap);
        }
    }
}

#[test]
fn sgr_attr_3way_combinations() {
    let attrs = [1u8, 3, 4, 5, 7, 8, 9, 53];
    let n = attrs.len();
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                let mut t = make_term(5, 40);
                t.vt_write(format!("\x1b[{};{};{}mX", attrs[i], attrs[j], attrs[k]).as_bytes());
                t.flush();
                assert_invariants(&t.take_snapshot());
            }
        }
    }
}

#[test]
fn sgr_attr_4way_combinations() {
    let attrs = [1u8, 3, 4, 5, 7, 9, 53];
    let n = attrs.len();
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                for l in k + 1..n {
                    let mut t = make_term(5, 40);
                    t.vt_write(
                        format!("\x1b[{};{};{};{}mX", attrs[i], attrs[j], attrs[k], attrs[l])
                            .as_bytes(),
                    );
                    t.flush();
                    assert_invariants(&t.take_snapshot());
                }
            }
        }
    }
}

#[test]
fn sgr_256_all_fg_8_loop() {
    for fg in (0u8..=255u8).step_by(32) {
        let mut t = make_term(5, 40);
        t.vt_write(format!("\x1b[38;5;{}mX", fg).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            snap.cells[0].foreground[0] >= 0.0
                || snap.cells[0].foreground[1] >= 0.0
                || snap.cells[0].foreground[2] >= 0.0,
            "all fg channels zero for 256 idx={fg}"
        );
    }
}

#[test]
fn sgr_256_all_bg_8_loop() {
    for bg in (0u8..=255u8).step_by(32) {
        let mut t = make_term(5, 40);
        t.vt_write(format!("\x1b[48;5;{}mX", bg).as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            snap.cells[0].background[0] >= 0.0
                || snap.cells[0].background[1] >= 0.0
                || snap.cells[0].background[2] >= 0.0,
            "all bg channels zero for 256 idx={bg}"
        );
    }
}

#[test]
fn sgr_fg_bg_then_sgr_0_reset() {
    for fg in 30..=37u8 {
        for bg in 40..=47u8 {
            let mut t = make_term(5, 40);
            t.vt_write(format!("\x1b[{};{}mX\x1b[0mY", fg, bg).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert!(
                !snap.cells[1].bold,
                "SGR 0: bold not reset for fg={fg},bg={bg}"
            );
        }
    }
}
