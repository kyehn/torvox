use super::*;

// ── 命令派发 ───────────────────────────────────────────────────

impl super::GhosttyTerminal {
    /// 处理单条命令，返回后续流程。
    ///
    /// 每族命令一个独立函数：各族的失效规则互不相同（换主题要清行缓存、
    /// 单纯滚动只平移缓存、单元格像素尺寸根本不置脏），合成一个大 `match`
    /// 则每次改动都要在几百行里找落点。
    pub(super) fn apply_command<'term>(
        command: Command,
        config: &RunConfig,
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) -> CommandOutcome {
        match command {
            Command::Write(data) => {
                Self::apply_write(&data, terminal, extractors, state, batch);
                CommandOutcome::Continue
            }
            Command::FlushAck(tx) => {
                // 扣到批尾构建完成才释放：`flush()` 的契约是返回时其前的每条命令
                // ——包括 CellData 推送——都已处理完。测试会调用 flush()
                // 再 receive_cell_data()；提前 ack 会与构建竞争，
                // 把空通道交给它们。
                batch.acks.push(tx);
                CommandOutcome::Continue
            }
            Command::SetTheme(theme) => {
                Self::apply_theme_command(theme, terminal, extractors, state, batch);
                CommandOutcome::Continue
            }
            Command::Resize { rows, cols } => {
                Self::apply_resize(rows, cols, config, terminal, extractors, state, batch)
            }
            Command::SetCellPixelSize {
                cell_width,
                cell_height,
            } => Self::apply_cell_pixel_size(cell_width, cell_height, config, terminal),
            Command::ScrollViewport(delta) => {
                Self::apply_scroll_viewport(delta, terminal, extractors, state, batch);
                CommandOutcome::Continue
            }
            Command::Repaint => {
                // 显式重推：内容可能与上次推送逐字节相同（切会话时终端本身毫无
                // 变化），去重基线必须一并失效，否则这次请求被吞掉。
                state.last_cell_data_push = None;
                batch.dirty = true;
                CommandOutcome::Continue
            }
            Command::Reset => {
                Self::apply_reset(terminal, extractors, state, batch);
                CommandOutcome::Continue
            }
            Command::SetSelection { start, end } => {
                Self::apply_set_selection(terminal, start, end, extractors, state, batch);
                CommandOutcome::Continue
            }
            Command::ClearSelection => {
                Self::apply_clear_selection(terminal, extractors, state, batch);
                CommandOutcome::Continue
            }
            Command::TakeSnapshot { tx } => {
                Self::apply_snapshot(tx, terminal, state);
                CommandOutcome::Continue
            }
            Command::Terminate => CommandOutcome::Terminate,
        }
    }

    /// 处理终端输出。
    fn apply_write<'term>(
        data: &[u8],
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) {
        terminal.vt_write(data);
        // 输出可能发生在同一批的滚动之后：此时待平移量描述的是旧网格，
        // 继续套用会跳过脏行判定、复用过期内容。
        extractors.clear_scroll_shift();
        mark_grid_dirty(&mut state.grid_dirty, &mut state.cached_all_text);
        batch.dirty = true;
        batch.content_changed = true;
    }

    /// 换主题。
    ///
    /// 默认前景/背景参与每个单元的颜色解算，故缓存中的颜色逐行过时。
    fn apply_theme_command<'term>(
        theme: &'static crate::terminal::theme::Theme,
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) {
        state.default_background = Self::color_to_float(theme.palette.background);
        state.default_foreground = Self::color_to_float(theme.palette.foreground);
        log::debug!(
            "SetTheme: {} -> default_background={:?} default_foreground={:?}",
            theme.name,
            state.default_background,
            state.default_foreground
        );
        // 用原生主题 API，而不是手写 OSC 10/11 序列：libghostty-vt 不处理
        // OSC 默认色转义（默认值归内嵌方所有，OSC 4 覆盖由上游处理），
        // 所以 OSC 方案会静默保留内置的 xterm 调色板。
        // 这些 setter 写入的默认色正是上游单元颜色查询的解析基准。
        Self::apply_theme(terminal, &theme.palette);
        mark_grid_dirty(&mut state.grid_dirty, &mut state.cached_all_text);
        extractors.invalidate_row_cache();
        batch.dirty = true;
        batch.content_changed = true;
    }

    /// 调整网格尺寸。
    ///
    /// 尺寸越界直接拒绝而非静默截断（恶意 Kotlin 调用方可能传入 >65535 而
    /// 回绕）。拒绝时结束本批：余下命令留到下一轮唤醒再处理，不在半途变更的
    /// 网格上继续。
    fn apply_resize<'term>(
        rows: u32,
        cols: u32,
        config: &RunConfig,
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) -> CommandOutcome {
        let (Ok(cols), Ok(rows)) = (u16::try_from(cols), u16::try_from(rows)) else {
            log::error!("ghostty_terminal: resize rejected — dimensions out of u16 range");
            return CommandOutcome::EndBatch;
        };
        if let Err(error) = terminal.resize(cols, rows, DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT) {
            // 网格没改成，但 `Session::resize` 已按「命令已入队」把缓存
            // 尺寸发布成新值、并把 `grid_dirty` 清了——同尺寸的下次 resize
            // 因此会短路，两者永久分歧。置位标记让那次重试真正发生。
            config.resize_failed.store(true, Ordering::Release);
            log::error!(
                "ghostty_terminal: resize failed: {error}; grid lags the PTY — next resize retries"
            );
        } else {
            config.resize_failed.store(false, Ordering::Release);
            // 整网重调回填默认几何，XTWINOPS 应答与上游实际一致。
            config
                .cell_size_px
                .0
                .store(DEFAULT_CELL_WIDTH, Ordering::Release);
            config
                .cell_size_px
                .1
                .store(DEFAULT_CELL_HEIGHT, Ordering::Release);
        }
        mark_grid_dirty(&mut state.grid_dirty, &mut state.cached_all_text);
        // 行数变化时行区间整体错位，缓存不可复用。
        // `RowCache::covers` 也会拦下这一情形，这里显式失效
        // 使失效时机与既有 `mark_grid_dirty` 保持同处。
        extractors.invalidate_row_cache();
        batch.dirty = true;
        batch.content_changed = true;
        CommandOutcome::Continue
    }

    /// 更新单元格像素几何（Kitty 放置/鼠标映射用）。
    ///
    /// 同行列重调，网格内容不变，不置脏，避免字体变化引发全量重绘。
    fn apply_cell_pixel_size<'term>(
        cell_width: u32,
        cell_height: u32,
        config: &RunConfig,
        terminal: &mut Terminal<'term, '_>,
    ) -> CommandOutcome {
        let (Ok(cols), Ok(rows)) = (terminal.cols(), terminal.rows()) else {
            return CommandOutcome::EndBatch;
        };
        if cell_width == 0 || cell_height == 0 {
            return CommandOutcome::EndBatch;
        }
        if let Err(error) = terminal.resize(cols, rows, cell_width, cell_height) {
            log::error!("ghostty_terminal: cell resize failed: {error}");
        } else {
            config.cell_size_px.0.store(cell_width, Ordering::Release);
            config.cell_size_px.1.store(cell_height, Ordering::Release);
        }
        CommandOutcome::Continue
    }

    /// 滚动视口（回滚浏览）。
    fn apply_scroll_viewport<'term>(
        delta: isize,
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) {
        // C ABI 返回 void，且越界（已在顶部/底部）会静默钳制。
        // 用滚动条偏移的前后差取实际移动行数，避免在边界上按
        // 请求量平移缓存、把内容推出画面。读取失败时不做平移，
        // 下一帧整帧重建（正确性优先）。
        let before = terminal.scrollbar().map(|bar| bar.offset).ok();
        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(delta));
        let after = terminal.scrollbar().map(|bar| bar.offset).ok();
        let actual_delta = match (before, after) {
            (Some(before), Some(after)) => Some(after as isize - before as isize),
            _ => None,
        };
        // 显式滚动强制重推：视口内容可能逐字节相同（如已在底部），
        // 内容去重会吞掉本次滑动的即时重绘。与 SetSelection 同款失效。
        state.last_cell_data_push = None;
        state.grid_dirty = true;
        batch.dirty = true;
        match actual_delta {
            // 只有纯视口滚动才平移缓存；本批已有内容变更时缓存
            // 相对新网格已过期，整帧重建。
            Some(actual) if !batch.content_changed => {
                if actual != 0 {
                    extractors.note_scroll(actual);
                }
                // actual == 0（已在边界）：没有行移动，缓存仍然有效，
                // 按常规行脏路径重推即可，不必整帧重建。
            }
            _ => extractors.invalidate_row_cache(),
        }
    }

    /// RIS 全重置：恢复初始状态并清空回滚；全部帧缓存失效。
    fn apply_reset<'term>(
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) {
        terminal.reset();
        // 显式清除终端持有的活动选区（幂等，不依赖上游副作用）。
        let _ = terminal.set_selection(None);
        state.cached_snapshot = None;
        state.last_cell_data_push = None;
        mark_grid_dirty(&mut state.grid_dirty, &mut state.cached_all_text);
        extractors.invalidate_row_cache();
        batch.dirty = true;
        batch.content_changed = true;
    }

    /// 安装线性活动选区。
    ///
    /// 终端持有化：选区经 set_selection 安装为终端状态（上游转为跟踪引用，
    /// 随滚动/输出/重排跟随文本）。选区变化改变每行反白，需重推帧。
    fn apply_set_selection<'term>(
        terminal: &Terminal<'term, '_>,
        start: (u32, u32),
        end: (u32, u32),
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) {
        Self::install_selection_impl(terminal, start, end);
        state.last_cell_data_push = None;
        state.grid_dirty = true;
        extractors.invalidate_row_cache();
        batch.dirty = true;
        batch.content_changed = true;
    }

    /// 清除线性活动选区。
    fn apply_clear_selection<'term>(
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) {
        if terminal.set_selection(None).is_err() {
            log::warn!("ghostty_terminal: clear selection failed");
        }
        state.last_cell_data_push = None;
        state.grid_dirty = true;
        extractors.invalidate_row_cache();
        batch.dirty = true;
        batch.content_changed = true;
    }

    /// 取当前视口的网格快照。
    ///
    /// 网格未变则克隆上次建好的缓存，省下逐格 ghostty FFI 重建。
    fn apply_snapshot(
        tx: flume::Sender<Arc<GridSnapshot>>,
        terminal: &Terminal,
        state: &mut CommandState,
    ) {
        let needs_rebuild =
            snapshot_needs_rebuild(state.grid_dirty, state.cached_snapshot.is_some());
        let snapshot = if needs_rebuild {
            let snap =
                Self::build_snapshot(terminal, state.default_foreground, state.default_background);
            let cached = Arc::new(snap);
            state.cached_snapshot = Some(Arc::clone(&cached));
            state.grid_dirty = false;
            cached
        } else {
            // 不变式：`needs_rebuild` 为 false 时 `cached_snapshot` 必为
            // Some（上面第三个子句保证）。违反即渲染管线缺陷，直接崩溃
            // 退出，不回退到空白网格（DESIGN 禁止掩盖错误）。
            state
                .cached_snapshot
                .as_ref()
                .map(Arc::clone)
                .expect("cached_snapshot 在 needs_rebuild=false 时必然存在")
        };
        try_send(
            &tx,
            snapshot,
            "ghostty_terminal: command channel send failed",
        );
    }
}
