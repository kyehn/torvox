//! OSC 7/8/52 步骤：驱动 [`OscHandler`] 并断言解码事件与透传输出。

use cucumber::{then, when};
use native::terminal::osc_handler::{OscEvent, OscHandler};

use super::{TerminalWorld, unescape};

#[when(expr = "终端输出写入转义字节 {string}")]
pub async fn feed_osc(world: &mut TerminalWorld, raw: String) {
    let mut handler = OscHandler::new();
    handler.process(&unescape(&raw));
    world.osc_output = handler.output().to_vec();
    world.osc_events = handler.events().to_vec();
}

#[then(expr = "剪贴板事件文本为 {string}")]
pub async fn expect_clipboard(world: &mut TerminalWorld, expected: String) {
    let actual = world.osc_events.iter().find_map(|event| match event {
        OscEvent::Clipboard(clipboard) => Some(clipboard.text.clone()),
        _ => None,
    });
    assert_eq!(
        actual.as_deref(),
        Some(expected.as_str()),
        "剪贴板事件不符：{events:?}",
        events = world.osc_events,
    );
}

#[then("收到剪贴板读取请求")]
pub async fn expect_clipboard_read(world: &mut TerminalWorld) {
    assert!(
        world
            .osc_events
            .iter()
            .any(|event| matches!(event, OscEvent::ClipboardRead(_))),
        "缺少剪贴板读取请求：{events:?}",
        events = world.osc_events,
    );
}

#[then(expr = "读取请求选择器为 {string}")]
pub async fn expect_clipboard_read_selection(world: &mut TerminalWorld, expected: String) {
    let actual = world.osc_events.iter().find_map(|event| match event {
        OscEvent::ClipboardRead(read) => Some(read.selection.clone()),
        _ => None,
    });
    assert_eq!(
        actual.as_deref(),
        Some(expected.as_str()),
        "读取请求选择器不符：{events:?}",
        events = world.osc_events,
    );
}

#[then(expr = "超链接打开事件地址为 {string}")]
pub async fn expect_hyperlink_open(world: &mut TerminalWorld, expected: String) {
    let actual = world.osc_events.iter().find_map(|event| match event {
        OscEvent::Hyperlink(link) => link.url.clone(),
        _ => None,
    });
    assert_eq!(
        actual.as_deref(),
        Some(expected.as_str()),
        "超链接打开事件不符：{events:?}",
        events = world.osc_events,
    );
}

#[then("收到超链接关闭事件")]
pub async fn expect_hyperlink_close(world: &mut TerminalWorld) {
    assert!(
        world.osc_events.iter().any(|event| matches!(
            event,
            OscEvent::Hyperlink(link) if link.url.is_none()
        )),
        "缺少超链接关闭事件：{events:?}",
        events = world.osc_events,
    );
}

#[then(expr = "工作目录事件路径为 {string}")]
pub async fn expect_cwd(world: &mut TerminalWorld, expected: String) {
    let actual = world.osc_events.iter().find_map(|event| match event {
        OscEvent::Cwd(cwd) => Some(cwd.path.clone()),
        _ => None,
    });
    assert_eq!(
        actual.as_deref(),
        Some(expected.as_str()),
        "工作目录事件不符：{events:?}",
        events = world.osc_events,
    );
}

#[then(expr = "透传输出为 {string}")]
pub async fn expect_passthrough(world: &mut TerminalWorld, expected: String) {
    assert_eq!(
        world.osc_output,
        expected.as_bytes(),
        "透传输出不符：{output:?}",
        output = String::from_utf8_lossy(&world.osc_output),
    );
}
