use super::super::output::MAX_CLIPBOARD_TEXT_QUEUE;
use super::super::*;
use super::hold_pty_write_lock;
use super::spawn_test_session;

use std::time::Duration;

/// OSC 52 读取应答以 `ESC ] 52 ; <selection> ; <base64> BEL` 写回 PTY。
#[test]
fn answer_clipboard_read_writes_esc52_reply() {
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
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
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
    session
        .answer_clipboard_read("c", "")
        .expect("answer must succeed");
    assert_eq!(handle.written(), b"\x1b]52;c;\x07");
}

/// 会话退出后写应答必须干净失败。
#[test]
fn answer_clipboard_read_after_exit_fails() {
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
    session.exited_flag().store(true, Ordering::Release);
    let result = session.answer_clipboard_read("c", "text");
    assert!(result.is_err(), "exited session must reject writes");
    assert!(handle.written().is_empty(), "nothing may reach the PTY");
}

/// 回归钉：OSC 52 读取应答的写入不得在用户输入占着主端写入锁时拖住会话锁。
///
/// 与 [`write_back_yields_the_frame_when_a_user_write_holds_the_master_lock`] 同源：
/// 那条钉的是逐帧收割（`drain_pty_write_back`），本条钉的是应答写入本身
/// （[`Session::answer_clipboard_read`]）。两条都持会话锁，且都与用户输入共用
/// `pty_write_order` 互斥量——用户输入走 5s 排空期限且每有部分写入就续额，
/// 持锁时长无上界。
///
/// 修复前应答走 `write_with_timeout`：取锁无期限、排空按进度续额。于是
/// 「整段持会话锁的时间以墙钟为界」在这条路上并不成立——一条应答就能把每帧取
/// 同一把锁的渲染与看门狗按住数秒，渲染停滞会让看门狗误判渲染线程已死并
/// 关掉会话（用户侧表现为「远端程序一读剪贴板，终端就卡死并被杀」）。
#[test]
fn clipboard_answer_yields_when_a_user_write_holds_the_master_lock() {
    use std::sync::mpsc;

    const READ_BUDGET: Duration = Duration::from_millis(500);

    let (session, handle) = crate::terminal::mock_pty::mock_arc_session();

    // 用户输入持写入锁停住（等价于 `feedPty` 卡在主端排空上），直到本用例放行。
    let hold = hold_pty_write_lock(&session, READ_BUDGET);

    // 应答在另一个线程里写：取不到写入锁的实现会一直等下去，直接在本线程调用
    // 会把用例挂死而不是判红——挂死的用例比判红的更难排查。
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn({
        let session = Arc::clone(&session);
        move || {
            let mut session = session.lock();
            let result = session.answer_clipboard_read("c", "hello");
            let _ = sender.send((result, session.pty_write_back_queue.len()));
        }
    });
    let reply = receiver.recv_timeout(READ_BUDGET);

    // 放行用户写入并回收其线程：持锁线程 panic 即判红（守卫析构只是兜底，不判红）。
    hold.release();
    worker.join().expect("应答线程 panicked");

    let (result, pending) = reply.expect("会话锁不得在用户输入持写入锁时被按住满排空上限");
    assert!(
        result.is_ok(),
        "等不到写入锁时应答须被接受（退回续写队列），实测 {result:?}"
    );
    assert_eq!(
        pending, 1,
        "等不到写入锁时应答须整条退回队列下帧续写，不得丢弃"
    );

    // 放行后逐帧收割把应答补齐：子应用正等它，丢了就永远等不到。
    session.lock().process_output();
    assert_eq!(
        handle.written(),
        b"\x1b]52;c;aGVsbG8=\x07",
        "应答须完整送达：截断在 base64 中间会让子应用等不到一个完整回答"
    );
}

/// 回归护栏：OSC 52 写入必须被收割，用户复制的内容才能进系统剪贴板。
///
/// 原实现把收割挂在 `flush_with_timeout(Duration::ZERO)` 成功分支上，而零期限
/// `recv_timeout` 实测 32 次 0 次命中 ack，故 OSC 52 事件永不上报
/// （`NativeBridgeSmokeTest > feedTerminal OSC52 write surfaces clipboard poll event`
/// 长期失败即由此而来）。本测试只依赖「投递 + 收割」，不依赖任何 flush 时序。
#[test]
fn osc52_write_is_harvested_without_relying_on_flush_ack() {
    use base64::Engine;

    let mut session = spawn_test_session();
    let marker = "harvest-probe";
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
    use base64::Engine;

    let mut session = spawn_test_session();
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
