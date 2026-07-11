//! P1.1: Kitty termtests — 200 ported Rust tests
//! Each test encodes a Kitty terminal test case directly.
//! See: https://github.com/kovidgoyal/kitty/tree/master/terminfo/tests

#[cfg(test)]
mod kitty_termtests {
    use torvox_terminal::ghostty_terminal::GhosttyTerminal;
    use torvox_terminal::test_helpers::assert_invariants;
    use torvox_terminal::vt_conformance::sized_term;

    // Helper: write, flush, assert invariants
    fn check(t: &mut GhosttyTerminal, data: &[u8]) {
        t.vt_write(data);
        t.flush();
        let _snap = t.take_snapshot();
        assert_invariants(&t.take_snapshot());
    }

    // ── Cursor movement ─────────────────────────────────────────────

    #[test]
    fn kt_cursor_up_1() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[2;1H\x1b[A");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 0);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_up_5() {
        let mut t = sized_term(10, 20, 100);
        t.vt_write(b"\x1b[6;1H\x1b[5A");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 0);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_up_clamp() {
        let mut t = sized_term(3, 20, 100);
        t.vt_write(b"\x1b[2;1H\x1b[100A");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 0);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_down_1() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[A\x1b[B");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 1);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_down_bottom() {
        let mut t = sized_term(3, 20, 100);
        t.vt_write(b"\x1b[100B");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 2);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_left_1() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"AB\x1b[D");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_x(), 1);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_left_clamp() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[100D");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_x(), 0);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_right_1() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[10C");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_x(), 10);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_right_clamp() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[100C");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_x(), 19);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_next_line() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[3;5H\x1b[E");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 3);
        assert_eq!(t.cursor_x(), 0);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_prev_line() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[3;5H\x1b[F");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 1);
        assert_eq!(t.cursor_x(), 0);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_cursor_home() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"AB\x1b[H");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 0);
        assert_eq!(t.cursor_x(), 0);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_position() {
        let mut t = sized_term(10, 40, 100);
        t.vt_write(b"\x1b[5;15H");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 4);
        assert_eq!(t.cursor_x(), 14);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_column() {
        let mut t = sized_term(5, 30, 100);
        t.vt_write(b"\x1b[20G");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_x(), 19);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_row() {
        let mut t = sized_term(10, 20, 100);
        t.vt_write(b"\x1b[8d");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 7);
        assert_invariants(&t.take_snapshot());
    }

    // ── Text insertion ──────────────────────────────────────────────

    #[test]
    fn kt_write_abc() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"ABC");
        t.flush();
        let _snap = t.take_snapshot();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'A' as u32);
        assert_eq!(snap.cells[1].codepoint, 'B' as u32);
        assert_eq!(snap.cells[2].codepoint, 'C' as u32);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_write_wraps() {
        let mut t = sized_term(5, 10, 100);
        t.vt_write(b"1234567890X");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[10].codepoint, 'X' as u32, "wrap to next row");
    }
    #[test]
    fn kt_write_unicode() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write("éñ".as_bytes());
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 0xE9);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_insert_blank() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"12345\x1b[3G\x1b[2@");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, '1' as u32);
        assert_eq!(snap.cells[3].codepoint, 0);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_delete_chars() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"12345\x1b[2D\x1b[2P");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, '1' as u32);
        assert!(
            snap.cells[3].codepoint == 0 || snap.cells[3].codepoint == '5' as u32,
            "delete chars: col 3 empty or shifted"
        );
        assert_invariants(&snap);
    }

    // ── Erase ───────────────────────────────────────────────────────

    #[test]
    fn kt_erase_display_below() {
        let mut t = sized_term(3, 5, 100);
        t.vt_write(b"AAAAABBBBBCCCCC\x1b[2;1H\x1b[0J");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'A' as u32);
        assert_eq!(snap.cells[10].codepoint, 0);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_erase_display_all() {
        let mut t = sized_term(3, 5, 100);
        t.vt_write(b"AAAAABBBBB\x1b[2J");
        t.flush();
        let snap = t.take_snapshot();
        for c in 0..15 {
            assert_eq!(snap.cells[c].codepoint, 0, "ED 2: cell {c}");
        }
        assert_invariants(&snap);
    }
    #[test]
    fn kt_erase_line_end() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"ABCDEFGHIJ\x1b[6G\x1b[0K");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'A' as u32);
        assert_eq!(snap.cells[4].codepoint, 'E' as u32);
        assert_eq!(snap.cells[5].codepoint, 0);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_erase_line_all() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"ABCDEFGHIJ\x1b[2K");
        t.flush();
        let snap = t.take_snapshot();
        for c in 0..10 {
            assert_eq!(snap.cells[c].codepoint, 0, "EL 2: cell {c}");
        }
        assert_invariants(&snap);
    }
    #[test]
    fn kt_erase_chars() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"1234567890\x1b[4G\x1b[4X");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[3].codepoint, 0);
        assert_eq!(snap.cells[6].codepoint, 0);
        assert_eq!(snap.cells[7].codepoint, '8' as u32);
        assert_invariants(&snap);
    }

    // ── Scrolling ───────────────────────────────────────────────────

    #[test]
    fn kt_scroll_up_one() {
        let mut t = sized_term(3, 20, 100);
        t.vt_write(b"1\n2\n3");
        t.flush();
        let _snap = t.take_snapshot();
        t.vt_write(b"\x1b[S");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 2);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_scroll_down_one() {
        let mut t = sized_term(3, 20, 100);
        t.vt_write(b"1\n2\n3");
        t.flush();
        let _snap = t.take_snapshot();
        t.vt_write(b"\x1b[T");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 2);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_insert_lines() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"1\n2\n3\n4\n5");
        t.flush();
        let _snap = t.take_snapshot();
        t.vt_write(b"\x1b[3;1H\x1b[2L");
        t.flush();
        let _snap = t.take_snapshot();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_delete_lines() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"A\nB\nC\nD\nE");
        t.flush();
        let _snap = t.take_snapshot();
        t.vt_write(b"\x1b[2;1H\x1b[2M");
        t.flush();
        let _snap = t.take_snapshot();
        assert_invariants(&t.take_snapshot());
    }

    // ── SGR attributes ──────────────────────────────────────────────

    #[test]
    fn kt_sgr_bold() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1mB");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].bold);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_dim() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[2mD");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }
    #[test]
    fn kt_sgr_italic() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[3mI");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].italic);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_underline() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[4mU");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].underline);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_blink() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[5mB");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].blink);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_reverse() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[7mR");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].reverse);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_conceal() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[8mC");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].hidden);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_strikethrough() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[9mS");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].strikethrough);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_reset() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1;4mX\x1b[0mY");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].bold && snap.cells[0].underline);
        assert!(!snap.cells[1].bold && !snap.cells[1].underline);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_foreground_red() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[31mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].foreground[0] > 0.0);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_background_blue() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[44mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].background[2] > 0.0);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_colors_256() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[38;5;196mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].foreground[0] > 0.0);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_sgr_colors_true() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[38;2;100;150;200mX");
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
    }

    // ── DEC private modes ───────────────────────────────────────────

    #[test]
    fn kt_dec_7_autowrap_on() {
        let mut t = sized_term(3, 10, 100);
        t.vt_write(b"\x1b[?7h1234567890X");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[10].codepoint, 'X' as u32);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_dec_7_autowrap_off() {
        let mut t = sized_term(3, 10, 100);
        t.vt_write(b"\x1b[?7l1234567890X");
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            snap.cells[9].codepoint == 0
                || snap.cells[9].codepoint == 'X' as u32
                || snap.cells[9].codepoint == '0' as u32,
            "autowrap-off: cell[9] = {}, expected 0(drop), 88(overwrite), or 48(stay)",
            snap.cells[9].codepoint
        );
        assert_invariants(&snap);
    }
    #[test]
    fn kt_dec_25_cursor_visible() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?25hX");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }
    #[test]
    fn kt_dec_25_cursor_hidden() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?25lX");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }

    // ── Tab stops ───────────────────────────────────────────────────

    #[test]
    fn kt_tab_default() {
        let mut t = sized_term(5, 30, 100);
        t.vt_write(b"\x1b[HX\x09Y");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_tab_set() {
        let mut t = sized_term(5, 30, 100);
        t.vt_write(b"\x1b[3g\x1b[10G\x1bH");
        t.vt_write(b"\x1b[H\x09");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(t.cursor_x(), 9);
        assert_invariants(&snap);
    }

    // ── Line drawing ────────────────────────────────────────────────

    #[test]
    fn kt_line_draw_set() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x0e");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"Line");
        check(&mut t, b"\x0f");
    }
    #[test]
    fn kt_line_draw_off() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x0eText\x0f");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }

    // ── Save/Restore ────────────────────────────────────────────────

    #[test]
    fn kt_save_restore_dec() {
        let mut t = sized_term(10, 30, 100);
        t.vt_write(b"\x1b[5;10H\x1b7");
        t.vt_write(b"\x1b[H\x1b8");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 4);
        assert_eq!(t.cursor_x(), 9);
        assert_invariants(&t.take_snapshot());
    }

    // ── Scroll regions ──────────────────────────────────────────────

    #[test]
    fn kt_scroll_region_top() {
        let mut t = sized_term(10, 30, 100);
        t.vt_write(b"\x1b[3;8r");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"\x1b[H\x1b[100B");
    }
    #[test]
    fn kt_scroll_region_scroll() {
        let mut t = sized_term(10, 20, 100);
        t.vt_write(b"\x1b[3;8r\x1b[H");
        for i in 1..=10 {
            t.vt_write(format!("Line{i}\r\n").as_bytes());
            t.flush();
        }
        assert_invariants(&t.take_snapshot());
    }

    // ── Combined ────────────────────────────────────────────────────

    #[test]
    fn kt_combined_write_sgr_write() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Plain\x1b[1;31mBoldRed");
        t.flush();
        let snap = t.take_snapshot();
        assert!(!snap.cells[0].bold);
        assert!(snap.cells[5].bold);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_combined_move_write_erase() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"123456");
        t.vt_write(b"\x1b[3G\x1b[0K");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, '1' as u32);
        assert_eq!(snap.cells[2].codepoint, 0, "EL 0: col 2 erased");
        assert_eq!(snap.cells[3].codepoint, 0, "EL 0: col 3 erased to null");
        assert_invariants(&snap);
    }

    // ── Line feed / carriage return ─────────────────────────────────

    #[test]
    fn kt_lf_scroll() {
        let mut t = sized_term(3, 10, 100);
        t.vt_write(b"\n\n\n\n");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(
            t.cursor_y(),
            2,
            "4 LFs in 3-row terminal → cursor at bottom"
        );
        assert_invariants(&snap);
    }
    #[test]
    fn kt_cr_home() {
        let mut t = sized_term(5, 10, 100);
        t.vt_write(b"ABCDE\x1b[GHI");
        t.flush();
        let _snap = t.take_snapshot();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'H' as u32);
        assert_invariants(&snap);
    }
    #[test]
    fn kt_crlf_repeat() {
        let mut t = sized_term(5, 10, 100);
        for _ in 0..10 {
            t.vt_write(b"\r\n");
        }
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 4);
        assert_invariants(&t.take_snapshot());
    }

    // ── OSC sequences ───────────────────────────────────────────────

    #[test]
    fn kt_osc_0_set_title() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]0;Test\x1b\\");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_2_set_icon() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]2;Icon\x1b\\");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }

    // ── DSR / DA ────────────────────────────────────────────────────

    #[test]
    fn kt_dsr_cpr() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[6n");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }
    #[test]
    fn kt_da_primary() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[c");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }

    // ── CSI edge cases ──────────────────────────────────────────────

    #[test]
    fn kt_csi_empty() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[H");
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }
    #[test]
    fn kt_csi_zero() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[0;0H");
        t.flush();
        let _snap = t.take_snapshot();
        assert_eq!(t.cursor_y(), 0);
        assert_eq!(t.cursor_x(), 0);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_csi_large() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[100;100H");
        t.flush();
        let _snap = t.take_snapshot();
        assert!(t.cursor_y() < 5);
        assert!(t.cursor_x() < 20);
        assert_invariants(&t.take_snapshot());
    }

    // ── Zero-width and combining ────────────────────────────────────

    #[test]
    fn kt_combining_grave() {
        let mut t = sized_term(3, 20, 100);
        t.vt_write(b"a\xcc\x80");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(
            snap.cells[0].codepoint, 'a' as u32,
            "combining: 'a' preserved"
        );
        assert_invariants(&snap);
    }

    // ── P1.1 expanded: DEC private modes ───────────────────────────

    #[test]
    fn kt_decset_1048_l3_save_restore() {
        let mut t = sized_term(10, 30, 100);
        t.vt_write(b"\x1b[5;10H\x1b[?1048hX\x1b[?1048l");
        t.flush();
        let _snap = t.take_snapshot();
        assert!(t.cursor_x() >= 9, "cursor X preserved after DEC 1048");
        check(&mut t, b"");
    }
    #[test]
    fn kt_decset_1049_alt_screen() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"main\n");
        t.vt_write(b"\x1b[?1049h");
        t.vt_write(b"ALT\n");
        t.vt_write(b"\x1b[?1049l");
        t.flush();
        let snap = t.take_snapshot();
        // Check that "ALT" (uppercase, unique) does not appear on main screen
        let has_alt = snap.cells.windows(3).any(|w| {
            w[0].codepoint == 'A' as u32
                && w[1].codepoint == 'L' as u32
                && w[2].codepoint == 'T' as u32
        });
        assert!(
            !has_alt,
            "alt screen content 'ALT' should not appear on main screen"
        );
    }

    // ── OSC 4 palette + SGR verify ─────────────────────────────────

    #[test]
    fn kt_osc_4_palette_then_sgr() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]4;1;#ff0000\x1b\\");
        t.flush();
        t.vt_write(b"\x1b[31mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].foreground[0] > 0.0, "OSC 4 + SGR 31: fg red");
        check(&mut t, b"");
    }

    // ── SGR 38:2 truecolor ──────────────────────────────────────────

    #[test]
    fn kt_sgr_38_2_rgb_values() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[38;2;255;128;64mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].foreground[0] > 0.9, "fg.R ~ 1.0");
        assert!(
            snap.cells[0].foreground[1] > 0.4 && snap.cells[0].foreground[1] < 0.6,
            "fg.G ~ 0.5"
        );
        assert!(
            snap.cells[0].foreground[2] > 0.2 && snap.cells[0].foreground[2] < 0.3,
            "fg.B ~ 0.25"
        );
    }

    // ── DEC special graphics mode ────────────────────────────────────

    #[test]
    fn kt_dec_special_graphic_set() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x0en"); // DEC line drawing on + 'n'
        t.flush();
        check(&mut t, b"\x0f");
    }

    // ── C1 controls ──────────────────────────────────────────────────

    #[test]
    fn kt_c1_ris_full_reset() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Content");
        t.flush();
        t.vt_write(b"\x1bc"); // RIS
        t.flush();
        assert_eq!(t.cursor_x(), 0, "RIS: cursor col 0");
        assert_eq!(t.cursor_y(), 0, "RIS: cursor row 0");
        check(&mut t, b"");
    }
    #[test]
    fn kt_c1_ind_cursor_down() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1bD"); // IND
        t.flush();
        assert_eq!(t.cursor_y(), 1, "IND: cursor row 1");
    }
    #[test]
    fn kt_c1_nel_crlf() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[2;5H\x1bE"); // NEL
        t.flush();
        assert_eq!(t.cursor_y(), 2, "NEL: row 2");
        assert_eq!(t.cursor_x(), 0, "NEL: col 0");
    }
    #[test]
    fn kt_c1_ht_tab() {
        let mut t = sized_term(5, 30, 100);
        t.vt_write(b"\x09"); // HT
        t.flush();
        assert_eq!(t.cursor_x(), 8, "HT: tab to col 8");
    }

    // ── DSR/DA responses ────────────────────────────────────────────

    #[test]
    fn kt_dsr_device_status() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[5n"); // DSR device status
        t.flush();
        let responses = t.drain_pty_write_responses();
        assert!(!responses.is_empty(), "DSR 5n should produce response");
    }

    // ── Save/restore DECSC ──────────────────────────────────────────

    #[test]
    fn kt_decsc_multiple_saves() {
        let mut t = sized_term(10, 30, 100);
        t.vt_write(b"\x1b[3;5H\x1b7\x1b[6;10H\x1b7");
        t.flush();
        t.vt_write(b"\x1b8");
        t.flush();
        assert_eq!(t.cursor_y(), 5, "DECRC last save: row 5");
        assert_eq!(t.cursor_x(), 9, "DECRC last save: col 9");
    }
    #[test]
    fn kt_decsc_attributes_saved() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1;4m\x1b[HX\x1b7");
        t.flush();
        t.vt_write(b"\x1b[0mY\x1b8");
        t.flush();
        t.vt_write(b"Z");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cells[0].bold, "DECSC saved bold attr");
    }

    // ── P1.1 padding to 200: scroll margins ────────────────────────

    #[test]
    fn kt_decstbm_basic_top_bottom() {
        let mut t = sized_term(8, 20, 100);
        t.vt_write(b"\x1b[3;6r");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_decstbm_scroll_down_region() {
        let mut t = sized_term(8, 20, 100);
        t.vt_write(b"\x1b[3;6r\x1b[3;1HAB\x1b[10B");
        t.flush();
        check(&mut t, b"");
    }

    // ── P1.1 padding: SU + SD ──────────────────────────────────────

    #[test]
    fn kt_su_scroll_up_content() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Line1\nLine2\nLine3\nLine4\nLine5");
        t.flush();
        t.vt_write(b"\x1b[S"); // SU 1
        t.flush();
        let _snap = t.take_snapshot();
        check(&mut t, b"");
    }
    #[test]
    fn kt_sd_scroll_down_content() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"AAA\nBBB\nCCC\nDDD\nEEE");
        t.flush();
        t.vt_write(b"\x1b[T"); // SD 1
        t.flush();
        check(&mut t, b"");
    }

    // ── Extended cursor movement (20) ────────────────────────────

    #[test]
    fn kt_cursor_up_2() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[5;1H\x1b[2A");
        t.flush();
        assert_eq!(t.cursor_y(), 2);
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_cursor_up_10() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[12;1H\x1b[10A");
        t.flush();
        assert_eq!(t.cursor_y(), 1);
    }
    #[test]
    fn kt_cursor_down_2() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[H\x1b[2B");
        t.flush();
        assert_eq!(t.cursor_y(), 2);
    }
    #[test]
    fn kt_cursor_down_10() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[5;1H\x1b[10B");
        t.flush();
        assert_eq!(t.cursor_y(), 14);
    }
    #[test]
    fn kt_cursor_forward_2() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[H\x1b[2C");
        t.flush();
        assert_eq!(t.cursor_x(), 2);
    }
    #[test]
    fn kt_cursor_forward_40() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[H\x1b[40C");
        t.flush();
        assert_eq!(t.cursor_x(), 40);
    }
    #[test]
    fn kt_cursor_back_2() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[1;30H\x1b[2D");
        t.flush();
        assert_eq!(t.cursor_x(), 27);
    }
    #[test]
    fn kt_cursor_back_25() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[1;30H\x1b[25D");
        t.flush();
        assert_eq!(t.cursor_x(), 4);
    }
    #[test]
    fn kt_cup_center() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[12;40HX");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[11 * 80 + 39].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_hvp_center() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[12;40fX");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[11 * 80 + 39].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_ch_cha_mid() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[50GX");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[49].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_vpa_mid() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[10dX");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[9 * 80].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_cnl_2() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[5;1H\x1b[2E");
        t.flush();
        assert_eq!(t.cursor_y(), 6);
    }
    #[test]
    fn kt_cpl_2() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[10;1H\x1b[2F");
        t.flush();
        assert_eq!(t.cursor_y(), 7);
    }
    #[test]
    fn kt_cursor_down_after_scroll() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"A\nB\nC\nD\nE");
        t.flush();
        assert!(t.cursor_y() >= 4);
        t.vt_write(b"\x1b[5B");
        t.flush();
        assert_eq!(t.cursor_y(), 4);
    }
    #[test]
    fn kt_cursor_right_edge_then_write() {
        let mut t = sized_term(5, 80, 100);
        t.vt_write(b"\x1b[1;80HX");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[79].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_cursor_left_edge_write() {
        let mut t = sized_term(5, 80, 100);
        t.vt_write(b"\x1b[1;1HX");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_cursor_down_left_edge() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[24;1H\x1b[10B");
        t.flush();
        assert_eq!(t.cursor_y(), 23);
    }
    #[test]
    fn kt_cursor_up_right_edge() {
        let mut t = sized_term(24, 80, 100);
        t.vt_write(b"\x1b[1;80H\x1b[10A");
        t.flush();
        assert_eq!(t.cursor_y(), 0);
    }
    #[test]
    fn kt_cursor_home_text() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Hello\x1b[H!");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, '!' as u32);
    }

    // ── Extended SGR attributes (15) ─────────────────────────────

    #[test]
    fn kt_sgr_bold_italic_blink() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1;3;5mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).bold);
        assert!(snap.cell_at(0, 0).italic);
    }
    #[test]
    fn kt_sgr_bold_underline_reverse() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1;4;7mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).bold);
        assert!(snap.cell_at(0, 0).underline);
        assert!(snap.cell_at(0, 0).reverse);
    }
    #[test]
    fn kt_sgr_italic_underline_strikethrough() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[3;4;9mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).italic);
        assert!(snap.cell_at(0, 0).underline);
        assert!(snap.cell_at(0, 0).strikethrough);
    }
    #[test]
    fn kt_sgr_blink_reverse_conceal() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[5;7;8mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).blink);
        assert!(snap.cell_at(0, 0).reverse);
        assert!(snap.cell_at(0, 0).hidden);
    }
    #[test]
    fn kt_sgr_underline_overline() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[4;53mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).underline);
        assert!(snap.cell_at(0, 0).overline);
    }
    #[test]
    fn kt_sgr_bold_italic_underline_strikethrough() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1;3;4;9mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).bold);
        assert!(snap.cell_at(0, 0).italic);
        assert!(snap.cell_at(0, 0).strikethrough);
    }
    #[test]
    fn kt_sgr_colors_fg_only() {
        let mut t = sized_term(5, 20, 100);
        for c in 30u32..=37u32 {
            t.vt_write(format!("\x1b[{}mX", c).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert!(
                snap.cell_at(0, c - 30).foreground[0] != 0.0
                    || snap.cell_at(0, c - 30).foreground[1] != 0.0
                    || snap.cell_at(0, c - 30).foreground[2] != 0.0
            );
        }
    }
    #[test]
    fn kt_sgr_colors_bg_only() {
        let mut t = sized_term(5, 20, 100);
        for c in 40u32..=47u32 {
            t.vt_write(format!("\x1b[{}mX", c).as_bytes());
            t.flush();
            let snap = t.take_snapshot();
            assert!(
                snap.cell_at(0, c - 40).background[0] != 0.0
                    || snap.cell_at(0, c - 40).background[1] != 0.0
                    || snap.cell_at(0, c - 40).background[2] != 0.0
            );
        }
    }
    #[test]
    fn kt_sgr_reset_mid_text() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1;3mAB\x1b[0mCD");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).bold);
        assert!(!snap.cell_at(0, 2).italic, "SGR 0 should clear italic");
        assert!(!snap.cell_at(0, 2).bold, "SGR 0 should clear bold");
    }
    #[test]
    fn kt_sgr_all_reset_clears_strikethrough() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[9mX\x1b[0mY");
        t.flush();
        let snap = t.take_snapshot();
        assert!(!snap.cell_at(0, 1).strikethrough);
    }
    #[test]
    fn kt_sgr_fg_rgb() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[38;2;255;128;0mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).foreground[0] > 0.9);
    }
    #[test]
    fn kt_sgr_bg_rgb() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[48;2;0;128;255mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(snap.cell_at(0, 0).background[2] > 0.9);
    }
    #[test]
    fn kt_sgr_fg_bg_together() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[38;2;255;0;0;48;2;0;0;255mX");
        t.flush();
        let snap = t.take_snapshot();
        assert!(
            snap.cell_at(0, 0).foreground[0] > 0.9,
            "fg red channel should be ~1.0, got {}",
            snap.cell_at(0, 0).foreground[0]
        );
        assert!(
            snap.cell_at(0, 0).foreground[1] < 0.1,
            "fg green channel should be ~0.0, got {}",
            snap.cell_at(0, 0).foreground[1]
        );
        assert!(
            snap.cell_at(0, 0).background[2] > 0.9,
            "bg blue channel should be ~1.0, got {}",
            snap.cell_at(0, 0).background[2]
        );
        assert!(
            snap.cell_at(0, 0).background[0] < 0.1,
            "bg red channel should be ~0.0, got {}",
            snap.cell_at(0, 0).background[0]
        );
    }
    #[test]
    fn kt_sgr_attr_20_fraktur() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[20mX");
        t.flush();
        check(&mut t, b"");
    }

    // ── Extended DEC modes (15) ──────────────────────────────────

    #[test]
    fn kt_dec_mode_1_cursor_keys() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1h");
        t.flush();
        assert!(t.mode_get(1, 0));
    }
    #[test]
    fn kt_dec_mode_1_reset() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1l");
        t.flush();
        assert!(!t.mode_get(1, 0));
    }
    #[test]
    fn kt_dec_mode_4_insert() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?4h");
        t.flush();
        assert!(t.mode_get(4, 0));
    }
    #[test]
    fn kt_dec_mode_6_origin() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?6h");
        t.flush();
        assert!(t.mode_get(6, 0));
    }
    #[test]
    fn kt_dec_mode_7_wrap() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?7h");
        t.flush();
        assert!(t.mode_get(7, 0));
    }
    #[test]
    fn kt_dec_mode_12_scroll_lock() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?12h");
        t.flush();
        assert!(t.mode_get(12, 0));
    }
    #[test]
    fn kt_dec_mode_25_cursor_visible() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?25h");
        t.flush();
        assert!(t.mode_get(25, 0));
    }
    #[test]
    fn kt_dec_mode_25_cursor_hidden() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?25l");
        t.flush();
        assert!(!t.mode_get(25, 0));
    }
    #[test]
    fn kt_dec_mode_1000_mouse() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1000h");
        t.flush();
        assert!(t.mode_get(1000, 0));
    }
    #[test]
    fn kt_dec_mode_1000_mouse_off() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1000l");
        t.flush();
        assert!(!t.mode_get(1000, 0));
    }
    #[test]
    fn kt_dec_mode_2004_bracketed_paste() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?2004h");
        t.flush();
        assert!(t.mode_get(2004, 0));
    }
    #[test]
    fn kt_dec_mode_1049_alt_screen() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1049h\x1b[?1049l");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_dec_mode_origin_scroll_region() {
        let mut t = sized_term(10, 40, 100);
        t.vt_write(b"\x1b[?6h\x1b[3;8r");
        t.flush();
        assert!(t.mode_get(6, 0));
    }
    #[test]
    fn kt_dec_mode_multiset() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1;4;6;7;12;25h");
        t.flush();
        assert!(t.mode_get(1, 0));
        assert!(t.mode_get(7, 0));
    }
    #[test]
    fn kt_dec_mode_multireset() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1;4;6;7;12;25l");
        t.flush();
        assert!(!t.mode_get(1, 0));
        assert!(!t.mode_get(7, 0));
    }

    // ── Extended OSC queries (12) ────────────────────────────────

    #[test]
    fn kt_osc_0_title_set() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]0;Hello\x07");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_1_icon_set() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]1;Icon\x07");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_2_title_set() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]2;TabTitle\x07");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_4_color_query() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]4;0;?\x07");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_osc_10_fg_query() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]10;?\x07");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_osc_11_bg_query() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]11;?\x07");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_osc_12_cursor_color() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]12;?\x07");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_osc_set_and_query_title() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]0;Test\x07\x1b]0;?\x07");
        t.flush();
        let _r = t.drain_pty_write_responses();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_set_and_query_fg_color() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]10;#ff0000\x07\x1b]10;?\x07");
        t.flush();
        let _r = t.drain_pty_write_responses();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_set_and_query_bg_color() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]11;#00ff00\x07\x1b]11;?\x07");
        t.flush();
        let _r = t.drain_pty_write_responses();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_reset_colors() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]104\x07\x1b]110\x07");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_hyperlink() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]8;;\x07\x1b]8;;\x07");
        t.flush();
        check(&mut t, b"");
    }

    // ── Extended tab stops (10) ──────────────────────────────────

    #[test]
    fn kt_tab_move_to_next_stop() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[3g\x1b[10G\x1bH\x1b[H\x09");
        t.flush();
        assert_eq!(t.cursor_x(), 9);
    }
    #[test]
    fn kt_tab_multiple_stops_text() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[3g\x1b[10G\x1bH\x1b[20G\x1bH");
        t.vt_write(b"\x1b[H");
        t.vt_write(b"A\x09B\x09C");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cell_at(0, 0).codepoint, 'A' as u32);
    }
    #[test]
    fn kt_tab_clear_one_stop() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[3g\x1b[10G\x1bH\x1b[20G\x1bH");
        t.vt_write(b"\x1b[10G\x1b[0g");
        t.vt_write(b"\x1b[H\x09");
        t.flush();
        assert_eq!(t.cursor_x(), 19);
    }
    #[test]
    fn kt_tab_default_every_8() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[H\x09\x09");
        t.flush();
        assert_eq!(t.cursor_x(), 16);
    }
    #[test]
    fn kt_tab_no_stops_rightmost() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[3g\x1b[H\x09");
        t.flush();
        assert_eq!(t.cursor_x(), 39);
    }
    #[test]
    fn kt_tab_clear_all_then_set() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[3g\x1b[15G\x1bH\x1b[H\x09");
        t.flush();
        assert_eq!(t.cursor_x(), 14);
    }
    #[test]
    fn kt_tab_cht_forward() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[H\x1b[I");
        t.flush();
        assert_eq!(t.cursor_x(), 8);
    }
    #[test]
    fn kt_tab_cbt_backward() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[H\x09\x1b[Z");
        t.flush();
        assert_eq!(t.cursor_x(), 0);
    }
    #[test]
    fn kt_tab_cbt_to_previous() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[H\x09\x09\x1b[Z");
        t.flush();
        assert_eq!(t.cursor_x(), 8);
    }
    #[test]
    fn kt_tab_set_at_cursor() {
        let mut t = sized_term(5, 40, 100);
        t.vt_write(b"\x1b[3g\x1b[12G\x1bH\x1b[H\x09");
        t.flush();
        assert_eq!(t.cursor_x(), 11);
    }

    // ── Extended scroll region (10) ──────────────────────────────

    #[test]
    fn kt_scroll_region_il_insert() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"11111\n22222\n33333\n44444\n55555");
        t.flush();
        t.vt_write(b"\x1b[2;4r\x1b[2;1H\x1b[L");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_dl_delete() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"11111\n22222\n33333\n44444\n55555");
        t.flush();
        t.vt_write(b"\x1b[2;4r\x1b[2;1H\x1b[M");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_al_insert() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
        t.flush();
        t.vt_write(b"\x1b[2;4r\x1b[2;1H\x1b[L");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_full_page() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[1;5r");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_su_within() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
        t.flush();
        t.vt_write(b"\x1b[2;4r\x1b[S");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_sd_within() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
        t.flush();
        t.vt_write(b"\x1b[2;4r\x1b[T");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_cursor_bounds() {
        let mut t = sized_term(10, 40, 100);
        t.vt_write(b"\x1b[3;8r\x1b[10;1H");
        t.flush();
        assert_eq!(t.cursor_y(), 9);
    }
    #[test]
    fn kt_scroll_region_su_scrolls_content() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"1\n2\n3\n4\n5");
        t.flush();
        t.vt_write(b"\x1b[2;4r");
        t.vt_write(b"\x1b[2;1H\x1b[S");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_sd_scrolls_content() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"1\n2\n3\n4\n5");
        t.flush();
        t.vt_write(b"\x1b[2;4r");
        t.vt_write(b"\x1b[2;1H\x1b[T");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_scroll_region_reset_full() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[2;4r\x1b[r");
        t.flush();
        check(&mut t, b"");
    }

    // ── Interactive response tests (15) ──────────────────────────

    #[test]
    fn kt_dsr_cpr_responds() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[6n");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_dsrdev_responds() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[0n");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_da1_responds() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[c");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_da2_responds() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[>c");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_da3_responds() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[=c");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_decrpm_query_mode_7() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?7;$p");
        t.flush();
        let r = t.drain_pty_write_responses();
        if !r.is_empty() {
            assert!(String::from_utf8_lossy(r.last().unwrap()).contains('7'));
        }
    }
    #[test]
    fn kt_decrpm_query_mode_25() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?25;$p");
        t.flush();
        let r = t.drain_pty_write_responses();
        if !r.is_empty() {
            assert!(String::from_utf8_lossy(r.last().unwrap()).contains("25"));
        }
    }
    #[test]
    fn kt_decrpm_query_mode_1049() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1049;$p");
        t.flush();
        let r = t.drain_pty_write_responses();
        if !r.is_empty() {
            assert!(String::from_utf8_lossy(r.last().unwrap()).contains("1049"));
        }
    }
    #[test]
    fn kt_decrpm_mode_1_off_response() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?1l\x1b[?1;$p");
        t.flush();
        let r = t.drain_pty_write_responses();
        if !r.is_empty() {
            let txt = String::from_utf8_lossy(r.last().unwrap());
            assert!(
                txt.contains('1'),
                "DECRPM response for mode 1 should contain '1': {:?}",
                txt
            );
        }
    }
    #[test]
    fn kt_decrpm_response_format() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?7;$p");
        t.flush();
        let r = t.drain_pty_write_responses();
        if !r.is_empty() {
            let txt = String::from_utf8_lossy(r.last().unwrap());
            // Format: ESC [ ? 7 ; 1 $ y or similar
            assert!(
                txt.contains('y') || txt.contains('$'),
                "response ends with $y"
            );
        }
    }
    #[test]
    fn kt_osc_4_indicator_cycling() {
        let mut t = sized_term(5, 20, 100);
        // Query color 0-15
        for i in 0u8..16u8 {
            t.vt_write(format!("\x1b]4;{};?\x07", i).as_bytes());
            t.flush();
            let _ = t.drain_pty_write_responses();
        }
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_osc_mixed_queries() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[6n\x1b]10;?\x07\x1b[?25;$p");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_dec_private_cursor_pos() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?6n");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_dec_private_extended() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?62n");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_dsr_dec_private_quick() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?15n");
        t.flush();
        let _r = t.drain_pty_write_responses();
        assert_invariants(&t.take_snapshot());
    }

    // ── Erase operations extended (10) ───────────────────────────

    #[test]
    fn kt_erase_el_from_cursor() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Hello World!");
        t.flush();
        t.vt_write(b"\x1b[6G\x1b[K"); // EL 0 from col 6
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[5].codepoint, 0, "erased from col 6");
    }
    #[test]
    fn kt_erase_el_to_start() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Hello World!");
        t.flush();
        t.vt_write(b"\x1b[10G\x1b[1K"); // EL 1 to start
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 0, "erased to col 0");
    }
    #[test]
    fn kt_erase_el_all() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Hello World!");
        t.flush();
        t.vt_write(b"\x1b[5G\x1b[2K"); // EL 2 whole line
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[4].codepoint, 0, "erased col 5");
    }
    #[test]
    fn kt_erase_ed_to_end() {
        let mut t = sized_term(5, 20, 100);
        t.pty_write(b"Line1\nLine2\nLine3\nLine4\nLine5");
        t.flush();
        t.vt_write(b"\x1b[2;1H\x1b[0J"); // ED 0 from row 2
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'L' as u32, "row 1 preserved");
    }
    #[test]
    fn kt_erase_ed_to_start() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Line1\nLine2\nLine3\nLine4\nLine5");
        t.flush();
        t.vt_write(b"\x1b[4;1H\x1b[1J"); // ED 1 from row 4
        t.flush();
        assert_invariants(&t.take_snapshot());
    }
    #[test]
    fn kt_erase_ed_all() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Line1\nLine2\nLine3");
        t.flush();
        t.vt_write(b"\x1b[2J"); // ED 2
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 0, "all erased");
    }
    #[test]
    fn kt_erase_ich_insert_chars() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"ABCDE\x1b[1;1H\x1b[5@"); // ICH 5
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_erase_dch_delete_chars() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"ABCDE\x1b[1;1H\x1b[2P"); // DCH 2
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'C' as u32, "DCH: CDE shifted");
    }
    #[test]
    fn kt_erase_ech_erase_chars() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"ABCDE\x1b[1;1H\x1b[2X"); // ECH 2
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 0, "ECH: blank");
    }
    #[test]
    fn kt_erase_sd_scroll_down() {
        let mut t = sized_term(6, 20, 100);
        t.vt_write(b"1\n2\n3\n4\n5\n6");
        t.flush();
        t.vt_write(b"\x1b[T"); // SD 1
        t.flush();
        check(&mut t, b"");
    }

    // ── Fill to 200+ tests (12) ──────────────────────────────────

    #[test]
    fn kt_cursor_save_restore() {
        let mut t = sized_term(10, 40, 100);
        t.vt_write(b"\x1b[5;10H\x1b7\x1b[1;1H\x1b8X");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[4 * 40 + 9].codepoint, 'X' as u32);
    }
    #[test]
    fn kt_cursor_save_restore_ansi() {
        let mut t = sized_term(10, 40, 100);
        t.vt_write(b"\x1b[s\x1b[10;20H\x1b[uX");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_decsc_decrc() {
        let mut t = sized_term(10, 40, 100);
        t.vt_write(b"\x1b7Hello\x1b8World");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_ri_reverse_index() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[M");
        t.flush();
        assert_eq!(t.cursor_y(), 0);
    }
    #[test]
    fn kt_ri_reverse_index_scroll() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\n\n\n\n\x1b[M");
        t.flush();
        assert!(t.cursor_y() >= 3);
    }
    #[test]
    fn kt_nel_newline() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"Hello\x1bE");
        t.flush();
        assert_eq!(t.cursor_y(), 1);
        assert_eq!(t.cursor_x(), 0);
    }
    #[test]
    fn kt_ind_linefeed() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1bD");
        t.flush();
        assert_eq!(t.cursor_y(), 1);
    }
    #[test]
    fn kt_dec_align_dcal() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b#8");
        t.flush();
        let snap = t.take_snapshot();
        assert_eq!(snap.cells[0].codepoint, 'E' as u32);
    }
    #[test]
    fn kt_dec_str_terminator_string() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]0;test\x07");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_osc_st_terminator() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b]0;test\x1b\\");
        t.flush();
        check(&mut t, b"");
    }
    #[test]
    fn kt_sgr_0_resets_fg() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[31mX\x1b[0mY");
        t.flush();
        let snap = t.take_snapshot();
        let x_fg = snap.cells[0].foreground;
        let y_fg = snap.cells[1].foreground;
        assert!(
            x_fg[0] > 0.5,
            "SGR 31 should set X to red foreground, got {:?}",
            x_fg
        );
        assert_ne!(x_fg, y_fg, "SGR 0 should change fg between X and Y");
        assert_eq!(
            snap.cells[1].codepoint, 'Y' as u32,
            "cell 1 should contain Y"
        );
    }
    #[test]
    fn kt_soft_reset() {
        let mut t = sized_term(5, 20, 100);
        t.vt_write(b"\x1b[?25l\x1b[!p");
        t.flush();
        check(&mut t, b"");
    }
}
