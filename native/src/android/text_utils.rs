//! 与 JNI 无关的按键修饰辅助函数，置于 `ffi.rs` 之外以便脱离 JVM 单元测试。

/// 终端收到的按键字节：Alt/Super 加 ESC 前缀，其余原样。
///
/// 此处**不做** Ctrl 折叠（`code & 0x1F`）：走本入口的调用方已保证 Ctrl 未激活
/// （`Bridge.processKeyEvent` 的两条回退分支都在 `!ctrlActive` 内），Ctrl 折叠由
/// 上游按键编码器负责。两端各实现一遍 Ctrl 规则只会让它们对同一按键产生分歧。
pub(crate) fn encode_meta_prefix(input: &[u8], meta_prefix: bool) -> Vec<u8> {
    const ESCAPE_BYTE: u8 = 0x1B;

    let mut output = Vec::with_capacity(input.len() + 1);
    if meta_prefix {
        output.push(ESCAPE_BYTE);
    }
    output.extend_from_slice(input);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_input_passes_through() {
        assert_eq!(encode_meta_prefix(b"hello", false), b"hello");
    }

    #[test]
    fn meta_prefix_leads_with_escape() {
        assert_eq!(encode_meta_prefix(b"a", true), b"\x1ba");
        // 多字节输入（如输入法给出的非 ASCII 字符）同样只加前缀，不逐字节处理。
        assert_eq!(encode_meta_prefix("é".as_bytes(), true), b"\x1b\xc3\xa9");
    }
}
