use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::terminal::ghostty_terminal::{CellSnapshot, DumpedGrid, GhosttyTerminal};

/// 快照格式版本，出现破坏性改动时递增。
const SNAPSHOT_VERSION: u32 = 1;

/// 语料统一使用的网格尺寸。
const CORPUS_ROWS: u32 = 6;
const CORPUS_COLS: u32 = 20;
const CORPUS_SCROLLBACK: u32 = 20;

/// 非默认样式的单元；默认样式不出现在期望文件中。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StyledCell {
    pub row: u32,
    pub col: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub foreground: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub background: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl StyledCell {
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if !self.foreground.is_empty() {
            parts.push(format!("fg={}", self.foreground));
        }
        if !self.background.is_empty() {
            parts.push(format!("bg={}", self.background));
        }
        if self.bold {
            parts.push("bold".to_string());
        }
        if self.italic {
            parts.push("italic".to_string());
        }
        if self.underline {
            parts.push("underline".to_string());
        }
        if self.reverse {
            parts.push("reverse".to_string());
        }
        parts.join(" ")
    }
}

/// 供回归测试使用的终端状态快照。
/// 与 `.seq` 输入文件并排存为 JSON 文件。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TestSnapshot {
    pub version: u32,
    pub rows: u32,
    pub cols: u32,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub cursor_visible: bool,
    /// 屏幕每行文本，已去除行尾空白与末尾空行。
    pub screen: Vec<String>,
    /// 屏幕内非默认样式单元。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styled: Vec<StyledCell>,
    /// 回滚区每行文本，已去除行尾空白。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scrollback: Vec<String>,
}

fn cell_char(cell: &CellSnapshot) -> String {
    if cell.codepoint == 0 {
        return String::new();
    }
    char::from_u32(cell.codepoint)
        .map(|character| character.to_string())
        .unwrap_or_default()
}

fn styled_color(channel: [f32; 4], is_default: bool) -> String {
    if is_default {
        return String::new();
    }
    format!(
        "{:02X}{:02X}{:02X}",
        (channel[0] * 255.0).round() as u8,
        (channel[1] * 255.0).round() as u8,
        (channel[2] * 255.0).round() as u8
    )
}

fn cell_styled(
    row: u32,
    col: u32,
    cell: &CellSnapshot,
    default_foreground: [f32; 4],
    default_background: [f32; 4],
) -> Option<StyledCell> {
    let foreground_is_default = cell.foreground == default_foreground;
    let background_is_default = cell.background == default_background;
    if foreground_is_default
        && background_is_default
        && !cell.bold
        && !cell.italic
        && !cell.underline
        && !cell.reverse
    {
        return None;
    }
    Some(StyledCell {
        row,
        col,
        foreground: styled_color(cell.foreground, foreground_is_default),
        background: styled_color(cell.background, background_is_default),
        bold: cell.bold,
        italic: cell.italic,
        underline: cell.underline,
        reverse: cell.reverse,
    })
}

fn row_text(cells: &[CellSnapshot]) -> String {
    let text: String = cells.iter().map(cell_char).collect();
    text.trim_end().to_string()
}

fn collect_styled(
    cells: &[CellSnapshot],
    cols: u32,
    default_foreground: [f32; 4],
    default_background: [f32; 4],
    styled: &mut Vec<StyledCell>,
) {
    for (index, cell) in cells.iter().enumerate() {
        let row = index as u32 / cols;
        let col = index as u32 % cols;
        if let Some(entry) =
            cell_styled(row, col, cell, default_foreground, default_background)
        {
            styled.push(entry);
        }
    }
}

/// 从当前终端状态采集 `TestSnapshot`。
pub fn capture_snapshot(terminal: &GhosttyTerminal) -> TestSnapshot {
    let dumped = terminal.dump_grid();
    let cursor_x = terminal.cursor_x();
    let cursor_y = terminal.cursor_y();
    let cursor_visible = terminal.cursor_visible();
    from_dumped_grid(&dumped, cursor_x, cursor_y, cursor_visible)
}

fn from_dumped_grid(
    dumped: &DumpedGrid,
    cursor_x: u32,
    cursor_y: u32,
    cursor_visible: bool,
) -> TestSnapshot {
    let mut screen = Vec::with_capacity(dumped.rows as usize);
    let mut styled = Vec::new();
    for row in 0..dumped.rows as usize {
        let start = row * dumped.cols as usize;
        let end = start + dumped.cols as usize;
        screen.push(row_text(&dumped.visible[start..end]));
        collect_styled(
            &dumped.visible[start..end],
            dumped.cols,
            dumped.default_foreground,
            dumped.default_background,
            &mut styled,
        );
    }
    while screen.last().is_some_and(|line| line.is_empty()) {
        screen.pop();
    }

    TestSnapshot {
        version: SNAPSHOT_VERSION,
        rows: dumped.rows,
        cols: dumped.cols,
        cursor_row: cursor_y,
        cursor_col: cursor_x,
        cursor_visible,
        screen,
        styled,
        scrollback: dumped.scrollback.iter().map(|row| row_text(row)).collect(),
    }
}

/// 两个快照的比较结果，每项为一条可读差异。
#[derive(Debug, Default)]
pub struct DiffResult {
    pub differences: Vec<String>,
}

impl DiffResult {
    pub fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }
}

fn compare_lines(expected: &[String], actual: &[String], label: &str, differences: &mut Vec<String>) {
    let count = expected.len().max(actual.len());
    for index in 0..count {
        let left = expected.get(index).map(String::as_str).unwrap_or("");
        let right = actual.get(index).map(String::as_str).unwrap_or("");
        if left != right {
            differences.push(format!("{label} {index}: expected {left:?} got {right:?}"));
        }
    }
}

/// 比较两个快照并返回差异。
pub fn diff(expected: &TestSnapshot, actual: &TestSnapshot) -> DiffResult {
    let mut differences = Vec::new();

    if expected.version != actual.version {
        differences.push(format!(
            "version {} expected, {} actual",
            expected.version, actual.version
        ));
    }
    if expected.rows != actual.rows || expected.cols != actual.cols {
        differences.push(format!(
            "size {}x{} expected, {}x{} actual",
            expected.cols, expected.rows, actual.cols, actual.rows
        ));
    } else {
        compare_lines(&expected.screen, &actual.screen, "screen", &mut differences);
        compare_lines(
            &expected.scrollback,
            &actual.scrollback,
            "scrollback",
            &mut differences,
        );
    }

    if expected.cursor_row != actual.cursor_row
        || expected.cursor_col != actual.cursor_col
        || expected.cursor_visible != actual.cursor_visible
    {
        differences.push(format!(
            "cursor ({},{}) vis={} expected, ({},{}) vis={} actual",
            expected.cursor_row,
            expected.cursor_col,
            expected.cursor_visible,
            actual.cursor_row,
            actual.cursor_col,
            actual.cursor_visible
        ));
    }

    if expected.styled.len() != actual.styled.len() {
        differences.push(format!(
            "styled {} cells expected, {} actual",
            expected.styled.len(),
            actual.styled.len()
        ));
    }
    for (left, right) in expected.styled.iter().zip(actual.styled.iter()) {
        if left != right {
            differences.push(format!(
                "styled ({},{}): expected [{}] got [{}]",
                left.row,
                left.col,
                left.describe(),
                right.describe()
            ));
        }
    }

    DiffResult { differences }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn make_terminal(rows: u32, cols: u32) -> GhosttyTerminal {
        GhosttyTerminal::new(rows, cols, 1000).expect("terminal")
    }

    fn corpus_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/terminal/testdata")
    }

    fn corpus_files(extension: &str) -> BTreeSet<String> {
        let mut names: Vec<PathBuf> = fs::read_dir(corpus_dir())
            .expect("read corpus directory")
            .map(|entry| entry.expect("corpus entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == extension))
            .collect();
        names.sort();
        names
            .iter()
            .map(|path| path.file_stem().expect("corpus stem").to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn capture_empty_snapshot() {
        let terminal = make_terminal(24, 80);
        let snapshot = capture_snapshot(&terminal);
        assert_eq!(snapshot.rows, 24);
        assert_eq!(snapshot.cols, 80);
        assert!(snapshot.cursor_visible);
        assert!(snapshot.screen.is_empty());
        assert!(snapshot.scrollback.is_empty());
    }

    #[test]
    fn capture_with_content() {
        let mut terminal = make_terminal(3, 10);
        terminal.vt_write(b"Hi");
        terminal.flush();
        let snapshot = capture_snapshot(&terminal);
        assert_eq!(snapshot.screen, vec!["Hi".to_string()]);
        assert_eq!(snapshot.cursor_row, 0);
        assert_eq!(snapshot.cursor_col, 2);
    }

    #[test]
    fn capture_keeps_non_default_style_only() {
        let mut terminal = make_terminal(3, 10);
        terminal.vt_write(b"\x1b[1;31mE\x1b[0mp");
        terminal.flush();
        let snapshot = capture_snapshot(&terminal);
        assert_eq!(snapshot.screen, vec!["Ep".to_string()]);
        assert_eq!(snapshot.styled.len(), 1);
        assert_eq!(snapshot.styled[0].col, 0);
        assert!(snapshot.styled[0].bold);
        assert!(!snapshot.styled[0].foreground.is_empty());
        assert!(snapshot.styled[0].background.is_empty());
    }

    #[test]
    fn capture_trims_trailing_blank_rows() {
        let mut terminal = make_terminal(5, 10);
        terminal.vt_write(b"a\r\n\r\nb\r\n");
        terminal.flush();
        let snapshot = capture_snapshot(&terminal);
        assert_eq!(snapshot.screen, vec!["a".to_string(), String::new(), "b".to_string()]);
    }

    #[test]
    fn diff_identical_is_empty() {
        let terminal = make_terminal(3, 5);
        let expected = capture_snapshot(&terminal);
        let actual = capture_snapshot(&terminal);
        assert!(diff(&expected, &actual).is_empty());
    }

    #[test]
    fn diff_detects_content_change() {
        let terminal = make_terminal(3, 5);
        let expected = capture_snapshot(&terminal);
        let mut actual = expected.clone();
        actual.screen = vec!["X".to_string()];
        let result = diff(&expected, &actual);
        assert_eq!(result.differences.len(), 1);
        assert!(result.differences[0].starts_with("screen 0:"));
    }

    #[test]
    fn diff_detects_cursor_change() {
        let terminal = make_terminal(3, 5);
        let expected = capture_snapshot(&terminal);
        let mut actual = expected.clone();
        actual.cursor_col = 4;
        let result = diff(&expected, &actual);
        assert_eq!(result.differences.len(), 1);
        assert!(result.differences[0].starts_with("cursor "));
    }

    #[test]
    fn diff_detects_style_change() {
        let terminal = make_terminal(3, 5);
        let expected = capture_snapshot(&terminal);
        let mut actual = expected.clone();
        actual.styled = vec![StyledCell {
            row: 0,
            col: 1,
            foreground: String::new(),
            background: String::new(),
            bold: true,
            italic: false,
            underline: false,
            reverse: false,
        }];
        let result = diff(&expected, &actual);
        assert_eq!(result.differences.len(), 1);
        assert!(result.differences[0].starts_with("styled "));
    }

    #[test]
    fn diff_detects_dimension_mismatch() {
        let terminal = make_terminal(3, 5);
        let expected = capture_snapshot(&terminal);
        let other = make_terminal(4, 5);
        let actual = capture_snapshot(&other);
        let result = diff(&expected, &actual);
        assert_eq!(result.differences.len(), 1);
        assert!(result.differences[0].starts_with("size "));
    }

    #[test]
    fn serde_round_trip() {
        let mut terminal = make_terminal(3, 10);
        terminal.vt_write(b"\x1b[4mHello\nWorld");
        terminal.flush();
        let snapshot = capture_snapshot(&terminal);
        let json = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");
        let restored: TestSnapshot = serde_json::from_str(&json).expect("deserialize snapshot");
        assert!(diff(&snapshot, &restored).is_empty(), "{:?}", diff(&snapshot, &restored));
    }

    #[test]
    fn scrollback_captured_in_order() {
        let mut terminal = GhosttyTerminal::new(2, 10, 100).expect("terminal");
        for index in 0..6 {
            terminal.vt_write(format!("line{index}\r\n").as_bytes());
        }
        terminal.flush();
        let snapshot = capture_snapshot(&terminal);
        assert_eq!(
            snapshot.scrollback,
            vec![
                "line0".to_string(),
                "line1".to_string(),
                "line2".to_string(),
                "line3".to_string(),
                "line4".to_string()
            ]
        );
        assert_eq!(snapshot.screen, vec!["line5".to_string()]);
    }

    #[test]
    fn corpus_pairs_are_complete() {
        let inputs = corpus_files("seq");
        let expectations = corpus_files("json");
        assert!(!inputs.is_empty(), "回归语料为空");
        assert_eq!(inputs, expectations, "回归语料输入与期望文件不成对");
    }

    #[test]
    fn corpus_matches_expectation() {
        let mut failures = Vec::new();
        for name in corpus_files("seq") {
            let input_path = corpus_dir().join(format!("{name}.seq"));
            let expectation_path = corpus_dir().join(format!("{name}.json"));
            let bytes = fs::read(&input_path).expect("read corpus input");

            let mut terminal =
                GhosttyTerminal::new(CORPUS_ROWS, CORPUS_COLS, CORPUS_SCROLLBACK)
                    .expect("corpus terminal");
            terminal.vt_write(&bytes);
            terminal.flush();
            let actual = capture_snapshot(&terminal);
            let rendered = serde_json::to_string_pretty(&actual).expect("serialize snapshot");

            let expected_json = match fs::read_to_string(&expectation_path) {
                Ok(json) => json,
                Err(error) => {
                    failures.push(format!("{name}: 读取期望文件失败 {error}\n{rendered}"));
                    continue;
                }
            };
            let expected: TestSnapshot = match serde_json::from_str(&expected_json) {
                Ok(expected) => expected,
                Err(error) => {
                    failures.push(format!("{name}: 解析期望文件失败 {error}\n{rendered}"));
                    continue;
                }
            };

            let result = diff(&expected, &actual);
            if !result.is_empty() {
                failures.push(format!(
                    "{name}:\n{}\n{rendered}",
                    result.differences.join("\n")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "回归语料不一致：\n{}",
            failures.join("\n")
        );
    }
}
