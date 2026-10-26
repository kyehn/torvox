use std::sync::Arc;

use parking_lot::Mutex;

use super::{RenderWake, SessionEntry};
use crate::terminal::mock_pty::MockPty;
use crate::terminal::session::Session;

/// 渲染采样读的是注册表里的共享原子量，不是会话锁后的字段：一旦有人改成
/// 另建一个标志，这条指针相等断言会先红，避免慢帧修复被静默回退。
#[test]
fn entry_shares_session_flags_for_lock_free_sampling() {
    let (pty, _handle) = MockPty::new(24, 80);
    let session =
        Session::with_pty_for_test(Box::new(pty) as Box<dyn crate::terminal::pty::Pty>, 24, 80)
            .expect("session");
    let new_output = session.new_output_flag();
    let alt_screen = session.alt_screen_active_flag();
    let entry = SessionEntry::new(
        Arc::new(Mutex::new(session)),
        Arc::new(RenderWake::default()),
    );
    assert!(Arc::ptr_eq(&entry.new_output, &new_output));
    assert!(Arc::ptr_eq(&entry.alt_screen_active, &alt_screen));
}
