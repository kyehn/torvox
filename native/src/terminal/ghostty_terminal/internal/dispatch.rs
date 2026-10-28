use super::*;

// ── 查询应答 ─────────────────────────────────────────────────

impl super::GhosttyTerminal {
    /// 处理单条查询。上游 `select_*` 查询会在查询路径内安装选区；成功安装时返回
    /// true，由 run 循环按 `Command::SetSelection` 同款规则失效行缓存并重推帧。
    pub(super) fn process_query<'term>(
        query: Query,
        terminal: &mut Terminal<'term, '_>,
        state: &mut CommandState,
        extractors: &mut RenderExtractors<'term>,
        query_context: &mut QueryContext<'term>,
    ) -> bool {
        let mut selection_installed = false;
        match query {
            Query::Rows(tx) => {
                try_send(
                    &tx,
                    grid_rows(terminal),
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::Cols(tx) => {
                try_send(
                    &tx,
                    grid_cols(terminal),
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::CursorX(tx) => {
                try_send(
                    &tx,
                    terminal.cursor_x().unwrap_or(DISCONNECTED_CURSOR_X as u16) as u32,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::CursorY(tx) => {
                try_send(
                    &tx,
                    terminal.cursor_y().unwrap_or(DISCONNECTED_CURSOR_Y as u16) as u32,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::CursorVisible(tx) => {
                // 上游取不到光标状态时按「不可见」处理，与查询通道断开时的回退一致：
                // 少画一个光标好过凭空显示一个，见 `DISCONNECTED_CURSOR_VISIBLE`。
                try_send(
                    &tx,
                    terminal
                        .is_cursor_visible()
                        .unwrap_or(DISCONNECTED_CURSOR_VISIBLE),
                    "query channel send failed",
                );
            }
            Query::Title(tx) => {
                try_send(
                    &tx,
                    String::from_utf8_lossy(terminal.title().unwrap_or_default()).into_owned(),
                    "query channel send failed",
                );
            }
            Query::ModeGet(mode, tx) => {
                // 模式查询失败按「未启用」作答，但必须出声：静默 false 会让调用方
                // 把「读不到」当成「应用没开」，焦点上报与粘贴包装都会走错分支。
                let enabled = match terminal.mode(mode) {
                    Ok(enabled) => enabled,
                    Err(error) => {
                        log::error!("ghostty_terminal: mode query failed: {error}");
                        false
                    }
                };
                try_send(&tx, enabled, "ghostty_terminal: query channel send failed");
            }
            Query::ScrollbackLength(tx) => {
                let scrollback_rows = scrollback_len(terminal);
                // 热路径：滚动期间每次手势都会查一次，debug 级别会在 logcat 里
                // 刷出每秒十几行、并给渲染线程添一次同步写。降为 trace。
                log::trace!("ghostty_terminal: scrollback_rows query returned {scrollback_rows}");
                try_send(
                    &tx,
                    scrollback_rows,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::ReadLineText { row, tx } => {
                try_send(
                    &tx,
                    Self::read_line_text_impl(terminal, row),
                    "query channel send failed",
                );
            }
            Query::RenderCursor(tx) => {
                // 只更新快照取光标视口，**不**走 build_cell_data：本查询要的只是
                // (row, col) 与可见性，而 build_cell_data 会把整张网格逐行抽一遍、
                // 写进跨帧行缓存、再 `snapshot.clean()` 标为已消费。为两个整数做这件事
                // 既浪费，又让「取光标」与「渲染」共用同一份可变抽取状态——此前本查询
                // 甚至会把抽取结果按调用处的默认色存进缓存。
                let (render_state, _, _, _) = extractors.parts();
                let visible_cursor = render_state
                    .update(terminal)
                    .map(|snapshot| cursor_in_viewport(&snapshot))
                    .ok()
                    .filter(|(_, _, _, visible)| *visible)
                    .map(|(row, col, _, _)| (row, col));
                try_send(&tx, visible_cursor, "query channel send failed");
            }
            Query::ReadVisibleText(tx) => {
                let rows = grid_rows(terminal);
                let scrollback_rows = scrollback_len(terminal);
                let mut text = String::new();
                for row in 0..rows {
                    // read_line_text_impl 要求绝对行号（回滚 + 视口）。
                    if let Some(line) = Self::read_line_text_impl(terminal, scrollback_rows + row) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                try_send(&tx, text, "ghostty_terminal: query channel send failed");
            }
            // 行域与 DumpGrid 相同（回滚顶部起连到视口末行），每格只读 codepoint。
            // 网格未变时复用上次产出：回滚填满时逐格 FFI 单趟即可越过
            // QUERY_TIMEOUT_MS，轮询方会读超时后的空值（表现为「内容不见了」）。
            Query::ReadAllText(tx) => {
                if state.cached_all_text.is_none() {
                    let total_rows = scrollback_len(terminal) + grid_rows(terminal);
                    let mut text = String::new();
                    for row in 0..total_rows {
                        text.push_str(&Self::read_all_text_row_impl(terminal, row));
                        text.push('\n');
                    }
                    state.cached_all_text = Some(text);
                }
                try_send(
                    &tx,
                    state.cached_all_text.clone().unwrap_or_default(),
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::SelectionText { start, end, tx } => {
                // Ghostty 原生的换行感知选区文本提取（termux
                // TerminalBuffer.getSelectedText 语义）：unwrap 拼接软换行行且
                // 不带 '\n'，trim 去掉尾部空白，格式化器内部把网格列映射为
                // 字符下标，故 CJK 宽字符不会被拆开。
                let text = Self::selection_text_impl(terminal, start, end);
                try_send(&tx, text, "selection text response send failed");
            }
            Query::SelectWordAt { row, col, tx } => {
                let bounds = Self::select_word_at_impl(terminal, row, col);
                selection_installed = bounds.is_some();
                try_send(&tx, bounds, "select word response send failed");
            }
            Query::SelectAll { tx } => {
                let bounds = Self::select_all_impl(terminal);
                selection_installed = bounds.is_some();
                try_send(&tx, bounds, "select all response send failed");
            }
            Query::HyperlinkAt { row, col, tx } => {
                let url = Self::hyperlink_at_impl(terminal, row, col);
                try_send(&tx, url, "hyperlink_at response send failed");
            }
            Query::WideCharTailCols { row, tx } => {
                try_send(
                    &tx,
                    Self::wide_char_tail_cols_impl(terminal, row),
                    "wide_char_tail_cols response send failed",
                );
            }
            Query::SearchSetNeedle { needle, tx } => {
                let total =
                    Self::search_set_needle_impl(&mut query_context.search, terminal, &needle);
                try_send(
                    &tx,
                    total,
                    "ghostty_terminal: search needle response send failed",
                );
            }
            Query::SearchSelect { forward, tx } => {
                let selection =
                    Self::search_select_impl(&mut query_context.search, terminal, forward);
                try_send(
                    &tx,
                    selection,
                    "ghostty_terminal: search select response send failed",
                );
            }
            Query::SearchViewportHighlights { tx } => {
                let highlights = Self::search_viewport_highlights_impl(
                    &mut query_context.search,
                    &mut query_context.match_buffer,
                    terminal,
                );
                try_send(
                    &tx,
                    highlights,
                    "ghostty_terminal: search viewport highlights response send failed",
                );
            }
            Query::DumpGrid { tx } => {
                let dumped = Self::build_dumped_grid(
                    terminal,
                    state.default_foreground,
                    state.default_background,
                );
                try_send(&tx, dumped, "ghostty_terminal: query channel send failed");
            }
            Query::TakeKittyGraphicsImage { id, tx } => {
                let kitty_graphics_data = (|| -> Option<KittyGraphicsImageData> {
                    let graphics = terminal.kitty_graphics().ok()?;
                    let image = graphics.image(id)?;
                    let (width, height, rgba) = Self::kitty_image_to_rgba(&image)?;
                    Some(KittyGraphicsImageData {
                        id,
                        width,
                        height,
                        data: rgba.to_vec(),
                    })
                })();
                try_send(
                    &tx,
                    kitty_graphics_data,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::TakeKittyPlacements { row_signatures, tx } => {
                let placements = Self::collect_kitty_placements(
                    terminal,
                    &mut query_context.kitty_cache,
                    row_signatures.as_deref(),
                );
                try_send(
                    &tx,
                    placements,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::EncodeMouseEvent {
                position,
                action,
                button,
                modifiers,
                cell_width,
                cell_height,
                tx,
            } => {
                let mut response = Vec::new();
                // 参考：zelland src-tauri/src/terminal.rs 的 `encode_mouse_event`——
                // 用 Ghostty 鼠标编码器配合渲染器的实时单元格尺寸，并在鼠标上报
                // 关闭时丢弃事件。编码器选项取自终端（跟踪模式 + 格式），故
                // SGR/X10/UTF-8 输出遵循应用的 DECSET 选择。
                Self::encode_mouse_event_into(
                    &mut query_context.encoders,
                    terminal,
                    &mut response,
                    position,
                    match action {
                        1 => mouse::Action::Release,
                        2 => mouse::Action::Motion,
                        _ => mouse::Action::Press,
                    },
                    Some(Self::mouse_button(button)),
                    modifiers,
                    cell_width,
                    cell_height,
                );
                try_send(&tx, response, "mouse encode response send failed");
            }
            Query::EncodeMouseWheel {
                position,
                button,
                modifiers,
                cell_width,
                cell_height,
                repeat,
                tx,
            } => {
                let mut response = Vec::new();
                // 一个滚轮档 = press + release。`repeat` 档合并在这一次应答里：
                // 触摸拖动一次事件滚过几行就编几档，PTY 也只写一次。
                let wheel = Some(Self::mouse_button(button));
                for _ in 0..repeat.max(1) {
                    Self::encode_mouse_event_into(
                        &mut query_context.encoders,
                        terminal,
                        &mut response,
                        position,
                        mouse::Action::Press,
                        wheel,
                        modifiers,
                        cell_width,
                        cell_height,
                    );
                    Self::encode_mouse_event_into(
                        &mut query_context.encoders,
                        terminal,
                        &mut response,
                        position,
                        mouse::Action::Release,
                        wheel,
                        modifiers,
                        cell_width,
                        cell_height,
                    );
                }
                try_send(&tx, response, "mouse encode response send failed");
            }
            Query::EncodeKeyEvent {
                key,
                action,
                modifiers,
                text,
                unshifted,
                tx,
            } => {
                let (key_encoder, key_event) = &mut query_context.encoders.key;
                let mods = Mods::from_bits_retain(modifiers);
                key_encoder.set_options_from_terminal(terminal);
                key_event
                    .set_action(action)
                    .set_key(key)
                    .set_mods(mods)
                    // Shift 已体现在文本里（如 Shift+A 的 `A`）：不标为已消费，
                    // 编码器会再叠一层 Shift 修饰序列，终端收到的是两次 Shift+A。
                    .set_consumed_mods(if text.is_some() {
                        mods & Mods::SHIFT
                    } else {
                        Mods::empty()
                    })
                    .set_utf8(text.map(String::from))
                    // `key::Event` 跨事件复用而 `set_unshifted_codepoint` 没有清零入口：
                    // 只在有值时设置，上一键的码位会残留到本次。上游以 0 表示「未提供」
                    // （kitty 表格未命中时不猜码位、Ctrl 折叠不套用大写字母的基字母），
                    // 故缺省时显式写 0。
                    .set_unshifted_codepoint(unshifted.unwrap_or('\0'));
                let mut response = Vec::new();
                if let Err(error) = key_encoder.encode_to_vec(key_event, &mut response) {
                    log::warn!("ghostty_terminal: key encode failed: {error}");
                }
                try_send(&tx, response, "key encode response send failed");
            }
            Query::EncodePaste { data, tx } => {
                // 括号标志取活体模式 2004（DECSET 私有模式），与按键/鼠标编码
                // 同属终端状态感知路径。查不到按未启用作答（即不包裹），但记 error：
                // 这是粘贴注入的防线，失效必须是可诊断的，而不是一次静默降级。
                let bracketed = match terminal.mode(Mode::BRACKETED_PASTE) {
                    Ok(bracketed) => bracketed,
                    Err(error) => {
                        log::error!("ghostty_terminal: bracketed paste mode query failed: {error}");
                        false
                    }
                };
                try_send(
                    &tx,
                    encode_paste_data(&data, bracketed),
                    "paste encode response send failed",
                );
            }
        }
        selection_installed
    }

    /// 排空查询通道，使查询看到最新终端状态。返回真表示本批中有查询安装了选区，
    /// 调用方须按 `Command::SetSelection` 同款规则失效行缓存并重推帧。
    /// 全量文本缓存跨批次保留：网格变动由调用方置 `None`（与快照缓存同款失效时机）。
    ///
    /// `first` 是本轮等待时已从通道取出的查询（见 [`VtEvent`]）：它已离开通道，
    /// 必须在此处理，否则会被丢弃、查询方只能等到超时。
    pub(super) fn drain_queries<'term>(
        query_receiver: &flume::Receiver<Query>,
        first: Option<Query>,
        terminal: &mut Terminal<'term, '_>,
        state: &mut CommandState,
        extractors: &mut RenderExtractors<'term>,
        query_context: &mut QueryContext<'term>,
    ) -> bool {
        let mut selection_installed = false;
        let mut taken = first;
        loop {
            let query = match taken.take() {
                Some(query) => query,
                None => match query_receiver.try_recv() {
                    Ok(query) => query,
                    Err(_) => break,
                },
            };
            // 累积而非覆盖：同批中选区安装查询之后若跟着普通查询（如 SelectionText），
            // 普通查询返回 false 会抹掉安装信号，导致行缓存不失效、反白帧不重推。
            selection_installed |=
                Self::process_query(query, terminal, state, extractors, query_context);
        }
        selection_installed
    }

    /// 纯文本单行：跳过 codepoint 0（宽字符尾格与空格），行尾去空白。
    ///
    /// 与 [`Self::read_line_text_impl`] 的区别是空单元不占位。全量文本面向
    /// 「这一行写了什么」，语义与 [`Self::build_dumped_grid`] 的消费侧一致。
    pub(crate) fn read_all_text_row_impl(terminal: &Terminal, row: u32) -> String {
        let cols = grid_cols(terminal);
        let mut text = String::new();
        for col in 0..cols {
            let codepoint = cell_codepoint(terminal, row, col);
            if let Some(character) = char::from_u32(codepoint).filter(|&c| c != '\0') {
                text.push(character);
            }
        }
        text.trim_end().to_string()
    }

    /// 单行文本，**每个网格列恰好一个字符**：codepoint 为 0 的空单元与宽字符尾格
    /// 都以空格占位。
    ///
    /// 字符下标即列号，故供需要按列寻址的消费方使用（选区拖动经 JNI 的
    /// `scrollbackLine` 取行文本后按列换算）。搜索高亮不用此读口：它按列填充会把宽字符拆成「字 + 空格」，
    /// CJK 多字命中因而无法定位到整字，见 [`Self::selection_endpoint_columns`]。
    pub(crate) fn read_line_text_impl(terminal: &Terminal, row: u32) -> Option<String> {
        let cols = grid_cols(terminal);
        let mut text = String::new();
        for col in 0..cols {
            // 每个网格列必须**恰好**产出一个字符：读不到单元时若少产一个字符，
            // 其后所有列号都会左移一格，且偏移会经软换行段累加继续放大。
            let codepoint = cell_codepoint(terminal, row, col);
            match char::from_u32(codepoint) {
                Some(character) if codepoint != 0 => text.push(character),
                _ => text.push(' '),
            }
        }
        let trimmed = text.trim_end().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}
