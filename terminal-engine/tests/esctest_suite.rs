use terminal_engine::ghostty_terminal::GhosttyTerminal;
use terminal_engine::test_helpers::assert_invariants;

/// esctest-compatible test case: send input, assert screen state.
///
/// Equivalent to esctest's `AssertScreenCharsInRectEqual`.
struct EscTestCase {
    input: &'static [u8],
    rows: u32,
    cols: u32,
    scrollback: u32,
    assertions: Vec<EscAssertion>,
}

enum EscAssertion {
    /// Assert a rectangular region contains exact characters (row-major strings).
    /// Equivalent to esctest `AssertScreenCharsInRectEqual`.
    RectChars {
        top: u32,
        left: u32,
        expected: &'static [&'static str],
    },
    /// Assert cursor is at (row, col) — 0-indexed.
    CursorPos(u32, u32),
    /// Assert a specific cell has given attribute flags.
    CellAttrs {
        row: u32,
        col: u32,
        bold: bool,
        italic: bool,
        underline: bool,
    },
    /// Assert invariants hold.
    Invariants,
    /// Assert a DSR/DA response was enqueued.
    HasResponse,
}

fn run_esctest_case(case: &EscTestCase) {
    let mut t = GhosttyTerminal::new(case.rows, case.cols, case.scrollback)
        .expect("esctest: create terminal");
    t.pty_write(case.input);
    t.flush();
    let snap = t.take_snapshot();
    for a in &case.assertions {
        match a {
            EscAssertion::RectChars {
                top,
                left,
                expected,
            } => {
                for (row_off, row_str) in expected.iter().enumerate() {
                    let abs_row = top + row_off as u32;
                    for (col_off, ch) in row_str.chars().enumerate() {
                        let abs_col = left + col_off as u32;
                        let idx = (abs_row * case.cols + abs_col) as usize;
                        let actual_cp = snap.cells[idx].codepoint;
                        let expected_cp = ch as u32;
                        if actual_cp == 0 && expected_cp == b' ' as u32 {
                            continue;
                        }
                        if actual_cp == b' ' as u32 && expected_cp == 0 {
                            continue;
                        }
                        assert_eq!(
                            actual_cp,
                            expected_cp,
                            "esctest rect ({},{}): expected {:?} got {:?}",
                            abs_row,
                            abs_col,
                            ch,
                            if actual_cp == 0 {
                                '·'
                            } else {
                                char::from_u32(actual_cp).unwrap_or('?')
                            }
                        );
                    }
                }
            }
            EscAssertion::CursorPos(r, c) => {
                assert_eq!(snap.cursor_row, *r, "esctest cursor row");
                assert_eq!(snap.cursor_col, *c, "esctest cursor col");
            }
            EscAssertion::CellAttrs {
                row,
                col,
                bold,
                italic,
                underline,
            } => {
                let idx = (*row * case.cols + *col) as usize;
                let cell = &snap.cells[idx];
                assert_eq!(cell.bold, *bold, "esctest attrs bold at ({},{})", row, col);
                assert_eq!(
                    cell.italic, *italic,
                    "esctest attrs italic at ({},{})",
                    row, col
                );
                assert_eq!(
                    cell.underline, *underline,
                    "esctest attrs underline at ({},{})",
                    row, col
                );
            }
            EscAssertion::Invariants => {
                assert_invariants(&snap);
            }
            EscAssertion::HasResponse => {
                let responses = t.drain_pty_write_responses();
                assert!(!responses.is_empty(), "esctest: expected DSR response");
            }
        }
    }
}

// ── Real esctest test cases ──────────────────────────────────────────

macro_rules! esctest {
    ($name:ident, $case:expr) => {
        #[test]
        fn $name() {
            run_esctest_case(&$case);
        }
    };
}

esctest!(
    esc_cursor_left,
    EscTestCase {
        input: b"AB\x1b[D",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["AB"]
            },
            EscAssertion::CursorPos(0, 1),
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_cursor_right,
    EscTestCase {
        input: b"A\x1b[CB",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["A B"]
            },
            EscAssertion::CursorPos(0, 3),
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_cursor_up,
    EscTestCase {
        input: b"A\n\n\x1b[A\x1b[GB",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["A", "B", ""]
            },
            EscAssertion::CursorPos(1, 1),
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_cursor_down,
    EscTestCase {
        input: b"A\x1b[B\x1b[GB",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["A", "B"]
            },
            EscAssertion::CursorPos(1, 1),
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_erase_display_0,
    EscTestCase {
        input: b"AAAAABBBBB\x1b[2;1H\x1b[0J",
        rows: 3,
        cols: 5,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["AAAAA", "", ""]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_erase_display_1,
    EscTestCase {
        input: b"AAAAABBBBB\x1b[2;1H\x1b[1J",
        rows: 3,
        cols: 5,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["", " BBBB", ""]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_erase_display_2,
    EscTestCase {
        input: b"AAAAABBBBB\x1b[2J",
        rows: 3,
        cols: 5,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["", "", ""]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_erase_line_0,
    EscTestCase {
        input: b"ABCDEFGHIJ\x1b[5G\x1b[0K",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["ABCD"]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_erase_line_1,
    EscTestCase {
        input: b"ABCDEFGHIJ\x1b[5G\x1b[1K",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["     FGHIJ"]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_sgr_attributes,
    EscTestCase {
        input: b"\x1b[1;3;4mBOLDITALICUNDER",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::CellAttrs {
                row: 0,
                col: 0,
                bold: true,
                italic: true,
                underline: true
            },
            EscAssertion::CellAttrs {
                row: 0,
                col: 4,
                bold: true,
                italic: true,
                underline: true
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_sgr_reset,
    EscTestCase {
        input: b"\x1b[1;4mB\x1b[0mN",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::CellAttrs {
                row: 0,
                col: 0,
                bold: true,
                italic: false,
                underline: true
            },
            EscAssertion::CellAttrs {
                row: 0,
                col: 1,
                bold: false,
                italic: false,
                underline: false
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_cursor_position,
    EscTestCase {
        input: b"\x1b[3;5HX",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::CursorPos(2, 5),
            EscAssertion::RectChars {
                top: 2,
                left: 0,
                expected: &["    X"]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_insert_line,
    EscTestCase {
        input: b"AAA\nBBB\nCCC\x1b[2;1H\x1b[1L",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["AAA", "", "BBB"]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_delete_line,
    EscTestCase {
        input: b"AAA\nBBB\nCCC\x1b[2;1H\x1b[1M",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![
            EscAssertion::RectChars {
                top: 0,
                left: 0,
                expected: &["AAA", "CCC", ""]
            },
            EscAssertion::Invariants,
        ],
    }
);

esctest!(
    esc_dsr_cpr,
    EscTestCase {
        input: b"\x1b[6n",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![EscAssertion::HasResponse, EscAssertion::Invariants,],
    }
);

esctest!(
    esc_da_primary,
    EscTestCase {
        input: b"\x1b[c",
        rows: 5,
        cols: 20,
        scrollback: 100,
        assertions: vec![EscAssertion::HasResponse, EscAssertion::Invariants,],
    }
);
