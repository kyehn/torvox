//! 字素簇排布的三条不变式，与 UAX #29 官方用例逐条核对。
//!
//! `GraphemeBreakTest.txt` 给出的是**簇边界期望**（哪些码位属于同一个簇），不含
//! 列宽期望，且引擎在单元层面并不遵循 UAX #29：`docs/specification/DESIGN.md` 规定
//! 以 Ghostty 为终端状态的单一来源、不重复实现其功能、不打本地补丁，因此边界不符
//! 无法在此修正。故本测试不断言簇边界分组，而断言三条与分组无关、且必须成立的
//! 排布不变式：
//!
//! 1. 守恒：权威表中占列的码位在网格里恰好出现一次——不丢、不重。
//! 2. 保序：读回的码位序列是输入序列的子序列——不乱序、不凭空产生。
//! 3. 列布局：单元的列区间自第 0 列起首尾相接，既不留缝也不重叠。
//!
//! 三条不变式覆盖 kitty、rio、wezterm 三家用内联期望或 GPL 数据覆盖的同一维度
//! （UAX #29），而宽度维度由 [`vt_width_classification`](super::vt_width_classification)
//! 单独核对。UCD 数据由 `flake.nix` 的 shellHook 固定拉取 Unicode 18.0.0 版本。
//!
//! 数据源：<https://www.unicode.org/Public/18.0.0/ucd/auxiliary/GraphemeBreakTest.txt>
//! 许可为 Unicode 条款（Unicode Terms of Use），非 GPL，与 kitty 的副本不同。

use std::collections::BTreeMap;

use unicode_width::UnicodeWidthChar;

use crate::terminal::ghostty_terminal::CellData;
use crate::terminal::ghostty_terminal::GhosttyTerminal;

/// UCD 官方用例；由 `flake.nix` 的 shellHook 拉取到固定路径。
const GRAPHEME_BREAK_TEST: &str = "/tmp/unicode-ucd/GraphemeBreakTest.txt";

/// Unicode 18.0.0 用例总数；语料被截断或格式变化时立即失败。
const EXPECTED_CASES: usize = 853;

/// 单批行数。整批一次写入，超过数百行时刷新确认无法在时限内完成。
const BATCH_ROWS: usize = 64;
/// 每行列数；用例最长 4 个簇，簇宽上限 2，余量充足。
const BATCH_COLS: u32 = 80;

/// 一条用例：按 UAX #29 划分的簇序列。
struct GraphemeCase {
    /// 文件原文，用于失败报告。
    source: String,
    /// 期望簇序列。
    clusters: Vec<Vec<u32>>,
}

impl GraphemeCase {
    /// 展平后的输入码位序列。
    fn codepoints(&self) -> Vec<u32> {
        self.clusters.iter().flatten().copied().collect()
    }
}

/// UCD 记法中的簇边界符 `÷`。
const BREAK: char = '\u{00F7}';
/// UCD 记法中的无边界符 `×`。
const NO_BREAK: char = '\u{00D7}';

/// 是否为终端会当作控制动作执行、因而不进入网格的码位。
///
/// UAX #29 的 GB4/GB5 让 `Control` 类码位独立成簇并在两侧断开，但 CR、LF、NUL
/// 在终端里是执行动作而非可见内容，写入后不产生单元，用例的边界期望无法在网格
/// 上表达，因此不纳入比对。
fn is_control_codepoint(codepoint: u32) -> bool {
    char::from_u32(codepoint).is_some_and(|character| character.is_control())
}

/// 解析一行 UCD 用例；格式为 `÷ XXXX × YYYY ÷ ZZZZ ÷`，行尾 `#` 后是注释。
fn parse_case(line: &str) -> GraphemeCase {
    let mut clusters: Vec<Vec<u32>> = Vec::new();
    let mut joined = false;
    for token in line.split_whitespace() {
        match token {
            marker if marker == BREAK.to_string() => {
                joined = false;
            }
            marker if marker == NO_BREAK.to_string() => {
                joined = true;
            }
            hex => {
                let codepoint = u32::from_str_radix(hex, 16).expect("用例含非十六进制记号");
                if joined && clusters.last().is_some_and(|last| !last.is_empty()) {
                    clusters.last_mut().expect("上一簇非空").push(codepoint);
                } else {
                    clusters.push(vec![codepoint]);
                }
                joined = false;
            }
        }
    }
    assert!(!clusters.is_empty(), "用例不含码位：{line}");
    GraphemeCase {
        source: line.to_string(),
        clusters,
    }
}

/// 读回全部用例；含控制码位的行不参与比对。
fn load_cases() -> Vec<GraphemeCase> {
    let text = std::fs::read_to_string(GRAPHEME_BREAK_TEST).unwrap_or_else(|error| {
        panic!("{GRAPHEME_BREAK_TEST} 读取失败（{error}）：进入 nix develop 由 shellHook 拉取")
    });
    let mut all = 0usize;
    let mut cases = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        all += 1;
        let case = parse_case(line);
        if case
            .codepoints()
            .iter()
            .any(|&codepoint| is_control_codepoint(codepoint))
        {
            continue;
        }
        cases.push(case);
    }
    assert_eq!(all, EXPECTED_CASES, "UCD 用例数变动，需复核语料与判定口径");
    cases
}

/// 把一批用例写入终端，按行取回网格单元。
fn cells_by_row(cases: &[GraphemeCase]) -> BTreeMap<u32, Vec<CellData>> {
    let rows = cases.len() as u32;
    let mut terminal = GhosttyTerminal::new(rows, BATCH_COLS, 0).expect("terminal");
    let mut payload = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        for codepoint in case.codepoints() {
            payload.extend_from_slice(
                char::from_u32(codepoint)
                    .expect("用例码位已分配")
                    .encode_utf8(&mut [0u8; 4])
                    .as_bytes(),
            );
        }
        // 末行不换行：换行会让末行滚动，整批读数错位。
        if index + 1 != cases.len() {
            payload.extend_from_slice(b"\r\n");
        }
    }
    terminal.vt_write(&payload);
    terminal.flush();
    let deadline = std::time::Instant::now() + crate::terminal::snapshot_test::CORPUS_TIMEOUT;
    loop {
        if let Some((cells, _)) = terminal.receive_cell_data() {
            let mut by_row: BTreeMap<u32, Vec<CellData>> = BTreeMap::new();
            for cell in cells {
                // 未写入的单元码位为 0，不属于任何簇。
                if cell.codepoint != 0 {
                    by_row.entry(cell.row).or_default().push(cell);
                }
            }
            for row_cells in by_row.values_mut() {
                row_cells.sort_by_key(|cell| cell.col);
            }
            return by_row;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "单元格数据未在时限内就绪"
        );
        std::thread::sleep(crate::terminal::snapshot_test::CORPUS_POLL_INTERVAL);
    }
}

/// 一行的实际码位序列：单元主码位后接该单元的字素簇续接码位。
fn flattened(row_cells: &[CellData]) -> Vec<u32> {
    row_cells
        .iter()
        .flat_map(|cell| {
            std::iter::once(cell.codepoint).chain(
                cell.grapheme_extra
                    .iter()
                    .copied()
                    .filter(|&codepoint| codepoint != 0),
            )
        })
        .collect()
}

/// 权威表中占列的码位；零宽码位允许被并入相邻单元或丢弃。
fn occupying_codepoints(codepoints: &[u32]) -> Vec<u32> {
    codepoints
        .iter()
        .copied()
        .filter(|&codepoint| {
            char::from_u32(codepoint)
                .and_then(|character| character.width())
                .is_some_and(|width| width > 0)
        })
        .collect()
}

/// `actual` 是否为 `source` 的子序列，即不乱序且不凭空产生码位。
fn is_subsequence(actual: &[u32], source: &[u32]) -> bool {
    let mut rest = source;
    for value in actual {
        loop {
            match rest.split_first() {
                Some((head, tail)) if head == value => {
                    rest = tail;
                    break;
                }
                Some((_, tail)) => rest = tail,
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ucd_corpus_is_loaded() {
        let cases = load_cases();
        assert!(
            cases.len() > 500,
            "可比对用例过少（{}），语料解析可能已失效",
            cases.len()
        );
        assert!(
            cases.iter().any(|case| case.clusters.len() > 1),
            "用例不含多簇样本，分簇维度未被覆盖"
        );
    }

    /// 每条用例的排布 MUST 同时满足守恒、保序与列布局三条不变式。
    #[test]
    fn grapheme_layout_satisfies_invariants() {
        let cases = load_cases();
        let mut failures = Vec::new();
        for chunk in cases.chunks(BATCH_ROWS) {
            let by_row = cells_by_row(chunk);
            for (row, case) in chunk.iter().enumerate() {
                let row_cells = by_row.get(&(row as u32)).map(Vec::as_slice).unwrap_or(&[]);
                let source = case.codepoints();
                let actual = flattened(row_cells);

                let expected_occupying = occupying_codepoints(&source);
                let mut actual_occupying = actual.clone();
                actual_occupying.retain(|&codepoint| {
                    char::from_u32(codepoint)
                        .and_then(|character| character.width())
                        .is_some_and(|width| width > 0)
                });
                if actual_occupying != expected_occupying {
                    failures.push(format!(
                        "守恒不符：{}\n  输入占列 {:?}\n  网格占列 {:?}",
                        case.source,
                        hex(&expected_occupying),
                        hex(&actual_occupying)
                    ));
                }

                if !is_subsequence(&actual, &source) {
                    failures.push(format!(
                        "保序不符：{}\n  输入 {}\n  网格 {}",
                        case.source,
                        hex(&source),
                        hex(&actual)
                    ));
                }

                let mut next_column = 0u32;
                for cell in row_cells {
                    if cell.col != next_column {
                        failures.push(format!(
                            "列布局不符：{} 第 {} 列起于 {} 列（应为 {next_column}）",
                            case.source, row, cell.col
                        ));
                        break;
                    }
                    next_column = cell.col + cell.width.max(1);
                }
                if next_column > BATCH_COLS {
                    failures.push(format!(
                        "列布局不符：{} 占 {} 列，超出 {BATCH_COLS} 列",
                        case.source, next_column
                    ));
                }
            }
        }

        assert!(
            failures.is_empty(),
            "字素簇排布不变式被破坏 {} 处 / {} 条用例：\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n")
        );
    }

    fn hex(codepoints: &[u32]) -> String {
        codepoints
            .iter()
            .map(|codepoint| format!("{codepoint:04X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
