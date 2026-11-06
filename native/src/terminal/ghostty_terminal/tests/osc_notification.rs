//! OSC 9 / OSC 777 桌面通知：程序请求弹通知的出口。
//!
//! 断言对象是本仓的接线（通知是否真的到达宿主可轮询的通道、负载如何解码），
//! 不是上游解析器本身。

use super::*;

/// OSC 9 通知必须到达宿主可轮询的通道，且无标题。
///
/// 判别力：只覆盖 OSC 777（带标题）时，OSC 9——实际最常见的写法——会被静默丢弃，
/// 用户侧表现是「程序报了通知，终端毫无反应」。
#[test]
fn osc9_notification_reaches_host() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]9;Hello world\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_notification_event(),
        Some((String::new(), "Hello world".to_string())),
        "OSC 9 通知必须到达通知通道，且标题为空"
    );
}

/// OSC 777 的 `notify` 形式带标题：标题与正文都不得丢。
///
/// 判别力：把 OSC 777 当成 OSC 9 解析（只取最后一段）时，标题被丢弃，
/// 通知在系统栏里只剩一段孤零零的正文。
#[test]
fn osc777_notification_carries_title() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]777;notify;Title;Body\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_notification_event(),
        Some(("Title".to_string(), "Body".to_string())),
        "OSC 777 notify 必须带标题与正文"
    );
}

/// 通知序列跨写入块时必须由上游重组：负载不得被截断。
#[test]
fn osc9_notification_split_buffer() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]9;Hello ");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"world\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_notification_event(),
        Some((String::new(), "Hello world".to_string())),
        "OSC 9 跨写入块必须重组为完整通知"
    );
}

/// 生产路径是 `pty_write`（`Session::poll_pty_output` 走 PTY 字节），不是
/// `vt_write`：生产路径不通时，用例仍全绿而真机无通知。
#[test]
fn osc9_notification_via_pty_write_path() {
    let mut terminal_under_test = terminal();
    terminal_under_test.pty_write(b"\x1b]9;Hello world\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_notification_event(),
        Some((String::new(), "Hello world".to_string())),
        "OSC 9 经 pty_write 必须到达通知通道"
    );
}

/// OSC 9 以 ST（`ESC \\`）终止时通知必须同样到达。
#[test]
fn osc9_notification_st_terminated() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]9;Hello world\x1b\\");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_notification_event(),
        Some((String::new(), "Hello world".to_string())),
        "OSC 9 以 ST 终止时通知必须到达"
    );
}

/// 与通知无关的 OSC 不得产生通知事件。
///
/// 判别力：把任意 OSC 都当通知处理时，标题设置（OSC 0）会弹出无意义通知。
#[test]
fn unrelated_osc_produces_no_notification() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]0;unrelated\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_notification_event(),
        None,
        "标题等无关 OSC 不得触发通知"
    );
}

/// 非法 UTF-8 的通知正文按替换字符解码，不得让终端崩溃或丢整条通知。
///
/// 上游明确不保证负载是合法 UTF-8；解码失败若被当作错误丢弃，用户看到的是
/// 「通知偶尔不出现」，而按替换字符口径至少保留可读部分。
#[test]
fn osc9_notification_non_utf8_body_is_lossy_decoded() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]9;A\xffB\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_notification_event(),
        Some((String::new(), "A\u{fffd}B".to_string())),
        "非法 UTF-8 正文必须按替换字符解码"
    );
    // 终端仍须可用。
    terminal_under_test.vt_write(b"After");
    terminal_under_test.flush();
    let snapshot = terminal_under_test.take_snapshot();
    assert!(
        snapshot
            .cells
            .iter()
            .any(|cell| cell.codepoint == 'A' as u32),
        "非法负载通知之后的文本仍须渲染"
    );
    assert_invariants(&snapshot);
}
