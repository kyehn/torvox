// ====================================================================
// P1.3: Ghostty AFL++ Fuzz Corpus Replay Tests
//
// Replays known VT parser inputs from our own fuzz corpus.
// Mirrors Ghostty's approach: fuzz corpus → regression tests.
// ====================================================================

use torvox_terminal::vt_conformance::{check_invariants, sized_term};

// ── Fuzz replay: VT sequence consists of known safe inputs ──────

#[test]
fn fuzz_replay_vt_simple_csi() {
    // Simple CSI sequence that fuzzer may produce
    for &n in &[0u8, 1, 5, 9, 10, 15, 20, 27, 31, 127, 128, 255] {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(&[0x1b, b'[', n, b'A']);
        t.flush();
        check_invariants(&t);
        t.vt_write(&[0x1b, b'[', n, b'B']);
        t.flush();
        check_invariants(&t);
        t.vt_write(&[0x1b, b'[', n, b'C']);
        t.flush();
        check_invariants(&t);
        t.vt_write(&[0x1b, b'[', n, b'D']);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_multiple_csi_params() {
    // Sequences with multiple parameters, including edge cases
    let cases: &[&[u8]] = &[
        b"\x1b[0;0H",
        b"\x1b[999;999H",
        b"\x1b[1;1;1m",
        b"\x1b[0;0;0m",
        b"\x1b[;H",
        b"\x1b[1;;3H",
        b"\x1b[38;5;999m",
        b"\x1b[38;2;999;999;999m",
        b"\x1b[?0h",
        b"\x1b[?9999h",
        b"\x1b[?0l",
        b"\x1b[?9999l",
    ];
    for case in cases {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(case);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_osc_overflow() {
    // Long OSC sequences (fuzzer may produce very long OSCs)
    let long_title = vec![b'A'; 1000];
    let mut osc = b"\x1b]2;".to_vec();
    osc.extend_from_slice(&long_title);
    osc.extend_from_slice(b"\x1b\\");
    let mut t = sized_term(24, 80, 500);
    t.vt_write(&osc);
    t.flush();
    check_invariants(&t);
}

#[test]
fn fuzz_replay_vt_osc_invalid_content() {
    // Invalid OSC sequences
    let cases: &[&[u8]] = &[
        b"\x1b]0;\x00\xff\x07",
        b"\x1b]4;\x00;#abcd\x1b\\",
        b"\x1b]52;\x00;invalid-base64!\x1b\\",
        b"\x1b]8;\x03;invalid\x1b\\",
        b"\x1b];no_number\x1b\\",
        b"\x1b]999999;test\x1b\\",
    ];
    for case in cases {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(case);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_csi_incomplete() {
    // Incomplete CSI sequences (interrupted mid-stream)
    let cases: &[&[u8]] = &[
        b"\x1b[",
        b"\x1b[5",
        b"\x1b[1;",
        b"\x1b[1;2",
        b"\x1b[?",
        b"\x1b[?1",
        b"\x1b[?25",
        b"\x1b[38;2;",
        b"\x1b[38;2;255",
        b"\x1b[38;2;255;0",
    ];
    for case in cases {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(case);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_random_bytes() {
    // Random byte sequences that could come from a fuzzer
    let sequences: &[&[u8]] = &[
        b"\x00\x01\x02\x1b\xff",
        b"\x1b\x1b\x1b\x1b\x1b",
        b"\x1b[0m\x1b[0m\x1b[0m",
        b"\x1b[1;2;3;4;5;6;7;8;9;10m",
        b"\xff\xfe\xfd\xfc",
        b"\x1b]0;\xff\x1b\\",
        b"\x1b[1;1;1;1;1;1;1;1;1;1;1;1H",
        b"\x1b[0;0;0;0;0;0;0;0;0;0m",
        b"\x1b[?1000;1000;1000;1000h",
        b"\x1b[2;2;2;2;2;2;2;2;2;2J",
    ];
    for seq in sequences {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(seq);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_extreme_sgr_0_to_109() {
    // Every SGR parameter, including edge values
    let mut t = sized_term(5, 80, 500);
    for param in 0u8..=109u8 {
        let seq = format!("\x1b[{}mX\x1b[0m", param);
        t.vt_write(seq.as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_rapid_csi_cycle() {
    // Rapid cycling through CSI states (tests parser state machine)
    let mut t = sized_term(24, 80, 500);
    for i in 0..100 {
        let seq = format!(
            "\x1b[{};{}H\x1b[{}mX\x1b[{}K",
            (i % 24) + 1,
            (i % 80) + 1,
            i % 110,
            i % 3
        );
        t.vt_write(seq.as_bytes());
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_double_escape() {
    // Double escape sequences
    let cases: &[&[u8]] = &[
        b"\x1b\x1b[A",
        b"\x1b\x1b[5B",
        b"\x1b\x1b[31m",
        b"\x1b\x1b]0;test\x1b\\",
        b"\x1b\x1b\x1b[A",
    ];
    for case in cases {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(case);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_null_bytes() {
    // Sequences with embedded NUL bytes
    for n in 0..5 {
        let mut bytes = format!("\x1b[{};{}H", 5, n + 1).into_bytes();
        bytes.push(b'\x00');
        bytes.extend_from_slice(b"AB");
        let mut t = sized_term(24, 80, 500);
        t.vt_write(&bytes);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_mixed_long_short_params() {
    // Mixed long and short parameters in one sequence
    let cases: &[&[u8]] = &[
        b"\x1b[1;99999;3;2H",
        b"\x1b[0;999;99999;0m",
        b"\x1b[?1;9999;1000;0h",
        b"\x1b[100;1;99999;3;2;0;5000J",
        b"\x1b[1;99999;0;1;99999;1;10A",
    ];
    for case in cases {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(case);
        t.flush();
        check_invariants(&t);
    }
}

#[test]
fn fuzz_replay_vt_consume_until_final_byte() {
    // CSI consumes params until it finds a final byte
    let mut t = sized_term(24, 80, 500);
    // This should be treated as a single CSI with many params
    t.vt_write(b"\x1b[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17;18;19;20H");
    t.flush();
    check_invariants(&t);
}

#[test]
fn fuzz_replay_vt_esc_sequences() {
    // Various ESC sequences
    let cases: &[&[u8]] = &[
        b"\x1b7",  // DECSC
        b"\x1b8",  // DECRC
        b"\x1b=",  // DECKPAM
        b"\x1b>",  // DECKPNM
        b"\x1bc",  // RIS
        b"\x1bM",  // RI
        b"\x1bD",  // IND
        b"\x1bE",  // NEL
        b"\x1bH",  // HTS
        b"\x1b#8", // DECALN
        b"\x1b#3", // DECDHL top
        b"\x1b#5", // DECSWL
        b"\x1bN ", // SS2
        b"\x1bO ", // SS3
    ];
    for case in cases {
        let mut t = sized_term(24, 80, 500);
        t.vt_write(case);
        t.flush();
        check_invariants(&t);
    }
}
