use super::state::scroll_target;
use super::{RenderWake, SessionEntry, rlock_session_registry, wlock_session_registry};
use crate::terminal::ghostty_terminal::ViewportTarget;
use std::sync::Arc;

/// 滚动记账归会话，重置须把它一并归零。
///
/// 该值此前挂在注册表项上，于是滚动与重置两条路径都得在**注册表写锁**内取会话锁
/// ——而会话锁可被 PTY 写入按住数秒，一条只读查找因此能冻住整台终端。归位到会话
/// 内（原子量）后两条路径只取读锁，语义不变。
#[test]
fn scroll_bookkeeping_lives_on_the_session_and_reset_clears_it() {
    let (session, _handle) = crate::terminal::mock_pty::mock_arc_session();
    let session_id = 98_090_101u64;
    wlock_session_registry().insert(
        session_id,
        SessionEntry::new(Arc::clone(&session), Arc::new(RenderWake::default())),
    );

    session.lock().set_last_scroll_offset(37);
    // 经注册表读锁取回的句柄看到的是同一份记账（两个会话各自独立，见下一条）。
    let through_registry = Arc::clone(
        &rlock_session_registry()
            .get(&session_id)
            .expect("已登记")
            .session,
    );
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
        let (session, _handle) = crate::terminal::mock_pty::mock_arc_session();
        session
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

/// 宿主偏移 0 必须译成「回到活动区」这个**意图**，不能译成增量回退。
///
/// 判别力：回滚浏览期间若有新输出，活动区底边已经下移；按增量回退只会退回浏览
/// 起点，停在离底边若干行处——新输出落在视口之外，终端看上去「死了」。
#[test]
fn scroll_target_maps_zero_offset_to_bottom() {
    assert_eq!(
        scroll_target(0, 3),
        Some(ViewportTarget::Bottom),
        "偏移 0 是回到底部的意图"
    );
    assert_eq!(scroll_target(0, 0), None, "偏移未变则无需滚动");
    assert_eq!(
        scroll_target(5, 3),
        Some(ViewportTarget::Delta(-2)),
        "向上滚入历史发负增量"
    );
    assert_eq!(
        scroll_target(1, 3),
        Some(ViewportTarget::Delta(2)),
        "回到更新内容发正增量"
    );
}
