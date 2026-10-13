//! kitty 键盘协议：把上游规范的功能键表当转义序列的真值。
//!
//! 本仓的编码字节全部由上游 `key::Encoder` 产出，所以这里断言的不是「我们编出正确
//! 的字节」，而是**接线**：应用用 `CSI > flags u` 请求渐进增强后，那份请求必须真的
//! 到达编码器。此前只有 Escape 一个键在 kitty 模式下被钉住（见
//! `key_encoding_tests.rs`），其余功能键在 kitty 模式下的编码没有任何用例。
//!
//! 键的取样面是 Android 能送出来的那些（`keymap.rs` 收录的集合）：规范里 PUA 段
//! 的 F13–F35 与本仓产品面无关，不取。
//!
//! 规范按发布标签取（标签内容冻结，可复现），运行期现取不入库。

use libghostty_vt::key::{self, Action, Mods};
use native::terminal::ghostty_terminal::GhosttyTerminal;

use super::support;
use support::fetch_text;

/// kitty 规范地址：钉在上游发布标签上，标签内容不变故可复现。
const KITTY_RELEASE_TAG: &str = "v0.49.2";

fn spec_url() -> String {
    format!(
        "https://raw.githubusercontent.com/kovidgoyal/kitty/{KITTY_RELEASE_TAG}/docs/keyboard-protocol.rst"
    )
}

/// 渐进增强的标志位（规范「Progressive enhancement」节）。
mod flags {
    /// 消除转义歧义：`ESC`/功能键改用 `CSI u`。
    pub const DISAMBIGUATE: u8 = 1;
    /// 所有键都按转义码上报，含 Enter/Tab/退格与修饰键本身。
    pub const REPORT_ALL: u8 = 8;
}

/// 规范功能键表：`名字 → 允许的转义形式`。
///
/// 表格是规范里由脚本生成的 CSV 表（`.. csv-table:: Functional key codes`），形如
/// `"ESCAPE", "``27 u``", "ENTER", "``13 u``"`。代码跨度里用的是不换行空格分隔码位
/// 与终止符，比较前一律归一成空格再去除。
fn spec_forms(spec: &str) -> Vec<(String, Vec<String>)> {
    let body = spec
        .split(".. start functional key table")
        .nth(1)
        .expect("规范缺少功能键表起点标记")
        .split(".. end functional key table")
        .next()
        .expect("规范缺少功能键表终点标记");
    let mut entries = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with('"') {
            continue;
        }
        let fields: Vec<&str> = line
            .split('"')
            .map(|field| field.trim())
            .filter(|field| !field.is_empty() && *field != ",")
            .collect();
        for pair in fields.chunks(2) {
            let forms: Vec<String> = pair[1]
                .replace('\u{a0}', " ")
                .split(" or ")
                .map(|form| format!("\u{1b}[{}", form.trim().trim_matches('`').replace(' ', "")))
                .collect();
            entries.push((pair[0].to_string(), forms));
        }
    }
    assert!(
        entries.len() > 100,
        "规范功能键表只解析出 {} 条，表格结构变了",
        entries.len()
    );
    entries
}

/// 建一个已按应用请求开启渐进增强的终端。
///
/// 走生产写入路径：应用的请求本身是终端输出，经 PTY 读入后落进同一个 VT 实例；
/// 编码器每次按键从该实例同步选项，故此处不设任何独立开关——那正是要验的接线。
fn terminal_with_flags(flags: u8) -> GhosttyTerminal {
    let mut terminal = GhosttyTerminal::new(24, 80, 100).expect("创建终端");
    terminal.pty_write(format!("\x1b[>{flags}u").as_bytes());
    terminal.flush();
    terminal
}

fn encode(terminal: &GhosttyTerminal, key: key::Key) -> String {
    let bytes = terminal
        .encode_key_event(key, Action::Press, Mods::empty().bits(), None, None)
        .expect("encode_key_event responds");
    String::from_utf8_lossy(&bytes).into_owned()
}

/// 断言语义落在规范列出的形式里。偏离即列出实测值，让差异一眼可读。
fn assert_matches_spec(
    terminal: &GhosttyTerminal,
    key_name: &str,
    key_value: key::Key,
    table: &[(String, Vec<String>)],
) {
    let expected = table
        .iter()
        .find(|(name, _)| name == key_name)
        .unwrap_or_else(|| panic!("规范功能键表没有 {key_name}"))
        .1
        .clone();
    let actual = encode(terminal, key_value);
    assert!(
        expected.contains(&actual),
        "{key_name} 编码为 {actual:?}，规范只列了 {expected:?}"
    );
}

/// 规范与上游实测一致、且 Android 能送出的键（消除歧义模式下）。
const CONFORMANT_UNDER_DISAMBIGUATE: &[(&str, key::Key)] = &[
    ("ESCAPE", key::Key::Escape),
    ("INSERT", key::Key::Insert),
    ("DELETE", key::Key::Delete),
    ("PAGE_UP", key::Key::PageUp),
    ("PAGE_DOWN", key::Key::PageDown),
    ("F3", key::Key::F3),
    ("F5", key::Key::F5),
    ("F6", key::Key::F6),
    ("F7", key::Key::F7),
    ("F8", key::Key::F8),
    ("F9", key::Key::F9),
    ("F10", key::Key::F10),
    ("F11", key::Key::F11),
    ("F12", key::Key::F12),
    ("SCROLL_LOCK", key::Key::ScrollLock),
    ("PRINT_SCREEN", key::Key::PrintScreen),
    ("KP_0", key::Key::Numpad0),
    ("KP_9", key::Key::Numpad9),
    ("KP_DECIMAL", key::Key::NumpadDecimal),
    ("KP_DIVIDE", key::Key::NumpadDivide),
    ("KP_MULTIPLY", key::Key::NumpadMultiply),
    ("KP_SUBTRACT", key::Key::NumpadSubtract),
    ("KP_ADD", key::Key::NumpadAdd),
    ("KP_ENTER", key::Key::NumpadEnter),
    ("KP_EQUAL", key::Key::NumpadEqual),
    ("KP_SEPARATOR", key::Key::NumpadSeparator),
];

/// 再叠加「所有键按转义码上报」后，规范与上游实测一致、且 Android 能送出的键。
const CONFORMANT_UNDER_REPORT_ALL: &[(&str, key::Key)] = &[
    ("ENTER", key::Key::Enter),
    ("TAB", key::Key::Tab),
    ("BACKSPACE", key::Key::Backspace),
    ("CAPS_LOCK", key::Key::CapsLock),
    ("NUM_LOCK", key::Key::NumLock),
    ("LEFT_SHIFT", key::Key::ShiftLeft),
    ("RIGHT_SHIFT", key::Key::ShiftRight),
    ("LEFT_CONTROL", key::Key::ControlLeft),
    ("RIGHT_CONTROL", key::Key::ControlRight),
    ("LEFT_ALT", key::Key::AltLeft),
    ("RIGHT_ALT", key::Key::AltRight),
    ("LEFT_SUPER", key::Key::MetaLeft),
    ("RIGHT_SUPER", key::Key::MetaRight),
];

/// 上游实测与规范不一致的键：钉住实测字节，并逐条注明分歧。
///
/// 这些是上游编码器的取舍，不是本仓接线的问题，故不按规范断言。钉实测值是为了
/// 上游哪天改按规范时立刻看见——失败信息即分歧点，不会无声漂移；同理，上游若
/// 修好了某条，把实测值更新过来即可。
const UPSTREAM_DIVERGES: &[(&str, key::Key, &[u8], &str)] = &[
    (
        "LEFT",
        key::Key::ArrowLeft,
        b"\x1b[D",
        "规范写 `CSI 1 D`，上游发 legacy 的 `CSI D`",
    ),
    (
        "RIGHT",
        key::Key::ArrowRight,
        b"\x1b[C",
        "规范写 `CSI 1 C`，上游发 `CSI C`",
    ),
    (
        "UP",
        key::Key::ArrowUp,
        b"\x1b[A",
        "规范写 `CSI 1 A`，上游发 `CSI A`",
    ),
    (
        "DOWN",
        key::Key::ArrowDown,
        b"\x1b[B",
        "规范写 `CSI 1 B`，上游发 `CSI B`",
    ),
    (
        "HOME",
        key::Key::Home,
        b"\x1b[H",
        "规范写 `CSI 1 H` 或 `CSI 7 ~`，上游发 `CSI H`",
    ),
    (
        "END",
        key::Key::End,
        b"\x1b[F",
        "规范写 `CSI 1 F` 或 `CSI 8 ~`，上游发 `CSI F`",
    ),
    (
        "F1",
        key::Key::F1,
        b"\x1b[P",
        "规范写 `CSI 1 P` 或 `CSI 11 ~`，上游发 `CSI P`",
    ),
    (
        "F2",
        key::Key::F2,
        b"\x1b[Q",
        "规范写 `CSI 1 Q` 或 `CSI 12 ~`，上游发 `CSI Q`",
    ),
    (
        "F4",
        key::Key::F4,
        b"\x1b[S",
        "规范写 `CSI 1 S` 或 `CSI 14 ~`，上游发 `CSI S`",
    ),
    (
        "MEDIA_PLAY_PAUSE",
        key::Key::MediaPlayPause,
        b"",
        "规范列 `57430 u`，上游不发字节",
    ),
    (
        "MEDIA_STOP",
        key::Key::MediaStop,
        b"",
        "规范列 `57432 u`，上游不发字节",
    ),
    (
        "MEDIA_TRACK_NEXT",
        key::Key::MediaTrackNext,
        b"",
        "规范列 `57435 u`，上游不发字节",
    ),
    (
        "MEDIA_TRACK_PREVIOUS",
        key::Key::MediaTrackPrevious,
        b"",
        "规范列 `57436 u`，上游不发字节",
    ),
    (
        "RAISE_VOLUME",
        key::Key::AudioVolumeUp,
        b"",
        "规范列 `57439 u`，上游不发字节",
    ),
    (
        "LOWER_VOLUME",
        key::Key::AudioVolumeDown,
        b"",
        "规范列 `57438 u`，上游不发字节",
    ),
    (
        "MUTE_VOLUME",
        key::Key::AudioVolumeMute,
        b"",
        "规范列 `57440 u`，上游不发字节",
    ),
];

#[test]
fn disambiguate_mode_reports_the_forms_the_spec_lists() {
    let table = spec_forms(&fetch_text(&spec_url()));
    let terminal = terminal_with_flags(flags::DISAMBIGUATE);
    for (key_name, key_value) in CONFORMANT_UNDER_DISAMBIGUATE {
        assert_matches_spec(&terminal, key_name, *key_value, &table);
    }

    // 规范明写：只开消除歧义时 Enter/Tab/退格**仍发 legacy 字节**，好让崩溃在
    // 开启该模式的程序里的用户还能敲 `reset`。这一条正是本仓必须守住的行为。
    for (name, key_value, legacy) in [
        ("ENTER", key::Key::Enter, "\r"),
        ("TAB", key::Key::Tab, "\t"),
        ("BACKSPACE", key::Key::Backspace, "\u{7f}"),
    ] {
        assert_eq!(
            encode(&terminal, key_value),
            legacy,
            "{name} 应保持 legacy 字节"
        );
    }
}

#[test]
fn report_all_keys_reports_the_forms_the_spec_lists() {
    let table = spec_forms(&fetch_text(&spec_url()));
    let terminal = terminal_with_flags(flags::DISAMBIGUATE | flags::REPORT_ALL);
    for (key_name, key_value) in CONFORMANT_UNDER_DISAMBIGUATE
        .iter()
        .chain(CONFORMANT_UNDER_REPORT_ALL)
    {
        assert_matches_spec(&terminal, key_name, *key_value, &table);
    }
}

/// 两种标志下都按实测值断言：分歧点已逐条注在表里。
#[test]
fn divergent_keys_hold_their_measured_encodings() {
    for flags in [flags::DISAMBIGUATE, flags::DISAMBIGUATE | flags::REPORT_ALL] {
        let terminal = terminal_with_flags(flags);
        for (key_name, key_value, expected, divergence) in UPSTREAM_DIVERGES {
            let actual = encode(&terminal, *key_value);
            assert_eq!(
                actual.as_bytes(),
                *expected,
                "{key_name} 在标志 {flags} 下的编码变了（{divergence}）：实测 {actual:?}"
            );
        }
    }
}

/// 取样表里的每个名字都要真在规范表里：否则断言会因拼错名字而恒空转。
#[test]
fn every_sampled_key_name_exists_in_the_spec_table() {
    let table = spec_forms(&fetch_text(&spec_url()));
    let sampled = CONFORMANT_UNDER_DISAMBIGUATE
        .iter()
        .map(|(name, _)| *name)
        .chain(CONFORMANT_UNDER_REPORT_ALL.iter().map(|(name, _)| *name))
        .chain(UPSTREAM_DIVERGES.iter().map(|(name, _, _, _)| *name));
    for key_name in sampled {
        assert!(
            table.iter().any(|(name, _)| name == key_name),
            "取样表里的 {key_name} 不在规范功能键表中"
        );
    }
}
