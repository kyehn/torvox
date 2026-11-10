//! Ghostty VT 引擎的对外 API：按职责拆到子模块，门面只留模块声明与共享的输入契约。

mod commands;
mod construction;
mod encoding;
mod events;
mod frames;
mod queries;

/// `vt_write` 的输入清洗：只剔除 NUL。
///
/// NUL 是 ECMA-48 忽略控制字符，必须在解析器看到前剔除，否则一个游离 NUL
///（mpv --vo=kitty 每帧追加一个）就会毁掉整块载荷，甚至让整张 Kitty 图像丢失。
/// 0xF8–0xFF 不在此处理：合法 UTF-8 里本就不会出现（4 字节上限 F4），而原始二进制
/// VT 载荷（如 Kitty `m=1` 直接 RGB）里它们是合法数据——静默替换为空格等于静默损坏
/// 图像。此函数是 `vt_write` 承诺的二进制安全路径的兑现点。
pub(crate) fn sanitize_vt_input(data: &[u8]) -> Vec<u8> {
    data.iter().filter(|&&b| b != 0x00).copied().collect()
}
