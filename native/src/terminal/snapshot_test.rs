use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::terminal::ghostty_terminal::{CellSnapshot, DumpedGrid, GhosttyTerminal};

/// 快照格式版本，出现破坏性改动时递增。
const SNAPSHOT_VERSION: u32 = 2;
/// 语料的刷新确认与查询就绪时限。
/// `dump_grid` 的单次查询预算为 `QUERY_TIMEOUT_MS`，繁忙时回退空网格；
/// 刷新确认与查询就绪是彼此独立的两个预算，故在此统一给出上界。
pub(crate) const CORPUS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// 就绪轮询间隔。
pub(crate) const CORPUS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

/// 非默认样式的单元；默认样式不出现在期望文件中。
#[derive(Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Clone, Serialize, Deserialize)]
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
    /// 语料终端声明的回滚上限；同一份语料的尺寸与上限一起决定解析行为。
    pub scrollback_limit: u32,
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

/// 语料锁定前景色、背景色、粗体、斜体、下划线与反显；其余属性（SGR 2/5/8/9/53）
/// 由 `ghostty_terminal` 的针对性用例覆盖。
fn cell_styled(row: u32, col: u32, cell: &CellSnapshot) -> Option<StyledCell> {
    if cell.foreground_is_default
        && cell.background_is_default
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
        foreground: styled_color(cell.foreground, cell.foreground_is_default),
        background: styled_color(cell.background, cell.background_is_default),
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

/// 空白单元渲染成空格的一行文本，只裁行尾空白。
///
/// `row_text` 把未写入单元表示为空串（码位 0），那是本项目快照的存储约定；
/// 此处保持单元的空格形状，供按字节比对上游语料——上游把未写入单元序列化为
/// 空格。两者互补，不可互相替代。
pub(crate) fn row_text_spaced(cells: &[CellSnapshot]) -> String {
    cells
        .iter()
        .map(|cell| {
            if cell.codepoint == 0 {
                ' '
            } else {
                char::from_u32(cell.codepoint).unwrap_or('?')
            }
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// 可见屏各行的 `row_text_spaced` 取值。
pub(crate) fn screen_rows_spaced(dumped: &DumpedGrid) -> Vec<String> {
    (0..dumped.rows as usize)
        .map(|row| {
            let start = row * dumped.cols as usize;
            row_text_spaced(&dumped.visible[start..start + dumped.cols as usize])
        })
        .collect()
}

fn collect_styled(row: u32, cells: &[CellSnapshot], styled: &mut Vec<StyledCell>) {
    for (col, cell) in cells.iter().enumerate() {
        if let Some(entry) = cell_styled(row, col as u32, cell) {
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
        collect_styled(row as u32, &dumped.visible[start..end], &mut styled);
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
        scrollback_limit: 0,
    }
}

/// 一条语料：`.seq` 原始字节输入与同名 `.json` 期望快照。
pub(crate) struct CorpusCase {
    /// 诊断标识：语料目录名 + 词干，使多份语料的报告可辨来源。
    pub name: String,
    pub input: std::path::PathBuf,
    pub expectation: std::path::PathBuf,
}

/// 扫描语料目录下成对的 `.seq` 与 `.json`，返回用例与未成对文件的问题描述。
/// 子目录不参与扫描，使 `seeds/` `conformance/` 等独立语料与顶层语料互不干扰。
pub(crate) fn corpus_cases(directory: &std::path::Path) -> (Vec<CorpusCase>, Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return (Vec::new(), Vec::new());
    };
    let source = directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let label = |stem: &str| format!("{source}/{stem}");
    let mut inputs = BTreeSet::new();
    let mut expectations = BTreeSet::new();
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Some(stem) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
        else {
            continue;
        };
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("seq") => {
                inputs.insert(stem);
            }
            Some("json") => {
                expectations.insert(stem);
            }
            _ => {}
        }
    }
    let mut problems = Vec::new();
    for name in inputs.difference(&expectations) {
        problems.push(format!("{}: 缺少同名 .json 期望文件", label(name)));
    }
    for name in expectations.difference(&inputs) {
        problems.push(format!("{}: 缺少同名 .seq 输入文件", label(name)));
    }
    let cases = inputs
        .intersection(&expectations)
        .map(|name| CorpusCase {
            name: label(name),
            input: directory.join(format!("{name}.seq")),
            expectation: directory.join(format!("{name}.json")),
        })
        .collect();
    (cases, problems)
}

/// 等到终端可以交付比对的状态：刷新已确认、网格尺寸就绪。
///
/// `dump_grid` 的查询超时是独立预算且会回退空网格，直接采集会把未就绪的网格
/// 报成内容不符，故须轮询到尺寸匹配为止。返回一条可读问题描述而非跳过语料。
pub(crate) fn settle_grid(
    terminal: &GhosttyTerminal,
    case: &str,
    rows: u32,
    cols: u32,
) -> Result<DumpedGrid, String> {
    if !terminal.flush_with_timeout(CORPUS_TIMEOUT) {
        return Err(format!("{case}: flush 未在超时内确认"));
    }
    let deadline = std::time::Instant::now() + CORPUS_TIMEOUT;
    loop {
        let dumped = terminal.dump_grid();
        if dumped.rows == rows && dumped.cols == cols {
            return Ok(dumped);
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("{case}: 网格快照未在超时内就绪"));
        }
        std::thread::sleep(CORPUS_POLL_INTERVAL);
    }
}

/// 运行单条语料：按期望声明创建终端、写入输入、确认刷新与网格就绪、逐项比对。
/// 期望文件缺失或损坏时返回差异项而非跳过该语料。
pub(crate) fn run_corpus_case(case: &CorpusCase) -> Vec<String> {
    let Ok(bytes) = std::fs::read(&case.input) else {
        return vec![format!("{}: 读取语料输入失败", case.name)];
    };
    let expectation_text = match std::fs::read_to_string(&case.expectation) {
        Ok(text) => text,
        Err(error) => return vec![format!("{}: 读取期望文件失败 {error}", case.name)],
    };
    let expected: TestSnapshot = match serde_json::from_str(&expectation_text) {
        Ok(expected) => expected,
        Err(error) => {
            return vec![format!(
                "{}: 解析期望文件失败 {error}\n{expectation_text}",
                case.name
            )];
        }
    };

    let mut terminal =
        match GhosttyTerminal::new(expected.rows, expected.cols, expected.scrollback_limit) {
            Ok(terminal) => terminal,
            Err(error) => return vec![format!("{}: 创建终端失败 {error}", case.name)],
        };
    terminal.vt_write(&bytes);
    let dumped = match settle_grid(&terminal, &case.name, expected.rows, expected.cols) {
        Ok(dumped) => dumped,
        Err(problem) => return vec![problem],
    };
    let actual = from_dumped_grid(
        &dumped,
        terminal.cursor_x(),
        terminal.cursor_y(),
        terminal.cursor_visible(),
    );

    let result = diff(&expected, &actual);
    if result.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "{}:\n{}\n{}",
        case.name,
        result.differences.join("\n"),
        serde_json::to_string_pretty(&actual).expect("序列化快照")
    )]
}

/// 两个快照的比较结果，每项为一条可读差异。
#[derive(Debug)]
pub struct DiffResult {
    pub differences: Vec<String>,
}

impl DiffResult {
    pub fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }
}

/// 逐行比对两段文本，缺失的行按空串计。每项差异带 `label` 与行号。
pub(crate) fn compare_lines(
    expected: &[String],
    actual: &[String],
    label: &str,
    differences: &mut Vec<String>,
) {
    let count = expected.len().max(actual.len());
    for index in 0..count {
        let left = expected.get(index).map(String::as_str).unwrap_or("");
        let right = actual.get(index).map(String::as_str).unwrap_or("");
        if left != right {
            differences.push(format!("{label} {index}: expected {left:?} got {right:?}"));
        }
    }
}

/// 按坐标索引；重复坐标记入 `differences` 而非中断，使其与其余语料一并报告。
fn index_styled<'a>(
    cells: &'a [StyledCell],
    origin: &str,
    differences: &mut Vec<String>,
) -> BTreeMap<(u32, u32), &'a StyledCell> {
    let mut map = BTreeMap::new();
    for cell in cells {
        let coordinate = (cell.row, cell.col);
        if map.insert(coordinate, cell).is_some() {
            differences.push(format!("styled {coordinate:?}: {origin}含重复坐标"));
        }
    }
    map
}

fn compare_styled(expected: &[StyledCell], actual: &[StyledCell], differences: &mut Vec<String>) {
    let left = index_styled(expected, "期望", differences);
    let right = index_styled(actual, "实际", differences);
    let coordinates: BTreeSet<(u32, u32)> = left.keys().chain(right.keys()).copied().collect();
    for coordinate in coordinates {
        match (left.get(&coordinate), right.get(&coordinate)) {
            (Some(expected_cell), Some(actual_cell)) if expected_cell == actual_cell => {}
            (Some(expected_cell), Some(actual_cell)) => differences.push(format!(
                "styled {coordinate:?}: expected [{}] got [{}]",
                expected_cell.describe(),
                actual_cell.describe()
            )),
            (Some(expected_cell), None) => differences.push(format!(
                "styled {coordinate:?}: expected [{}] got [none]",
                expected_cell.describe()
            )),
            (None, Some(actual_cell)) => differences.push(format!(
                "styled {coordinate:?}: expected [none] got [{}]",
                actual_cell.describe()
            )),
            (None, None) => {}
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

    compare_styled(&expected.styled, &actual.styled, &mut differences);

    DiffResult { differences }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_terminal(rows: u32, cols: u32) -> GhosttyTerminal {
        GhosttyTerminal::new(rows, cols, 1000).expect("terminal")
    }

    fn corpus_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/terminal/testdata")
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
        assert_eq!(
            snapshot.screen,
            vec!["a".to_string(), String::new(), "b".to_string()]
        );
    }

    #[test]
    fn styled_cells_locate_rows_and_columns() {
        let mut terminal = make_terminal(2, 10);
        terminal.vt_write(b"\x1b[1mA\r\nB");
        terminal.flush();
        let snapshot = capture_snapshot(&terminal);
        let coordinates: Vec<(u32, u32)> = snapshot
            .styled
            .iter()
            .map(|entry| (entry.row, entry.col))
            .collect();
        assert_eq!(coordinates, vec![(0, 0), (1, 0)]);
    }

    /// 显式色恰好等于默认色取值时仍须记入 `styled`。
    /// 判据是颜色来源而非数值相等；此用例是该区别的唯一守卫。
    #[test]
    fn capture_keeps_explicit_color_equal_to_default() {
        let mut terminal = make_terminal(3, 10);
        // 205,214,244 即 Catppuccin Mocha 默认前景，与未设置色解算结果相同。
        terminal.vt_write(b"\x1b[38;2;205;214;244mX");
        terminal.flush();
        let snapshot = capture_snapshot(&terminal);
        assert_eq!(snapshot.styled.len(), 1);
        assert_eq!((snapshot.styled[0].row, snapshot.styled[0].col), (0, 0));
        assert_eq!(snapshot.styled[0].foreground, "CDD6F4");
    }

    /// 重复坐标必须成为差异项而非中断，且不掩盖同一次比对中的其它差异。
    #[test]
    fn diff_reports_duplicate_coordinates_without_masking_other_differences() {
        let terminal = make_terminal(3, 5);
        let expected = capture_snapshot(&terminal);
        let mut actual = expected.clone();
        actual.styled = vec![
            StyledCell {
                row: 1,
                col: 2,
                foreground: String::new(),
                background: String::new(),
                bold: true,
                italic: false,
                underline: false,
                reverse: false,
            },
            StyledCell {
                row: 1,
                col: 2,
                foreground: String::new(),
                background: String::new(),
                bold: false,
                italic: false,
                underline: false,
                reverse: false,
            },
        ];
        actual.screen = vec!["X".to_string()];

        let result = diff(&expected, &actual);
        assert!(
            result
                .differences
                .iter()
                .any(|difference| difference.contains("重复坐标")),
            "{:?}",
            result.differences
        );
        assert!(
            result
                .differences
                .iter()
                .any(|difference| difference.starts_with("screen 0:")),
            "{:?}",
            result.differences
        );
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
        assert!(
            diff(&snapshot, &restored).is_empty(),
            "{:?}",
            diff(&snapshot, &restored)
        );
    }

    /// 顶层语料的输入与期望文件必须成对：未成对即失败，不静默跳过。
    #[test]
    fn corpus_pairs_are_complete() {
        let (_, problems) = corpus_cases(&corpus_dir());
        assert!(
            problems.is_empty(),
            "回归语料输入与期望文件不成对：\n{}",
            problems.join("\n")
        );
    }

    /// 顶层语料的整体快照比对；非空断言在运行器内，使按名字单独过滤也不会空跑通过。
    #[test]
    fn corpus_matches_expectation() {
        let (cases, problems) = corpus_cases(&corpus_dir());
        let mut failures = problems;
        assert!(!cases.is_empty(), "回归语料为空");
        for case in &cases {
            failures.extend(run_corpus_case(case));
        }

        assert!(
            failures.is_empty(),
            "回归语料不一致：\n{}",
            failures.join("\n")
        );
    }
}
