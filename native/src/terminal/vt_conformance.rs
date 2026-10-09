//! 跨引擎一致性语料：上游独立引擎的真实应用录音与期望屏幕。
//!
//! 语料来自 Alacritty（Apache-2.0，`alacritty_terminal/tests/ref/` 的
//! `alacritty.recording` 与序列化网格）。这些录音是 vim、tmux、htop、less 一类
//! 程序在真实 PTY 上的输出，输入侧完全可移植；期望屏幕由独立引擎产生，与
//! libghostty 引擎独立演化，故可作为一致性基准。
//!
//! 与 `snapshot_test` 顶层语料的差别只在表示约定：上游把未写入单元序列化为
//! 空格，本项目快照把同一单元表示为码位 0。因此这里的屏幕行按「空白单元渲染
//! 成空格」取值（`row_text_spaced`），再与上游期望逐字比对；不套用
//! `capture_snapshot` 的空白裁剪。
//!
//! 单元样式与回滚区不参与比对：两引擎用不同调色板解算颜色，回滚修剪粒度也不同，
//! 断言这些字段等于把解算差异当成终端缺陷。

use std::path::{Path, PathBuf};

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::snapshot_test::{
    compare_lines, corpus_cases, screen_rows_spaced, settle_grid,
};

/// 上游序列化的期望可见屏。
#[derive(serde::Deserialize)]
pub struct ReferenceScreen {
    pub rows: u32,
    pub cols: u32,
    /// 语料终端声明的回滚上限。
    pub scrollback_limit: u32,
    /// 可见屏各行文本，行尾空白已裁剪，末尾空行已裁掉。
    pub screen: Vec<String>,
}

fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/terminal/testdata/conformance")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 输入与期望必须成对，未成对即失败。
    #[test]
    fn conformance_cases_are_paired() {
        let (cases, problems) = corpus_cases(&conformance_dir());
        assert!(!cases.is_empty(), "跨引擎语料为空");
        assert!(
            problems.is_empty(),
            "跨引擎语料输入与期望文件不成对：\n{}",
            problems.join("\n")
        );
    }

    /// 每份录音的可见屏文本、行序与网格尺寸 MUST 与独立引擎的期望完全一致。
    #[test]
    fn conformance_screen_matches_reference_engine() {
        let (cases, problems) = corpus_cases(&conformance_dir());
        let mut failures = problems;
        assert!(!cases.is_empty(), "跨引擎语料为空");
        for case in &cases {
            failures.extend(screen_matches_reference(case));
        }

        assert!(
            failures.is_empty(),
            "跨引擎语料不一致：\n{}",
            failures.join("\n")
        );
    }

    /// 跑一条录音，返回差异描述；读取、解析与建终端的失败都作为差异返回。
    fn screen_matches_reference(case: &crate::terminal::snapshot_test::CorpusCase) -> Vec<String> {
        let text = match std::fs::read_to_string(&case.expectation) {
            Ok(text) => text,
            Err(error) => return vec![format!("{}: 读取期望文件失败 {error}", case.name)],
        };
        let reference: ReferenceScreen = match serde_json::from_str(&text) {
            Ok(reference) => reference,
            Err(error) => return vec![format!("{}: 解析期望文件失败 {error}\n{text}", case.name)],
        };
        let input = match std::fs::read(&case.input) {
            Ok(input) => input,
            Err(error) => return vec![format!("{}: 读取语料输入失败 {error}", case.name)],
        };
        let mut terminal = match GhosttyTerminal::new(
            reference.rows,
            reference.cols,
            reference.scrollback_limit,
        ) {
            Ok(terminal) => terminal,
            Err(error) => return vec![format!("{}: 创建终端失败 {error}", case.name)],
        };
        terminal.vt_write(&input);
        let dumped = match settle_grid(&terminal, &case.name, reference.rows, reference.cols) {
            Ok(dumped) => dumped,
            Err(problem) => return vec![problem],
        };

        let mut actual = screen_rows_spaced(&dumped);
        while actual.last().is_some_and(|line| line.is_empty()) {
            actual.pop();
        }
        let mut differences = Vec::new();
        compare_lines(&reference.screen, &actual, "screen", &mut differences);
        if differences.is_empty() {
            return Vec::new();
        }
        vec![format!("{}:\n{}", case.name, differences.join("\n"))]
    }
}
