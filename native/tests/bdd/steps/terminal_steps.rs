//! 终端引擎步骤：驱动 [`GhosttyTerminal`] 并断言行文本、光标、标题与回滚。

use cucumber::{given, then, when};
use native::terminal::ghostty_terminal::GhosttyTerminal;

use super::{TerminalWorld, row_text, unescape};

#[given(expr = "创建 {int} 行 {int} 列终端")]
pub async fn create_terminal(world: &mut TerminalWorld, rows: usize, cols: usize) {
    world.term.inner =
        Some(GhosttyTerminal::new(rows as u32, cols as u32, 100).expect("创建终端失败"));
}

#[when(expr = "程序输出写入 {string}")]
pub async fn write_output(world: &mut TerminalWorld, text: String) {
    let term = world.term.expect_term();
    term.pty_write(text.as_bytes());
    term.flush();
}

#[when(expr = "程序输出写入转义字节 {string}")]
pub async fn write_output_escaped(world: &mut TerminalWorld, raw: String) {
    let term = world.term.expect_term();
    term.pty_write(&unescape(&raw));
    term.flush();
}

#[then(expr = "第 {int} 行文本为 {string}")]
pub async fn expect_row_text(world: &mut TerminalWorld, row: usize, expected: String) {
    let snapshot = world.term.expect_term().take_snapshot();
    assert_eq!(
        row_text(&snapshot, row as u32),
        expected,
        "第 {row} 行文本不符",
    );
}

#[then(expr = "光标位于第 {int} 行第 {int} 列")]
pub async fn expect_cursor(world: &mut TerminalWorld, row: usize, col: usize) {
    let snapshot = world.term.expect_term().take_snapshot();
    assert_eq!(
        (snapshot.cursor_row, snapshot.cursor_col),
        (row as u32, col as u32),
        "光标位置不符",
    );
}

#[then(expr = "窗口标题为 {string}")]
pub async fn expect_title(world: &mut TerminalWorld, expected: String) {
    assert_eq!(world.term.expect_term().title(), expected, "窗口标题不符",);
}

#[then(expr = "回滚行数为 {int}")]
pub async fn expect_scrollback(world: &mut TerminalWorld, expected: usize) {
    let snapshot = world.term.expect_term().take_snapshot();
    assert_eq!(snapshot.scrollback_length, expected as u32, "回滚行数不符",);
}
