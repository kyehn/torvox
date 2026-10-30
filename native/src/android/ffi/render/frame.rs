use super::super::query::session_query_channel;
use super::super::*;
use super::overlay::{
    build_cursor, frame_reflects_scroll, log_frame_failure, mark_dirty_rows,
    request_session_repaint, should_repaint_idle,
};
use super::session_ops::ATLAS_SIZE;

// ── 帧准备与呈现 ────────────────────────────────────────────────────
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

/// 设置光标闪烁间隔（毫秒）；0 = 不闪（`terminal-cursor-blink-rate`）。
///
/// 只写一个原子量、不碰渲染态：这条设置在 Surface 挂载之前下发，为一个「间隔」
/// 去初始化 wgpu 适配器只会把首帧前的失败点提前。宿主要在每次间隔边界上补一帧
/// （`forceRender`），否则静止的终端里光标会停在某个半相位上。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setCursorBlinkRate<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    rate_ms: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        // 负值当 0：宿主的取值范围是 0..2000，负数只可能来自配置解析之外的路径。
        set_cursor_blink_rate(rate_ms.max(0) as u32);
    })
}

/// 一帧的单元数据：有新输出时携带自有单元与光标，空闲时只表示可绘制。
pub(super) enum FrameData {
    New(NewFrame),
    Idle,
}

/// 新帧的全部数据：单元、光标、网格尺寸与宿主滚动状态。
///
/// 成组传递：取帧与出图按同一组数据走，拆成散参数传一遍只会让两处签名各写
/// 一份字段清单，改一处漏一处。
pub(super) struct NewFrame {
    cells: Vec<crate::terminal::ghostty_terminal::CellData>,
    cursor_info: crate::terminal::ghostty_terminal::CursorInfo,
    rows: u32,
    cols: u32,
    scroll_offset: i64,
    /// 宿主已送达的最后一个滚动请求序号（0 = 从未下发过滚动命令）。
    latest_scroll_request_id: u64,
}

/// 渲染状态锁：持锁、给出渲染状态引用、记下取锁时刻。
///
/// 取锁时刻一并记下——它是慢帧诊断的分界点（锁内耗时与锁外耗时分开计），
/// 三条帧路径都经此取锁，各写一遍时漏记一处即让慢帧归因失真。
struct RenderStateLock {
    state: std::sync::MutexGuard<'static, Option<RenderState>>,
    lock_done: std::time::Instant,
}

impl RenderStateLock {
    /// 渲染状态的可变引用；渲染状态缺失即报错退出。
    fn get(&mut self) -> Result<&mut RenderState, jint> {
        let Some(render_state) = self.state.as_mut() else {
            log::error!("render: render state missing");
            return Err(-1);
        };
        Ok(render_state)
    }
}

/// 取渲染状态锁并记下取锁时刻。
fn lock_render_state() -> RenderStateLock {
    let state = render_state_mut();
    let lock_done = std::time::Instant::now();
    RenderStateLock { state, lock_done }
}

/// 绘制渲染状态里的缓存帧。
///
/// 单元数据、行列数与光标都取自渲染状态的帧缓存：新帧路径先把本帧存入缓存
/// 再绘制，空闲路径直接画缓存。绘制参数序列只此一处——两处各写一遍时，
/// 新增一个绘制参数漏改一处即表现为「某一帧少画一层」。
fn draw_cached_frame(
    render_state: &mut RenderState,
    cursor: crate::render::CellCursor,
) -> Result<(), crate::render::GpuError> {
    // 帧缓存与绘制输入同处一个结构，须按字段拆开借用：缓存取不可变，
    // 渲染器与字体管道取可变。
    let RenderState {
        renderer,
        font_pipeline,
        search_highlights,
        dirty_mask,
        kitty_instances,
        kitty_overlay_dirty,
        last_frame,
        ..
    } = render_state;
    let Some((cells, _, rows, cols, _)) = last_frame.as_ref() else {
        return Ok(());
    };
    renderer.render_cell_data(
        cells,
        *rows,
        *cols,
        cursor,
        font_pipeline,
        &search_highlights.current,
        Some(&dirty_mask[..]),
        kitty_instances,
        *kitty_overlay_dirty,
    )
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
    // 回滚长度搭载在经单元帧邮箱传递的 `CursorInfo` 上
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
            Ok(FrameData::New(NewFrame {
                cells,
                cursor_info,
                rows,
                cols,
                scroll_offset: session.last_scroll_offset(),
                latest_scroll_request_id: session.terminal().latest_scroll_request_id(),
            }))
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
        FrameData::New(new_frame) => render_new_frame(session_id, new_frame),
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
    frame: NewFrame,
) -> Result<(jint, std::time::Instant, std::time::Instant), jint> {
    let NewFrame {
        cells,
        cursor_info,
        rows,
        cols,
        scroll_offset,
        latest_scroll_request_id,
    } = frame;
    let mut locked = lock_render_state();
    let lock_done = locked.lock_done;
    let render_state = locked.get()?;
    // 本帧是否早于在途的滚动命令：VT 尚未处理该命令时推送的帧仍是旧视口，
    // 此刻呈现会用旧视口配新亚行余量——画出的位置比实际视口少滚了整行数，
    // 视觉上内容先上跳半行、待新视口到达再下跳整行（上下滑动晃动）。
    // 故陈旧帧一律不呈现，也不解除滚动在途门控：等 VT 处理命令后推送的帧。
    if !frame_reflects_scroll(cursor_info.scroll_request_id, latest_scroll_request_id) {
        // 逐帧事件（滚动期间每帧都可能命中）：按 logcat-hygiene 规范记 trace，
        // 不得以 debug 逐次输出把 logcat 刷满。
        log::trace!(
            "scroll frame stale: processed={} latest={}",
            cursor_info.scroll_request_id,
            latest_scroll_request_id
        );
        // 取帧之后 surface 才被拆走：本帧数据已被 VT 破坏性取出，丢弃就再没有
        // 补发（滚动命令强制重推的是滚动后的视口，不是这一帧的视口）。存进
        // last_frame 并解除滚动在途，重挂载后的空闲重绘据此补画。
        if render_state.renderer.surface.is_none() {
            render_state.last_frame = Some((cells, cursor_info, rows, cols, session_id));
            render_state.scroll_frame_pending = false;
        }
        return Ok((0, lock_done, std::time::Instant::now()));
    }
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
    drop(locked);
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
    let mut locked = lock_render_state();
    let render_state = locked.get()?;
    if render_state.renderer.surface.is_none() {
        // collect 之后 surface 才被拆走：本帧数据已被 VT 破坏性取出，丢弃就再没有
        // 补发。存进 last_frame，让重挂载后的空闲重绘能把它画出来；同时解除滚动
        // 在途，否则那个门控会把这次唯一的送达机会也挡掉。
        render_state.last_frame = Some((cells, cursor_info, rows, cols, session_id));
        render_state.scroll_frame_pending = false;
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
    // 新帧总会整帧下发，故只推进相位、不需要翻转信号。
    render_state.advance_cursor_blink();
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
    // 内容脏标志在此消费而非函数入口：上面的提前返回（例如无 surface）没有
    // 呈现任何内容，不能把「字体/主题已变」这类仅靠该标志送达的信号吃掉。
    render_state.dirty.swap(false, Ordering::AcqRel);
    // 本帧先入缓存再绘制：新帧与空闲帧都从缓存取单元数据，绘制参数序列
    // 只有一处。绘制失败时缓存里已是本帧，下一次重试直接重画它。
    render_state.last_frame = Some((cells, cursor_info, rows, cols, session_id));
    let result = draw_cached_frame(render_state, cursor);
    let render_done = std::time::Instant::now();
    // 新帧已就位（成功呈现，或留待空闲重绘重试）：滚动在途状态解除。
    // 渲染失败时同样解除——那一帧的单元数据已存进 `last_frame`，重试由
    // `dirty` 标志驱动；若标志不解，重试会被空闲路径自己挡掉。
    render_state.scroll_frame_pending = false;
    match result {
        Ok(()) => {
            render_state.kitty_overlay_dirty = false;
            render_state.search_highlights.drawn = render_state.search_highlights.current.clone();
            render_state.last_scroll_px = render_state.renderer.viewport_scroll_px;
            Ok((1, lock_done, render_done))
        }
        Err(error) => {
            log_frame_failure(&error);
            // receive_cell_data() 是破坏性取数，VT 去重基线已推进，未呈现的帧
            // 不会被 VT 补发。本帧 cells 已在 last_frame（绘制前存入），并置
            // dirty 确保下一帧（Idle 或 New）重试，避免该帧内容永久丢失；
            // 全量标记同样归还，重试才能覆盖失败帧可能只画了一半的区域。
            render_state.dirty.store(true, Ordering::Release);
            render_state.full_repaint = true;
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
    let mut locked = lock_render_state();
    let lock_done = locked.lock_done;
    let render_state = locked.get()?;
    // 空闲路径：引用 `last_frame`——零克隆。会话不一致时不重绘，避免把旧会话画到新会话。
    // 单元数据与列数由绘制函数自己从缓存取，故此处只取光标、行数与会话号。
    let Some((_, cached_cursor, cached_rows, _, cached_session)) = render_state.last_frame else {
        return Err(0);
    };
    if cached_session != session_id {
        return Err(0);
    }
    // 取 dirty 放在缓存可用性检查之后：没有可画帧时本次不消费任何变化信号。
    let content_dirty = render_state.dirty.swap(false, Ordering::AcqRel);
    // 相位推进必须在取光标之前：本帧画出的光标要用推进后的相位。
    // 只有 VT 自己认为光标可见（DECTCEM 开）时，相位翻转才改变画面：光标本就
    // 隐藏的话，翻转前后都不画它，为它补一帧只是白算。`advance` 必须在 `&&` 左侧
    // 无条件求值——它同时负责记录相位，短路掉状态就跟不上时基了。
    let cursor_blink_flipped = render_state.advance_cursor_blink() && cached_cursor.visible;
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
        cursor_blink_flipped,
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
    let result = draw_cached_frame(render_state, cursor);
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
            // 空闲路径是字体/主题等「只置 dirty、无新帧」改动的唯一送达通道：
            // 本帧失败已把该标志换走，不归还就再没有任何触发点，改动永久丢失。
            // 全量标记同理——失败帧可能只画了部分区域，重试必须整幅覆盖。
            render_state.dirty.store(true, Ordering::Release);
            render_state.full_repaint = true;
            Ok((-1, lock_done, render_done))
        }
    }
}
