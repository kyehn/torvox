//! 解析器分块写入不变性：上游手写种子语料驱动。
//!
//! 语料来自 Ghostty（MIT，`test/fuzz-libghostty/corpus/` 的 `parser-initial`
//! `stream-initial` `osc-initial` 三组手写种子）。上游各 fuzz 目标用输入首字节
//! 选择目标内部路径（切片/标量路径、OSC 终止符变体），该字节不属于终端输入，
//! 导入时已逐文件去除，见 `testdata/seeds/README.md`。
//!
//! 断言的性质属于本项目而非上游：PTY 读取按块到达、IME 提交按小写入到达，两者
//! 都经同一条有界命令通道投递（`GhosttyTerminal::vt_write` 满则丢弃）。因此
//! 「最终状态与切块方式无关」只有在每块都被确认排空后才成立，测试必须逐块确认，
//! 否则投递层的丢弃会被误报成解析缺陷。

use std::path::{Path, PathBuf};

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::snapshot_test::{
    CORPUS_TIMEOUT, row_text_spaced, screen_rows_spaced, settle_grid,
};

/// 与上游 fuzz 目标一致的网格规格。
const SEED_ROWS: u32 = 24;
const SEED_COLS: u32 = 80;
const SEED_SCROLLBACK: u32 = 100;

/// 终端在交付比对前必须达到的稳定状态：刷新已确认、网格已就绪。
#[derive(PartialEq)]
struct SeedState {
    rows: u32,
    cols: u32,
    cursor_x: u32,
    cursor_y: u32,
    screen: Vec<String>,
    scrollback: Vec<String>,
}

impl SeedState {
    fn capture(terminal: &GhosttyTerminal, case: &str) -> Result<Self, String> {
        let dumped = settle_grid(terminal, case, SEED_ROWS, SEED_COLS)?;
        Ok(Self {
            rows: dumped.rows,
            cols: dumped.cols,
            cursor_x: terminal.cursor_x(),
            cursor_y: terminal.cursor_y(),
            screen: screen_rows_spaced(&dumped),
            scrollback: dumped
                .scrollback
                .iter()
                .map(|row| row_text_spaced(row))
                .collect(),
        })
    }
}

fn seeds_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/terminal/testdata/seeds")
}

/// 目录名 → 已去除首字节的种子字节。
fn seed_inputs() -> Vec<(String, Vec<u8>)> {
    let mut corpora: Vec<PathBuf> = std::fs::read_dir(seeds_dir())
        .expect("read seeds directory")
        .map(|entry| entry.expect("seeds entry").path())
        .filter(|path| path.is_dir())
        .collect();
    corpora.sort();
    let mut seeds = Vec::new();
    for directory in corpora {
        let corpus = directory
            .file_name()
            .expect("corpus name")
            .to_string_lossy()
            .into_owned();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&directory)
            .expect("read corpus directory")
            .map(|entry| entry.expect("corpus entry").path())
            .filter(|path| path.is_file())
            .collect();
        entries.sort();
        for path in entries {
            let name = format!(
                "{corpus}/{}",
                path.file_name().expect("seed name").to_string_lossy()
            );
            let bytes = std::fs::read(&path).expect("read seed");
            seeds.push((name, bytes));
        }
    }
    seeds
}

fn seed_terminal() -> GhosttyTerminal {
    GhosttyTerminal::new(SEED_ROWS, SEED_COLS, SEED_SCROLLBACK).expect("seed terminal")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同一份种子的最终状态 MUST 与写入切块方式无关。
    #[test]
    fn seed_final_state_is_chunk_independent() {
        let seeds = seed_inputs();
        assert!(!seeds.is_empty(), "种子语料为空");
        let mut failures = Vec::new();
        for (name, bytes) in &seeds {
            let mut whole = seed_terminal();
            whole.vt_write(bytes);
            let expected = match SeedState::capture(&whole, name) {
                Ok(state) => state,
                Err(problem) => {
                    failures.push(problem);
                    continue;
                }
            };

            let mut per_byte = seed_terminal();
            let mut drained = true;
            for byte in bytes {
                per_byte.vt_write(&[*byte]);
                // 逐字节走同一条有界命令通道，必须逐块确认排空；未确认时通道
                // 容量耗尽即丢弃，会把投递层的丢弃误报成解析差异。
                drained &= per_byte.flush_with_timeout(CORPUS_TIMEOUT);
            }
            if !drained {
                failures.push(format!("{name}: flush 未在超时内确认"));
                continue;
            }
            let actual = match SeedState::capture(&per_byte, name) {
                Ok(state) => state,
                Err(problem) => {
                    failures.push(problem);
                    continue;
                }
            };
            if expected != actual {
                failures.push(format!("{name}: 逐字节写入的最终状态与整块写入不一致"));
            }
        }

        assert!(
            failures.is_empty(),
            "解析器分块写入不一致：\n{}",
            failures.join("\n")
        );
    }
}
