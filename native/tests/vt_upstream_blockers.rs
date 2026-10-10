//! 上游引擎缺口的**特征化**断言：把 libghostty-vt 当前的行为钉住，并附上游出处。
//!
//! 这些断言**不是**正确性测试——它们记下的是「标准要求如此、上游尚未做到」的偏差。
//! 它们存在的唯一目的是当上游补齐后立刻失败：失败即信号，回到
//! `vt_conformance.rs` 的 `NOT_ADOPTED` 表逐例重新裁定，符合条件的转为采纳用例。
//! 若上游收紧为别的不合规行为，同样会在这里失败，而不是悄悄改变终端语义。
//!
//! 每条断言都写清三件事：标准怎么说、上游现在怎么做、出自上游哪个位置。
//! 出处一律为 `src/terminal/stream.zig`（本仓 `Cargo.toml` 钉的 libghostty-vt rev
//! 所对应的 ghostty 源码，见 `openspec/specs/upstream-alignment`）。
//!
//! 相关语料用例见 `native/tests/vt_conformance.rs` 的 `NOT_ADOPTED`。

use native::terminal::ghostty_terminal::GhosttyTerminal;

fn screen_text(input: &[u8], rows: u32, cols: u32) -> Vec<String> {
    let mut terminal = GhosttyTerminal::new(rows, cols, 100).expect("terminal");
    terminal.pty_write(input);
    terminal.flush();
    let snapshot = terminal.take_snapshot();
    (0..snapshot.rows)
        .map(|row| {
            let mut text = String::new();
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
        })
        .collect()
}

fn cursor_after(input: &[u8], rows: u32, cols: u32) -> (u32, u32) {
    let mut terminal = GhosttyTerminal::new(rows, cols, 100).expect("terminal");
    terminal.pty_write(input);
    terminal.flush();
    let snapshot = terminal.take_snapshot();
    (snapshot.cursor_row, snapshot.cursor_col)
}

/// `CSI g`（无参数）丢弃整条 TBC。
///
/// 标准：ECMA-48 TBC 的 Ps 缺省为 0 = 清除活动位置的制表位。
/// 上游：`stream.zig` 的 `'g'` 分发只接受 `params.len == 1`，无参数走
/// `log.warn("invalid tab clear command")` 后 return。
#[test]
fn tbc_without_parameter_is_dropped_by_upstream_parser() {
    // 基线：显式 CSI 0g 清除当前列的制表位后，制表跳过该列（8 → 16）。
    let explicit = screen_text(b"\x1b[1;9H\x1b[0g\x1b[1;1H\tA\n", 4, 40);
    assert_eq!(
        explicit[0].find('A'),
        Some(16),
        "显式 CSI 0g 必须清掉列 8 的制表位"
    );

    // 无参数形式：制表位未被清除，仍停在列 8。
    let implicit = screen_text(b"\x1b[1;9H\x1b[g\x1b[1;1H\tA\n", 4, 40);
    assert_eq!(
        implicit[0].find('A'),
        Some(8),
        "无参数 CSI g 当前被上游整条丢弃（ECMA-48 要求按 Ps=0 清除当前列）"
    );
}

/// `CSI 3g`（全清）本周起仍然生效——制表位走默认区间时的另一端也一并受控。
#[test]
fn tbc_clear_all_takes_effect() {
    let rows = screen_text(b"AB\x1b[3g\tY\n", 4, 40);
    // 全清后无制表位，制表落到最后一列（上游 Terminal.tabClear(.all) → tabstops.reset(0)）。
    assert_eq!(rows[0].find('Y'), Some(39));
}

/// 多参数序列被**整条丢弃**，而不是取首参数。
///
/// 标准：ECMA-48 §5.4.3 只说明参数个数由控制函数规定，未规定多参的处置。
/// 上游：`stream.zig` 的 `CSI Pn C` 分发写死 `params.len == 1`，多一个参数即
/// `log.warn("invalid cursor right command")` 后 return，光标完全不动。
/// xterm 取首参数移动 10 列，语料期望因此不同。
#[test]
fn cursor_forward_with_extra_parameter_is_dropped_whole_sequence() {
    // 单参数：右移 10 列。
    let single = cursor_after(b"\x1b[1;5H\x1b[10C", 4, 40);
    assert_eq!(single, (0, 14), "CSI 10 C 必须把光标移到列 14");
    // 多参数：整条丢弃，光标原地不动。
    let doubled = cursor_after(b"\x1b[1;5H\x1b[10;3C", 4, 40);
    assert_eq!(
        doubled,
        (0, 4),
        "CSI 10;3 C 当前被上游整条丢弃（xterm 取首参数移动 10 列）"
    );
}

/// `CSI Ps k`（VPB）与 `CSI Ps A`（CUU）同路：触顶即停，不滚动区域内容。
///
/// 标准：DEC STD 070 的 VPB 要求光标越过区域上边距时**下滚**区域内容。
/// 上游：`stream.zig` 的 `'A','k'` 合并分发到 `cursor_up`，滚动语义缺失。
#[test]
fn vpb_alias_to_cursor_up_does_not_scroll_at_region_top() {
    // 区域第 2..3 行，光标置于区域末行并请求越过上边距的 VPB。
    let rows = screen_text(
        b"L1\r\nL2\r\nL3\r\nL4\r\n\x1b[2;3r\x1b[3;1H\x1b[99k",
        10,
        40,
    );
    assert_eq!(rows[0], "L1", "VPB 越过区域上边距时上游不滚动区域内容");
    assert_eq!(rows[1], "L2");
    assert_eq!(rows[2], "L3");
    assert_eq!(rows[3], "L4");
}

/// `CSI Pn SP A`（SR）与 `CSI Pn SP @`（SL）均未实现。
///
/// 标准：DEC STD 070 的 SR/SL 在左右边距内逐行移动内容。
/// 上游：`stream.zig` 对带中间字符的 CSI A 与 CSI @ 直接
/// `log.warn("ignoring unimplemented CSI A with intermediates")` 后丢弃。
#[test]
fn scroll_right_and_left_are_unimplemented_upstream() {
    let before = screen_text(
        b"ABCDEFGHIJKLMNOP\r\nABCDEFGHIJKLMNOP\r\nABCDEFGHIJKLMNOP\r\n\x1b[2;3r\x1b[3;1H",
        6,
        40,
    );
    let after_right = screen_text(
        b"ABCDEFGHIJKLMNOP\r\nABCDEFGHIJKLMNOP\r\nABCDEFGHIJKLMNOP\r\n\x1b[2;3r\x1b[3;1H\x1b[2 A",
        6,
        40,
    );
    assert_eq!(
        before, after_right,
        "CSI 2 SP A（SR）当前被上游丢弃，屏幕无任何变化"
    );
    let after_left = screen_text(
        b"ABCDEFGHIJKLMNOP\r\nABCDEFGHIJKLMNOP\r\nABCDEFGHIJKLMNOP\r\n\x1b[2;3r\x1b[3;1H\x1b[2 @",
        6,
        40,
    );
    assert_eq!(
        before, after_left,
        "CSI 2 SP @（SL）当前被上游丢弃，屏幕无任何变化"
    );
}

/// DECSC/DECRC 正确还原光标行列；差异只在 pending wrap 标志。
///
/// 该断言钉住的是「无末列折行挂起时落点与 xterm 一致」这一半，
/// 使上面那半偏差（见 vt_conformance 的 t0060/t0061）不会掩盖一个真实的落点缺陷。
#[test]
fn decsc_decrc_restore_the_saved_row_and_column() {
    let rows = screen_text(b"\x1b[7;79Hv\x1b7\r\n...\x1b[Sooo\x1b8^", 10, 80);
    assert_eq!(rows[5].find('v'), Some(78), "末列内容应随滚动上移一行");
    assert_eq!(rows[6].find('^'), Some(79), "DECRC 必须还原到保存的列");
}

/// DECSC/DECRC 会连同 pending wrap 标志一起还原；xterm 只还原行列。
///
/// 这条不是缺口本身，而是缺口的确切位置：末列打印后进入 pending wrap，
/// 此时 DECSC → DECRC → 打印一个字符，上游先折行再打印，xterm 直接落在原列。
#[test]
fn decrc_restores_pending_wrap_which_xterm_does_not() {
    let rows = screen_text(b"\x1b[7;80Hv\x1b7\r\n...\x1b[Sooo\x1b8^", 10, 80);
    assert_eq!(
        rows[7].find('^'),
        Some(0),
        "上游 DECRC 还原了 pending wrap，故字符先折行到下一行行首；xterm 落在原列 79"
    );
}
