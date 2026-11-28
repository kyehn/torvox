use super::super::query::session_handle;
use super::super::{
    Env, EnvUnowned, Event, FLOOD_SHED_WARN_THROTTLE, JClass, JString,
    MAX_FLOOD_SHED_ANSWERS_PER_FRAME, MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION,
    ThrowRuntimeExAndFailure, cancel_request, expire_stale_requests, flood_shed_warn_throttled,
    jlong, register_request, take_request_slot, unanswered_clipboard_reads,
};

use crate::terminal::session::SessionError;

/// 一条待写回的剪贴板应答：(会话 id, 选择器名, 应答文本)。
type ClipboardAnswerJob = (u64, String, Option<String>);

/// 已答复的剪贴板应答队列：宿主答复后入队，由唯一的应答写线程消费。
///
/// 一条读请求一个线程的做法每次请求要付一次线程创建（实测 26.5µs/次；洪泛时
/// 每帧 8 条即 212µs 渲染线程时间，每秒 480 个线程），改为单线程消费后请求数
/// 不再转化为线程数。
static CLIPBOARD_ANSWER_JOBS: std::sync::OnceLock<std::sync::mpsc::Sender<ClipboardAnswerJob>> =
    std::sync::OnceLock::new();

/// 应答写线程的清扫间隔：超期未答复的槽位在这个节拍上被清掉。
///
/// 取 250ms：超期判定本身以 2s 为界（`CLIPBOARD_ANSWER_TIMEOUT`），清扫间隔只决定
/// 判定精度——远端等的是应答本身，不是槽位消失的时刻。但精度仍须够细：槽位驻留
/// 期间占着本会话的「未作答」额度，驻留越久，后续请求越早被洪泛削减。
///
/// 清扫 MUST NOT 只在队列空闲时做：宿主只答了部分请求时队列一直非空，只在空闲时
/// 清扫会让那部分始终不应答的槽位永远驻留。
const CLIPBOARD_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

// ── OSC 52 剪贴板 ───────────────────────────────────────────────────
/// 处理待处理的 OSC 52 剪贴板读取请求（在注册表/会话锁之外）：登记一次性应答槽位、
/// 推入事件，并启动短生命周期的应答线程把宿主应用的答复写回 PTY。
///
/// VT 线程绝不能阻塞等待宿主应用，且 `clipboardResult` 可能在任意线程到达。
pub(super) fn handle_clipboard_reads(
    pending_clipboard_reads: Vec<(u64, String)>,
    pending_events: &mut Vec<Event>,
) {
    let mut flood_shed_answers = 0usize;
    // 洪泛削减的帧级墙钟预算：削减回写要同步写 PTY，而子进程不读 stdin 时每次写入
    // 都会等满各自的排空期限（`WRITE_BACK_TIMEOUT`，50ms）。每条各拿满额时，最坏耗时
    // 是「每帧条数上限 × 单次期限」= 16 × 50ms = 800ms——一帧就把渲染线程按住远超帧
    // 预算，看门狗会误判渲染线程已死并关掉会话。
    //
    // 故本帧的削减回写共享一份预算：每条只拿**剩余**部分，预算用尽即止。答不完的
    // 部分退回会话队列，由逐帧收割补齐——与逐帧收割同一条墙钟口径（见
    // `Session::drain_pty_write_back`）。预算只在第一条削减回写时起算：此前的登记
    // 工作不属于削减回写，把它算进预算会让负载下的整帧削减全被推迟，遥遥无期。
    let mut flood_shed_deadline: Option<std::time::Instant> = None;
    // 每会话「未作答计数」的帧内缓存：`unanswered_clipboard_reads` 要锁
    // `REQUEST_REGISTRY` 并遍历全表，而一个 8KB 输出块可含数百条读请求、一帧十块即
    // 数千次全表扫描——洪泛削减本是为挡住病态远端，不能自己先成为瓶颈。
    // 只缓存**已超限**的会话：削减分支不注册槽位，故其计数在本帧内不变；未超限的
    // 会话走正常分支会注册槽位使计数 +1，缓存会失准，故逐条重算（每会话至多
    // `MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION` 条走这条路径）。
    let mut shed_counts: Vec<(u64, usize)> = Vec::new();
    // 帧级上限/预算挡下的请求必须退回会话队列：远端在等应答，丢弃即永久挂住。
    let mut deferred: Vec<(u64, String)> = Vec::new();
    // 首次登记读请求时启动应答写线程：答复从宿主 UI 线程来，写回 PTY 要等主端
    // 写入锁，既不能在那里做，也不能在渲染线程上做。
    if !pending_clipboard_reads.is_empty() {
        ensure_answer_writer();
    }
    for (session_id, selection) in pending_clipboard_reads {
        let unanswered =
            if let Some((_, count)) = shed_counts.iter().find(|(id, _)| *id == session_id) {
                *count
            } else {
                let count = unanswered_clipboard_reads(session_id);
                if count >= MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION {
                    shed_counts.push((session_id, count));
                }
                count
            };
        if unanswered >= MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION {
            // 洪泛削减：就地显式作答空串，不注册、不推事件、不起应答线程。
            // 槽位、线程与保护事件三者都不再增长，Exit 永远有位置。
            //
            // 但「就地作答」本身要写 PTY（base64 + 带期限的写入，子进程不读 stdin
            // 时每次可等到 50ms），而这段代码跑在渲染线程上：一次 8KB 输出块里能塞
            // 约 900 条读请求，一帧十块即约 9000 次带锁写入，光等待就足以吃掉整帧
            // 预算。故削减回写也按帧封顶——答不完的请求退回队列，下一帧再答。
            if flood_shed_answers >= MAX_FLOOD_SHED_ANSWERS_PER_FRAME {
                deferred.push((session_id, selection));
                continue;
            }
            // 帧级预算用尽即止：不再回写，也不占条数额度。
            let deadline = *flood_shed_deadline.get_or_insert_with(|| {
                std::time::Instant::now() + crate::terminal::pty::WRITE_BACK_TIMEOUT
            });
            let budget = deadline.saturating_duration_since(std::time::Instant::now());
            if budget.is_zero() {
                deferred.push((session_id, selection));
                continue;
            }
            flood_shed_answers += 1;
            flood_shed_warn_throttled(session_id, unanswered, &FLOOD_SHED_WARN_THROTTLE);
            // 读锁而非写锁：本分支只查表，而随后的 `answer_clipboard_read` 要经
            // `PtyMaster::write_with_timeout_counted` 等 `pty_write_order` 互斥量——
            // 该锁可能被用户输入写入按住、持有时长**无上界**。持注册表**写**锁等它，
            // 会把 `pollEvent`/`switchSession`/`destroySession` 连同其余会话的取句柄
            // 一起冻在锁外；本文件余下部分与 `pump_sessions` 一律在注册表锁外做会话工作。
            // 取到 `Arc` 后 guard 即随本句结束而释放，写 PTY 发生在锁外。
            //
            // 应答自身的取锁与排空已受 `WRITE_BACK_TIMEOUT` 约束（见
            // `Session::answer_clipboard_read`），故此处等锁最坏也只有一帧级期限；
            // 但注册表**写**锁会把这段等待放大成全局停顿，仍不可取。
            if let Some(session) = session_handle(session_id) {
                let mut session = session.lock();
                if let Err(error) = session.answer_clipboard_read(&selection, "", budget) {
                    log_clipboard_write_back_failure(&error, &CLIPBOARD_WRITE_BACK_FAILURE_LOG);
                }
            }
            continue;
        }
        let request_id = register_request(session_id, selection.clone());
        pending_events.push(Event::ClipboardRead {
            session_id,
            request_id,
            selection: selection.clone(),
        });
        if session_handle(session_id).is_none() {
            // 会话已消失：丢弃槽位，避免它白占本会话的「未作答」额度。
            cancel_request(session_id, request_id);
        }
    }
    // 退回队列：同一会话的退回请求一次灌回并只裁一次。逐条灌回时每次 push 后都要
    // 裁，被裁掉的会是刚灌回的那条——与到达路径「丢最旧」的口径相反。
    let mut deferred_by_session: Vec<(u64, Vec<String>)> = Vec::new();
    for (session_id, selection) in deferred {
        match deferred_by_session
            .iter_mut()
            .find(|(id, _)| *id == session_id)
        {
            Some((_, selections)) => selections.push(selection),
            None => deferred_by_session.push((session_id, vec![selection])),
        }
    }
    for (session_id, selections) in deferred_by_session {
        if let Some(session) = session_handle(session_id) {
            session.lock().requeue_clipboard_reads(selections);
        }
    }
}

#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clipboardResult<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    text: JString<'local>,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        clipboard_result_inner(env, session_id, request_id, text);
    });
}

/// 剪贴板应答回写失败是否属于「会话已结束」这一正常终态。
///
/// 会话已退出（`Closed`），或 PTY 已随子进程关闭（写端收到 `EIO`/`EPIPE`）时，
/// 远端询问的会话已不存在，应答无处可去——与 `feedPty` 的判定同口径。
fn clipboard_write_back_session_gone(error: &SessionError) -> bool {
    match error {
        SessionError::Closed => true,
        SessionError::Io(io_error) => matches!(
            io_error.raw_os_error().map(nix::errno::Errno::from_raw),
            Some(nix::errno::Errno::EIO | nix::errno::Errno::EPIPE)
        ),
        _ => false,
    }
}

/// 剪贴板应答回写失败告警的限频闸门（窗口内累计失败次数）。
///
/// 洪泛削减时每帧最多写 16 次应答，逐次记 warn 会把 logcat 刷成洪水；逐帧诊断
/// 按窗口汇总是 `logcat-hygiene` 的要求。失败本身仍以 warn 记——升级信号不被
/// 限频压住，窗口内的失败次数也照常累计。
static CLIPBOARD_WRITE_BACK_FAILURE_LOG: crate::slow_log::CountedLogThrottle =
    crate::slow_log::CountedLogThrottle::new();

/// 剪贴板应答回写失败的分级：会话已结束是正常退化路径，记 debug；其余失败按
/// 窗口汇总后记 warn。
fn log_clipboard_write_back_failure(
    error: &SessionError,
    failure_log: &crate::slow_log::CountedLogThrottle,
) {
    if clipboard_write_back_session_gone(error) {
        log::debug!("osc52: 会话已结束，丢弃剪贴板应答");
        return;
    }
    if let Some(failures_in_window) = failure_log.record() {
        log::warn!(
            "osc52: read answer write-back failed {failures_in_window} times in the last window: {error}"
        );
    }
}

pub(super) fn clipboard_result_inner<'local>(
    env: &mut Env<'local>,
    session_id: jlong,
    request_id: jlong,
    text: JString<'local>,
) {
    let session_id = session_id as u64;
    let request_id = request_id as u64;

    // null 是宿主读取失败，解码失败同理：都不能退化成空串，
    // 否则远端把「读不到剪贴板」当成「用户清空了剪贴板」（见 write_back_clipboard_answer）。
    let answer = if text.is_null() {
        None
    } else {
        match text.try_to_string(env) {
            Ok(decoded) => Some(decoded),
            Err(error) => {
                log::error!("osc52: clipboard answer decode failed: {error}");
                None
            }
        }
    };
    deliver_host_answer(session_id, request_id, answer);
}

/// 交付一条宿主答复：取走待答槽位并把应答交给应答写线程。
///
/// 槽位在即答复（迟到的答复即无操作）：槽位持有选择器名与登记时刻，取走即
/// 释放该会话的「未作答」额度。解码在取走槽位之前完成，故注册表锁从不跨过
/// JNI 调用（`try_to_string` 可能触发 JVM GC safepoint）。
///
/// 写回应答要等主端写入锁（最坏 `WRITE_BACK_TIMEOUT`），故不能在答复到达的线程
/// （宿主 UI 线程）上做，也不能在渲染线程上做。
fn deliver_host_answer(session_id: u64, request_id: u64, answer: Option<String>) -> bool {
    let Some(slot) = take_request_slot(session_id, request_id) else {
        return false;
    };
    let Some(sender) = answer_job_sender() else {
        return false;
    };
    let _ = sender.send((session_id, slot.selection, answer));
    true
}

/// 应答队列的发送端；写线程尚未启动时为 `None`（此时不会有待答请求）。
fn answer_job_sender() -> Option<std::sync::mpsc::Sender<ClipboardAnswerJob>> {
    CLIPBOARD_ANSWER_JOBS.get().cloned()
}

/// 启动应答写线程（首次登记读请求时调用），返回应答队列的发送端。
///
/// 进程内只起一个写线程：请求数不再转化为线程数。
fn ensure_answer_writer() -> std::sync::mpsc::Sender<ClipboardAnswerJob> {
    CLIPBOARD_ANSWER_JOBS
        .get_or_init(|| {
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || answer_writer_loop(receiver));
            sender
        })
        .clone()
}

/// 应答写线程的主循环：消费已答复的读请求并写回 PTY；周期性清掉超期槽位。
fn answer_writer_loop(receiver: std::sync::mpsc::Receiver<ClipboardAnswerJob>) {
    let mut last_sweep = std::time::Instant::now();
    loop {
        let wait = CLIPBOARD_SWEEP_INTERVAL.saturating_sub(last_sweep.elapsed());
        match receiver.recv_timeout(wait) {
            Ok(job) => {
                // 单条应答的 panic 不得带走整条应答管道：写线程是进程内唯一的，
                // 它一死，之后所有宿主答复都只入队不写回（远端永远等不到应答）。
                // panic 钩子已记下现场，这里只把该条记为失败并继续服务下一条。
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    write_back_clipboard_answer(job);
                }));
                if outcome.is_err() {
                    log::error!("osc52: clipboard answer write-back panicked");
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // 清扫与队列是否空闲无关：宿主只答部分请求时队列一直非空，只在空闲时清扫
        // 会让那部分始终不应答的槽位永远驻留，白占「未作答」额度。
        if last_sweep.elapsed() >= CLIPBOARD_SWEEP_INTERVAL {
            expire_stale_requests();
            last_sweep = std::time::Instant::now();
        }
    }
}

/// 把一条已答复的剪贴板读请求写回对应会话的 PTY。
fn write_back_clipboard_answer(job: ClipboardAnswerJob) {
    let (session_id, selection, answer) = job;
    // 空答复（宿主读取失败）不写回：空串等于告诉远端「用户剪贴板是空的」，
    // tmux/ssh 会粘出空白且像是用户清空了剪贴板。
    let Some(text) = answer else { return };
    let Some(session) = session_handle(session_id) else {
        // 会话已销毁：应答无处可去。与写失败同口径记 debug（见 `logcat-hygiene`
        // 的「会话已结束后的剪贴板应答回写」）。
        log::debug!("osc52: 会话已结束，丢弃剪贴板应答");
        return;
    };
    let mut session = session.lock();
    if let Err(error) =
        session.answer_clipboard_read(&selection, &text, crate::terminal::pty::WRITE_BACK_TIMEOUT)
    {
        log_clipboard_write_back_failure(&error, &CLIPBOARD_WRITE_BACK_FAILURE_LOG);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        Event, SessionError, clipboard_write_back_session_gone, deliver_host_answer,
        handle_clipboard_reads, log_clipboard_write_back_failure,
    };
    use crate::terminal::mock_pty::MockPtyHandle;

    /// 宿主答复 "hello" 后的 OSC 52 应答帧（base64 编码负载）。
    const ANSWER_FRAME: &[u8] = b"\x1b]52;c;aGVsbG8=\x07";

    /// 登记一个会话并走完「读请求登记 → 事件产出」，返回请求 id 与 PTY 写入面。
    ///
    /// 两条用例的这段铺陈完全相同：各写一份就是一处真重复（jscpd 命中）。
    fn register_read_request(session_id: u64) -> (u64, MockPtyHandle) {
        let (session, handle) = crate::terminal::mock_pty::mock_arc_session();
        super::super::super::wlock_session_registry().insert(
            session_id,
            super::super::super::SessionEntry::new(
                Arc::clone(&session),
                Arc::new(super::super::super::RenderWake::default()),
            ),
        );
        let mut pending_events = Vec::new();
        handle_clipboard_reads(vec![(session_id, "c".to_string())], &mut pending_events);
        let request_id = match pending_events.as_slice() {
            [
                Event::ClipboardRead {
                    session_id: event_session,
                    request_id,
                    ..
                },
            ] => {
                assert_eq!(*event_session, session_id, "事件须归属登记的会话");
                *request_id
            }
            other => panic!("读请求须产出且只产出一条读事件，实际 {other:?}"),
        };
        (request_id, handle)
    }

    /// 等应答帧落到 PTY（最多 3s）：写回在应答写线程上异步发生。
    ///
    /// 返回落盘的全部字节：调用方要数应答帧的份数。
    fn wait_for_answer_frame(handle: &MockPtyHandle) -> Vec<u8> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let written = handle.written();
            if written
                .windows(ANSWER_FRAME.len())
                .any(|window| window == ANSWER_FRAME)
            {
                return written;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "答复必须在期限内写回 PTY，实际写回 {written:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// 从会话注册表移除测试会话。
    fn remove_session(session_id: u64) {
        super::super::super::wlock_session_registry().remove(&session_id);
    }

    /// 一条读请求被宿主答复后，应答必须回到 PTY——整条链（登记 → 答复 → 写回）
    /// 在共享的应答写线程上跑通。
    ///
    /// 判别力：写回路径若断在「答复入队」或「写线程消费」任一环，PTY 上就没有应答，
    /// 远端会一直等下去；而只断言事件已产出则完全看不出后半段是否通。
    #[test]
    fn an_answered_read_request_is_written_back_to_the_pty() {
        let session_id = 0x0C11_0BA5_u64;
        let (request_id, handle) = register_read_request(session_id);

        assert!(
            deliver_host_answer(session_id, request_id, Some("hello".to_string())),
            "答复须命中已登记的请求"
        );
        wait_for_answer_frame(&handle);

        remove_session(session_id);
    }

    /// 重复答复同一请求只有第一次生效：槽位取走即消失，第二次是无操作。
    ///
    /// 判别力：槽位若在答复后仍留着，重复答复会把同一条应答写回两次，远端于是
    /// 收到两份 OSC 52 应答。
    #[test]
    fn a_repeated_answer_is_a_no_op() {
        let session_id = 0x0C11_0BA6_u64;
        let (request_id, handle) = register_read_request(session_id);

        assert!(deliver_host_answer(
            session_id,
            request_id,
            Some("hello".to_string())
        ));
        assert!(
            !deliver_host_answer(session_id, request_id, Some("hello".to_string())),
            "同一请求的第二次答复须是无操作"
        );

        let written = wait_for_answer_frame(&handle);
        // 只写回一次：应答帧不重复。
        assert_eq!(
            written
                .windows(ANSWER_FRAME.len())
                .filter(|window| *window == ANSWER_FRAME)
                .count(),
            1,
            "同一次请求只许写回一条应答"
        );

        remove_session(session_id);
    }

    /// 洪泛削减时每帧最多写 16 次应答，逐次记 warn 会把 logcat 刷成洪水。
    /// 失败按窗口汇总：首条立即交出窗口计数，窗口内的失败攒着下次一并交出。
    #[test]
    fn write_back_failures_are_aggregated_per_window() {
        let failure_log = crate::slow_log::CountedLogThrottle::new();
        let no_space = std::io::Error::from_raw_os_error(nix::errno::Errno::ENOSPC as i32);
        let error = SessionError::Io(no_space);

        log_clipboard_write_back_failure(&error, &failure_log);
        assert_eq!(
            failure_log.pending_count(),
            0,
            "首条失败须立即交出窗口计数，不得被限频吞掉"
        );

        log_clipboard_write_back_failure(&error, &failure_log);
        log_clipboard_write_back_failure(&error, &failure_log);
        assert_eq!(
            failure_log.pending_count(),
            2,
            "窗口内的失败须攒着下次一并交出，不得丢弃"
        );
    }

    /// 会话已结束是正常退化路径，不计入失败窗口：否则每次会话收尾都会留下一条
    /// 无意义的失败计数。
    #[test]
    fn a_gone_session_is_not_counted_as_a_write_back_failure() {
        let failure_log = crate::slow_log::CountedLogThrottle::new();
        log_clipboard_write_back_failure(&SessionError::Closed, &failure_log);
        log_clipboard_write_back_failure(&SessionError::Closed, &failure_log);
        assert_eq!(failure_log.pending_count(), 0, "会话已结束不得计入失败窗口");
    }

    #[test]
    fn exited_session_write_back_is_not_a_failure() {
        assert!(clipboard_write_back_session_gone(&SessionError::Closed));
    }

    #[test]
    fn closed_pty_write_back_is_not_a_failure() {
        let broken_pipe = std::io::Error::from_raw_os_error(nix::errno::Errno::EPIPE as i32);
        assert!(clipboard_write_back_session_gone(&SessionError::Io(
            broken_pipe
        )));
    }

    #[test]
    fn real_write_failure_stays_a_failure() {
        let no_space = std::io::Error::from_raw_os_error(nix::errno::Errno::ENOSPC as i32);
        assert!(!clipboard_write_back_session_gone(&SessionError::Io(
            no_space
        )));
    }
}
