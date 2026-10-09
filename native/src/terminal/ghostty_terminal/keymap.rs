use libghostty_vt::key::Key;

/// 将 Android `KeyEvent` 键码映射为 ghostty `key::Key`。
/// 参考 <https://developer.android.com/reference/android/view/KeyEvent>
pub(crate) fn map_android_key_code(key_code: u32) -> Key {
    match key_code {
        // 字母键
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
        // 数字键
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
        // 符号键
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
        // 导航与编辑
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
        // 修饰键
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
        // 功能键
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
        // 系统键
        117 => Key::MetaLeft,
        118 => Key::MetaRight,
        120 => Key::PrintScreen,
        121 => Key::Pause,
        // 小键盘
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
        // 媒体键
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

    /// Android `KeyEvent` 的权威键码 → ghostty `Key` 全表。
    ///
    /// 逐条取自 `android.view.KeyEvent` 的常量值（`javap -constants` 取自 SDK
    /// `android.jar`，非本仓复述），故任一条写错都会指向另一个物理键。断言覆盖
    /// **全部**已映射键码（本表即实现的全集，两者必须一一对应）：
    /// 抽样断言漏掉的正是这种错配——把 `10 => Key::Digit3` 写成 `Digit4` 时，
    /// 任何「抽样几个键看看对不对」的测试都会通过。
    #[test]
    fn every_mapped_android_code_maps_to_its_own_key() {
        // (KeyEvent 常量名, 键码值, ghostty Key)
        let table: &[(&str, u32, Key)] = &[
            ("0", 7, Key::Digit0),
            ("1", 8, Key::Digit1),
            ("2", 9, Key::Digit2),
            ("3", 10, Key::Digit3),
            ("4", 11, Key::Digit4),
            ("5", 12, Key::Digit5),
            ("6", 13, Key::Digit6),
            ("7", 14, Key::Digit7),
            ("8", 15, Key::Digit8),
            ("9", 16, Key::Digit9),
            ("A", 29, Key::A),
            ("B", 30, Key::B),
            ("C", 31, Key::C),
            ("D", 32, Key::D),
            ("E", 33, Key::E),
            ("F", 34, Key::F),
            ("G", 35, Key::G),
            ("H", 36, Key::H),
            ("I", 37, Key::I),
            ("J", 38, Key::J),
            ("K", 39, Key::K),
            ("L", 40, Key::L),
            ("M", 41, Key::M),
            ("N", 42, Key::N),
            ("O", 43, Key::O),
            ("P", 44, Key::P),
            ("Q", 45, Key::Q),
            ("R", 46, Key::R),
            ("S", 47, Key::S),
            ("T", 48, Key::T),
            ("U", 49, Key::U),
            ("V", 50, Key::V),
            ("W", 51, Key::W),
            ("X", 52, Key::X),
            ("Y", 53, Key::Y),
            ("Z", 54, Key::Z),
            ("COMMA", 55, Key::Comma),
            ("PERIOD", 56, Key::Period),
            ("ALT_LEFT", 57, Key::AltLeft),
            ("ALT_RIGHT", 58, Key::AltRight),
            ("SHIFT_LEFT", 59, Key::ShiftLeft),
            ("SHIFT_RIGHT", 60, Key::ShiftRight),
            ("TAB", 61, Key::Tab),
            ("SPACE", 62, Key::Space),
            ("ENTER", 66, Key::Enter),
            ("DEL", 67, Key::Backspace),
            ("GRAVE", 68, Key::Backquote),
            ("MINUS", 69, Key::Minus),
            ("EQUALS", 70, Key::Equal),
            ("LEFT_BRACKET", 71, Key::BracketLeft),
            ("RIGHT_BRACKET", 72, Key::BracketRight),
            ("BACKSLASH", 73, Key::Backslash),
            ("SEMICOLON", 74, Key::Semicolon),
            ("APOSTROPHE", 75, Key::Quote),
            ("SLASH", 76, Key::Slash),
            ("MEDIA_PLAY_PAUSE", 85, Key::MediaPlayPause),
            ("MEDIA_STOP", 86, Key::MediaStop),
            ("MEDIA_NEXT", 87, Key::MediaTrackNext),
            ("MEDIA_PREVIOUS", 88, Key::MediaTrackPrevious),
            ("PAGE_UP", 92, Key::PageUp),
            ("PAGE_DOWN", 93, Key::PageDown),
            ("DPAD_UP", 19, Key::ArrowUp),
            ("DPAD_DOWN", 20, Key::ArrowDown),
            ("DPAD_LEFT", 21, Key::ArrowLeft),
            ("DPAD_RIGHT", 22, Key::ArrowRight),
            ("ESCAPE", 111, Key::Escape),
            ("FORWARD_DEL", 112, Key::Delete),
            ("CTRL_LEFT", 113, Key::ControlLeft),
            ("CTRL_RIGHT", 114, Key::ControlRight),
            ("CAPS_LOCK", 115, Key::CapsLock),
            ("SCROLL_LOCK", 116, Key::ScrollLock),
            ("META_LEFT", 117, Key::MetaLeft),
            ("META_RIGHT", 118, Key::MetaRight),
            ("FUNCTION", 119, Key::Fn),
            ("SYSRQ", 120, Key::PrintScreen),
            ("BREAK", 121, Key::Pause),
            ("MOVE_HOME", 122, Key::Home),
            ("MOVE_END", 123, Key::End),
            ("INSERT", 124, Key::Insert),
            ("F1", 131, Key::F1),
            ("F2", 132, Key::F2),
            ("F3", 133, Key::F3),
            ("F4", 134, Key::F4),
            ("F5", 135, Key::F5),
            ("F6", 136, Key::F6),
            ("F7", 137, Key::F7),
            ("F8", 138, Key::F8),
            ("F9", 139, Key::F9),
            ("F10", 140, Key::F10),
            ("F11", 141, Key::F11),
            ("F12", 142, Key::F12),
            ("NUM_LOCK", 143, Key::NumLock),
            ("NUMPAD_0", 144, Key::Numpad0),
            ("NUMPAD_1", 145, Key::Numpad1),
            ("NUMPAD_2", 146, Key::Numpad2),
            ("NUMPAD_3", 147, Key::Numpad3),
            ("NUMPAD_4", 148, Key::Numpad4),
            ("NUMPAD_5", 149, Key::Numpad5),
            ("NUMPAD_6", 150, Key::Numpad6),
            ("NUMPAD_7", 151, Key::Numpad7),
            ("NUMPAD_8", 152, Key::Numpad8),
            ("NUMPAD_9", 153, Key::Numpad9),
            ("NUMPAD_DIVIDE", 154, Key::NumpadDivide),
            ("NUMPAD_MULTIPLY", 155, Key::NumpadMultiply),
            ("NUMPAD_SUBTRACT", 156, Key::NumpadSubtract),
            ("NUMPAD_ADD", 157, Key::NumpadAdd),
            ("NUMPAD_DOT", 158, Key::NumpadDecimal),
            ("NUMPAD_COMMA", 159, Key::NumpadComma),
            ("NUMPAD_ENTER", 160, Key::NumpadEnter),
            ("NUMPAD_EQUALS", 161, Key::NumpadEqual),
        ];
        for (name, code, want) in table {
            assert_eq!(
                map_android_key_code(*code),
                *want,
                "KEYCODE_{name}({code}) 映射错误"
            );
        }
    }

    /// 表与实现双向同步：遍历平台可能的全部键码，凡映射出非 `Unidentified` 的
    /// MUST 都在表里，且表里每个码都映射出非 `Unidentified`。
    /// 删一个 match 分支、或往表里加一个实现没有的码，这里立刻暴露。
    #[test]
    fn table_and_match_branches_are_in_sync() {
        let table: Vec<u32> = vec![
            7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
            41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
            66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 85, 86, 87, 88, 19, 20, 21, 22, 92, 93,
            111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 131, 132, 133,
            134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150,
            151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161,
        ];
        // 平台公开键码的最大值（KEYCODE_MAX = 287）。
        let mapped: Vec<u32> = (0..=287u32)
            .filter(|code| map_android_key_code(*code) != Key::Unidentified)
            .collect();
        let mut expected = table.clone();
        expected.sort_unstable();
        assert_eq!(expected, mapped, "实现有映射但表里没有（或反之）的键码");
    }

    #[test]
    fn unknown_codes_map_to_unidentified() {
        assert_eq!(map_android_key_code(0), Key::Unidentified);
        assert_eq!(map_android_key_code(1), Key::Unidentified);
        assert_eq!(map_android_key_code(999), Key::Unidentified);
        // 200 = KEYCODE_CAPTIONS，表中未映射。
        // 注意 KEYCODE_SYSRQ(120) 是**已**映射的 PrintScreen，不能拿它当反例。
        assert_eq!(map_android_key_code(200), Key::Unidentified);
    }
}
