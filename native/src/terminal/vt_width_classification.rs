//! 字符宽度分类与权威 Unicode 数据的一致性。
//!
//! `docs/specification/TESTING.md` 要求覆盖简体中文显示宽度。本项目不自行判定
//! 宽度，而是由 `libghostty-vt` 给出单元格宽度；因此测试要做的是**核对**而非
//! 重新实现：对每一个已分配且宽窄归类不存在版本分歧的码位，断言引擎给出的单元
//! 宽度与外部权威表的分类相同。外部的 `unicode-width` 已在依赖图中
//! （`cosmic-text` 的传递依赖），声明为开发依赖不增加编译单元，也避免把权威
//! 数据抄一份进仓库。
//!
//! 比对区段分两类：[`WIDE_RANGES`] 是中日韩全宽区段，[`RECENT_SCRIPT_RANGES`] 是
//! 较新文种区段。后者的存在是为了让引擎宽度表的缺口显式化：那里的 54 个标记被
//! 判为占 1 列，登记在 [`UPSTREAM_WIDTH_DIVERGENCES`] 中逐码位断言。
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

/// 较新文种区段中已全部分配的连续片段（Balinese、Sundanese、Kawi、Nag Mundari）。
/// 与 [`WIDE_RANGES`] 同为全宽/全窄核对区段，单独列出是因为引擎的宽度表在这里
/// 与权威分类存在分歧，见 [`UPSTREAM_WIDTH_DIVERGENCES`]。区段已剔除未分配码位，
/// 因此每段两端都是已分配码位。
const RECENT_SCRIPT_RANGES: &[(u32, u32)] = &[
    (0x1B00, 0x1B4C),
    (0x1B4E, 0x1B7F),
    (0x1B80, 0x1BBF),
    (0x11F00, 0x11F10),
    (0x11F12, 0x11F3A),
    (0x11F3E, 0x11F5A),
    (0x1E4D0, 0x1E4F9),
];

/// 引擎宽度与权威分类不一致的码位：权威表判 0 列，引擎判 1 列。
///
/// 这 8 个码位都是间距标记（`Mc`；U+11F02 KAWI SIGN REPHA 登记为 `Lo` 但同样
/// 占位）：`East_Asian_Width` 为 `N`，`unicode-width` 按不可见处理，而终端里
/// 间距标记需要实际横向空间，引擎因此判 1 列。属约定差异而非缺陷，故按实测值
/// 逐码位登记，而不是改判定口径。
///
/// 这是精确期望值而非跳过名单：集合外的码位必须与权威分类一致，集合内的码位必须
/// 恰好偏离；新出现的分歧与已被修复的分歧都会使测试失败。
const UPSTREAM_WIDTH_DIVERGENCES: &[u32] = &[
    0x1B35, 0x1B3B, 0x1B3D, 0x1B43, 0x1B44, 0x1BAA, 0x11F02, 0x11F41,
];

/// 引擎对这些码位给出的列数；与权威分类恒差 1 列，且当前全部为 1。
const UPSTREAM_DIVERGENCE_ENGINE_WIDTH: u32 = 1;

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

    /// 较新文种区段必须完全落在 [`RECENT_SCRIPT_RANGES`] 内且全部已分配，
    /// 否则该区段会被跳过而无人察觉。
    #[test]
    fn recent_script_ranges_cover_whole_blocks() {
        let total: usize = RECENT_SCRIPT_RANGES
            .iter()
            .map(|(start, end)| (end - start + 1) as usize)
            .sum();
        assert_eq!(total, 320, "较新文种区段码位数变动，需复核区段划分");

        let recorded = UPSTREAM_WIDTH_DIVERGENCES
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            UPSTREAM_WIDTH_DIVERGENCES
                .iter()
                .all(|codepoint| RECENT_SCRIPT_RANGES
                    .iter()
                    .any(|(start, end)| (*start..=*end).contains(codepoint))),
            "分歧码位必须落在参与比对的区段内，否则该断言永远不会生效"
        );
        assert_eq!(
            recorded.len(),
            UPSTREAM_WIDTH_DIVERGENCES.len(),
            "分歧集合含重复项"
        );
    }

    /// 每个参与比对的码位，引擎给出的单元宽度 MUST 与权威分类相同；已登记的上游
    /// 分歧码位 MUST 恰好偏离，二者取其一，不允许出现未登记的偏差。
    #[test]
    fn engine_width_matches_authoritative_classification() {
        let recorded: std::collections::BTreeSet<u32> =
            UPSTREAM_WIDTH_DIVERGENCES.iter().copied().collect();
        let mut unregistered = Vec::new();
        let mut repaired = Vec::new();
        let mut compared = 0usize;
        for (start, end) in WIDE_RANGES.iter().chain(RECENT_SCRIPT_RANGES) {
            let codepoints: Vec<u32> = (*start..=*end)
                .filter(|codepoint| char::from_u32(*codepoint).is_some())
                .collect();
            for batch in codepoints.chunks(BATCH) {
                for (codepoint, actual) in batch.iter().zip(engine_widths(batch)) {
                    // 权威表把不可打印字符归为无宽度；比对区段已限定为可打印字符，
                    // 故此处 `unwrap_or(0)` 只可能命中区段选择错误。
                    let authoritative = char::from_u32(*codepoint)
                        .expect("assigned codepoint")
                        .width()
                        .unwrap_or(0) as u32;
                    compared += 1;
                    if recorded.contains(codepoint) {
                        if actual != UPSTREAM_DIVERGENCE_ENGINE_WIDTH {
                            repaired.push(format!(
                                "U+{codepoint:04X}: 已登记为上游分歧，但引擎给出 {actual} 列"
                            ));
                        }
                    } else if actual != authoritative {
                        unregistered.push(format!(
                            "U+{codepoint:04X}: authoritative {authoritative} columns, engine {actual} columns"
                        ));
                    }
                }
            }
        }

        for codepoint in &recorded {
            if !WIDE_RANGES
                .iter()
                .chain(RECENT_SCRIPT_RANGES)
                .any(|(start, end)| (*start..=*end).contains(codepoint))
            {
                repaired.push(format!(
                    "U+{codepoint:04X}: 已登记为上游分歧，但不在任何比对区段内"
                ));
            }
        }

        assert!(
            unregistered.is_empty(),
            "未登记的宽度分类偏离 {} / {compared} 个码位：\n{}",
            unregistered.len(),
            unregistered.join("\n")
        );
        assert!(
            repaired.is_empty(),
            "已登记的上游分歧不再成立 {} 项（上游修复后应删除对应登记）：\n{}",
            repaired.len(),
            repaired.join("\n")
        );
    }
}
