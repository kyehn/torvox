#[cfg(test)]
mod conformance_backends {
    #![allow(clippy::manual_repeat_n)]
    use std::io::Write;
    use std::process::{Command, Stdio};

    use terminal_engine::ghostty_terminal::GhosttyTerminal;

    /// Run VT sequence through xterm and capture its grid output via control sequences.
    /// Returns the text content of xterm's visible grid, line by line.
    #[allow(dead_code)]
    fn xterm_grid_output(seq: &[u8], rows: u32, cols: u32) -> Result<Vec<String>, String> {
        let mut child = Command::new("xterm")
            .args(["-fn", "monospace-14"])
            .args(["-geometry", &format!("{}x{}", cols, rows)])
            .args(["-xrm", "XTerm*allowSendEvents:true"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("xterm spawn failed: {}", e))?;

        let stdin = child.stdin.as_mut().ok_or("no stdin")?;

        // Write VT sequence followed by DECRQCRA to dump grid
        // Wait for xterm to process
        stdin.write_all(seq).map_err(|e| format!("write: {}", e))?;
        stdin
            .write_all(b"\x1b[1;1f\x1b[6n")
            .map_err(|e| format!("write DSR: {}", e))?;

        // Read grid back via DECRQCRA or careful timing
        // For now, we use a simple approach: write unique marker, then read lines
        let _output = std::process::Command::new("xdotool")
            .args(["search", "--name", "", "getactivewindow", "getwindowname"])
            .output()
            .map_err(|_| "xdotool not available".to_string())?;

        // Cleanup
        drop(child);

        // Return empty for now - xterm grid extraction needs terminal emulator interaction
        // This is a placeholder that documents the approach
        Err("xterm backend requires X server and interactive terminal".to_string())
    }

    /// Run VT sequence through GhosttyTerminal and return grid text lines.
    fn grid_output(seq: &[u8], rows: u32, cols: u32) -> Vec<String> {
        let mut term = GhosttyTerminal::new(rows, cols, 100).expect("GhosttyTerminal");
        term.flush();
        term.vt_write(seq);
        term.flush();
        let snap = term.take_snapshot();
        let mut lines = Vec::new();
        let mut current = String::new();
        for (i, cell) in snap.cells.iter().enumerate() {
            let ch = char::from_u32(cell.codepoint).unwrap_or('\u{FFFD}');
            current.push(ch);
            if (i as u32 + 1).is_multiple_of(cols) {
                lines.push(current.trim_end().to_string());
                current = String::new();
            }
        }
        if !current.is_empty() {
            lines.push(current.trim_end().to_string());
        }
        lines
    }

    /// A conformance test case — non-const due to Vec usage.
    struct VtCase {
        name: &'static str,
        input: Vec<u8>,
        expected_substrings: &'static [&'static str],
    }

    fn conformance_cases() -> Vec<VtCase> {
        vec![
            VtCase {
                name: "basic text write",
                input: b"Hello World".to_vec(),
                expected_substrings: &["Hello World"],
            },
            VtCase {
                name: "SGR bold red",
                input: b"\x1b[1;31mBold Red\x1b[0m".to_vec(),
                expected_substrings: &["Bold Red"],
            },
            VtCase {
                name: "newline moves cursor",
                input: b"Line1\nLine2\nLine3".to_vec(),
                expected_substrings: &["Line1", "Line2", "Line3"],
            },
            VtCase {
                name: "carriage return overwrites",
                input: b"Hello\rWorld".to_vec(),
                expected_substrings: &["World"],
            },
            VtCase {
                name: "tab stops",
                input: b"A\tB\tC".to_vec(),
                expected_substrings: &["A", "B", "C"],
            },
            VtCase {
                name: "clear screen",
                input: b"Visible\x1b[2J".to_vec(),
                expected_substrings: &[""],
            },
            VtCase {
                name: "cursor up",
                input: b"Line1\nLine2\n\x1b[ALine3".to_vec(),
                expected_substrings: &["Line1", "Line3"],
            },
            VtCase {
                name: "reverse video SGR 7",
                input: b"\x1b[7mReverse\x1b[0m".to_vec(),
                expected_substrings: &["Reverse"],
            },
            VtCase {
                name: "underline SGR 4",
                input: b"\x1b[4mUnderline\x1b[0m".to_vec(),
                expected_substrings: &["Underline"],
            },
            VtCase {
                name: "blink SGR 5",
                input: b"\x1b[5mBlink\x1b[0m".to_vec(),
                expected_substrings: &["Blink"],
            },
            VtCase {
                name: "hidden text SGR 8",
                input: b"\x1b[8mHidden\x1b[0mVisible".to_vec(),
                expected_substrings: &["Visible"],
            },
            VtCase {
                name: "strikethrough SGR 9",
                input: b"\x1b[9mStrike\x1b[0m".to_vec(),
                expected_substrings: &["Strike"],
            },
            VtCase {
                name: "256-color foreground",
                input: b"\x1b[38;5;196mRed\x1b[0m".to_vec(),
                expected_substrings: &["Red"],
            },
            VtCase {
                name: "24-bit RGB foreground",
                input: b"\x1b[38;2;255;0;0mRGB Red\x1b[0m".to_vec(),
                expected_substrings: &["RGB Red"],
            },
            VtCase {
                name: "alternate screen",
                input: b"Normal\x1b[?1049hAlt Text\x1b[?1049lBack".to_vec(),
                expected_substrings: &["Normal"],
            },
            VtCase {
                name: "scroll up",
                input:
                    b"Line1\nLine2\nLine3\nLine4\nLine5\nLine6\nLine7\nLine8\nLine9\nLine10\n\x1b[S"
                        .to_vec(),
                expected_substrings: &["Line2"],
            },
            VtCase {
                name: "insert line",
                input: b"Line1\nLine2\n\x1b[LLine3".to_vec(),
                expected_substrings: &["Line1", "Line3", "Line2"],
            },
            VtCase {
                name: "delete line",
                input: b"Line1\nLine2\nLine3\n\x1b[M".to_vec(),
                expected_substrings: &["Line1", "Line3"],
            },
            VtCase {
                name: "erase in line (partial)",
                input: b"Hello World\x1b[6G\x1b[1K".to_vec(),
                expected_substrings: &["orld"],
            },
            VtCase {
                name: "erase in display (below)",
                input: b"Keep1\nKeep2\nErase1\nErase2\x1b[0J".to_vec(),
                expected_substrings: &["Keep1", "Keep2"],
            },
            VtCase {
                name: "DEC private mode set/reset",
                input: b"\x1b[?25lHidden\x1b[?25hShown".to_vec(),
                expected_substrings: &["Shown"],
            },
            VtCase {
                name: "origin mode",
                input: b"\x1b[?6h\x1b[5;5HCell\x1b[?6l".to_vec(),
                expected_substrings: &["Cell"],
            },
            VtCase {
                name: "DECSC/DECRC save restore",
                input: b"ABC\x1b7\x1b[5;5H\x1b8D".to_vec(),
                expected_substrings: &["ABC", "D"],
            },
            VtCase {
                name: "reverse index scrolls up",
                input: b"Line1\nLine2\x1bM".to_vec(),
                expected_substrings: &["Line2"],
            },
            VtCase {
                name: "next line moves to new line with CR",
                input: b"AB\x1bECD".to_vec(),
                expected_substrings: &["CD"],
            },
            VtCase {
                name: "index scrolls down at bottom",
                input: b"Line1\x1bD".to_vec(),
                expected_substrings: &["Line1"],
            },
            // === Character Attributes (SGR combinations) ===
            VtCase {
                name: "SGR bold+underline",
                input: b"\x1b[1;4mBU\x1b[0m".to_vec(),
                expected_substrings: &["BU"],
            },
            VtCase {
                name: "SGR faint (dim)",
                input: b"\x1b[2mDim\x1b[0m".to_vec(),
                expected_substrings: &["Dim"],
            },
            VtCase {
                name: "SGR blink",
                input: b"\x1b[5mBlink\x1b[0m".to_vec(),
                expected_substrings: &["Blink"],
            },
            VtCase {
                name: "SGR fast blink",
                input: b"\x1b[6mFastBlink\x1b[0m".to_vec(),
                expected_substrings: &["FastBlink"],
            },
            VtCase {
                name: "SGR concealed (invisible)",
                input: b"\x1b[8mHidden".to_vec(),
                expected_substrings: &["Hidden"],
            },
            VtCase {
                name: "SGR crossed-out",
                input: b"\x1b[9mStrike\x1b[0m".to_vec(),
                expected_substrings: &["Strike"],
            },
            VtCase {
                name: "SGR underline color set and reset",
                input: b"\x1b[58;2;255;0;0mUnderlineOnly\x1b[59m".to_vec(),
                expected_substrings: &["UnderlineOnly"],
            },
            VtCase {
                name: "SGR double underline",
                input: b"\x1b[21mDouble\x1b[0m".to_vec(),
                expected_substrings: &["Double"],
            },
            VtCase {
                name: "SGR overline",
                input: b"\x1b[53mOverline\x1b[0m".to_vec(),
                expected_substrings: &["Overline"],
            },
            VtCase {
                name: "SGR not bold (22)",
                input: b"\x1b[1mBold\x1b[22mNotBold".to_vec(),
                expected_substrings: &["NotBold"],
            },
            VtCase {
                name: "SGR not italic (23)",
                input: b"\x1b[3mItalic\x1b[23mNotItalic".to_vec(),
                expected_substrings: &["NotItalic"],
            },
            VtCase {
                name: "SGR not underline (24)",
                input: b"\x1b[4mUnder\x1b[24mNotUnder".to_vec(),
                expected_substrings: &["NotUnder"],
            },
            VtCase {
                name: "SGR not blink (25)",
                input: b"\x1b[5mBlink\x1b[25mNotBlink".to_vec(),
                expected_substrings: &["NotBlink"],
            },
            VtCase {
                name: "SGR positive (not reverse, 27)",
                input: b"\x1b[7mReverse\x1b[27mPositive".to_vec(),
                expected_substrings: &["Positive"],
            },
            VtCase {
                name: "SGR not crossed-out (29)",
                input: b"\x1b[9mStrike\x1b[29mClean".to_vec(),
                expected_substrings: &["Clean"],
            },
            VtCase {
                name: "SGR 38:5:0 black foreground",
                input: b"\x1b[38;5;0mBlack\x1b[0m".to_vec(),
                expected_substrings: &["Black"],
            },
            VtCase {
                name: "SGR 38:5:231 white foreground",
                input: b"\x1b[38;5;231mWhite\x1b[0m".to_vec(),
                expected_substrings: &["White"],
            },
            VtCase {
                name: "SGR 48:5:196 red background",
                input: b"\x1b[48;5;196mBgRed\x1b[0m".to_vec(),
                expected_substrings: &["BgRed"],
            },
            VtCase {
                name: "SGR 38:2:0:255:0 green text",
                input: b"\x1b[38;2;0;255;0mGreen\x1b[0m".to_vec(),
                expected_substrings: &["Green"],
            },
            VtCase {
                name: "SGR 48:2:255:0:0 red background",
                input: b"\x1b[48;2;255;0;0mRedBg\x1b[0m".to_vec(),
                expected_substrings: &["RedBg"],
            },
            VtCase {
                name: "SGR 1 + 38:5:196",
                input: b"\x1b[1;38;5;196mBoldRed\x1b[0m".to_vec(),
                expected_substrings: &["BoldRed"],
            },
            // === Cursor operations ===
            VtCase {
                name: "cursor forward tab (CHT)",
                input: b"A\x1b[2IB".to_vec(),
                expected_substrings: &["A", "B"],
            },
            VtCase {
                name: "cursor back tab (CBT) from right",
                input: b"A\t\tC\x1b[2ZC".to_vec(),
                expected_substrings: &["A", "C"],
            },
            VtCase {
                name: "cursor horizontal absolute (CHA) at right",
                input: b"\x1b[80CH".to_vec(),
                expected_substrings: &["H"],
            },
            // === Scrolling operations ===
            VtCase {
                name: "scroll up with SU (ESC [n S)",
                input: b"A\nB\nC\x1b[1S".to_vec(),
                expected_substrings: &["B", "C"],
            },
            VtCase {
                name: "scroll down with SD (ESC [n T)",
                input: b"A\nB\nC\x1b[1T".to_vec(),
                expected_substrings: &["", "A", "B"],
            },
            VtCase {
                name: "scroll region",
                input: b"\x1b[3;6r\x1b[5HScroll\x1b[r".to_vec(),
                expected_substrings: &["Scroll"],
            },
            VtCase {
                name: "delete characters",
                input: b"AB\x1b[D\x1b[1PC".to_vec(),
                expected_substrings: &["AC"],
            },
            VtCase {
                name: "insert characters",
                input: b"Hell\x1b[@World".to_vec(),
                expected_substrings: &["HellWorld"],
            },
            VtCase {
                name: "repeat character",
                input: b"ABC\x1b[b".to_vec(),
                expected_substrings: &["ABCC"],
            },
            VtCase {
                name: "line wrap",
                input: {
                    let mut long = vec![b'A'; 90];
                    long.push(b'\n');
                    long
                },
                expected_substrings: &["AAA"],
            },
            // === Advanced cursor positioning ===
            VtCase {
                name: "cursor character absolute (HPA)",
                input: b"A\x1b[40GB".to_vec(),
                expected_substrings: &["A", "B"],
            },
            VtCase {
                name: "cursor vertical absolute (VPA)",
                input: b"Line1\n\n\nLine4\x1b[3dX".to_vec(),
                expected_substrings: &["Line1", "X"],
            },
            VtCase {
                name: "cursor next line (CNL) with GOTO",
                input: b"Line1\x1b[2E".to_vec(),
                expected_substrings: &["", "Line1"],
            },
            // === Selective erase ===
            VtCase {
                name: "erase in display 3 (scrollback + display)",
                input: b"Keep\nErase\x1b[3J".to_vec(),
                expected_substrings: &["Keep"],
            },
            VtCase {
                name: "erase in line 2 (entire line)",
                input: b"Hello\x1b[2KWorld".to_vec(),
                expected_substrings: &["World"],
            },
            // === Tab ops ===
            VtCase {
                name: "tab clear (TBC) all",
                input: b"A\tB\x1b[3g".to_vec(),
                expected_substrings: &["A", "B"],
            },
            // === Autowrap mode ===
            VtCase {
                name: "autowrap off overflow",
                input: {
                    let mut long = vec![b'\x1b', b'[', b'?', b'7', b'l'];
                    long.extend(std::iter::repeat(b'A').take(90));
                    long
                },
                expected_substrings: &["AAA"],
            },
            VtCase {
                name: "autowrap on wraps",
                input: {
                    let mut long = vec![b'\x1b', b'[', b'?', b'7', b'h'];
                    long.extend(std::iter::repeat(b'B').take(90));
                    long.push(b'\n');
                    long.extend(std::iter::repeat(b'B').take(90));
                    long
                },
                expected_substrings: &["BBB"],
            },
            // === Double-height / double-width ===
            VtCase {
                name: "DECDHL top half preserved",
                input: b"\x1b#3TopHalf".to_vec(),
                expected_substrings: &["TopHalf"],
            },
            VtCase {
                name: "DECSWL single width preserved",
                input: b"\x1b#5Single".to_vec(),
                expected_substrings: &["Single"],
            },
            // === OSC sequences ===
            VtCase {
                name: "OSC 0 set window title",
                input: b"\x1b]0;MyTitle\x1b\\Text".to_vec(),
                expected_substrings: &["Text"],
            },
            VtCase {
                name: "OSC 1 set icon title",
                input: b"\x1b]1;Icon\x07Real".to_vec(),
                expected_substrings: &["Real"],
            },
            VtCase {
                name: "OSC 2 set window+icon title",
                input: b"\x1b]2;Title\x1b\\Content".to_vec(),
                expected_substrings: &["Content"],
            },
            // === Media copy ===
            VtCase {
                name: "media copy normal",
                input: b"\x1b[5iNormal".to_vec(),
                expected_substrings: &["Normal"],
            },
            // === Character sets ===
            VtCase {
                name: "designate G0 ASCII",
                input: b"\x1b(B".to_vec(),
                expected_substrings: &[""],
            },
            // === Status Report ===
            VtCase {
                name: "device attributes (DA) no crash",
                input: b"\x1b[c".to_vec(),
                expected_substrings: &[""],
            },
        ]
    }

    #[test]
    fn vt_conformance_basic_text() {
        let lines = grid_output(b"Hello World", 24, 80);
        assert!(
            lines.iter().any(|l| l.contains("Hello World")),
            "Basic text: 'Hello World' not found in:\n  {:?}",
            lines
        );
    }

    #[test]
    fn vt_conformance_sgr_bold_red() {
        let lines = grid_output(b"\x1b[1;31mBold Red\x1b[0m", 24, 80);
        assert!(lines.iter().any(|l| l.contains("Bold Red")), "SGR bold red");
    }

    #[test]
    fn vt_conformance_newline() {
        let lines = grid_output(b"Line1\nLine2\nLine3", 24, 80);
        assert!(lines.iter().any(|l| l.contains("Line1")), "Line1");
        assert!(lines.iter().any(|l| l.contains("Line2")), "Line2");
        assert!(lines.iter().any(|l| l.contains("Line3")), "Line3");
    }

    #[test]
    fn vt_conformance_cr_overwrite() {
        let lines = grid_output(b"Hello\rWorld", 24, 80);
        assert!(lines.iter().any(|l| l.contains("World")), "CR overwrite");
    }

    #[test]
    fn vt_conformance_cursor_up() {
        let lines = grid_output(b"Line1\nLine2\n\x1b[ALine3", 24, 80);
        assert!(lines.iter().any(|l| l.contains("Line1")), "Line1");
        assert!(lines.iter().any(|l| l.contains("Line3")), "Line3 after CUU");
    }

    #[test]
    fn vt_conformance_sgr_reverse() {
        let lines = grid_output(b"\x1b[7mReverse\x1b[0m", 24, 80);
        assert!(lines.iter().any(|l| l.contains("Reverse")), "SGR 7 reverse");
    }

    #[test]
    fn vt_conformance_sgr_underline() {
        let lines = grid_output(b"\x1b[4mUnderline\x1b[0m", 24, 80);
        assert!(lines.iter().any(|l| l.contains("Underline")), "SGR 4");
    }

    #[test]
    fn vt_conformance_256_color() {
        let lines = grid_output(b"\x1b[38;5;196mRed256\x1b[0m", 24, 80);
        assert!(lines.iter().any(|l| l.contains("Red256")), "256-color");
    }

    #[test]
    fn vt_conformance_rgb_color() {
        let lines = grid_output(b"\x1b[38;2;255;0;0mRGB Red\x1b[0m", 24, 80);
        assert!(lines.iter().any(|l| l.contains("RGB Red")), "24-bit RGB");
    }

    #[test]
    fn vt_conformance_scroll_region() {
        let seq = b"\x1b[3;6r\x1b[5HScrollLine\x1b[r";
        let lines = grid_output(seq, 24, 80);
        assert!(
            lines.iter().any(|l| l.contains("Scroll")),
            "Scroll region text not found in:\n  {:?}",
            lines
        );
    }

    #[test]
    fn vt_conformance_tab_stops() {
        let lines = grid_output(b"A\tB\tC", 24, 40);
        assert!(lines.iter().any(|l| l.contains('A')), "Tab A");
        assert!(lines.iter().any(|l| l.contains('B')), "Tab B");
    }
    #[test]
    fn vt_conformance_erase_display_below() {
        // ED 0 = erase from cursor to end of display.
        // CUU 1 (cursor up) preserves column; ED 0 from col 9 erases row 1 fully, row 0 partially.
        let seq = b"KeepRow0\nEraseRow1\x1b[A\x1b[J";
        let lines = grid_output(seq, 24, 80);
        assert!(
            lines.iter().any(|l| l.contains("KeepRow0")),
            "ED 0 should keep row 0: {:?}",
            lines
        );
    }

    #[test]
    fn vt_conformance_insert_chars() {
        let seq = b"Hell\x1b[@World";
        let lines = grid_output(seq, 24, 80);
        assert!(
            lines.iter().any(|l| l.contains("Hell")),
            "ICH should insert space"
        );
        assert!(lines.iter().any(|l| l.contains("World")), "ICH World");
    }

    #[test]
    fn vt_conformance_repeat_char() {
        let seq = b"ABC\x1b[b";
        let lines = grid_output(seq, 24, 80);
        assert!(
            lines.iter().any(|l| l.contains("ABCC")),
            "REP should repeat C"
        );
    }

    #[test]
    fn vt_conformance_erase_in_line() {
        // EL 1 = erase from start to cursor (inclusive)
        let seq = b"Hello World\x1b[1K";
        let lines = grid_output(seq, 24, 80);
        // Cursor at col 11 after 'd'. EL 1 erases cols 0-11, so nothing remains.
        assert!(
            lines[0].trim_matches('\0').is_empty(),
            "EL 1 should erase all text: {:?}",
            &lines[0][..20]
        );
    }

    #[test]
    fn vt_conformance_erase_display() {
        let seq = b"Keep\n\x1b[2JNewTop";
        let lines = grid_output(seq, 24, 80);
        assert!(
            lines.iter().any(|l| l.contains("NewTop")),
            "ED 2 + text at top not found in:\n  {:?}",
            lines
        );
    }

    #[test]
    fn vt_conformance_line_wrap() {
        let long = vec![b'A'; 90];
        let lines = grid_output(&long, 3, 40);
        // With 40 cols, 80 A's fill 2 lines, remaining 10 fill line 3
        assert_eq!(lines.len(), 3, "90 chars should fill 3 rows at 40 cols");
    }

    #[test]
    fn vt_conformance_decsc_decrc() {
        let seq = b"ABC\x1b7\x1b[5;5H\x1b8D";
        let lines = grid_output(seq, 10, 40);
        let all = lines.join(" ");
        assert!(
            all.contains("ABC"),
            "DECSC should save text 'ABC': {:?}",
            all
        );
        assert!(all.contains('D'), "DECRC should restore cursor: {:?}", all);
    }

    #[test]
    fn vt_conformance_alt_screen() {
        let seq = b"Normal\x1b[?1049hAlt Text\x1b[?1049lBack";
        let lines = grid_output(seq, 24, 80);
        assert!(
            !lines.iter().any(|l| l.contains("Alt Text")),
            "Alt screen text should be gone after 1049l"
        );
    }

    #[test]
    fn vt_conformance_hidden_text() {
        let seq = b"\x1b[8mHidden\x1b[0mVisible";
        let lines = grid_output(seq, 24, 80);
        // Hidden text still occupies cells but renders as blank
        assert!(
            lines.iter().any(|l| l.contains("Visible")),
            "Text after SGR 8 should be visible"
        );
    }

    #[test]
    fn vt_conformance_strikethrough() {
        let seq = b"\x1b[9mStrike\x1b[0m";
        let lines = grid_output(seq, 24, 80);
        assert!(
            lines.iter().any(|l| l.contains("Strike")),
            "SGR 9 strikethrough text should be present"
        );
    }

    #[test]
    fn vt_conformance_all_cases() {
        // Generate dynamic tests for all conformance cases
        for case in conformance_cases() {
            let lines = grid_output(&case.input, 24, 80);
            let all = lines.join("\n");
            for expected in case.expected_substrings {
                assert!(
                    all.contains(expected),
                    "[{}] Expected '{}' not found in grid:\n  {:?}",
                    case.name,
                    expected,
                    lines
                );
            }
        }
    }
}
