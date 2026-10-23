//! 按键编码：上游 `key::Encoder` 在不同终端模式下的输出。
//!
//! 期望值不是本仓的偏好，而是上游 Zig 测试（`key_encode.zig` / `function_keys.zig`）
//! 已断言的协议事实：xterm ctlseqs 的 `CSI 27;mod;code~`、PC 风格功能键表、
//! kitty 键盘协议，以及 DECCKM/DECKPAM/DECBKM/modifyOtherKeys 四种模式。
//! 上游表里带 `utf8`/`unshifted_codepoint` 的用例在此同样必须带文本字段——
//! 可打印键没有文本就不编码，这是上游文档化的约束。

use super::*;
use libghostty_vt::key::{self, Action, Mods};

/// 模式查询（测试侧统一期限，与内建查询同口径）。
fn mode_is_set(terminal: &GhosttyTerminal, mode: libghostty_vt::terminal::Mode) -> bool {
    terminal.mode_get_with_timeout(
        mode,
        std::time::Duration::from_millis(crate::terminal::ghostty_terminal::QUERY_TIMEOUT_MS),
    )
}

fn terminal() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal create")
}

/// 无文本的按键（方向键、回车、退格、功能键）编码结果。
fn encode(terminal: &GhosttyTerminal, key: key::Key, mods: Mods) -> Vec<u8> {
    terminal
        .encode_key_event(key, Action::Press, mods.bits(), None, None)
        .expect("encode_key_event responds")
}

/// 带平台文本的按键编码结果。`text` 是平台按当前布局与修饰生成的文本，
/// `unshifted` 是该键无修饰时的码点。
fn encode_text(
    terminal: &GhosttyTerminal,
    key: key::Key,
    mods: Mods,
    text: char,
    unshifted: char,
) -> Vec<u8> {
    terminal
        .encode_key_event(key, Action::Press, mods.bits(), Some(text), Some(unshifted))
        .expect("encode_key_event responds")
}

/// 打开 DEC 私有模式。
fn set_dec_mode(terminal: &mut GhosttyTerminal, mode: &[u8]) {
    let mut enable = vec![b'\x1b', b'[', b'?'];
    enable.extend_from_slice(mode);
    enable.extend_from_slice(b"h");
    terminal.vt_write(&enable);
    terminal.flush();
}

/// 关闭 DEC 私有模式。
fn reset_dec_mode(terminal: &mut GhosttyTerminal, mode: &[u8]) {
    let mut disable = vec![b'\x1b', b'[', b'?'];
    disable.extend_from_slice(mode);
    disable.extend_from_slice(b"l");
    terminal.vt_write(&disable);
    terminal.flush();
}

// ── 可打印键 ──────────────────────────────────────────────────

#[test]
fn printable_key_needs_platform_text_or_it_sends_nothing() {
    // 上游文档：编码器以逻辑键配修饰位、以文本定字面量。可打印键没有文本
    // 就不再编码——平台必须给出文本（实体键盘的 getUnicodeChar）。
    let terminal = terminal();
    assert!(encode(&terminal, key::Key::A, Mods::empty()).is_empty());
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::empty(), 'a', 'a'),
        b"a"
    );
}

#[test]
fn ctrl_folds_printable_key_to_control_byte() {
    let terminal = terminal();
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::CTRL, 'a', 'a'),
        [0x01]
    );
    assert_eq!(
        encode_text(&terminal, key::Key::Space, Mods::CTRL, ' ', ' '),
        [0x00],
        "Ctrl+Space is NUL"
    );
    // 没有传统控制字节的键（1/9/0）不折叠，按文本原样发出（上游 fixterm 规则）。
    assert_eq!(
        encode_text(&terminal, key::Key::Digit1, Mods::CTRL, '1', '1'),
        b"1"
    );
}

#[test]
fn alt_prefixes_printable_text_with_escape() {
    let terminal = terminal();
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::ALT, 'a', 'a'),
        b"\x1ba"
    );
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::CTRL | Mods::ALT, 'a', 'a'),
        b"\x1b\x01"
    );
}

#[test]
fn shift_is_carried_by_the_platform_text_not_encoded_twice() {
    // Shift 已把 'a' 变成 'A'。调用方须同时给文本与 Shift 修饰位；
    // 编码器不再叠一层 Shift 修饰序列。
    let terminal = terminal();
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::SHIFT, 'A', 'a'),
        b"A"
    );
    // 只有修饰位没有文本时不编码（Shift 不该凭空产生字节）。
    assert!(encode(&terminal, key::Key::A, Mods::SHIFT).is_empty());
}

#[test]
fn lone_modifiers_produce_no_bytes() {
    let terminal = terminal();
    for key in [
        key::Key::ControlLeft,
        key::Key::ShiftLeft,
        key::Key::CapsLock,
        key::Key::NumLock,
    ] {
        assert!(
            encode(&terminal, key, Mods::empty()).is_empty(),
            "{key:?} alone must not reach the pty"
        );
    }
}

#[test]
fn release_without_kitty_report_events_produces_no_bytes() {
    // 上游 legacy() 只处理 press/repeat：Release 在 kitty REPORT_EVENTS 之外
    // 不产生字节。此处锁住「若将来转发 ACTION_UP」的语义仍由终端模式决定，
    // 而非本仓再判一次。
    let terminal = terminal();
    let released = terminal
        .encode_key_event(
            key::Key::A,
            Action::Release,
            Mods::empty().bits(),
            Some('a'),
            Some('a'),
        )
        .expect("encode_key_event responds");
    assert!(released.is_empty());
}

// ── 回车 / Tab ───────────────────────────────────────────────

#[test]
fn enter_family_sends_carriage_return() {
    // 终端的回车是 CR：raw 模式的 TUI 应用把 LF 读成 Ctrl+J 即字母 j。
    let terminal = terminal();
    assert_eq!(encode(&terminal, key::Key::Enter, Mods::empty()), b"\r");
    assert_eq!(
        encode(&terminal, key::Key::NumpadEnter, Mods::empty()),
        b"\r"
    );
    assert_eq!(encode(&terminal, key::Key::Enter, Mods::ALT), b"\x1b\r");
    assert_eq!(
        encode(&terminal, key::Key::Enter, Mods::SHIFT),
        b"\x1b[27;2;13~",
        "Shift+Enter must not send LF"
    );
}

#[test]
fn tab_family_matches_xterm() {
    let terminal = terminal();
    assert_eq!(encode(&terminal, key::Key::Tab, Mods::empty()), b"\t");
    assert_eq!(
        encode(&terminal, key::Key::Tab, Mods::SHIFT),
        b"\x1b[Z",
        "Shift+Tab is back-tab (terminfo kcbt)"
    );
    assert_eq!(encode(&terminal, key::Key::Tab, Mods::ALT), b"\x1b\t");
}

// ── PC 风格功能键 ────────────────────────────────────────────

#[test]
fn unmodified_function_keys_match_pc_style_table() {
    let terminal = terminal();
    assert_eq!(encode(&terminal, key::Key::F1, Mods::empty()), b"\x1bOP");
    assert_eq!(encode(&terminal, key::Key::F4, Mods::empty()), b"\x1bOS");
    assert_eq!(encode(&terminal, key::Key::F5, Mods::empty()), b"\x1b[15~");
    assert_eq!(encode(&terminal, key::Key::F12, Mods::empty()), b"\x1b[24~");
    assert_eq!(encode(&terminal, key::Key::Home, Mods::empty()), b"\x1b[H");
    assert_eq!(encode(&terminal, key::Key::End, Mods::empty()), b"\x1b[F");
    assert_eq!(
        encode(&terminal, key::Key::PageUp, Mods::empty()),
        b"\x1b[5~"
    );
    assert_eq!(
        encode(&terminal, key::Key::Delete, Mods::empty()),
        b"\x1b[3~"
    );
}

#[test]
fn modifier_param_counts_shift_alt_ctrl() {
    // xterm 的 modifier_code：无修饰 1，Shift+1，Alt+2，Ctrl+4。
    let terminal = terminal();
    assert_eq!(encode(&terminal, key::Key::F5, Mods::SHIFT), b"\x1b[15;2~");
    assert_eq!(encode(&terminal, key::Key::F5, Mods::ALT), b"\x1b[15;3~");
    assert_eq!(encode(&terminal, key::Key::F5, Mods::CTRL), b"\x1b[15;5~");
    assert_eq!(
        encode(
            &terminal,
            key::Key::F5,
            Mods::SHIFT | Mods::ALT | Mods::CTRL
        ),
        b"\x1b[15;8~"
    );
}

// ── 终端模式 ────────────────────────────────────────────────

#[test]
fn arrows_follow_cursor_key_application_mode() {
    let mut terminal = terminal();
    assert_eq!(
        encode(&terminal, key::Key::ArrowUp, Mods::empty()),
        b"\x1b[A"
    );

    set_dec_mode(&mut terminal, b"1");
    assert!(
        mode_is_set(
            &terminal,
            libghostty_vt::terminal::Mode::new(1, libghostty_vt::terminal::ModeKind::Dec)
        ),
        "DECCKM must be set"
    );
    assert_eq!(
        encode(&terminal, key::Key::ArrowUp, Mods::empty()),
        b"\x1bOA"
    );
    assert_eq!(
        encode(&terminal, key::Key::ArrowUp, Mods::CTRL),
        b"\x1b[1;5A",
        "modified arrows stay CSI even in application cursor mode"
    );

    // DECRST 必须真回到普通形式：只断言置位时，「复位」这条路径没人钉。
    reset_dec_mode(&mut terminal, b"1");
    assert_eq!(
        encode(&terminal, key::Key::ArrowUp, Mods::empty()),
        b"\x1b[A"
    );
}

#[test]
fn horizontal_arrows_encode_in_both_cursor_modes() {
    // 修饰栏的左右键与硬件键共用同一编码器：DECCKM 关闭时 CSI，
    // 开启时 SS3。只测上键会漏掉左右键的键码映射或序列分支。
    let mut terminal = terminal();
    assert_eq!(
        encode(&terminal, key::Key::ArrowLeft, Mods::empty()),
        b"\x1b[D"
    );
    assert_eq!(
        encode(&terminal, key::Key::ArrowRight, Mods::empty()),
        b"\x1b[C"
    );

    set_dec_mode(&mut terminal, b"1");
    assert_eq!(
        encode(&terminal, key::Key::ArrowLeft, Mods::empty()),
        b"\x1bOD"
    );
    assert_eq!(
        encode(&terminal, key::Key::ArrowRight, Mods::empty()),
        b"\x1bOC"
    );
}

#[test]
fn keypad_stays_numerical_while_numlock_ignore_is_on() {
    // DEC 私有模式 1035 默认开启，含义是「忽略 numlock，即永远数值小键盘」。
    // 上游以该默认值为准，故应用小键盘模式（66）在默认状态下不生效。
    let mut terminal = terminal();
    assert!(
        mode_is_set(
            &terminal,
            libghostty_vt::terminal::Mode::new(1035, libghostty_vt::terminal::ModeKind::Dec)
        ),
        "mode 1035 defaults on"
    );

    set_dec_mode(&mut terminal, b"66");
    assert!(
        mode_is_set(
            &terminal,
            libghostty_vt::terminal::Mode::new(66, libghostty_vt::terminal::ModeKind::Dec)
        ),
        "DECKPAM must be set"
    );
    assert_eq!(
        encode_text(&terminal, key::Key::Numpad1, Mods::empty(), '1', '1'),
        b"1"
    );
    assert_eq!(
        encode(&terminal, key::Key::NumpadEnter, Mods::empty()),
        b"\r"
    );

    // 关掉 1035 后应用小键盘才真正接管：kp_1 / kp_enter 走 SS3 形态。
    reset_dec_mode(&mut terminal, b"1035");
    assert!(
        !mode_is_set(
            &terminal,
            libghostty_vt::terminal::Mode::new(1035, libghostty_vt::terminal::ModeKind::Dec)
        ),
        "mode 1035 must be off"
    );
    assert_eq!(
        encode_text(&terminal, key::Key::Numpad1, Mods::empty(), '1', '1'),
        b"\x1bOq"
    );
    assert_eq!(
        encode(&terminal, key::Key::NumpadEnter, Mods::empty()),
        b"\x1bOM"
    );
}

#[test]
fn backspace_follows_backarrow_key_mode() {
    // DEC 私有模式 67（backarrow key mode）：开则退格发 BS(0x08)，
    // 关则发 DEL(0x7F)。
    let mut terminal = terminal();
    assert_eq!(
        encode(&terminal, key::Key::Backspace, Mods::empty()),
        b"\x7f"
    );

    set_dec_mode(&mut terminal, b"67");
    assert!(
        mode_is_set(
            &terminal,
            libghostty_vt::terminal::Mode::new(67, libghostty_vt::terminal::ModeKind::Dec)
        ),
        "DECBKM must be set"
    );
    assert_eq!(
        encode(&terminal, key::Key::Backspace, Mods::empty()),
        b"\x08"
    );
    // 前删键（Delete）不受 DECBKM 影响。
    assert_eq!(
        encode(&terminal, key::Key::Delete, Mods::empty()),
        b"\x1b[3~"
    );
}

#[test]
fn modify_other_keys_state_two_wraps_modified_chars() {
    let mut terminal = terminal();
    terminal.vt_write(b"\x1b[>4;2m");
    terminal.flush();
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::CTRL, 'a', 'a'),
        b"\x1b[27;5;97~"
    );
    // 无修饰键不受影响。
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::empty(), 'a', 'a'),
        b"a"
    );
}

#[test]
fn kitty_disambiguate_reports_escape_by_codepoint() {
    let mut terminal = terminal();
    terminal.vt_write(b"\x1b[>1u");
    terminal.flush();
    assert_eq!(
        encode(&terminal, key::Key::Escape, Mods::empty()),
        b"\x1b[27u"
    );
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::empty(), 'a', 'a'),
        b"a",
        "plain text stays plain under disambiguate alone"
    );
}

// ── 跨事件状态 ───────────────────────────────────────────────

#[test]
fn absent_unshifted_codepoint_never_carries_over_from_the_previous_key() {
    // `key::Event` 逐事件复用：无修饰码位缺省时若不显式清零，上一键的码位就会
    // 充当本次输入。两条读它的路径都以 0 表示「未提供」：legacy 的 Alt ESC 前缀
    // 回退、kitty 协议对功能键表未收录键的码位上报。
    // 测试键取 KEYCODE_NUMPAD_PAREN_LEFT：Android 侧 getUnshiftedCharacter() 为 0
    // （它没有字面量），且上游 kitty 功能键表未收录。
    let mut terminal = terminal();
    assert_eq!(
        encode(&terminal, key::Key::NumpadParenLeft, Mods::ALT),
        Vec::<u8>::new(),
        "无字面量、无无修饰码位的键不得凭空产生 ESC 前缀字节"
    );

    // 先按一个带无修饰码位的字母，再按同一个键：结果必须与首个按键完全相同。
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::empty(), 'a', 'a'),
        b"a"
    );
    assert_eq!(
        encode(&terminal, key::Key::NumpadParenLeft, Mods::ALT),
        Vec::<u8>::new(),
        "不得沿用上一个按键的无修饰码位"
    );

    // kitty 键盘协议同理：表格未收录的键拿无修饰码位当上报码位，
    // 陈旧值会让「按一个字母，再按 Paste」变成一次幽灵的字母击键。
    terminal.vt_write(b"\x1b[>1u");
    terminal.flush();
    assert_eq!(
        encode(&terminal, key::Key::NumpadParenLeft, Mods::empty()),
        Vec::<u8>::new(),
        "kitty 协议下无无修饰码位时不得上报序列"
    );
    assert_eq!(
        encode_text(&terminal, key::Key::A, Mods::empty(), 'a', 'a'),
        b"a"
    );
    assert_eq!(
        encode(&terminal, key::Key::NumpadParenLeft, Mods::empty()),
        Vec::<u8>::new(),
        "kitty 协议下不得上报上一个按键的码位"
    );
}
