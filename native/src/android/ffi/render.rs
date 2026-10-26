use super::*;

// ── 渲染生命周期 ──────────────────────────────────────────────────
/// 推进全部会话的终端状态，并把本轮产生的事件推入事件队列。
///
/// 与 [`poll_event_inner`] 的步骤 2 分离，使 [`Java_terminal_emulator_bridge_NativeBridge_pumpSessions`]
/// 可在渲染暂停期间单独调用：此时渲染线程已停，`pollEvent` 无人调用，若会话也不再
/// 推进，有界输出通道会填满 → 读线程阻塞 → PTY 内核缓冲填满 → 后台作业停摆。
/// 本函数不消费事件队列，故后台期间产生的剪贴板、退出等事件仍在队列中，
/// 渲染恢复后由 `pollEvent` 正常派发。
pub(super) fn pump_sessions() {
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

/// 处理待处理的 OSC 52 剪贴板读取请求（在注册表/会话锁之外）：登记一次性应答槽位、
/// 推入事件，并启动短生命周期的应答线程把宿主应用的答复写回 PTY。
///
/// VT 线程绝不能阻塞等待宿主应用，且 `clipboardResult` 可能在任意线程到达。
pub(super) fn handle_clipboard_reads(
    pending_clipboard_reads: Vec<(u64, String)>,
    pending_events: &mut Vec<Event>,
) {
    let mut flood_shed_answers = 0usize;
    for (session_id, selection) in pending_clipboard_reads {
        let unanswered = unanswered_clipboard_reads(session_id);
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
            let registry = wlock_session_registry();
            let session = registry.get(&session_id).map(|entry| entry.session.clone());
            if let Some(session) = session {
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

/// 不消费事件的会话推进泵：渲染暂停期间维持终端状态前进，避免后台作业停摆。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_pumpSessions(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| pump_sessions());
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：attachWindow
// ══════════════════════════════════════════════════════════════════════════

/// 挂载 Android Surface（仅 Android）。surface 挂载时由 Bridge.kt 调用：
/// `TerminalRuntime` 把 Android Surface 越过 JNI 边界交给渲染线程，后者经原生窗口消费；
/// surface 随后由 `detachWindow` 卸载。
///
/// # Safety
/// 仅由 JVM 经 JNI 调用，`surface` 须为本次调用期间有效的 Surface 对象。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_terminal_emulator_bridge_NativeBridge_attachWindow(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    surface: jobject,
    width: jint,
    height: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        // SAFETY: 外层已由 JVM 保证参数有效，此处透传同一调用期的引用。
        unsafe { attach_window_inner(env, _class, _session_id, surface, width, height) }
    })
}

#[cfg(target_os = "android")]
unsafe fn attach_window_inner(
    env: &mut Env,
    _class: JClass,
    _session_id: jlong,
    surface: jobject,
    width: jint,
    height: jint,
) {
    if surface.is_null() {
        log::error!("FFI: attachWindow called with null surface");
        return;
    }

    // 从 Surface 对象取得 ANativeWindow 裸指针。
    let raw_env = env.get_raw();
    // SAFETY: `raw_env` 来自 `get_raw()`，返回有效的 JNIEnv 指针；`surface` 是 JNI
    // 方法参数，JVM 运行时保证其有效。`ANativeWindow_fromSurface` 是 `libandroid.so`
    // 中有文档的 NDK 函数。
    let ptr = unsafe { ANativeWindow_fromSurface(raw_env as *mut _, surface as *mut _) };

    if ptr.is_null() {
        log::error!("FFI: attachWindow — ANativeWindow_fromSurface returned NULL");
        return;
    }

    log::info!("FFI: attachWindow ptr={:p} {}x{}", ptr, width, height);

    // 把 ANativeWindow 交给渲染器：wgpu 在创建 surface 时自行取得引用，故此处释放
    // `ANativeWindow_fromSurface` 返回的调用方自有引用（wgpu surface 会保持窗口存活
    // 直至 `detachWindow` 丢弃它）。
    //
    // 显式把交换链配置对齐到 BufferQueue：以 TextureView 为后端的 surface 否则可能
    // 停留在陈旧/默认几何，而模拟器上的 SwiftShader 会拒绝出队格式/几何与队列不符的
    // 缓冲（dequeueBuffer 超时）。WINDOW_FORMAT_RGBA_8888 = 1。
    //
    // 尺寸先钳位再两处共用：NDK 调用与 `attach_surface` 必须看到同一组尺寸，
    // 否则队列几何与交换链几何各说各话，SwiftShader 仍会拒绝出队。
    let surface_width = width.max(0);
    let surface_height = height.max(0);
    // SAFETY: `ptr` 是有效的 `ANativeWindow*`（上方已检查非空）；
    // `ANativeWindow_setBuffersGeometry` 是有文档的 NDK 函数。
    let geometry_status =
        unsafe { ANativeWindow_setBuffersGeometry(ptr, surface_width, surface_height, 1) };
    if geometry_status != 0 {
        // 非 0 即失败。丢弃它会让几何错配在 dequeueBuffer 超时后才暴露，且无从归因，
        // 故不挂载 surface 并在此出声。宿主不观察挂载结果（返回 Unit），因此
        // 「未挂载」不会被自动恢复：渲染线程照常跑，surface 失效位与看门狗照常记账。
        log::error!(
            "FFI: attachWindow — ANativeWindow_setBuffersGeometry failed: {geometry_status}"
        );
        // 与下方 `attach_surface` 失败同一条自愈路径：不得让任何会话继续「拥有」一个
        // 已不存在的 surface，否则旧属主的 detachWindow 仍能 CAS 成功并清掉状态，
        // 而新窗口无人挂载——渲染对「无 surface」返回 0 与空闲帧不可区分，终端永久黑屏。
        // SAFETY: 同下方的 `ANativeWindow_release`，此处无 surface 接管该引用。
        // 先释放再碰渲染状态：`render_state_mut()` 在渲染器尚未建好时会创建它并可能
        // panic，引用在那之后泄漏就再也归还不回来了。
        unsafe { ANativeWindow_release(ptr) };
        {
            let mut state = render_state_mut();
            state
                .as_mut()
                .expect("render_state_mut always initializes")
                .renderer
                .mark_surface_invalidated();
        }
        ATTACHED_SESSION_ID.store(0, std::sync::atomic::Ordering::Release);
        return;
    }
    let mut state = render_state_mut();
    let renderer = &mut state
        .as_mut()
        .expect("render_state_mut always initializes")
        .renderer;
    let session_id = _session_id as u64;
    match renderer.attach_surface(ptr.cast(), surface_width as u32, surface_height as u32) {
        Ok(()) => {
            ATTACHED_SESSION_ID.store(session_id, std::sync::atomic::Ordering::Release);
            log::info!("FFI: attachWindow surface attached (session {session_id})");
        }
        Err(error) => {
            log::error!("FFI: attachWindow surface attach failed: {error}");
            // 不得让任何会话继续「拥有」一个已不存在的 surface：旧属主的 detachWindow
            // 仍能 CAS 成功并清掉状态，而新窗口无人挂载，渲染对「无 surface」返回 0
            // 与空闲帧不可区分——终端永久黑屏。改为判死并上报，宿主据此换一个新窗口
            // 重新挂载（与 surface 丢失同一条自愈路径）。
            renderer.mark_surface_invalidated();
            ATTACHED_SESSION_ID.store(0, std::sync::atomic::Ordering::Release);
        }
    }
    // SAFETY: `ptr` 是来自 `ANativeWindow_fromSurface` 的有效 `ANativeWindow*`。
    // 无论挂载成功或失败，调用方自有的引用都必须释放：成功路径下 wgpu 在创建 surface
    // 时已取得自己的引用，失败路径下则无人接管。`ANativeWindow_release` 是有文档的
    // NDK 函数，会递减窗口引用计数。
    unsafe { ANativeWindow_release(ptr) };
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：render——经 CellData 快路径渲染一帧
// ══════════════════════════════════════════════════════════════════════════

/// 为活跃会话渲染一帧。返回：1 = 有输出且已呈现帧；0 = 会话无待处理单元数据（空闲）；
/// -1 = 出错（surface 缺失、GPU 失败、会话未知）。
///
/// 由 Kotlin 的渲染循环（单一线程）调用。`width`/`height` 仅供参考（真实尺寸由已挂载
/// 的 surface 配置决定），保留它们以便将来的 resize 路径可在此重配置交换链。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_render<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    _width: jint,
    _height: jint,
) -> jint {
    jni_export_guard!(&mut unowned_env, -1, |_env| render_inner(session_id as u64))
}

/// 由 `CursorInfo` 快照构建光标，光标色取当前主题。
/// 光标样式恒为默认块状：样式覆盖已随光标样式设置一并移除。
pub(super) fn build_cursor(
    render_state: &RenderState,
    cursor_info: &crate::terminal::ghostty_terminal::CursorInfo,
) -> crate::render::CellCursor {
    crate::render::CellCursor {
        // 光标落在宽字符尾列时 `col` 指的是第二格，而单元数据只为该宽字符发出一条
        // 头列记录（`width=2` 一条即整个字形）——渲染侧按列匹配，故在此折算成锚列。
        // 归位在本函数：它是 VT 真值与渲染锚点之间唯一的转换口，而 `getCursor-
        // ViewportPacked` 等可观测查询继续报 VT 的真实列。
        row: cursor_info.row,
        col: cursor_info.col - u32::from(cursor_info.at_wide_tail),
        visible: cursor_info.visible,
        style: cursor_info.style,
        color: crate::terminal::ghostty_terminal::GhosttyTerminal::color_to_float(
            render_state.theme.palette.cursor,
        ),
    }
}

/// 在掩码中把叠加行（搜索高亮）标脏。此类逐行视觉叠加的增删会改变像素而不触及单元内容。
/// 选区无需叠加处理：VT 线程把跟踪选区的反显烘焙进 `CellData`，选区变化经正常的
/// 脏路径作为新单元内容到达。
///
/// 脏带标记需覆盖的高亮行：当前高亮 ∪ 上次绘制的高亮。
///
/// 取并集是因为高亮被清除或移动时，陈旧高亮像素仍留在累加器里，漏掉任何一侧都会让
/// 那些行不再被重绘。两条渲染路径（VT 新帧与空闲同步）各需一次，故收在此处作单一真源。
/// 收两个切片而非 `&RenderState`：调用点已持有 `dirty_mask` 的可变借用，
/// 方法调用会连带借出整个结构体，与之冲突；字段级不相交借用则可用。
pub(super) fn highlight_rows_to_mark<'a>(
    current: &'a [crate::render::cell_builder::SearchHighlight],
    last_drawn: &'a [crate::render::cell_builder::SearchHighlight],
) -> impl Iterator<Item = &'a crate::render::cell_builder::SearchHighlight> {
    current.iter().chain(last_drawn.iter())
}

/// 搜索高亮行，当前与上次绘制的都要算：高亮是逐行叠加，增删移动会改变像素而
/// 不改变单元内容。直接遍历两段切片，不为每帧的这次调用分配中间 `Vec<i32>`。
pub(super) fn mark_overlay_dirty_rows<'a>(
    dirty_mask: &mut [bool],
    rows_usize: usize,
    highlights: impl Iterator<Item = &'a crate::render::cell_builder::SearchHighlight>,
) {
    for highlight_row in highlights.map(|highlight| highlight.row) {
        if highlight_row >= 0 && (highlight_row as usize) < rows_usize {
            dirty_mask[highlight_row as usize] = true;
        }
    }
}

/// 取会话句柄并请求其重推一帧。会话不存在时为 false。
///
/// 锁序：注册表 → 会话，且在**不持有** RENDER_STATE 时调用（见下方调用点）。
pub(super) fn request_session_repaint(session_id: u64) -> bool {
    session_handle(session_id)
        .map(|session| session.lock().terminal().request_repaint())
        .is_some()
}

/// 帧失败日志的分级：可恢复的一类记 debug，真异常记 error。
///
/// 分级依据是**错误类型**（[GpuError::is_recoverable]），不是文案——文案会变，
/// 类型不会。可恢复的一类（本帧没有可呈现的纹理：渲染暂停、surface 尚未挂载、
/// 平台收走了原生窗口）逐帧可能重复，按 debug 记才不会把 W/E 闸门淹掉；
/// 宿主侧另有「瞬时渲染错误」的限频汇总（`logcat-hygiene` 规范），两者分工：
/// 本处给细节，宿主给频率。
///
/// 真异常（管线缺失、单元转换失败、attach 失败……）仍记 error，不降级。
pub(super) fn log_frame_failure(error: &GpuError) {
    if error.is_recoverable() {
        log::debug!("render: frame unavailable: {error}");
    } else {
        log::error!("render: frame failed: {error}");
    }
}

/// 空闲路径的重绘门控（纯函数，表驱动单测）。
///
/// 五个变化信号任一为真即需重绘：搜索高亮、亚行余量、内容脏标志、累加器失效、
/// 需要整屏重绘。
///
/// 滚动命令在途（`scroll_frame_pending`）时一律不重绘：屏幕内容尚未反映本次滚动，
/// 此刻重绘只能用缓存帧（旧视口）配新余量，画出的内容比实际视口少滚了整行数
/// （滑动慢一拍），且那是一次多余的全量提交。滚动后的内容由终端推送的新帧呈现。
pub(super) fn should_repaint_idle(
    highlights_changed: bool,
    scroll_px_changed: bool,
    content_dirty: bool,
    frame_invalidated: bool,
    full_repaint: bool,
    scroll_frame_pending: bool,
) -> bool {
    !scroll_frame_pending
        && (highlights_changed
            || scroll_px_changed
            || content_dirty
            || frame_invalidated
            || full_repaint)
}

/// 一帧的单元数据：有新输出时携带自有单元与光标，空闲时只表示可绘制。
pub(super) enum FrameData {
    New {
        cells: Vec<crate::terminal::ghostty_terminal::CellData>,
        cursor_info: crate::terminal::ghostty_terminal::CursorInfo,
        rows: u32,
        cols: u32,
        scroll_offset: i64,
    },
    Idle,
}

/// 阶段 0 + 阶段 1：切会话后索要一帧，并完成渲染前准备（单次持有 RENDER_STATE 锁）。
///
/// 返回 false 表示本帧不渲染（无渲染态、无 surface 或管线尺寸未知）。
pub(super) fn prepare_render(session_id: u64) -> bool {
    // 累加器里是上一个会话的像素，而 `last_frame` 缓存也属于它：阶段 3 的空闲路径
    // 会因会话不一致而拒绘（防止把旧会话网格画到新会话），目标会话若无新输出就
    // 永远不推帧——屏幕一直停在旧会话内容上，直到碰巧有输入（实测切换后 25s 未变）。
    // 故此处按当前会话索要一次重推：内容去重在 VT 线程上被失效，下一帧即为
    // `FrameData::New`，阶段 3 对会话变化的 `_ => fill(true)` 分支整幅重画。
    // 只读一次 RENDER_STATE 并立即释放，随后才按注册表 → 会话的锁序取句柄。
    let cached_frame_session = {
        let state = render_state_mut();
        state
            .as_ref()
            .and_then(|render_state| render_state.last_frame.as_ref())
            .map(|(_, _, _, _, cached_session)| *cached_session)
    };
    if cached_frame_session.is_some_and(|cached_session| cached_session != session_id) {
        request_session_repaint(session_id);
    }

    // 检查 surface 就绪状态、上传图集脏区、惰性创建管线——全部在**一次**加锁内完成
    // 而非原先的三次，使每帧的 RENDER_STATE 加锁往返从 3 次降到 2 次（第二次在阶段 3）。
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        return false;
    };
    // 任何渲染工作之前必须已挂载 surface。
    if render_state.renderer.surface.is_none() {
        return false;
    }
    // 切会话时按会话键丢弃上一会话的搜索高亮（见 `SessionScoped`）：在阶段 1 这一次
    // 加锁内完成，两条帧路径共用同一份结论。
    render_state.search_highlights.enter_session(session_id);
    // 惰性一次性创建管线。
    if render_state.renderer.cell_pipeline.is_none() {
        let (surface_width, surface_height) = render_state
            .renderer
            .surface_config
            .as_ref()
            .map_or((0, 0), |config| (config.width, config.height));
        if surface_width == 0 || surface_height == 0 {
            return false;
        }
        render_state.renderer.initialize_pipeline_and_bind_group(
            ATLAS_SIZE,
            ATLAS_SIZE,
            surface_width,
            surface_height,
        );
        // initialize_pipeline_and_bind_group 重建了**空的**图集纹理，而 CPU 侧的
        // 字形缓存仍全部命中 → 没有任何字形会被重新 blit，终端只剩背景、无文字，
        // 且不会自愈（重建路径只在 surface 格式变化时触发）。故整幅标脏重传。
        render_state.font_pipeline.reset_dirty_rect_full();
        render_state.renderer.cell_cache = None;
        render_state.renderer.frame_invalidated = true;
    }
    // 上传字形图集的脏区（即便在空闲帧：新光栅化的字形必须在下次绘制前抵达 GPU 纹理）。
    if let Some(rect) = render_state.font_pipeline.take_dirty_rect() {
        let (atlas_width, atlas_height) = render_state.font_pipeline.atlas_dimensions();
        render_state.renderer.upload_atlas(
            render_state.font_pipeline.atlas_bitmap(),
            atlas_width,
            atlas_height,
            Some(rect),
        );
    }
    true
}

/// 阶段 2：收集单元数据（仅持会话锁）。
///
/// 有新数据时从通道接收自有的 `CellData`（零拷贝移动）；空闲时只记录事实——
/// 阶段 3 将直接引用 `last_frame`，省去此处原有的整帧克隆（CellData 96B×行列数）。
/// 暂停期不消费通道：receive 是破坏性取数，暂停帧取走后永不呈现
///（render_frame 直接丢弃），恢复后 VT 去重也不再重推——“IME 弹出
/// 时输入不可见”的主因。帧留在通道里，恢复后第一帧即最新。
///
/// `Err(退出码)` 表示本帧不渲染：暂停中或无 surface 记 0，渲染态缺失或会话不存在记 -1。
pub(super) fn collect_frame_data(session_id: u64) -> Result<FrameData, jint> {
    // 无 ANativeWindow 时同样不得消费通道：receive 是破坏性取数，而 VT 的去重基线
    // 在**入队**时就推进（见 push_cell_data），故被本函数取走又丢弃的帧不会被补发——
    // 换 Surface 的空档期（releaseGpuSurface → attachSurface）产出的输出会一直缺失，
    // 直到终端内容再次变化才重绘。与 paused 同一条判据，故同处提前返回。
    //
    // 只读一次 render_state 并立即释放：下方阶段 3 才取 RENDER_STATE，
    // 此处不得跨 SESSION_REGISTRY/Session 两级锁持有它（见阶段 3 的锁序说明）。
    {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            log::error!("render: render state missing");
            return Err(-1);
        };
        if render_state.renderer.render_paused || render_state.renderer.surface.is_none() {
            return Err(0);
        }
    }
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&session_id) else {
        log::warn!("render: unknown session {session_id}");
        return Err(-1);
    };
    let session = entry.session.lock();
    // 关键：**不要**在此调用 `session.terminal().scrollback_length()`——
    // 它是发往 VT 线程的同步 RPC，VT 线程繁忙时会把渲染线程阻塞最多 500ms。
    // 回滚长度搭载在经单元数据通道传递的 `CursorInfo` 上
    // （见 `push_cell_data` → `CursorInfo.scrollback_length`）。
    // 网格行列同理：`session.grid_size()` 在 resize 入队时提前发布，
    // VT 尚未应用新网格时与本帧 CellData 不是同一套尺寸；
    // `CursorInfo.rows/cols` 是产出本帧时的真实网格，与单元数据同源。
    match session.terminal().receive_cell_data() {
        Some((cells, cursor_info)) => {
            let rows = cursor_info.rows;
            let cols = cursor_info.cols;
            // 光标随帧发布给会话条目：输入法锚点上报（主线程、逐键）读它，
            // 不再为此发一次同步 VT 查询。发布点在注册表读锁 + 会话锁内，
            // 而载体本身是原子量——两侧都不因这次发布多等一把锁。
            entry
                .rendered_cursor
                .store(pack_rendered_cursor(&cursor_info), Ordering::Release);
            entry.rendered_scrollback.store(
                i32::try_from(cursor_info.scrollback_length).unwrap_or(i32::MAX),
                Ordering::Release,
            );
            Ok(FrameData::New {
                cells,
                cursor_info,
                rows,
                cols,
                scroll_offset: session.last_scroll_offset(),
            })
        }
        None => Ok(FrameData::Idle),
    }
}

pub(super) fn render_inner(session_id: u64) -> jint {
    let render_start = std::time::Instant::now();
    if !prepare_render(session_id) {
        return 0;
    }
    let phase1_done = std::time::Instant::now();
    let frame_data = match collect_frame_data(session_id) {
        Ok(frame_data) => frame_data,
        Err(exit_code) => return exit_code,
    };
    let phase2_done = std::time::Instant::now();

    // ── 阶段 3：渲染（持渲染状态锁）────────────────────────────────────
    // 本阶段是全局锁序 SESSION_REGISTRY → Session → RENDER_STATE 中唯一反向的一处：
    // 其余入口取 RENDER_STATE 前必已释放前两级锁，故不构成环形等待。新增反向取锁的
    // 代码必须先释放注册表与会话锁（`setSelection`/`setTheme` 同款作用域写法）。
    // 按新数据/空闲分支——空闲路径引用 `last_frame` 而不克隆，省去整帧克隆
    // （CellData 96B×行列数）。
    let outcome = match frame_data {
        FrameData::New {
            cells,
            cursor_info,
            rows,
            cols,
            scroll_offset,
        } => render_new_frame(session_id, cells, cursor_info, rows, cols, scroll_offset),
        FrameData::Idle => render_idle_frame(session_id),
    };
    let (count, lock_done, render_done) = match outcome {
        Ok(outcome) => outcome,
        Err(exit_code) => return exit_code,
    };
    let total = render_done.duration_since(render_start);
    // 限频：持续慢时逐帧都会命中，逐帧记会把 logcat 刷成慢帧洪水
    //（实测软件 Vulkan 上一轮采集里此类条目占应用侧 W 级绝大多数）。
    // 规范要求两次输出间隔不小于 500ms；本类诊断独占一个闸门。
    if total >= std::time::Duration::from_millis(30) && SLOW_RENDER_INNER_THROTTLE.should_log() {
        let milliseconds = |duration: std::time::Duration| duration.as_secs_f64() * 1000.0;
        log::warn!(
            "SLOW_RENDER_INNER phase1={:.3}ms receive={:.3}ms lock={:.3}ms render={:.3}ms total={:.3}ms count={count}",
            milliseconds(phase1_done.duration_since(render_start)),
            milliseconds(phase2_done.duration_since(phase1_done)),
            milliseconds(lock_done.duration_since(phase2_done)),
            milliseconds(render_done.duration_since(lock_done)),
            milliseconds(total),
        );
    }
    count
}

/// 阶段 3 的新帧路径：绘制本帧单元数据。
///
/// 返回 `(渲染计数, 渲染状态锁获取时刻, 呈现完成时刻)`；中途放弃渲染时返回
/// `Err(退出码)`，此时不进入慢帧诊断。
pub(super) fn render_new_frame(
    session_id: u64,
    cells: Vec<crate::terminal::ghostty_terminal::CellData>,
    cursor_info: crate::terminal::ghostty_terminal::CursorInfo,
    rows: u32,
    cols: u32,
    scroll_offset: i64,
) -> Result<(jint, std::time::Instant, std::time::Instant), jint> {
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::error!("render: render state missing");
        return Err(-1);
    };
    let lock_done = std::time::Instant::now();
    // 内容脏标志：新帧整幅重画，标志在此消费（与空闲路径同源）。
    render_state.dirty.swap(false, Ordering::AcqRel);
    // Kitty 同步：生成戳为 0 且无缓存时跳过查询（纯文本零开销）；
    // 会话/生成戳/滚动/网格任一变化才重查放置；图集仅在生成戳变化时
    // 打包重传，滚动/缩放只经无拷贝布局重建实例。
    // 全局锁序：渲染状态锁内绝不嵌套注册表与会话锁。会话查询在锁外完成
    // 后再重取渲染状态锁应用结果，消除渲染线程与主题线程的环形等待。
    let kitty_generation = cursor_info.kitty_generation;
    // 纯文本会话（生成戳为 0）不做逐格扫描：签名只服务于图像放置缓存。
    let placeholder_rows = if kitty_generation != 0 {
        placeholder_row_signatures(&cells, rows)
    } else {
        Vec::new()
    };
    let absolute_base_row = i64::from(cursor_info.scrollback_length).saturating_sub(scroll_offset);
    let placeholder_signature =
        placeholder_layout_signature(&placeholder_rows, absolute_base_row.max(0) as u64);
    // 网格单元格像素（与 render_cell_data 同口径：字体度量×光栅缩放）。
    let (font_width, font_height) = render_state.font_pipeline.cell_metrics();
    let raster_scale = render_state.font_pipeline.get_raster_scale();
    let grid_cell_width = font_width * raster_scale;
    let grid_cell_height = font_height * raster_scale;
    let kitty_keys = KittyPlacementKeys {
        session_id,
        generation: kitty_generation,
        placement_generation: cursor_info.kitty_placement_generation,
        scroll_offset,
        scrollback_length: cursor_info.scrollback_length,
        placeholder_signature,
        rows,
        cols,
    };
    let kitty_cell_changed = (grid_cell_width - render_state.kitty_cell_width).abs() > f32::EPSILON
        || (grid_cell_height - render_state.kitty_cell_height).abs() > f32::EPSILON;
    // 滚动进行中（视口键与上一帧不同）只平移已有放置帧；连续两帧视口键相同
    // 即视为停稳，才补一次真实查询。否则每一行滚动都要在渲染线程同步等一次
    // VT 查询（yazi 预览下实测约 60ms/行，滚动掉到 ~16fps）。
    let kitty_scroll = (scroll_offset, cursor_info.scrollback_length);
    let scroll_moving = kitty_scroll != render_state.last_kitty_scroll;
    render_state.last_kitty_scroll = kitty_scroll;
    // 失效规则集中在 `kitty_cache_invalidation`：放置键或单元格几何变化才重查，
    // 纯文本会话零开销跳过。
    let invalidation = kitty_cache_invalidation(
        kitty_keys,
        render_state.kitty_keys,
        kitty_cell_changed,
        !scroll_moving && render_state.kitty_frames_approximate,
    );
    // 单元格几何失步：终端侧仍为旧值时上游按旧几何重算像素尺寸，
    // 与新来源口径不一致。先同步新尺寸到终端再重查
    // （先排空命令积压再处理查询，命令先发即先生效，无竞态）。
    let need_cell_sync = invalidation.sync_cell_size;
    let need_kitty_fetch = invalidation.refetch_placements;
    let sync_cell_width = grid_cell_width.max(1.0) as u32;
    let sync_cell_height = grid_cell_height.max(1.0) as u32;
    drop(state);
    if need_cell_sync {
        let registry = rlock_session_registry();
        if let Some(entry) = registry.get(&session_id) {
            entry
                .session
                .lock()
                .terminal()
                .set_cell_pixel_size(sync_cell_width, sync_cell_height);
        }
    }
    // 渲染状态锁与会话锁之外查询：仅放置键变化时触发；刚推送帧时通常
    // 毫秒级返回（卡住时由 500 毫秒超时兜底），期间其他渲染状态消费者
    // 不再排队，按帧取会话锁的写入路径也不再排队。
    let fetched_kitty_frames = if need_kitty_fetch {
        session_query_channel(session_id)
            .map(|query_tx| {
                GhosttyTerminal::take_kitty_placements_on(
                    &query_tx,
                    (kitty_generation != 0).then(|| std::sync::Arc::from(placeholder_rows)),
                )
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::error!("render: render state missing");
        return Err(-1);
    };
    if render_state.renderer.surface.is_none() {
        return Err(0);
    }
    // 强制重查：旧帧像素尺寸按旧几何解算，必须按新几何重算。
    if invalidation.overlay_dirty {
        // 本帧刷新了 kitty 状态（图像增删或放置集合/几何变化）：上一帧留在
        // 累加器里的图像像素已失效，必须整帧重绘才能去掉残影。状态未变的
        // 帧不需要全量——叠加层每帧照画，文本层照旧走脏带。
        render_state.kitty_overlay_dirty = true;
        render_state.kitty_keys = kitty_keys;
        render_state.kitty_cell_width = grid_cell_width;
        render_state.kitty_cell_height = grid_cell_height;
        if kitty_generation == 0 {
            render_state.kitty_frames.clear();
            render_state.kitty_instances.clear();
            render_state.kitty_frames_approximate = false;
            if render_state.kitty_uploaded_generation != 0 {
                render_state.renderer.set_kgp_atlas(&[], 0, 0);
                render_state.kitty_uploaded_generation = 0;
            }
        } else {
            // 滚动中不查会话，只按视口顶行差量平移已有帧；帧坐标随之与当前
            // 视口对齐，停稳后的那一次真实查询负责恢复被旧视口裁掉的边缘。
            if invalidation.viewport_shift_rows != 0 {
                let shift = i32::try_from(invalidation.viewport_shift_rows).unwrap_or(i32::MAX);
                for frame in &mut render_state.kitty_frames {
                    frame.viewport_row = frame.viewport_row.saturating_add(shift);
                }
                render_state.kitty_frames_approximate = true;
            }
            // 锁外已取回放置帧，此处仅应用，不再持渲染状态锁查询会话。
            if invalidation.refetch_placements {
                render_state.kitty_frames = fetched_kitty_frames;
                render_state.kitty_frames_approximate = false;
            }
            if invalidation.repack_atlas {
                repack_kitty_atlas(
                    render_state,
                    grid_cell_width,
                    grid_cell_height,
                    kitty_generation,
                );
            } else if let Some((atlas_width, atlas_height, entries)) =
                crate::render::kitty::layout_entries(&render_state.kitty_frames)
            {
                // 同图集复用：仅重建实例。防御已上传图集尺寸漂移则全量重传。
                let (uploaded_width, uploaded_height) = (
                    render_state.renderer.kgp_atlas_width,
                    render_state.renderer.kgp_atlas_height,
                );
                if atlas_width == uploaded_width && atlas_height == uploaded_height {
                    render_state.kitty_instances = crate::render::kitty::build_kitty_instances(
                        &render_state.kitty_frames,
                        atlas_width,
                        atlas_height,
                        &entries,
                        grid_cell_width,
                        grid_cell_height,
                    );
                } else {
                    repack_kitty_atlas(
                        render_state,
                        grid_cell_width,
                        grid_cell_height,
                        kitty_generation,
                    );
                }
            } else {
                render_state.kitty_instances.clear();
            }
        }
    }
    let cursor = build_cursor(render_state, &cursor_info);
    // 用预分配的缓冲构建脏掩码。
    let rows_usize = rows as usize;
    // 先取不可变快照：掩码是对 `render_state.dirty_mask` 的 `&mut` 借用，
    // 无法与下方对 `render_state` 其他字段的读取共存。
    let previous_cursor_row =
        render_state
            .last_frame
            .as_ref()
            .and_then(|(_, old_cursor_info, _, _, cached_session)| {
                if *cached_session == session_id {
                    Some(old_cursor_info.row)
                } else {
                    None
                }
            });
    let dirty_mask = &mut render_state.dirty_mask;
    dirty_mask.clear();
    dirty_mask.resize(rows_usize, false);
    match &render_state.last_frame {
        Some((old_cells, _, old_rows, old_cols, cached_session))
            if *cached_session == session_id && *old_rows == rows && *old_cols == cols =>
        {
            crate::render::cell_builder::diff_dirty_rows_into(
                old_cells,
                &cells,
                rows,
                &mut dirty_mask[..rows_usize],
            );
        }
        _ => {
            dirty_mask[..rows_usize].fill(true);
        }
    }
    mark_dirty_rows(
        dirty_mask,
        rows_usize,
        cursor.row,
        previous_cursor_row,
        &render_state.search_highlights.current,
        &render_state.search_highlights.drawn,
        render_state.full_repaint,
    );
    render_state.full_repaint = false;
    let result = render_state.renderer.render_cell_data(
        &cells,
        rows,
        cols,
        cursor,
        &mut render_state.font_pipeline,
        &render_state.search_highlights.current,
        Some(&render_state.dirty_mask),
        &render_state.kitty_instances,
        render_state.kitty_overlay_dirty,
    );
    let render_done = std::time::Instant::now();
    // 新帧已就位（成功呈现，或留待空闲重绘重试）：滚动在途状态解除。
    // 渲染失败时同样解除——那一帧的单元数据已存进 `last_frame`，重试由
    // `dirty` 标志驱动；若标志不解，重试会被空闲路径自己挡掉。
    render_state.scroll_frame_pending = false;
    match result {
        Ok(()) => {
            render_state.kitty_overlay_dirty = false;
            render_state.last_frame = Some((cells, cursor_info, rows, cols, session_id));
            render_state.search_highlights.drawn = render_state.search_highlights.current.clone();
            render_state.last_scroll_px = render_state.renderer.viewport_scroll_px;
            Ok((1, lock_done, render_done))
        }
        Err(error) => {
            log_frame_failure(&error);
            // receive_cell_data() 是破坏性取数，VT 去重基线已推进，未呈现的帧
            // 不会被 VT 补发。把本帧 cells 留在 last_frame，并置 dirty 确保
            // 下一帧（Idle 或 New）重试，避免该帧内容永久丢失。
            render_state.last_frame = Some((cells, cursor_info, rows, cols, session_id));
            render_state.dirty.store(true, Ordering::Release);
            Ok((-1, lock_done, render_done))
        }
    }
}

/// 阶段 3 的空闲路径：复用上一帧的单元数据，按门控决定是否重绘。
///
/// 返回 `(渲染计数, 渲染状态锁获取时刻, 呈现完成时刻)`；中途放弃渲染时返回
/// `Err(退出码)`，此时不进入慢帧诊断。
pub(super) fn render_idle_frame(
    session_id: u64,
) -> Result<(jint, std::time::Instant, std::time::Instant), jint> {
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::error!("render: render state missing");
        return Err(-1);
    };
    let lock_done = std::time::Instant::now();
    let content_dirty = render_state.dirty.swap(false, Ordering::AcqRel);
    // 空闲路径：引用 `last_frame`——零克隆。会话不一致时不重绘，避免把旧会话画到新会话。
    let Some((ref cached_cells, cached_cursor, cached_rows, cached_cols, cached_session)) =
        render_state.last_frame
    else {
        return Err(0);
    };
    if cached_session != session_id {
        return Err(0);
    }
    let cursor = build_cursor(render_state, &cached_cursor);
    // 空闲重绘门控：仅当确有变化时重绘——搜索高亮、滚动偏移、内容脏标志被置位，
    // 或累加器失效（surface 重挂载/resize：新交换链从未收到过帧，空闲 shell
    // 会永远黑屏）。选区经 VT 线程作为新单元内容到达，故无需自己的门控。
    // 滚动命令在途时一律不重绘：屏幕内容尚未反映本次滚动，此刻只能用旧视口
    // 配新余量重绘（见 [`should_repaint_idle`]）。
    let highlights_changed =
        render_state.search_highlights.current != render_state.search_highlights.drawn;
    let scroll_px_changed =
        (render_state.renderer.viewport_scroll_px - render_state.last_scroll_px).abs()
            > f32::EPSILON;
    let needs_repaint = should_repaint_idle(
        highlights_changed,
        scroll_px_changed,
        content_dirty,
        render_state.renderer.frame_invalidated,
        render_state.full_repaint,
        render_state.scroll_frame_pending,
    );
    if !needs_repaint {
        // 只有「滚动在途」会带着脏标志跳过：归还它——字体/主题等改动
        // 只置此标志、没有别的触发点，被这一帧消费掉就再也不会画出来。
        if content_dirty {
            render_state.dirty.store(true, Ordering::Release);
        }
        return Err(0);
    }
    let rows_usize = cached_rows as usize;
    let dirty_mask = &mut render_state.dirty_mask;
    dirty_mask.clear();
    dirty_mask.resize(rows_usize, false);
    mark_dirty_rows(
        dirty_mask,
        rows_usize,
        cursor.row,
        None,
        &render_state.search_highlights.current,
        &render_state.search_highlights.drawn,
        render_state.full_repaint,
    );
    render_state.full_repaint = false;
    // Idle Kitty 同步：字体/缩放变化不经过 VT（无 New 帧），缓存实例
    // 会按旧单元格尺寸错位。纯本地经布局重建（无 RPC，图集未变不重传）。
    if !render_state.kitty_frames.is_empty() {
        let (idle_font_width, idle_font_height) = render_state.font_pipeline.cell_metrics();
        let idle_scale = render_state.font_pipeline.get_raster_scale();
        let idle_cell_width = idle_font_width * idle_scale;
        let idle_cell_height = idle_font_height * idle_scale;
        let idle_cell_changed = (idle_cell_width - render_state.kitty_cell_width).abs()
            > f32::EPSILON
            || (idle_cell_height - render_state.kitty_cell_height).abs() > f32::EPSILON;
        if idle_cell_changed {
            let rebuilt = crate::render::kitty::layout_entries(&render_state.kitty_frames)
                .filter(|(layout_width, layout_height, _)| {
                    *layout_width == render_state.renderer.kgp_atlas_width
                        && *layout_height == render_state.renderer.kgp_atlas_height
                })
                .map(|(layout_width, layout_height, entries)| {
                    crate::render::kitty::build_kitty_instances(
                        &render_state.kitty_frames,
                        layout_width,
                        layout_height,
                        &entries,
                        idle_cell_width,
                        idle_cell_height,
                    )
                });
            if let Some(instances) = rebuilt {
                render_state.kitty_instances = instances;
            }
            render_state.kitty_cell_width = idle_cell_width;
            render_state.kitty_cell_height = idle_cell_height;
        }
    }
    let result = render_state.renderer.render_cell_data(
        cached_cells,
        cached_rows,
        cached_cols,
        cursor,
        &mut render_state.font_pipeline,
        &render_state.search_highlights.current,
        Some(&render_state.dirty_mask),
        &render_state.kitty_instances,
        render_state.kitty_overlay_dirty,
    );
    let render_done = std::time::Instant::now();
    if result.is_ok() {
        render_state.kitty_overlay_dirty = false;
        render_state.search_highlights.drawn = render_state.search_highlights.current.clone();
        render_state.last_scroll_px = render_state.renderer.viewport_scroll_px;
        // 注意：空闲时**不**更新 `last_frame`——单元未变。
    }
    match result {
        Ok(()) => Ok((1, lock_done, render_done)),
        Err(error) => {
            log_frame_failure(&error);
            Ok((-1, lock_done, render_done))
        }
    }
}

/// 标脏光标行、搜索高亮行与整屏重绘行。
///
/// 两条帧路径共用：差异只有「上一帧光标行」——新帧路径需要标它（光标移动后
/// 旧位置的光标像素会残留），空闲路径不标（单元未变，光标位置随之不变）。
pub(super) fn mark_dirty_rows(
    dirty_mask: &mut [bool],
    rows: usize,
    cursor_row: u32,
    previous_cursor_row: Option<u32>,
    current_highlights: &[SearchHighlight],
    last_drawn_highlights: &[SearchHighlight],
    full_repaint: bool,
) {
    // 光标行：无论是否可见都标脏——光标是叠加在实例之上的
    // （可见性/位置变化会改变像素而不改单元内容），而脏带渲染必须重绘
    // 每一受影响的行，否则陈旧的光标像素会残留。
    if (cursor_row as usize) < rows {
        dirty_mask[cursor_row as usize] = true;
    }
    if let Some(previous_row) = previous_cursor_row
        && (previous_row as usize) < rows
    {
        dirty_mask[previous_row as usize] = true;
    }
    mark_overlay_dirty_rows(
        dirty_mask,
        rows,
        highlight_rows_to_mark(current_highlights, last_drawn_highlights),
    );
    // 主题变更等需要整屏重绘：把所有行标脏。
    if full_repaint {
        dirty_mask[..rows].fill(true);
    }
}

// ══════════════════════════════════════════════════════════════════════════
/// 「视口全空」哨兵：10 位全 1 模式。
pub(super) const LAST_CONTENT_ROW_NONE_BITS: i64 = 0x3FF;
/// 打包位偏移：内容下沿 43（10 位）、surface 失效位 53、备用屏 54。
pub(super) const SURFACE_INVALIDATED_BIT: i64 = 1 << 53;
/// 备用屏（helix/vim 等全屏 TUI）激活标志。随每帧渲染结果上报，与内容下沿同批采样，
/// 宿主据此让输入法跟随位移归零。
pub(super) const ALT_SCREEN_ACTIVE_BIT: i64 = 1 << 54;
/// 采样段 panic 时的上报值：渲染计数为负（-1）、`new_output` 为 0、内容下沿取
/// 未知哨兵、失效位为 0——与本导出文档声明的出错位形一致。
pub(super) const RENDER_SAMPLE_FAILURE_BITS: i64 = 0xFFFF_FFFF | (LAST_CONTENT_ROW_NONE_BITS << 43);

/// 把一帧的采样拼成上报位。
///
/// 抽成纯函数是为了让「保留位 33..42 MUST 为 0」这条不变量可测：直接在导出里写
/// `| (a << 32) | (b << 43) | …` 时，任何人加进一个新字段都可能悄悄落进保留位，
/// 而 Kotlin 侧不读那 10 位，于是没有任何一侧的测试会红。
#[must_use]
pub(super) fn pack_render_sample(
    count: i64,
    new_output: i64,
    content_row_bits: i64,
    surface_invalidated: i64,
    alt_screen_active: i64,
) -> i64 {
    count | (new_output << 32) | (content_row_bits << 43) | surface_invalidated | alt_screen_active
}

/// 把已渲染帧缓存的单元数据映射为上报位：视口全空时回未知哨兵，否则取视口内
/// 最后一个有内容的行（`cell_builder::last_content_row`）。
pub(super) fn last_content_row_bits_for_frame(
    cells: &[crate::terminal::ghostty_terminal::CellData],
    rows: u32,
) -> i64 {
    // 超出 10 位即按哨兵处理（而非截断）：截断会把 1024 报成 0、1025 报成 1，
    // 让 Kotlin 按一个**貌似合理**的错误行号去算位移；哨兵则让调用方保守地不位移。
    match crate::render::cell_builder::last_content_row(cells, rows) {
        Some(row) if (row as i64) < LAST_CONTENT_ROW_NONE_BITS => row as i64,
        _ => LAST_CONTENT_ROW_NONE_BITS,
    }
}

// JNI 导出：renderWithNewOutput
// ══════════════════════════════════════════════════════════════════════════

/// 合并的 render + consumeNewOutput：单次 JNI 穿越即完成渲染一帧**与**读取每会话的
/// `new_output` 标志，比分开两次调用每帧省约 0.1-0.3ms。
///
/// 返回打包的 `jlong`：位 0..31 = 渲染计数（语义同 `render()`）；位 32 = `new_output`
/// 标志（1 = 已摄入 PTY 输出，0 = 空闲）；位 33..42 = **保留，必须为 0**（历史光标行
/// 位形：该字段已无消费方，随输入法位移改按内容下沿裁剪而删除）；位 43..52 = 视口最后
/// 一个有内容的行（0x3FF = 视口全空）；位 53 = surface 已判死（缓存的 `ANativeWindow`
/// 对应被遗弃的 BufferQueue，需宿主换新的 `ANativeWindow` 才能恢复；重建成功即回落
/// 为 0）；位 54 = 备用屏激活（全屏 TUI）。
///
/// 行字段占 10 位：Android 网格行数上限远小于 1024（最小字号行高 ≥18px、
/// 屏幕高 ≤4096px ⇒ ≤227 行），超出即按哨兵处理，内容未知退化为不位移，是保守方向。
///
/// 内容下沿与失效位的采样刻意**不**挂在渲染计数门下：空闲帧 `render_inner`
/// 返回 0（无新单元数据，无需 GPU 呈现——正确），但 IME 跟随平移恰在空闲定居后最需要
/// 这些坐标；而失效位只在失败帧翻转，空闲门控会把它永远压在 0。采样复用本帧已渲染缓存，
/// 不新增发往 VT 线程的同步查询，故空闲帧无阻塞风险。
///
/// Kotlin 必须分别掩码每个字段：裸读 `(packed shr 32) != 0` 会把更高的位误当作输出。
/// 出错时渲染计数为负、`new_output` 为 0、内容下沿为 0x3FF、失效位为 0。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_renderWithNewOutput<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    _width: jint,
    _height: jint,
) -> jlong {
    let jni_start = std::time::Instant::now();
    let count = jni_export_guard!(&mut unowned_env, -1i32, |_env| render_inner(
        session_id as u64
    ));
    let inner_done = std::time::Instant::now();
    // 三个采样**必须**在计数门控之外：死 surface 的每一帧都以 -1 失败，把它们挂在
    // `count >= 0` 门下会让失效位永远送不到宿主（宿主因此永不换窗口，终端永久黑屏）。
    // 空闲帧（count == 0）同样要采样：空闲时无新单元数据、无需 GPU 呈现，但 IME 位移
    // 恰在空闲定居后最需要这些坐标。这里不调用 `render_cursor()`：它要走 VT 线程同步
    // 查询，缓存才是无阻塞且与绘制同源的坐标。
    //
    // 整段（连同 `new_output` 的消费）必须同在**一个**守卫内：守卫之间留任何一行裸
    // 代码，就等于把一次 panic 放回 `extern "system"` 边界外——那会 abort 进程而非抛
    // Java 异常。守卫内 `render_state_mut()` 在 `RENDER_STATE` 仍为 `None` 时会建渲染器
    // 并于 GPU 初始化失败时 panic，而 `render_inner` 刚失败（count == -1）的正是同一条
    // 路径。
    jni_export_guard!(&mut unowned_env, RENDER_SAMPLE_FAILURE_BITS, |_env| {
        // 就地消费 `new_output` 标志，省一次 JNI 穿越，
        // 并读取本帧已渲染缓存的光标与内容下沿，使跟随输入法的位移看到本帧绘制的坐标；
        // 同时上报渲染器判死的 surface，让宿主换新的 `ANativeWindow`（死窗口的
        // BufferQueue 不可 reconfigure 复活，实测会永久黑屏）。
        // `new_output` 是破坏性消费：只在计数非负（帧未被判失败）时取，未呈现的新数据
        // 必须留在通道里等下一帧，否则失败帧会吞掉它。
        // 会话锁序：注册表读锁 → 会话锁 → RENDER_STATE（`setSelection` 同款，
        // 不可跨作用域持有）。
        //
        // `new_output` 与备用屏标志都从注册表的共享原子量读取，**不取会话锁**：
        // 滚动期间 PTY 洪流会把会话锁按住数十毫秒，渲染线程若在此排队，Kotlin
        // 侧实测每帧 55-60ms（原生 render 本身 <30ms，故 SLOW_RENDER_INNER 不触发）。
        // `new_output` 仍是读后即清；失败帧（count < 0）不消费，留给下一帧。
        let (new_output, alt_screen_active) = {
            let registry = rlock_session_registry();
            match registry.get(&(session_id as u64)) {
                Some(entry) => {
                    let new_output = count >= 0 && entry.new_output.swap(false, Ordering::AcqRel);
                    let alt_screen_active = entry.alt_screen_active.load(Ordering::Acquire);
                    (
                        i64::from(new_output),
                        i64::from(alt_screen_active) * ALT_SCREEN_ACTIVE_BIT,
                    )
                }
                None => (0, 0),
            }
        };
        let render_state = render_state_mut();
        let last_frame = render_state
            .as_ref()
            .and_then(|render_state| render_state.last_frame.as_ref())
            .filter(|(.., cached_session)| *cached_session == session_id as u64);
        let content_row_bits = match last_frame {
            Some((cells, _, rows, _, _)) => last_content_row_bits_for_frame(cells, *rows),
            None => LAST_CONTENT_ROW_NONE_BITS,
        };
        let surface_invalidated = i64::from(
            render_state
                .as_ref()
                .is_some_and(|render_state| render_state.renderer.surface_invalidated()),
        ) * SURFACE_INVALIDATED_BIT;
        // 原生 render 本身未达 30ms（SLOW_RENDER_INNER 不触发）而整个 JNI 穿越
        // 超过 30ms：差额只可能落在渲染后的采样段（注册表/会话锁或 RENDER_STATE）。
        // 这条日志把「Kotlin 看到 58ms、原生 render 日志却沉默」的盲区钉住。
        let inner_ms = inner_done.duration_since(jni_start).as_secs_f64() * 1000.0;
        let sample_ms = inner_done.elapsed().as_secs_f64() * 1000.0;
        if inner_ms < 30.0
            && inner_ms + sample_ms >= 30.0
            && SLOW_RENDER_SAMPLE_THROTTLE.should_log()
        {
            log::warn!(
                "SLOW_RENDER_SAMPLE inner={inner_ms:.3}ms sample={sample_ms:.3}ms count={count}"
            );
        }
        pack_render_sample(
            count as i64 & 0xFFFF_FFFF,
            new_output,
            content_row_bits,
            surface_invalidated,
            alt_screen_active,
        )
    })
}

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：detachWindow
// ══════════════════════════════════════════════════════════════════════════

/// 卸载当前 surface（仅 Android）。
///
/// 实现：带属主检查（`ATTACHED_SESSION_ID` CAS）地释放 wgpu surface，并拆除该会话的
/// `RenderState`，使下次挂载时重建。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_detachWindow(
    mut _unowned: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
) {
    jni_export_guard!(&mut _unowned, (), |env| {
        detach_window_inner(env, _class, _session_id)
    })
}

#[cfg(target_os = "android")]
pub(super) fn detach_window_inner(_env: &mut Env, _class: JClass, _session_id: jlong) {
    let session_id = _session_id as u64;
    // 只有拥有已挂载 surface 的会话才能丢弃它：`switchSession` 先挂载新会话的 surface
    // 再卸载旧的，无条件卸载会抹掉刚挂载的 surface（首个之后的每个会话都黑屏）。
    //
    // 属主检查与 surface 丢弃在**同一个** `RENDER_STATE` 锁内完成
    // （`attach_window_inner` 也取该锁）：若拆成“先检查后丢弃”，并发的挂载
    // （已存入新属主）可能插入其间而其新建的 surface 被丢弃。
    //
    // 中毒后照常取内层值（与 `render_state_mut` 同口径）：渲染期 panic 已被
    // `jni_export_guard` 转成 Java 异常，锁随之中毒；此处置为放弃就永久漏掉
    // `release_surface` 与 `ATTACHED_SESSION_ID` 清零，surface 自愈路径随之失效。
    let mut state = RENDER_STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if ATTACHED_SESSION_ID
        .compare_exchange(
            session_id,
            0,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_err()
    {
        log::debug!(
            "detachWindow: session {session_id} does not own the attached surface, ignoring",
        );
        return;
    }
    log::info!("FFI: detachWindow (session {session_id})");
    // 丢弃 wgpu surface；下次 `attachWindow` 重建它。wgpu 持有的 ANativeWindow
    // 引用在 Surface 丢弃时释放。
    if let Some(render_state) = state.as_mut() {
        render_state.renderer.release_surface();
    }
}

// ══════════════════════════════════════════════════════════════════════════
// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：clipboardResult——Kotlin 应答 OSC 52 读取请求
// ══════════════════════════════════════════════════════════════════════════

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

// ══════════════════════════════════════════════════════════════════════════
/// 返回活跃会话 id 的数组（长整型，非 JSON）。
///
/// 注：由 `NativeBridgeSmokeTest` 覆盖（会话枚举的 JVM 边界）；生产 Kotlin 侧
/// 在 `TerminalRuntime.sessionIds` 中跟踪 id。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listSessions<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jlongArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        list_sessions_inner(env, _class)
    })
}

/// 按传入顺序返回各会话的终端标题；未知会话对应元素为 null。
///
/// 会话列表刷新原本逐会话各发一次 `getTitle`，N 个会话就是 N 次同步往返。标题查询
/// 与列表刷新同批发生，故一次取回。未知会话不抛：调用方此时正在按自己的会话表回填，
/// 竞态下少一个会话是正常状态，空标题即可表达。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionTitles<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_ids: jlongArray,
) -> jobjectArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        get_session_titles_inner(env, session_ids)
    })
}

pub(super) fn get_session_titles_inner<'local>(
    env: &mut Env<'local>,
    session_ids: jlongArray,
) -> jobjectArray {
    // SAFETY：`session_ids` 是本次 JNI 调用的实参，生命周期覆盖本函数。
    let ids = unsafe { JLongArray::from_raw(env, session_ids) };
    let Ok(length) = ids.len(env) else {
        log::error!("getSessionTitles: cannot read session id array length");
        return std::ptr::null_mut();
    };
    let mut session_id_values = vec![0 as jlong; length];
    if ids.get_region(env, 0, &mut session_id_values).is_err() {
        log::error!("getSessionTitles: cannot read session ids");
        return std::ptr::null_mut();
    }
    let Ok(string_class) = env.find_class(jni_str!("java/lang/String")) else {
        log::error!("getSessionTitles: find_class java/lang/String failed");
        return std::ptr::null_mut();
    };
    let Ok(array) = env.new_object_array(
        session_id_values.len() as jsize,
        string_class,
        JObject::null(),
    ) else {
        log::error!("getSessionTitles: new_object_array failed");
        return std::ptr::null_mut();
    };
    for (index, id) in session_id_values.iter().enumerate() {
        // 未知会话留 null：调用方按空标题处理，不抛异常。
        let element: JObject = match session_title(*id as u64) {
            Some(title) => match env.new_string(title) {
                Ok(text) => text.into(),
                Err(error) => {
                    log::error!("getSessionTitles: cannot encode title for {id}: {error}");
                    return std::ptr::null_mut();
                }
            },
            None => JObject::null(),
        };
        if let Err(error) = array.set_element(env, index, &element) {
            log::error!("getSessionTitles: cannot store title {index}: {error}");
            return std::ptr::null_mut();
        }
    }
    array.into_raw()
}

/// Rust 字符串 → JNI `jstring`：编码失败返回 null，不抛异常、不吞错（调用方按 null 处理）。
pub(super) fn string_to_jstring(env: &mut Env, text: &str) -> jstring {
    match env.new_string(text) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 会话标识列表 → JNI `LongArray`；注册表为空时返回空数组。
pub(super) fn list_sessions_inner<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
) -> jlongArray {
    let ids: Vec<u64> = rlock_session_registry().keys().copied().collect();
    u64_vec_to_long_array(env, &ids)
}

/// 字体管线默认的图集单元尺寸（像素）。
pub(super) const DEFAULT_FONT_CELL_SIZE: f32 = 14.0;

/// 字形管线初始化用的图集尺寸。UV 归一化的尺寸由管线自身持有并对外提供，
/// 渲染侧不再并传第二份，故此处是唯一声明点。
///
/// 取 2048²（原为 1024²）：真机约 3x 显示密度下 14sp 字形光栅化后约 40px，
/// 原图集只能容纳约 700 个字形——几百个不同字符的滚动日志屏会持续冲刷 LRU，
/// 逐帧淘汰并重新光栅化同一批字形（CPU 密集）。2048² 使容量翻两番（约 2800 个字形），
/// 且内存代价有界（设备 16MB + 暂存位图 16MB）。
pub(super) const ATLAS_SIZE: u32 = 2048;

// ══════════════════════════════════════════════════════════════════════════
// JNI 导出：TerminalQueryPort（搜索/回滚/文本/字体）
//
// Kotlin TerminalQueryPort 接缝的原生侧。每个导出都遵循 resize_inner 的模式：
// 注册表读锁 → 会话锁 → 引擎查询；会话 id 未知时抛 IllegalStateException。
// 字体查询不需要会话 id，直接读全局 RENDER_STATE。
// ══════════════════════════════════════════════════════════════════════════

/// 返回某会话的终端标题（OSC 0/2），无则 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTitle<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 单会话查询保持严格契约：会话未知即抛，不返回空标题——调用方要能区分
        // 「没有标题」与「会话没了」。
        let Some(title) = session_title(session_id as u64) else {
            throw_session_missing(env, "getTitle");
            return Ok(std::ptr::null_mut());
        };
        string_to_jstring(env, &title)
    })
}

/// 会话标题的查询通道外置（理由同 `wideCharTailCols`：标题随每次状态更新拉取）。
/// 会话未知返回 `None`，与 `getTitle` 的抛错契约区分开。
pub(super) fn session_title(session_id: u64) -> Option<String> {
    let query_tx = session_query_channel(session_id)?;
    Some(crate::terminal::ghostty_terminal::GhosttyTerminal::title_on(&query_tx))
}
