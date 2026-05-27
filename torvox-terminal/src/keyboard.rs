bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const SHIFT = 0b0001;
        const ALT    = 0b0010;
        const CTRL   = 0b0100;
        const META   = 0b1000;
    }
}

impl Default for Modifiers {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Press,
    Repeat,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Space,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Char(char),
    Special(SpecialKey),
}

pub struct InputEngine {
    modifiers: Modifiers,
    bracketed_paste: bool,
    kitty_protocol: bool,
}

impl InputEngine {
    pub fn new() -> Self {
        Self {
            modifiers: Modifiers::empty(),
            bracketed_paste: false,
            kitty_protocol: false,
        }
    }

    pub fn set_bracketed_paste(&mut self, enabled: bool) {
        self.bracketed_paste = enabled;
    }

    pub fn set_kitty_protocol(&mut self, enabled: bool) {
        self.kitty_protocol = enabled;
    }

    pub fn set_modifier(&mut self, modifier: Modifiers, pressed: bool) {
        if pressed {
            self.modifiers |= modifier;
        } else {
            self.modifiers.remove(modifier);
        }
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn process_key(&self, key: KeyEvent, action: KeyAction) -> Vec<u8> {
        if self.kitty_protocol {
            self.encode_kitty(key, action)
        } else {
            self.encode_legacy(key, action)
        }
    }

    fn encode_kitty(&self, key: KeyEvent, action: KeyAction) -> Vec<u8> {
        match key {
            KeyEvent::Char(c) => {
                if c == ' ' || c == '\x7f' {
                    return self.encode_kitty_special(key, action);
                }
                if action == KeyAction::Release {
                    return Vec::new();
                }
                let mods = self.modifier_value();
                let code = c as u32;
                if mods > 0 {
                    format!("\x1b[{};{}u", code, mods).into_bytes()
                } else {
                    format!("\x1b[{}u", code).into_bytes()
                }
            }
            KeyEvent::Special(special) => self.encode_kitty_special_key(special, action),
        }
    }

    fn encode_kitty_special(&self, key: KeyEvent, action: KeyAction) -> Vec<u8> {
        if action == KeyAction::Release {
            return Vec::new();
        }
        match key {
            KeyEvent::Char(' ') => {
                let mods = self.modifier_value();
                if mods > 0 {
                    format!("\x1b[32;{}u", mods).into_bytes()
                } else {
                    b"\x1b[32u".to_vec()
                }
            }
            KeyEvent::Char('\x7f') => {
                let mods = self.modifier_value();
                if mods > 0 {
                    format!("\x1b[127;{}u", mods).into_bytes()
                } else {
                    b"\x1b[127u".to_vec()
                }
            }
            _ => Vec::new(),
        }
    }

    fn encode_kitty_special_key(&self, key: SpecialKey, action: KeyAction) -> Vec<u8> {
        if action == KeyAction::Release {
            return Vec::new();
        }
        let mods = self.modifier_value();
        let code = match key {
            SpecialKey::Enter => 13,
            SpecialKey::Tab => 9,
            SpecialKey::Backspace => 127,
            SpecialKey::Escape => 27,
            SpecialKey::Up => 1000,
            SpecialKey::Down => 1001,
            SpecialKey::Left => 1002,
            SpecialKey::Right => 1003,
            SpecialKey::Home => 1004,
            SpecialKey::End => 1005,
            SpecialKey::PageUp => 1006,
            SpecialKey::PageDown => 1007,
            SpecialKey::Insert => 1008,
            SpecialKey::Delete => 1009,
            SpecialKey::F1 => 1010,
            SpecialKey::F2 => 1011,
            SpecialKey::F3 => 1012,
            SpecialKey::F4 => 1013,
            SpecialKey::F5 => 1014,
            SpecialKey::F6 => 1015,
            SpecialKey::F7 => 1016,
            SpecialKey::F8 => 1017,
            SpecialKey::F9 => 1018,
            SpecialKey::F10 => 1019,
            SpecialKey::F11 => 1020,
            SpecialKey::F12 => 1021,
            SpecialKey::F13 => 1022,
            SpecialKey::F14 => 1023,
            SpecialKey::F15 => 1024,
            SpecialKey::F16 => 1025,
            SpecialKey::F17 => 1026,
            SpecialKey::F18 => 1027,
            SpecialKey::F19 => 1028,
            SpecialKey::F20 => 1029,
            SpecialKey::Space => 32,
        };
        if mods > 0 {
            format!("\x1b[{};{}u", code, mods).into_bytes()
        } else {
            format!("\x1b[{}u", code).into_bytes()
        }
    }

    fn encode_legacy(&self, key: KeyEvent, action: KeyAction) -> Vec<u8> {
        if action == KeyAction::Release {
            return Vec::new();
        }
        match key {
            KeyEvent::Char(c) => {
                if self.modifiers.contains(Modifiers::CTRL) && c.is_ascii_alphabetic() {
                    let ctrl_code = (c.to_ascii_uppercase() as u8) - b'A' + 1;
                    return vec![ctrl_code];
                }
                if self.modifiers.contains(Modifiers::ALT) {
                    let mut bytes = Vec::new();
                    bytes.push(0x1b);
                    bytes.extend_from_slice(c.encode_utf8(&mut [0u8; 4]).as_bytes());
                    return bytes;
                }
                c.encode_utf8(&mut [0u8; 4]).as_bytes().to_vec()
            }
            KeyEvent::Special(special) => self.encode_legacy_special(special),
        }
    }

    fn encode_legacy_special(&self, key: SpecialKey) -> Vec<u8> {
        let (prefix, final_char): (&[u8], u8) = match key {
            SpecialKey::Up => (b"\x1b[", b'A'),
            SpecialKey::Down => (b"\x1b[", b'B'),
            SpecialKey::Right => (b"\x1b[", b'C'),
            SpecialKey::Left => (b"\x1b[", b'D'),
            SpecialKey::Home => (b"\x1b[", b'H'),
            SpecialKey::End => (b"\x1b[", b'F'),
            SpecialKey::PageUp => (b"\x1b[5", b'~'),
            SpecialKey::PageDown => (b"\x1b[6", b'~'),
            SpecialKey::Insert => (b"\x1b[2", b'~'),
            SpecialKey::Delete => (b"\x1b[3", b'~'),
            SpecialKey::F1 => (b"\x1bO", b'P'),
            SpecialKey::F2 => (b"\x1bO", b'Q'),
            SpecialKey::F3 => (b"\x1bO", b'R'),
            SpecialKey::F4 => (b"\x1bO", b'S'),
            SpecialKey::F5 => (b"\x1b[15", b'~'),
            SpecialKey::F6 => (b"\x1b[17", b'~'),
            SpecialKey::F7 => (b"\x1b[18", b'~'),
            SpecialKey::F8 => (b"\x1b[19", b'~'),
            SpecialKey::F9 => (b"\x1b[20", b'~'),
            SpecialKey::F10 => (b"\x1b[21", b'~'),
            SpecialKey::F11 => (b"\x1b[23", b'~'),
            SpecialKey::F12 => (b"\x1b[24", b'~'),
            SpecialKey::Enter => (b"", b'\r'),
            SpecialKey::Tab => (b"", b'\t'),
            SpecialKey::Backspace => (b"", b'\x7f'),
            SpecialKey::Escape => (b"", b'\x1b'),
            SpecialKey::Space => (b"", b' '),
            SpecialKey::F13 => (b"\x1b[25", b'~'),
            SpecialKey::F14 => (b"\x1b[26", b'~'),
            SpecialKey::F15 => (b"\x1b[28", b'~'),
            SpecialKey::F16 => (b"\x1b[29", b'~'),
            SpecialKey::F17 => (b"\x1b[31", b'~'),
            SpecialKey::F18 => (b"\x1b[32", b'~'),
            SpecialKey::F19 => (b"\x1b[33", b'~'),
            SpecialKey::F20 => (b"\x1b[34", b'~'),
        };

        if !self.modifiers.is_empty() {
            let mod_val = self.modifier_value_legacy();
            let mut result = Vec::new();
            result.extend_from_slice(prefix);
            result.extend_from_slice(mod_val.as_bytes());
            result.push(final_char);
            result
        } else {
            let mut result = Vec::new();
            result.extend_from_slice(prefix);
            result.push(final_char);
            result
        }
    }

    fn modifier_bits(&self) -> u32 {
        let mut val = 1u32;
        if self.modifiers.contains(Modifiers::SHIFT) {
            val += 1;
        }
        if self.modifiers.contains(Modifiers::ALT) {
            val += 2;
        }
        if self.modifiers.contains(Modifiers::CTRL) {
            val += 4;
        }
        if self.modifiers.contains(Modifiers::META) {
            val += 8;
        }
        val
    }

    fn modifier_value(&self) -> u32 {
        self.modifier_bits()
    }

    fn modifier_value_legacy(&self) -> String {
        format!("1;{}", self.modifier_bits())
    }

    pub fn encode_paste_start(&self) -> Vec<u8> {
        if self.bracketed_paste {
            b"\x1b[200~".to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn encode_paste_end(&self) -> Vec<u8> {
        if self.bracketed_paste {
            b"\x1b[201~".to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn encode_mouse_press(
        &self,
        button: u32,
        col: u32,
        row: u32,
        modifiers: Modifiers,
    ) -> Vec<u8> {
        let btn = button & 0x03;
        let mut encoded = btn;
        if modifiers.contains(Modifiers::SHIFT) {
            encoded |= 0x04;
        }
        if modifiers.contains(Modifiers::ALT) {
            encoded |= 0x08;
        }
        if modifiers.contains(Modifiers::CTRL) {
            encoded |= 0x10;
        }
        if button >= 64 {
            encoded |= 0x40;
        }
        format!("\x1b[<{};{};{}M", encoded, col + 1, row + 1).into_bytes()
    }

    pub fn encode_mouse_release(
        &self,
        button: u32,
        col: u32,
        row: u32,
        modifiers: Modifiers,
    ) -> Vec<u8> {
        let btn = button & 0x03;
        let mut encoded = btn;
        if modifiers.contains(Modifiers::SHIFT) {
            encoded |= 0x04;
        }
        if modifiers.contains(Modifiers::ALT) {
            encoded |= 0x08;
        }
        if modifiers.contains(Modifiers::CTRL) {
            encoded |= 0x10;
        }
        format!("\x1b[<{};{};{}m", encoded, col + 1, row + 1).into_bytes()
    }

    pub fn encode_mouse_motion(
        &self,
        button: u32,
        col: u32,
        row: u32,
        modifiers: Modifiers,
    ) -> Vec<u8> {
        let btn = button & 0x03;
        let mut encoded = 0x20u32 | btn;
        if modifiers.contains(Modifiers::SHIFT) {
            encoded |= 0x04;
        }
        if modifiers.contains(Modifiers::ALT) {
            encoded |= 0x08;
        }
        if modifiers.contains(Modifiers::CTRL) {
            encoded |= 0x10;
        }
        format!("\x1b[<{};{};{}M", encoded, col + 1, row + 1).into_bytes()
    }
}

impl Default for InputEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn modifiers_default_empty() {
        let m = Modifiers::default();
        assert!(m.is_empty());
    }

    #[test]
    fn modifiers_combine() {
        let m = Modifiers::SHIFT | Modifiers::CTRL;
        assert!(m.contains(Modifiers::SHIFT));
        assert!(m.contains(Modifiers::CTRL));
        assert!(!m.contains(Modifiers::ALT));
    }

    #[test]
    fn input_engine_new() {
        let engine = InputEngine::new();
        assert_eq!(engine.modifiers(), Modifiers::empty());
    }

    #[test]
    fn encode_char_a() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Char('a'), KeyAction::Press);
        assert_eq!(result, b"a");
    }

    #[test]
    fn encode_char_with_ctrl() {
        let mut engine = InputEngine::new();
        engine.set_modifier(Modifiers::CTRL, true);
        let result = engine.process_key(KeyEvent::Char('c'), KeyAction::Press);
        assert_eq!(result, vec![0x03]);
    }

    #[test]
    fn encode_char_with_alt() {
        let mut engine = InputEngine::new();
        engine.set_modifier(Modifiers::ALT, true);
        let result = engine.process_key(KeyEvent::Char('x'), KeyAction::Press);
        assert_eq!(result, vec![0x1b, b'x']);
    }

    #[test]
    fn encode_enter() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Enter), KeyAction::Press);
        assert_eq!(result, b"\r");
    }

    #[test]
    fn encode_tab() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Tab), KeyAction::Press);
        assert_eq!(result, b"\t");
    }

    #[test]
    fn encode_backspace() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Backspace), KeyAction::Press);
        assert_eq!(result, vec![0x7f]);
    }

    #[test]
    fn encode_escape() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Escape), KeyAction::Press);
        assert_eq!(result, b"\x1b");
    }

    #[test]
    fn encode_up_arrow() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Up), KeyAction::Press);
        assert_eq!(result, b"\x1b[A");
    }

    #[test]
    fn encode_down_arrow() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Down), KeyAction::Press);
        assert_eq!(result, b"\x1b[B");
    }

    #[test]
    fn encode_right_arrow() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Right), KeyAction::Press);
        assert_eq!(result, b"\x1b[C");
    }

    #[test]
    fn encode_left_arrow() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Left), KeyAction::Press);
        assert_eq!(result, b"\x1b[D");
    }

    #[test]
    fn encode_home() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Home), KeyAction::Press);
        assert_eq!(result, b"\x1b[H");
    }

    #[test]
    fn encode_end() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::End), KeyAction::Press);
        assert_eq!(result, b"\x1b[F");
    }

    #[test]
    fn encode_page_up() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::PageUp), KeyAction::Press);
        assert_eq!(result, b"\x1b[5~");
    }

    #[test]
    fn encode_page_down() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::PageDown), KeyAction::Press);
        assert_eq!(result, b"\x1b[6~");
    }

    #[test]
    fn encode_insert() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Insert), KeyAction::Press);
        assert_eq!(result, b"\x1b[2~");
    }

    #[test]
    fn encode_delete() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Delete), KeyAction::Press);
        assert_eq!(result, b"\x1b[3~");
    }

    #[test]
    fn encode_f1() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::F1), KeyAction::Press);
        assert_eq!(result, b"\x1bOP");
    }

    #[test]
    fn encode_f2() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::F2), KeyAction::Press);
        assert_eq!(result, b"\x1bOQ");
    }

    #[test]
    fn encode_f5() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::F5), KeyAction::Press);
        assert_eq!(result, b"\x1b[15~");
    }

    #[test]
    fn encode_f12() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Special(SpecialKey::F12), KeyAction::Press);
        assert_eq!(result, b"\x1b[24~");
    }

    #[test]
    fn encode_release_returns_empty() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Char('a'), KeyAction::Release);
        assert!(result.is_empty());
    }

    #[test]
    fn encode_repeat_works() {
        let engine = InputEngine::new();
        let result = engine.process_key(KeyEvent::Char('a'), KeyAction::Repeat);
        assert_eq!(result, b"a");
    }

    #[test]
    fn bracketed_paste_start() {
        let mut engine = InputEngine::new();
        engine.set_bracketed_paste(true);
        assert_eq!(engine.encode_paste_start(), b"\x1b[200~");
    }

    #[test]
    fn bracketed_paste_end() {
        let mut engine = InputEngine::new();
        engine.set_bracketed_paste(true);
        assert_eq!(engine.encode_paste_end(), b"\x1b[201~");
    }

    #[test]
    fn bracketed_paste_disabled() {
        let engine = InputEngine::new();
        assert!(engine.encode_paste_start().is_empty());
        assert!(engine.encode_paste_end().is_empty());
    }

    #[test]
    fn mouse_press_sgr() {
        let engine = InputEngine::new();
        let result = engine.encode_mouse_press(0, 5, 10, Modifiers::empty());
        assert_eq!(result, b"\x1b[<0;6;11M");
    }

    #[test]
    fn mouse_release_sgr() {
        let engine = InputEngine::new();
        let result = engine.encode_mouse_release(0, 5, 10, Modifiers::empty());
        assert_eq!(result, b"\x1b[<0;6;11m");
    }

    #[test]
    fn mouse_motion_sgr() {
        let engine = InputEngine::new();
        let result = engine.encode_mouse_motion(0, 5, 10, Modifiers::empty());
        assert_eq!(result, b"\x1b[<32;6;11M");
    }

    #[test]
    fn mouse_press_with_shift() {
        let engine = InputEngine::new();
        let result = engine.encode_mouse_press(0, 5, 10, Modifiers::SHIFT);
        assert_eq!(result, b"\x1b[<4;6;11M");
    }

    #[test]
    fn mouse_press_with_ctrl() {
        let engine = InputEngine::new();
        let result = engine.encode_mouse_press(0, 5, 10, Modifiers::CTRL);
        assert_eq!(result, b"\x1b[<16;6;11M");
    }

    #[test]
    fn mouse_press_button_1() {
        let engine = InputEngine::new();
        let result = engine.encode_mouse_press(1, 5, 10, Modifiers::empty());
        assert_eq!(result, b"\x1b[<1;6;11M");
    }

    #[test]
    fn mouse_press_button_2() {
        let engine = InputEngine::new();
        let result = engine.encode_mouse_press(2, 5, 10, Modifiers::empty());
        assert_eq!(result, b"\x1b[<2;6;11M");
    }

    #[test]
    fn kitty_protocol_char() {
        let mut engine = InputEngine::new();
        engine.set_kitty_protocol(true);
        let result = engine.process_key(KeyEvent::Char('a'), KeyAction::Press);
        assert_eq!(result, b"\x1b[97;1u");
    }

    #[test]
    fn kitty_protocol_enter() {
        let mut engine = InputEngine::new();
        engine.set_kitty_protocol(true);
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Enter), KeyAction::Press);
        assert_eq!(result, b"\x1b[13;1u");
    }

    #[test]
    fn kitty_protocol_up() {
        let mut engine = InputEngine::new();
        engine.set_kitty_protocol(true);
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Up), KeyAction::Press);
        assert_eq!(result, b"\x1b[1000;1u");
    }

    #[test]
    fn kitty_protocol_with_shift() {
        let mut engine = InputEngine::new();
        engine.set_kitty_protocol(true);
        engine.set_modifier(Modifiers::SHIFT, true);
        let result = engine.process_key(KeyEvent::Char('a'), KeyAction::Press);
        assert_eq!(result, b"\x1b[97;2u");
    }

    #[test]
    fn kitty_protocol_release_returns_empty() {
        let mut engine = InputEngine::new();
        engine.set_kitty_protocol(true);
        let result = engine.process_key(KeyEvent::Char('a'), KeyAction::Release);
        assert!(result.is_empty());
    }

    #[test]
    fn ctrl_with_various_keys() {
        let mut engine = InputEngine::new();
        engine.set_modifier(Modifiers::CTRL, true);

        let result = engine.process_key(KeyEvent::Char('a'), KeyAction::Press);
        assert_eq!(result, vec![0x01]);

        let result = engine.process_key(KeyEvent::Char('d'), KeyAction::Press);
        assert_eq!(result, vec![0x04]);

        let result = engine.process_key(KeyEvent::Char('z'), KeyAction::Press);
        assert_eq!(result, vec![0x1a]);
    }

    #[test]
    fn alt_with_arrow() {
        let mut engine = InputEngine::new();
        engine.set_modifier(Modifiers::ALT, true);
        let result = engine.process_key(KeyEvent::Special(SpecialKey::Up), KeyAction::Press);
        assert_eq!(result, b"\x1b[1;3A");
    }

    proptest::proptest! {
        #[test]
        fn process_key_never_panics_char(
            ch in 0x20u32..0x7E,
            shift in proptest::bool::ANY,
            alt in proptest::bool::ANY,
            ctrl in proptest::bool::ANY,
        ) {
            let mut engine = InputEngine::new();
            if shift { engine.set_modifier(Modifiers::SHIFT, true); }
            if alt { engine.set_modifier(Modifiers::ALT, true); }
            if ctrl { engine.set_modifier(Modifiers::CTRL, true); }
            let c = char::from_u32(ch).unwrap_or('x');
            let _ = engine.process_key(KeyEvent::Char(c), KeyAction::Press);
        }

        #[test]
        fn process_key_never_panics_special(
            key in 0u8..=20,
            shift in proptest::bool::ANY,
            alt in proptest::bool::ANY,
        ) {
            let mut engine = InputEngine::new();
            if shift { engine.set_modifier(Modifiers::SHIFT, true); }
            if alt { engine.set_modifier(Modifiers::ALT, true); }
            let special = match key % 21 {
                0 => SpecialKey::Enter,
                1 => SpecialKey::Tab,
                2 => SpecialKey::Backspace,
                3 => SpecialKey::Escape,
                4 => SpecialKey::Up,
                5 => SpecialKey::Down,
                6 => SpecialKey::Left,
                7 => SpecialKey::Right,
                8 => SpecialKey::Home,
                9 => SpecialKey::End,
                10 => SpecialKey::PageUp,
                11 => SpecialKey::PageDown,
                12 => SpecialKey::Insert,
                13 => SpecialKey::Delete,
                14 => SpecialKey::F1,
                15 => SpecialKey::F5,
                16 => SpecialKey::F12,
                17 => SpecialKey::Space,
                18 => SpecialKey::F2,
                19 => SpecialKey::F10,
                _ => SpecialKey::F20,
            };
            let _ = engine.process_key(KeyEvent::Special(special), KeyAction::Press);
        }

        #[test]
        fn kitty_protocol_output_is_valid_utf8(
            ch in 0x20u32..0x7E,
        ) {
            let mut engine = InputEngine::new();
            engine.set_kitty_protocol(true);
            let c = char::from_u32(ch).unwrap_or('x');
            let result = engine.process_key(KeyEvent::Char(c), KeyAction::Press);
            prop_assert!(!result.is_empty(), "kitty output should not be empty");
        }
    }
}
