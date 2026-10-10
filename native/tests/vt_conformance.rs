//! VT 一致性：跑 vttest 派生语料，逐格断言屏幕文本。
//!
//! 语料来自 xterm.js 的 `test/fixtures/escape_sequence_files/`，血统一路回到 DEC STD 070
//! 与 ECMA-48：期望屏幕是**从 xterm 逐帧手抄**的，不是任何一方的实现输出，故这是外部
//! 验证而非自验证。许可与血统见同目录 `PROVENANCE.md`。
//!
//! 采纳范围不是全部 76 例。按 `openspec/specs/upstream-alignment` 的分流口径：
//! 输入格式合规、语义唯一的用例在此断言；输入畸形、或该序列在快照成文时尚未实现的，
//! 快照记的是 xterm 的历史行为而非标准，采纳即等于让本仓与之对齐——那批连同尚待按
//! 标准裁定的用例，逐例列在 `NOT_ADOPTED` 并写明理由。
//!
//! `every_corpus_case_is_classified` 断言语料目录里恰好只有 `ADOPTED` 与 `NOT_ADOPTED`
//! 覆盖的用例：上游新增语料时该用例直接失败，逼使逐例给出结论，而不是静默放过。

use native::terminal::ghostty_terminal::{GhosttyTerminal, GridSnapshot};
use std::path::{Path, PathBuf};

/// 语料约定的终端尺寸；`PROVENANCE.md` 明确所有用例按 80×25 设计。
const CORPUS_ROWS: u32 = 25;
const CORPUS_COLS: u32 = 80;

/// 回滚长度：与 BDD 侧 `GhosttyTerminal::new` 的第三个参数同值。
const SCROLLBACK_LINES: u32 = 100;

/// 采纳用例数，实测记录在案；改动采纳范围时 MUST 同步更新此处与 openspec 规范。
const ADOPTED_CASE_COUNT: usize = 58;

/// 屏幕渲染：未写过的格子 codepoint 为 0，必须补空格而不是跳过——跳过会让后续所有
/// 列左移，把「光标没到位」误报成「内容不同」。
fn row_text(snapshot: &GridSnapshot, row: u32) -> String {
    let mut text = String::with_capacity(snapshot.cols as usize);
    for col in 0..snapshot.cols {
        let ch = snapshot
            .cells
            .get((row * snapshot.cols + col) as usize)
            .and_then(|cell| char::from_u32(cell.codepoint))
            .filter(|ch| *ch != '\0')
            .unwrap_or(' ');
        text.push(ch);
    }
    text.trim_end().to_string()
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xterm-escape-sequence")
}

/// 期望文件是 80×25 屏幕的逐帧抄本：尾部空行在文件里被省略，故缺失行视作空串，
/// 而不是把期望整体 trim 掉尾部换行再整块比——那会把行数差异掩盖成内容差异。
fn expected_rows(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect()
}

/// 逐行比对并把首处差异连同期望与实得一起报出：只报「不等」没有可操作性。
#[track_caller]
fn assert_screen_matches(stem: &str) {
    let expected = std::fs::read_to_string(corpus_dir().join(format!("{stem}.text")))
        .unwrap_or_else(|e| panic!("读取语料 {stem}.text 失败：{e}"));
    let input = std::fs::read(corpus_dir().join(format!("{stem}.in")))
        .unwrap_or_else(|e| panic!("读取语料 {stem}.in 失败：{e}"));
    let mut terminal =
        GhosttyTerminal::new(CORPUS_ROWS, CORPUS_COLS, SCROLLBACK_LINES).expect("创建终端失败");
    terminal.pty_write(&input);
    terminal.flush();
    let snapshot = terminal.take_snapshot();
    let expected = expected_rows(&expected);
    let mismatch: Vec<String> = (0..CORPUS_ROWS)
        .filter_map(|row| {
            let want = expected.get(row as usize).map(String::as_str).unwrap_or("");
            let got = row_text(&snapshot, row);
            (want != got).then(|| format!("  第 {row} 行 期望 |{want}| 实得 |{got}|"))
        })
        .collect();
    assert!(
        mismatch.is_empty(),
        "{stem} 屏幕不符：\n{}",
        mismatch.join("\n")
    );
}

/// 采纳：输入格式合规且语义唯一，期望即外部真相。
const ADOPTED: &[&str] = &[
    "t0001-all_printable",
    "t0002-history",
    "t0002j-simple_string",
    "t0003-line_wrap",
    "t0003j-LF",
    "t0004-LF",
    "t0004j-CR",
    "t0005-CR",
    "t0006-IND",
    "t0007-space_at_end",
    "t0008-BS",
    "t0009-NEL",
    "t0010-RI",
    "t0011-RI_scroll",
    "t0012-VT",
    "t0013-FF",
    "t0014-CAN",
    "t0015-SUB",
    "t0016-SU",
    "t0017-SD",
    "t0023-CUU_scroll",
    "t0025-CUP",
    "t0026-CNL",
    "t0027-CPL",
    "t0035-HVP",
    "t0050-ICH",
    "t0051-IL",
    "t0052-DL",
    "t0053-DCH",
    "t0054-ECH",
    "t0055-EL",
    "t0056-ED",
    "t0057-ED3",
    "t0070-DECSTBM_LF",
    "t0071-DECSTBM_IND",
    "t0072-DECSTBM_NEL",
    "t0073-DECSTBM_RI",
    "t0074-DECSTBM_SU_SD",
    "t0075-DECSTBM_CUU_CUD",
    "t0076-DECSTBM_IL_DL",
    "t0078-DECSTBM_CPL_CNL",
    "t0079-DECSTBM_VPR",
    "t0080-HT",
    "t0082-HTS",
    "t0083-CHT",
    "t0084-CBT",
    "t0090-alt_screen",
    "t0091-alt_screen_ED3",
    "t0092-alt_screen_DECSC",
    "t0100-IRM",
    "t0101-NLM",
    "t0102-DECAWM",
    "t0300-vttest1",
    "t0500-bash_long_line",
    "t0501-bash_ls",
    "t0502-bash_ls_color",
    "t0503-zsh_ls_color",
    "t0504-vim",
];

/// 不采纳：逐例理由如下。语料仍在仓库里（血统与许可需要保留），但这些用例的期望值
/// 不是标准，采纳会让本仓与 xterm 的历史行为对齐——与「测试必须是正确性测试」的要求
/// 方向相反。`openspec/specs/upstream-alignment` 要求把本表收敛到空。
const NOT_ADOPTED: &[(&str, &str)] = &[
    // ── 输入畸形：单参数序列给了多个参数，标准未规定 ──
    (
        "t0020-CUF",
        "输入含 CSI 10;3 C：CUF 按标准只取一个参数，多参处标准未规定，快照记的是 xterm 的解读",
    ),
    (
        "t0021-CUB",
        "输入含 CSI 2;3 D：CUB 只取一个参数，xterm 把第二个参数当行移动，本仓按标准取首参数",
    ),
    (
        "t0022-CUU",
        "输入含 CSI 3;5 A：CUU 只取一个参数，多参处标准未规定",
    ),
    (
        "t0024-CUD",
        "输入含 CSI 3;5 B：CUD 只取一个参数，多参处标准未规定",
    ),
    (
        "t0030-HPR",
        "输入含 CSI 10;3 a：HPR 只取一个参数，多参处标准未规定",
    ),
    (
        "t0032-VPB",
        "输入含 CSI 3;5 k：VPB 只取一个参数，多参处标准未规定",
    ),
    (
        "t0034-VPR",
        "输入含 CSI 3;5 e：VPR 只取一个参数，多参处标准未规定",
    ),
    (
        "t0077-DECSTBM_quirks",
        "输入含 CSI 6;7;8 r 与 CSI 15;0 r：三参数 DECSTBM 与下边距取 0 都是 xterm 的历史怪癖，非标准",
    ),
    // ── 快照成文时该序列尚未实现，快照缺的是实现而不是期望 ──
    (
        "t0040-REP",
        "CSI Ps b 是 ECMA-48 的 REP：xterm 快照成文时未实现，本仓实现了。此处快照比标准错，按标准不看快照",
    ),
    (
        "t0103-reverse_wrap",
        "CSI ? 45 h（reverse wraparound）xterm 快照成文时未实现，属 DEC 扩展，无可比对象",
    ),
    // ── 输入格式合规，但语义尚未按标准裁定 ──
    (
        "t0033-VPB_scroll",
        "VPB 删除计数超过滚动区域高度时的边界行为，尚未按 DEC STD 070 裁定；快照记的是 xterm 的处理",
    ),
    (
        "t0060-DECSC",
        "DECSC 存光标后经 DECSU 滚动再 DECRC 恢复，落点语义尚未按标准裁定",
    ),
    (
        "t0061-CSI_s",
        "CSI s 与 DECSC 同义，恢复落点语义尚未按标准裁定",
    ),
    (
        "t0081-TBC",
        "TBC 清制表位与后续 VPA/VPD 混排时的纵向下标行为尚未按标准裁定",
    ),
    (
        "t600-DECSTBM_SR",
        "在滚动区域内写满行宽触发滚动时的区域边界行为，尚未按标准裁定",
    ),
    ("t601-DECSTBM_SL", "同上，行内滚动方向一侧"),
    (
        "t602-DECSTBM_DECIC",
        "同上，左右边距插入列在滚动区域内的交互",
    ),
    (
        "t603-DECSTBM_DECDC",
        "同上，左右边距删除列在滚动区域内的交互",
    ),
];

/// 语料目录里的每个用例都必须有结论：采纳或不采纳。二者覆盖不全即失败，
/// 使上游新增语料无法被静默放过。
#[test]
fn every_corpus_case_is_classified() {
    let mut on_disk: Vec<String> = std::fs::read_dir(corpus_dir())
        .expect("读取语料目录失败")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".in").map(str::to_string)
        })
        .collect();
    on_disk.sort();

    let mut classified: Vec<&str> = ADOPTED
        .iter()
        .copied()
        .chain(NOT_ADOPTED.iter().map(|(name, reason)| {
            assert!(!reason.is_empty(), "{} 未写不采纳理由", name);
            *name
        }))
        .collect();
    classified.sort_unstable();

    let on_disk_refs: Vec<&str> = on_disk.iter().map(String::as_str).collect();
    assert_eq!(
        classified, on_disk_refs,
        "语料目录与判定表不一致：新增用例必须逐例给出采纳结论或理由",
    );
    assert_eq!(
        ADOPTED.len(),
        ADOPTED_CASE_COUNT,
        "采纳用例数与 openspec 记录不符"
    );
}

#[test]
fn corpus_t0001_all_printable() {
    assert_screen_matches("t0001-all_printable");
}

#[test]
fn corpus_t0002_history() {
    assert_screen_matches("t0002-history");
}

#[test]
fn corpus_t0002j_simple_string() {
    assert_screen_matches("t0002j-simple_string");
}

#[test]
fn corpus_t0003_line_wrap() {
    assert_screen_matches("t0003-line_wrap");
}

#[test]
fn corpus_t0003j_l_f() {
    assert_screen_matches("t0003j-LF");
}

#[test]
fn corpus_t0004_l_f() {
    assert_screen_matches("t0004-LF");
}

#[test]
fn corpus_t0004j_c_r() {
    assert_screen_matches("t0004j-CR");
}

#[test]
fn corpus_t0005_c_r() {
    assert_screen_matches("t0005-CR");
}

#[test]
fn corpus_t0006_i_n_d() {
    assert_screen_matches("t0006-IND");
}

#[test]
fn corpus_t0007_space_at_end() {
    assert_screen_matches("t0007-space_at_end");
}

#[test]
fn corpus_t0008_b_s() {
    assert_screen_matches("t0008-BS");
}

#[test]
fn corpus_t0009_n_e_l() {
    assert_screen_matches("t0009-NEL");
}

#[test]
fn corpus_t0010_r_i() {
    assert_screen_matches("t0010-RI");
}

#[test]
fn corpus_t0011_r_i_scroll() {
    assert_screen_matches("t0011-RI_scroll");
}

#[test]
fn corpus_t0012_v_t() {
    assert_screen_matches("t0012-VT");
}

#[test]
fn corpus_t0013_f_f() {
    assert_screen_matches("t0013-FF");
}

#[test]
fn corpus_t0014_c_a_n() {
    assert_screen_matches("t0014-CAN");
}

#[test]
fn corpus_t0015_s_u_b() {
    assert_screen_matches("t0015-SUB");
}

#[test]
fn corpus_t0016_s_u() {
    assert_screen_matches("t0016-SU");
}

#[test]
fn corpus_t0017_s_d() {
    assert_screen_matches("t0017-SD");
}

#[test]
fn corpus_t0023_c_u_u_scroll() {
    assert_screen_matches("t0023-CUU_scroll");
}

#[test]
fn corpus_t0025_c_u_p() {
    assert_screen_matches("t0025-CUP");
}

#[test]
fn corpus_t0026_c_n_l() {
    assert_screen_matches("t0026-CNL");
}

#[test]
fn corpus_t0027_c_p_l() {
    assert_screen_matches("t0027-CPL");
}

#[test]
fn corpus_t0035_h_v_p() {
    assert_screen_matches("t0035-HVP");
}

#[test]
fn corpus_t0050_i_c_h() {
    assert_screen_matches("t0050-ICH");
}

#[test]
fn corpus_t0051_i_l() {
    assert_screen_matches("t0051-IL");
}

#[test]
fn corpus_t0052_d_l() {
    assert_screen_matches("t0052-DL");
}

#[test]
fn corpus_t0053_d_c_h() {
    assert_screen_matches("t0053-DCH");
}

#[test]
fn corpus_t0054_e_c_h() {
    assert_screen_matches("t0054-ECH");
}

#[test]
fn corpus_t0055_e_l() {
    assert_screen_matches("t0055-EL");
}

#[test]
fn corpus_t0056_e_d() {
    assert_screen_matches("t0056-ED");
}

#[test]
fn corpus_t0057_e_d3() {
    assert_screen_matches("t0057-ED3");
}

#[test]
fn corpus_t0070_d_e_c_s_t_b_m_l_f() {
    assert_screen_matches("t0070-DECSTBM_LF");
}

#[test]
fn corpus_t0071_d_e_c_s_t_b_m_i_n_d() {
    assert_screen_matches("t0071-DECSTBM_IND");
}

#[test]
fn corpus_t0072_d_e_c_s_t_b_m_n_e_l() {
    assert_screen_matches("t0072-DECSTBM_NEL");
}

#[test]
fn corpus_t0073_d_e_c_s_t_b_m_r_i() {
    assert_screen_matches("t0073-DECSTBM_RI");
}

#[test]
fn corpus_t0074_d_e_c_s_t_b_m_s_u_s_d() {
    assert_screen_matches("t0074-DECSTBM_SU_SD");
}

#[test]
fn corpus_t0075_d_e_c_s_t_b_m_c_u_u_c_u_d() {
    assert_screen_matches("t0075-DECSTBM_CUU_CUD");
}

#[test]
fn corpus_t0076_d_e_c_s_t_b_m_i_l_d_l() {
    assert_screen_matches("t0076-DECSTBM_IL_DL");
}

#[test]
fn corpus_t0078_d_e_c_s_t_b_m_c_p_l_c_n_l() {
    assert_screen_matches("t0078-DECSTBM_CPL_CNL");
}

#[test]
fn corpus_t0079_d_e_c_s_t_b_m_v_p_r() {
    assert_screen_matches("t0079-DECSTBM_VPR");
}

#[test]
fn corpus_t0080_h_t() {
    assert_screen_matches("t0080-HT");
}

#[test]
fn corpus_t0082_h_t_s() {
    assert_screen_matches("t0082-HTS");
}

#[test]
fn corpus_t0083_c_h_t() {
    assert_screen_matches("t0083-CHT");
}

#[test]
fn corpus_t0084_c_b_t() {
    assert_screen_matches("t0084-CBT");
}

#[test]
fn corpus_t0090_alt_screen() {
    assert_screen_matches("t0090-alt_screen");
}

#[test]
fn corpus_t0091_alt_screen_e_d3() {
    assert_screen_matches("t0091-alt_screen_ED3");
}

#[test]
fn corpus_t0092_alt_screen_d_e_c_s_c() {
    assert_screen_matches("t0092-alt_screen_DECSC");
}

#[test]
fn corpus_t0100_i_r_m() {
    assert_screen_matches("t0100-IRM");
}

#[test]
fn corpus_t0101_n_l_m() {
    assert_screen_matches("t0101-NLM");
}

#[test]
fn corpus_t0102_d_e_c_a_w_m() {
    assert_screen_matches("t0102-DECAWM");
}

#[test]
fn corpus_t0300_vttest1() {
    assert_screen_matches("t0300-vttest1");
}

#[test]
fn corpus_t0500_bash_long_line() {
    assert_screen_matches("t0500-bash_long_line");
}

#[test]
fn corpus_t0501_bash_ls() {
    assert_screen_matches("t0501-bash_ls");
}

#[test]
fn corpus_t0502_bash_ls_color() {
    assert_screen_matches("t0502-bash_ls_color");
}

#[test]
fn corpus_t0503_zsh_ls_color() {
    assert_screen_matches("t0503-zsh_ls_color");
}

#[test]
fn corpus_t0504_vim() {
    assert_screen_matches("t0504-vim");
}
