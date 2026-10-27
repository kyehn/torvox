use super::render::string_to_jstring;
use super::session::read_jni_string;
use super::*;

// ── 文本与滚动查询 ──────────────────────────────────────────────
/// 返回某会话的回滚行数。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_scrollbackLength(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut unowned_env, 0, |env| {
        // 通道外置：查询在两把锁之外执行（理由同 `getTitle`）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "scrollbackLength");
            return Ok(0);
        };
        crate::terminal::ghostty_terminal::GhosttyTerminal::scrollback_length_on(&query_tx) as jint
    })
}

/// 返回终端光标的视口位置，打包为 `(y << 32) | x`；光标隐藏或在视口外时返回 -1。
///
/// **当前真值、同步查询**：`Query::RenderCursor` → `cursor_in_viewport`，取值即 VT
/// 快照此刻的光标格，代价是一次 VT 线程往返（忙时上限两次查询超时）。正确性断言
/// 要「刚写进去的数据立刻反映出来」时用本方法；逐帧/逐键的热路径 MUST 改用
/// [`Java_terminal_emulator_bridge_NativeBridge_getRenderedCursorPacked`]。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCursorViewportPacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, -1, |env| {
        // 通道外置：光标坐标取自整网格构建，锁内等待会把按帧取这两把锁的渲染线程
        // 一并堵住（理由同 `getTitle`，此处代价更高）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "getCursorViewportPacked");
            return Ok(-1);
        };
        let Some((row, col)) =
            crate::terminal::ghostty_terminal::GhosttyTerminal::render_cursor_on(&query_tx)
        else {
            return Ok(-1);
        };
        ((row as jlong) << 32) | (col as jlong)
    })
}

/// 返回**最近一帧**渲染线程取走的光标视口位置，打包同 `getCursorViewportPacked`；
/// 首帧被取走之前（或该帧光标不可见）返回 -1。
///
/// 与 `getCursorViewportPacked` 同口径（同出 `cursor_in_viewport`）而不同时效：这里读
/// 的是已消费帧的光标，代价是一次原子读——不发查询、不取会话锁、不等待 VT 线程。
/// 逐键发生的输入法锚点上报取本方法；需要「刚写入的数据立刻反映」的断言取前者。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getRenderedCursorPacked(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut unowned_env, RENDERED_CURSOR_NONE, |env| {
        let Some(packed) = rendered_cursor_packed(session_id as u64) else {
            throw_session_missing(env, "getRenderedCursorPacked");
            return Ok(RENDERED_CURSOR_NONE);
        };
        packed
    })
}

/// 最近一帧的回滚长度（0 起）。首帧尚未取走时为 -1；未知会话抛异常（与光标载体一致，
/// 让调用方能区分「会话没了」与「暂时没有帧」）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getRenderedScrollbackLength(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut unowned_env, RENDERED_SCROLLBACK_NONE, |env| {
        let Some(length) = rendered_scrollback(session_id as u64) else {
            throw_session_missing(env, "getRenderedScrollbackLength");
            return Ok(RENDERED_SCROLLBACK_NONE);
        };
        length
    })
}

/// 会话**已应用**的滚动偏移（0 = 视口贴底）。
///
/// 与 `TerminalRuntime.activeSessionScrollOffset()` 的区别是「已应用」：后者是渲染线程
/// 尚未消费的命令槽，读到的是转瞬即逝的中间态。仪器测试要断言「视口归位」时读这里——
/// 一次原子读，不写终端、不发 VT 查询（旧做法向网格注入标记行，会把真实内容挤出
/// 无回滚的视口，连带把提示符丢出屏幕）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionScrollOffset(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut unowned_env, 0, |_env| {
        session_handle(session_id as u64).map_or(0, |session| {
            i32::try_from(session.lock().last_scroll_offset()).unwrap_or(i32::MAX)
        })
    })
}

/// 返回单个回滚行去除首尾空白后的文本，空行返回 null。
/// **惰性访问语义**——Ghostty 只保留被显式读取过的行，多数下标返回 null。
/// 不可用于全文迭代——应改用 `dump_grid`（经 `getTerminalText`）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_scrollbackLine<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Ok(row) = u32::try_from(row) else {
            // 负行号表示无数据而非调用错误：视口拖过回滚顶部时正常出现，
            // 按查询契约返回 null，不抛异常。
            return Ok(std::ptr::null_mut());
        };
        // 通道外置：查询在两把锁之外执行（滚动时逐行拉取，理由同 `getTitle`）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "scrollbackLine");
            return Ok(std::ptr::null_mut());
        };
        // Kotlin 传入的是绝对行号（回滚 + 视口偏移，经
        // `scrollbackLength - scrollOffset + row` 计算），故直接透传给期望绝对行号的
        // `read_line_text`。
        let text =
            crate::terminal::ghostty_terminal::GhosttyTerminal::read_line_text_on(&query_tx, row);
        match text {
            Some(text) => string_to_jstring(env, &text),
            None => std::ptr::null_mut(),
        }
    })
}

/// 返回可见区与回滚区以换行拼接的文本。
///
/// 只需字符，故走 `read_all_text`（每格一次 codepoint 读取）而非 `dump_grid`：
/// 后者每格还要取样式、解析颜色与 flags，回滚区填满时（上限 2000 行）单次查询
/// 会把 VT 线程按在数十万次跨语言调用上，挤占命令通道——测试与 UI 的查询突发
/// 随即双双超时回退，表现为「标记不落格」与渲染掉帧。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTerminalText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 通道外置：全回滚扫描最慢，必须在两把锁之外跑，否则期间渲染与事件
        // 收割全部停摆（该导出每次全选/复制都调用）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "getTerminalText");
            return Ok(std::ptr::null_mut());
        };
        let text = crate::terminal::ghostty_terminal::GhosttyTerminal::read_all_text_on(&query_tx);
        string_to_jstring(env, &text)
    })
}

/// 返回当前**可见视口**的文本（每行以换行拼接，行数 == 网格行数）。
///
/// 与 `getTerminalText`（视口 + 全回滚）相对：调用方要判断「某行是否看得见」时
/// 用它，代价随网格面积而非回滚深度走。仪器测试以此确认视口确实停在底部。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getVisibleText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            throw_session_missing(env, "getVisibleText");
            return Ok(std::ptr::null_mut());
        };
        let text =
            crate::terminal::ghostty_terminal::GhosttyTerminal::read_visible_text_on(&query_tx);
        string_to_jstring(env, &text)
    })
}

/// 用 Ghostty 原生格式化器提取选中文本（换行感知、宽字符安全，语义同
/// termux 的 `TerminalBuffer.getSelectedText`）。
/// `startRow`/`startCol`/`endRow`/`endCol` 为网格行列（绝对：第 0 行是回滚顶部，
/// 与 `scrollbackLine` 一致）。出错时返回 ""。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectionText<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    start_row: jint,
    start_col: jint,
    end_row: jint,
    end_col: jint,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        // 通道外置：格式化取文在两把锁之外跑（复制动作，期间渲染不该停摆）。
        let Some(query_tx) = session_query_channel(id) else {
            return Ok(std::ptr::null_mut());
        };
        let text = crate::terminal::ghostty_terminal::GhosttyTerminal::selection_text_on(
            &query_tx,
            (start_row.max(0) as u32, start_col.max(0) as u32),
            (end_row.max(0) as u32, end_col.max(0) as u32),
        );
        string_to_jstring(env, &text)
    })
}

/// `jint` 切片 → JNI `IntArray`；分配失败返回 null。
pub(super) fn jint_slice_to_int_array(env: &mut Env, values: &[jint]) -> jintArray {
    let Ok(array) = env.new_int_array(values.len()) else {
        return std::ptr::null_mut();
    };
    if array.set_region(env, 0, values).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

/// 会话标识 → JNI `LongArray`（空表即无会话）；分配失败返回 null。
pub(super) fn u64_vec_to_long_array(env: &mut Env, values: &[u64]) -> jlongArray {
    let wide: Vec<jlong> = values.iter().map(|value| *value as jlong).collect();
    let Ok(array) = env.new_long_array(wide.len()) else {
        return std::ptr::null_mut();
    };
    if array.set_region(env, 0, &wide).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

/// 有序选区界限（绝对网格坐标，0 = 回滚顶部）→ JNI `IntArray
/// [startRow, startCol, endRow, endCol]`；`None`（无可选内容）→ null。
pub(super) fn bounds_to_int_array(
    env: &mut Env,
    bounds: Option<((u32, u32), (u32, u32))>,
) -> jintArray {
    let Some(((start_row, start_col), (end_row, end_col))) = bounds else {
        return std::ptr::null_mut();
    };
    let values = [
        start_row.min(jint::MAX as u32) as jint,
        start_col.min(jint::MAX as u32) as jint,
        end_row.min(jint::MAX as u32) as jint,
        end_col.min(jint::MAX as u32) as jint,
    ];
    jint_slice_to_int_array(env, &values)
}

/// `u32` 列号序列 → JNI `IntArray`（宽字符尾格查询回传）；分配失败返回 null。
pub(super) fn u32_vec_to_int_array(env: &mut Env, values: &[u32]) -> jintArray {
    let clamped: Vec<jint> = values
        .iter()
        .map(|value| (*value).min(jint::MAX as u32) as jint)
        .collect();
    jint_slice_to_int_array(env, &clamped)
}

/// 以落点为锚在终端安装选区并回传界限。会话锁与注册表读锁都在构造界限
/// 后立即释放：选区查询走 VT 线程，绝不能持锁跨越。
pub(super) fn select_bounds_export<'local>(
    mut unowned_env: EnvUnowned<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
    // 通道外置：选区派生在两把锁之外跑（理由同 `getTitle`）。
    select: impl FnOnce(
        &flume::Sender<crate::terminal::ghostty_terminal::Query>,
        u32,
        u32,
    ) -> Option<((u32, u32), (u32, u32))>,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(std::ptr::null_mut());
        };
        let bounds = select(&query_tx, row.max(0) as u32, col.max(0) as u32);
        bounds_to_int_array(env, bounds)
    })
}

/// 上游 select_word：以落点为锚派生词选区并安装到终端（ghostty 默认词
/// 边界），回传 `[startRow, startCol, endRow, endCol]`（绝对网格坐标）；
/// 落点无可选词或查询失败返回 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectWordAt<'local>(
    unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jintArray {
    select_bounds_export(unowned_env, session_id, row, col, |query_tx, row, col| {
        GhosttyTerminal::select_word_at_on(query_tx, row, col)
    })
}

/// 该行中作为宽字符后半格（`SpacerTail`）的列号，升序。
///
/// 会话不存在或查询失败时返回空数组（等价于无吸附）：保持原列，不猜。会话锁与
/// 通道外置：查询在两把锁之外执行（每次拖动手柄都调用，锁内等 VT 应答会连同
/// 按帧取锁的渲染一起卡住）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_wideCharTailCols<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(u32_vec_to_int_array(env, &[]));
        };
        let tail_cols = crate::terminal::ghostty_terminal::GhosttyTerminal::wide_char_tail_cols_on(
            &query_tx,
            row.max(0) as u32,
        );
        u32_vec_to_int_array(env, &tail_cols)
    })
}

/// 上游 select_all：全部内容（回滚 + 视口，界限不含尾部空行/空列）派生
/// 并安装，回传与失败语义同 selectWordAt。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectAll<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(std::ptr::null_mut());
        };
        let bounds = crate::terminal::ghostty_terminal::GhosttyTerminal::select_all_on(&query_tx);
        bounds_to_int_array(env, bounds)
    })
}

/// 查询网格单元处的 OSC 8 超链接 URI（第 0 行 = 回滚顶部，与 `scrollbackLine` 一致），
/// 无链接时返回 null。纯文本裸 URL 不做识别：libghostty-vt 只提供 OSC 8 超链接。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_hyperlinkAt<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 通道外置：查询在两把锁之外执行（长按取链接时调用，理由同 `getTitle`）。
        let Some(query_tx) = session_query_channel(session_id as u64) else {
            return Ok(std::ptr::null_mut());
        };
        let url = GhosttyTerminal::hyperlink_at_on(&query_tx, row.max(0) as u32, col.max(0) as u32);
        match url {
            Some(url) => string_to_jstring(env, &url),
            None => std::ptr::null_mut(),
        }
    })
}

// ── 搜索与选择 ──────────────────────────────────────────────────
/// 设置搜索 needle 并同步跑完搜索，返回命中总数。会话不存在、查询超时或断连时返回 0。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchSetNeedle<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    needle: JString<'local>,
) -> jint {
    jni_export_guard!(&mut unowned_env, 0, |env| {
        let id = session_id as u64;
        let Some(needle) = read_jni_string(env, &needle, "searchSetNeedle", "needle") else {
            return Ok(0);
        };
        let Some(query_tx) = session_query_channel(id) else {
            throw_session_missing(env, "searchSetNeedle");
            return Ok(0);
        };
        let total = crate::terminal::ghostty_terminal::GhosttyTerminal::search_set_needle_on(
            &query_tx, &needle,
        );
        log::info!("searchSetNeedle: needle={needle:?} matches={total}");
        total as jint
    })
}

/// 选中下一个（`forward = true`）或上一个命中，返回 `[序号, 总数, 行, 起始列, 结束列]`；
/// 无命中、会话不存在、查询超时或断连时返回 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchSelect<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    forward: jboolean,
) -> jintArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let id = session_id as u64;
        let Some(query_tx) = session_query_channel(id) else {
            throw_session_missing(env, "searchSelect");
            return Ok(std::ptr::null_mut());
        };
        let Some(selected) = crate::terminal::ghostty_terminal::GhosttyTerminal::search_select_on(
            &query_tx, forward,
        ) else {
            return Ok(std::ptr::null_mut());
        };
        let values = vec![
            selected.index as jint,
            selected.total as jint,
            selected.matched.row.min(jint::MAX as u32) as jint,
            selected.matched.start_col.min(jint::MAX as u32) as jint,
            selected.matched.end_col.min(jint::MAX as u32) as jint,
        ];
        jint_slice_to_int_array(env, &values)
    })
}

/// 取会话句柄。注册表读锁与**会话锁**都在此释放：两者都可能被 PTY 写入按住数秒
/// （`feed_pty` 的写入面正是这样克隆出去再写），持锁跨越会让销毁会话的写锁饥饿，
/// 渲染线程更不该在注册表**写**锁内排队——只读查找一律走本函数。
pub(super) fn session_handle(session_id: u64) -> Option<Arc<Mutex<Session>>> {
    rlock_session_registry()
        .get(&session_id)
        .map(|entry| entry.session.clone())
}

/// [`Self::session_handle`] 的查询通道版。查询经 500 毫秒有界超时，VT 查询本身
/// 在会话锁外跑。会话不存在时返回 None，由调用方抛出。
pub(super) fn session_query_channel(
    session_id: u64,
) -> Option<flume::Sender<crate::terminal::ghostty_terminal::Query>> {
    Some(
        session_handle(session_id)?
            .lock()
            .terminal()
            .query_channel(),
    )
}
