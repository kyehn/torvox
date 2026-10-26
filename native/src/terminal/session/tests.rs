use super::*;

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
    Session::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None, None).expect("spawn failed")
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

#[test]
fn session_echo_hello() {
    let mut session = spawn_test_session();
    session.write(b"echo hello_p12\n").expect("write failed");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        session.process_output();
        let rows = session.terminal().rows();
        for row in 0..rows {
            if let Some(line) = session.terminal().read_line_text(row)
                && line.contains("hello_p12")
            {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(found, "did not find 'hello_p12' in terminal");
}

#[test]
fn session_resize() {
    let mut session = spawn_test_session();
    session.resize(40, 120).expect("resize failed");
    assert_eq!(session.terminal().rows(), 40);
    assert_eq!(session.terminal().cols(), 120);
}

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
    )
    .expect("spawn failed");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    drain_output(&mut session, deadline);
    assert!(session.is_exited());
    assert_eq!(
        wait_for_reported_exit(&session, deadline),
        Some(ReportedExit::Code(100 + libc::ENOENT)),
        "execve 失败的退出码必须是 100 + errno 编码"
    );
}

/// 回归护栏：PTY 写入不得在会话锁内阻塞。
///
/// 子进程长时间不读 stdin 时 `write_all` 最多等 `WRITE_DRAIN_TIMEOUT`（5s），
/// 而会话锁每帧都被 `render_inner` 与 `poll_event` 取得——写入一旦持锁，
/// 整台终端（渲染、事件收割、其它会话）就一起停摆。本测试用只到第一个字节就
/// 让写入线程挂起的替身（真实 `write_all` 到此返回 `Ok(1)`，不进入等待分支），
/// 断言会话锁在这段时间内仍可取得。
#[test]
fn pty_write_does_not_hold_the_session_lock() {
    use std::io;
    use std::os::unix::io::{OwnedFd, RawFd};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    /// 写入挂起直到本测试放行：模拟真实 `write_all` 在主端缓冲满时的等待分支。
    struct BlockingPty {
        entered: Arc<AtomicBool>,
        released: Arc<AtomicBool>,
    }

    impl Pty for BlockingPty {
        fn write(&self, buf: &[u8]) -> io::Result<usize> {
            self.entered.store(true, Ordering::Release);
            while !self.released.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(buf.len())
        }
        fn read(&self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "no data"))
        }
        fn resize(&self, _rows: u16, _cols: u16) -> Result<(), PtyError> {
            Ok(())
        }
        fn get_winsize(&self) -> Result<(u16, u16), PtyError> {
            Ok((24, 80))
        }
        fn child_pid(&self) -> nix::unistd::Pid {
            nix::unistd::Pid::from_raw(4_194_305)
        }
        fn master_fd(&self) -> RawFd {
            -1
        }
        fn try_clone_reader_fd(&self) -> io::Result<OwnedFd> {
            std::fs::File::open("/dev/null").map(OwnedFd::from)
        }
        fn set_nonblocking(&self) -> Result<(), PtyError> {
            Ok(())
        }
    }

    let entered = Arc::new(AtomicBool::new(false));
    let released = Arc::new(AtomicBool::new(false));
    let session = Arc::new(std::sync::Mutex::new(
        Session::with_pty_for_test(
            Box::new(BlockingPty {
                entered: entered.clone(),
                released: released.clone(),
            }) as Box<dyn Pty>,
            24,
            80,
        )
        .expect("with_pty must succeed"),
    ));
    let writer = session.lock().expect("session lock").pty_master();

    let writer_thread = std::thread::spawn(move || writer.write(b"x"));
    // 写入线程已进入 `Pty::write` 并挂起在等待分支。
    let entered_deadline = Instant::now() + Duration::from_secs(5);
    while !entered.load(Ordering::Acquire) && Instant::now() < entered_deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        entered.load(Ordering::Acquire),
        "写入线程必须已挂起在 PTY 写入内，否则本测试无判别力"
    );

    // 关键断言：写入挂起期间会话锁仍可取得。锁被占用即代表写入持锁——
    // 渲染与事件收割每帧都取这把锁，阻塞写入会把整台终端冻住。
    {
        let locked = session
            .try_lock()
            .expect("PTY 写入不得持有会话锁：渲染与事件收割每帧都取这把锁");
        assert_eq!(locked.terminal().rows(), 24, "取得的必须是本会话");
    }

    // 放行后写入照常完成（锁外写入的字节序与结果语义不变）。
    released.store(true, Ordering::Release);
    writer_thread
        .join()
        .expect("writer thread panicked")
        .expect("放行后写入必须成功");
    assert_eq!(session.lock().expect("session lock").terminal().rows(), 24);
}

/// 零行/零列不是合法网格：必须在触及 PTY 与 Ghostty 之前被拒绝，
/// 且不消耗一次 resize 计数（PTY 未被改动）。
#[test]
fn session_resize_rejects_zero_dimensions() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
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

#[test]
fn session_resize_same_size_is_applied_noop() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
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

/// OSC 52 读取应答以 `ESC ] 52 ; <selection> ; <base64> BEL` 写回 PTY。
#[test]
fn answer_clipboard_read_writes_esc52_reply() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    session
        .answer_clipboard_read("c", "Hello, 世界")
        .expect("answer must succeed");
    let written = handle.written();
    assert_eq!(
        written, b"\x1b]52;c;SGVsbG8sIOS4lueVjA==\x07",
        "answer must be base64-encoded OSC 52 with BEL terminator"
    );
}

/// 剪贴板应答为空时仍产生合法（空载荷）的 OSC 52 回复，即兼容 xterm 的“空剪贴板”响应。
#[test]
fn answer_clipboard_read_empty_text() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    session
        .answer_clipboard_read("c", "")
        .expect("answer must succeed");
    assert_eq!(handle.written(), b"\x1b]52;c;\x07");
}

/// 会话退出后写应答必须干净失败。
#[test]
fn answer_clipboard_read_after_exit_fails() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    session.exited_flag().store(true, Ordering::Release);
    let result = session.answer_clipboard_read("c", "text");
    assert!(result.is_err(), "exited session must reject writes");
    assert!(handle.written().is_empty(), "nothing may reach the PTY");
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
fn session_after_exit_returns_error() {
    let mut session = spawn_and_exit();
    let result = session.write(b"echo after-exit\n");
    assert!(
        matches!(result, Err(SessionError::Closed)),
        "write after exit must report Closed, got: {result:?}"
    );
}

#[test]
fn session_new_creates_pty() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    assert_eq!(
        session.terminal().rows(),
        24,
        "terminal rows must be 24 after creation"
    );
    assert_eq!(
        session.terminal().cols(),
        80,
        "terminal cols must be 80 after creation"
    );
    assert!(
        !session.is_exited(),
        "new session must not be in exited state"
    );
}

#[test]
fn session_resize_sends_signal() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
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

#[test]
fn session_write_input() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    session.write(b"hello world").expect("write must succeed");
    let written = handle.written();
    assert_eq!(
        written, b"hello world",
        "input written to session must reach PTY master"
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

/// 回归护栏：OSC 52 写入必须被收割，用户复制的内容才能进系统剪贴板。
///
/// 原实现把收割挂在 `flush_with_timeout(Duration::ZERO)` 成功分支上，而零期限
/// `recv_timeout` 实测 32 次 0 次命中 ack，故 OSC 52 事件永不上报
/// （`NativeBridgeSmokeTest > feedTerminal OSC52 write surfaces clipboard poll event`
/// 长期失败即由此而来）。本测试只依赖「投递 + 收割」，不依赖任何 flush 时序。
#[test]
fn osc52_write_is_harvested_without_relying_on_flush_ack() {
    let mut session = spawn_test_session();
    let marker = "harvest-probe";
    use base64::Engine;
    let payload = base64::engine::general_purpose::STANDARD.encode(marker.as_bytes());
    // 与 `feedTerminal` 同路径：直接投进 VT 解析器（不经 PTY，故不依赖回显）。
    session
        .terminal_mut()
        .vt_write(format!("\u{1b}]52;c;{payload}\u{7}").as_bytes());
    // 单次收割即须命中：VT 线程处理完 `vt_write` 后回调事件已在通道里，
    // `harvest_vt_side_effects` 是非阻塞取。这里**不轮询**——轮询会把「收割被
    // 条件门控住、每帧才有机会捡到」的实现也判为通过，护栏就形同虚设。
    // 与生产渲染帧完全同口径：只 `process_output` + 收割，**不 flush、不轮询
    // 额外同步**。VT 线程是异步的，事件在它处理完 `vt_write` 之后的某一帧被收割；
    // 这正是「每帧重复的拉取式收割」的设计前提，也让本护栏对「收割被条件门控住」
    // 的旧实现为红（门控一旦失配，事件在所有帧里都取不到）。
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut harvested = session.poll_clipboard();
    while harvested.is_none() && std::time::Instant::now() < deadline {
        session.process_output();
        std::thread::sleep(Duration::from_millis(5));
        harvested = session.poll_clipboard();
    }
    assert_eq!(
        harvested.as_deref(),
        Some(marker),
        "OSC 52 payload must reach the clipboard slot within a few render frames"
    );
}

/// 渲染暂停（`pollEvent` 停调）期间的连续 OSC 52 写入必须全部保留：
/// 单槽锁存会让后一次覆盖前一次，用户丢剪贴板内容。
#[test]
fn osc52_writes_queue_in_order_when_not_polled() {
    let mut session = spawn_test_session();
    use base64::Engine;
    let mut written = Vec::new();
    for marker in ["first-copy", "second-copy"] {
        let payload = base64::engine::general_purpose::STANDARD.encode(marker.as_bytes());
        session
            .terminal_mut()
            .vt_write(format!("\u{1b}]52;c;{payload}\u{7}").as_bytes());
        written.push(marker);
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut harvested = Vec::new();
    while harvested.len() < written.len() && std::time::Instant::now() < deadline {
        session.process_output();
        if let Some(text) = session.poll_clipboard() {
            harvested.push(text);
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert_eq!(
        harvested, written,
        "渲染暂停期间的 OSC 52 写入必须按序全部上报，不得互相覆盖"
    );
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

#[test]
fn session_title_default_is_empty() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    assert_eq!(session.terminal().title(), "");
}

#[test]
fn session_mode_get_default_false() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    // 模式 2004（bracketed paste）默认应关闭。
    assert!(!session.terminal().mode_get_with_timeout(
        libghostty_vt::terminal::Mode::BRACKETED_PASTE,
        std::time::Duration::from_millis(crate::terminal::ghostty_terminal::QUERY_TIMEOUT_MS),
    ));
}

/// 子进程未启用 1004 时不得发出任何焦点字节。
///
/// 契约写在 `focus_event_bytes` 上：序列必须**直写**子 PTY（送入 VT 引擎只会把
/// `CSI I` 当成 CHT 把光标移到下一个制表位），且只在子进程自己开 1004 时才发。
/// 此前该函数**没有任何断言**（只调用不检查），这正是它在无人察觉的情况下被改坏
/// 的原因。
///
/// 「已启用时写出 `CSI I`/`CSI O`」这半边在宿主单测里**不覆盖**：它要先问一次
/// VT 线程模式，答复要穿过 VT 线程的空闲 tick 与整网格重建，并发跑全量测试时
/// 该往返耗时无上界，写成断言就是一条不稳定用例（TESTING.md 禁止）。此处如实
/// 登记为覆盖缺口，不以重试或放宽断言冒充覆盖；与本契约直接相关、且能确定覆盖的
/// 那一半——查询窗口必须宽于该 tick——由下一条用例钉住。
#[test]
fn focus_event_stays_silent_until_the_child_enables_mode_1004() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");

    assert_eq!(session.focus_event_bytes(true), None);
    assert_eq!(session.focus_event_bytes(false), None);
    assert!(
        handle.written().is_empty(),
        "1004 未启用时不得发出任何字节，收到 {:?}",
        String::from_utf8_lossy(&handle.written())
    );
}

/// 部分写成功后回写队列只保留**未写出**的尾巴。
///
/// 主端满时 `write_all` 可能写出一部分再超时。此时若把整条应答退回队列，下一帧
/// 会把已发出的前缀再发一遍，子应用于是收到两条 DECRPM/DSR 应答；而全部丢掉又
/// 让它永远等不到一个完整回答。
#[test]
fn partial_write_back_requeues_only_the_unwritten_tail() {
    let query_count = 2;
    let queries = "\x1b[6n".repeat(query_count);

    // 基准：主端畅通时同样两条查询写出的字节总数。它是后面那条关系式的**外部**
    // 真值——若拿队列自身的长度当期望，「整条退回」与「全部丢掉」两种错法都会
    // 恰好对上（本轮第一版正是如此，两次变异都全绿）。
    let (plain_pty, plain_handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut plain = Session::with_pty_for_test(Box::new(plain_pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    plain.inject_output_for_test(queries.as_bytes());
    plain.process_output();
    plain.terminal_mut().flush();
    for _ in 0..query_count + 1 {
        plain.process_output();
    }
    let expected_total = plain_handle.written().len();
    assert!(expected_total > 0, "前置：基准写出须非空");

    let (mut pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    // 只写出前 3 字节、其后恒 EAGAIN：确定性地复现「部分写出后主端满」。
    pty.write_partial_then_block(3);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    session.inject_output_for_test(queries.as_bytes());
    session.process_output();
    session.terminal_mut().flush();

    session.process_output();
    assert!(
        session
            .pty_write_back_queue
            .iter()
            .all(|tail| !tail.is_empty()),
        "每条尾巴都须非空：整条退回会让已写前缀被重发，全部丢掉则丢应答"
    );
    let written_before_unblock = handle.written().len();
    assert!(
        written_before_unblock > 0 && written_before_unblock < expected_total,
        "前置：替身应写出一部分而非全部，实测 {written_before_unblock}/{expected_total}"
    );

    // 随后模拟子进程恢复消费：逐帧排空。总字节数须与基准**完全一致**——
    // 多写即重发前缀，少写即丢应答，两种错法都被这一条挡住。
    handle.force_unblock_writes();
    for _ in 0..query_count + 1 {
        session.process_output();
    }
    assert!(session.pty_write_back_queue.is_empty(), "队列须排空");
    assert_eq!(
        handle.written().len(),
        expected_total,
        "受阻后写出的总字节数须与畅通时一致（{expected_total}），既不重发也不丢"
    );
}

/// 焦点模式查询的窗口必须严格宽于 VT 线程的空闲 tick。
///
/// VT 线程处理命令批期间不排空查询（批尾才应答），故一次查询最坏要等一整批
/// 命令处理完。两值相等即赛跑：超时先到就被当成「子进程没开 1004」，这一次焦点
/// 上报静默丢失，vim 一类按 1004 切高亮的程序随即停在错误状态。
///
/// 行为用例（`focus_event_stays_silent_until_the_child_enables_mode_1004`）抓不住
/// 它——那要撞上线程调度，红不红随机器而变。故此处直接钉住这条不等式。
#[test]
fn focus_mode_query_window_outlives_the_vt_idle_tick() {
    let tick_ms = crate::terminal::ghostty_terminal::VT_IDLE_TICK_MS;
    assert!(
        Session::FOCUS_MODE_QUERY_TIMEOUT_MS > tick_ms,
        "查询窗口 {}ms 须严格大于 VT 空闲 tick {tick_ms}ms，否则焦点上报随机丢失",
        Session::FOCUS_MODE_QUERY_TIMEOUT_MS
    );
}

#[test]
fn vt_write_back_is_capped_per_frame_and_carries_over() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    let query_count = MAX_PTY_WRITES_PER_FRAME * 3;
    let queries = "\x1b[6n".repeat(query_count);
    session.inject_output_for_test(queries.as_bytes());
    // 先把查询送进 VT 线程并等它处理完，否则收割到的应答条数取决于线程调度。
    session.process_output();
    session.terminal_mut().flush();

    // 断言从**可观测的写入**反推，不假设「第一帧恰好写走整上限条」：`flush()`
    // 之后 VT 线程是否已回完一轮仍取决于调度，那一帧可能一条都没写。
    // 每条 DSR 应答形如 `ESC [ 行 ; 列 R`，故 `ESC [` 的个数即已写出的应答数。
    let written_responses = || {
        handle
            .written()
            .windows(2)
            .filter(|pair| *pair == b"\x1b[")
            .count()
    };
    session.process_output();
    assert!(
        written_responses() <= MAX_PTY_WRITES_PER_FRAME,
        "单帧写出的条数须有上界，实测 {}",
        written_responses()
    );
    for _ in 0..=query_count {
        if session.pty_write_back_queue.is_empty() {
            break;
        }
        session.process_output();
    }
    assert!(
        session.pty_write_back_queue.is_empty(),
        "逐帧续写后队列须排空，不得丢应答"
    );
    assert_eq!(
        written_responses(),
        query_count,
        "全部 {query_count} 条应答须恰好各写一次"
    );
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
    let mut session =
        Session::spawn_with_pty(Box::new(pty), 30, 100, None, Some(std::env::temp_dir()))
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

/// 回归钉：持锁写入进行中，`PtyMaster::child_pid` 仍须立即可读。
///
/// `PtyMaster::write` 持互斥量跑完整个 `write_all`，而它每有部分写入就重置期限，
/// 故持锁时间无上界。`request_exit` 经 `destroySession` 跑在 UI 线程上，若它也去
/// 取这把锁，子进程不读 stdin 时销毁会阻塞主线程，`Session::drop` 的
/// `join_with_timeout` 随之超时并分离等待线程。
///
/// 用 gate 确定性地停写入在临界区内（不是 sleep，无竞态）；读取放在**另一个线程**
/// 里经超时收取：若读取要取锁就会一直等下去，直接在主线程调用会把用例挂死而不是
/// 判红——挂死的用例比判红的更难排查。先收结果、再放行、最后才断言，保证任何分支
/// 下写入线程都会被放出来。
#[test]
fn child_pid_stays_readable_while_a_write_holds_the_master_lock() {
    use crate::terminal::mock_pty::{MockPty, WriteGate};
    use std::sync::mpsc;

    /// 读取上限。取锁的实现会等到 gate 放行才返回，故任何非零上限都能判红；
    /// 给足余量使正常路径绝不会因调度抖动而误报。
    const READ_BUDGET: Duration = Duration::from_millis(500);

    let (mut pty, _handle) = MockPty::new(24, 80);
    let gate = WriteGate::new();
    pty.gate_writes(gate.clone());
    let master = PtyMaster::new(Box::new(pty), Arc::new(AtomicBool::new(false)));

    let writer = {
        let master = master.clone();
        std::thread::spawn(move || {
            let _ = master.write(b"blocked");
        })
    };
    gate.wait_until_entered();

    // 只读路径两条一起验：`child_pid`（取字段）与 `resize`（ioctl）。
    // 后者是本轮由「与写入争锁」改为无锁的那一条。
    let (sender, receiver) = mpsc::channel();
    let reader = {
        let master = master.clone();
        std::thread::spawn(move || {
            let _ = sender.send((master.child_pid(), master.resize(30, 100)));
        })
    };
    let received = receiver.recv_timeout(READ_BUDGET);

    // 先收尾再断言：任何情况下都不把写入线程留在闸里。
    gate.release();
    writer.join().expect("writer thread panicked");
    reader.join().expect("reader thread panicked");

    let (pid, resize_result) = received.expect("只读路径不得取主端互斥量：此刻写入正持锁");
    assert!(pid.as_raw() > 0, "child pid 应为构造期缓存的值");
    assert!(
        resize_result.is_ok(),
        "resize 是纯 ioctl，排在无上界的写入之后必须仍是成功的：{resize_result:?}"
    );
}

/// 回归钉：逐帧应答续写等不到主端写入锁时按墙钟退出，而非拖住会话锁。
///
/// 墙钟闸此前只封住了 `write_all` 本身，取写入锁却是无期限的——而那条锁正是
/// 用户输入（`feedPty`）与 VT 应答共用的互斥量。于是「整段持会话锁的时间以墙钟
/// 为界」在用户输入正占着主端时并不成立：一条应答就能把渲染与看门狗按住满排空
/// 上限。用 gate 确定性地把一条用户输入停在临界区内（不靠 sleep，无竞态）。
#[test]
fn write_back_yields_the_frame_when_a_user_write_holds_the_master_lock() {
    use std::sync::mpsc;

    const READ_BUDGET: Duration = Duration::from_millis(500);

    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Arc::new(Mutex::new(
        Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed"),
    ));

    // 用户输入持写入锁停住（等价于 `feedPty` 卡在主端排空上），直到本用例放行。
    let (locked_sender, locked_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let user_write = {
        let session = session.clone();
        std::thread::spawn(move || {
            // 写入面克隆出来再取锁：生产路径（`feedPty`）正是如此，
            // 否则持锁的是会话锁而非主端写入锁，两条就不是同一个状态。
            let pty_master = session.lock().pty_master();
            let _order = pty_master.pty_write_order();
            locked_sender.send(()).expect("已持锁");
            release_receiver.recv().expect("等放行");
        })
    };
    locked_receiver
        .recv_timeout(READ_BUDGET)
        .expect("用户写入须在期限内取到主端写入锁");

    // 收割在另一个线程里做：取不到写入锁的实现会一直等下去，直接在本线程调用
    // 会把用例挂死而不是判红——挂死的用例比判红的更难排查。
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn({
        let session = session.clone();
        move || {
            let mut session = session.lock();
            session
                .pty_write_back_queue
                .push_back(b"\x1b[1;1R".to_vec());
            session.drain_pty_write_back();
            let _ = sender.send(session.pty_write_back_queue.len());
        }
    });
    let received = receiver.recv_timeout(READ_BUDGET);

    release_sender.send(()).expect("放行");
    user_write.join().expect("用户写入线程 panicked");
    worker.join().expect("收割线程 panicked");

    let pending = received.expect("会话锁不得在用户输入持写入锁时被按住满排空上限");
    assert_eq!(
        pending, 1,
        "等不到写入锁时应答须整条退回队列下帧续写，不得丢弃"
    );
}

/// 回归钉：待上报的剪贴板写入队列必须有界，且丢最旧。
///
/// 该队列此前的深度说明写着「受 `EVENT_CHANNEL_CAPACITY` 约束」——那只约束
/// 上游回调通道，而 `drain_callback_events` 每帧把通道排空进本队列、
/// `poll_clipboard` 每帧只取一条。程序以快于每帧一条的速率发 OSC 52 时队列
/// 每帧净增十余条且无上限；渲染暂停（设置页/输入法弹出）期间消费彻底停摆。
///
/// 走真实路径：投进 VT 解析器再 `process_output` 收割，不在用例里复刻封顶逻辑
/// ——那只会断言测试自己刚写下的代码。
#[test]
fn clipboard_write_queue_is_bounded_and_drops_oldest() {
    use base64::Engine;
    let mut session = spawn_test_session();
    let total = MAX_CLIPBOARD_TEXT_QUEUE + 5;
    for index in 0..total {
        let payload =
            base64::engine::general_purpose::STANDARD.encode(format!("clip-{index}").as_bytes());
        session
            .terminal_mut()
            .vt_write(format!("\u{1b}]52;c;{payload}\u{7}").as_bytes());
    }
    // 收割是逐帧的：反复 process_output 让 VT 线程把全部事件递进通道，
    // 每轮都不调用 poll_clipboard（模拟渲染暂停期消费停摆）。
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while session.clipboard_text.lock().len() < total && std::time::Instant::now() < deadline {
        session.process_output();
        std::thread::sleep(Duration::from_millis(10));
    }

    let queue = session.clipboard_text.lock();
    assert!(
        queue.len() <= MAX_CLIPBOARD_TEXT_QUEUE,
        "待上报队列必须有界（上限 {}，实得 {}），否则内存随上游发送速率增长",
        MAX_CLIPBOARD_TEXT_QUEUE,
        queue.len(),
    );
    assert!(
        !queue.is_empty(),
        "过载后队列不得为空：丢最旧也仍须留住较新的那批"
    );
    // 队列里是解码后的正文（base64 只用于线路传输），不是载荷本身。
    assert_eq!(
        queue.back().map(String::as_str),
        Some(format!("clip-{}", total - 1).as_str()),
        "丢的必须是较旧的那批，队首才是等待最久的一条",
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
fn session_write_after_exit_returns_error() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    // 置会话的退出标志，使 `write()` 在调用 PTY 前先检查它。
    session.exited_flag().store(true, Ordering::Release);
    let result = session.write(b"test");
    assert!(
        result.is_err(),
        "write after exit must return error, got Ok"
    );
}

#[test]
fn read_error_action_classifies_errno() {
    // EINTR/EAGAIN 是瞬时错误，必须保持读取循环存活。
    assert_eq!(
        read_error_action(Some(libc::EINTR)),
        ReaderErrorAction::Retry
    );
    assert_eq!(
        read_error_action(Some(libc::EAGAIN)),
        ReaderErrorAction::Retry
    );
    // EIO 表示 PTY 从端已关闭（Linux PTY 上的 EOF）——停止。
    assert_eq!(read_error_action(Some(libc::EIO)), ReaderErrorAction::Stop);
    // 其他 errno（或无 errno）同样停止读取。
    assert_eq!(
        read_error_action(Some(libc::EPIPE)),
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
