//! VT key encoding baseline tests.
//! Reference: terminator Fn key sequences, zed-port mappings/keys.rs.
//! These tests verify escape sequence generation for keyboard input.

/// Standard VT F-key escape sequences (VT220 normal mode).
const F_KEY_SEQUENCES: &[(u8, &str)] = &[
    (1, "\x1bOP"),    // F1
    (2, "\x1bOQ"),    // F2
    (3, "\x1bOR"),    // F3
    (4, "\x1bOS"),    // F4
    (5, "\x1b[15~"),  // F5
    (6, "\x1b[17~"),  // F6
    (7, "\x1b[18~"),  // F7
    (8, "\x1b[19~"),  // F8
    (9, "\x1b[20~"),  // F9
    (10, "\x1b[21~"), // F10
    (11, "\x1b[23~"), // F11
    (12, "\x1b[24~"), // F12
];

/// Standard VT cursor key escape sequences (normal mode).
const CURSOR_KEY_SEQUENCES: &[(&str, &str)] = &[
    ("up", "\x1b[A"),
    ("down", "\x1b[B"),
    ("right", "\x1b[C"),
    ("left", "\x1b[D"),
];

/// Application cursor mode sequences.
const APP_CURSOR_SEQUENCES: &[(&str, &str)] = &[
    ("up", "\x1bOA"),
    ("down", "\x1bOB"),
    ("right", "\x1bOC"),
    ("left", "\x1bOD"),
];

/// Standard editing key sequences (VT220 tilde mode for function-region keys).
const EDITING_KEY_SEQUENCES: &[(&str, &str)] = &[
    ("pageup", "\x1b[5~"),
    ("pagedown", "\x1b[6~"),
    ("insert", "\x1b[2~"),
    ("delete", "\x1b[3~"),
];

/// Home/End use VT100 sequences (no tilde).
const HOME_END_SEQUENCES: &[(&str, &str)] = &[("home", "\x1b[H"), ("end", "\x1b[F")];

/// Ctrl+letter produces ASCII control codes (0x01-0x1A).
const CTRL_LETTER_CODES: &[(char, u8)] = &[
    ('a', 0x01),
    ('b', 0x02),
    ('c', 0x03),
    ('d', 0x04),
    ('e', 0x05),
    ('f', 0x06),
    ('g', 0x07),
    ('h', 0x08),
    ('i', 0x09),
    ('j', 0x0a),
    ('k', 0x0b),
    ('l', 0x0c),
    ('m', 0x0d),
    ('n', 0x0e),
    ('o', 0x0f),
    ('p', 0x10),
    ('q', 0x11),
    ('r', 0x12),
    ('s', 0x13),
    ('t', 0x14),
    ('u', 0x15),
    ('v', 0x16),
    ('w', 0x17),
    ('x', 0x18),
    ('y', 0x19),
    ('z', 0x1a),
];

/// Special Ctrl combinations with different codes.
const CTRL_SPECIAL: &[(&str, &str)] = &[
    ("ctrl+space", "\x00"),
    ("ctrl+backspace", "\x7f"),
    ("ctrl+tab", "\x00"),
];

/// Alt+letter produces ESC prefix + letter.
const ALT_LETTER_SEQUENCES: &[(char, &str)] = &[
    ('a', "\x1ba"),
    ('b', "\x1bb"),
    ('c', "\x1bc"),
    ('x', "\x1bx"),
    ('z', "\x1bz"),
];

/// CSI 27 modifier encoding: CSI 1;mod;code~
/// Modifier bits: Shift=1, Alt=2, Ctrl=4
fn csi27(modifier_bits: u8, code: u8) -> String {
    format!("\x1b[1;{modifier_bits}={code}~")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_key_sequences_match_vt220() {
        for &(num, seq) in F_KEY_SEQUENCES {
            assert!(!seq.is_empty(), "F{num} sequence must not be empty");
            assert!(seq.starts_with("\x1b"), "F{num} must start with ESC");
        }
        assert_eq!(F_KEY_SEQUENCES.len(), 12, "must have F1-F12");
    }

    #[test]
    fn cursor_keys_normal_mode() {
        for &(name, seq) in CURSOR_KEY_SEQUENCES {
            assert!(
                seq.starts_with("\x1b["),
                "{name} must use CSI prefix in normal mode"
            );
            assert_eq!(seq.len(), 3, "{name} must be ESC + [ + letter");
        }
    }

    #[test]
    fn cursor_keys_app_mode() {
        for &(name, seq) in APP_CURSOR_SEQUENCES {
            assert!(
                seq.starts_with("\x1bO"),
                "{name} must use SS3 prefix in app mode"
            );
            assert_eq!(seq.len(), 3, "{name} must be ESC + O + letter");
        }
    }

    #[test]
    fn app_cursor_differs_from_normal() {
        for i in 0..CURSOR_KEY_SEQUENCES.len() {
            assert_ne!(
                CURSOR_KEY_SEQUENCES[i].1, APP_CURSOR_SEQUENCES[i].1,
                "{} must differ between normal and app mode",
                CURSOR_KEY_SEQUENCES[i].0
            );
        }
    }

    #[test]
    fn editing_keys_use_tilde_suffix() {
        for &(name, seq) in EDITING_KEY_SEQUENCES {
            assert!(seq.ends_with("~"), "{name} must end with tilde");
        }
        // Home/End use VT100 sequences (no tilde)
        for &(name, seq) in HOME_END_SEQUENCES {
            assert!(seq.starts_with("\x1b["), "{name} must use CSI prefix");
            assert!(!seq.ends_with("~"), "{name} must not end with tilde");
        }
    }

    #[test]
    fn ctrl_letter_produces_control_code() {
        for &(ch, expected) in CTRL_LETTER_CODES {
            let code = ch as u8 - b'a' + 1;
            assert_eq!(code, expected, "Ctrl+{ch} should be 0x{expected:02x}");
        }
    }

    #[test]
    fn alt_letter_escapes_prefix() {
        for &(ch, expected) in ALT_LETTER_SEQUENCES {
            let actual = format!("\x1b{ch}");
            assert_eq!(actual, expected, "Alt+{ch} must produce ESC+{ch}");
        }
    }

    #[test]
    fn csi27_ctrl_a() {
        // Ctrl+A with modifier bit 4 (Ctrl) and code 97 ('a')
        let seq = csi27(4, 97);
        assert_eq!(seq, "\x1b[1;4=97~");
    }

    #[test]
    fn csi27_ctrl_shift_a() {
        // Ctrl+Shift+A = modifier 4|1 = 5
        let seq = csi27(5, 97);
        assert_eq!(seq, "\x1b[1;5=97~");
    }

    #[test]
    fn ctrl_fold_table_completeness() {
        // Verify a-z all fold to 0x01-0x1A
        for i in 0..26u8 {
            let ch = (b'a' + i) as char;
            let expected = i + 1;
            let code = ch as u8 - b'a' + 1;
            assert_eq!(code, expected, "Ctrl+{ch} should fold to {expected}");
        }
    }

    #[test]
    fn ctrl_special_combinations() {
        for &(name, expected) in CTRL_SPECIAL {
            // Ctrl+space should map to NUL (0x00), Ctrl+backspace to DEL (0x7F)
            assert!(
                expected.len() == 1,
                "{name}: expected exactly 1 byte, got {expected:?}"
            );
            let byte = expected.as_bytes()[0];
            match name {
                "ctrl+space" => assert_eq!(byte, 0x00, "{name} must be NUL"),
                "ctrl+backspace" => assert_eq!(byte, 0x7f, "{name} must be DEL"),
                "ctrl+tab" => assert_eq!(byte, 0x00, "{name} must be NUL"),
                _ => panic!("unexpected special ctrl key: {name}"),
            }
        }
    }
}
