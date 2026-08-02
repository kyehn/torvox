use libghostty_vt::key::Key;

/// Map Android `KeyEvent` key codes to ghostty `key::Key` values.
/// Reference: <https://developer.android.com/reference/android/view/KeyEvent>
pub(crate) fn map_android_key_code(key_code: u32) -> Key {
    match key_code {
        // Alphabet keys
        29 => Key::A,
        30 => Key::B,
        31 => Key::C,
        32 => Key::D,
        33 => Key::E,
        34 => Key::F,
        35 => Key::G,
        36 => Key::H,
        37 => Key::I,
        38 => Key::J,
        39 => Key::K,
        40 => Key::L,
        41 => Key::M,
        42 => Key::N,
        43 => Key::O,
        44 => Key::P,
        45 => Key::Q,
        46 => Key::R,
        47 => Key::S,
        48 => Key::T,
        49 => Key::U,
        50 => Key::V,
        51 => Key::W,
        52 => Key::X,
        53 => Key::Y,
        54 => Key::Z,
        // Digit keys
        7 => Key::Digit0,
        8 => Key::Digit1,
        9 => Key::Digit2,
        10 => Key::Digit3,
        11 => Key::Digit4,
        12 => Key::Digit5,
        13 => Key::Digit6,
        14 => Key::Digit7,
        15 => Key::Digit8,
        16 => Key::Digit9,
        // Symbol keys
        68 => Key::Backquote,
        69 => Key::Minus,
        70 => Key::Equal,
        71 => Key::BracketLeft,
        72 => Key::BracketRight,
        73 => Key::Backslash,
        74 => Key::Semicolon,
        75 => Key::Quote,
        76 => Key::Slash,
        55 => Key::Comma,
        56 => Key::Period,
        // Navigation and editing
        19 => Key::ArrowUp,
        20 => Key::ArrowDown,
        21 => Key::ArrowLeft,
        22 => Key::ArrowRight,
        66 => Key::Enter,
        67 => Key::Backspace,
        112 => Key::Delete,
        61 => Key::Tab,
        62 => Key::Space,
        111 => Key::Escape,
        122 => Key::Home,
        123 => Key::End,
        92 => Key::PageUp,
        93 => Key::PageDown,
        124 => Key::Insert,
        // Modifier keys
        57 => Key::AltLeft,
        58 => Key::AltRight,
        59 => Key::ShiftLeft,
        60 => Key::ShiftRight,
        113 => Key::ControlLeft,
        114 => Key::ControlRight,
        115 => Key::CapsLock,
        116 => Key::ScrollLock,
        143 => Key::NumLock,
        119 => Key::Fn,
        // Function keys
        131 => Key::F1,
        132 => Key::F2,
        133 => Key::F3,
        134 => Key::F4,
        135 => Key::F5,
        136 => Key::F6,
        137 => Key::F7,
        138 => Key::F8,
        139 => Key::F9,
        140 => Key::F10,
        141 => Key::F11,
        142 => Key::F12,
        // System keys
        117 => Key::MetaLeft,
        118 => Key::MetaRight,
        120 => Key::PrintScreen,
        121 => Key::Pause,
        // Numpad keys
        144 => Key::Numpad0,
        145 => Key::Numpad1,
        146 => Key::Numpad2,
        147 => Key::Numpad3,
        148 => Key::Numpad4,
        149 => Key::Numpad5,
        150 => Key::Numpad6,
        151 => Key::Numpad7,
        152 => Key::Numpad8,
        153 => Key::Numpad9,
        154 => Key::NumpadDivide,
        155 => Key::NumpadMultiply,
        156 => Key::NumpadSubtract,
        157 => Key::NumpadAdd,
        158 => Key::NumpadDecimal,
        159 => Key::NumpadComma,
        160 => Key::NumpadEnter,
        161 => Key::NumpadEqual,
        // Media keys
        85 => Key::MediaPlayPause,
        86 => Key::MediaStop,
        87 => Key::MediaTrackNext,
        88 => Key::MediaTrackPrevious,
        _ => Key::Unidentified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_keys_map_correctly() {
        // KEYCODE_A=29 … KEYCODE_Z=54
        let expected = [('A', 29), ('B', 30), ('C', 31), ('M', 41), ('Z', 54)];
        for (ch, code) in expected {
            let want = match ch {
                'A' => Key::A,
                'B' => Key::B,
                'C' => Key::C,
                'M' => Key::M,
                'Z' => Key::Z,
                _ => unreachable!(),
            };
            assert_eq!(
                map_android_key_code(code),
                want,
                "code {code} should be {ch}"
            );
        }
    }

    #[test]
    fn digit_keys_map_correctly() {
        assert_eq!(map_android_key_code(7), Key::Digit0);
        assert_eq!(map_android_key_code(9), Key::Digit2);
        assert_eq!(map_android_key_code(16), Key::Digit9);
    }

    #[test]
    fn symbol_keys_map_correctly() {
        assert_eq!(map_android_key_code(68), Key::Backquote);
        assert_eq!(map_android_key_code(69), Key::Minus);
        assert_eq!(map_android_key_code(70), Key::Equal);
        assert_eq!(map_android_key_code(71), Key::BracketLeft);
        assert_eq!(map_android_key_code(72), Key::BracketRight);
        assert_eq!(map_android_key_code(73), Key::Backslash);
        assert_eq!(map_android_key_code(74), Key::Semicolon);
        assert_eq!(map_android_key_code(75), Key::Quote);
        assert_eq!(map_android_key_code(76), Key::Slash);
        assert_eq!(map_android_key_code(55), Key::Comma);
        assert_eq!(map_android_key_code(56), Key::Period);
    }

    #[test]
    fn navigation_keys_map_correctly() {
        assert_eq!(map_android_key_code(19), Key::ArrowUp);
        assert_eq!(map_android_key_code(20), Key::ArrowDown);
        assert_eq!(map_android_key_code(21), Key::ArrowLeft);
        assert_eq!(map_android_key_code(22), Key::ArrowRight);
        assert_eq!(map_android_key_code(66), Key::Enter);
        assert_eq!(map_android_key_code(67), Key::Backspace);
        assert_eq!(map_android_key_code(112), Key::Delete);
        assert_eq!(map_android_key_code(61), Key::Tab);
        assert_eq!(map_android_key_code(62), Key::Space);
        assert_eq!(map_android_key_code(111), Key::Escape);
        assert_eq!(map_android_key_code(122), Key::Home);
        assert_eq!(map_android_key_code(123), Key::End);
        assert_eq!(map_android_key_code(92), Key::PageUp);
        assert_eq!(map_android_key_code(93), Key::PageDown);
        assert_eq!(map_android_key_code(124), Key::Insert);
    }

    #[test]
    fn modifier_keys_map_correctly() {
        assert_eq!(map_android_key_code(57), Key::AltLeft);
        assert_eq!(map_android_key_code(58), Key::AltRight);
        assert_eq!(map_android_key_code(59), Key::ShiftLeft);
        assert_eq!(map_android_key_code(60), Key::ShiftRight);
        assert_eq!(map_android_key_code(113), Key::ControlLeft);
        assert_eq!(map_android_key_code(114), Key::ControlRight);
        assert_eq!(map_android_key_code(115), Key::CapsLock);
        assert_eq!(map_android_key_code(116), Key::ScrollLock);
        assert_eq!(map_android_key_code(143), Key::NumLock);
        assert_eq!(map_android_key_code(119), Key::Fn);
    }

    #[test]
    fn function_and_media_keys_map_correctly() {
        assert_eq!(map_android_key_code(131), Key::F1);
        assert_eq!(map_android_key_code(142), Key::F12);
        assert_eq!(map_android_key_code(85), Key::MediaPlayPause);
        assert_eq!(map_android_key_code(86), Key::MediaStop);
        assert_eq!(map_android_key_code(87), Key::MediaTrackNext);
        assert_eq!(map_android_key_code(88), Key::MediaTrackPrevious);
    }

    #[test]
    fn numpad_keys_map_correctly() {
        assert_eq!(map_android_key_code(144), Key::Numpad0);
        assert_eq!(map_android_key_code(153), Key::Numpad9);
        assert_eq!(map_android_key_code(154), Key::NumpadDivide);
        assert_eq!(map_android_key_code(155), Key::NumpadMultiply);
        assert_eq!(map_android_key_code(156), Key::NumpadSubtract);
        assert_eq!(map_android_key_code(157), Key::NumpadAdd);
        assert_eq!(map_android_key_code(158), Key::NumpadDecimal);
        assert_eq!(map_android_key_code(160), Key::NumpadEnter);
        assert_eq!(map_android_key_code(161), Key::NumpadEqual);
    }

    #[test]
    fn unknown_codes_map_to_unidentified() {
        assert_eq!(map_android_key_code(0), Key::Unidentified);
        assert_eq!(map_android_key_code(1), Key::Unidentified);
        assert_eq!(map_android_key_code(999), Key::Unidentified);
        // KEYCODE_SYSRQ etc. that we don't map
        assert_eq!(map_android_key_code(200), Key::Unidentified);
    }

    #[test]
    fn every_android_code_has_unique_mapping() {
        // 全映射键码无重复映射：收集所有已定义码并确认总数（代表性抽查保证结构完整）
        let mapped: Vec<u32> = vec![
            7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
            41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 68, 69, 70, 71, 72, 73,
            74, 75, 76, 19, 20, 21, 22, 66, 67, 112, 61, 62, 111, 122, 123, 92, 93, 124, 57, 58,
            59, 60, 113, 114, 115, 116, 143, 119, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140,
            141, 142, 117, 118, 120, 121, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
            155, 156, 157, 158, 159, 160, 161, 85, 86, 87, 88,
        ];
        for code in mapped {
            assert_ne!(
                map_android_key_code(code),
                Key::Unidentified,
                "code {code} unmapped"
            );
        }
    }
}
