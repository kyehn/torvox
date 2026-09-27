//! 与 JNI 无关的文本与修饰键辅助函数，置于 `ffi.rs` 之外以便脱离 JVM 单元测试。

use crate::terminal::session::Session;

/// 将一行终端网格（绝对行号：先回滚区后可见区）渲染为每列恰含一个字符的字符串：
/// 宽字符展开为两个相同字符，使正则的字节偏移↔列号靠字符数映射；
/// 续接单元（codepoint 0）必须跳过，因前导宽字符单元已占满两列。
pub(crate) fn plain_text_url_at(session: &Session, row_abs: u32, col: u32) -> Option<String> {
    let grid = session.terminal().dump_grid();
    let row_abs = row_abs as usize;
    let line: Option<String> = if row_abs < grid.scrollback.len() {
        Some(cell_line_text(&grid.scrollback[row_abs]))
    } else {
        let visible_row = row_abs.saturating_sub(grid.scrollback.len());
        let cols = grid.cols as usize;
        let start = visible_row * cols;
        let end = start.saturating_add(cols).min(grid.visible.len());
        if start < end {
            Some(cell_line_text(&grid.visible[start..end]))
        } else {
            None
        }
    };
    line.and_then(|text| crate::terminal::url_regex::url_at_column(&text, col as usize))
}

/// 每列一个字符；宽字符（width>=2）占两个副本，使字符下标等于终端列号。
pub(crate) fn cell_line_text(cells: &[crate::terminal::ghostty_terminal::CellSnapshot]) -> String {
    let mut text = String::with_capacity(cells.len());
    for cell in cells {
        if cell.codepoint == 0 {
            // 宽字符的续接单元：两列已由前导单元覆盖。
            continue;
        }
        if let Some(ch) = char::from_u32(cell.codepoint) {
            text.push(ch);
            if cell.width > 1 {
                text.push(ch);
            }
        }
    }
    text
}

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

    // ── cell_line_text ────────────────────────────────────────────

    fn cell(codepoint: u32, width: u8) -> crate::terminal::ghostty_terminal::CellSnapshot {
        crate::terminal::ghostty_terminal::CellSnapshot {
            codepoint,
            graphemes: vec![],
            foreground: [0.0; 4],
            background: [0.0; 4],
            underline_color: [0.0; 4],
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: false,
            strikethrough: false,
            blink: false,
            hidden: false,
            overline: false,
            double_underline: false,
            width,
        }
    }

    #[test]
    fn cell_line_text_ascii_joins_in_order() {
        let cells = [
            cell(b'a' as u32, 1),
            cell(b'b' as u32, 1),
            cell(b'c' as u32, 1),
        ];
        assert_eq!(cell_line_text(&cells), "abc");
    }

    #[test]
    fn cell_line_text_wide_char_duplicates_column() {
        // 宽度 2 的单元贡献两个副本，使字符下标等于终端列号。
        let cells = [cell('中' as u32, 2), cell(b'x' as u32, 1)];
        assert_eq!(cell_line_text(&cells), "中中x");
    }

    #[test]
    fn cell_line_text_skips_continuation_cells() {
        // codepoint 0 为宽字符续接单元，不得产生多余字符。
        let cells = [cell('中' as u32, 2), cell(0, 2), cell(b'y' as u32, 1)];
        assert_eq!(cell_line_text(&cells), "中中y");
    }

    #[test]
    fn cell_line_text_empty() {
        assert_eq!(cell_line_text(&[]), "");
    }

    #[test]
    fn cell_line_text_skips_invalid_codepoint() {
        // 代理区码点不是合法 Unicode 标量值，必须跳过而不崩溃。
        let cells = [cell(0xD800, 1), cell(b'a' as u32, 1)];
        assert_eq!(cell_line_text(&cells), "a");
    }

    // ── plain_text_url_at ─────────────────────────────────────────

    fn url_session(rows: u32, cols: u32) -> Session {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(rows as u16, cols as u16);
        Session::with_pty(
            Box::new(pty) as Box<dyn crate::terminal::pty::Pty>,
            rows,
            cols,
        )
        .expect("with_pty must succeed")
    }

    fn write_line(session: &mut Session, line: &[u8]) {
        session.terminal_mut().vt_write(line);
        session.terminal_mut().flush();
    }

    #[test]
    fn plain_text_url_at_visible_row() {
        let mut session = url_session(24, 80);
        write_line(&mut session, b"visit https://example.com now");
        assert_eq!(
            plain_text_url_at(&session, 0, 6),
            Some("https://example.com".to_string())
        );
        assert_eq!(plain_text_url_at(&session, 0, 0), None);
    }

    #[test]
    fn plain_text_url_at_scrollback_row() {
        let mut session = url_session(3, 40);
        write_line(&mut session, b"go https://example.com ok");
        for index in 0..7 {
            write_line(&mut session, format!("line{index}\n").as_bytes());
        }
        let dumped = session.terminal().dump_grid();
        assert!(
            !dumped.scrollback.is_empty(),
            "scrollback should hold scrolled-off lines"
        );
        let row_abs = dumped
            .scrollback
            .iter()
            .position(|row| cell_line_text(row).contains("https://example.com"))
            .expect("scrollback must contain the url line") as u32;
        assert_eq!(
            plain_text_url_at(&session, row_abs, 3),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn plain_text_url_at_out_of_range_returns_none() {
        let mut session = url_session(24, 80);
        write_line(&mut session, b"visit https://example.com now");
        assert_eq!(plain_text_url_at(&session, u32::MAX, 0), None);
    }

    // ── encode_modifiers ──────────────────────────────────────────

    #[test]
    fn encode_modifiers_plain_passthrough() {
        assert_eq!(encode_modifiers(b"hello", 0), b"hello");
    }

    #[test]
    fn encode_modifiers_ctrl_printable_uses_mask() {
        // Ctrl+A → 0x01, Ctrl+Z → 0x1A.
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
        // Meta (8) behaves like Alt.
        assert_eq!(encode_modifiers(b"a", 8), b"\x1ba");
    }

    #[test]
    fn encode_modifiers_ctrl_alt_combined() {
        // Alt+Ctrl+A → ESC + Ctrl+A.
        assert_eq!(encode_modifiers(b"a", 2 | 4), b"\x1b\x01");
    }

    #[test]
    fn encode_modifiers_ctrl_multichar_no_mask() {
        // Ctrl 下多字节：无单字符可套公式，原样透传。
        assert_eq!(encode_modifiers(b"ab", 4), b"ab");
    }
}
