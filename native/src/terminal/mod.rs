//! 终端会话编排：PTY 生命周期、Ghostty VT 解析引擎（[`ghostty_terminal`]，封装
//! `libghostty-vt`）与串接 PTY 读取、输入写入、进程等待、渲染器的 [`session`] 协调者。
//! 键编码器每个终端 worker 只分配一次并复用，模式每次按键经 `set_options_from_terminal`
//! 重新同步。
//! * OSC 52（剪贴板写入）直达 Ghostty，以上游为单一来源：session 经上游回调收割
//!   事件存入剪贴板槽；仅上游明确忽略的 OSC 52 读取请求（`?`）由
//!   [`output_processor`] 的最小扫描器拦截。
//! * PTY 卫生配置（setsid + 控制终端、IUTF8、清除 IXON/IXOFF、`ws_xpixel`/`ws_ypixel`、
//!   关闭游离 fd）位于 [`pty`]。

pub mod ghostty_terminal;
/// 测试用 PTY 替身：可控的写入闸门、EINTR 打断与部分写入（仅测试构建）。
#[cfg(test)]
pub mod mock_pty;
pub mod output_processor;
pub mod pty;
pub mod session;

pub mod shell_env;
pub mod theme;

#[cfg(test)]
pub(crate) mod test_helpers;

#[cfg(test)]
pub use mock_pty::{MockPty, MockPtyHandle};
pub use pty::{Pty, PtyError, PtyPair};
pub use shell_env::ShellEnv;

// 核心类型再导出。
pub use ghostty_terminal::{CellData, CursorInfo, CursorStyle};

/// OSC 52 剪贴板负载的字节上限（1MiB）。
///
/// 读写两个方向共用一份：写入方向是终端交给宿主的载荷，读取方向是宿主回给远端的
/// 应答。远超任何真实剪贴板内容，又不至于让一份异常负载拖垮进程内存——写入方向
/// 的负载经 `String` 事件 → 事件队列 → Kotlin 剪贴板逐层复制，读取方向的负载还要
/// 整份 base64 编码后写进 PTY（编码后还要再涨三分之一）。
pub(crate) const MAX_CLIPBOARD_PAYLOAD_BYTES: usize = 1024 * 1024;

/// 把 OSC 52 剪贴板负载截到 [`MAX_CLIPBOARD_PAYLOAD_BYTES`]，返回截断后的切片与
/// 「是否发生过截断」。
///
/// 切点落在字符边界：按字节上限硬切时落点可能在一个多字节序列中间，随后的
/// `from_utf8_lossy` 会补一个 U+FFFD——用户拿到的剪贴板末尾凭空多一个替换符。
/// 续字节（`10xxxxxx`）一律退回去，最多退 3 字节（4 字节字符的续字节数）。
pub(crate) fn truncate_clipboard_payload(payload: &[u8]) -> (&[u8], bool) {
    if payload.len() <= MAX_CLIPBOARD_PAYLOAD_BYTES {
        return (payload, false);
    }
    let mut end = MAX_CLIPBOARD_PAYLOAD_BYTES;
    while end > 0 && payload[end] & 0xC0 == 0x80 {
        end -= 1;
    }
    (&payload[..end], true)
}
