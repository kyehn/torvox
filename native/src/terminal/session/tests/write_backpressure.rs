use super::super::output::{
    MAX_PENDING_PTY_WRITES, MAX_PTY_WRITES_PER_FRAME, drop_excess_pty_responses,
};
use super::super::*;
use super::hold_pty_write_lock;

use std::time::Duration;

/// 回归护栏：PTY 写入不得在会话锁内阻塞。
///
/// 子进程长时间不读 stdin 时 `write_all` 最多等 `WRITE_DRAIN_TIMEOUT`（5s），
/// 而会话锁每帧都被 `render_inner` 与 `poll_event` 取得——写入一旦持锁，
/// 整台终端（渲染、事件收割、其它会话）就一起停摆。本测试用只到第一个字节就
/// 让写入线程挂起的替身（真实 `write_all` 到此返回 `Ok(1)`，不进入等待分支），
/// 断言会话锁在这段时间内仍可取得。
#[test]
fn pty_write_does_not_hold_the_session_lock() {
    use crate::terminal::mock_pty::{MockPty, WriteGate};
    use std::time::Duration;

    // 写入挂起直到本测试放行：模拟真实 `write_all` 在主端缓冲满时的等待分支。
    // 用共享的 [`MockPty`] + [`WriteGate`]：同文件其余用例走的是同一条闸，自造一份
    // PTY 替身会让「写入如何被挂起」出现两种口径（一份靠 sleep 轮询，一份靠闸）。
    let gate = WriteGate::new();
    let (mut pty, _handle) = MockPty::new(24, 80);
    pty.gate_writes(Arc::clone(&gate));
    let session = Arc::new(std::sync::Mutex::new(
        Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed"),
    ));
    let writer = session.lock().expect("session lock").pty_master();

    let writer_thread = std::thread::spawn(move || writer.write(b"x"));
    // 写入线程已进入 `Pty::write` 并挂起在等待分支。等待有期限：挂死的用例比判红的
    // 更难排查，写入若根本没进临界区，本用例应判红而不是永远挂着。
    const ENTER_BUDGET: Duration = Duration::from_secs(5);
    let entered_in_time = gate.wait_until_entered_with_timeout(ENTER_BUDGET);

    // 关键断言：写入挂起期间会话锁仍可取得。锁被占用即代表写入持锁——
    // 渲染与事件收割每帧都取这把锁，阻塞写入会把整台终端冻住。
    let lock_result = session.try_lock();
    // 先放行再断言：任何分支下写入线程都会被放出来，不会被挂在闸里。
    gate.release();
    let write_result = writer_thread.join().expect("writer thread panicked");
    if !entered_in_time {
        panic!("写入线程未在期限内进入 PTY 写入，本测试无判别力");
    }
    {
        let locked = lock_result.expect("PTY 写入不得持有会话锁：渲染与事件收割每帧都取这把锁");
        assert_eq!(locked.terminal().rows(), 24, "取得的必须是本会话");
    }
    // 放行后写入照常完成（锁外写入的字节序与结果语义不变）。
    write_result.expect("放行后写入必须成功");
    assert_eq!(session.lock().expect("session lock").terminal().rows(), 24);
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
    let (mut plain, plain_handle) = crate::terminal::mock_pty::mock_session();
    plain.inject_output_for_test(queries.as_bytes());
    plain.process_output();
    plain.terminal_mut().flush();
    for _ in 0..=query_count {
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
    for _ in 0..=query_count {
        session.process_output();
    }
    assert!(session.pty_write_back_queue.is_empty(), "队列须排空");
    assert_eq!(
        handle.written().len(),
        expected_total,
        "受阻后写出的总字节数须与畅通时一致（{expected_total}），既不重发也不丢"
    );
}

#[test]
fn vt_write_back_is_capped_per_frame_and_carries_over() {
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
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
    // 上面那次 `process_output` 也可能已经写走一整批，故只能量**本次**调用的增量；
    // 直接对累计值断言，会把前一帧的 16 条算进本帧（实测 32 > 16 的假红）。
    let before_this_frame = written_responses();
    session.process_output();
    let this_frame = written_responses() - before_this_frame;
    assert!(
        this_frame <= MAX_PTY_WRITES_PER_FRAME,
        "单帧写出的条数须有上界，实测 {this_frame}"
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

/// 回归钉：VT 应答队列溢出的告警必须按窗口汇总，不得逐帧刷屏。
///
/// 子进程不读 stdin 且持续查询设备状态时，队列溢出逐帧命中。旧实现每帧记一行
/// error 级日志，几秒内就能把 logcat 刷成洪水，把真正的异常淹没——`logcat-hygiene`
/// 要求逐帧诊断以窗口汇总，且窗口内的丢弃条数不得因限频而丢失。
#[test]
fn vt_response_overflow_is_reported_once_per_window_with_the_count() {
    let drop_log = crate::slow_log::CountedLogThrottle::new();
    let mut queue: std::collections::VecDeque<Vec<u8>> = std::collections::VecDeque::new();
    for _ in 0..(MAX_PENDING_PTY_WRITES + 5) {
        queue.push_back(vec![0u8]);
    }
    assert_eq!(
        drop_excess_pty_responses(&mut queue, &drop_log),
        Some(5),
        "首条必须可见，且带上本窗口丢弃的条数"
    );
    assert_eq!(
        queue.len(),
        MAX_PENDING_PTY_WRITES,
        "队列须裁到上限，超出的应答按丢最旧处理"
    );

    // 队列此刻正好在上限上：再塞三条即溢出三条。
    for _ in 0..3 {
        queue.push_back(vec![0u8]);
    }
    assert_eq!(
        drop_excess_pty_responses(&mut queue, &drop_log),
        None,
        "窗口内的再次溢出不得再输出一行"
    );
    assert_eq!(
        drop_log.pending_count(),
        3,
        "窗口内丢弃的条数须攒着下次一并交出，不得丢弃"
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
    pty.gate_writes(Arc::clone(&gate));
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
        let master = master;
        std::thread::spawn(move || {
            let _ = sender.send((master.child_pid(), master.resize(30, 100)));
        })
    };
    let reply = receiver.recv_timeout(READ_BUDGET);

    // 先收尾再断言：任何情况下都不把写入线程留在闸里。
    gate.release();
    writer.join().expect("writer thread panicked");
    reader.join().expect("reader thread panicked");

    let (pid, resize_result) = reply.expect("只读路径不得取主端互斥量：此刻写入正持锁");
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

    let (session, _handle) = crate::terminal::mock_pty::mock_arc_session();

    // 用户输入持写入锁停住（等价于 `feedPty` 卡在主端排空上），直到本用例放行。
    let hold = hold_pty_write_lock(&session, READ_BUDGET);

    // 收割在另一个线程里做：取不到写入锁的实现会一直等下去，直接在本线程调用
    // 会把用例挂死而不是判红——挂死的用例比判红的更难排查。
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut session = session.lock();
        session
            .pty_write_back_queue
            .push_back(b"\x1b[1;1R".to_vec());
        session.drain_pty_write_back();
        let _ = sender.send(session.pty_write_back_queue.len());
    });
    let reply = receiver.recv_timeout(READ_BUDGET);

    // 放行用户写入并回收其线程：持锁线程 panic 即判红（守卫析构只是兜底，不判红）。
    hold.release();
    worker.join().expect("收割线程 panicked");

    let pending = reply.expect("会话锁不得在用户输入持写入锁时被按住满排空上限");
    assert_eq!(
        pending, 1,
        "等不到写入锁时应答须整条退回队列下帧续写，不得丢弃"
    );
}

/// 回归钉：逐帧应答续写的**整段**预算（等锁 + 写入）不得翻倍。
///
/// [`PtyMaster::write_with_timeout_counted`](super::PtyMaster::write_with_timeout_counted)
/// 先等主端写入锁、再把**同一份**期限交给 `write_all`：两段各用满整份期限时最坏耗时
/// 是 `2 × drain_timeout`，而调用方（[`Session::drain_pty_write_back`]）的约定是
/// 「整段持会话锁以墙钟为界」。于是一次锁争用就能把逐帧取同一把锁的渲染与看门狗
/// 按住双倍时间。
///
/// 复现是确定的：用户写入持锁 200ms（等价于 `feedPty` 卡在主端排空上），随后以
/// 500ms 预算续写一条应答——修复前耗时约 700ms，修复后约 500ms（等锁吃掉的
/// 200ms 从写入份额里扣掉）。
#[test]
fn write_back_budget_covers_lock_wait_and_write_as_one_whole() {
    use std::sync::mpsc;

    const LOCK_HOLD: Duration = Duration::from_millis(200);
    const BUDGET: Duration = Duration::from_millis(500);
    /// 调度余量：只吸收线程唤醒与 sleep 的抖动，不得宽到让「翻倍」蒙混过关。
    const SLACK: Duration = Duration::from_millis(100);

    let (mut pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    // 写出 1 字节后恒返回 EAGAIN：`write_all` 于是走到「等主端排空」分支，把
    // 剩余期限睡满——正是逐帧应答在主端满时的真实形态。
    pty.write_partial_then_block(1);
    let session = Arc::new(Mutex::new(
        Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed"),
    ));

    // 用户输入持写入锁 200ms。
    let (locked_sender, locked_receiver) = mpsc::channel();
    let holder = {
        let session = Arc::clone(&session);
        std::thread::spawn(move || {
            let pty_master = session.lock().pty_master();
            let _order = pty_master.pty_write_order();
            locked_sender.send(()).expect("已持锁");
            std::thread::sleep(LOCK_HOLD);
        })
    };
    locked_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("用户写入须在期限内取到主端写入锁");

    let started = std::time::Instant::now();
    let (written, result) = session
        .lock()
        .pty_master()
        .write_with_timeout_counted(b"\x1b[1;1R", BUDGET)
        .expect("预算须足以取到写入锁");
    let elapsed = started.elapsed();

    holder.join().expect("持锁线程 panicked");

    assert!(
        result.is_err(),
        "主端恒满时写入必须如实报错，不得伪装成成功"
    );
    assert_eq!(written, 1, "已写出的字节必须如实上报");
    assert_eq!(handle.written(), b"\x1b", "首字节确已落到主端");
    assert!(
        elapsed <= BUDGET + SLACK,
        "等锁与写入必须共用一份预算：实际耗时 {elapsed:?}，上限 {:?}",
        BUDGET + SLACK
    );
}

/// VT 命令通道满时，输出收割必须停手并把整块留到下一帧重试，而不是丢字节。
///
/// 判别力：旧实现每块都 `try_send`，失败即丢并逐块记日志——一次输出高峰实测
/// 丢两万余块（终端状态与 PTY 永久分叉）并刷出两万余行日志。本用例用容量 1 的
/// 通道确定性地复现「命令通道满」，断言未送达的块原样滞留、顺序不乱、字节不丢。
#[test]
fn full_command_channel_backpressures_output_instead_of_dropping() {
    let (mut session, _handle) = crate::terminal::mock_pty::mock_session();
    // 容量 1 且无人接收：第一块占满槽位，第二块必然入队失败。
    let (command_tx, command_rx) = flume::bounded(1);
    session.terminal.cmd_tx = command_tx;
    session.inject_output_for_test(b"first\n");
    session.inject_output_for_test(b"second\n");

    assert!(session.poll_pty_output(10), "首块须已进入命令通道");
    assert_eq!(
        session.pending_pty_output.as_deref(),
        Some(b"second\r\n".as_slice()),
        "未送达的块必须原样滞留（LF 已按 PTY 语义成形为 CRLF），不得丢弃"
    );
    match command_rx.try_recv() {
        Ok(crate::terminal::ghostty_terminal::Command::Write(bytes)) => {
            assert_eq!(bytes, b"first\r\n");
        }
        Ok(_) => panic!("命令通道内应是首块 Write"),
        Err(error) => panic!("命令通道内应有首块：{error}"),
    }

    // 通道空出后下一帧必须先补投滞留块，再取新数据。
    session.poll_pty_output(10);
    assert!(
        session.pending_pty_output.is_none(),
        "通道腾空后滞留块必须在本帧送达"
    );
    match command_rx.try_recv() {
        Ok(crate::terminal::ghostty_terminal::Command::Write(bytes)) => {
            assert_eq!(
                bytes, b"second\r\n",
                "补投的必须是原块，不得重复成形（否则 CR 翻倍）"
            );
        }
        Ok(_) => panic!("命令通道内应是滞留块 Write"),
        Err(error) => panic!("命令通道内应有滞留块：{error}"),
    }
}

/// PTY 投递忙标记：卡在背压里时非零，投递完成即清零。
///
/// 背压（通道满 → `send` 阻塞）是「输出已读出却交不出去」的唯一可观测信号：
/// 读取线程空闲时阻塞在 `read()` 上，与挂起在进程内不可区分，故不能拿
/// 「读取线程没有进展」当挂起证据。
#[test]
fn pty_send_busy_marker_tracks_backpressure() {
    use super::super::spawn::send_pty_output;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    let (output_tx, output_rx) = flume::bounded::<Vec<u8>>(1);
    // 占满槽位：下一段输出必然卡在投递里。
    output_tx.send(vec![b'f']).expect("占位投递必须成功");

    let busy_marker = Arc::new(AtomicU64::new(0));
    let sender = output_tx.clone();
    let marker = Arc::clone(&busy_marker);
    let send_thread = std::thread::spawn(move || send_pty_output(&sender, &marker, vec![b's']));

    let deadline = Instant::now() + Duration::from_secs(5);
    while busy_marker.load(Ordering::Acquire) == 0 {
        assert!(
            Instant::now() < deadline,
            "投递卡在背压里时忙标记必须非零，否则挂起判定看不到「输出交不出去」"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    // 腾出槽位：投递随即完成，标记必须清零（未清零前宿主会把空闲读成挂起）。
    output_rx.recv().expect("槽位必须腾出");
    send_thread
        .join()
        .expect("投递线程必须退出")
        .expect("腾出槽位后投递必须成功");
    assert_eq!(
        busy_marker.load(Ordering::Acquire),
        0,
        "投递完成后忙标记必须清零，否则空闲会话会被判成挂起"
    );
}

/// 回归钉：子进程已退出时，待写的 VT 应答必须被丢弃，而不是逐帧重试。
///
/// 应答是请求-响应语义，读端（子进程）已消失后它们永远无人接收。旧实现不判
/// 退出态，失败的应答退回队首、下帧再写，每帧记一条 error——logcat 被刷成洪水，
/// 而「子进程已退出」本属正常退化。
#[test]
fn an_exited_session_drops_pending_vt_responses() {
    let (mut session, handle) = crate::terminal::mock_pty::mock_session();
    session
        .pty_write_back_queue
        .push_back(b"\x1b[1;1R".to_vec());
    session.exited_flag().store(true, Ordering::Release);

    session.drain_pty_write_back();

    assert!(
        session.pty_write_back_queue.is_empty(),
        "子进程已退出后不得再留着无人接收的应答"
    );
    assert!(handle.written().is_empty(), "子进程已退出后不得再往 PTY 写");
}

/// 回归钉：VT 应答写回失败必须按窗口汇总，不得逐帧刷 error。
///
/// 子进程不读 stdin 时主端缓冲会满，写入每帧都等满期限后失败；失败的应答退回
/// 队首，下帧重试——逐帧记一行 error 会把 logcat 刷成洪水，把真正的异常淹没。
#[test]
fn write_back_failures_are_aggregated_per_window() {
    let (mut pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    // 写出两个字节后主端即不再接收：写入等满期限后失败。
    pty.write_partial_then_block(2);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    session
        .pty_write_back_queue
        .push_back(b"\x1b[1;1R".to_vec());

    session.drain_pty_write_back();
    assert_eq!(
        session.pty_write_back_fail_log.pending_count(),
        0,
        "首次失败即放行，窗口计数 1 已随那一行交出"
    );
    assert!(
        !session.pty_write_back_queue.is_empty(),
        "失败的应答必须留在队首续写，不得丢弃"
    );

    session.drain_pty_write_back();
    assert_eq!(
        session.pty_write_back_fail_log.pending_count(),
        1,
        "窗口内的再次失败不得再输出一行，但次数须攒着下次交出"
    );
}
