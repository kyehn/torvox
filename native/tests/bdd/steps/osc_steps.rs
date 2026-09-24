//! OSC 8/52 步骤：经 GhosttyTerminal 真实链路断言，不再直驱解析器。
//! OSC 52 写入直达上游（回调推送事件）；OSC 52 读取请求上游忽略，
//! 由 OutputProcessor 的最小扫描器拦截；超链接按单元格查询断言。

use cucumber::{then, when};
use native::terminal::ghostty_terminal::GhosttyTerminal;
use native::terminal::output_processor::OutputProcessor;

use super::{TerminalWorld, unescape};

fn terminal(world: &mut TerminalWorld) -> &mut GhosttyTerminal {
    world.terminal.expect_terminal()
}

#[then(expr = "剪贴板事件文本为 {string}")]
pub async fn expect_clipboard(world: &mut TerminalWorld, expected: String) {
    let actual = terminal(world).poll_clipboard_event().map(|(_, text)| text);
    assert_eq!(actual.as_deref(), Some(expected.as_str()), "剪贴板事件不符",);
}

#[when(expr = "会话层写入转义字节 {string}")]
pub async fn feed_session_layer(world: &mut TerminalWorld, raw: String) {
    let mut processor = OutputProcessor::new();
    let snapshot = processor.process(&unescape(&raw));
    world.clipboard_read = snapshot.clipboard_read;
}

#[then("收到剪贴板读取请求")]
pub async fn expect_clipboard_read(world: &mut TerminalWorld) {
    assert!(world.clipboard_read.is_some(), "缺少剪贴板读取请求",);
}

#[then(expr = "读取请求选择器为 {string}")]
pub async fn expect_clipboard_read_selection(world: &mut TerminalWorld, expected: String) {
    assert_eq!(
        world.clipboard_read.as_deref(),
        Some(expected.as_str()),
        "读取请求选择器不符",
    );
}

#[then(expr = "第 {int} 行第 {int} 列超链接为 {string}")]
pub async fn expect_hyperlink(world: &mut TerminalWorld, row: usize, col: usize, expected: String) {
    assert_eq!(
        terminal(world)
            .hyperlink_at(row as u32, col as u32)
            .as_deref(),
        Some(expected.as_str()),
        "超链接不符",
    );
}

#[then(expr = "第 {int} 行第 {int} 列无超链接")]
pub async fn expect_no_hyperlink(world: &mut TerminalWorld, row: usize, col: usize) {
    assert_eq!(
        terminal(world).hyperlink_at(row as u32, col as u32),
        None,
        "不应存在超链接",
    );
}
