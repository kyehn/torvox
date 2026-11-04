use super::super::lifecycle::exit_signal_target;
use super::super::spawn::{ReaderErrorAction, read_error_action};
use super::super::*;
use super::{drain_output, spawn_test_session, wait_for_reported_exit};

use crate::terminal::shell_env::ShellEnv;
use nix::errno::Errno;
use std::time::Duration;

/// execve 失败的会话：退出码是 `100 + errno` 编码，失败原因由回报管道给出。
///
/// 判别力：旧实现靠退出码反推 errno（`code - 100`），shell 被 SIGHUP 杀掉时的
/// 退出码 129 会被误判成「errno=29 的 execve 失败」，于是正常会话收尾也打出
/// E 级日志。现在失败与否只由子进程写回管道决定。
#[test]
fn exec_failure_session_exits_with_errno_encoding() {
    let mut session = Session::spawn(
        "/nonexistent/session-shell-for-exec-failure",
        24,
        80,
        &ShellEnv::default(),
        None,
        None,
        2000,
    )
    .expect("spawn failed");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    drain_output(&mut session, deadline);
    assert!(session.is_exited());
    assert_eq!(
        wait_for_reported_exit(&session, deadline),
        Some(ReportedExit::Code(100 + Errno::ENOENT as i32)),
        "execve 失败的退出码必须是 100 + errno 编码"
    );
}

#[test]
fn take_reported_exit_waits_for_the_real_code_then_reports_once() {
    // 这是本次改动的核心契约：真实 `exit 3` 的会话必须报出 3 而不是 0。
    // 旧实现先锁存上报标志、再忙等退出码，等不到就报 0，且退出事件不会重发——
    // 于是一次 3 退出被永久谎报成 0。
    let mut session = spawn_test_session();
    // 用非零码：裸 `exit` 的惯例码是 0，断言 0 等于没断言——正是本缺陷的形态。
    session.write(b"exit 3\n").expect("write failed");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    drain_output(&mut session, deadline);
    assert!(session.is_exited());
    // 每帧轮询直到可上报；真实退出码必然在等待线程里被写入，无需忙等。
    let reported = wait_for_reported_exit(&session, deadline)
        .expect("exit code must become reportable well within 3s");
    assert_eq!(reported, ReportedExit::Code(3));
    // 恰好一次：此后不再有可上报的退出。
    assert_eq!(session.take_reported_exit(), None);
}

#[test]
fn take_reported_exit_is_silent_before_exit() {
    // 运行中的会话不得产生退出上报——否则会误报一个还在跑的 shell。
    let session = spawn_test_session();
    assert!(!session.is_exited());
    assert_eq!(session.take_reported_exit(), None);
}

#[test]
fn unknown_exit_code_is_reported_rather_than_suppressed() {
    // `waitpid` 失败时代码槽为 Unknown。必须照常上报（`ReportedExit::Unknown`），
    // 否则退出事件永不到达、会话表泄漏——比报出错误的码更糟。
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    *session.exit_code.lock() = ExitCodeSlot::Unknown;
    session.exited.store(true, Ordering::Release);
    assert_eq!(session.take_reported_exit(), Some(ReportedExit::Unknown));
}

#[test]
fn mark_exit_reported_is_idempotent_under_concurrency() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    // 上报竞态中只能有一个调用者胜出；其余（并发的 pollEvent 线程）都须看到 false，
    // 使 Exit 事件永不重复。`Arc<Mutex<..>>` 镜像生产 SESSION_REGISTRY 的形状
    // （`Box<dyn Pty>` 非 Sync，故会话须经互斥锁共享）。
    let session = std::sync::Arc::new(parking_lot::Mutex::new(session));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let session = session.clone();
        handles.push(std::thread::spawn(move || {
            session.lock().mark_exit_reported()
        }));
    }
    let winners = handles
        .into_iter()
        .filter_map(|join_handle| join_handle.join().ok())
        .filter(|won| *won)
        .count();
    assert_eq!(winners, 1, "exactly one caller must report the exit");
    // 后续调用保持 false。
    assert!(!session.lock().mark_exit_reported());
}

/// `request_exit` 必须同时置位会话自己的退出标志。
///
/// 此前这条用例断言的是 `MockPtyHandle::is_exited`，与会话无关：把
/// `request_exit` 整段删掉它照样全绿。`destroySession` 正是靠该标志让此后所有写入
/// 报 `SessionError::Closed`，标志不置位则已关闭的会话仍可接收输入。
#[test]
fn request_exit_marks_the_session_exited() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    assert!(!session.is_exited(), "新建会话不得已是退出态");
    session.request_exit();
    assert!(session.is_exited(), "request_exit 后必须置位退出标志");
    assert!(
        session.exited_flag().load(Ordering::Acquire),
        "对外暴露的标志与 is_exited 必须同源"
    );
}

/// 护栏：满输出通道下的会话拆卸必须及时完成。
///
/// 读取线程会持续生产直到通道满（128 块）然后阻塞在 `send`。`Drop` 体运行时
/// `output_rx` 仍存活，故若不先换掉接收端，读取线程永远等不到解除——四个
/// `wait_finished` 窗口全部超时，每次关闭会话都要付满约 400ms 并泄漏线程句柄。
/// 这里用真实 PTY 产生远超通道容量的输出，再断言拆卸在预算内返回。
#[test]
fn drop_does_not_stall_on_a_full_output_channel() {
    // 预算：正常拆卸路径含 SIGHUP/SIGCONT 与两个 50ms grace 窗口，理论下界远小于
    // 1s。旧实现在此稳定超过 1s（四个窗口全超时 + 重试）。
    let start = std::time::Instant::now();
    {
        let mut session = spawn_test_session();
        session.write(b"seq 1 4000000\n").expect("write failed");
        // 等到通道真的填满：此时读取线程必然阻塞在 `send` 上。
        //
        // 不等填满就 drop 是测不出来的——子进程可能还没产出 128 块，读取线程于是
        // 从 EOF 正常退出，断言有没有修复都会通过。这正是我第一版测试无效的原因。
        let fill_deadline = std::time::Instant::now() + Duration::from_secs(10);
        while session.output_rx.len() < OUTPUT_CHANNEL_BOUND {
            assert!(
                std::time::Instant::now() < fill_deadline,
                "output channel never filled; this test cannot discriminate"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "dropping a session with a full output channel took {elapsed:?},              the reader thread is blocked on send()"
    );
}

#[test]
fn request_exit_kills_child_while_session_still_alive() {
    // destroySession 从注册表摘下条目后即向宿主返回，此时剪贴板应答等异步
    // 克隆仍持有 Arc<Mutex<Session>>，Drop 尚未执行——shell 必须在此时就死，
    // 否则宿主已经认为会话销毁、shell 却在后台继续跑（生命周期契约破裂）。
    let mut session = spawn_test_session();
    session.write(b"echo alive\n").expect("write failed");
    let pid = session.pty.child_pid();
    assert!(pid.as_raw() > 0, "spawned session must have a child pid");

    session.request_exit();
    // 重复拆卸必须短路，不 panic，也不再碰子进程 PID。
    session.request_exit();

    // 子进程必须在 request_exit 返回后即已消失（信号已投递）。
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut still_running = true;
    while std::time::Instant::now() < deadline {
        match nix::sys::signal::kill(pid, None) {
            Err(nix::errno::Errno::ESRCH) => {
                still_running = false;
                break;
            }
            Ok(()) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => panic!("unexpected kill probe error: {error}"),
        }
    }
    assert!(
        !still_running,
        "request_exit returned but child {} is still alive — destroySession would \
             report success while the shell keeps running",
        pid.as_raw()
    );
    // 会话对象本身仍在（模拟异步克隆持有），清理交给 Drop。
    assert!(session.is_exited());
}

#[test]
fn read_error_action_classifies_errno() {
    // EINTR/EAGAIN 是瞬时错误，必须保持读取循环存活。
    assert_eq!(
        read_error_action(Some(Errno::EINTR)),
        ReaderErrorAction::Retry
    );
    assert_eq!(
        read_error_action(Some(Errno::EAGAIN)),
        ReaderErrorAction::Retry
    );
    // EIO 表示 PTY 从端已关闭（Linux PTY 上的 EOF）——停止。
    assert_eq!(read_error_action(Some(Errno::EIO)), ReaderErrorAction::Stop);
    // 其他 errno（或无 errno）同样停止读取。
    assert_eq!(
        read_error_action(Some(Errno::EPIPE)),
        ReaderErrorAction::Stop
    );
    assert_eq!(read_error_action(None), ReaderErrorAction::Stop);
}

/// 收尾信号目标的选择就是自杀 guard 的全部逻辑：三种退化都必须直杀子进程。
#[test]
fn exit_signal_target_declines_group_kill_when_unsafe() {
    use nix::unistd::Pid;
    let pid = Pid::from_raw(4242);
    let own = Pid::from_raw(100);
    let child_group = Pid::from_raw(4243);
    // 正常情形：子进程自成组且与本进程不同组 —— 组杀。
    assert_eq!(
        exit_signal_target(pid, Some(child_group), Some(own)),
        Pid::from_raw(-4243)
    );
    // setsid 前：子进程与本进程同组，组杀会连带杀死本进程 —— 直杀子进程。
    assert_eq!(exit_signal_target(pid, Some(own), Some(own)), pid);
    // pgid == 1 时 `-pgid` 就是 -1，会命中本进程有权发信号的所有进程 —— 直杀子进程。
    assert_eq!(
        exit_signal_target(pid, Some(Pid::from_raw(1)), Some(own)),
        pid
    );
    // 本进程组号取不到：确认不了就不组杀。
    assert_eq!(exit_signal_target(pid, Some(child_group), None), pid);
    // 子进程组号取不到 —— 直杀子进程。
    assert_eq!(exit_signal_target(pid, None, Some(own)), pid);
}
