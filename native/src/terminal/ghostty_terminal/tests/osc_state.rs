//! OSC 7 / 10 / 11 / 12 与明暗上报：程序探测终端状态的出口。
//!
//! 断言对象是本仓的接线（主题色是否真的成为应答来源、OSC 7 是否被记录），
//! 不是上游解析器本身。

use super::*;

fn theme() -> &'static crate::terminal::theme::Theme {
    crate::terminal::theme::default_dark()
}

/// 从 `rgb:RRRR/GGGG/BBBB` 应答里取三通道字节。xterm 允许每通道 1–4 位十六进制，
/// 按位宽缩放；测试只依赖「归一化后的颜色」，不锁定上游的位数写法。
fn parse_rgb_reply(reply: &str) -> Option<[u8; 3]> {
    let rest = reply.split("rgb:").nth(1)?;
    let mut channels = [0u8; 3];
    for (index, part) in rest.split('/').take(3).enumerate() {
        let digits = part.trim_end_matches(|c: char| !c.is_ascii_hexdigit());
        let value = u32::from_str_radix(digits, 16).ok()?;
        let max = (1u32 << (4 * digits.len() as u32)) - 1;
        channels[index] = ((value * 255 + max / 2) / max) as u8;
    }
    Some(channels)
}

fn query(terminal_under_test: &mut GhosttyTerminal, request: &[u8]) -> String {
    terminal_under_test.vt_write(request);
    terminal_under_test.flush();
    let responses = terminal_under_test.drain_pty_write_responses();
    String::from_utf8_lossy(&responses.concat()).into_owned()
}

/// OSC 7 的原始 URI 必须能被读回：工作目录跟踪的数据源。
#[test]
fn osc7_pwd_is_tracked_verbatim() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]7;file://localhost/home/user/src\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.pwd(),
        "file://localhost/home/user/src",
        "OSC 7 must be readable as the raw URI"
    );
}

/// 空 pwd 表示「shell 尚未上报」，不得被解释成任何目录。
#[test]
fn pwd_is_empty_before_any_report() {
    let terminal_under_test = terminal();
    assert_eq!(terminal_under_test.pwd(), "");
}

/// OSC 11（背景）查询必须以主题背景作答：这是程序判断深浅色的依据。
#[test]
fn osc11_reports_theme_background() {
    let mut terminal_under_test = terminal();
    let reply = query(&mut terminal_under_test, b"\x1b]11;?\x07");
    let background = theme().palette.background;
    let channels = parse_rgb_reply(&reply)
        .unwrap_or_else(|| panic!("OSC 11 reply must carry an rgb colour, got {reply:?}"));
    assert_eq!(
        channels,
        [background.r, background.g, background.b],
        "OSC 11 must report the theme background, got {reply:?}"
    );
}

/// OSC 10（前景）查询同理。
#[test]
fn osc10_reports_theme_foreground() {
    let mut terminal_under_test = terminal();
    let reply = query(&mut terminal_under_test, b"\x1b]10;?\x07");
    let foreground = theme().palette.foreground;
    let channels = parse_rgb_reply(&reply)
        .unwrap_or_else(|| panic!("OSC 10 reply must carry an rgb colour, got {reply:?}"));
    assert_eq!(
        channels,
        [foreground.r, foreground.g, foreground.b],
        "OSC 10 must report the theme foreground, got {reply:?}"
    );
}

/// OSC 12（光标）查询必须报主题光标色，而不是终端内建默认色。
#[test]
fn osc12_reports_theme_cursor() {
    let mut terminal_under_test = terminal();
    let reply = query(&mut terminal_under_test, b"\x1b]12;?\x07");
    let cursor = theme().palette.cursor;
    let channels = parse_rgb_reply(&reply)
        .unwrap_or_else(|| panic!("OSC 12 reply must carry an rgb colour, got {reply:?}"));
    assert_eq!(
        channels,
        [cursor.r, cursor.g, cursor.b],
        "OSC 12 must report the theme cursor, got {reply:?}"
    );
}

/// 程序用 OSC 10/11 覆写颜色后又查询，必须读回覆写值（默认色只是缺省）。
#[test]
fn osc11_override_wins_over_theme() {
    let mut terminal_under_test = terminal();
    let _ = query(&mut terminal_under_test, b"\x1b]11;#123456\x07");
    let reply = query(&mut terminal_under_test, b"\x1b]11;?\x07");
    let channels = parse_rgb_reply(&reply)
        .unwrap_or_else(|| panic!("OSC 11 reply must carry an rgb colour, got {reply:?}"));
    assert_eq!(
        channels,
        [0x12, 0x34, 0x56],
        "override must be reported back: {reply:?}"
    );
}

/// 模式 2031 开启后，主题由深变浅必须主动推 `CSI ? 997 ; 2 n`。
///
/// 这是 shell/编辑器跟随系统明暗的唯一途径：它们不会轮询，只监听这条上报。
#[test]
fn theme_brightness_change_is_reported_when_mode_2031_is_set() {
    let mut terminal_under_test = terminal();
    let _ = query(&mut terminal_under_test, b"\x1b[?2031h");
    terminal_under_test.set_theme(crate::terminal::theme::default_light());
    terminal_under_test.flush();
    let report = String::from_utf8_lossy(&terminal_under_test.drain_pty_write_responses().concat())
        .into_owned();
    assert!(
        report.contains("\x1b[?997;2n"),
        "light theme must be reported as light (997;2), got {report:?}"
    );
}
