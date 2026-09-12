//! 链接识别步骤：驱动 [`url_at_column`] 并断言识别结果。

use cucumber::{given, then, when};
use native::terminal::url_regex::url_at_column;

use super::TerminalWorld;

#[given(expr = "待测文本行为 {string}")]
pub async fn set_line(world: &mut TerminalWorld, line: String) {
    world.probe_line = line;
}

#[when(expr = "查询第 {int} 列的链接")]
pub async fn probe_column(world: &mut TerminalWorld, col: usize) {
    world.probe_col = col;
    world.found_url = url_at_column(&world.probe_line, col);
}

#[then(expr = "识别结果为 {string}")]
pub async fn expect_url(world: &mut TerminalWorld, expected: String) {
    assert_eq!(
        world.found_url.as_deref(),
        Some(expected.as_str()),
        "行 {:?} 第 {} 列识别结果不符",
        world.probe_line,
        world.probe_col,
    );
}

#[then("没有识别出链接")]
pub async fn expect_no_url(world: &mut TerminalWorld) {
    assert_eq!(
        world.found_url, None,
        "行 {:?} 第 {} 列不应识别出链接，实际 {:?}",
        world.probe_line, world.probe_col, world.found_url,
    );
}
