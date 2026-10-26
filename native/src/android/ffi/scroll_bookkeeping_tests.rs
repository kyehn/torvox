use super::{RenderWake, SessionEntry, rlock_session_registry, wlock_session_registry};
use crate::terminal::pty::Pty;
use crate::terminal::session::Session;
use std::sync::Arc;

/// 滚动记账归会话，重置须把它一并归零。
///
/// 该值此前挂在注册表项上，于是滚动与重置两条路径都得在**注册表写锁**内取会话锁
/// ——而会话锁可被 PTY 写入按住数秒，一条只读查找因此能冻住整台终端。归位到会话
/// 内（原子量）后两条路径只取读锁，语义不变。
#[test]
fn scroll_bookkeeping_lives_on_the_session_and_reset_clears_it() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Arc::new(parking_lot::Mutex::new(
        Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed"),
    ));
    let session_id = 98_090_101u64;
    wlock_session_registry().insert(
        session_id,
        SessionEntry::new(session.clone(), Arc::new(RenderWake::default())),
    );

    session.lock().set_last_scroll_offset(37);
    // 经注册表读锁取回的句柄看到的是同一份记账（两个会话各自独立，见下一条）。
    let through_registry = rlock_session_registry()
        .get(&session_id)
        .expect("已登记")
        .session
        .clone();
    assert_eq!(through_registry.lock().last_scroll_offset(), 37);

    through_registry.lock().reset_terminal();
    assert_eq!(
        session.lock().last_scroll_offset(),
        0,
        "重置后 VT 视口已归零，滚动记账必须同步归零，否则下一次滚动按 stale 值算 delta"
    );

    wlock_session_registry().remove(&session_id);
}

/// 两个会话的滚动记账互不相干。
///
/// 判别力：记账若挂在全局或注册表某处（此前正是「单一全局偏移」的历史成因），
/// B 会话滚动后 A 的记账也会变。
#[test]
fn scroll_bookkeeping_is_per_session() {
    let new_session = || {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        Arc::new(parking_lot::Mutex::new(
            Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
                .expect("with_pty must succeed"),
        ))
    };
    let first = new_session();
    let second = new_session();
    first.lock().set_last_scroll_offset(11);
    assert_eq!(
        second.lock().last_scroll_offset(),
        0,
        "另一会话的滚动不得推进本会话的记账"
    );
}
