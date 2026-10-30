//! 响铃（BEL = 0x07）：`termux.properties` 的 `bell-character` 的数据源。
//!
//! 断言对象是本仓的接线——BEL 是否被上游回调捕获、是否以「有/无」的二值形式交给宿主，
//! 以及作为 OSC 终止符出现的 BEL 会不会被误当成一次响铃。

use super::*;

/// 普通 BEL 必须置位待上报标志。
#[test]
fn bel_sets_the_pending_flag() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x07");
    terminal_under_test.flush();
    assert!(
        terminal_under_test.take_bell(),
        "BEL must be reported to the host"
    );
}

/// 取走之后标志清零：否则一次响铃会被每帧重复上报，用户听到无止境的提示音。
#[test]
fn taking_the_bell_clears_it() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x07");
    terminal_under_test.flush();
    assert!(terminal_under_test.take_bell());
    assert!(
        !terminal_under_test.take_bell(),
        "the flag must clear after being taken"
    );
}

/// 连续多次 BEL 合并为一次待上报：程序连发 20 个与发 1 个对用户是同一种提示，
/// 而按次入队会让一个每秒刷新的 TUI 把事件队列灌满并挤掉剪贴板事件。
#[test]
fn consecutive_bells_collapse_into_one() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x07\x07\x07");
    terminal_under_test.flush();
    assert!(terminal_under_test.take_bell());
    assert!(
        !terminal_under_test.take_bell(),
        "N bells in one batch must collapse into a single pending flag"
    );
}

/// 以 BEL 结尾的 OSC 序列（xterm 允许）里的那个 BEL 是终止符，不是响铃。
///
/// 若不区分，每次程序设置窗口标题都会「叮」一声——而这恰恰是最常见的 OSC 用法。
#[test]
fn bel_terminating_an_osc_sequence_does_not_ring() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]0;build finished\x07");
    terminal_under_test.flush();
    assert!(
        !terminal_under_test.take_bell(),
        "an OSC terminator BEL must not be reported as a bell"
    );
}
