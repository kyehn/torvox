use super::super::query::session_handle;
use super::super::*;

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
    // 每会话「未作答计数」的帧内缓存：`unanswered_clipboard_reads` 要锁
    // `REQUEST_REGISTRY` 并遍历全表，而一个 8KB 输出块可含数百条读请求、一帧十块即
    // 数千次全表扫描——洪泛削减本是为挡住病态远端，不能自己先成为瓶颈。
    // 只缓存**已超限**的会话：削减分支不注册槽位，故其计数在本帧内不变；未超限的
    // 会话走正常分支会注册槽位使计数 +1，缓存会失准，故逐条重算（每会话至多
    // `MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION` 条走这条路径）。
    let mut shed_counts: Vec<(u64, usize)> = Vec::new();
    for (session_id, selection) in pending_clipboard_reads {
        let unanswered = match shed_counts.iter().find(|(id, _)| *id == session_id) {
            Some((_, count)) => *count,
            None => {
                let count = unanswered_clipboard_reads(session_id);
                if count >= MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION {
                    shed_counts.push((session_id, count));
                }
                count
            }
        };
        if unanswered >= MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION {
            // 洪泛削减：就地显式作答空串，不注册、不推事件、不起应答线程。
            // 槽位、线程与保护事件三者都不再增长，Exit 永远有位置。
            //
            // 但「就地作答」本身要写 PTY（base64 + 带期限的写入，子进程不读 stdin
            // 时每次可等到 50ms），而这段代码跑在渲染线程上：一次 8KB 输出块里能塞
            // 约 900 条读请求，一帧十块即约 9000 次带锁写入，光等待就足以吃掉整帧
            // 预算。故削减回写也按帧封顶——被削减的请求本就是「拒绝」，不写即拒绝。
            if flood_shed_answers >= MAX_FLOOD_SHED_ANSWERS_PER_FRAME {
                continue;
            }
            flood_shed_answers += 1;
            flood_shed_warn_throttled(session_id, unanswered);
            // 读锁而非写锁：本分支只查表，而随后的 `answer_clipboard_read` 要经
            // `PtyMaster::write_with_timeout` 等 `pty_write_order` 互斥量——该锁可能被
            // 用户输入写入按住、持有时长**无上界**。持注册表**写**锁等它，会把
            // `pollEvent`/`switchSession`/`destroySession` 连同其余会话的取句柄一起
            // 冻在锁外；本文件余下部分与 `pump_sessions` 一律在注册表锁外做会话工作。
            // 取到 `Arc` 后 guard 即随本句结束而释放，写 PTY 发生在锁外。
            if let Some(session) = session_handle(session_id) {
                let mut session = session.lock();
                if let Err(error) = session.answer_clipboard_read(&selection, "") {
                    log::warn!("osc52: flood-shed read answer write-back failed: {error}");
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
                if let Err(error) = session.answer_clipboard_read(&selection, &text) {
                    log::warn!("osc52: clipboard read answer write-back failed: {error}");
                }
            });
        } else {
            // 应答线程启动前会话已消失：丢弃一次性槽位，避免 `REQUEST_REGISTRY` 泄漏 Sender。
            cancel_request(session_id, request_id);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clipboardResult<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    text: JString<'local>,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        clipboard_result_inner(env, _class, session_id, request_id, text)
    })
}

pub(super) fn clipboard_result_inner<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
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
