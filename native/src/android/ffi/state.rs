use super::query::session_handle;
use super::{
    EnvUnowned, JClass, JNI_FALSE, JNI_TRUE, Ordering, SessionEntry, ThrowRuntimeExAndFailure,
    jboolean, jfloat, jint, jlong, render_state_mut, rlock_session_registry,
};

use crate::terminal::ghostty_terminal::ViewportTarget;

/// 滚动指令投递失败告警的限频闸门（带计数：行内给出窗口内失败的次数）。
///
/// 滚动期间宿主每帧都调用 `setScrollOffset`：命令通道满时逐帧记一行会把 logcat
/// 刷成洪水，而窗口内失败了几次才是排障要的形状。
static SCROLL_SEND_FAILED_THROTTLE: crate::slow_log::CountedLogThrottle =
    crate::slow_log::CountedLogThrottle::new();

// ── 网格尺寸查询 ────────────────────────────────────────────────
/// 当前单元格宽高（字号单位，取自渲染器的字体管线；物理像素再乘 `raster_scale`），
/// 打包为一个 `jlong`：低 32 位是宽度的 `f32` 位、高 32 位是高度。无渲染状态时为 0。
///
/// 宽高一并取回：两者只可能被同一处消费（触摸映射与网格重算），分两次查询只是多
/// 一次 JNI 往返。宽度对字号**严格线性**（`advance × size / upem`，等宽分支无
/// 取整），故它是「原生是否真的接受了该字号」最可靠的判据：原生拒收时单元格度量
/// 不变。行高经过 `.ceil()`，对字号非线性。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCellSizePacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
) -> jlong {
    jni_export_guard!(unowned_env, 0, |_env| {
        // 这次查询只是给 spawn 算网格：GPU 暂不可用时返回 0，Kotlin 侧保留旧度量或退回
        // config 默认网格（`refreshCellMetricsFrom` 已写明 0 的语义），**不**在这里 abort。
        // 与 prefetchRenderState 同口径：GPU 失败可重试、只记账，字体库不可用才致命；
        // 真无 GPU 的设备仍在首帧由渲染路径给出明确错误。
        if let Err(initialization_error) = crate::render::context::try_global_gpu() {
            log::error!("cell metrics query skipped: {initialization_error}");
            return Ok(0);
        }
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return Ok(0);
        };
        let (width, height) = render_state.font_pipeline.cell_metrics();
        cell_metrics_packed_from(width, height)
    })
}

/// 宽高 → 打包 `jlong`。宽度 MUST 用零扩展（`u64::from`）：写成 `as u64` 会把
/// 宽度位模式的最高位符号扩展进高半区，高度随之变成一个天文数字。
pub(super) fn cell_metrics_packed_from(width: f32, height: f32) -> jlong {
    (((u64::from(height.to_bits())) << 32) | u64::from(width.to_bits())) as jlong
}

/// 在会话注册表里查找会话并读取其状态；会话未知时返回 `None`。
///
/// 注册表读锁在读取期间持有，与逐个导出各自取锁的旧写法等价：读的都是会话
/// 条目里已发布的共享量（原子镜像标志、忙标记、网格尺寸）。
pub(super) fn with_session_entry<T>(
    session_id: u64,
    read: impl FnOnce(&SessionEntry) -> T,
) -> Option<T> {
    let registry = rlock_session_registry();
    let entry = registry.get(&session_id)?;
    Some(read(entry))
}

/// 当前网格尺寸，打包为 `(rows << 32) | cols`；会话未知时为 0。
/// 供 `Bridge.getGridRowsColsPacked` 使用：Kotlin 桩实现原本永远返回 0，
/// 导致 resize 被丢弃后 `syncGridDimensions` 永远无法收敛到原生网格。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getGridRowsColsPacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, 0, |_env| {
        with_session_entry(session_id as u64, |entry| {
            let session = entry.session.lock();
            let (rows, cols) = session.grid_size();
            (i64::from(rows) << 32) | i64::from(cols)
        })
        .unwrap_or(0)
    })
}

/// 读会话的 VT 线程维护的原子镜像标志（鼠标上报、备用屏等）。
///
/// **不取会话锁**：镜像由 VT 线程每帧写入注册表持有的共享原子量，读它只需一次
/// 原子读。会话锁则可能被 PTY 写入按住数秒（`feed_pty` 的写入面正是这样克隆
/// 出去再写），而这两个标志在输入路径上**每个事件都要判一次**（触摸拖动的
/// MOVE、外接滚轮）——持锁读会把 UI 线程按在写入洪流上，表现为滚动卡顿。
///
/// 会话未知时返回 `false`：那意味着「此刻无法确认该状态」，与标志本身为假同义，
/// 故调用方无需区分。
pub(super) fn read_terminal_atomic_flag(
    session_id: u64,
    read: impl Fn(&SessionEntry) -> bool,
) -> bool {
    with_session_entry(session_id, |entry| read(entry)).unwrap_or(false)
}

/// 读原子镜像标志并转成 JNI 布尔值。
///
/// 备用屏状态与鼠标上报状态两个导出的形状相同：读标志 → 真假映射到
/// `JNI_TRUE`/`JNI_FALSE`。映射只此一处，两端各写一遍时，「读到的假」与
/// 「读不到」的分界就可能分叉。
pub(super) fn read_terminal_flag_jboolean(
    session_id: u64,
    read: impl Fn(&SessionEntry) -> bool,
) -> jboolean {
    if read_terminal_atomic_flag(session_id, read) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

// ── 滚动、模式与状态查询 ────────────────────────────────────────
/// 宿主滚动偏移 → 视口滚动目标。`None` = 无需滚动（偏移未变）。
///
/// 偏移 0 是「回到实时视图」这个**意图**，不能按增量回退：回滚浏览期间若有新输出，
/// 活动区底边已经下移，按增量回退只会退回浏览起点，停在离底边若干行处——新输出落在
/// 视口之外，终端看上去「死了」（剩余输出不可见）。
///
/// 其余偏移按增量换算：负值 = 向上滚入历史，正值 = 回到更新内容。宿主偏移在用户
/// 向上滑（进入历史）时增大，故这里必须取反——此前符号写反，向下滑到底部时发的是
/// 负增量，结果反而滚进了历史。
pub(super) fn scroll_target(target: i64, last_offset: i64) -> Option<ViewportTarget> {
    if target == last_offset {
        return None;
    }
    if target == 0 {
        return Some(ViewportTarget::Bottom);
    }
    Some(ViewportTarget::Delta(-((target - last_offset) as isize)))
}

/// 设置视口滚动偏移（回滚区内的行数，0 = 活跃屏幕）。与上次偏移的差值在 VT 线程
/// 经 `scroll_viewport(Delta)` 应用，故下一次 `CellData` 推送即携带滚动后的视图
/// （此前在 Kotlin 侧是空操作）。
///
/// 返回 true 表示本次写入已生效（或无需写入）。会话锁被 PTY 写入按住时返回 false 且
/// **不阻塞**：调用方保留上一次已提交的值，下一帧重试。滚动期间每帧都调用本函数，
/// 在这里等锁等于把帧率钉在 PTY 输出的节奏上（真机日志实测 pre 段 100ms+）。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setScrollOffset(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
    offset: jint,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_TRUE, |_env| {
        let target = i64::from(offset.max(0));
        let (sent, request_id) = {
            let Some(session) = session_handle(session_id as u64) else {
                return Ok(JNI_TRUE);
            };
            // 每会话独立计算增量：此前代码按单一全局 `render_state.scroll_offset`
            // 计算，切换会话会污染恢复后会话的视口。
            let Some(session) = session.try_lock() else {
                return Ok(JNI_FALSE);
            };
            let Some(viewport_target) = scroll_target(target, session.last_scroll_offset()) else {
                return Ok(JNI_TRUE);
            };
            let sent = session.terminal().scroll_viewport(viewport_target);
            if sent {
                session.set_last_scroll_offset(target);
                // 触摸滚动的每个 MOVE 都会走到这里（数十次/手势），与同路径的
                // `setScrollYPx` 同级：日志级别取 trace，debug 级只会把 logcat 刷满。
                // 偏移是否正确在画面上立刻可见，逐次数值不是排障信号。
                log::trace!("setScrollOffset: target={target} viewport_target={viewport_target:?}");
            } else {
                // 命令通道已满或 VT 线程已消失：**不**推进 `last_scroll_offset`，
                // 使下次以相同目标调用时能重试该增量而非静默丢弃。此前先推进全局
                // 偏移，发送失败后视口会永久停留在陈旧状态。
                if let Some(failed_scrolls) = SCROLL_SEND_FAILED_THROTTLE.record() {
                    log::warn!(
                        "setScrollOffset: scroll_viewport send failed (target={target}); \
                         will retry (failed_scrolls={failed_scrolls})"
                    );
                }
            }
            (sent, session.terminal().latest_scroll_request_id())
        };
        // 滚动命令在途：VT 线程推送新 CellData 是异步的，此刻屏幕内容仍是旧视口。
        // 置位使空闲重绘在此期间跳过——否则只能用旧视口配新余量重绘，画出的内容
        // 比实际视口少滚了整行数（滑动慢一拍），且那是一次多余的全量提交。
        // 滚动后的帧到达即解除（渲染线程被帧唤醒钩子叫醒，无需在此强制重绘）。
        //
        // 此前这里置 `dirty` 强制重绘：那是「内容去重会吞掉滚动重绘」时代的补丁
        // （滑动期间无 New 帧时 Idle 门控不含行偏移，滑动被吞到下一次输出才出来）。
        // 去重缺陷已修——滚动命令会失效去重基线并强制推帧——补丁却留下了多余的提交。
        if sent {
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                render_state.begin_scroll_pending(request_id);
            }
        }
        if sent { JNI_TRUE } else { JNI_FALSE }
    })
}

/// 设置逐像素平滑滚动用的视口 Y 像素偏移（正值 = 内容下移，与 Kotlin 手势余数
/// 符号一致）。只登记待用值：真正写 uniform 的是随后那一帧的 `begin_frame`，
/// 它每帧都按当前偏移重写同一份字节。
///
/// 此前这里当场 `refresh_cell_uniforms`（一次 `queue.write_buffer`）——与同一线程
/// 紧接着开帧的那次写入完全重复，却在洪流期间排队等 GPU 队列 ~100ms。渲染循环的
/// pre 段因此从 <1ms 涨到 `100ms+`，真机日志（`SLOW_LOOP` pre=102-123ms）即此。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setScrollYPx(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    _session_id: jlong,
    offset_px: jfloat,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        if !offset_px.is_finite() {
            return Ok(());
        }
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(());
        };
        render_state.renderer.set_viewport_scroll_px(offset_px);
        // 触摸滚动的每个 MOVE 都会走到这里（数十次/手势），日志级别取 trace：
        // 与同路径的 `setScrollOffset` 同级，debug 级只会把 logcat 刷满。
        log::trace!("setScrollYPx: {offset_px}");
    });
}

/// 远端当前是否处于备用屏幕缓冲（vim / less / htop）。无锁读取 VT 线程在每次
/// `Query::AltScreen` 查询时维护的镜像——可在 Android 输入路径的每次触摸滚动事件
/// 中调用而不阻塞 UI 线程。
///
/// 供 `Bridge.isAltScreenActive` 使用，使备用屏上的触摸滚动能作为滚轮转义序列
/// 转发给远端，而非滚动本地回滚（备用屏会吞掉滚轮）。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getAltScreenState(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |_env| {
        read_terminal_flag_jboolean(session_id as u64, |entry| {
            entry.alt_screen_active.load(Ordering::Acquire)
        })
    })
}

// ── 会话存活忙标记 ────────────────────────────────────────────────
/// 忙标记 → 已忙时长（纳秒）。0 = 空闲（标记未置位）。
///
/// **时钟域必须留在 Rust 侧**：忙标记存的是 Rust 单调时钟的读数（自进程内某个
/// 基准起算），而 JVM 的 `System.nanoTime()` 基准是另一个原点，跨边界相减得到的
/// 「时长」没有意义。故此处把差值算完再交给 Kotlin，Kotlin 只拿到一个时长。
pub(super) fn busy_elapsed_nanos(busy_since_nanos: u64) -> jlong {
    if busy_since_nanos == 0 {
        return 0;
    }
    // 已置位即「在忙」：时长最小取 1ns，使「忙」与「空闲」在导出值上不重合
    // （同一纳秒内读出 0 会被当成空闲）。
    crate::monotonic_nanos()
        .saturating_sub(busy_since_nanos)
        .max(1) as jlong
}

/// VT 线程已忙多久（纳秒）；0 = 空闲等待。
///
/// 供 `Bridge.vtBusyNanos` 使用：看门狗据此判「VT 层挂起」（有工作在途却久不
/// 返回）。空闲会话恒为 0——按「一段时间没有进展」判定会把空闲 shell 判死。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getVtBusyNanos(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, 0, |_env| {
        with_session_entry(session_id as u64, |entry| {
            busy_elapsed_nanos(entry.vt_busy_since_nanos.load(Ordering::Acquire))
        })
        .unwrap_or(0)
    })
}

/// PTY 输出投递已阻塞多久（纳秒）；0 = 未在投递。
///
/// 供 `Bridge.ptySendBusyNanos` 使用：非零且久不归零即「输出已读出却交不出去」
/// ——归因指向下游（VT 线程）未消费，而不是读取线程卡死。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getPtySendBusyNanos(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, 0, |_env| {
        with_session_entry(session_id as u64, |entry| {
            busy_elapsed_nanos(entry.pty_send_since_nanos.load(Ordering::Acquire))
        })
        .unwrap_or(0)
    })
}

// ── 文本选区 ──────────────────────────────────────────────────────
/// 设置下一渲染帧的活动文本选区。
///
/// 终端持有选区安装口：坐标为绝对网格行（row 0 = 回滚顶部），与控制柄逻辑一致。
/// 设置当前文本选区（绝对网格行列，0 = 回滚顶部）；高亮颜色由终端侧按主题调色板烘焙，本通道不传颜色参数。
/// 选区经 `GhosttyTerminal::set_selection` 安装为终端状态（跟踪引用，随滚动/
/// 输出/重排跟随文本），高亮由 VT 线程按行级选区反白直接烘焙进 `CellData`；
/// 本层不再存储单元格位置，仅透传。`hasSelection=false` 清除选区。
/// 选区恒为线性（块选通道已随 mode 一并移除）。
///
/// 返回 true 表示本次写入已生效（或无需写入）。会话锁被 PTY 写入按住时返回 false 且
/// **不阻塞**：调用方（渲染循环）保留上一次已提交的值，下一帧重试。拖动选区时每帧都会
/// 调用本函数，阻塞在这里等于把每帧拖到 PTY 输出的时长（真机实测单帧 100ms+）。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSelection(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
    start_row: jint,
    start_col: jint,
    end_row: jint,
    end_col: jint,
    has_selection: jboolean,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_TRUE, |_env| {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return Ok(JNI_TRUE);
        };
        let Some(session) = entry.session.try_lock() else {
            return Ok(JNI_FALSE);
        };
        if has_selection == jni::sys::JNI_TRUE {
            session.terminal().set_selection(
                (start_row.max(0) as u32, start_col.max(0) as u32),
                (end_row.max(0) as u32, end_col.max(0) as u32),
            );
        } else {
            session.terminal().clear_selection();
        }
        drop(session);
        drop(registry);
        // 置脏标志：选区变化必须重绘，即便终端空闲（VT 线程也会重推 CellData，
        // 此处只是唤醒渲染门控而不必等待它）。
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.dirty.store(true, Ordering::Relaxed);
        }
        JNI_TRUE
    })
}
