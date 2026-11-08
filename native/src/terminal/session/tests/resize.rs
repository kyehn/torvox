use super::super::*;
use super::spawn_test_session;

#[test]
fn session_resize() {
    let mut session = spawn_test_session();
    session.resize(40, 120).expect("resize failed");
    assert_eq!(session.terminal().rows(), 40);
    assert_eq!(session.terminal().cols(), 120);
}

/// 零行/零列不是合法网格：必须在触及 PTY 与 Ghostty 之前被拒绝，
/// 且不消耗一次 resize 计数（PTY 未被改动）。
#[test]
fn session_resize_rejects_zero_dimensions() {
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
    let before = handle.resize_count();
    for (rows, cols) in [(0, 80), (24, 0), (0, 0)] {
        assert!(
            matches!(
                session.resize(rows, cols),
                Err(SessionError::InvalidDimensions)
            ),
            "{rows}x{cols} 必须被判为非法尺寸"
        );
    }
    assert_eq!(handle.resize_count(), before, "非法尺寸不得下发到 PTY");
}

/// 创建与缩放共用同一条尺寸判据：越界值不得靠 `as u16` 静默截断成「合法」网格。
///
/// 截断后的尺寸（70000 → 4464）本身是合法网格，没有任何日志可循——会话一建好就是错的，
/// 而 `resize` 对同一个值如实报错。两条路径必须给出同一个答案。
#[test]
fn session_spawn_rejects_dimensions_outside_the_u16_grid() {
    for (rows, cols) in [
        (0, 80),
        (24, 0),
        (u32::from(u16::MAX) + 1, 80),
        (24, 70_000),
    ] {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let result = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, rows, cols);
        assert!(
            matches!(result, Err(SessionError::InvalidDimensions)),
            "{rows}x{cols} 必须在创建时被判为非法尺寸"
        );
    }
}

#[test]
fn session_resize_same_size_is_applied_noop() {
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
    let before = handle.resize_count();
    // 干净状态：同尺寸 resize 短路并报告 Applied。
    let outcome = session.resize(24, 80).expect("resize failed");
    assert!(matches!(outcome, ResizeOutcome::Applied));
    assert!(!session.grid_dirty.load(Ordering::Acquire));
    // 短路不得改动 PTY。
    assert_eq!(
        handle.resize_count(),
        before,
        "pty.resize must not be called"
    );
    // 网格未变（仍是 spawn 时的尺寸）。
    assert_eq!(session.grid_size(), (24, 80));
    assert_eq!(session.terminal().rows(), 24);
}

#[test]
fn session_resize_dirty_same_size_still_retries() {
    let mut session = spawn_test_session();
    // 模拟网格命令被丢弃：置脏标志，缓存保持旧尺寸。
    session.grid_dirty.store(true, Ordering::Release);
    // 同尺寸 resize 不得短路：须重发 ioctl 与网格命令并清除脏标志（修复分歧）。
    let outcome = session.resize(24, 80).expect("resize failed");
    assert!(matches!(outcome, ResizeOutcome::Applied));
    assert!(!session.grid_dirty.load(Ordering::Acquire));
    assert_eq!(session.grid_size(), (24, 80));
}

#[test]
fn session_resize_dirty_cleared_on_success() {
    let mut session = spawn_test_session();
    session.grid_dirty.store(true, Ordering::Release);
    // 真正不同的尺寸在成功后清除脏标志。
    let outcome = session.resize(40, 120).expect("resize failed");
    assert!(matches!(outcome, ResizeOutcome::Applied));
    assert!(!session.grid_dirty.load(Ordering::Acquire));
    assert_eq!(session.grid_size(), (40, 120));
}

#[test]
fn session_resize_sends_signal() {
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
    session.resize(40, 120).expect("resize must succeed");
    assert_eq!(
        session.terminal().rows(),
        40,
        "terminal rows must update after resize"
    );
    assert_eq!(
        session.terminal().cols(),
        120,
        "terminal cols must update after resize"
    );
    assert_eq!(handle.rows(), 40, "PTY rows must update after resize");
    assert_eq!(handle.cols(), 120, "PTY cols must update after resize");
}

/// 回归钉：VT 线程上的 resize 失败后，同尺寸的下次 resize 不得再被短路。
///
/// `GhosttyTerminal::resize` 只报告命令**是否入队**，而 `Terminal::resize` 真正
/// 在 VT 线程上执行，二者之间还隔着一次可能失败的上游调用。此前该失败只记日志：
/// `Session::resize` 已按「已入队」把缓存尺寸发布成新值并清掉 `grid_dirty`，
/// 于是同尺寸的下一次 resize 走 `!dirty && 尺寸未变` 短路——PTY 与网格永久分歧，
/// 且没有任何重试时机。
///
/// 判别力看 `MockPty::resize_count`：短路分支根本不碰 PTY，故计数不增；
/// 重试则必然经过 `pty.resize`。（前一版只断言 `grid_dirty`/`resize_failed`
/// 这类间接量，还原实现后仍全绿——它并没有观察是否真的重发。）
#[test]
fn vt_thread_resize_failure_forces_the_next_resize_to_retry() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::spawn_with_pty(
        Box::new(pty),
        30,
        100,
        None,
        Some(std::env::temp_dir()),
        2000,
    )
    .expect("session");

    // 基线：同尺寸第二次本就该短路（既有去重，不能被破坏）。
    session.resize(30, 100).expect("first resize");
    let after_first = handle.resize_count();
    session.resize(30, 100).expect("second resize");
    assert_eq!(
        handle.resize_count(),
        after_first,
        "无失败标记时同尺寸必须继续短路"
    );

    // 模拟「命令已入队但 VT 线程上失败」。
    session.terminal.set_resize_failed_for_test(true);
    session.resize(30, 100).expect("retry resize");
    assert_eq!(
        handle.resize_count(),
        after_first + 1,
        "VT 线程 resize 失败后，同尺寸的下一次必须真正重发（PTY 计数应 +1）"
    );
}
