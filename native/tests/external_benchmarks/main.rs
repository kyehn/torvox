//! 外部基准：一个测试目标装下全部「取外部权威数据当判据」的用例。
//!
//! 合并成一个目标而不是每份数据一个目标，是因为公共取数辅助会被两个模块各用一半——
//! 拆成两个目标时各自有一半是死代码，而抑制死代码检查正是本仓明令禁止的。
//!
//! - [`unicode_width`]：Unicode UCD 当显示宽度真值。
//! - [`upstream_corpus`]：ghostty 模糊测试语料回放。
//! - [`keyboard_protocol`]：kitty 键盘协议规范当转义序列真值。

mod keyboard_protocol;
mod support;
mod unicode_width;
mod upstream_corpus;
