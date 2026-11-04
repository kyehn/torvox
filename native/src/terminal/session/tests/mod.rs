//! 会话层测试：按职责拆成若干子模块，夹具留在本文件里共享。
//!
//! 子模块用 `use super::super::*;` 取会话层的接口，用 `use super::{…};` 取本文件的
//! 夹具。夹具保持私有即可——子模块是本模块的后代，私有项对后代可见。

use super::{ReportedExit, Session};
use crate::terminal::shell_env::ShellEnv;
use std::time::Duration;

mod basics;
mod clipboard;
mod focus;
mod process;
mod resize;
mod write_backpressure;

fn drain_output(session: &mut Session, deadline: std::time::Instant) {
    while std::time::Instant::now() < deadline {
        session.process_output();
        if session.is_exited() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 等到退出码变为可上报，超时返回 `None`。
///
/// 退出标志有两条置位路径：读线程（PTY EOF）与等待线程（拿到退出码），
/// 二者无序——标志已置而码尚未发布时 `take_reported_exit()` 返回 `None`。
/// 生产路径正是逐帧轮询直到可上报（见 [`Session::take_reported_exit`]），
/// 测试若只读一次就会读到中间的 `None` 而偶发失败。
fn wait_for_reported_exit(session: &Session, deadline: std::time::Instant) -> Option<ReportedExit> {
    loop {
        if let Some(reported) = session.take_reported_exit() {
            return Some(reported);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// 为测试创建 24x80 的 `/bin/sh` 会话。
fn spawn_test_session() -> Session {
    Session::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None, None, 2000).expect("spawn failed")
}

/// 启动 shell、发送 `exit` 并等待会话报告退出。
fn spawn_and_exit() -> Session {
    let mut session = spawn_test_session();
    session.write(b"exit\n").expect("write failed");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    drain_output(&mut session, deadline);
    assert!(session.is_exited());
    session
}
