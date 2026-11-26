use super::super::query::session_handle;
use super::super::{
    Env, EnvUnowned, Event, FLOOD_SHED_WARN_THROTTLE, JClass, JString,
    MAX_FLOOD_SHED_ANSWERS_PER_FRAME, MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION, REQUEST_REGISTRY,
    ThrowRuntimeExAndFailure, cancel_request, flood_shed_warn_throttled, jlong, register_request,
    unanswered_clipboard_reads, wait_for_clipboard_answer,
};

use crate::terminal::session::SessionError;

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
        let (request_id, rx) = register_request(session_id);
        pending_events.push(Event::ClipboardRead {
            session_id,
            request_id,
            selection: selection.clone(),
        });
        if let Some(session) = session_handle(session_id) {
            std::thread::spawn(move || {
                let answer = wait_for_clipboard_answer(rx);
                // 应答线程是槽位的唯一所有者：无论宿主是否作答（事件被队列淘汰、
                // UI 未处理、答复晚于截止时间），槽位至多存活 CLIPBOARD_ANSWER_TIMEOUT。
                // 否则远端反复 `\e]52;c;?` 会让本表无界增长。
                // 迟到的 clipboardResult 在此之后到达即为无操作（见 clipboard_result_inner）。
                cancel_request(session_id, request_id);
                let Some(text) = answer else { return };
                let mut session = session.lock();
                if let Err(error) = session.answer_clipboard_read(
                    &selection,
                    &text,
                    crate::terminal::pty::WRITE_BACK_TIMEOUT,
                ) {
                    log_clipboard_write_back_failure(&error, &CLIPBOARD_WRITE_BACK_FAILURE_LOG);
                }
            });
        } else {
            // 应答线程启动前会话已消失：丢弃一次性槽位，避免 `REQUEST_REGISTRY` 泄漏 Sender。
            cancel_request(session_id, request_id);
        }
    }
    // 倒序回灌：逐条 push_front 后，同一会话的退回请求仍按原序排在队首。
    for (session_id, selection) in deferred.into_iter().rev() {
        if let Some(session) = session_handle(session_id) {
            session.lock().requeue_clipboard_read(selection);
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

    // 先取出并立即释放 guard：`try_to_string` 可能触发 JVM GC safepoint，
    // guard 若跨过 JNI 调用会把锁持有时间拉长。
    let tx = REQUEST_REGISTRY.lock().remove(&(session_id, request_id));
    if let Some(tx) = tx {
        // null 是宿主读取失败，解码失败同理：都不能退化成空串，
        // 否则远端把「读不到剪贴板」当成「用户清空了剪贴板」（见 wait_for_clipboard_answer）。
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
        let _ = tx.send(answer);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SessionError, clipboard_write_back_session_gone, log_clipboard_write_back_failure,
    };

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
