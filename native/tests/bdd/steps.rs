//! 中文 BDD 世界状态与共享辅助：各步骤文件只含步骤函数，状态集中于此。

use cucumber::World;

use native::terminal::ghostty_terminal::{GhosttyTerminal, GridSnapshot};

/// 全部场景共享的可变状态，cucumber 为每个场景新建一个。
#[derive(Debug, Default, World)]
pub struct TerminalWorld {
    pub home: String,
    pub prefix: Option<String>,
    pub mkshrc_path: Option<String>,
    pub env: Vec<(String, String)>,
    pub clipboard_read: Option<String>,
    pub terminal: TermSlot,
    pub probe_line: String,
    pub probe_col: usize,
    pub found_url: Option<String>,
}

/// 延迟创建的终端槽位：场景先 `创建 N 行 M 列终端` 再写入输出。
#[derive(Default)]
pub struct TermSlot {
    pub inner: Option<GhosttyTerminal>,
}

impl std::fmt::Debug for TermSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TermSlot")
            .field("created", &self.inner.is_some())
            .finish()
    }
}

impl TermSlot {
    pub fn expect_terminal(&mut self) -> &mut GhosttyTerminal {
        self.inner.as_mut().expect("终端尚未创建")
    }
}

pub mod env_steps;
pub mod osc_steps;
pub mod terminal_steps;
pub mod url_steps;

/// 还原 feature 文件中的转义写法：`\x1b`、`\x07`、`\n` 等变为真实字节。
pub fn unescape(raw: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(current) = chars.next() {
        if current != '\\' {
            output.extend_from_slice(current.encode_utf8(&mut [0; 4]).as_bytes());
            continue;
        }
        match chars.next() {
            Some('x') => {
                let high = chars.next().unwrap_or('0');
                let low = chars.next().unwrap_or('0');
                let byte =
                    u8::from_str_radix(&[high, low].iter().collect::<String>(), 16).unwrap_or(b'?');
                output.push(byte);
            }
            Some('n') => output.push(b'\n'),
            Some('r') => output.push(b'\r'),
            Some('t') => output.push(b'\t'),
            Some('0') => output.push(0),
            Some(other) => {
                output.push(b'\\');
                output.extend_from_slice(other.encode_utf8(&mut [0; 4]).as_bytes());
            }
            None => output.push(b'\\'),
        }
    }
    output
}

/// 提取快照中指定行的可见文本，尾部空白已裁剪。
pub fn row_text(snapshot: &GridSnapshot, row: u32) -> String {
    let mut text = String::new();
    for col in 0..snapshot.cols {
        let index = (row * snapshot.cols + col) as usize;
        if let Some(cell) = snapshot.cells.get(index)
            && cell.codepoint != 0
            && let Some(character) = char::from_u32(cell.codepoint)
        {
            text.push(character);
        }
    }
    text.trim_end().to_string()
}
