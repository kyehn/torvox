use std::sync::Arc;
use std::sync::atomic::Ordering;

use jni::sys::JNI_FALSE;
use parking_lot::Mutex;

use super::{RenderWake, SessionEntry};
use crate::terminal::session::Session;

/// 渲染采样读的是注册表里的共享原子量，不是会话锁后的字段：一旦有人改成
/// 另建一个标志，这条指针相等断言会先红，避免慢帧修复被静默回退。
#[test]
fn entry_shares_session_flags_for_lock_free_sampling() {
    let (session, _handle) = crate::terminal::mock_pty::mock_session();
    let new_output = session.new_output_flag();
    let alt_screen = session.alt_screen_active_flag();
    let mouse_tracking = session.mouse_tracking_active_flag();
    let entry = SessionEntry::new(
        Arc::new(Mutex::new(session)),
        Arc::new(RenderWake::default()),
    );
    assert!(Arc::ptr_eq(&entry.new_output, &new_output));
    assert!(Arc::ptr_eq(&entry.alt_screen_active, &alt_screen));
    assert!(Arc::ptr_eq(&entry.mouse_tracking_active, &mouse_tracking));
}

/// 输入路径的状态查询（备用屏、鼠标上报）必须**不取会话锁**。
///
/// 会话锁可被 PTY 写入按住数秒（`feed_pty` 的写入面正是这样克隆出去再写），
/// 而这两个查询在触摸滚动的每个 MOVE 事件里都要判一次。持锁读会把 UI 线程
/// 按在写入洪流上，表现为滚动卡顿。
///
/// 回归钉的方式：**占住会话锁**，再走导出所用的读取路径
/// （`read_terminal_flag_jboolean` → 注册表查找 → 共享原子量读）——取锁的
/// 实现会在此阻塞到锁被释放，而无锁实现立即返回。锁由本测试线程持有并在读
/// 完后释放，故若实现取了锁，本测试会死锁（而非失败）——这正是「必须不取锁」
/// 的判定。为免死锁掩盖成挂起，用 `try_lock` 先验证锁确实被占住。
///
/// 会话须登记进注册表，否则读取路径在拿锁之前就短路返回「未知会话」，
/// 断言便测不到锁。
#[test]
fn input_path_state_queries_do_not_take_the_session_lock() {
    use parking_lot::MutexGuard;

    const SESSION_ID: u64 = 0xF11E_5EED_BEEF;

    let (session, _handle) = crate::terminal::mock_pty::mock_arc_session();
    let entry = SessionEntry::new(Arc::clone(&session), Arc::new(RenderWake::default()));

    // 登记进注册表：读取路径必须能真的找到这条会话才谈得上「读它的标志」。
    super::wlock_session_registry().insert(SESSION_ID, entry);

    // 占住会话锁：此后任何 `session.lock()` 都会阻塞到 guard 被丢弃。
    let guard: MutexGuard<'_, Session> = session.lock();
    assert!(session.try_lock().is_none(), "前置条件：会话锁须已被占住");

    // 走导出所用的读取路径（注册表查找 + 共享原子量读），全程不碰会话锁。
    let alt_screen = super::state::read_terminal_flag_jboolean(SESSION_ID, |entry| {
        entry.alt_screen_active.load(Ordering::Acquire)
    });
    let mouse_tracking = super::state::read_terminal_flag_jboolean(SESSION_ID, |entry| {
        entry.mouse_tracking_active.load(Ordering::Acquire)
    });
    assert_eq!(alt_screen, JNI_FALSE, "备用屏初始须为假");
    assert_eq!(mouse_tracking, JNI_FALSE, "鼠标上报初始须为假");

    drop(guard);
    super::wlock_session_registry().remove(&SESSION_ID);
}
