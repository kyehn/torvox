use super::*;

// ── VT 线程主循环 ────────────────────────────────────────────

impl super::GhosttyTerminal {
    pub(crate) fn run(config: RunConfig) {
        // 把整个函数体包进 `catch_unwind`，任何意外的 FFI panic
        // （例如来自 Ghostty 的 C 代码）只记日志，不会静默杀掉线程。
        // 此处用 `AssertUnwindSafe` 是安全的，因为：
        // - Terminal 因内部 C 指针而是 `!UnwindSafe`，但其 Drop
        //   实现会在 unwind 时调用 `ghostty_terminal_free`。
        // - panic 后线程总是退出，不存在二次使用。
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Self::run_inner(config)));
        if let Err(panic) = result {
            log::error!("ghostty_terminal: VT thread panicked: {panic:?}");
        }
    }

    /// 内层 run 循环（拆出来供 `catch_unwind` 调用）。
    pub(crate) fn run_inner(config: RunConfig) {
        let Some(mut terminal) = Self::configure_terminal(&config) else {
            return;
        };
        let mut query_context = match QueryContext::new() {
            Ok(context) => context,
            Err(error) => {
                log::error!("ghostty_terminal: key encoder unavailable: {error} — thread exiting");
                return;
            }
        };

        // 逐帧渲染迭代器与其行缓存：与终端同生命周期，不逐帧新建。上游 `RenderState`
        // 跨帧持有上次快照状态，逐帧新建会使 `rows = 0` 初值把每次 `update` 判为
        // 「维度变化 → 全量重建」，行级脏标记恒为全脏（实测命中率 0%）。
        // 创建失败即终止 VT 线程：缺了它们既取不到网格也读不到光标，
        // 以「本帧返回空网格」延后只会在第一次渲染时静默降级。
        let mut extractors = match RenderExtractors::new() {
            Some(extractors) => extractors,
            None => {
                log::error!("ghostty_terminal: render extractors unavailable — thread exiting");
                return;
            }
        };
        // 测试构建：换成该终端实例自己的计数句柄，使并行用例互不污染。
        #[cfg(test)]
        {
            extractors.cache_stats = config.cache_stats.clone();
            query_context.kitty_cache.stats = config.kitty_stats.clone();
        }

        // 初始主题：libghostty-vt 不处理 OSC 10/11 默认色转义（默认值归内嵌方所有，
        // OSC 4 调色板覆盖由上游处理），故走原生 setter。否则终端会保留内置的
        // xterm 调色板，主题颜色永远进不到网格里。
        Self::apply_theme(&mut terminal, &config.theme.palette);

        let mut state = CommandState::new(&config.theme.palette);
        // 查询通道是否仍在参与等待：通道断开后 `flume::Selector` 对它恒为就绪，
        // 继续监听会变成忙等（发送端随会话一起销毁）。
        let mut query_channel_open = true;

        'vt_loop: loop {
            // 本轮已从通道取出的查询：查询就绪时它已离开通道，必须一并处理。
            let mut taken_query = None;
            let mut pending = match Self::wait_for_event(&config, query_channel_open) {
                Err(flume::select::SelectError::Timeout) => None,
                Ok(VtEvent::Command(Ok(command))) => Some(command),
                Ok(VtEvent::Command(Err(_))) => break,
                Ok(VtEvent::Query(Ok(query))) => {
                    taken_query = Some(query);
                    None
                }
                Ok(VtEvent::Query(Err(_))) => {
                    // 查询通道断开（会话销毁）：此后只等命令通道。
                    query_channel_open = false;
                    continue;
                }
            };
            // 无命令就绪：要么节拍到点，要么有查询到达。命令与查询同属一条逻辑
            // 队列，故先取走已入队的命令——有命令就走命令批处理（批尾统一应答
            // 查询，查询因而能看到命令的效果），没有命令才直接应答查询。
            if pending.is_none() {
                pending = config.command_receiver.try_recv().ok();
            }
            if pending.is_none() {
                Self::respond_queries_and_publish(
                    &config,
                    taken_query,
                    &mut terminal,
                    &mut extractors,
                    &mut query_context,
                    &mut state,
                );
                continue;
            }
            // 整批命令处理完只做一次 cell-data 构建。poll_pty_output 每个渲染帧
            // 最多喂入 MAX_CHUNKS_PER_FRAME 条 Write 命令；若每条 Write 之后
            // 都重建整个网格（每次遍历 ~2444 次 Ghostty FFI 调用），SwiftShader
            // 上持续输出的渲染会被压到 ~6 fps——N 次构建里有 N−1 次在渲染线程
            // 看到之前就被覆盖了。中间状态有意合并——渲染线程只会消费最新状态。
            //
            // `grid_dirty` 仍由每条改动型命令就地置位（而非推迟到批尾）：
            // 同批到达的 TakeSnapshot 必须能观察到 FIFO 序上排在它之前的写入，
            // 并重建而不是提供陈旧缓存。
            let mut batch = BatchState::default();
            'batch: while let Some(command) = pending.take() {
                match Self::apply_command(
                    command,
                    &config,
                    &mut terminal,
                    &mut extractors,
                    &mut state,
                    &mut batch,
                ) {
                    CommandOutcome::Continue => {}
                    CommandOutcome::EndBatch => break 'batch,
                    CommandOutcome::Terminate => break 'vt_loop,
                }
                pending = config.command_receiver.try_recv().ok();
            }
            Self::finish_command_batch(
                &config,
                taken_query,
                &mut terminal,
                &mut extractors,
                &mut query_context,
                &mut state,
                &mut batch,
            );
        }
    }

    /// 等待命令或查询就绪；超时返回 `Err`。
    ///
    /// 同时等待命令与查询：任一就绪即醒。查询必须即时应答——只按空闲节拍
    /// 轮转查询通道时，无命令在途的终端上每次查询都要等满一个节拍
    /// （`VT_IDLE_TICK_MS`），渲染线程的 kitty 放置查询每等满一节拍
    /// 就丢掉一帧（真机实测 ~54ms 的慢帧即此）。节拍只负责周期职责。
    fn wait_for_event(
        config: &RunConfig,
        query_channel_open: bool,
    ) -> Result<VtEvent, flume::select::SelectError> {
        let timeout = std::time::Duration::from_millis(super::super::types::VT_IDLE_TICK_MS);
        let selector = flume::Selector::new().recv(&config.command_receiver, VtEvent::Command);
        let selector = if query_channel_open {
            selector.recv(&config.query_receiver, VtEvent::Query)
        } else {
            selector
        };
        selector.wait_timeout(timeout)
    }

    /// 批尾：应答查询、补推积压的帧，最后释放 flush 应答。
    fn finish_command_batch<'term>(
        config: &RunConfig,
        taken_query: Option<Query>,
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        query_context: &mut QueryContext<'term>,
        state: &mut CommandState,
        batch: &mut BatchState,
    ) {
        // 整批处理完后排空待处理查询，让它们看到完全更新后的终端状态。
        // 本轮等待时已取出的那条查询（`taken_query`）也在这里一并应答：
        // 它已离开通道，不带上就会被丢弃、查询方只能等到超时。
        Self::respond_queries_and_publish(
            config,
            taken_query,
            terminal,
            extractors,
            query_context,
            state,
        );
        // flush 应答只在上面的构建之后释放，保持 `flush()` 原有的
        // 逐命令顺序保证。
        for ack in batch.acks.drain(..) {
            try_send(&ack, (), "command channel send failed");
        }
    }

    /// 应答查询并补推积压的帧。空闲节拍与批尾共用：两者都先排空查询，
    /// 查询内安装选区视同 `Command::SetSelection` 失效行缓存，再按
    /// `cell_data_dirty` 决定是否构建一帧。
    fn respond_queries_and_publish<'term>(
        config: &RunConfig,
        taken_query: Option<Query>,
        terminal: &mut Terminal<'term, '_>,
        extractors: &mut RenderExtractors<'term>,
        query_context: &mut QueryContext<'term>,
        state: &mut CommandState,
    ) {
        let selection_installed = Self::drain_queries(
            &config.query_receiver,
            taken_query,
            terminal,
            state,
            extractors,
            query_context,
        );
        if selection_installed {
            state.last_cell_data_push = None;
            state.grid_dirty = true;
            extractors.invalidate_row_cache();
            state.cell_data_dirty = true;
        }
        if state.cell_data_dirty {
            Self::publish_cell_data(config, terminal, extractors, state);
        }
    }

    /// 构建并推送一帧 CellData（自动推送路径）。
    ///
    /// 同步更新（模式 2026）保持中不发布中间帧：渲染线程继续显示进入保持前
    /// 的那一帧，ESU 的写入重新置脏并在批尾发布最终帧。
    fn publish_cell_data<'alloc>(
        config: &RunConfig,
        terminal: &mut Terminal<'alloc, '_>,
        extractors: &mut RenderExtractors<'alloc>,
        state: &mut CommandState,
    ) {
        if synchronized_output_hold(terminal, &mut state.synchronized_output_held_since) {
            return;
        }
        Self::refresh_cell_data(
            config,
            terminal,
            state.default_foreground,
            state.default_background,
            &mut state.last_cell_data_push,
            extractors,
        );
        state.cell_data_dirty = false;
    }

    /// 注册 OSC 52 剪贴板写回回调。
    ///
    /// 上游要求回调返回前必须 `reply`，否则视为拒绝写入并暂停 VT 流。
    /// OSC 52 无应答时取值被丢弃，但调用本身不可省略。
    pub(crate) fn register_clipboard_write_callback(
        terminal: &mut Terminal<'_, '_>,
        clipboard_tx: flume::Sender<(String, String)>,
    ) {
        let result = terminal.on_clipboard_write(move |_terminal, write| {
            // 选择器字母沿用 xterm 约定（Kotlin 侧原样透传）：c=剪贴板、p=主选区、s=次选区。
            let selection = match write.location() {
                libghostty_vt::terminal::ClipboardLocation::Standard => "c",
                libghostty_vt::terminal::ClipboardLocation::Primary => "p",
                libghostty_vt::terminal::ClipboardLocation::Selection => "s",
            }
            .to_string();
            let mut fallback: Option<&[u8]> = None;
            let mut chosen: Option<&[u8]> = None;
            for content in write.contents() {
                if fallback.is_none() {
                    fallback = Some(content.data);
                }
                if content.mime.starts_with(b"text/") {
                    chosen = Some(content.data);
                    break;
                }
            }
            let payload = chosen.or(fallback).unwrap_or(&[]);
            // OSC 52 的负载由被测程序任意大，而下游是 `String` 事件、事件队列与
            // Kotlin 侧剪贴板：一份几十 MB 的负载会一路复制到 UI 线程并撑爆内存。
            // 上限取「远超任何真实剪贴板内容、又不至于拖垮进程」的量级，并在
            // 截断时显式记录——静默截断只会让用户拿到半个剪贴板却不知为何。
            let original_len = payload.len();
            let truncated = original_len > MAX_CLIPBOARD_PAYLOAD_BYTES;
            let payload = if truncated {
                &payload[..MAX_CLIPBOARD_PAYLOAD_BYTES]
            } else {
                payload
            };
            let text = String::from_utf8_lossy(payload).into_owned();
            if truncated {
                log::warn!(
                    "OSC 52 clipboard payload truncated from {original_len} to {MAX_CLIPBOARD_PAYLOAD_BYTES} bytes"
                );
            }
            // 通道满（`bounded(16)`）且消费侧（渲染暂停时停调）跟不上时丢弃——
            // 阻塞 VT 线程更糟。丢则记日志并回 `Busy`，不静默。
            if clipboard_tx.try_send((selection, text)).is_err() {
                log::warn!("OSC 52 clipboard dropped: channel full");
                write.reply(
                    Err(libghostty_vt::terminal::ClipboardWriteError::Busy),
                    false,
                );
            } else {
                write.reply(Ok(()), false);
            }
        });
        if let Err(error) = result {
            log::error!(
                "ghostty_terminal: on_clipboard_write callback registration failed: {error}"
            );
        }
    }

    /// 创建上游终端实例并注册应答回调：回滚上限、Kitty 介质、PTY 回写、
    /// OSC 52 写回与 XTWINOPS 尺寸。
    ///
    /// 返回 `None` 表示终端实例创建失败，调用方据此退出 VT 线程。
    pub(crate) fn configure_terminal(config: &RunConfig) -> Option<Terminal<'_, '_>> {
        let mut terminal = match Terminal::new(config.cols as u16, config.rows as u16) {
            Ok(terminal) => terminal,
            Err(_) => {
                log::error!("ghostty_terminal: Terminal::new failed — thread exiting");
                return None;
            }
        };
        // 上游（git master）的 `Terminal::new` 已不再接收 options，
        // 回滚改用 `set_scrollback_max_lines` setter 配置。
        // 非零值即启用回滚（回滚关闭时 scrollback_rows 查询返回 0）。
        // 字节预算必须同步解除：上游默认字节上限先于行数触发，回滚
        // （DEFAULT_SCROLLBACK_LINES 行）会被拦腰截断到几百行。行数是唯一约束。
        if let Err(error) = terminal.set_scrollback_max_bytes(None) {
            log::error!("ghostty_terminal: set_scrollback_max_bytes failed: {error}");
        }
        if let Err(error) =
            terminal.set_scrollback_max_lines(Some(config.scrollback_lines as usize))
        {
            log::error!("ghostty_terminal: set_scrollback_max_lines failed: {error}");
        }
        if let Some(style) = config.default_cursor_style {
            let upstream = match style {
                crate::terminal::ghostty_terminal::CursorStyle::Block => {
                    libghostty_vt::terminal::CursorStyle::Block
                }
                crate::terminal::ghostty_terminal::CursorStyle::Bar => {
                    libghostty_vt::terminal::CursorStyle::Bar
                }
                crate::terminal::ghostty_terminal::CursorStyle::Underline => {
                    libghostty_vt::terminal::CursorStyle::Underline
                }
            };
            if let Err(error) = terminal.set_default_cursor_style(Some(upstream)) {
                log::error!("ghostty_terminal: set_default_cursor_style failed: {error}");
            }
        }

        // 初始化 Kitty Graphics Protocol（KGP）支持
        if let Err(error) = terminal.set_kitty_image_storage_limit(KGP_STORAGE_LIMIT) {
            log::error!("ghostty_terminal: set_kitty_image_storage_limit failed: {error}");
        }
        // 默认仅启用 direct 介质；kitten icat / yazi 等客户端会使用 file / shared memory /
        // temp file 介质传输大图，不启用会导致 "EINVAL: unsupported medium"，图像无法显示。
        if let Err(error) = terminal.set_kitty_image_from_file_allowed(true) {
            log::error!("ghostty_terminal: set_kitty_image_from_file_allowed failed: {error}");
        }
        if let Err(error) = terminal.set_kitty_image_from_shared_mem_allowed(true) {
            log::error!(
                "ghostty_terminal: set_kitty_image_from_shared_mem_allowed failed: {error}"
            );
        }
        if let Err(error) = terminal.set_kitty_image_temp_file_dir(config.kitty_temp_dir.as_deref())
        {
            log::error!("ghostty_terminal: set_kitty_image_temp_file_dir failed: {error}");
        }
        // 注册终端应答的 PTY 回写回调
        // （DECRPM 模式报告、DSR、DA 等）
        if let Err(error) = terminal.on_pty_write({
            let response_buffer = config.response_buffer.clone();
            move |_terminal, data| {
                if let Ok(mut guard) = response_buffer.lock() {
                    guard.push(data.to_vec());
                }
            }
        }) {
            log::error!("ghostty_terminal: on_pty_write callback registration failed: {error}");
        }
        // OSC 52 剪贴板写入改走上游回调：序列直达 Ghostty，事件经通道推送、
        // 由调用方在 flush 后收割。本仓不再自建 OSC 解析器；try_send 永不阻塞
        // VT 线程；OSC 52 读取请求（`?`）上游明确忽略，仍由 OutputProcessor
        // 的最小扫描器拦截（FR-036）。
        Self::register_clipboard_write_callback(&mut terminal, config.clipboard_tx.clone());
        // XTWINOPS 尺寸查询（CSI 14/16/18t）走上游 on_size 回调：行列取终端
        // 实时值，单元格像素取本层回填的共享几何（Resize 填默认，
        // SetCellPixelSize 填真实字形度量）。无锁读取，永不阻塞 VT 线程。
        if let Err(error) = terminal.on_size({
            let cell_size_px = config.cell_size_px.clone();
            move |terminal| {
                let (Ok(rows), Ok(cols)) = (terminal.rows(), terminal.cols()) else {
                    return None;
                };
                Some(libghostty_vt::terminal::SizeReportSize {
                    rows,
                    columns: cols,
                    cell_width: cell_size_px.0.load(Ordering::Acquire),
                    cell_height: cell_size_px.1.load(Ordering::Acquire),
                })
            }
        }) {
            log::error!("ghostty_terminal: on_size callback registration failed: {error}");
        }
        Some(terminal)
    }

    /// 用原生主题 API 设置默认背景/前景色与 16 色 ANSI 调色板。
    /// libghostty-vt 不处理 OSC 10/11 默认色转义（默认值归内嵌方所有，
    /// OSC 4 调色板覆盖由上游处理），故内嵌方必须直接推送主题色；
    /// 否则终端会保留内置的 xterm 调色板。
    pub(crate) fn apply_theme(
        terminal: &mut libghostty_vt::terminal::Terminal,
        palette: &crate::terminal::theme::Palette,
    ) {
        if terminal
            .set_default_bg_color(Some(palette.background))
            .is_err()
        {
            log::error!("ghostty_terminal: apply_theme failed to set background color");
        }
        if terminal
            .set_default_fg_color(Some(palette.foreground))
            .is_err()
        {
            log::error!("ghostty_terminal: apply_theme failed to set foreground color");
        }
        // 光标色也是 OSC 12 查询的应答来源：不推会让 `printf '\e]12;?\a'`
        // 拿到内建默认色而非主题色，程序按错的底色决定光标对比度。
        if terminal
            .set_default_cursor_color(Some(palette.cursor))
            .is_err()
        {
            log::error!("ghostty_terminal: apply_theme failed to set cursor color");
        }
        match terminal.default_color_palette() {
            Ok(mut colors) => {
                for (index, color) in ANSI_PALETTE_INDICES.iter().zip(palette.ansi.iter()) {
                    colors.set(*index, *color);
                }
                if terminal.set_default_color_palette(Some(colors)).is_err() {
                    log::error!("ghostty_terminal: apply_theme failed to set ANSI palette");
                }
            }
            Err(error) => {
                log::error!("ghostty_terminal: apply_theme failed to read ANSI palette: {error}");
            }
        }
    }

    /// 把 `StyleColor` 解析到终端**生效的** 256 色调色板（默认色加上 OSC 4
    /// 覆盖）；未设置或读不到时回退到 `default`。取代了历史上的静态
    /// 16 色表 + xterm 公式，后者会静默忽略 OSC 4 改色。
    pub(crate) fn resolve_style_color(
        terminal: &Terminal,
        color: &libghostty_vt::style::StyleColor,
        default: [f32; 4],
    ) -> [f32; 4] {
        match color {
            libghostty_vt::style::StyleColor::Rgb(c) => Self::color_to_float(*c),
            libghostty_vt::style::StyleColor::Palette(idx) => terminal
                .color_palette()
                .map(|palette| Self::color_to_float(palette.get(*idx)))
                .unwrap_or(default),
            _ => default,
        }
    }

    pub(crate) fn color_to_float(color: libghostty_vt::style::RgbColor) -> [f32; 4] {
        [
            Self::byte_to_float(color.r),
            Self::byte_to_float(color.g),
            Self::byte_to_float(color.b),
            1.0,
        ]
    }

    pub(crate) fn byte_to_float(value: u8) -> f32 {
        value as f32 / 255.0
    }
}
