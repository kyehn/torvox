use super::*;

/// VT 线程每帧刷新的无锁屏幕状态镜像。
///
/// Android 输入路径在每一次触摸/滚轮事件里都要读它（决定「转发给远端」还是
/// 「滚本地回滚」），因此这些值绝不能靠查询通道获得：一次查询就要等 VT 线程
/// 从解析里抽身应答，终端正刷输出时会把 UI 线程按满一个 `QUERY_TIMEOUT_MS`。
pub(crate) struct ScreenMirrors<'a> {
    /// 备用屏（vim / less / htop）是否活动。
    pub(crate) alt_screen: &'a Arc<AtomicBool>,
    /// 鼠标上报（DECSET 1000/1002/1003）是否启用。
    pub(crate) mouse_tracking: &'a Arc<AtomicBool>,
}

// ── 帧构建 ───────────────────────────────────────────────────

impl super::GhosttyTerminal {
    /// 构建用于渲染的扁平全量 `CellData` 网格。
    ///
    /// 逐格抽取只对上游判脏的行执行，干净行整段复制 [`RowCache`] 的上一帧内容。
    /// 判脏依据是 [`RowIteration::dirty`]，它依赖跨帧复用的 [`RenderState`]——
    /// 本函数的迭代器由 [`Self::run`] 跨帧持有，且每次成功抽完后经
    /// [`Snapshot::clean`] 标记快照已消费，两者缺一则全部行恒为脏。
    ///
    /// 每帧的行数据全部读出后才 `clean`：上游要求整帧渲染完成后才可清脏，
    /// 提前标记会让本帧尚未读完的行被下一帧当作干净行复用。
    ///
    /// 跳过干净快照（网格级）由 `snapshot_needs_rebuild` 在上层完成，与行级判定
    /// 是同一决策的两层：网格未变时本函数根本不执行。
    pub(crate) fn build_cell_data<'alloc, 'cb>(
        terminal: &Terminal<'alloc, 'cb>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        mirrors: &ScreenMirrors<'_>,
        extractors: &mut RenderExtractors<'alloc>,
    ) -> Option<(Vec<CellData>, CursorInfo)> {
        // VT 线程每推一帧就同步这个无锁的备用屏镜像。Android 输入路径在每次
        // 触摸滚动时无锁读取它，以决定手势是否转发给远端
        // （Haven 研究：altScreen 滚轮消费）。
        mirrors.alt_screen.store(
            terminal
                .active_screen()
                .is_ok_and(|screen| screen == libghostty_vt::screen::Screen::Alternate),
            Ordering::Release,
        );
        // 鼠标上报开关同样每帧镜像：输入路径（触摸拖动、外接滚轮）每次事件都要
        // 决定「转发给远端还是滚本地回滚」，走查询通道会把 UI 线程钉在 VT 线程
        // 的一次应答上；读原子量是纯本地判断。
        mirrors.mouse_tracking.store(
            terminal.is_mouse_tracking().unwrap_or(false),
            Ordering::Release,
        );
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        let size = (rows * cols) as usize;

        // 计数句柄须在 `parts()` 的独占借用之前取一份副本：`extractors` 在整个
        // 抽取期间已被可变借用，此处再借即冲突。句柄内部是 `Arc`，副本共享
        // 同一组原子量。
        #[cfg(test)]
        let cache_stats = extractors.cache_stats.clone();
        // 只在本帧消费：平移量描述的是「上一帧缓存 → 本帧网格」的关系，
        // 一旦本帧抽取失败或内容另有变更，后续帧不得继续套用。
        let scroll_shift = extractors.take_scroll_shift();
        let (render_state, row_iter, cell_iter, row_cache) = extractors.parts();

        let snapshot = match render_state.update(terminal) {
            Ok(render_snapshot) => render_snapshot,
            Err(update_error) => {
                log::error!("build_cell_data: render_state.update failed: {update_error}");
                row_cache.invalidate();
                return None;
            }
        };

        let mut row_iter_impl = match row_iter.update(&snapshot) {
            Ok(row_iterator) => row_iterator,
            Err(row_error) => {
                log::error!("build_cell_data: row_iter.update failed: {row_error}");
                row_cache.invalidate();
                return None;
            }
        };

        // 缓存行数与网格不一致（尺寸变更/重置/首次抽取）时不可复用：
        // 区间按行数划分，错位复用会把别行的内容写进本行。
        // 旧帧缓冲经 `take_previous` 移出后即由本帧输出重建，故不留悬空副本。
        let reusable_rows = row_cache.covers(rows);
        let (mut previous_cells, mut previous_ranges) = if reusable_rows {
            row_cache.take_previous()
        } else {
            (Vec::new(), Vec::new())
        };
        // 纯视口滚动：上一帧的每一行内容整体上下移动，平移缓存即可复用，
        // 只需逐格抽取露出的 |delta| 行。没有这一步时每次滚轮事件都会
        // 重走全网格（真机 24×80 约 58ms/帧），滚动明显卡顿。
        let shifted_clean = if reusable_rows
            && let Some(delta) = scroll_shift
            && delta != 0
        {
            Some(RowCache::shift_previous(
                &mut previous_cells,
                &mut previous_ranges,
                delta,
                rows,
            ))
        } else {
            None
        };
        let mut data = Vec::with_capacity(size);
        let mut current_row = 0u32;
        let mut extraction_complete = true;
        // 本次抽取的行级分流结果，仅测试读取；生产构建不声明，零开销。
        #[cfg(test)]
        let (mut reused_rows, mut extracted_rows) = (0u64, 0u64);
        // 无选区时跳过每行一次 selection FFI。读失败按「无选区」作答并出声：
        // 若真读成 false 而终端确有选区，本帧的反白会整帧消失且无处可查。
        let selection_active = match terminal.selection() {
            Ok(selected) => selected.is_some(),
            Err(error) => {
                log::error!("build_cell_data: selection query failed: {error}");
                false
            }
        };

        while let Some(row) = row_iter_impl.next() {
            // 行级脏判定：缓存不可复用（尺寸变更/首次）时全部行逐格抽取；
            // 上游报该行干净时整段复制上一帧该行——内容逐字节相同，省下
            // 一次逐单元 FFI 往返。判读失败按「脏」处理：宁可多抽，
            // 不可拿可能失配的缓存冒充本帧内容。
            let row_dirty = match &shifted_clean {
                // 平移帧：缓存命中行内容就是本帧该行（整体移动），直接复用；
                // 露出行没有旧内容，必须逐格抽取，不能再问上游脏标记——
                // 上游只报告内容变化，不报告视口移动。
                Some(clean) => !clean[current_row as usize],
                None => {
                    !reusable_rows
                        || match row.dirty() {
                            Ok(dirty) => dirty,
                            Err(error) => {
                                log::warn!("build_cell_data: row dirty query failed: {error}");
                                true
                            }
                        }
                }
            };
            if !row_dirty {
                data.extend_from_slice(RowCache::row_slice(
                    &previous_cells,
                    &previous_ranges,
                    current_row,
                ));
                #[cfg(test)]
                {
                    reused_rows += 1;
                }
                current_row += 1;
                continue;
            }
            #[cfg(test)]
            {
                extracted_rows += 1;
            }

            // 行级选区范围（无选区时为 None）：每行一次 FFI，避免逐单元格查询。
            let row_selection = if selection_active {
                row.selection().ok().flatten()
            } else {
                None
            };

            let mut row_data = Vec::with_capacity(cols as usize);
            let mut cell_iter_impl = match cell_iter.update(row) {
                Ok(ci) => ci,
                Err(error) => {
                    log::warn!("build_cell_data: cell_iter.update failed: {error}");
                    // 该行已不完整，不得以缓存版本冒充：整帧作废并清空缓存。
                    row_cache.invalidate();
                    extraction_complete = false;
                    break;
                }
            };

            let mut current_col = 0u32;
            // CellRun 式逐行样式缓存（termlib CellRun.kt）：共享同一 style_id 的
            // 连续单元只解析一次 style/前景/背景；扁平 CellData 输出不变，
            // 但该 run 内跳过了逐单元的 FFI 调用（style/前景/背景）。
            let mut cached_style_id: Option<libghostty_vt::style::Id> = None;
            let mut cached_foreground = default_foreground;
            let mut cached_background = default_background;
            let mut cached_underline = default_foreground;
            let mut cached_flags = 0u32;

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(raw_cell) => raw_cell,
                    Err(_) => {
                        // `raw_cell()` 失败则宽度无从得知（宽度只存在于 raw cell），
                        // 只能按 1 列推进——这是该失败路径的固有上限。
                        Self::push_blank_cell(
                            &mut row_data,
                            default_foreground,
                            default_background,
                            current_row,
                            current_col,
                            1,
                        );
                        current_col += 1;
                        continue;
                    }
                };

                let style_id = raw.style_id().ok();
                let (_style, foreground, background, underline_color, flags) = if style_id.is_some()
                    && style_id == cached_style_id
                {
                    // 同一样式 run：复用已缓存的解析颜色。
                    (
                        None,
                        cached_foreground,
                        cached_background,
                        cached_underline,
                        cached_flags,
                    )
                } else {
                    match cell.style() {
                        Ok(style) => {
                            let foreground = Self::cell_color(cell.fg_color(), default_foreground);
                            let background = Self::cell_color(cell.bg_color(), default_background);
                            let underline = Self::resolve_style_color(
                                terminal,
                                &style.underline_color,
                                foreground,
                            );
                            let style_flags = Self::pack_style_flags(&style);
                            cached_style_id = style_id;
                            cached_foreground = foreground;
                            cached_background = background;
                            cached_underline = underline;
                            cached_flags = style_flags;
                            (Some(style), foreground, background, underline, style_flags)
                        }
                        Err(_) => {
                            // 宽度仍可从 raw cell 读到，必须照真实宽度推进：
                            // 宽字符（中日韩）按 1 列推进会让本行其后每一个单元的
                            // `col` 左移一格，且行长度不再等于 cols。
                            // spacer（None）：两列已由宽字符本身消耗，直接跳过。
                            if let Some(width) = Self::cell_columns(&raw) {
                                Self::push_blank_cell(
                                    &mut row_data,
                                    default_foreground,
                                    default_background,
                                    current_row,
                                    current_col,
                                    width,
                                );
                                current_col += width;
                            }
                            continue;
                        }
                    }
                };

                let codepoint = raw.codepoint().unwrap_or(0);

                // 跳过 Ghostty 为宽字符发出的 spacer 单元（SpacerTail、SpacerHead）。
                // 它们没有内容，若照常推进会让 `current_col` 错位，
                // 导致其后所有单元右移一列。
                // spacer 单元不产出 CellData 条目。
                // `current_col` 保持不变——两列已由宽字符本身消耗。
                let Some(width) = Self::cell_columns(&raw) else {
                    continue;
                };

                let mut grapheme_extra = [0u32; MAX_GRAPHEME_EXTRA];
                if let Ok(graphemes) = cell.graphemes() {
                    for (grapheme_index, &codepoint) in graphemes
                        .iter()
                        .enumerate()
                        .skip(1)
                        .take(MAX_GRAPHEME_EXTRA)
                    {
                        grapheme_extra[grapheme_index - 1] = codepoint as u32;
                    }
                }

                // 终端持有选区的行内反白（经典反白：前景背景互换，与覆盖层旧语义一致；
                // 下划线色同步取反白后的前景，保证选中文本的下划线仍可见）。
                let (foreground, background, underline_color) =
                    if row_selection.as_ref().is_some_and(|range| {
                        let col = current_col as u16;
                        col >= range.start_x && col <= range.end_x
                    }) {
                        (background, foreground, background)
                    } else {
                        (foreground, background, underline_color)
                    };

                row_data.push(CellData {
                    codepoint,
                    width,
                    grapheme_extra,
                    foreground,
                    background,
                    underline_color,
                    flags,
                    row: current_row,
                    col: current_col,
                });
                current_col += width;
            }
            data.extend_from_slice(&row_data);
            current_row += 1;
        }
        // 行迭代在此自然结束：它持有 `RowIterator` 的可变借用与快照的共享借用，
        // 而后续的 `Snapshot::clean` 与读光标都只取 `&self`，两者不冲突——
        // 无需提前 drop（`RowIteration` 不实现 `Drop`，显式 drop 只延长借用而无作用）。

        // 抽取中断：该帧网格不完整，不得把半张网格推给渲染线程，也不留作缓存。
        if !extraction_complete {
            row_cache.invalidate();
            return None;
        }

        // 本帧完整后才刷新缓存与标记快照已消费：
        // · 缓存按 `build_row_ranges` 的行边界划分，行数与内容不符即无法复用；
        // · 上游要求整帧完成后才可 `clean`，提前标记会让下一帧误判未读出的行为干净。
        //
        // 一次整网格拷贝（约 150KB/24×80）换下一帧少走 22 行的逐单元 FFI 往返
        // （实测同一 `RenderState` 跨帧复用时，改一行只判脏该行与末行）：
        // 净收益为正，故此处不做「全干净帧免拷贝」之类的特化——该路径本就
        // 很少进入（`build_cell_data` 由 `batch_dirty`/`cell_data_dirty` 双重守卫），
        // 而特化会引入一条需要单独证明正确的相等性判断。
        row_cache.store(&data, rows);
        if let Err(clean_error) = snapshot.clean() {
            log::warn!("build_cell_data: snapshot clean failed: {clean_error}");
        }
        #[cfg(test)]
        cache_stats.record(reused_rows, extracted_rows);

        let cursor_style = Self::cursor_style_from_snapshot(&snapshot);
        // 光标取视口相对坐标：口径与判据见 [cursor_in_viewport]。
        let (cursor_row, cursor_col, cursor_at_wide_tail, cursor_visible) =
            cursor_in_viewport(&snapshot);
        Some((
            data,
            CursorInfo {
                row: cursor_row,
                col: cursor_col,
                at_wide_tail: cursor_at_wide_tail,
                visible: cursor_visible,
                style: cursor_style,
                scrollback_length: scrollback_len(terminal),
                rows,
                cols,
                kitty_generation: terminal
                    .kitty_graphics()
                    .ok()
                    .and_then(|graphics| graphics.generation().ok())
                    .unwrap_or(0),
                kitty_placement_generation: snapshot.image_placements_generation().unwrap_or(0),
            },
        ))
    }

    /// 把样式属性打包成与 `cell.wgsl` 着色器布局一致的位掩码：
    /// 第 0 位=bold、1=italic、2=reverse、3=underline、
    /// 4=blink、5=strikethrough、6=overline、7=dim、8=double_underline
    /// （blink 会带上，着色器忽略它；留待将来使用）。
    pub(crate) fn pack_style_flags(style: &libghostty_vt::style::Style) -> u32 {
        use crate::terminal::ghostty_terminal::cell_flags;
        let mut flags = 0u32;
        if style.bold {
            flags |= 1 << cell_flags::BOLD;
        }
        if style.italic {
            flags |= 1 << cell_flags::ITALIC;
        }
        if style.inverse {
            flags |= 1 << cell_flags::REVERSE;
        }
        if style.blink {
            flags |= 1 << cell_flags::BLINK;
        }
        if has_visible_underline(style.underline) {
            flags |= 1 << cell_flags::UNDERLINE;
        }
        if style.strikethrough {
            flags |= 1 << cell_flags::STRIKETHROUGH;
        }
        if style.overline {
            flags |= 1 << cell_flags::OVERLINE;
        }
        if style.faint {
            flags |= 1 << cell_flags::FAINT;
        }
        if is_double_underline(style.underline) {
            flags |= 1 << cell_flags::DOUBLE_UNDERLINE;
        }
        flags
    }

    pub(crate) fn build_snapshot(
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
    ) -> GridSnapshot {
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        let size = (rows * cols) as usize;
        let mut cells = Vec::with_capacity(size);

        // 独立的 RenderState 与迭代器：快照路径每次整帧重建，不做行级缓存，
        // 故不复用 `build_cell_data` 跨帧持有的那一套。上游的脏状态按实例保存
        // （见 render.rs 模块文档「dirty tracking」），两套实例互不消费对方的
        // 脏标记——共用会使行级判定读到已被快照路径 `clean` 清掉的状态。
        let (mut render_state, mut row_iter, mut cell_iter) = match create_render_iterators() {
            Some(iterator_bundle) => iterator_bundle,
            None => return GridSnapshot::fallback(rows, cols),
        };

        let snapshot = match render_state.update(terminal) {
            Ok(render_snapshot) => render_snapshot,
            Err(update_error) => {
                log::error!("build_snapshot: render_state.update failed: {update_error}");
                return GridSnapshot::fallback(rows, cols);
            }
        };

        let mut row_iter_impl = match row_iter.update(&snapshot) {
            Ok(row_iterator) => row_iterator,
            Err(row_error) => {
                log::error!("build_snapshot: row_iter.update failed: {row_error}");
                return GridSnapshot::fallback(rows, cols);
            }
        };

        // ── CellIterator 循环 ──
        // 先用 RowIterator 遍历全部可见行，再用 CellIterator 遍历每行的
        // 全部单元。取代了逐单元的 grid_ref。
        while let Some(row) = row_iter_impl.next() {
            let mut cell_iter_impl = match cell_iter.update(row) {
                Ok(cell_iterator) => cell_iterator,
                Err(error) => {
                    log::warn!("build_snapshot: cell_iter.update failed: {error}");
                    break;
                }
            };

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(raw_cell) => raw_cell,
                    Err(error) => {
                        log::warn!("build_snapshot: raw_cell failed: {error}");
                        cells.push(CellSnapshot {
                            foreground: default_foreground,
                            background: default_background,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let style = match cell.style() {
                    Ok(cell_style) => cell_style,
                    Err(error) => {
                        log::warn!("build_snapshot: cell style failed: {error}");
                        cells.push(CellSnapshot {
                            foreground: default_foreground,
                            background: default_background,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let codepoint = raw.codepoint().unwrap_or(0);

                // 快照路径保留占位：与 CellData 跳过 Spacer 不同，快照按网格索引逐格存放，宽字符占位格须保留 width=1 条目以对齐行列。仅测试行使。
                let width = match raw.wide() {
                    Ok(libghostty_vt::screen::CellWide::Wide) => 2,
                    _ => 1,
                };

                let graphemes: Vec<u32> = match cell.graphemes() {
                    Ok(grapheme_slice) if grapheme_slice.len() <= MAX_GRAPHEME_CLUSTERS => {
                        grapheme_slice
                            .iter()
                            .map(|&codepoint| codepoint as u32)
                            .collect()
                    }
                    Ok(grapheme_slice) => grapheme_slice
                        .iter()
                        .take(MAX_GRAPHEME_CLUSTERS)
                        .map(|&codepoint| codepoint as u32)
                        .collect(),
                    Err(_) => vec![codepoint],
                };

                let foreground = Self::cell_color(cell.fg_color(), default_foreground);
                let background = Self::cell_color(cell.bg_color(), default_background);
                let underline_color =
                    Self::resolve_style_color(terminal, &style.underline_color, foreground);

                cells.push(CellSnapshot {
                    codepoint,
                    graphemes,
                    foreground,
                    background,
                    underline_color,
                    bold: style.bold,
                    dim: style.faint,
                    italic: style.italic,
                    underline: has_visible_underline(style.underline),
                    reverse: style.inverse,
                    strikethrough: style.strikethrough,
                    blink: style.blink,
                    hidden: style.invisible,
                    overline: style.overline,
                    double_underline: is_double_underline(style.underline),
                    width,
                });
            }
        }

        // 视口相对的光标坐标（与按视口行存放的单元行一致）：口径见 [cursor_in_viewport]。
        let (cursor_row, cursor_col, _, cursor_visible) = cursor_in_viewport(&snapshot);

        let dirty = vec![true; (rows as usize) * (cols as usize)];

        GridSnapshot {
            rows,
            cols,
            cursor_row,
            cursor_col,
            cursor_visible,
            cursor_style: Self::cursor_style_from_snapshot(&snapshot),
            cells,
            dirty,

            // 标题读失败不静默留空：会话列表会把该会话显示成无名，而空标题与
            // 「应用本来就没设标题」不可区分。
            title: match terminal.title() {
                Ok(title) => String::from_utf8_lossy(title).into_owned(),
                Err(error) => {
                    log::error!("ghostty_terminal: title query failed: {error}");
                    String::new()
                }
            },
            scrollback_length: scrollback_len(terminal),
        }
    }

    pub(crate) fn apply_style_to_snapshot(
        data: &mut CellSnapshot,
        style: &libghostty_vt::style::Style,
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
    ) {
        data.foreground = Self::resolve_style_color(terminal, &style.fg_color, default_foreground);
        data.background = Self::resolve_style_color(terminal, &style.bg_color, default_background);
        // SGR 58 下划线色：未设置时回退到解析后的前景（着色器旧 `deco = foreground` 语义）。
        data.underline_color =
            Self::resolve_style_color(terminal, &style.underline_color, data.foreground);
        data.bold = style.bold;
        data.dim = style.faint;
        data.italic = style.italic;
        data.strikethrough = style.strikethrough;
        data.overline = style.overline;
        data.blink = style.blink;
        data.hidden = style.invisible;
        data.underline = has_visible_underline(style.underline);
        data.double_underline = is_double_underline(style.underline);
        data.reverse = style.inverse;
    }

    /// `default_foreground`/`default_background` 必须与渲染路径
    /// [`Self::build_cell_data`] 收到的是同一对值（由 run 循环经 [`RunConfig`] 初始化、
    /// 经 `SetTheme` 更新）。转储是渲染的对照物：若这里另行从终端反查默认色，两条路径
    /// 一旦分歧，测试就会对着与 GPU 不同的颜色下断言。不设兜底——主题由构造参数给定，
    /// 不存在"查不到"。
    pub(crate) fn build_dumped_grid(
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
    ) -> DumpedGrid {
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        let scrollback_rows = scrollback_len(terminal);

        let snapshot_at = |point: Point| {
            let mut data = CellSnapshot::default();
            if let Ok(point) = terminal.grid_ref(point) {
                if let Ok(cell) = point.cell() {
                    data.codepoint = cell.codepoint().unwrap_or(0);
                }
                if let Ok(style) = point.style() {
                    Self::apply_style_to_snapshot(
                        &mut data,
                        &style,
                        terminal,
                        default_foreground,
                        default_background,
                    );
                }
            }
            data
        };

        let mut visible = Vec::with_capacity((rows * cols) as usize);
        for row in 0..rows {
            for col in 0..cols {
                visible.push(snapshot_at(Point::Viewport(PointCoordinate {
                    x: col as u16,
                    y: row,
                })));
            }
        }

        let mut scrollback = Vec::with_capacity(scrollback_rows as usize);
        for scrollback_row in 0..scrollback_rows {
            let mut row_cells = Vec::with_capacity(cols as usize);
            for col in 0..cols {
                row_cells.push(snapshot_at(Point::History(PointCoordinate {
                    x: col as u16,
                    y: scrollback_row,
                })));
            }
            scrollback.push(row_cells);
        }

        DumpedGrid {
            rows,
            cols,
            visible,
            scrollback,
        }
    }

    /// 向行数据推入空白（码点 0）单元。用于原始 ghostty 数据或样式
    /// 无法解析的单元。
    pub(crate) fn push_blank_cell(
        row_data: &mut Vec<CellData>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        row: u32,
        col: u32,
        width: u32,
    ) {
        row_data.push(CellData {
            codepoint: 0,
            width,
            grapheme_extra: [0; MAX_GRAPHEME_EXTRA],
            foreground: default_foreground,
            background: default_background,
            underline_color: default_foreground,
            flags: 0,
            row,
            col,
        });
    }

    /// 单元占用的列数：`None` 表示 spacer 单元（宽字符的尾/头格），
    /// 它不产出 `CellData`，两列已由宽字符本身消耗。
    pub(crate) fn cell_columns(raw: &libghostty_vt::screen::Cell) -> Option<u32> {
        match raw.wide() {
            Ok(libghostty_vt::screen::CellWide::Wide) => Some(2),
            Ok(
                libghostty_vt::screen::CellWide::SpacerTail
                | libghostty_vt::screen::CellWide::SpacerHead,
            ) => None,
            _ => Some(1),
        }
    }

    /// 把 ghostty 光标可视样式映射为应用层光标样式。
    /// 空心块暂时按实心块渲染（细分形状留待以后）。
    /// 上游将来新增的未知样式回退为块：光标必须始终画出点什么
    /// （DESIGN 的极端崩溃不适用于每帧可见元素）。
    pub(crate) fn cursor_style_from_snapshot(
        snapshot: &libghostty_vt::render::Snapshot<'_, '_>,
    ) -> CursorStyle {
        use libghostty_vt::render::CursorVisualStyle;
        match snapshot.cursor_visual_style() {
            Ok(CursorVisualStyle::Bar) => CursorStyle::Bar,
            Ok(CursorVisualStyle::Underline) => CursorStyle::Underline,
            _ => CursorStyle::Block,
        }
    }

    /// 把单元颜色解析为 `[r, g, b, 1.0]` 浮点值；FFI 报错或颜色透明时
    /// 回退到单元默认值。
    pub(crate) fn cell_color(
        color: Result<Option<libghostty_vt::style::RgbColor>, libghostty_vt::error::Error>,
        default: [f32; 4],
    ) -> [f32; 4] {
        match color {
            Ok(Some(rgb)) => Self::color_to_float(rgb),
            _ => default,
        }
    }

    /// 按当前终端状态重建 CellData 并推给渲染线程的通道（未挂消费者时
    /// 为空操作）。Write/SetTheme/Resize/ScrollViewport 共用的帧推送块
    /// 只有这一个调用点。
    /// 终端网格变化后推送一份新的 cell-data 快照。
    /// [`Self::push_cell_data`] 的薄封装，补齐各命令处理器共用的
    /// 标准 config/状态接线。
    pub(crate) fn refresh_cell_data<'alloc>(
        config: &RunConfig,
        terminal: &libghostty_vt::terminal::Terminal<'alloc, '_>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        last_push: &mut Option<(Vec<CellData>, CursorInfo)>,
        extractors: &mut RenderExtractors<'alloc>,
    ) {
        Self::push_cell_data(
            config.cell_data_tx.as_ref(),
            config.frame_wake.as_ref(),
            &ScreenMirrors {
                alt_screen: &config.alt_screen_active,
                mouse_tracking: &config.mouse_tracking_active,
            },
            terminal,
            default_foreground,
            default_background,
            last_push,
            extractors,
        );
    }

    pub(crate) fn push_cell_data<'alloc>(
        cell_data_tx: Option<&Sender<(Vec<CellData>, CursorInfo)>>,
        frame_wake: Option<&crate::terminal::session::OutputWake>,
        mirrors: &ScreenMirrors<'_>,
        terminal: &Terminal<'alloc, '_>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        last_push: &mut Option<(Vec<CellData>, CursorInfo)>,
        extractors: &mut RenderExtractors<'alloc>,
    ) {
        if let Some(tx) = cell_data_tx
            && let Some(data) = Self::build_cell_data(
                terminal,
                default_foreground,
                default_background,
                mirrors,
                extractors,
            )
        {
            // 自上次推送以来什么都没变时跳过发送：循环定时器（50ms 空闲唤醒）
            // 会在终端毫无活动时重跑这条路径，而渲染线程把收到的每批都当作
            // 新数据 → 每帧全量重绘。把 bytemuck Pod 单元按原始字节（外加光标）
            // 比较，在 24x80 网格上约 150KB memcmp——微秒级，与它避免的
            // 全量渲染开销相比可忽略。
            let unchanged = match last_push {
                Some((last_cells, last_cursor)) => {
                    *last_cursor == data.1
                        && bytemuck::cast_slice::<CellData, u8>(last_cells)
                            == bytemuck::cast_slice::<CellData, u8>(&data.0)
                }
                None => false,
            };
            if unchanged {
                return;
            }
            // 去重基线只在帧真正入队后推进：通道满时本帧被丢弃，若同时推进基线，
            // 下一帧同一内容会命中 unchanged 而不再补发，屏幕将永久停留在旧帧。
            match tx.try_send((data.0.clone(), data.1)) {
                Ok(()) => {
                    *last_push = Some(data);
                    // 帧已可被取走：叫醒可能正睡在空闲闭锁上的渲染线程。
                    if let Some(wake) = frame_wake {
                        wake();
                    }
                }
                Err(error) => warn_drop_throttled(&error),
            }
        }
    }
}
