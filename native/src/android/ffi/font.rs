use super::query::session_query_channel;
use super::render::string_to_jstring;
use super::session::read_jni_string;
use super::*;

// ── 字体与主题 ──────────────────────────────────────────────────
/// 空族文件列表：字族已在渲染字体库中时无需按需装入。
const NO_FAMILY_FILES: &[String] = &[];

/// 返回字体库的族名列表（fonts.xml 声明的文件集 + 用户投放目录）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listFontFamilies<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jobjectArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        // 字体族列表：设备上取进程级族索引——它与渲染状态无关，MUST 在渲染状态锁外
        // 构造：首次构建索引要读完全部声明字体文件（真机实测 127ms），持锁构造会
        // 把渲染线程冻结同样时长（打开字体选择器时可见）。
        #[cfg(target_os = "android")]
        let families = crate::render::font::font_db::family_display_names();
        // 宿主构建的列表取自渲染字体库，须持锁取。
        #[cfg(not(target_os = "android"))]
        let families = {
            let state = render_state_mut();
            let Some(render_state) = state.as_ref() else {
                return Ok(std::ptr::null_mut());
            };
            render_state.font_pipeline.list_monospace_fonts()
        };

        let string_class = env.find_class(jni_str!("java/lang/String"));
        let Ok(string_class) = string_class else {
            return Ok(std::ptr::null_mut());
        };
        let array = env.new_object_array(families.len() as jsize, string_class, JObject::null());
        let Ok(array) = array else {
            return Ok(std::ptr::null_mut());
        };
        // 元素初值为 null：任一 `new_string` / `set_element` 失败都会留下 null，
        // 而 Kotlin 侧声明是 `Array<String>`，null 元素会在字体选择器里变成 NPE。
        // 故失败即整体作废，由 Kotlin 侧按「无数据」处理。
        for (index, family) in families.iter().enumerate() {
            let Ok(element) = env.new_string(family) else {
                log::error!("listFontFamilies: cannot encode family {family:?}, aborting list");
                return Ok(std::ptr::null_mut());
            };
            if let Err(error) = array.set_element(env, index, &element) {
                log::error!(
                    "listFontFamilies: cannot store family {family:?}, aborting list: {error}"
                );
                return Ok(std::ptr::null_mut());
            }
        }
        array.into_raw()
    })
}

/// 返回默认字体族名。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getDefaultFontName<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(name) =
            with_render_state(|render_state| render_state.font_pipeline.default_font_name())
        else {
            return Ok(std::ptr::null_mut());
        };
        string_to_jstring(env, &name)
    })
}

/// 搜索查询串长度上限（原生为唯一真源，UI 侧据此截断输入）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchQueryMaxChars<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jint {
    jni_export_guard!(unowned_env, -1, |_env| {
        GhosttyTerminal::MAX_SEARCH_QUERY_CHARS as jint
    })
}

/// 主题目录与全部调色板的一次性打包（布局见 [`crate::terminal::theme::pack_catalog`]）。
///
/// 一次同步调用取代「逐个标识取调色板」的 `1 + N` 次往返：目录在构建期即固定，
/// 进程内不变，而设置页要一次显示全部主题。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getThemeCatalog(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let packed = crate::terminal::theme::pack_catalog();
        match env.byte_array_from_slice(&packed) {
            Ok(array) => array.into_raw(),
            Err(error) => {
                log::error!("getThemeCatalog: byte_array_from_slice failed: {error}");
                std::ptr::null_mut()
            }
        }
    })
}

/// 返回字体信息的打包字节（布局见本文件的 `pack_font_info`）。渲染状态尚未初始化时为 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getFontInfo(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
) -> jbyteArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(packed) = with_render_state(|render_state| {
            pack_font_info(&render_state.font_pipeline.font_info())
        }) else {
            return Ok(std::ptr::null_mut());
        };
        match env.byte_array_from_slice(&packed) {
            Ok(array) => array.into_raw(),
            Err(error) => {
                log::error!("getFontInfo: byte_array_from_slice failed: {error}");
                std::ptr::null_mut()
            }
        }
    })
}

/// 字体信息的打包头部字节数：标志 4 + 三个 `f32` 12 + 名称数 2 + CJK 名称数 2。
pub(super) const FONT_INFO_HEADER_BYTES: usize = 20;

/// 字体信息的跨语言打包布局（Kotlin 侧对应 `FontInfo` 的解码）。
///
/// ```text
/// u32  flags         bit0 主字体等宽；CJK 状态取判别序号（见 CjkFallbackState）
/// f32 ×3             单元格宽高（物理像素）、逻辑字号（sp）
/// u16  名称个数       主字体 + CJK 回退族 + 其他被使用族
/// u16  CJK 名称数     紧随其后的一段是 CJK 回退族，其余为其他被使用族
/// 名称 × n            UTF-8、NUL 分隔，先主字体族名（无主字体则空串），
///                     再 CJK 回退族名，最后其他被使用族名
/// ```
pub(crate) fn pack_font_info(info: &crate::render::font::pipeline::FontInfo) -> Vec<u8> {
    // CJK 状态占 bit1 起（bit0 留给等宽标记），与 Kotlin 的 `CjkState.entries[flags ushr 1]` 对齐。
    let mut flags = (info.cjk_state as u32) << 1;
    if info.active.as_ref().is_some_and(|active| active.monospaced) {
        flags |= 1;
    }
    let mut names = info.cjk_families.clone();
    names.extend(info.other_families.iter().cloned());
    names.insert(
        0,
        info.active
            .as_ref()
            .map_or_else(String::new, |active| active.name.clone()),
    );
    let mut packed = Vec::with_capacity(
        FONT_INFO_HEADER_BYTES + names.iter().map(|name| name.len() + 1).sum::<usize>(),
    );
    packed.extend_from_slice(&flags.to_le_bytes());
    for value in [info.cell_width_px, info.cell_height_px, info.font_size] {
        packed.extend_from_slice(&value.to_le_bytes());
    }
    // 计数用 u16 承载：族名数量一旦超出就明确报错，不静默回绕
    // （回绕会让 Kotlin 侧的「名称数与实际个数相符」校验抛异常，
    //  表现为设置页字体信息崩溃）。panic 由 JNI 守卫转成 Java 异常。
    let name_count =
        u16::try_from(names.len()).expect("font family count exceeds the u16 wire field");
    let cjk_count = u16::try_from(info.cjk_families.len())
        .expect("cjk family count exceeds the u16 wire field");
    packed.extend_from_slice(&name_count.to_le_bytes());
    packed.extend_from_slice(&cjk_count.to_le_bytes());
    for name in &names {
        packed.extend_from_slice(name.as_bytes());
        packed.push(0);
    }
    packed
}

/// 清除渲染器下一帧的搜索高亮区间。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clearSearchHighlights(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
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
    })
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
pub(super) fn build_search_highlights(
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
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_applySearchHighlights(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    scroll_offset: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let query_tx = match session_query_channel(session_id as u64) {
            Some(query_tx) => query_tx,
            None => {
                log::error!("applySearchHighlights: session {session_id} not found");
                return Ok(());
            }
        };
        let highlights =
            crate::terminal::ghostty_terminal::GhosttyTerminal::search_viewport_highlights_on(
                &query_tx,
            );
        // 视口顶行的绝对行号。`scroll_offset` 与 VT 视口是同一来源（宿主统一滚动），
        // 上游默认的 `select_next` 自动滚动已关闭，二者不会各走各的。
        let viewport_top = highlights.scrollback_lines as i32 - scroll_offset;
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
    })
}

/// 设置下一渲染帧的活动文本选区。
///
/// 终端持有选区安装口：坐标为绝对网格行（row 0 = 回滚顶部），与控制柄逻辑一致。
/// 设置当前文本选区（绝对网格行列，0 = 回滚顶部）；高亮颜色由终端侧按主题调色板烘焙，本通道不传颜色参数。
/// 选区经 `GhosttyTerminal::set_selection` 安装为终端状态（跟踪引用，随滚动/
/// 输出/重排跟随文本），高亮由 VT 线程按行级选区反白直接烘焙进 CellData；
/// 本层不再存储单元格位置，仅透传。`hasSelection=false` 清除选区。
/// 选区恒为线性（块选通道已随 mode 一并移除）。
///
/// 返回 true 表示本次写入已生效（或无需写入）。会话锁被 PTY 写入按住时返回 false 且
/// **不阻塞**：调用方（渲染循环）保留上一次已提交的值，下一帧重试。拖动选区时每帧都会
/// 调用本函数，阻塞在这里等于把每帧拖到 PTY 输出的时长（真机实测单帧 100ms+）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSelection(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
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

/// 按标识应用主题：终端默认色与调色板进 VT 线程，光标色与搜索高亮色供渲染态取用。
/// 标识未知即抛异常——主题真源在 [`crate::terminal::theme`]，Kotlin 无从构造清单外的主题。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setTheme(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    session_id: jlong,
    theme_name: JString,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let id = session_id as u64;
        let Some(name) = read_jni_string(env, &theme_name, "setTheme", "theme name") else {
            return Ok(());
        };
        let Some(entry) = theme::by_name(&name) else {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                JNIString::from(format!("setTheme: unknown theme {name}")),
            );
            return Ok(());
        };
        let background = entry.palette.background;
        // 基底先落，覆盖通道（`setPaletteOverride`）随后就以它为底重算：
        // 换主题时若还留着上一次的覆盖，那覆盖也必须按新主题重算一遍。
        *super::ACTIVE_THEME.write() = Some(entry);
        {
            let registry = rlock_session_registry();
            let Some(session_entry) = registry.get(&id) else {
                throw_session_missing(env, "setTheme");
                return Ok(());
            };
            let session = session_entry.session.lock();
            session.terminal().set_theme(entry);
            // 先让 VT 线程把换主题后的单元数据推出来，再标记渲染脏：
            // 否则渲染线程可能先用旧单元数据 + 新背景色重绘一帧，造成新旧底色混屏。
            session.terminal().flush();
        }
        // 全局锁序：注册表读锁 → 会话锁 → RENDER_STATE。`render_inner` 在阶段 3
        // 持 RENDER_STATE 反向取注册表与会话锁，若此处嵌套持有即构成环形等待
        // （parking_lot 互斥锁不可重入），渲染线程与设置应用线程将同时冻结。
        //
        // 单元格着色器的 Fix F 判定会把每个单元格的背景与 `uniforms.default_background`
        // 比对，后者取自 `Renderer::background`。不同步时终端主题与渲染器默认值不一致，
        // `is_default_background` 恒为 false，背景色判定失效。
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.palette = entry.palette.clone();
            render_state.renderer.set_background_color(background);
            // 主题色存于实例缓存，闲时无新 CellData 则颜色过期，一并重绘。
            render_state.renderer.cell_cache = None;
            // VT 线程已推送换主题后的单元数据：标记脏并整幅重绘。
            render_state.dirty.store(true, Ordering::Relaxed);
            render_state.full_repaint = true;
        }
        drop(state);
        log::info!("setTheme: session {id} theme={name}");
    })
}

/// 覆盖某会话的调色板（`~/.termux/colors.properties`）。
///
/// `colors` 为 19 个 `0x00RRGGBB`：背景、前景、光标、十六个 ANSI 色（0..15）。
/// 长度不符即抛 `IllegalArgumentException`——静默截断会让颜色整体错位一格，
/// 而错位表现为「某些颜色不对」，无从定位。
///
/// **`-1` 表示该槽位不覆盖**，取当前基底主题的原色。`colors.properties` 通常只写
/// 其中几个键，让 Kotlin 为没写的槽位补值就得让 Kotlin 也持有主题色副本（含光标色，
/// 而光标色根本不在主题目录的打包里）——补不对就是「改了背景，光标跟着变白」。
/// 由原生按基底补齐，Kotlin 只管「写了什么」。
///
/// 选区底色不在覆盖范围内（colors.properties 没有该键），沿用基底主题的值。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setPaletteOverride<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    colors: jintArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        const EXPECTED: usize = 19;
        /// 该槽位不覆盖，沿用基底主题原色。
        const NO_OVERRIDE: jint = -1;
        if colors.is_null() {
            throw_illegal_argument(env, "setPaletteOverride: colors is null");
            return Ok(());
        }
        // SAFETY: `colors` 是 JNI 参数，本次调用期间有效；from_raw 只包装不取所有权。
        let array = unsafe { jni::objects::JIntArray::from_raw(env, colors) };
        let length = match array.len(env) {
            Ok(length) => length,
            Err(error) => {
                throw_illegal_argument(
                    env,
                    &format!("setPaletteOverride: unreadable array ({error})"),
                );
                return Ok(());
            }
        };
        if length != EXPECTED {
            throw_illegal_argument(
                env,
                &format!("setPaletteOverride: expected {EXPECTED} colours, got {length}"),
            );
            return Ok(());
        }
        let mut values = [NO_OVERRIDE; EXPECTED];
        if let Err(error) = array.get_region(env, 0, &mut values) {
            throw_illegal_argument(env, &format!("setPaletteOverride: copy failed ({error})"));
            return Ok(());
        }
        let mut overrides = [None; crate::terminal::theme::OVERRIDE_SLOTS];
        for (slot, value) in overrides.iter_mut().zip(values.iter()) {
            *slot = if *value == NO_OVERRIDE {
                None
            } else {
                Some(libghostty_vt::style::RgbColor {
                    r: ((*value >> 16) & 0xFF) as u8,
                    g: ((*value >> 8) & 0xFF) as u8,
                    b: (*value & 0xFF) as u8,
                })
            };
        }
        let palette = super::active_theme().palette.with_overrides(&overrides);
        let background = palette.background;
        let id = session_id as u64;
        {
            let registry = rlock_session_registry();
            let Some(session_entry) = registry.get(&id) else {
                throw_session_missing(env, "setPaletteOverride");
                return Ok(());
            };
            let session = session_entry.session.lock();
            session.terminal().set_colors(palette.clone());
            // 与 setTheme 同序：先让 VT 线程推出新单元数据，再标记渲染脏。
            session.terminal().flush();
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.palette = palette;
            render_state.renderer.set_background_color(background);
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
            render_state.full_repaint = true;
        }
        drop(state);
        log::info!("setPaletteOverride: session {id} applied");
    })
}

/// 抛 `IllegalArgumentException`。错误路径可能已在 JNI 上挂起异常，重复抛会
/// 覆盖首个原因；故仅在无挂起异常时使用。
fn throw_illegal_argument(env: &mut Env, message: &str) {
    let _ = env.throw_new(
        jni_str!("java/lang/IllegalArgumentException"),
        JNIString::from(message.to_string()),
    );
}

/// 开启/关闭持续的 surface 级取纹理失败（见 `Renderer::set_surface_loss_injected_for_test`）。
///
/// 仅供仪器化用例复刻「BufferQueue 被遗弃」以验证自愈：无生产调用方；关闭后立刻
/// 恢复真实取纹理。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSurfaceLossInjected<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    _session_id: jlong,
    injected: jboolean,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, false, |_env| {
        let mut state = render_state_mut();
        match state.as_mut() {
            Some(render_state) => {
                render_state
                    .renderer
                    .set_surface_loss_injected_for_test(injected);
                true
            }
            None => false,
        }
    })
}

/// 暂停/恢复渲染器（如设置界面打开期间或 surface 已销毁时）。Rust 渲染器已在
/// `render_frame` 中检查 `render_paused`；本 JNI 导出补上缺失的传输通道。
///
/// 暂停标志挂在**唯一的全局渲染器**上，与会话无关，故不收会话 id：调用方
/// 曾经按会话逐个调用（每次都要抢被整帧 GPU 工作持有的渲染状态锁），
/// 而零会话时又永远清不掉标志——下一个会话会渲染纯黑。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRenderPaused(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    paused: jboolean,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.renderer.set_render_paused(paused);
            // 恢复时强制全量重绘：暂停期消费的 New 帧永不呈现
            // （`render_frame` 直接返回），且 `last_frame` 已是新数据。
            // 只置 `dirty=true` 会走 Idle 稀疏路径（光标行 + 高亮行），暂停期写入的
            // 内容行不在 bands 内、被 Load 残留覆盖——“隐藏后仍不可见、滑动后部分
            // 出现”。故走 `frame_invalidated` 全量路径。
            if !paused {
                render_state.renderer.frame_invalidated = true;
            }
            log::info!("setRenderPaused: paused={}", paused);
        }
    })
}

/// 设置渲染器字体管线的字体族；找到该族时返回 true。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontFamily(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    family: JString,
) -> jboolean {
    jni_export_guard!(&mut unowned_env, JNI_FALSE, |env| {
        let Some(family_str) = read_jni_string(env, &family, "setFontFamily", "family") else {
            return Ok(JNI_FALSE);
        };
        // 字族是否已在渲染字体库中：常驻三族与用户选文件装入的面命中这条快径，
        // 无需族文件列表——族索引也就不会被构建。
        let family_resident = {
            let state = render_state_mut();
            let Some(render_state) = state.as_ref() else {
                return Ok(JNI_FALSE);
            };
            render_state.font_pipeline.family_resident(&family_str)
        };
        // 族文件列表 MUST 在渲染状态锁外解析：解析要走族索引（首次构建真机实测
        // 127ms），持锁解析会把渲染线程冻结同样时长。
        let family_files = if family_resident {
            NO_FAMILY_FILES
        } else {
            crate::render::font::font_db::family_files(&family_str).unwrap_or(NO_FAMILY_FILES)
        };
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(JNI_FALSE);
        };
        let previous = render_state.font_pipeline.font_id;
        let found = render_state
            .font_pipeline
            .set_font_family(&family_str, family_files);
        let changed = render_state.font_pipeline.font_id != previous;
        if changed {
            // 字体源变化：实例缓存的图集 UV 与行高全部过期，同尺寸下仍判兼容，必须整库丢弃并重绘。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
            log::info!("setFontFamily: {family_str} found={found} changed={changed}");
        } else {
            log::debug!("setFontFamily: {family_str} found={found} changed={changed}");
        }
        if found { JNI_TRUE } else { JNI_FALSE }
    })
}

/// 原生接受的光栅缩放（sp→px 系数）区间。**唯一真源**。
///
/// 同一区间此前在两端各存一份字面量——Rust 的 `setRasterScale` 守卫与 Kotlin 的
/// `coerceSpToPxScale` 钳位——互相只由注释绑定。它们漂移的后果与字号上界那次
/// 完全同型：Kotlin 用被原生拒收的系数算出 `FONT_SIZE_MAX_PX / 系数` 的字号上界，
/// 而原生静默拒收该系数（记 error），滑块上界与实际渲染重新脱节。故守卫与对外
/// 查询（[Java_terminal_emulator_bridge_NativeBridge_getRasterScaleRange]）同取此值。
pub(super) const RASTER_SCALE_MIN: f32 = 0.5;
pub(super) const RASTER_SCALE_MAX: f32 = 8.0;

/// 缩放是否落在原生接受区间内。守卫与对外查询同走这一处判定，故「Kotlin 钳到的
/// 区间」与「原生接受的区间」在结构上就是同一个区间，不依赖两处字面量对齐。
pub(super) fn is_raster_scale_acceptable(scale: f32) -> bool {
    (RASTER_SCALE_MIN..=RASTER_SCALE_MAX).contains(&scale)
}

/// 原生接受的最大字号（sp）：字形位图必须放得进图集，故上界由图集边长推导。
///
/// 这是**唯一**的真实约束；用户可选区间由 Kotlin 侧 `SettingsRepository.fontSizeMaxSp`
/// （对标 Termux 的 256px 上限）给出，本值必须始终不小于它，否则滑块能划到的字号会被
/// 原生拒绝，用户看到的就是「设置条范围与实际可设置范围不一致」。
/// `font_size_cap_tests` 钉住这一关系。
///
/// 边长取自**管线自身**（[FontPipeline::atlas_width]，即实际分配的那张图集）而非本模块
/// 的 `ATLAS_SIZE` 常量：后者只是初始化时的入参，两者一旦不同步，「由图集推导」就成了
/// 假话，上界会悄悄偏到另一个尺寸上。
pub(super) fn font_size_cap_sp(atlas_edge_px: f32, raster_scale: f32) -> f32 {
    // 系数已由 `FontPipeline::get_raster_scale` 钳到 ≥ EPSILON，此处不再重复。
    atlas_edge_px / raster_scale
}

/// 字号是否落在原生可接受区间内。
pub(super) fn is_font_size_selectable(size_sp: f32, cap_sp: f32) -> bool {
    size_sp.is_finite() && size_sp > 0.0 && size_sp <= cap_sp
}

/// 设置字号（单位为十分之一 sp，与 Kotlin 滑块一致）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontSizeInPlace(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    size_tenths: jint,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        let size = (size_tenths as f32) / 10.0;
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return Ok(());
        };
        // 用户可选区间只有一处定义：`SettingsRepository.fontSizeMaxSp`（Termux 的
        // 256px 上限换算）。此前这里另有一份 `4.0..=100.0` 的魔数守卫，且对超限值
        // **静默丢弃**——Kotlin 侧那份重复常量一旦漂移，用户拖到的字号就被原生悄悄
        // 丢掉，表现正是「设置条范围和实际可设置范围不一致」。守卫上界改由图集边长
        // 推导（唯一真实约束），越界记错误日志而非无声忽略。
        let max_size = font_size_cap_sp(
            render_state.font_pipeline.atlas_width as f32,
            render_state.font_pipeline.get_raster_scale(),
        );
        if !is_font_size_selectable(size, max_size) {
            // warn 而非 error：捏合缩放的 preview 路径以动画帧率调用本函数，
            // 一次越界会刷出每帧一条 error，把真正的错误日志淹掉。
            log::warn!(
                "setFontSizeInPlace: 字号 {size} 越界（合法区间 (0, {max_size}] sp，字形位图必须放进图集）"
            );
            return Ok(());
        }
        // 同值跳过：手势 preview 高频推送同一字号时不清图集，
        // 避免每帧重光栅 ASCII + 丢实例缓存（缩放撕裂/卡顿）。
        if (render_state.font_pipeline.font_size - size).abs() < f32::EPSILON {
            return Ok(());
        }
        let (cell_width, cell_height) = render_state.font_pipeline.set_font_size_in_place(size);
        // 置脏标志：字号变化必须重绘，即便终端空闲（字形度量已变 → 缓存帧过期）。
        // 同时丢弃实例缓存：干净行存的是旧图集 UV 与旧行高，同网格尺寸仍判兼容，
        // 不丢则丢字/错位/行高混杂（压扁·撕裂）。
        render_state.renderer.cell_cache = None;
        render_state.dirty.store(true, Ordering::Relaxed);
        log::info!(
            "setFontSizeInPlace: {} -> cell {cell_width:.1}x{cell_height:.1}",
            size_tenths
        );
    })
}

/// 原生接受的光栅缩放区间 `[min, max]` → JNI `FloatArray`（原生为唯一真源）。
///
/// Kotlin 侧的 `coerceSpToPxScale` 必须钳到本区间：字号的 sp→px 换算
/// （`SettingsRepository.fontSizeMaxSp`）以该系数为分母，系数一旦落在区间外，
/// 算出的字号上界就不再对应原生真实的光栅尺度。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getRasterScaleRange<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jfloatArray {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let bounds = [RASTER_SCALE_MIN, RASTER_SCALE_MAX];
        let Ok(array) = env.new_float_array(bounds.len()) else {
            return Ok(std::ptr::null_mut());
        };
        if array.set_region(env, 0, &bounds).is_err() {
            return Ok(std::ptr::null_mut());
        }
        array.into_raw()
    })
}

/// 设置字体光栅化缩放（设备像素密度）。字形位图按 `font_size * raster_scale`
/// 光栅化，使高密度屏上的文字保持锐利。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRasterScale(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    scale: jfloat,
) {
    jni_export_guard!(&mut unowned_env, (), |_env| {
        // 越界记错误日志而非静默丢弃：Kotlin 侧 `coerceSpToPxScale` 已钳到同区间
        // （该区间经 `getRasterScaleRange` 从这里取，二者是同一常量），两端常量
        // 漂移时必须在此留痕，否则字号上限换算与实际光栅尺度悄悄脱节。
        if !is_raster_scale_acceptable(scale) {
            log::error!(
                "setRasterScale: 缩放 {scale} 越界（合法区间 {RASTER_SCALE_MIN}..={RASTER_SCALE_MAX}）"
            );
            return Ok(());
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            // 缩放只落在字体管线上：字形位图按 `font_size*scale` 光栅化，
            // 单元几何（`cell_builder`）与 bearing 也按同一 scale 算，着色器一律
            // 1:1 采样。渲染器侧曾有一个同名的 uniform 字段，但它从不被着色器读取
            // （再乘一次反而会把字形缩回逻辑像素），故已删除而非同步两处。
            render_state.font_pipeline.set_raster_scale(scale);
            // 位图全部重光栅：旧 UV 指向错误图块，丢缓存并重绘。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    })
}

/// 把自定义字体文件载入渲染器的字体库。返回文件中的首个字体族名，失败时 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_loadFontFile<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    _session_id: jlong,
    path: JString<'local>,
) -> jstring {
    jni_export_guard!(&mut unowned_env, std::ptr::null_mut(), |env| {
        let Some(path_str) = read_jni_string(env, &path, "loadFontFile", "path") else {
            return Ok(std::ptr::null_mut());
        };
        // 自定义字体加载是 Android 专属特性（带额外路径的字体库仅 Android 有；
        // 桌面构建使用系统字体）。其他目标上拒绝加载。
        #[cfg(target_os = "android")]
        let family = {
            let mut state = render_state_mut();
            let Some(render_state) = state.as_mut() else {
                return Ok(std::ptr::null_mut());
            };
            // 直接装进当前字体库：文件路径已知，另起一个库探测族名只是把同一个
            // 文件解析两遍。装入后族已在库内，按族名选中不必枚举全部字体文件
            // （真机实测枚举 187 个族要 127ms）。
            let Some(family) = render_state
                .font_pipeline
                .load_font_file(std::path::Path::new(&path_str))
            else {
                log::warn!("loadFontFile: no family name in {path_str}");
                return Ok(std::ptr::null_mut());
            };
            // 登记到字体列表的扫描路径：追加而非覆盖，用户字体目录已在此前
            // 注册，覆盖会丢掉目录内字体。
            crate::render::font::font_db::add_extra_font_path(std::path::PathBuf::from(&path_str));
            let previous = render_state.font_pipeline.font_id;
            // 族刚装进库中：无需族文件列表（族索引不因装载字体文件而构建）。
            if !render_state
                .font_pipeline
                .set_font_family(&family, NO_FAMILY_FILES)
            {
                // 丢弃它会让设置页显示新字体名而终端仍用管线的默认字体渲染，
                // 且无任何日志（对比 setFontFamily 导出：同一结果在此被上报为 false）。
                log::error!("loadFontFile: cannot apply family {family:?} from {path_str}");
                return Ok(std::ptr::null_mut());
            }
            if render_state.font_pipeline.font_id != previous {
                // 字体源变化：实例缓存的图集 UV 与行高全部过期，同尺寸下仍判兼容，
                // 必须整库丢弃并重绘。
                render_state.renderer.cell_cache = None;
                render_state.dirty.store(true, Ordering::Relaxed);
            }
            family
        };
        #[cfg(not(target_os = "android"))]
        let family: String = {
            log::warn!("loadFontFile: unsupported on this target");
            String::new()
        };
        if family.is_empty() {
            return Ok(std::ptr::null_mut());
        }
        log::info!("loadFontFile: {} -> family {family}", path_str);
        string_to_jstring(env, &family)
    })
}

/// 设置系统 locale（BCP 47，如 `zh-CN`）：决定 fonts.xml 里选哪个区域回退族。
///
/// 进程启动时 Kotlin 就会调一次（此时还没有渲染管线），故 locale 同时写入渲染层
/// 进程级静态供 `load_font_database` 取用；管线已存在时
/// 再推给它并作废同 UV 缓存（回退族变化会改变字形来源）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSystemLocale(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    locale: JString,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        let Some(locale_str) = read_jni_string(env, &locale, "setSystemLocale", "locale") else {
            return Ok(());
        };
        log::info!("setSystemLocale: {locale_str}");
        // 管线创建前的区域决策读渲染层静态（与 setExtraFontPaths 同一门控惯例）。
        #[cfg(target_os = "android")]
        crate::render::font::font_db::set_current_locale(locale_str.clone());
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.font_pipeline.set_system_locale(&locale_str);
            // CJK 回退排序变化会改变字形来源，同 UV 缓存过期。
            render_state.renderer.cell_cache = None;
            render_state.dirty.store(true, Ordering::Relaxed);
        }
    })
}

/// 登记额外的字体目录/文件（应用私有字体目录）：供字体列表枚举（`family_index`）
/// 与选中时按需装入（`load_family`）使用；渲染常驻字体库不读这些路径
/// （见 `load_font_database`），故不重建管线——重建会把主字体族与光栅缩放
/// 一并复位（新建管线回到默认族、缩放 1.0），用户看到字体与字号突然改变。
///
/// # Safety
/// 仅由 JVM 经 JNI 调用，`paths` 须为本次调用期间有效的 `String[]`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setExtraFontPaths(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass,
    _session_id: jlong,
    paths: jobjectArray,
) {
    jni_export_guard!(&mut unowned_env, (), |env| {
        #[cfg(target_os = "android")]
        {
            // 从 Java 的 `String[]` 解析。任一项解码失败即抛异常并整体放弃：
            // 异常待决期间继续调 JNI 是未定义行为（JNI 规范只允许极少数调用），
            // 且本次调用的结果本就被该异常作废——继续读剩余项毫无意义。
            let mut path_list: Vec<std::path::PathBuf> = Vec::new();
            // SAFETY: `paths` 是 JNI 方法参数，JVM 运行时保证其在本次调用期间有效
            // （与 `feed_pty_inner` 同一模式）。
            let array = unsafe { jni::objects::JObjectArray::<JString>::from_raw(env, paths) };
            // 读长度失败不得当成空数组：本次调用会**替换**已登记的路径，空列表等于
            // 把用户的 `.termux/fonts` 静默注销、字体列表随之缩水，且无任何报错。
            let count = match array.len(env) {
                Ok(count) => count,
                Err(error) => {
                    let message = jni::strings::JNIString::from(format!(
                        "setExtraFontPaths: cannot read array length: {error}"
                    ));
                    let _ = env.throw_new(jni_str!("java/lang/RuntimeException"), &message);
                    return Ok(());
                }
            };
            for font_index in 0..count {
                let Some(item) = array.get_element(env, font_index).ok() else {
                    log::warn!("setExtraFontPaths: 读不到第 {font_index} 个元素，放弃");
                    return Ok(());
                };
                let Some(text) = read_jni_string(env, &item, "setExtraFontPaths", "paths[]") else {
                    return Ok(());
                };
                path_list.push(std::path::PathBuf::from(text));
            }
            crate::render::font::font_db::set_extra_font_paths(path_list);
            log::info!("setExtraFontPaths: registered extra font paths");
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (env, paths);
            log::warn!("setExtraFontPaths: unsupported on this target");
        }
    })
}
