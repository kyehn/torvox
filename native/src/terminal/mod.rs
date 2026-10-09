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
#[cfg(test)]
pub mod mock_pty;
pub mod output_processor;
pub mod pty;
pub mod session;
pub use session::ThemeConfig;
pub mod shell_env;

#[cfg(test)]
pub(crate) mod snapshot_test;
#[cfg(test)]
pub(crate) mod test_helpers;
#[cfg(test)]
pub(crate) mod vt_conformance;
#[cfg(test)]
pub(crate) mod vt_seed_chunking;
#[cfg(test)]
pub(crate) mod vt_width_classification;

#[cfg(test)]
pub use mock_pty::{MockPty, MockPtyHandle};
pub use pty::{Pty, PtyError, PtyPair};
pub use shell_env::ShellEnv;

// 核心类型再导出。
pub use ghostty_terminal::{CellData, CursorInfo, CursorStyle};
