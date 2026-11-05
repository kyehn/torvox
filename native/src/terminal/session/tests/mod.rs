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

/// 持住主端写入锁的守卫：放行并回收那条「用户输入」线程。
///
/// 放行绑在守卫上：忘记放行的用例会持着锁不放，把整个测试进程拖死，
/// 而「挂死的用例」比「判红的用例」更难排查。
struct PtyWriteLockHold {
    release: Option<std::sync::mpsc::Sender<()>>,
    holder: Option<std::thread::JoinHandle<()>>,
}

impl PtyWriteLockHold {
    /// 放行持锁线程并回收它；持锁线程 panic 即判红。
    ///
    /// 用例须显式调它：守卫析构只是兜底（析构里 panic 会在展开中二次 panic 而 abort），
    /// 兜底路径不判红。原先两份拷贝里的 `join().expect(...)` 就是这条断言，
    /// 收敛到共享夹具后不得把它丢掉。
    fn release(mut self) {
        self.signal_and_join(true);
    }

    /// 放行并回收；`strict` 为真时持锁线程的 panic 向上传播。
    fn signal_and_join(&mut self, strict: bool) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        // `JoinHandle::join` 消耗句柄，只能取出来再 join。
        if let Some(holder) = self.holder.take() {
            if strict {
                holder.join().expect("用户写入线程 panicked");
            } else {
                let _ = holder.join();
            }
        }
    }
}

impl Drop for PtyWriteLockHold {
    fn drop(&mut self) {
        self.signal_and_join(false);
    }
}

/// 起一条「用户输入」线程持住主端写入锁，直到返回的守卫被释放。
///
/// 持住的是**主端写入锁**（`pty_write_order`）而不是会话锁：被测路径争的正是前者，
/// 两者不是同一个状态，否则钉不住「用户输入占着写入锁时另一条路径必须让开」这件事。
///
/// `acquire_budget` 是取锁等待上限：线程若迟迟拿不到锁，用例须判红而不是干等。
fn hold_pty_write_lock(
    session: &std::sync::Arc<parking_lot::Mutex<Session>>,
    acquire_budget: Duration,
) -> PtyWriteLockHold {
    use std::sync::mpsc;

    let (locked_sender, locked_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let session = std::sync::Arc::clone(session);
    let holder = std::thread::spawn(move || {
        // 写入面克隆出来再取锁：生产路径（`feedPty`）正是如此，
        // 否则持锁的是会话锁而非主端写入锁，两条就不是同一个状态。
        let pty_master = session.lock().pty_master();
        let _order = pty_master.pty_write_order();
        locked_sender.send(()).expect("已持锁");
        release_receiver.recv().expect("等放行");
    });
    locked_receiver
        .recv_timeout(acquire_budget)
        .expect("用户写入须在期限内取到主端写入锁");
    PtyWriteLockHold {
        release: Some(release_sender),
        holder: Some(holder),
    }
}
