use std::sync::Arc;
use std::sync::atomic::Ordering;

use parking_lot::Mutex;

use super::state::busy_elapsed_nanos;
use super::{RenderWake, SessionEntry};
use crate::terminal::session::Session;

/// 忙标记未置位时读出的时长恒为 0。
///
/// 0 是「空闲」的哨兵：看门狗按「时长超过阈值」判定挂起，若空闲也给出正数，
/// 每个空闲超过阈值的 shell 都会被判死——那比原缺陷更严重。
#[test]
fn idle_busy_marker_reads_as_zero() {
    assert_eq!(busy_elapsed_nanos(0), 0);
}

/// 忙标记置位后读出的是**时长**而不是时刻。
///
/// 时钟域必须留在 Rust 侧：忙标记存 Rust 单调时钟的读数，JVM 的
/// `System.nanoTime()` 基准是另一个原点，跨边界相减得到的「时长」没有意义。
/// 故导出必须给时长，且「刚置位」也要给出正值——否则与「空闲」的 0 重合。
#[test]
fn busy_marker_reads_out_elapsed_duration() {
    let busy_since = crate::monotonic_nanos();
    let elapsed = busy_elapsed_nanos(busy_since);
    assert!(elapsed > 0, "已置位的标记必须读出正时长：{elapsed}");
    assert!(
        elapsed < 1_000_000_000,
        "刚置位的标记时长须在 1s 内：{elapsed}ns"
    );
}

/// 注册表条目与会话共享同一份忙标记原子量。
///
/// 指针相等断言：一旦有人改成在条目里另建一个标记，看门狗读到的就永远不是
/// VT 线程真正写的那份，挂起判定会静默失效（恒读到 0＝永远「空闲」）。
#[test]
fn entry_shares_session_busy_markers() {
    let (session, _handle) = crate::terminal::mock_pty::mock_session();
    let vt_busy = session.vt_busy_since_nanos_flag();
    let pty_send = session.pty_send_since_nanos_flag();
    let entry = SessionEntry::new(
        Arc::new(Mutex::new(session)),
        Arc::new(RenderWake::default()),
    );
    assert!(Arc::ptr_eq(&entry.vt_busy_since_nanos, &vt_busy));
    assert!(Arc::ptr_eq(&entry.pty_send_since_nanos, &pty_send));
}

/// 忙标记的读取路径不取会话锁。
///
/// 会话锁可被 PTY 写入按住数秒，而看门狗每 2s 就要读一次忙标记——持锁读会把
/// 看门狗按在写入洪流上，反而在最需要诊断的时候拿不到读数。
#[test]
fn busy_marker_reads_do_not_take_the_session_lock() {
    use parking_lot::MutexGuard;

    const SESSION_ID: u64 = 0xB051_0000_0001;

    let (session, _handle) = crate::terminal::mock_pty::mock_arc_session();
    let entry = SessionEntry::new(Arc::clone(&session), Arc::new(RenderWake::default()));
    super::wlock_session_registry().insert(SESSION_ID, entry);

    let guard: MutexGuard<'_, Session> = session.lock();
    assert!(session.try_lock().is_none(), "前置条件：会话锁须已被占住");

    let registry = super::rlock_session_registry();
    let entry = registry.get(&SESSION_ID).expect("会话须已登记");
    // 空闲时两个标记都是 0；读它们不得阻塞（本测试若取锁会死锁）。
    assert_eq!(
        busy_elapsed_nanos(entry.vt_busy_since_nanos.load(Ordering::Acquire)),
        0
    );
    assert_eq!(
        busy_elapsed_nanos(entry.pty_send_since_nanos.load(Ordering::Acquire)),
        0
    );

    drop(registry);
    drop(guard);
    super::wlock_session_registry().remove(&SESSION_ID);
}
