//! Android 键码（`android.view.KeyEvent.KEYCODE_*`）→ 上游 `key::Key` 的对应。
//!
//! 对应表放在原生侧而非 Kotlin：上游 `key::Key` 的判别值随每次依赖更新都可能
//! 变化，在 Kotlin 里硬编码这些数字会让错配悄无声息（字母仍能打，方向键却变成
//! 别的键）。此处用上游的具名常量构造 `key::Key`，上游怎么重排都编译期可见。
//!
//! 取值来自 android.jar 的 `android.view.KeyEvent`（`javap -constants`），键码是
//! 平台 ABI 常量：API 24 到 36 只增不改，核对方式见本文件末尾的测试。
//!
//! 未收录的键码返回 `None`：那些键要么没有终端语义（BACK/APP_SWITCH 等系统导航
//! 键），要么本就该由键的文本表达（如 KEYCODE_AT 的 `@` 实为 Shift+2）。调用方
//! 据此回退到按键文本，不静默丢弃按键。

use libghostty_vt::key;

// ── android.view.KeyEvent 键码常量 ─────────────────────────────
// 连续段只列两端：段内按键等差，端点即段的定义。

/// KEYCODE_0（KEYCODE_1..KEYCODE_9 依次 +1）。
const KEYCODE_0: i32 = 7;
/// KEYCODE_9（KEYCODE_0..KEYCODE_9 连续段的末键）。
const KEYCODE_9: i32 = 16;
/// KEYCODE_A（KEYCODE_B..KEYCODE_Z 依次 +1）。
const KEYCODE_A: i32 = 29;
/// KEYCODE_Z（KEYCODE_A..KEYCODE_Z 连续段的末键）。
const KEYCODE_Z: i32 = 54;
/// KEYCODE_F1（KEYCODE_F2..KEYCODE_F12 依次 +1）。
const KEYCODE_F1: i32 = 131;
/// KEYCODE_F12（KEYCODE_F1..KEYCODE_F12 连续段的末键）。
const KEYCODE_F12: i32 = 142;
/// KEYCODE_NUMPAD_0（KEYCODE_NUMPAD_1..KEYCODE_NUMPAD_9 依次 +1）。
const KEYCODE_NUMPAD_0: i32 = 144;
/// KEYCODE_NUMPAD_9（KEYCODE_NUMPAD_0..KEYCODE_NUMPAD_9 连续段的末键）。
const KEYCODE_NUMPAD_9: i32 = 153;

/// Android 键码对应的上游逻辑键。布局差异（AZERTY 的 A/Q 互换）由调用方传来的
/// 文本承担：编码器以逻辑键配修饰位、以文本定字面量，两者互补。
pub(crate) fn key_for_android_keycode(key_code: i32) -> Option<key::Key> {
    let digit = key::Key::Digit0 as i32;
    let letter = key::Key::A as i32;
    let function = key::Key::F1 as i32;
    let numpad_digit = key::Key::Numpad0 as i32;
    Some(match key_code {
        KEYCODE_0..=KEYCODE_9 => key::Key::try_from(digit + (key_code - KEYCODE_0)).ok()?,
        KEYCODE_A..=KEYCODE_Z => key::Key::try_from(letter + (key_code - KEYCODE_A)).ok()?,
        KEYCODE_F1..=KEYCODE_F12 => key::Key::try_from(function + (key_code - KEYCODE_F1)).ok()?,
        KEYCODE_NUMPAD_0..=KEYCODE_NUMPAD_9 => {
            key::Key::try_from(numpad_digit + (key_code - KEYCODE_NUMPAD_0)).ok()?
        }

        19 => key::Key::ArrowUp,    // KEYCODE_DPAD_UP
        20 => key::Key::ArrowDown,  // KEYCODE_DPAD_DOWN
        21 => key::Key::ArrowLeft,  // KEYCODE_DPAD_LEFT
        22 => key::Key::ArrowRight, // KEYCODE_DPAD_RIGHT
        23 => key::Key::Enter,      // KEYCODE_DPAD_CENTER：导航键中心，与回车同义
        24 => key::Key::AudioVolumeUp,
        25 => key::Key::AudioVolumeDown,
        26 => key::Key::Power,
        55 => key::Key::Comma,  // KEYCODE_COMMA
        56 => key::Key::Period, // KEYCODE_PERIOD
        57 => key::Key::AltLeft,
        58 => key::Key::AltRight,
        59 => key::Key::ShiftLeft,
        60 => key::Key::ShiftRight,
        61 => key::Key::Tab,
        62 => key::Key::Space,
        66 => key::Key::Enter,     // KEYCODE_ENTER
        67 => key::Key::Backspace, // KEYCODE_DEL：Android 的退格键
        68 => key::Key::Backquote, // KEYCODE_GRAVE
        69 => key::Key::Minus,     // KEYCODE_MINUS
        70 => key::Key::Equal,     // KEYCODE_EQUALS
        71 => key::Key::BracketLeft,
        72 => key::Key::BracketRight,
        73 => key::Key::Backslash,
        74 => key::Key::Semicolon,
        75 => key::Key::Quote, // KEYCODE_APOSTROPHE
        76 => key::Key::Slash,
        85 => key::Key::MediaPlayPause,
        86 => key::Key::MediaStop,
        87 => key::Key::MediaTrackNext,     // KEYCODE_MEDIA_NEXT
        88 => key::Key::MediaTrackPrevious, // KEYCODE_MEDIA_PREVIOUS
        92 => key::Key::PageUp,
        93 => key::Key::PageDown,
        111 => key::Key::Escape,
        112 => key::Key::Delete, // KEYCODE_FORWARD_DEL
        113 => key::Key::ControlLeft,
        114 => key::Key::ControlRight,
        115 => key::Key::CapsLock,
        116 => key::Key::ScrollLock,
        117 => key::Key::MetaLeft,
        118 => key::Key::MetaRight,
        120 => key::Key::PrintScreen, // KEYCODE_SYSRQ
        121 => key::Key::Pause,       // KEYCODE_BREAK：Termux 同键发 `ESC [ 34 ~`
        122 => key::Key::Home,        // KEYCODE_MOVE_HOME
        123 => key::Key::End,         // KEYCODE_MOVE_END
        124 => key::Key::Insert,
        143 => key::Key::NumLock,
        154 => key::Key::NumpadDivide,
        155 => key::Key::NumpadMultiply,
        156 => key::Key::NumpadSubtract,
        157 => key::Key::NumpadAdd,
        158 => key::Key::NumpadDecimal,   // KEYCODE_NUMPAD_DOT
        159 => key::Key::NumpadSeparator, // KEYCODE_NUMPAD_COMMA
        160 => key::Key::NumpadEnter,
        161 => key::Key::NumpadEqual,
        162 => key::Key::NumpadParenLeft,
        163 => key::Key::NumpadParenRight,
        164 => key::Key::AudioVolumeMute, // KEYCODE_VOLUME_MUTE
        211 => key::Key::KanaMode,        // KEYCODE_ZENKAKU_HANKAKU
        212 => key::Key::Convert,         // KEYCODE_EISU
        216 => key::Key::IntlYen,         // KEYCODE_YEN
        217 => key::Key::IntlRo,          // KEYCODE_RO
        219 => key::Key::Help,            // KEYCODE_ASSIST
        223 => key::Key::Sleep,
        224 => key::Key::WakeUp,
        277 => key::Key::Cut,
        278 => key::Key::Copy,
        279 => key::Key::Paste,

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 连续段的两端必须落在预期的上游键上：段的减法算术一旦写错，
    /// 只有端点断言能立刻发现（例如 KEYCODE_0 之后不再是 Digit9）。
    #[test]
    fn contiguous_key_ranges_land_on_expected_keys() {
        assert_eq!(key_for_android_keycode(7), Some(key::Key::Digit0));
        assert_eq!(key_for_android_keycode(16), Some(key::Key::Digit9));
        assert_eq!(key_for_android_keycode(29), Some(key::Key::A));
        assert_eq!(key_for_android_keycode(54), Some(key::Key::Z));
        assert_eq!(key_for_android_keycode(131), Some(key::Key::F1));
        assert_eq!(key_for_android_keycode(142), Some(key::Key::F12));
        assert_eq!(key_for_android_keycode(144), Some(key::Key::Numpad0));
        assert_eq!(key_for_android_keycode(153), Some(key::Key::Numpad9));
    }

    #[test]
    fn navigation_and_editing_keys_map() {
        assert_eq!(key_for_android_keycode(19), Some(key::Key::ArrowUp));
        assert_eq!(key_for_android_keycode(22), Some(key::Key::ArrowRight));
        assert_eq!(key_for_android_keycode(66), Some(key::Key::Enter));
        // Android 的退格键叫 DEL（67），前删键才叫 FORWARD_DEL（112）——
        // 二者对调会让退格变成前删。
        assert_eq!(key_for_android_keycode(67), Some(key::Key::Backspace));
        assert_eq!(key_for_android_keycode(112), Some(key::Key::Delete));
    }

    /// Pause/Break 必须到达上游编码器：未映射时该键退回按键文本（为空），
    /// 整个键静默失效——Termux 在同键发 `ESC [ 34 ~`。
    #[test]
    fn pause_break_key_maps_to_pause() {
        assert_eq!(key_for_android_keycode(121), Some(key::Key::Pause));
    }

    #[test]
    fn keys_without_terminal_meaning_are_unmapped() {
        assert_eq!(key_for_android_keycode(4), None); // KEYCODE_BACK
        assert_eq!(key_for_android_keycode(187), None); // KEYCODE_APP_SWITCH
        assert_eq!(key_for_android_keycode(84), None); // KEYCODE_SEARCH
    }
}
