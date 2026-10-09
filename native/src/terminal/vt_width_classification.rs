//! 字符宽度分类与权威 Unicode 数据的一致性。
//!
//! `docs/specification/TESTING.md` 要求覆盖简体中文显示宽度。本项目不自行判定
//! 宽度，而是由 `libghostty-vt` 给出单元格宽度；因此测试要做的是**核对**而非
//! 重新实现：对每一个已分配且宽窄归类不存在版本分歧的 CJK 区段码位，断言引擎
//! 给出的单元宽度与外部权威表的分类相同。外部的 `unicode-width` 已在依赖图中
//! （`cosmic-text` 的传递依赖），声明为开发依赖不增加编译单元，也避免把权威
//! 数据抄一份进仓库。
//!
//! 测量法：每行写「候选字符 + `#`」，`#` 落在第几列即该字符占用几列。末行
//! 不能再写 `\r\n`——那会让末行滚动，使末行读数为 0。

use unicode_width::UnicodeWidthChar;

use crate::terminal::ghostty_terminal::GhosttyTerminal;

/// 参与比对的区段：全部已分配，且两侧表对宽窄归类一致。
/// 未分配码位在不同 Unicode 版本间的归类不一致，故不含任何未分配区段。
const WIDE_RANGES: &[(u32, u32)] = &[
    (0x3041, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xA000, 0xA4CF),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0xFE30, 0xFE6F),
    (0xFF01, 0xFF60),
    (0xFFE0, 0xFFE6),
    (0x17000, 0x18AFF),
    (0x1B000, 0x1B2FF),
    (0x20000, 0x2FFFD),
    (0x30000, 0x3FFFD),
];

/// 单批字符数；每行一个候选字符，故同时是终端行数。
const BATCH: usize = 2000;
/// 每行列数：候选字符最多占 2 列，加分隔符后仍有余量。
const BATCH_COLS: u32 = 6;
const BATCH_SCROLLBACK: u32 = 0;

/// 单批字符各自占用的列数，顺序与 `codepoints` 一致。
fn engine_widths(codepoints: &[u32]) -> Vec<u32> {
    let mut terminal = GhosttyTerminal::new(codepoints.len() as u32, BATCH_COLS, BATCH_SCROLLBACK)
        .expect("terminal");
    let mut payload = Vec::with_capacity(codepoints.len() * 5);
    for (index, codepoint) in codepoints.iter().enumerate() {
        let character = char::from_u32(*codepoint).expect("assigned codepoint");
        payload.extend_from_slice(character.encode_utf8(&mut [0u8; 4]).as_bytes());
        // 末行不换行：换行会使末行滚动，该行的 `#` 位置读数为 0。
        payload.extend_from_slice(if index + 1 == codepoints.len() {
            b"#"
        } else {
            b"#\r\n"
        });
    }
    terminal.vt_write(&payload);
    terminal.flush();
    let dumped = terminal.dump_grid();
    (0..codepoints.len())
        .map(|row| {
            dumped.visible[row * BATCH_COLS as usize..(row + 1) * BATCH_COLS as usize]
                .iter()
                .position(|cell| cell.codepoint == u32::from(b'#'))
                .unwrap_or(0) as u32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_ranges_have_assigned_codepoints() {
        let total: usize = WIDE_RANGES
            .iter()
            .map(|(start, end)| (end - start + 1) as usize)
            .sum();
        assert!(total > 100_000, "区段码位过少（{total}），比对失去意义");
    }

    /// 每个参与比对的码位，引擎给出的单元宽度 MUST 与权威分类相同。
    #[test]
    fn engine_width_matches_authoritative_classification() {
        let mut failures = Vec::new();
        let mut compared = 0usize;
        for (start, end) in WIDE_RANGES {
            let codepoints: Vec<u32> = (*start..=*end)
                .filter(|codepoint| char::from_u32(*codepoint).is_some())
                .collect();
            for batch in codepoints.chunks(BATCH) {
                for (codepoint, actual) in batch.iter().zip(engine_widths(batch)) {
                    // 权威表把不可打印字符归为无宽度；比对区段已限定为可打印字符，
                    // 故此处 `unwrap_or(0)` 只可能命中区段选择错误。
                    let expected = char::from_u32(*codepoint)
                        .expect("assigned codepoint")
                        .width()
                        .unwrap_or(0) as u32;
                    compared += 1;
                    if actual != expected {
                        failures.push(format!(
                            "U+{codepoint:04X}: authoritative {expected} columns, engine {actual} columns"
                        ));
                    }
                }
            }
        }

        assert!(
            failures.is_empty(),
            "宽度分类偏离 {} / {compared} 个码位：\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
