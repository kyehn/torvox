//! Ghostty 模糊测试语料回放。
//!
//! 上游 `test/fuzz-libghostty/corpus/stream-initial/` 的每条种子都是一段控制流样例的
//! 畸形变体（首字节恒为 NUL 或控制符，正是 fuzz 器的初始网格状态）。逐条灌入本仓接线，
//! 按种子名声明的意图断言**可观测回读**：网格文本、光标、宽字符续列、标题、超链接、
//! 剪贴板事件、回写应答与模式位。
//!
//! 语料按上游发布标签取（标签内容冻结，可复现），运行期现取不入库。
//!
//! 这里不复刻上游 fuzz 的判据：断言的是本仓接线在这些字节上的结果，不是解析器与上游
//! 是否一致——那属上游自己的正确性，本仓只钉自己的接线（见 `docs/research/30`）。

use super::support;

use libghostty_vt::terminal::Mode;
use native::terminal::ghostty_terminal::{CellSnapshot, GhosttyTerminal, GridSnapshot};
use native::terminal::theme;
use support::fetch_bytes;

/// 取语料的地址：钉在上游发布标签上。标签内容不可变，故本用例可复现；上游 fuzz 语料
/// 只增不改，标签升级时按下面的清单逐条复核即可。
const UPSTREAM_TAG: &str = "v1.3.1";
const CORPUS_URL: &str = "https://raw.githubusercontent.com/ghostty-org/ghostty";

/// 颜色比较容差：与渲染侧同口径（±5/255），够吸收浮点换算误差又不掩盖真实色差。
const COLOR_TOLERANCE: f32 = 5.0 / 255.0;

/// 网格尺寸：行数够放下语料里的滚屏与插入删除，列数够放下制表位与多字节行。
const ROWS: u32 = 24;
const COLUMNS: u32 = 80;

/// 回滚上限。语料里没有产生滚屏的种子，但回滚为 0 会让写满一行的种子行为失真。
const SCROLLBACK: u32 = 1000;

/// 一条语料灌入后的可观测状态。
struct Replay {
    terminal: GhosttyTerminal,
    snapshot: GridSnapshot,
}

impl Replay {
    fn cell(&self, row: u32, col: u32) -> &CellSnapshot {
        &self.snapshot.cells[(row * self.snapshot.cols + col) as usize]
    }

    /// 行文本：主码位连成串，去掉行尾空格（空单元码位为 0）。
    fn row_text(&self, row: u32) -> String {
        (0..self.snapshot.cols)
            .map(|col| self.cell(row, col).codepoint)
            .filter(|codepoint| *codepoint != 0)
            .filter_map(char::from_u32)
            .collect()
    }

    /// 非空单元的 `(列, 字符)`：断言字符落点时用它，省去逐格下标推算。
    fn occupied(&self, row: u32) -> Vec<(u32, char)> {
        (0..self.snapshot.cols)
            .filter_map(|col| {
                let codepoint = self.cell(row, col).codepoint;
                (codepoint != 0).then(|| (col, char::from_u32(codepoint).expect("合法码位")))
            })
            .collect()
    }

    /// 颜色相等：逐通道比较，容差同 [`COLOR_TOLERANCE`]。
    fn color_eq(left: &[f32; 4], right: [f32; 4]) -> bool {
        left.iter()
            .zip(right)
            .all(|(left, right)| (left - right).abs() <= COLOR_TOLERANCE)
    }

    /// 调色板色 → 渲染侧色值。渲染侧按 `字节 / 255` 换算，此处照同一口径，避免
    /// 断言里出现第二个色值定义。
    fn palette_color(color: libghostty_vt::style::RgbColor) -> [f32; 4] {
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            1.0,
        ]
    }

    /// xterm-256 色立方：索引 16..232 按 `16 + 36r + 6g + b`，分量按 0, 95, 135, 175,
    /// 215, 255 六级取值。写成公式而非写死字面量，语料换色号时断言仍指向规范本身。
    fn cube_color(index: u32) -> [f32; 4] {
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let offset = index - 16;
        [
            LEVELS[(offset / 36) as usize] as f32 / 255.0,
            LEVELS[((offset % 36) / 6) as usize] as f32 / 255.0,
            LEVELS[(offset % 6) as usize] as f32 / 255.0,
            1.0,
        ]
    }

    /// 断言整屏空白：无语料可见效果的种子用得上。
    fn assert_blank(&self, context: &str) {
        assert_eq!(
            self.row_text(0),
            "",
            "{context}：首行应有文本，实际 {:?}",
            self.row_text(0)
        );
        assert_eq!(
            (self.snapshot.cursor_row, self.snapshot.cursor_col),
            (0, 0),
            "{context}：光标应回到原点"
        );
        assert_eq!(
            self.snapshot.scrollback_length, 0,
            "{context}：不该产生回滚"
        );
    }
}

/// 取语料并按生产写入路径（`pty_write` → VT → 快照）回放。
fn replay(name: &str) -> Replay {
    let bytes = seed_bytes(name);
    let mut terminal = GhosttyTerminal::new(ROWS, COLUMNS, SCROLLBACK).expect("创建终端");
    terminal.pty_write(&bytes);
    terminal.flush();
    let snapshot = terminal.take_snapshot();
    assert_eq!(
        (snapshot.rows, snapshot.cols),
        (ROWS, COLUMNS),
        "{name}：快照网格尺寸与建终端时不一致"
    );
    assert_eq!(
        snapshot.cells.len(),
        (ROWS * COLUMNS) as usize,
        "{name}：快照单元数与网格不符"
    );
    Replay { terminal, snapshot }
}

/// 取一条语料的原始字节。
fn seed_bytes(name: &str) -> Vec<u8> {
    fetch_bytes(&format!(
        "{CORPUS_URL}/{UPSTREAM_TAG}/test/fuzz-libghostty/corpus/stream-initial/{name}"
    ))
}

/// 逐字节回放，记录每一步的 `(光标可见, 备用屏)` 状态。
///
/// 只看终态分不出「置位/复位配平」与「整串模式切换全被忽略」——两者终态相同。故对
/// 需要区分的语料取全程状态序列。
fn state_walk(name: &str) -> Vec<(bool, bool)> {
    let mut terminal = GhosttyTerminal::new(ROWS, COLUMNS, SCROLLBACK).expect("创建终端");
    seed_bytes(name)
        .into_iter()
        .map(|byte| {
            terminal.pty_write(&[byte]);
            terminal.flush();
            (
                terminal.mode_get(Mode::CURSOR_VISIBLE),
                terminal.alt_screen_active_atomic(),
            )
        })
        .collect()
}

/// 语料清单：种子名 → 按其意图的断言。
///
/// 名即意图（上游给每条种子起的就是行为名），故清单按名排序即可覆盖上游目录；上游
/// 换标签后若有增删，`fetch_bytes` 的 404 会直接判红，届时逐条复核即可。
const SEED_CHECKS: &[(&str, fn(&Replay))] = &[
    ("01-plain-text-slice", plain_text_slice),
    ("02-plain-text-scalar", plain_text_slice),
    ("03-csi-cursor-sgr", cursor_positioning_and_sgr),
    ("04-csi-erase", erase_then_text),
    ("05-osc-title-bel", title_from_osc),
    ("06-osc-title-st", title_from_osc),
    ("07-dcs-decrqss", decrqss_reply),
    ("08-apc", apc_is_ignored),
    ("09-mixed-text-csi", text_then_csi_then_line),
    ("10-sgr-256-rgb", indexed_and_true_color_sgr),
    ("11-utf8-multibyte", multibyte_widths),
    ("12-malformed-utf8", malformed_utf8_becomes_replacement),
    ("13-incomplete-csi", incomplete_csi_leaves_no_trace),
    ("14-decset-decrst", decset_and_decrst),
    ("15-scroll-region", scroll_region_then_reset),
    ("16-c1-controls", c1_bytes_are_text),
    ("17-tab-backspace", tabs_and_backspaces),
    ("18-insert-delete", insert_and_delete_on_blank_screen),
    ("19-many-params", many_sgr_params),
    ("20-csi-subparams", sgr_subparameters),
    ("21-osc-hyperlink", hyperlink_covers_text),
    ("22-osc-clipboard", clipboard_write_event),
    ("23-empty", only_control_byte),
    ("24-esc-misc", save_and_restore_cursor),
];

#[test]
fn upstream_fuzz_corpus_seeds_replay_into_observable_state() {
    for (name, check) in SEED_CHECKS {
        check(&replay(name));
    }
}

/// 语料条数是上游目录的规模，删一条断言就会在这里露出来。
#[test]
fn seed_list_matches_upstream_corpus_size() {
    assert_eq!(
        SEED_CHECKS.len(),
        24,
        "语料清单与上游 stream-initial 目录规模不符"
    );
    let mut names: Vec<&str> = SEED_CHECKS.iter().map(|(name, _)| *name).collect();
    let sorted = {
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted
    };
    assert_eq!(
        names, sorted,
        "语料清单未按名排序，与上游目录一一对应的前提就没了"
    );
    names.dedup();
    assert_eq!(names.len(), SEED_CHECKS.len(), "语料清单有重名");
}

// ── 各条语料的断言 ───────────────────────────────────────────────────────

/// 01/02：整行文本落在首行，行末换行推进到下一行行首。
fn plain_text_slice(replay: &Replay) {
    assert_eq!(replay.row_text(0), "Hello, World!");
    assert_eq!(
        (replay.snapshot.cursor_row, replay.snapshot.cursor_col),
        (1, 0)
    );
}

/// 03：`CSI 10;20H` 把「foo」放到第 9 行第 19 列，紧随其后的 `SGR 31` 让「red」
/// 带上调色板红。
fn cursor_positioning_and_sgr(replay: &Replay) {
    assert_eq!(
        replay.occupied(9),
        vec![
            (19, 'f'),
            (20, 'o'),
            (21, 'o'),
            (22, 'r'),
            (23, 'e'),
            (24, 'd')
        ]
    );
    assert_eq!(
        (replay.snapshot.cursor_row, replay.snapshot.cursor_col),
        (9, 25)
    );
    let red = Replay::palette_color(theme::default_dark().palette.ansi[1]);
    for col in 22..25 {
        assert!(
            Replay::color_eq(&replay.cell(9, col).foreground, red),
            "SGR 31 的前景色应是调色板红，第 {col} 列实测 {:?}",
            replay.cell(9, col).foreground
        );
    }
    let default_foreground = Replay::palette_color(theme::default_dark().palette.foreground);
    assert!(Replay::color_eq(
        &replay.cell(9, 19).foreground,
        default_foreground
    ));
}

/// 04：整屏擦除后光标归位，REP（`CSI b`）在没有前一个可见字符时不产生重复。
fn erase_then_text(replay: &Replay) {
    assert_eq!(replay.row_text(0), "line");
    assert_eq!(
        (replay.snapshot.cursor_row, replay.snapshot.cursor_col),
        (0, 4)
    );
}

/// 05/06：BEL 与 ST 终止的 OSC 0 都必须落到标题。
fn title_from_osc(replay: &Replay) {
    assert_eq!(replay.snapshot.title, "My Title");
    assert_eq!(replay.terminal.title(), "My Title", "标题查询与快照不一致");
    replay.assert_blank("设置标题");
}

/// 07：DECRQSS 的应答必须回到会话（PTY 回写面），而不是被解析器吞掉。
fn decrqss_reply(replay: &Replay) {
    let responses = replay.terminal.drain_pty_write_responses();
    assert_eq!(responses.len(), 1, "DECRQSS 应答条数不符");
    let reply = String::from_utf8_lossy(&responses[0]).into_owned();
    assert!(
        reply.starts_with("\u{1b}P") && reply.contains("$r") && reply.ends_with("\u{1b}\\"),
        "DECRQSS 应答应是含 `$r` 的 DCS 回报，实际 {reply:?}"
    );
    replay.assert_blank("DECRQSS");
}

/// 08：APC 串不产生可见效果。
fn apc_is_ignored(replay: &Replay) {
    replay.assert_blank("APC");
}

/// 09：文本、CSI 与换行混排：SGR 之后的文本接在已有文本之后，换行推进到下一行。
fn text_then_csi_then_line(replay: &Replay) {
    assert_eq!(replay.row_text(0), "ABCDhello");
    assert_eq!(replay.row_text(1), "world");
    assert_eq!(
        (replay.snapshot.cursor_row, replay.snapshot.cursor_col),
        (1, 5)
    );
}

/// 10：256 色号前景与真彩背景都要落到单元上。
fn indexed_and_true_color_sgr(replay: &Replay) {
    assert_eq!(replay.row_text(0), "redgreen bg");
    let red = Replay::cube_color(196);
    for col in 0..3 {
        assert!(
            Replay::color_eq(&replay.cell(0, col).foreground, red),
            "256 色号 196 的前景实测 {:?}",
            replay.cell(0, col).foreground
        );
    }
    let green = [0.0, 1.0, 0.0, 1.0];
    for col in 3..11 {
        assert!(
            Replay::color_eq(&replay.cell(0, col).background, green),
            "真彩背景实测 {:?}",
            replay.cell(0, col).background
        );
    }
}

/// 11：多字节字符各按自身宽度占列，续列由 `wide_char_tail_cols` 给出。
fn multibyte_widths(replay: &Replay) {
    assert_eq!(replay.occupied(0), vec![(0, 'é'), (1, '中'), (3, '😀')]);
    assert_eq!(
        (replay.snapshot.cursor_row, replay.snapshot.cursor_col),
        (0, 5)
    );
    assert_eq!(
        replay.terminal.wide_char_tail_cols(0),
        vec![2, 4],
        "宽字符续列应是 2 与 4"
    );
}

/// 12：非法 UTF-8 逐字节落替换字符，各占一列。
fn malformed_utf8_becomes_replacement(replay: &Replay) {
    let cells: Vec<u32> = (0..8).map(|col| replay.cell(0, col).codepoint).collect();
    assert!(
        cells.iter().all(|codepoint| *codepoint == 0xFFFD),
        "非法 UTF-8 应逐字节落替换字符，实际 {cells:X?}"
    );
    assert_eq!(
        (replay.snapshot.cursor_row, replay.snapshot.cursor_col),
        (0, 8)
    );
}

/// 13：残缺 CSI 被丢弃，不落任何单元。
fn incomplete_csi_leaves_no_trace(replay: &Replay) {
    replay.assert_blank("残缺 CSI");
}

/// 14：置位与复位成对出现（`?25l`…`?25h`、`?1049h`…`?1049l`），终态应回到初始态；
/// 但只断言终态分不出「配平」与「整串被忽略」，故还要断言中途确实进过备用屏。
fn decset_and_decrst(replay: &Replay) {
    let states = state_walk("14-decset-decrst");
    assert!(
        states.contains(&(false, true)),
        "DECSET 1049 之后应经过「光标不可见且处于备用屏」，实际状态序列 {states:?}"
    );
    assert_eq!(
        *states.last().expect("状态序列非空"),
        (true, false),
        "DECRST 之后应回到初始态"
    );
    replay.assert_blank("DECSET/DECRST");
}

/// 15：滚屏区设置与复位都不改动空白屏；复位后光标仍在原点。
fn scroll_region_then_reset(replay: &Replay) {
    replay.assert_blank("滚屏区");
}

/// 16：C1 控制字节（`0x9B`/`0x9D`/`0x9C`）在 UTF-8 模式下不是控制符，按替换字符落格，
/// 因此 OSC 也不生效——标题保持为空。
fn c1_bytes_are_text(replay: &Replay) {
    assert_eq!(replay.snapshot.title, "", "C1 形式的 OSC 不应设置标题");
    let text = replay.row_text(0);
    assert!(
        text.starts_with('\u{FFFD}') && text.ends_with('\u{FFFD}') && text.contains("My Title"),
        "C1 字节应按替换字符落格，实际 {text:?}"
    );
}

/// 17：制表位按 8 列步进，退格只移动光标不改内容。
fn tabs_and_backspaces(replay: &Replay) {
    assert_eq!(replay.occupied(0), vec![(8, 'A'), (16, 'B'), (24, 'C')]);
    assert_eq!(
        (replay.snapshot.cursor_row, replay.snapshot.cursor_col),
        (0, 23)
    );
}

/// 18：插入行、删除字符、擦除字符、插入字符在空白屏上都不产生可见内容。
fn insert_and_delete_on_blank_screen(replay: &Replay) {
    replay.assert_blank("插入删除");
}

/// 19：超长参数列表被逐个解析，未知参数忽略，屏上不留痕迹。
fn many_sgr_params(replay: &Replay) {
    replay.assert_blank("多参数 SGR");
}

/// 20：带子参数的 SGR（`38:2::255:0:0`）被上游接受，屏上不留痕迹。
fn sgr_subparameters(replay: &Replay) {
    replay.assert_blank("SGR 子参数");
}

/// 21：OSC 8 的链接覆盖到链接文本的每个单元。
fn hyperlink_covers_text(replay: &Replay) {
    assert_eq!(replay.row_text(0), "click");
    for col in 0..5 {
        assert_eq!(
            replay.terminal.hyperlink_at(0, col).as_deref(),
            Some("https://example.com"),
            "第 {col} 列应在链接内"
        );
    }
    assert_eq!(
        replay.terminal.hyperlink_at(0, 5),
        None,
        "链接文本之后的单元不应带链接"
    );
}

/// 22：OSC 52 的 base64 载荷解码后经剪贴板回调回到会话。
fn clipboard_write_event(replay: &Replay) {
    assert_eq!(
        replay.terminal.poll_clipboard_event(),
        Some(("c".to_string(), "Hello".to_string())),
        "OSC 52 应解出剪贴板事件"
    );
    replay.assert_blank("OSC 52");
}

/// 23：单个 NUL 被写入路径剔除，屏上不留痕迹。
fn only_control_byte(replay: &Replay) {
    replay.assert_blank("单个 NUL");
}

/// 24：DECSC/DECRC 把光标送回保存点，尾部的残缺转义不落字。
fn save_and_restore_cursor(replay: &Replay) {
    replay.assert_blank("保存恢复光标");
}
