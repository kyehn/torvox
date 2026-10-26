use super::*;

// ── 选区与搜索 ───────────────────────────────────────────────

impl super::GhosttyTerminal {
    /// 经 Ghostty 原生格式化器做换行感知的选区文本提取。
    ///
    /// 参考：termux-app TerminalBuffer.getSelectedText（joinBackLines）加
    /// TerminalRow.findStartOfColumn。格式化器的 `unwrap` 拼接软换行行且不带
    /// '\n'，`trim` 去掉尾部空白；网格列内部映射为字符下标，故 CJK 宽字符
    /// 不会被拆开（代理对不产生列到字符的漂移）。
    ///
    /// 坐标是绝对网格行（0 = 回滚顶部；视口自 `scrollback_rows` 起）。
    /// 回移行经 Point::History 解析，视口行经 Point::Viewport（等价于带绝对
    /// y 的 Point::Screen）。选区无效时返回空串。
    pub(crate) fn selection_text_impl(
        terminal: &Terminal,
        start: (u32, u32),
        end: (u32, u32),
    ) -> String {
        // 绝对网格行（0 = 回滚顶部）→ Point 的空间解析统一见 absolute_point。
        let start_point = Self::absolute_point(terminal, start.0, start.1);
        let end_point = Self::absolute_point(terminal, end.0, end.1);
        let (Ok(start_gref), Ok(end_gref)) =
            (terminal.grid_ref(start_point), terminal.grid_ref(end_point))
        else {
            return String::new();
        };
        let selection =
            libghostty_vt::selection::Selection::new(start_gref, end_gref, SELECTION_RECTANGLE);
        let mut formatter = match libghostty_vt::fmt::Formatter::new(
            terminal,
            libghostty_vt::fmt::FormatterOptions::new()
                .with_unwrap(true)
                .with_trim(true)
                .with_selection(&selection),
        ) {
            Ok(initialized_formatter) => initialized_formatter,
            Err(error) => {
                log::error!("ghostty_terminal: formatter new failed: {error}");
                return String::new();
            }
        };
        match formatter.format_alloc(None) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => {
                log::error!("ghostty_terminal: formatter format failed: {error}");
                String::new()
            }
        }
    }

    /// 该行中作为宽字符后半格（`SpacerTail`）的列号，升序。
    ///
    /// 宽字符占几列只有网格知道：行文本里尾格与真空白同为 `' '`，据此反推必然出错
    /// （见 change design 第 1 节）。`SpacerHead` 不计入——它出现在软换行续行的行首，
    /// 该行前一列属于折行前的上一行，把它当尾格会把选区跨过折行边界。
    pub(crate) fn wide_char_tail_cols_impl(terminal: &Terminal, row: u32) -> Vec<u32> {
        (0..grid_cols(terminal))
            .filter(|&col| {
                terminal
                    .grid_ref(Self::absolute_point(terminal, row, col))
                    .and_then(|point| point.cell())
                    .is_ok_and(|cell| {
                        matches!(cell.wide(), Ok(libghostty_vt::screen::CellWide::SpacerTail))
                    })
            })
            .collect()
    }

    /// 绝对网格行（0 = 回滚顶部）→ Point：唯一前向映射规则，列钳制到网格宽度。
    ///
    /// 恒用 [`PointSpace::Screen`]/`Point::Screen`（上游 `.screen` 的原点恒为
    /// `pages.first`，即回滚顶部的绝对行）。`Point::Viewport` 的原点随
    /// `pages.viewport` 滚动而移动，把它当绝对坐标会在上滚 `s` 行后整体错位 `s` 行。
    /// 供 selection_text_impl、install_selection_impl 与上游选择派生共用。
    pub(crate) fn absolute_point(terminal: &Terminal, row: u32, col: u32) -> Point {
        let clamped_col = col.min(grid_cols(terminal).saturating_sub(1)) as u16;
        Point::Screen(PointCoordinate {
            x: clamped_col,
            y: row,
        })
    }

    /// 绝对网格坐标 → gref：三处选择派生与超链接查询的公共读口，
    /// 越界/无效坐标一律 None（调用方据此返回"无该格"）。
    pub(crate) fn grid_ref_at<'a>(
        terminal: &'a Terminal,
        row: u32,
        col: u32,
    ) -> Option<GridRef<'a>> {
        terminal
            .grid_ref(Self::absolute_point(terminal, row, col))
            .ok()
    }

    /// gref → 绝对网格坐标：absolute_point 的逆，单次 Screen 空间回读
    /// （`.screen` 原点与 `absolute_point` 同一），无法表达该格时返回 None。
    pub(crate) fn absolute_coordinate(
        terminal: &Terminal,
        grid_ref: &GridRef,
    ) -> Option<(u32, u32)> {
        let coordinate = terminal
            .point_from_grid_ref(grid_ref, PointSpace::Screen)
            .ok()??;
        Some((coordinate.y, u32::from(coordinate.x)))
    }

    /// 上游派生选区的公共收尾（design 决策 1 的顺序）：to_ordered(Forward)
    /// 取序 → gref 反解为绝对坐标 → set_selection 安装 → 回传界限。
    /// 反解或安装失败时返回 None，绝不回传未安装到终端的界限。
    pub(crate) fn installed_bounds(
        terminal: &Terminal,
        selection: Selection<'_>,
    ) -> Option<((u32, u32), (u32, u32))> {
        let ordered = selection.to_ordered(terminal, Order::Forward).ok()?;
        let start = Self::absolute_coordinate(terminal, &ordered.start())?;
        let end = Self::absolute_coordinate(terminal, &ordered.end())?;
        if terminal.set_selection(Some(&ordered)).is_err() {
            log::warn!("ghostty_terminal: install derived selection failed");
            return None;
        }
        Some((start, end))
    }

    /// 上游词选语义（`Terminal::select_word`，Ghostty 默认词边界码位）：
    /// 派生该格所属词的选区、安装并回传有序绝对界限。
    pub(crate) fn select_word_at_impl(
        terminal: &Terminal,
        row: u32,
        col: u32,
    ) -> Option<((u32, u32), (u32, u32))> {
        let grid_ref = Self::grid_ref_at(terminal, row, col)?;
        let selection = terminal
            .select_word(SelectWordOptions::new(grid_ref))
            .ok()??;
        Self::installed_bounds(terminal, selection)
    }

    /// 上游全选语义（`Terminal::select_all`，“all selectable terminal
    /// content”）：界限不含尾部空行/空列（design 决策 2，由 cargo 测试钉住）。
    pub(crate) fn select_all_impl(terminal: &Terminal) -> Option<((u32, u32), (u32, u32))> {
        let selection = terminal.select_all().ok()??;
        Self::installed_bounds(terminal, selection)
    }

    /// 将绝对网格坐标的选区安装为终端持有的活动选区（跟踪引用）。
    /// 与 selection_text_impl 同一坐标系：绝对网格行（0 = 回滚顶部）。
    /// 无效端点时静默忽略（调用方已标记脏并重推，保持帧一致）。
    pub(crate) fn install_selection_impl(terminal: &Terminal, start: (u32, u32), end: (u32, u32)) {
        let (Ok(start_ref), Ok(end_ref)) = (
            terminal.grid_ref(Self::absolute_point(terminal, start.0, start.1)),
            terminal.grid_ref(Self::absolute_point(terminal, end.0, end.1)),
        ) else {
            return;
        };
        let selection =
            libghostty_vt::selection::Selection::new(start_ref, end_ref, SELECTION_RECTANGLE);
        if terminal.set_selection(Some(&selection)).is_err() {
            log::warn!("ghostty_terminal: install selection failed");
        }
    }

    /// 查询某网格单元处的 OSC 8 超链接 URI（等价 termux TerminalView 的
    /// openLinkAt；ghostty 侧用 cell.has_hyperlink + hyperlink_uri）。
    pub(crate) fn hyperlink_at_impl(terminal: &Terminal, row: u32, col: u32) -> Option<String> {
        let cols = grid_cols(terminal);
        let Ok(total_rows) = terminal.total_rows() else {
            log::error!("ghostty_terminal: total_rows failed during hyperlink hit test");
            return None;
        };
        if col >= cols || row >= total_rows as u32 {
            return None;
        }
        let grid_ref = Self::grid_ref_at(terminal, row, col)?;
        let cell = grid_ref.cell().ok()?;
        match cell.has_hyperlink() {
            Ok(true) => {}
            Ok(false) => return None,
            // 读不到时当「无链接」，但必须出声：静默 false 会让用户点不动链接
            // 且无从判断是坐标没落在链接上还是读取失败。
            Err(error) => {
                log::warn!("ghostty_terminal: has_hyperlink failed: {error}");
                return None;
            }
        }
        // 超长 URI（>4KiB）按上游 OutOfSpace{required} 重试，避免截断长链接。
        let mut buf = [0u8; HYPERLINK_INLINE_BUFFER_SIZE];
        match grid_ref.hyperlink_uri(&mut buf) {
            Ok(0) => None,
            Ok(len) => Some(String::from_utf8_lossy(&buf[..len]).into_owned()),
            Err(libghostty_vt::error::Error::OutOfSpace { required }) => {
                // 超过上限的 URI 直接丢弃并出声：截断的链接指向错误地址，
                // 比没有链接更糟；上限内才值得第二次精确读取。
                if required == 0 || required > MAX_HYPERLINK_URI_BYTES {
                    log::warn!(
                        "ghostty_terminal: hyperlink URI unusable (required {required} bytes), dropping link"
                    );
                    return None;
                }
                let mut grown = vec![0u8; required];
                match grid_ref.hyperlink_uri(&mut grown) {
                    Ok(0) => None,
                    Ok(grown_len) => {
                        Some(String::from_utf8_lossy(&grown[..grown_len]).into_owned())
                    }
                    Err(error) => {
                        log::warn!("ghostty_terminal: hyperlink URI reread failed: {error}");
                        None
                    }
                }
            }
            Err(error) => {
                log::warn!("ghostty_terminal: hyperlink URI read failed: {error}");
                None
            }
        }
    }

    /// 搜索 needle 长度上限：超长 needle 判为无命中，避免上游搜索器为不可能命中的
    /// 长串付出代价。唯一真源，Kotlin 侧经 `NativeBridge.searchQueryMaxChars()` 取值
    /// 截断输入，不再自带副本（改这里即两侧同步生效）。
    pub(crate) const MAX_SEARCH_QUERY_CHARS: usize = 128;
    /// 行内最后一个有内容的列的开区间上界：行尾未写下的填充不计，高亮不越过它。
    ///
    /// 自右向左扫。宽字符的尾格自身无 codepoint，故命中尾格即以 `col + 1` 收尾
    /// （它与前一格的首列同属一个宽字符）。
    pub(crate) fn row_content_columns(terminal: &Terminal, row: u32) -> u32 {
        for col in (0..grid_cols(terminal)).rev() {
            let Ok(grid_ref) = terminal.grid_ref(Self::absolute_point(terminal, row, col)) else {
                continue;
            };
            let Ok(cell) = grid_ref.cell() else { continue };
            if matches!(cell.wide(), Ok(CellWide::SpacerTail)) {
                return col + 1;
            }
            if cell_codepoint(terminal, row, col) != 0 {
                return col + Self::cell_columns(&cell).unwrap_or(1);
            }
        }
        0
    }

    /// 选区端点单元的 `(行, 首列, 开区间上界)`：宽字符覆盖两列，端点落在它的头/尾格
    /// 时起点回退到该宽字符的首列。端点格读不出时返回 None。
    pub(crate) fn selection_endpoint_columns(
        terminal: &Terminal,
        grid_ref: &GridRef,
    ) -> Option<(u32, u32, u32)> {
        let (row, col) = Self::absolute_coordinate(terminal, grid_ref)?;
        let cell = grid_ref.cell().ok()?;
        let start_col = match cell.wide() {
            Ok(CellWide::SpacerTail | CellWide::SpacerHead) => col.saturating_sub(1),
            _ => col,
        };
        let columns = match cell.wide() {
            Ok(CellWide::Wide | CellWide::SpacerTail | CellWide::SpacerHead) => 2,
            _ => 1,
        };
        Some((row, start_col, start_col + columns))
    }

    /// 上游命中的 [`Selection`] → 按物理行拆分的绝对网格区间。
    ///
    /// 上游按软换行把跨行命中表达为一条选区，本仓的高亮按行表达：首行自命中起点到
    /// 行内容末尾，末行自 0 到命中终点，中间整行覆盖，且每行末尾按行内容裁剪。
    /// 端点可能反向（上游保留选择方向），按行优先序对调即得正向区间。
    pub(crate) fn search_matches_from_selection(
        terminal: &Terminal,
        selection: &Selection,
        matches: &mut Vec<SearchMatch>,
    ) {
        let Some((start_row, start_col, start_limit)) =
            Self::selection_endpoint_columns(terminal, &selection.start())
        else {
            return;
        };
        let Some((end_row, end_col, end_limit)) =
            Self::selection_endpoint_columns(terminal, &selection.end())
        else {
            return;
        };
        let ((first_row, first_col), (last_row, last_limit)) =
            if (start_row, start_col) <= (end_row, end_col) {
                ((start_row, start_col), (end_row, end_limit))
            } else {
                ((end_row, end_col), (start_row, start_limit))
            };
        for row in first_row..=last_row {
            let content_columns = Self::row_content_columns(terminal, row);
            let row_start = if row == first_row { first_col } else { 0 };
            let row_end = if row == last_row {
                last_limit
            } else {
                content_columns
            };
            let start_col = row_start.min(content_columns);
            let end_col = row_end.min(content_columns);
            if start_col < end_col {
                matches.push(SearchMatch {
                    row,
                    start_col,
                    end_col,
                });
            }
        }
    }

    /// 设置 needle 并同步跑完搜索（`feed` + `tick` 至 `Complete`），返回命中总数。
    ///
    /// 搜索对象随 VT 线程存活：首次调用时创建，之后复用同一份索引状态，故连续
    /// 查询不必重建。needle 超上限不截断而是判为无命中——截断会让用户搜到另一个
    /// 串的结果。
    pub(crate) fn search_set_needle_impl<'term>(
        search: &mut Option<Search<'_, 'term>>,
        terminal: &mut Terminal<'term, '_>,
        needle: &str,
    ) -> usize {
        let needle = if needle.chars().count() > Self::MAX_SEARCH_QUERY_CHARS {
            log::warn!(
                "ghostty_terminal: search needle over {} chars ignored",
                Self::MAX_SEARCH_QUERY_CHARS
            );
            ""
        } else {
            needle
        };
        // 关闭搜索（空 needle）且从未建过搜索对象时无事可做：不为一次清空
        // 分配并运行一个 idle 搜索。
        if search.is_none() && needle.is_empty() {
            return 0;
        }
        if search.is_none() {
            match Search::new(terminal) {
                Ok(mut created) => {
                    // 视口滚动只能有一个来源：本仓的 `setScrollOffset` 以会话记录的
                    // `last_scroll_offset` 为增量基线，Kotlin 再按同一偏移算高亮行与
                    // 居中目标。上游默认 `IfNeeded` 会在 `select_next` 里**偷偷移动视口**
                    // （真机日志：选中后高亮与定位一起偏到非匹配行），基线不随之更新，
                    // 下一次增量就会落到错误行。故关掉上游滚动，由宿主居中。
                    if let Err(error) = created.set_scroll(Scroll::None) {
                        log::error!("ghostty_terminal: search scroll policy failed: {error}");
                    }
                    *search = Some(created);
                }
                Err(error) => {
                    log::error!("ghostty_terminal: search::Search::new failed: {error}");
                    return 0;
                }
            }
        }
        let Some(active) = search.as_mut() else {
            return 0;
        };
        if let Err(error) = active.set_needle(terminal, needle.as_bytes()) {
            log::error!("ghostty_terminal: search set_needle failed: {error}");
            return 0;
        }
        if let Err(error) = active.run(terminal) {
            log::error!("ghostty_terminal: search run failed: {error}");
            return 0;
        }
        match active.total_matches() {
            Ok(total) => total,
            Err(error) => {
                log::error!("ghostty_terminal: search total_matches failed: {error}");
                0
            }
        }
    }

    /// 选中下一个（`forward = true`，朝更旧内容）或上一个命中，返回选中态。
    ///
    /// 上游按 `Scroll::None` 选中、不移动视口；居中由宿主在拿到行号后完成，
    /// 滚动来源只有宿主的 `setScrollOffset`（见 `text-search` 规范）。
    pub(crate) fn search_select_impl<'term>(
        search: &mut Option<Search<'_, 'term>>,
        terminal: &mut Terminal<'term, '_>,
        forward: bool,
    ) -> Option<SearchSelection> {
        let active = search.as_mut()?;
        let selected = if forward {
            active.select_next(terminal)
        } else {
            active.select_prev(terminal)
        };
        match selected {
            Ok(true) => {}
            Ok(false) => return None,
            Err(error) => {
                log::error!("ghostty_terminal: search select failed: {error}");
                return None;
            }
        }
        let total = match active.total_matches() {
            Ok(total) => total,
            Err(error) => {
                log::error!("ghostty_terminal: search total_matches failed: {error}");
                return None;
            }
        };
        let index = active.selected_index().ok().flatten()?;
        let snapshot = match active.snapshot(terminal) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::error!("ghostty_terminal: search snapshot failed: {error}");
                return None;
            }
        };
        let selected_match = match snapshot.selected_match() {
            Ok(Some(selected_match)) => selected_match,
            Ok(None) => return None,
            Err(error) => {
                log::error!("ghostty_terminal: search selected_match failed: {error}");
                return None;
            }
        };
        let mut matches = Vec::new();
        Self::search_matches_from_selection(snapshot.terminal(), &selected_match, &mut matches);
        Some(SearchSelection {
            index,
            total,
            matched: matches.first()?.clone(),
        })
    }

    /// 视口高亮输入：一次 `snapshot` 同时取视口命中与选中命中（同一快照内一致，
    /// 避免两次读取之间输出到达导致选中态与命中集不同源），回滚长度供渲染侧折算
    /// 绝对行到网格行。
    pub(crate) fn search_viewport_highlights_impl<'term>(
        search: &mut Option<Search<'_, 'term>>,
        match_buffer: &mut MatchBuffer,
        terminal: &mut Terminal<'term, '_>,
    ) -> SearchViewportHighlights {
        let scrollback_lines = scrollback_len(terminal);
        let Some(active) = search.as_mut() else {
            return SearchViewportHighlights {
                matches: Vec::new(),
                selected: Vec::new(),
                scrollback_lines,
            };
        };
        let snapshot = match active.snapshot(terminal) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::error!("ghostty_terminal: search snapshot failed: {error}");
                return SearchViewportHighlights {
                    matches: Vec::new(),
                    selected: Vec::new(),
                    scrollback_lines,
                };
            }
        };
        let selected = match snapshot.selected_match() {
            Ok(Some(selected_match)) => {
                let mut selected_rows = Vec::new();
                Self::search_matches_from_selection(
                    snapshot.terminal(),
                    &selected_match,
                    &mut selected_rows,
                );
                selected_rows
            }
            Ok(None) => Vec::new(),
            Err(error) => {
                log::error!("ghostty_terminal: search selected_match failed: {error}");
                Vec::new()
            }
        };
        let Ok(viewport_matches) = snapshot.viewport_matches(match_buffer) else {
            log::error!("ghostty_terminal: search viewport_matches failed");
            return SearchViewportHighlights {
                matches: Vec::new(),
                selected,
                scrollback_lines,
            };
        };
        let mut matches = Vec::new();
        for selection in viewport_matches {
            Self::search_matches_from_selection(snapshot.terminal(), &selection, &mut matches);
        }
        SearchViewportHighlights {
            matches,
            selected,
            scrollback_lines,
        }
    }
}
