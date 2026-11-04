use super::super::*;

/// 子进程未启用 1004 时不得发出任何焦点字节。
///
/// 契约写在 `focus_event_bytes` 上：序列必须**直写**子 PTY（送入 VT 引擎只会把
/// `CSI I` 当成 CHT 把光标移到下一个制表位），且只在子进程自己开 1004 时才发。
/// 此前该函数**没有任何断言**（只调用不检查），这正是它在无人察觉的情况下被改坏
/// 的原因。
///
/// 「已启用时写出 `CSI I`/`CSI O`」这半边在宿主单测里**不覆盖**：它要先问一次
/// VT 线程模式，答复要穿过 VT 线程的空闲 tick 与整网格重建，并发跑全量测试时
/// 该往返耗时无上界，写成断言就是一条不稳定用例（TESTING.md 禁止）。此处如实
/// 登记为覆盖缺口，不以重试或放宽断言冒充覆盖；与本契约直接相关、且能确定覆盖的
/// 那一半——查询窗口必须宽于该 tick——由下一条用例钉住。
#[test]
fn focus_event_stays_silent_until_the_child_enables_mode_1004() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");

    assert_eq!(session.focus_event_bytes(true), None);
    assert_eq!(session.focus_event_bytes(false), None);
    assert!(
        handle.written().is_empty(),
        "1004 未启用时不得发出任何字节，收到 {:?}",
        String::from_utf8_lossy(&handle.written())
    );
}

/// 焦点模式查询的窗口必须严格宽于 VT 线程的空闲 tick。
///
/// VT 线程处理命令批期间不排空查询（批尾才应答），故一次查询最坏要等一整批
/// 命令处理完。两值相等即赛跑：超时先到就被当成「子进程没开 1004」，这一次焦点
/// 上报静默丢失，vim 一类按 1004 切高亮的程序随即停在错误状态。
///
/// 行为用例（`focus_event_stays_silent_until_the_child_enables_mode_1004`）抓不住
/// 它——那要撞上线程调度，红不红随机器而变。故此处直接钉住这条不等式。
#[test]
fn focus_mode_query_window_outlives_the_vt_idle_tick() {
    let tick_ms = crate::terminal::ghostty_terminal::VT_IDLE_TICK_MS;
    assert!(
        Session::FOCUS_MODE_QUERY_TIMEOUT_MS > tick_ms,
        "查询窗口 {}ms 须严格大于 VT 空闲 tick {tick_ms}ms，否则焦点上报随机丢失",
        Session::FOCUS_MODE_QUERY_TIMEOUT_MS
    );
}
