use super::super::*;
use super::clipboard::handle_clipboard_reads;

// ── 会话推进泵 ──────────────────────────────────────────────────────
/// 推进全部会话的终端状态，并把本轮产生的事件推入事件队列。
///
/// 与 [`poll_event_inner`] 的步骤 2 分离，使 [`Java_terminal_emulator_bridge_NativeBridge_pumpSessions`]
/// 可在渲染暂停期间单独调用：此时渲染线程已停，`pollEvent` 无人调用，若会话也不再
/// 推进，有界输出通道会填满 → 读线程阻塞 → PTY 内核缓冲填满 → 后台作业停摆。
/// 本函数不消费事件队列，故后台期间产生的剪贴板、退出等事件仍在队列中，
/// 渲染恢复后由 `pollEvent` 正常派发。
pub(crate) fn pump_sessions() {
    // 步骤 1：轮询活跃会话的新事件。先收集事件，释放会话锁后再推入，
    // 以保持锁顺序 `SESSION_REGISTRY` → `Session` → `EVENT_QUEUE`。
    // 绝不在持有 `Session` 锁时锁 `EVENT_QUEUE`。
    let mut pending_events: Vec<Event> = Vec::new();
    // 本帧可上报 Exit 事件的会话（退出码已就绪者）及其退出结果与存活时长。
    // 退出码在会话锁内就地判定：只读一次 Mutex，不再有原先「先锁存上报标志、再忙等
    // 最多 100ms 取码」的两段式——那会在渲染线程上引入可见停顿，且超时只能上报
    // 伪造的 0。改为每帧问一次「退出了吗」，未就绪就下一帧再问，零阻塞。
    let mut pending_exits: Vec<(u64, crate::terminal::session::ReportedExit)> = Vec::new();
    // 剪贴板/退出的轮询对活跃会话与所有后台会话完全相同，共用同一实现。
    let collect_session_events =
        |session_id: u64,
         session: &mut Session,
         events: &mut Vec<Event>,
         exits: &mut Vec<(u64, crate::terminal::session::ReportedExit)>| {
            if let Some(text) = session.poll_clipboard() {
                events.push(Event::Clipboard { session_id, text });
            }
            // 退出码就绪才上报（`take_reported_exit` 内部同时锁存「已上报」），后台扫描
            // 分支走同一判定：既保证退出事件恰好一次，也保证报出的是真实退出码。
            if let Some(reported) = session.take_reported_exit() {
                exits.push((session_id, reported));
            }
        };
    let mut pending_clipboard_reads: Vec<(u64, String)> = Vec::new();
    let active_id = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
    // 采集本帧要处理的会话句柄：读锁内只克隆 Arc，不做任何会话工作。
    // 慢会话的 VT 解析/查询若顶着读锁跑，会把 setScrollOffset 等写锁饿死，
    // 并把单个会话的停顿放大成全局停顿。
    // 句柄是 Arc：锁外处理期间会话被销毁，也只是对着已死的 Arc 做无害功——
    // 事件都带 session_id（Kotlin 侧对未知会话本就是空操作/幂等回收），
    // take 系调用是锁存式的（恰好一次），不存在双重上报。
    let poll_batch: Vec<(u64, Arc<Mutex<Session>>, bool)> = {
        let registry = rlock_session_registry();
        let mut batch = Vec::new();
        if active_id != 0
            && let Some(entry) = registry.get(&active_id)
        {
            batch.push((active_id, entry.session.clone(), true));
        }
        // 后台会话同样先收句柄（跳过活跃者，避免一帧处理两次）。
        for (id, entry) in registry.iter() {
            if active_id != 0 && *id == active_id {
                continue;
            }
            batch.push((*id, entry.session.clone(), false));
        }
        batch
    };
    // 锁外逐个处理：活跃会话全量，其余每帧 2 块保管道流动。
    for (session_id, session_handle, is_active) in &poll_batch {
        let mut session = session_handle.lock();
        if *is_active {
            // 处理来自 PTY 读取线程的 VT 输出。这是驱动全部终端状态更新的关键路径：
            // 从 `output_rx` 读取数据送入 Ghostty 的 VT 解析器，并填充下方轮询的
            // 各类事件标志（剪贴板等）。缺少此调用则终端永远不处理输出，输出通道死锁。
            session.process_output();
            // 检查 OSC 52 剪贴板读取请求（`ESC ] 52 ; c ; ?`）。此处（会话锁内）
            // 取走全部待答 selection；一次性槽位与应答线程在注册表/会话锁释放后才建立
            // （见下方），保持锁顺序单一方向。超限由下方单会话上限显式作答。
            for selection in session.take_clipboard_reads() {
                pending_clipboard_reads.push((*session_id, selection));
            }
        } else {
            // 每个后台会话每帧 2 块：足以在持续输出下不让读取线程阻塞。
            // 后台会话的输出通道一旦填满（读取线程阻塞在有界发送 → PTY 内核缓冲填满
            // → 子进程写入阻塞）就会冻结后台作业；每帧排空若干块既保持管道流动，
            // 又不饿死活跃会话的帧预算。
            session.poll_pty_output(PTY_POLL_CHUNKS_PER_FRAME);
            // 后台会话同样收割读请求。只排活跃者会让本队列无界增长（后台程序持续
            // 发读请求时每帧进一条、从不取出），且那些程序永远收不到应答。走的是
            // 与活跃会话同一条削减路径，故成本同样受下方的每帧上限约束。
            for selection in session.take_clipboard_reads() {
                pending_clipboard_reads.push((*session_id, selection));
            }
        }
        // 立即消费事件标志并带上正确的 `session_id` 推入。若留着不管，
        // 它们会在数分钟后该会话重新活跃时被重放（陈旧重放）。
        collect_session_events(
            *session_id,
            &mut session,
            &mut pending_events,
            &mut pending_exits,
        );
        // 会话锁在此释放（循环迭代末尾）。
    }
    handle_clipboard_reads(pending_clipboard_reads, &mut pending_events);

    for (id, reported) in pending_exits {
        pending_events.push(Event::Exit {
            session_id: id,
            code: match reported {
                crate::terminal::session::ReportedExit::Code(code) => Some(code),
                crate::terminal::session::ReportedExit::Unknown => None,
            },
        });
    }
    // 推入已收集的事件——此时不持有 `Session` 或 `SESSION_REGISTRY` 锁。
    for event in pending_events {
        EVENT_QUEUE.push(event);
    }
}

/// 不消费事件的会话推进泵：渲染暂停期间维持终端状态前进，避免后台作业停摆。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_pumpSessions(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| pump_sessions());
}
