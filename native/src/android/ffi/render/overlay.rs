use super::super::query::session_handle;
use super::super::query::session_query_channel;
use super::super::{
    EnvUnowned, GpuError, JClass, Ordering, RenderState, SEARCH_HIGHLIGHT_CURRENT_ALPHA,
    SEARCH_HIGHLIGHT_OTHER_ALPHA, SearchHighlight, ThrowRuntimeExAndFailure, jint, jlong,
    render_state_mut, rendered_viewport_row,
};

// ── 叠加层与脏区 ────────────────────────────────────────────────────
/// 由 `CursorInfo` 快照构建光标，光标色取当前主题。
///
/// 样式取自帧：默认样式由配置下发（`setCursorStyle`，termux 的
/// `terminal-cursor-style`），应用运行期间的 DECSCUSR 覆盖优先。两处都落在 VT 侧，
/// 渲染层只负责照搬——在这里再夹一层「恒为块状」会让应用设的光标形状失效。
pub(crate) fn build_cursor(
    render_state: &RenderState,
    cursor_info: &crate::terminal::ghostty_terminal::CursorInfo,
) -> crate::render::CellCursor {
    crate::render::CellCursor {
        // 光标落在宽字符尾列时 `col` 指的是第二格，而单元数据只为该宽字符发出一条
        // 头列记录（`width=2` 一条即整个字形）——渲染侧按列匹配，故在此折算成锚列。
        // 归位在本函数：它是 VT 真值与渲染锚点之间唯一的转换口，而 `getCursor-
        // ViewportPacked` 等可观测查询继续报 VT 的真实列。
        row: cursor_info.row,
        // 饱和减法而非裸减：`at_wide_tail` 为真而 `col == 0` 是网格与光标失配时的
        // 陈旧数据，裸减在 debug 构建里 panic、在 release 里绕成 `u32::MAX`——后者会把
        // 光标画到视口外、且再无任何记录能匹配它。这里按「宁可不画」兜底。
        col: cursor_info
            .col
            .saturating_sub(u32::from(cursor_info.at_wide_tail)),
        visible: cursor_info.visible,
        style: cursor_info.style,
        color: crate::terminal::ghostty_terminal::GhosttyTerminal::color_to_float(
            render_state.palette.cursor,
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
pub(crate) fn highlight_rows_to_mark<'a>(
    current: &'a [crate::render::cell_builder::SearchHighlight],
    last_drawn: &'a [crate::render::cell_builder::SearchHighlight],
) -> impl Iterator<Item = &'a crate::render::cell_builder::SearchHighlight> {
    current.iter().chain(last_drawn.iter())
}

/// 搜索高亮行，当前与上次绘制的都要算：高亮是逐行叠加，增删移动会改变像素而
/// 不改变单元内容。直接遍历两段切片，不为每帧的这次调用分配中间 `Vec<i32>`。
pub(crate) fn mark_overlay_dirty_rows<'a>(
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
/// 锁序：注册表 → 会话，且在**不持有** `RENDER_STATE` 时调用（见下方调用点）。
pub(super) fn request_session_repaint(session_id: u64) -> bool {
    session_handle(session_id)
        .map(|session| session.lock().terminal().request_repaint())
        .is_some()
}

/// 帧失败日志的分级：可恢复的一类记 debug，真异常记 error。
///
/// 分级依据是**错误类型**（[`GpuError::is_recoverable`]），不是文案——文案会变，
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
/// （滑动慢一拍），且那是一次多余的全量提交。该位的值 MUST 由
/// [`RenderState::scroll_pending`] 给出（含超时兜底），不得直接读字段。滚动后的内容由终端推送的新帧呈现。
pub(crate) fn should_repaint_idle(
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

/// 新帧是否已反映在途的滚动命令（纯函数，表驱动单测）。
///
/// 帧携带「VT 已处理的最后一个滚动请求序号」，宿主持有「已送达的最后一个序号」。
/// 帧的序号更小即说明该帧早于在途命令——VT 尚未处理该命令时推送的帧仍是旧视口。
/// 此刻呈现会用旧视口配新的亚行余量，画出的位置比实际视口少滚了整行数：
/// 内容先上跳半行、待新视口到达再下跳整行（上下滑动晃动）。
///
/// 序号相等或更大都算已反映：命令送达后 VT 处理它才会推出带该序号的帧，
/// 而处理前的帧携带的是上一个序号。
pub(crate) fn frame_reflects_scroll(frame_request_id: u64, latest_request_id: u64) -> bool {
    frame_request_id >= latest_request_id
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

// ── 搜索高亮 ──────────────────────────────────────────────────────
/// 清除渲染器下一帧的搜索高亮区间。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clearSearchHighlights(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    _session_id: jlong,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.search_highlights.current.clear();
            // 不动 `session`：清空 `current` 已足以让本会话不再有高亮，而改键会让下一帧
            // 走进 `enter_session` 的换会话分支——那里保留 `drawn` 正是为了把旧高亮行
            // 再标脏一次，键一改这条补救路径就白走一趟。
            // 置脏标志：高亮清除必须送达屏幕，即便终端处于空闲
            // （否则陈旧高亮会一直残留到下次 PTY 输出——正是 #5 这类回归）。
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    });
}

/// 把视口命中折算成可绘制的高亮区间。
///
/// 折算口径 `gridRow = 绝对行 - viewport_top`，`viewport_top` 是视口顶行的绝对行号
/// （回滚区顶部 = 0）。落在视口外的命中两侧都裁掉：上游 `viewport_matches` 按页读取，
/// 会连带返回与视口同页、实际在视口外的命中（上游文档明确要求调用方裁剪）。
///
/// 当前选中命中的 alpha MUST 与其余命中不同（255 vs 160）：两者同色系时，
/// 「当前在哪」不可区分——用户报障「当前匹配与其他匹配高亮应不同」。
///
/// 抽成纯函数以便表驱动测试：颜色分配与折算裁剪是两组独立不变式，混在 JNI 导出
/// 里就只能靠像素级仪器化测试间接覆盖。
pub(crate) fn build_search_highlights(
    matches: &[crate::terminal::ghostty_terminal::SearchMatch],
    selected: &[crate::terminal::ghostty_terminal::SearchMatch],
    viewport_top: i32,
    grid_rows: i32,
    palette: &crate::terminal::theme::Palette,
) -> Vec<SearchHighlight> {
    matches
        .iter()
        .filter_map(|matched| {
            let row = matched.row as i32 - viewport_top;
            if row < 0 || row >= grid_rows {
                return None;
            }
            let color = if selected.contains(matched) {
                let current = palette.foreground;
                [
                    current.r,
                    current.g,
                    current.b,
                    SEARCH_HIGHLIGHT_CURRENT_ALPHA,
                ]
            } else {
                let other = palette.selection_background;
                [other.r, other.g, other.b, SEARCH_HIGHLIGHT_OTHER_ALPHA]
            };
            Some(SearchHighlight {
                row,
                start_col: matched.start_col as i32,
                end_col_exclusive: matched.end_col as i32,
                color,
            })
        })
        .collect()
}

/// 把当前搜索的视口命中写入渲染态，供下一帧绘制。
///
/// 命中与选中态取自上游 `search::Search`（VT 线程内），Kotlin 只提供视口滚动偏移与
/// 两个主题色：视口折算口径 `gridRow = 绝对行 - (回滚长度 - 滚动偏移)`，`scrollOffset`
/// 为 0 表示贴底。落在视口外的命中不写入——高亮只画在可见格上。
#[unsafe(no_mangle)]
pub(super) extern "system" fn Java_terminal_emulator_bridge_NativeBridge_applySearchHighlights(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    session_id: jlong,
    scroll_offset: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            log::error!("applySearchHighlights: session {session_id} not found");
            return Ok(());
        };
        let highlights =
            crate::terminal::ghostty_terminal::GhosttyTerminal::search_viewport_highlights_on(
                &query_tx,
            );
        // 视口顶行的绝对行号：以 VT 真值（`rendered_viewport_row`）为准——锚在内容上，
        // 浏览期间新输出不移动它，不会随宿主滚动偏移漂移。首帧未发布时回退到
        // `scrollback_lines - scroll_offset`（宿主旧口径），保证首帧也有高亮。
        let viewport_top = match rendered_viewport_row(session_id as u64) {
            Some(row) if row >= 0 => row,
            _ => highlights.scrollback_lines as i32 - scroll_offset,
        };
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(());
        };
        // 上游 `viewport_matches` 会按页多给一些视口外的命中（文档明确说明），
        // 折算后必须两侧都裁：只裁负行会让视口下方的命中进入高亮列表，
        // 一旦偏移换算有误差就画到无关行上。
        let grid_rows = render_state
            .last_frame
            .as_ref()
            .map_or(i32::MAX, |(_, _, rows, _, _)| *rows as i32);
        // 当前选中命中用主题前景色，其余用主题选区色：颜色真源是渲染态持有的
        // 当前主题（见 `setTheme`），Kotlin 不参与。
        let palette = &render_state.palette;
        render_state.search_highlights.current = build_search_highlights(
            &highlights.matches,
            &highlights.selected,
            viewport_top,
            grid_rows,
            palette,
        );
        render_state.search_highlights.session = session_id as u64;
        // 置脏标志：无 PTY 流量时新高亮也必须绘制到下一帧
        //（#5 空闲屏回归）。
        render_state.dirty.store(true, Ordering::Relaxed);
    });
}
