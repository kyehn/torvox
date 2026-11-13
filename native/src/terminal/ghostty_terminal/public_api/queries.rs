//! 只读查询：网格、选区、搜索与终端状态，一律经查询通道取回。

use std::sync::atomic::Ordering;

use flume::{Sender, bounded};

use super::super::GhosttyTerminal;
use super::super::commands::Query;
use super::super::types::{
    DISCONNECTED_COLS, DISCONNECTED_CURSOR_VISIBLE, DISCONNECTED_CURSOR_X, DISCONNECTED_CURSOR_Y,
    DISCONNECTED_PWD, DISCONNECTED_ROWS, DISCONNECTED_SCROLLBACK, DISCONNECTED_TITLE, DumpedGrid,
    QUERY_TIMEOUT_MS, SearchSelection, SearchViewportHighlights,
};

impl GhosttyTerminal {
    /// 当前网格行数；查询失败时回退为断连默认值。
    pub fn rows(&self) -> u32 {
        self.query(Query::Rows, DISCONNECTED_ROWS, "rows")
    }

    /// 当前网格列数；查询失败时回退为断连默认值。
    pub fn cols(&self) -> u32 {
        self.query(Query::Cols, DISCONNECTED_COLS, "cols")
    }

    /// 光标列号；查询失败时回退为断连默认值。
    pub fn cursor_x(&self) -> u32 {
        self.query(Query::CursorX, DISCONNECTED_CURSOR_X, "cursor_x")
    }

    /// 光标行号；查询失败时回退为断连默认值。
    pub fn cursor_y(&self) -> u32 {
        self.query(Query::CursorY, DISCONNECTED_CURSOR_Y, "cursor_y")
    }

    /// 光标是否可见；查询失败时回退为「隐藏」。
    pub fn cursor_visible(&self) -> bool {
        self.query(
            Query::CursorVisible,
            DISCONNECTED_CURSOR_VISIBLE,
            "cursor_visible",
        )
    }

    /// 查询某个终端模式是否开启，自带超时参数。
    pub fn mode_get_with_timeout(
        &self,
        mode: libghostty_vt::mode::Mode,
        timeout: std::time::Duration,
    ) -> bool {
        Self::mode_get_on(&self.query_tx, mode, timeout)
    }

    /// [`Self::mode_get_with_timeout`] 的通道外置版，理由同 [`Self::title_on`]。
    /// 本查询自带超时参数（其余走 `query_on` 的统一期限），故不并入 `query_on`。
    pub(crate) fn mode_get_on(
        query_tx: &Sender<Query>,
        mode: libghostty_vt::mode::Mode,
        timeout: std::time::Duration,
    ) -> bool {
        let (tx, rx) = bounded(1);
        if let Err(error) = query_tx.try_send(Query::ModeGet(mode, tx)) {
            // 发送失败即无人会应答：继续 recv 只会白等满整个 timeout，
            // 而调用方普遍持有会话锁，故立即返回（与 query() 同策略）。
            log::warn!("ghostty_terminal: query_tx full/dropped failed: {error}");
            return false;
        }
        if let Ok(mode) = rx.recv_timeout(timeout) {
            mode
        } else {
            log::warn!(
                "ghostty_terminal: mode_get({mode:?}) timed out or disconnected — returning false"
            );
            false
        }
    }

    /// 备用屏是否处于活动状态（读原子标志，不走查询通道）。
    pub fn alt_screen_active_atomic(&self) -> bool {
        self.alt_screen_active.load(Ordering::Acquire)
    }

    /// 鼠标上报（DECSET 1000/1002/1003）是否启用。
    ///
    /// 镜像由 VT 线程每帧刷新，故本调用不碰查询通道、不等待 VT 线程——
    /// 触摸拖动的每个事件都要问一次，等一次应答就够卡住手势了。
    pub fn mouse_tracking_active_atomic(&self) -> bool {
        self.mouse_tracking_active.load(Ordering::Acquire)
    }

    /// 向 VT 线程发送无状态 [`Query`] 并等待应答，发送失败或超时回退到 `fallback`；
    /// 全部查询方法共用这唯一一处有界通道 + 超时样板。
    pub(super) fn query<T>(
        &self,
        build: impl Fn(Sender<T>) -> Query,
        fallback: T,
        method: &str,
    ) -> T {
        Self::query_on(&self.query_tx, build, fallback, method)
    }

    /// [`Self::query`] 的通道外置版：调用方在会话锁内克隆查询通道，
    /// 随后在锁外执行长查询，使大回滚搜索不冻结按帧取锁的渲染。
    /// 查询重试次数：VT 线程正常在 `VT_IDLE_TICK_MS` 内应答，但系统过载（并行测试、
    /// 后台编译）时可能整体错过一个 `QUERY_TIMEOUT_MS` 窗口。重试一次可把「偶发
    /// 错过」与「VT 线程卡死」区分开：幂等查询的第二次尝试通常直接命中（读缓存
    /// 查询会命中上一轮已落盘的缓存，直读查询等到线程恢复后的第一次应答），后者
    /// 只会再等满一个期限后仍回退——调用方拿到的兜底值语义不变。
    /// 有副作用的动作型查询（如 `SearchSelect` 推进搜索游标）经
    /// [`Self::query_on_attempts`] 只发一次，防止滞留首查应答丢失后重发双跳。
    const QUERY_ATTEMPTS: u8 = 2;

    pub(super) fn query_on<T>(
        query_tx: &Sender<Query>,
        build: impl Fn(Sender<T>) -> Query,
        fallback: T,
        method: &str,
    ) -> T {
        Self::query_on_attempts(query_tx, build, fallback, method, Self::QUERY_ATTEMPTS)
    }

    fn query_on_attempts<T>(
        query_tx: &Sender<Query>,
        build: impl Fn(Sender<T>) -> Query,
        fallback: T,
        method: &str,
        attempts: u8,
    ) -> T {
        for attempt in 1..=attempts {
            let (tx, rx) = bounded(1);
            if let Err(error) = query_tx.try_send(build(tx)) {
                log::warn!("ghostty_terminal: query_tx full/dropped failed for {method}: {error}");
                return fallback;
            }
            match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
                Ok(value) => return value,
                Err(error) if attempt < attempts => {
                    log::warn!(
                        "ghostty_terminal: {method} query attempt {attempt} hit deadline ({error:?}); retrying once"
                    );
                }
                Err(error) => {
                    log::warn!(
                        "ghostty_terminal: {method} timed out or disconnected after {attempts} attempt(s) ({error:?}) — returning fallback"
                    );
                }
            }
        }
        fallback
    }

    /// 克隆 VT 查询通道（见 [`Self::query_on`]）。
    pub(crate) fn query_channel(&self) -> Sender<Query> {
        self.query_tx.clone()
    }

    /// 当前终端标题；查询失败时回退为空串。
    pub fn title(&self) -> String {
        Self::title_on(&self.query_tx)
    }

    /// [`Self::title`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    /// 标题随每次状态更新从 Kotlin 侧拉取，频率与击键同级。
    pub(crate) fn title_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(
            query_tx,
            Query::Title,
            DISCONNECTED_TITLE.to_string(),
            "title",
        )
    }

    /// 终端从 OSC 7 记录的工作目录（原始值，通常是 `file://host/path`）。
    ///
    /// shell 在每次提示符前上报；空串表示尚未上报，调用方不得把它当成「根目录」。
    pub fn pwd(&self) -> String {
        Self::pwd_on(&self.query_tx)
    }

    /// [`Self::pwd`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn pwd_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(query_tx, Query::Pwd, DISCONNECTED_PWD.to_string(), "pwd")
    }

    /// 回滚区行数；查询失败时回退为 0。
    pub fn scrollback_length(&self) -> u32 {
        Self::scrollback_length_on(&self.query_tx)
    }

    /// [`Self::scrollback_length`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn scrollback_length_on(query_tx: &Sender<Query>) -> u32 {
        Self::query_on(
            query_tx,
            Query::ScrollbackLength,
            DISCONNECTED_SCROLLBACK,
            "scrollback_length",
        )
    }

    /// 读取指定行的文本；该行不存在时为 None。
    pub fn read_line_text(&self, row: u32) -> Option<String> {
        Self::read_line_text_on(&self.query_tx, row)
    }

    /// [`Self::read_line_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn read_line_text_on(query_tx: &Sender<Query>, row: u32) -> Option<String> {
        Self::query_on(
            query_tx,
            |tx| Query::ReadLineText { row, tx },
            None,
            "read_line_text",
        )
    }

    /// 读取视口内的全部文本。
    pub fn read_visible_text(&self) -> String {
        Self::read_visible_text_on(&self.query_tx)
    }

    /// [`Self::read_visible_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn read_visible_text_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(
            query_tx,
            Query::ReadVisibleText,
            String::new(),
            "read_visible_text",
        )
    }

    /// 视口 + 回滚区全量文本。`dump_grid` 的等价替代：调用方只需字符，无需样式。
    pub fn read_all_text(&self) -> String {
        Self::read_all_text_on(&self.query_tx)
    }

    /// [`Self::read_all_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn read_all_text_on(query_tx: &Sender<Query>) -> String {
        Self::query_on(query_tx, Query::ReadAllText, String::new(), "read_all_text")
    }

    /// 提取选区文本（用上游格式化器：软换行合并、去尾随空白）。
    pub fn selection_text(&self, start: (u32, u32), end: (u32, u32)) -> String {
        Self::selection_text_on(&self.query_tx, start, end)
    }

    /// [`Self::selection_text`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn selection_text_on(
        query_tx: &Sender<Query>,
        start: (u32, u32),
        end: (u32, u32),
    ) -> String {
        Self::query_on(
            query_tx,
            |tx| Query::SelectionText { start, end, tx },
            String::new(),
            "selection_text",
        )
    }

    /// 在指定单元导出词选区并安装，返回有序边界；无可选词时为 None。
    pub fn select_word_at(&self, row: u32, col: u32) -> Option<((u32, u32), (u32, u32))> {
        Self::select_word_at_on(&self.query_tx, row, col)
    }

    /// [`Self::select_word_at`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn select_word_at_on(
        query_tx: &Sender<Query>,
        row: u32,
        col: u32,
    ) -> Option<((u32, u32), (u32, u32))> {
        Self::query_on(
            query_tx,
            |tx| Query::SelectWordAt { row, col, tx },
            None,
            "select_word_at",
        )
    }

    /// 该行中作为宽字符后半格的列号，升序。查询失败时为空 vec（等价于无吸附）：
    /// 保持原列，不猜。
    pub fn wide_char_tail_cols(&self, row: u32) -> Vec<u32> {
        Self::wide_char_tail_cols_on(&self.query_tx, row)
    }

    /// [`Self::wide_char_tail_cols`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    /// 本方法每次拖动手柄都调用，故不能持双锁等 VT 应答。
    pub(crate) fn wide_char_tail_cols_on(query_tx: &Sender<Query>, row: u32) -> Vec<u32> {
        Self::query_on(
            query_tx,
            |tx| Query::WideCharTailCols { row, tx },
            Vec::new(),
            "wide_char_tail_cols",
        )
    }

    /// 导出全部可选内容并安装，返回有序边界；无可选内容时为 None。
    pub fn select_all(&self) -> Option<((u32, u32), (u32, u32))> {
        Self::select_all_on(&self.query_tx)
    }

    /// [`Self::select_all`] 的通道外置版，理由同 [`Self::encode_key_event_on`]。
    pub(crate) fn select_all_on(query_tx: &Sender<Query>) -> Option<((u32, u32), (u32, u32))> {
        Self::query_on(query_tx, |tx| Query::SelectAll { tx }, None, "select_all")
    }

    /// 查询网格单元处的 OSC 8 超链接 URI（行 0 = 回滚顶部，与 scrollbackLine 一致）；
    /// 无链接时为 None。
    pub fn hyperlink_at(&self, row: u32, col: u32) -> Option<String> {
        Self::hyperlink_at_on(&self.query_tx, row, col)
    }

    /// [`Self::hyperlink_at`] 的通道外置版，理由同 [`Self::title_on`]。
    pub(crate) fn hyperlink_at_on(query_tx: &Sender<Query>, row: u32, col: u32) -> Option<String> {
        Self::query_on(
            query_tx,
            |tx| Query::HyperlinkAt { row, col, tx },
            None,
            "hyperlink_at",
        )
    }

    /// 设置搜索 needle 并同步跑完搜索，返回命中总数；空 needle 清除搜索。
    ///
    /// 匹配、软换行跨行与增量同步均由上游 `search::Search` 承担，搜索对象随
    /// VT 线程存活，故连续查询复用同一份索引状态。
    pub fn search_set_needle(&self, needle: &str) -> usize {
        Self::search_set_needle_on(&self.query_tx, needle)
    }

    /// [`Self::search_set_needle`] 的通道外置版（见 [`Self::query_on`]）。
    pub(crate) fn search_set_needle_on(query_tx: &Sender<Query>, needle: &str) -> usize {
        Self::query_on(
            query_tx,
            |tx| Query::SearchSetNeedle {
                needle: needle.to_string(),
                tx,
            },
            0,
            "search_set_needle",
        )
    }

    /// 选中下一个（`forward = true`，朝更旧内容）或上一个命中，返回选中态。
    /// 无命中时 None；上游不移动视口（`Scroll::None`），居中由宿主完成。
    pub fn search_select(&self, forward: bool) -> Option<SearchSelection> {
        Self::search_select_on(&self.query_tx, forward)
    }

    /// [`Self::search_select`] 的通道外置版（见 [`Self::query_on`]）。
    /// 推进搜索游标非幂等，故只发一次。
    pub(crate) fn search_select_on(
        query_tx: &Sender<Query>,
        forward: bool,
    ) -> Option<SearchSelection> {
        Self::query_on_attempts(
            query_tx,
            |tx| Query::SearchSelect { forward, tx },
            None,
            "search_select",
            1,
        )
    }

    /// 绘制视口高亮所需的全部输入（视口命中、选中命中、回滚长度）。
    pub fn search_viewport_highlights(&self) -> SearchViewportHighlights {
        Self::search_viewport_highlights_on(&self.query_tx)
    }

    /// [`Self::search_viewport_highlights`] 的通道外置版（见 [`Self::query_on`]）。
    pub(crate) fn search_viewport_highlights_on(
        query_tx: &Sender<Query>,
    ) -> SearchViewportHighlights {
        Self::query_on(
            query_tx,
            |tx| Query::SearchViewportHighlights { tx },
            SearchViewportHighlights {
                matches: Vec::new(),
                selected: Vec::new(),
                scrollback_lines: 0,
            },
            "search_viewport_highlights",
        )
    }

    /// 导出整个网格（活动区 + 回滚区），供跨 FFI 序列化。
    pub fn dump_grid(&self) -> DumpedGrid {
        self.query(
            |tx| Query::DumpGrid { tx },
            DumpedGrid {
                rows: 0,
                cols: 0,
                visible: Vec::new(),
                scrollback: Vec::new(),
            },
            "dump_grid",
        )
    }
}
