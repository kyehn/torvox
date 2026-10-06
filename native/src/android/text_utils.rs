//! 与 JNI 无关的按键修饰辅助函数，置于 `ffi.rs` 之外以便脱离 JVM 单元测试。

/// 对按键串施加 Ctrl/Alt/Meta 修饰语义，返回终端收到的字节序列。
/// 可打印 ASCII 配 Ctrl 走标准 `code & 0x1F` 公式；Alt/Meta 加 ESC 前缀；
/// 控制字符与非 ASCII 字节原样透传。掩码值对齐 `KeyModifiers`。
pub(crate) fn encode_modifiers(input: &[u8], modifiers: i32) -> Vec<u8> {
    const CTRL_MASK: i32 = 4;
    const ALT_MASK: i32 = 2;
    const META_MASK: i32 = 8;
    const ESCAPE_BYTE: u8 = 0x1B;
    const CONTROL_FORMULA_MASK: u8 = 0x1F;
    const PRINTABLE_ASCII_RANGE: std::ops::RangeInclusive<u8> = 0x20..=0x7E;

    let ctrl = (modifiers & CTRL_MASK) != 0;
    let alt_or_meta = (modifiers & (ALT_MASK | META_MASK)) != 0;

    let mut output = Vec::with_capacity(input.len() + 2);

    if alt_or_meta {
        output.push(ESCAPE_BYTE); // Alt/Meta 加 ESC 前缀
    }

    if ctrl && input.len() == 1 {
        let code = input[0];
        // 可打印 ASCII 套用 Ctrl 公式；控制字符与非 ASCII 字节原样透传。
        if PRINTABLE_ASCII_RANGE.contains(&code) {
            output.push(code & CONTROL_FORMULA_MASK);
            return output;
        }
    }

    output.extend_from_slice(input);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── encode_modifiers ──────────────────────────────────────────

    #[test]
    fn encode_modifiers_plain_passthrough() {
        assert_eq!(encode_modifiers(b"hello", 0), b"hello");
    }

    #[test]
    fn encode_modifiers_ctrl_printable_uses_mask() {
        // Ctrl+A → 0x01，Ctrl+Z → 0x1A。
        assert_eq!(encode_modifiers(b"a", 4), b"\x01");
        assert_eq!(encode_modifiers(b"z", 4), b"\x1a");
    }

    #[test]
    fn encode_modifiers_ctrl_non_printable_passthrough() {
        // Ctrl 作用于已有控制字符时原样透传。
        assert_eq!(encode_modifiers(b"\x01", 4), b"\x01");
        // Ctrl 作用于非 ASCII 字节时原样透传。
        assert_eq!(encode_modifiers(&[0xC3, 0xA9], 4), b"\xc3\xa9");
    }

    #[test]
    fn encode_modifiers_alt_prefixes_esc() {
        assert_eq!(encode_modifiers(b"a", 2), b"\x1ba");
        // Meta (8) 行为与 Alt 相同。
        assert_eq!(encode_modifiers(b"a", 8), b"\x1ba");
    }

    #[test]
    fn encode_modifiers_ctrl_alt_combined() {
        // Alt+Ctrl+A → ESC + Ctrl+A。
        assert_eq!(encode_modifiers(b"a", 2 | 4), b"\x1b\x01");
    }

    #[test]
    fn encode_modifiers_ctrl_multichar_no_mask() {
        // Ctrl 下多字节：无单字符可套公式，原样透传。
        assert_eq!(encode_modifiers(b"ab", 4), b"ab");
    }
}
