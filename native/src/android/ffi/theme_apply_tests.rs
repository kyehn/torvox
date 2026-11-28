//! 换主题的锁纪律：等待 VT 线程排空期间不得持有注册表锁。

use super::appearance::apply_theme_to_session;

/// 换主题等待 VT 线程排空期间，注册表必须仍可写。
///
/// 换主题要等 VT 线程把新单元数据推出来（上限 5s）。这段等待若跨过注册表读锁，
/// 销毁/切换会话（注册表写锁）就要等满这 5s——一次主题切换把整台终端冻住。
///
/// 回归钉的方式：**占住注册表写锁**，再走换主题的会话侧动作——它若还要取
/// 注册表读锁，此刻即阻塞到本测试释放（parking_lot 读写锁不可由读升级为写），
/// 即本测试死锁而非失败；无注册表依赖的实现立即返回。锁由本测试线程持有并在
/// 动作做完后释放。
#[test]
fn theme_application_does_not_touch_the_registry_lock() {
    let (session, _handle) = crate::terminal::mock_pty::mock_arc_session();
    let theme = crate::terminal::theme::default_dark();

    // 占住注册表写锁：换主题的会话侧动作若取注册表读锁，此刻即阻塞。
    let registry_guard = super::wlock_session_registry();
    apply_theme_to_session(&session, theme.name, theme.palette.clone());
    drop(registry_guard);
}
