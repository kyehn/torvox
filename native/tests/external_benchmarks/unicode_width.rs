//! 显示宽度的外部基准：Unicode UCD 当唯一真值，回读本仓网格里每个单元的宽度。
//!
//! 两份数据都在运行期现取（不入库，理由见 [`support`]）：
//!
//! - `EastAsianWidth.txt`（UAX #11）：`W`/`F` 两类必占两列，`Na`/`H`/`N`/`A` 占一列。
//! - `extracted/DerivedGeneralCategory.txt`：`Mn` 类是组合符，不单独占列。
//!
//! 判定范围是**全部**已分配的可打印码位（`L`/`N`/`P`/`S`，17 万量级），不做抽样：
//! 抽样留出的分歧正是这类基准最容易放过的东西。控制符、格式符、私用区与未分配码位
//! 写进网格后不存在「占几列」这回事，纳入比对只会得到无意义的分歧，故不参与。
//!
//! 期望宽度由上述两个文件推导，因此 UCD 年度发布调整分类只会改期望、不会改用例的
//! 稳定性；真判红说明本仓渲染与最新版 UCD 不符，那是要查的信号，不是噪声。

use super::support;

use native::terminal::ghostty_terminal::GhosttyTerminal;
use support::fetch_text;

/// UCD 的对外取数入口。用 `latest` 而非钉死版本号：期望值随同一份数据推导，版本
/// 前进只改期望、不改稳定性。
const UCD_BASE: &str = "https://www.unicode.org/Public/UCD/latest/ucd";

/// 每批比对的码位数。批次决定网格规模与单次快照成本：再大则内存随可打印码位总数
/// 线性上涨，再小则 VT 线程反复起停。
const BATCH: usize = 4096;

/// 每行放多少个码位。字与字之间写一个空格分隔：相邻码位若能合成字素簇（两个区域
/// 指示符即成国旗），不隔开就会并成一格，按格回读便无从对齐。
const PER_ROW: usize = 16;

/// 网格列数：每格至多两列再加一个分隔空格，`PER_ROW × 3` 为上界，再留一列余量。
const COLUMNS: u32 = PER_ROW as u32 * 3 + 1;

/// 组合符样本上限。
///
/// 「基础字母 + 组合符」每对在 debug 档约 12ms：`cargo test` 的 debug 档把 Zig 依赖
/// 也编成未优化（上游构建脚本按 cargo 的 `DEBUG` 选 `-Doptimize=Debug`），实测 1600
/// 对在 debug 档要 21s、在 release 档只要 1.6ms。故零宽回放按等距抽样限量——全量
/// 2090 个组合符会顶穿 `flush` 的 5s 上限，让随后的快照查询超时，那是构建档差异而
/// 不是被测行为。
const COMBINING_SAMPLE_LIMIT: usize = 128;

/// 终端宽度惯例：UAX #11 描述的是「东亚显示宽度」，终端网格另有既成约定，上游宽度
/// 表按这些约定取值。全量比对实测 17 万个可打印码位里只有下列几处与之不同，逐条
/// 钉在这里——它们不是待修的偏差，而是终端与 UCD 的既定分歧；上游一旦改动某处，
/// 本用例即判红要求复核，不让新约定无声溜进来。
///
/// `(左, 右, 终端宽度)`，右闭区间。
const TERMINAL_WIDTH_OVERRIDES: &[(u32, u32, u8)] = &[
    // 韩文组合型中声/终声：与前一音节合成一个音节，不单独占列（U+1160、U+11FF 为填充符）。
    (0x1160, 0x11FF, 0),
    // 韩文字母扩展 B 的组合型字母，规则同上。
    (0xD7B0, 0xD7C6, 0),
    (0xD7CB, 0xD7FB, 0),
    // 小型谚文字母的连写形。
    (0x16D63, 0x16D63, 0),
    (0x16D67, 0x16D6A, 0),
    // 韩文初声/中声填充符：UAX 归为 W，终端按零宽连写处理。
    (0x115F, 0x115F, 0),
    (0x3164, 0x3164, 0),
    // 半宽韩文填充符：UAX 归为 H，终端同样按零宽处理。
    (0xFFA0, 0xFFA0, 0),
    // 二倍/三倍长破折号：字形本身两格宽，UAX 的 N 未反映字形宽度。
    (0x2E3A, 0x2E3B, 2),
    // 区域指示符：两两成国旗 emoji 占两格，单独出现同样按 emoji 呈现取两格。
    (0x1F1E6, 0x1F1FF, 2),
];

fn terminal_width_override(codepoint: u32) -> Option<u8> {
    TERMINAL_WIDTH_OVERRIDES
        .iter()
        .find(|(left, right, _)| (*left..=*right).contains(&codepoint))
        .map(|(_, _, width)| *width)
}

fn parse_hex(text: &str) -> u32 {
    let trimmed = text.trim();
    let digits = trimmed
        .strip_prefix("U+")
        .or_else(|| trimmed.strip_prefix("u+"))
        .unwrap_or(trimmed);
    u32::from_str_radix(digits, 16)
        .unwrap_or_else(|error| panic!("UCD 码位解析失败 {trimmed:?}：{error}"))
}

/// UCD 文件的一段码位区间及其属性值（`Left[..Right];Value`，两份文件同形）。
///
/// 保留区间而非逐码位展开：`DerivedGeneralCategory` 有三十余万码位，展开成映射既
/// 费内存也慢，而二分查找区间是同一份数据的直接表示。
struct PropertyRanges(Vec<(u32, u32, String)>);

impl PropertyRanges {
    fn parse(text: &str) -> Self {
        let mut ranges = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((range, value)) = line.split_once(';') else {
                panic!("UCD 行缺分号：{line}");
            };
            // 值后允许跟 `#` 注释（如 `Lu # L&  [26] …`）。
            let value = value.split('#').next().unwrap_or_default().trim();
            let range = range.trim();
            let (left, right) = match range.split_once("..") {
                Some((left, right)) => (parse_hex(left), parse_hex(right)),
                None => {
                    let single = parse_hex(range);
                    (single, single)
                }
            };
            assert!(left <= right, "UCD 区间倒置：{line}");
            ranges.push((left, right, value.to_string()));
        }
        assert!(!ranges.is_empty(), "UCD 文件解析出零区间");
        ranges.sort_by_key(|(left, _, _)| *left);
        Self(ranges)
    }

    fn get(&self, codepoint: u32) -> Option<&str> {
        let index = self
            .0
            .partition_point(|(left, _, _)| *left <= codepoint)
            .checked_sub(1)?;
        let (left, right, value) = &self.0[index];
        (*left..=*right)
            .contains(&codepoint)
            .then_some(value.as_str())
    }
}

/// UAX #11 给出的列数：`W`/`F` 两列，其余一列。
fn east_asian_width_columns(east_asian_width: &str) -> u8 {
    match east_asian_width {
        "W" | "F" => 2,
        _ => 1,
    }
}

/// 取两段 UCD 数据。同一份数据既给期望值也给样本集，版本前进时两者同步前进。
fn ucd() -> (PropertyRanges, PropertyRanges) {
    (
        PropertyRanges::parse(&fetch_text(&format!("{UCD_BASE}/EastAsianWidth.txt"))),
        PropertyRanges::parse(&fetch_text(&format!(
            "{UCD_BASE}/extracted/DerivedGeneralCategory.txt"
        ))),
    )
}

fn character_of(codepoint: u32) -> char {
    char::from_u32(codepoint).expect("样本码位是合法标量")
}

/// 把码位编码进载荷。
///
/// 整批一次写入，与生产的 PTY 读取路径同形：`pty_write` 按块投递，命令通道有界，
/// 逐码位各投一块会在通道排空前被丢弃（该路径按设计不阻塞调用方），回读到的便不是
/// 写进去的东西了。
fn encode_codepoints(codepoints: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut buffer = Vec::new();
    for codepoint in codepoints {
        let mut encoded = [0u8; 4];
        buffer.extend_from_slice(character_of(codepoint).encode_utf8(&mut encoded).as_bytes());
    }
    buffer
}

/// 把「每行 `PER_ROW` 个码位、字间以空格分隔」的样本写进网格，逐格回读宽度。
///
/// 走生产写入路径（`pty_write` → VT → 快照）：断言的是本仓可观测结果，不碰上游
/// 宽度表本身。分隔空格顺带钉住另一件事——每个码位恰好推进它应推进的列数。
fn observe_layout(samples: &[(u32, u8)]) -> Vec<String> {
    let rows = samples.len().div_ceil(PER_ROW) as u32 + 1;
    let mut payload = Vec::new();
    for chunk in samples.chunks(PER_ROW) {
        for (codepoint, _) in chunk {
            payload.extend_from_slice(&encode_codepoints([*codepoint]));
            payload.push(b' ');
        }
        payload.push(b'\n');
    }
    let mut terminal = GhosttyTerminal::new(rows, COLUMNS, 1000).expect("创建终端");
    terminal.pty_write(&payload);
    terminal.flush();
    let snapshot = terminal.take_snapshot();

    let mut mismatches = Vec::new();
    let mut col = 0usize;
    for (index, (codepoint, expected)) in samples.iter().enumerate() {
        if index % PER_ROW == 0 {
            col = 0;
        }
        let row_start = (index / PER_ROW) * COLUMNS as usize;
        let head = &snapshot.cells[row_start + col];
        if head.codepoint != *codepoint || head.width != *expected {
            mismatches.push(format!(
                "U+{codepoint:04X}（{}）期望宽度 {expected}，实测 {}（该格主码位 U+{:04X}）",
                character_of(*codepoint).escape_debug(),
                head.width,
                head.codepoint
            ));
        }
        // 宽字符的续列必须是空格占位（终端把宽格的后半截留空，不重复画字形）。
        if *expected == 2 {
            let tail = &snapshot.cells[row_start + col + 1];
            if tail.codepoint != 0 {
                mismatches.push(format!(
                    "U+{codepoint:04X} 宽字符的续列非空（U+{:04X}）",
                    tail.codepoint
                ));
            }
        }
        // 码位之后必须紧跟分隔空格：它同时钉住「恰好推进应推进的列数」——少一列
        // 会让下一格压在分隔空格上，多一列则两者之间留出空洞。
        let separator = &snapshot.cells[row_start + col + *expected as usize];
        if separator.codepoint != u32::from(' ') {
            mismatches.push(format!(
                "U+{codepoint:04X} 占 {expected} 列之后第 {} 列不是分隔空格（U+{:04X}）",
                col + *expected as usize,
                separator.codepoint
            ));
        }
        col += *expected as usize + 1;
    }
    mismatches
}

#[test]
fn printable_codepoints_occupy_the_columns_east_asian_width_assigns_them() {
    let (width, category) = ucd();
    let mut samples: Vec<(u32, u8)> = Vec::new();
    let mut zero_width = 0usize;
    for codepoint in 0..=0x10FFFF {
        if char::from_u32(codepoint).is_none() {
            continue;
        }
        let (Some(east_asian_width), Some(general_category)) =
            (width.get(codepoint), category.get(codepoint))
        else {
            continue;
        };
        if !general_category.starts_with(['L', 'N', 'P', 'S']) {
            continue;
        }
        match terminal_width_override(codepoint) {
            Some(0) => zero_width += 1,
            Some(terminal) => samples.push((codepoint, terminal)),
            None => samples.push((codepoint, east_asian_width_columns(east_asian_width))),
        }
    }
    assert!(
        samples.len() > 150_000,
        "可打印码位样本只有 {} 个，UCD 解析多半没生效",
        samples.len()
    );
    assert!(
        zero_width > 200,
        "零宽惯例码位只数到 {zero_width} 个，例外表与 UCD 已脱节"
    );

    let mut mismatches = Vec::new();
    for batch in samples.chunks(BATCH) {
        mismatches.extend(observe_layout(batch));
    }
    assert!(
        mismatches.is_empty(),
        "{} 个码位的显示宽度与 UAX #11 不符（比对 {} 个可打印码位）：\n{}",
        mismatches.len(),
        samples.len(),
        mismatches.join("\n")
    );
}

/// 把「基础字母 + 零宽码位 + 空格」逐行写进网格，断言零宽码位并入基础字母那一格。
///
/// 行内只需两列一个样本：基础字母占一列，零宽码位并入同一格，空格分隔相邻样本。
fn observe_joined(samples: &[u32]) -> Vec<String> {
    let mut payload = Vec::new();
    for chunk in samples.chunks(PER_ROW) {
        for codepoint in chunk {
            payload.push(b'a');
            payload.extend_from_slice(&encode_codepoints([*codepoint]));
            payload.push(b' ');
        }
        payload.push(b'\n');
    }
    let mut terminal =
        GhosttyTerminal::new(samples.len().div_ceil(PER_ROW) as u32 + 1, COLUMNS, 1000)
            .expect("创建终端");
    terminal.pty_write(&payload);
    terminal.flush();
    let snapshot = terminal.take_snapshot();

    samples
        .iter()
        .enumerate()
        .filter_map(|(index, codepoint)| {
            let row_start = (index / PER_ROW) * COLUMNS as usize;
            let column = (index % PER_ROW) * 2;
            let cell = &snapshot.cells[row_start + column];
            let joined = cell.codepoint == u32::from('a')
                && cell.graphemes.contains(codepoint)
                && snapshot.cells[row_start + column + 1].codepoint == u32::from(' ');
            (!joined).then(|| {
                format!(
                    "U+{codepoint:04X}（{}）未并入前一格：主码位 U+{:04X}，续接 {:?}",
                    character_of(*codepoint).escape_debug(),
                    cell.codepoint,
                    cell.graphemes
                )
            })
        })
        .collect()
}

#[test]
fn combining_marks_join_the_cell_of_the_base_before_them() {
    let (_, category) = ucd();
    let marks: Vec<u32> = (0..=0x10FFFF)
        .filter(|codepoint| category.get(*codepoint) == Some("Mn"))
        .collect();
    assert!(
        marks.len() > 1_500,
        "组合符只有 {} 个，UCD 解析多半没生效",
        marks.len()
    );

    let stride = marks.len().div_ceil(COMBINING_SAMPLE_LIMIT);
    let samples: Vec<u32> = marks.iter().step_by(stride).copied().collect();

    let mismatches = observe_joined(&samples);
    assert!(
        mismatches.is_empty(),
        "{} 个组合符没有并入基础码位的单元（比对 {} 个，Mn 共 {} 个）：\n{}",
        mismatches.len(),
        samples.len(),
        marks.len(),
        mismatches.join("\n")
    );
}

#[test]
fn zero_width_convention_codepoints_join_the_cell_of_the_base_before_them() {
    let samples: Vec<u32> = TERMINAL_WIDTH_OVERRIDES
        .iter()
        .filter(|(_, _, width)| *width == 0)
        .flat_map(|(left, right, _)| *left..=*right)
        .filter(|codepoint| char::from_u32(*codepoint).is_some())
        .collect();
    assert!(
        samples.len() > 200,
        "零宽惯例码位只有 {} 个，例外表与 UCD 已脱节",
        samples.len()
    );

    let mismatches = observe_joined(&samples);
    assert!(
        mismatches.is_empty(),
        "{} 个零宽惯例码位没有并入基础码位的单元（比对 {} 个）：\n{}",
        mismatches.len(),
        samples.len(),
        mismatches.join("\n")
    );
}
