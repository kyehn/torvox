//! 单元格实例构建器——把终端网格状态转换为 GPU 实例数据。
//!
//! 要求：
//! - FR-050——surface 生命周期：resize / surface 重建时重建单元格实例
mod emit;

use crate::render::CellInstance;
use crate::slow_log::SlowLogThrottle;
use crate::terminal::CursorStyle;
use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::ghostty_terminal::KITTY_PLACEHOLDER;
use crate::terminal::ghostty_terminal::build_row_ranges;
use crate::terminal::ghostty_terminal::cell_flags;

use foldhash::fast::RandomState;

use emit::append_row_instances;

/// 同一帧内为拿到稳定图集的最大发射遍数。
///
/// 第一遍发射时可能仍在解算本帧尚未装入的字形（例如刚从脏行以外来的簇形），
/// 触发一次搬迁；再多给两遍足以让「零星缺失」收敛。再多遍不会带来新的判定
/// 信息——若容量本身不够，遍数再多也不收敛（见循环末的告警）。
const MAX_STABLE_ATTEMPTS: usize = 3;

/// 「图集抖动」告警的限频闸门。
///
/// 工作集超过图集容量时每一遍都要搬迁字形，UV 永远无法在帧内收敛。这是
/// 逐帧发生的故障形态，若逐帧输出会把 logcat 刷成洪水（见 `logcat-hygiene`
/// 规范），故与其他逐帧诊断同口径限频。
static ATLAS_THRASH_THROTTLE: SlowLogThrottle = SlowLogThrottle::new();

/// 搜索高亮 alpha 超过该阈值时交换前景/背景色
/// （高 alpha = 不透明高亮，交换后视觉更清晰）。
/// 低于该阈值只做混合（淡着色）。
const SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD: u8 = 128;

/// 当前选中命中的高亮 alpha：完全不透明。
pub const SEARCH_HIGHLIGHT_CURRENT_ALPHA: u8 = 255;

/// 其余命中的高亮 alpha：同样反色但降低不透明度，使当前项仍能突出。
///
/// 取 160 是因为它仍 ≥ `SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD`（128），故与当前项
/// 走同一条「交换前景/背景再混合」的分支，只是不透明度不同。
pub const SEARCH_HIGHLIGHT_OTHER_ALPHA: u8 = 160;
/// 竖线光标宽度占单元格宽度比例（DECSCUSR 竖线样式）。
const BAR_CURSOR_WIDTH_FRACTION: f32 = 0.25;
/// 下划线光标高度占单元格高度比例（DECSCUSR 下划线样式）。
const UNDERLINE_CURSOR_HEIGHT_FRACTION: f32 = 0.15;
/// 光标标记最小厚度像素，保证低分辨率下仍然可见。
const CURSOR_MARKER_MINIMUM_THICKNESS: f32 = 1.0;
/// 竖线/下划线光标标记的不透明度系数：细条压在字形之上，半透明保证原文可读。
/// 方块光标不走这条路径——它整格反色，背景是不透明的光标色。
const CURSOR_MARKER_ALPHA_SCALE: f32 = 0.7;

use std::collections::{HashMap, HashSet};

/// 把网格码位转成 `char`：非法值退化为 U+FFFD，但**必记一条 error**。
///
/// libghostty 只产出合法标量值，故这里不该被走到。原先这里只有一个 `unwrap_or`
/// （外加一个 `debug_assert!`）——release 构建里非法值会被静默替换成方框，现场不留
/// 任何痕迹，而 DESIGN 明令禁止掩盖错误。保留替换本身是刻意的：这是渲染热路径，
/// panic 只会换来黑屏而非更好的诊断。
fn codepoint_to_char(codepoint: u32) -> char {
    if let Some(character) = char::from_u32(codepoint) {
        character
    } else {
        static WARNED: std::sync::Once = std::sync::Once::new();
        WARNED.call_once(|| {
            log::error!("cell_builder: 网格给出非法码位 {codepoint:#x}，按 U+FFFD 渲染");
        });
        '�'
    }
}

/// 供 `build_instances_from_cell_data()` 渲染光标用的光标状态。
#[derive(Debug, Clone, Copy, Default)]
pub struct CellCursor {
    /// 光标所在网格行号。
    pub row: u32,
    /// 光标所在网格列号。
    pub col: u32,
    /// 光标是否可见。
    pub visible: bool,
    /// 光标形状。
    pub style: CursorStyle,
    /// 当前主题的光标色，由调用方从 [`crate::terminal::theme`] 解析后传入。
    pub color: [f32; 4],
}

impl CellCursor {
    /// 竖线/下划线光标标记的半透明背景。
    ///
    /// 细条压在字形之上，保持原文可读；方块光标不用它——方块光标整格反色
    /// （见 `append_row_instances`），背景是不透明的光标色。
    fn marker_background(self) -> [f32; 4] {
        let cursor_color = self.color;
        [
            cursor_color[0],
            cursor_color[1],
            cursor_color[2],
            cursor_color[3] * CURSOR_MARKER_ALPHA_SCALE,
        ]
    }
}

/// 一次单元格实例构建所需的配置。
///
/// 全量构建（[`build_instances_from_cell_data`]）与增量构建
/// （`build_instances_cached`）共用；大块数据仍作独立参数传入：
/// 单元缓冲、可变的字体管线以及输出
/// 实例缓冲。
#[derive(Debug, Clone, Copy)]
pub struct CellInstanceConfig<'a> {
    /// 网格行数。
    pub rows: u32,
    /// 网格列数。
    pub cols: u32,
    /// 单元格宽（像素）。
    pub grid_cell_width: f32,
    /// 单元格高（像素）。
    pub grid_cell_height: f32,
    /// 光标状态（位置、可见性与形状）。
    pub cursor: CellCursor,
    /// 字形图集宽（像素）。
    pub atlas_width: f32,
    /// 字形图集高（像素）。
    pub atlas_height: f32,
    /// 搜索高亮区间（行范围与颜色）。
    pub search_highlights: &'a [SearchHighlight],
    /// 主题背景色；空单元与它同色时无需再画背景四边形，清除色已经铺好。
    pub default_background: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// 搜索结果高亮的一段行范围。
pub struct SearchHighlight {
    /// 高亮所在行（绝对行号）。
    pub row: i32,
    /// 高亮起始列（含）。
    pub start_col: i32,
    /// 高亮结束列（不含，开区间上界）。
    pub end_col_exclusive: i32,
    /// 高亮颜色 [R, G, B, A]。
    pub color: [u8; 4],
}

/// 空白单元不画背景四边形的前提：无光标、无装饰线，且背景与清除色一致。
///
/// 全量帧由 `LoadOp::Clear` 铺好背景，部分帧由脏带清除实例铺好；
/// 只有与主题背景同色的空白格才能安全省略，否则会丢掉显式背景色。
fn blank_cell_needs_quad(
    flags: u32,
    is_cursor: bool,
    background: [f32; 4],
    default_background: [f32; 4],
) -> bool {
    const DECORATIONS: u32 = (1 << cell_flags::UNDERLINE)
        | (1 << cell_flags::STRIKETHROUGH)
        | (1 << cell_flags::OVERLINE)
        | (1 << cell_flags::DOUBLE_UNDERLINE);
    is_cursor
        || flags & DECORATIONS != 0
        || background
            .iter()
            .zip(default_background)
            .any(|(actual, expected)| (actual - expected).abs() > 1.0 / 255.0)
}

/// 判断某单元格是否落在搜索高亮区间内。
pub(crate) fn cell_highlight<'a>(
    row: u32,
    col: u32,
    by_row: &'a HashMap<i32, Vec<&'a SearchHighlight>, RandomState>,
) -> Option<&'a [u8; 4]> {
    let highlight_list = by_row.get(&(row as i32))?;
    let highlight = highlight_list.iter().find(|candidate| {
        (col as i32) >= candidate.start_col && (col as i32) < candidate.end_col_exclusive
    })?;
    Some(&highlight.color)
}

/// 把高亮 RGBA 混合进浮点颜色（字节通道经终端侧同源函数归一）。
pub(crate) fn blend_highlight(base: [f32; 4], hl_rgba: [u8; 4]) -> [f32; 4] {
    let alpha = GhosttyTerminal::byte_to_float(hl_rgba[3]);
    if alpha <= 0.0 {
        return base;
    }
    let hr = GhosttyTerminal::byte_to_float(hl_rgba[0]);
    let hg = GhosttyTerminal::byte_to_float(hl_rgba[1]);
    let hb = GhosttyTerminal::byte_to_float(hl_rgba[2]);
    [
        base[0] * (1.0 - alpha) + hr * alpha,
        base[1] * (1.0 - alpha) + hg * alpha,
        base[2] * (1.0 - alpha) + hb * alpha,
        1.0,
    ]
}

#[inline]
/// 把搜索高亮 RGBA 施加到单元格前景/背景。
///
/// 混合基准恒为**原背景**：交换只决定文字色（取原背景，反色后仍可读），不参与
/// 底色计算。若先交换再拿交换后的背景当基准，基准已变成原前景色；默认主题的
/// 选区色又正是前景色（无 `[colors.selection]` 时上游回退到前景），于是
/// `blend(前景, 前景)` 与 alpha 无关——当前命中与其余命中渲染成同一块颜色，
/// 用户看到的正是「两种高亮没有区别」。
pub(crate) fn apply_search_highlight(
    foreground: &mut [f32; 4],
    background: &mut [f32; 4],
    hl: [u8; 4],
) {
    let original_background = *background;
    if hl[3] >= SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD {
        std::mem::swap(foreground, background);
    }
    *background = blend_highlight(original_background, hl);
}

/// 连续的一段脏网格行，并解析出其实例切片。
///
/// 由 [`compute_dirty_bands`] + `CachedInstances::band_slice` 产出，
/// 供 GPU 脏带渲染路径消费（render-vulkan-performance）：
/// 只提交该带的实例，也只触碰累加器的带区域，
/// 干净行保留上一帧像素。
#[derive(Debug, Clone, PartialEq)]
pub struct DirtyBand {
    /// 该带首个脏行（网格索引）。
    pub start_row: usize,
    /// 该带最后一个脏行的后一行。
    pub end_row_exclusive: usize,
    /// 本帧实例列表中的 `[instance_start, instance_end)` 切片
    /// （行主序；带就是一段行，故连续）。
    pub instance_start: usize,
    /// 本帧实例列表中该带实例切片的末尾（不含）。
    pub instance_end: usize,
}

impl DirtyBand {
    /// 该带是否覆盖 `rows` 高网格的全部行——即
    /// 与整帧重绘无从区分。
    #[cfg(test)]
    pub fn covers_all_rows(&self, rows: u32) -> bool {
        self.start_row == 0 && self.end_row_exclusive >= rows as usize
    }
}

/// 累加器架构下每帧的呈现计划。
#[derive(Debug, Default, Clone)]
pub struct FramePatch {
    /// 需重绘的脏行带（`load: Load`，保留原有内容）。
    pub bands: Vec<DirtyBand>,
    /// 一个网格行的渲染像素高度（脏带清除实例的几何）。
    pub cell_height_px: f32,
}

/// 把行脏标记合并成最少的连续 `[start, end)` 行段。
/// 纯函数——表驱动单测（`compute_dirty_bands_tests`）。
#[must_use]
pub fn compute_dirty_bands(dirty: &[bool]) -> Vec<(usize, usize)> {
    let mut bands: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for (row, &d) in dirty.iter().enumerate() {
        if d {
            if start.is_none() {
                start = Some(row);
            }
        } else if let Some(s) = start.take() {
            bands.push((s, row));
        }
    }
    if let Some(s) = start {
        bands.push((s, dirty.len()));
    }
    bands
}

/// 视口内最后一个有内容的行（0 起）；视口全空返回 `None`。
///
/// 「有内容」= 该行存在码点不为 `NUL`/空格/制表的单元。单元数据行优先且行号
/// 单调不减（与 `build_row_ranges` 同一约定），故自尾部回溯遇见的首个内容单元
/// 必属行号最大的有内容行——首个命中即返回，无需逐行成段、无分配。
///
/// 空格与制表**不**计内容：空白行在像素上与背景无异，若计入则「内容较少」
/// 退化为「网格填满」，输入法跟随平移又会整体上移，把首行内容推出屏幕
/// （实测提示符由 y=134 落到 y=−686）。
///
/// 行号越界（≥ `rows`）的单元跳过：布局元数据与单元数据失配时的陈旧数据，
/// 与 `build_row_ranges` 返回 `None` 的判据同源，不可据以上报下沿。
///
/// 行坐标即渲染视口坐标：`cells` 已按视口滚动偏移取样（见 `set_scroll_offset`
/// → `scroll_viewport`），故视口滚入回滚区时返回的仍是屏幕上真实可见的那一行。
/// 颜色填充（SGR 48）而无字形的单元不计内容——背景色已按主题默认值解析，
/// 无法与默认背景区分。
#[must_use]
pub fn last_content_row(
    cells: &[crate::terminal::ghostty_terminal::CellData],
    rows: u32,
) -> Option<u32> {
    cells
        .iter()
        .rev()
        .find(|cell| cell.row < rows && is_content_codepoint(cell.codepoint))
        .map(|cell| cell.row)
}

/// 码点是否为可见内容：排除 `NUL`（未写入）、空格与制表（终端内已展开为空格）。
fn is_content_codepoint(codepoint: u32) -> bool {
    !matches!(char::from_u32(codepoint), None | Some(' ' | '\t' | '\0'))
}

/// 增量渲染的行级实例缓存（FR-013 / NFR-010）。
///
/// 与全量路径 `build_instances_from_cell_data`（`row_ends` + 每行的实例切片）
/// 保持一致，差异由 `cached_incremental_rebuild_matches_full_build`
/// 逐实例比对：构建后 `row_ends[r]` 即
/// 行 `r` 的实例在 `instances` 中的结束下标（不含）。干净行可
/// 直接从上一帧复制，不必再经字体图集
/// 遍历其单元格（NFR-010：只重绘脏行）。
#[derive(Debug)]
pub struct CachedInstances {
    row_ends: Vec<usize>,
    instances: Vec<CellInstance>,
    rows: u32,
    cols: u32,
    /// 是否有构建填充过本缓存。新建的缓存
    /// （如 resize 刚结束时）在尺寸上「兼容」新网格，
    /// 却不含任何行数据：从中提供「干净」行会
    /// 复制 0 个实例，导致丢行回归）。
    built: bool,
    /// 缓存实例构建时所依据的字体图集代际。
    /// 图集重建与字形驱逐会搬迁 UV，故代际不匹配时
    /// 强制全量重建（陈旧 UV 会渲染出错或
    /// 空白，直到各行碰巧重新变脏）。
    atlas_generation: u64,
}

impl CachedInstances {
    /// 创建空的实例缓存：按给定行列数预留行边界。
    #[must_use]
    pub fn new(rows: u32, cols: u32) -> Self {
        Self {
            row_ends: vec![0; rows as usize],
            instances: Vec::new(),
            rows,
            cols,
            built: false,
            atlas_generation: 0,
        }
    }

    /// 上次构建的完整实例列表（行主序，见 `row_ends`）。
    #[must_use]
    pub fn instances(&self) -> &[CellInstance] {
        &self.instances
    }

    /// 缓存是否仍匹配当前网格尺寸（resize 会
    /// 使行布局失效并强制全量重建）且持有上次构建的行
    /// 数据。空的、从未构建过的缓存不可用于
    /// 增量提供。
    #[must_use]
    pub fn is_compatible(&self, rows: u32, cols: u32) -> bool {
        self.built && self.rows == rows && self.cols == cols && self.row_ends.len() == rows as usize
    }

    /// 缓存实例构建时所依据的图集代际。
    /// 脏带切片还额外要求代际相等：
    /// 重建/驱逐会搬迁 UV，陈旧代际下的稀疏带会让
    /// 干净行采样到已搬迁的区域（空白/错字）。
    #[must_use]
    pub fn built_atlas_generation(&self) -> u64 {
        self.atlas_generation
    }

    /// `row` 所属的 `[start, end)` 实例切片。
    pub(crate) fn row_slice(&self, row: usize) -> (usize, usize) {
        let start = if row == 0 { 0 } else { self.row_ends[row - 1] };
        (start, self.row_ends[row])
    }

    /// 覆盖行 `start_row..end_row` 的 `[start, end)` 实例切片
    /// （连续：实例为行主序）。调用方须保证
    /// `end_row <= self.row_ends.len()`。
    pub(crate) fn band_slice(&self, start_row: usize, end_row: usize) -> (usize, usize) {
        debug_assert!(
            end_row >= start_row && end_row <= self.row_ends.len(),
            "band_slice 的行区间越界：start_row={start_row}, end_row={end_row}, 行数={}",
            self.row_ends.len()
        );
        let start = self.row_slice(start_row).0;
        let end = self.row_slice(end_row.saturating_sub(1)).1;
        (start, end)
    }

    /// 构建后替换缓存内容，并盖上实例构建所依据的
    /// 图集代际。
    fn update(
        &mut self,
        rows: u32,
        cols: u32,
        instances: &[CellInstance],
        row_ends: Vec<usize>,
        atlas_generation: u64,
    ) {
        self.rows = rows;
        self.cols = cols;
        self.row_ends = row_ends;
        self.instances.clear();
        self.instances.extend_from_slice(instances);
        self.atlas_generation = atlas_generation;
        self.built = true;
    }

    /// 仅测试：直接写入 `row_ends`（`band_slice` 解析测试无需
    /// 真实实例数据，只要累计布局）。
    #[cfg(test)]
    pub(crate) fn update_for_test(&mut self, row_ends_cumulative: &[usize]) {
        self.row_ends = row_ends_cumulative.to_vec();
        let total = *row_ends_cumulative.last().unwrap_or(&0);
        let zeroed: crate::render::CellInstance = bytemuck::Zeroable::zeroed();
        self.instances = vec![zeroed; total];
        self.built = true;
    }
}

/// 簇文本写入调用方缓冲：主体码位后接全部有效组合记号（`grapheme_extra` 中的 0
/// 为占位，跳过）。
///
/// 预热遍与发射遍都需要它，故只此一处构造；写入跨单元复用的缓冲，命中整形
/// 缓存时整条路径不做堆分配（此前每单元一次 `String` 分配）。
fn cluster_text_into(
    text: &mut String,
    character: char,
    grapheme_extra: &[u32; crate::terminal::ghostty_terminal::MAX_GRAPHEME_EXTRA],
) {
    text.clear();
    text.push(character);
    for &codepoint in grapheme_extra {
        if codepoint == 0 {
            continue;
        }
        if let Some(mark) = char::from_u32(codepoint) {
            text.push(mark);
        }
    }
}

/// 按字节比较两段行内单元切片。`CellData` 是 POD
/// bytemuck 结构，故这样做有效且快于逐字段比对。
fn rows_equal(
    old: &[crate::terminal::ghostty_terminal::CellData],
    new: &[crate::terminal::ghostty_terminal::CellData],
) -> bool {
    let old_bytes: &[u8] = bytemuck::cast_slice(old);
    let new_bytes: &[u8] = bytemuck::cast_slice(new);
    old_bytes == new_bytes
}

/// 零分配的逐行脏标记：写入预分配缓冲
/// （长度须 ≥ `rows`）。避免 120fps 下每帧分配
/// `Vec<bool>`（约 300 字节 × 120 = 36KB/s）。
pub(crate) fn diff_dirty_rows_into(
    old: &[crate::terminal::ghostty_terminal::CellData],
    new: &[crate::terminal::ghostty_terminal::CellData],
    rows: u32,
    dirty: &mut [bool],
) {
    debug_assert!(
        dirty.len() >= rows as usize,
        "脏标记数组比行数短：rows={rows}, dirty.len()={}",
        dirty.len()
    );
    let (Some(old_ranges), Some(new_ranges)) =
        (build_row_ranges(old, rows), build_row_ranges(new, rows))
    else {
        dirty[..rows as usize].fill(true);
        return;
    };
    for row_index in 0..rows as usize {
        let old_row = &old[old_ranges[row_index].clone()];
        let new_row = &new[new_ranges[row_index].clone()];
        dirty[row_index] = !rows_equal(old_row, new_row);
    }
}

/// 把预构建的 `CellData` 切片转换为 GPU 实例数据。
///
/// 携带搜索高亮参数，使渲染器无需完整 `GridSnapshot`
/// 即可给出视觉反馈（搜索高亮叠加）。
/// 选区高亮归终端所有：VT 线程把受跟踪选区的反显
/// 烘焙进 `CellData` 颜色，故本构建器
/// 自身不做选区处理。
///
/// 转换失败（如字体图集不可用）时返回 `None`。
pub fn build_instances_from_cell_data(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    config: CellInstanceConfig<'_>,
    font_pipeline: &mut crate::render::font::FontPipeline,
    instances: &mut Vec<CellInstance>,
) -> Option<()> {
    build_row_instances_into(cell_data, config, font_pipeline, None, None, instances)
}

/// [`build_instances_from_cell_data`] 的增量变体（行级脏缓存，
/// FR-013 / NFR-010）：只重建 `dirty_rows` 中标记的行，
/// 干净行原样从 `cache` 复制；`cache` 就地更新，
/// 下一帧即可复用。脏标记短于 `rows`，
/// 或缓存与网格尺寸不兼容时，降级为全部行的
/// 完整重建。
pub fn build_instances_cached(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    config: CellInstanceConfig<'_>,
    font_pipeline: &mut crate::render::font::FontPipeline,
    dirty_rows: &[bool],
    cache: &mut CachedInstances,
    instances: &mut Vec<CellInstance>,
) -> Option<()> {
    build_row_instances_into(
        cell_data,
        config,
        font_pipeline,
        Some(dirty_rows),
        Some(cache),
        instances,
    )
}

/// 全量与增量构建器背后的共用实现。
///
/// 为每个网格行构建四边形实例。提供了 `dirty_rows` 与 `cache`
/// 且二者一致时，干净行直接复制其缓存实例，
/// 而不再经字体图集遍历其单元格。
fn build_row_instances_into(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    config: CellInstanceConfig<'_>,
    font_pipeline: &mut crate::render::font::FontPipeline,
    dirty_rows: Option<&[bool]>,
    mut cache: Option<&mut CachedInstances>,
    instances: &mut Vec<CellInstance>,
) -> Option<()> {
    let CellInstanceConfig {
        rows,
        cols,
        grid_cell_width,
        grid_cell_height,
        cursor,
        atlas_width,
        atlas_height,
        search_highlights,
        default_background,
    } = config;
    // 四边形几何用调用方给的网格单元尺寸，即 FONT 单元
    // 大小（cell_metrics * raster_scale），而不是 surface/rows。
    // 按 surface/rows 定尺寸会在高屏幕上留下行间空隙
    // （2209px / 24 行 = 92px，而字体单元约 20px）。
    let (cell_width, cell_height) = (grid_cell_width, grid_cell_height);
    // 用 trace 级别：每次脏重建都会触发；info 级别会淹没
    // logcat（逐行 binder IPC）并引发帧时间抖动。
    log::trace!(
        "cell_builder: grid {rows}x{cols} cell {cell_width:.1}x{cell_height:.1} cells={}",
        cell_data.len()
    );
    let ascent_pixels = font_pipeline.ascent_pixels();
    let raster_scale = font_pipeline.get_raster_scale();
    // 跨帧复用缓冲：Vec 由调用方（Renderer）持有，
    // 在此清空，避免 60fps 下每帧约 100KB 的分配
    // （约 6MB/s 分配流量）。
    instances.clear();
    instances.reserve(cell_data.len());

    // 用 foldhash（0.2，已在依赖树中）而非 std 默认的
    // SipHash13：该 map 每帧重建并查询
    // （60fps 约 1920 次哈希/帧）；foldhash 在 i32 键上快约 5-10 倍。
    // 无高亮时（常见情形）跳过每帧 HashMap 重建，
    // 每帧省下约 1920 次哈希。
    let mut highlights_by_row: HashMap<i32, Vec<&SearchHighlight>, RandomState> =
        HashMap::with_hasher(RandomState::default());
    if !search_highlights.is_empty() {
        for highlight in search_highlights {
            highlights_by_row
                .entry(highlight.row)
                .or_default()
                .push(highlight);
        }
    }

    // 一次性划分为每行范围；同时用于增量
    // 脏行判定与下面的逐行迭代。
    let row_ranges = build_row_ranges(cell_data, rows)?;
    // 增量提供要求尺寸兼容**且**图集代际为当前值：
    // 重建/驱逐会搬迁字形 UV，故按更旧代际缓存的实例
    // 会渲染出错或空白。
    let mut incremental = dirty_rows.is_some_and(|dirty_flags| dirty_flags.len() >= rows as usize)
        && cache.as_ref().is_some_and(|instance_cache| {
            instance_cache.is_compatible(rows, cols)
                && instance_cache.atlas_generation == font_pipeline.atlas_generation()
        });

    // 组合簇文本的跨单元复用缓冲：预热遍与发射遍共用，命中整形缓存时零分配。
    let mut cluster_text = String::new();
    // 发射前预热：本帧要重建的行里的字形先光栅化（结果丢弃）。atlas 驱逐搬迁 UV，
    // 帧内 rasterize 会使同帧早建实例失效（同字不同区渲染不一致）。
    // 预热后发射遍为纯查表，UV 稳定；若代际仍变化（防御），缓存已热，
    // 重建一遍即稳定，最多两遍。
    let generation_at_entry = font_pipeline.atlas_generation();
    // 增量帧只重建脏行：干净行的字形早已在图集里——缓存代际一致即证明它们
    // 没被驱逐、UV 也没搬迁——预热整屏纯属白费（实测整屏预热 68µs，
    // 而一行脏行的重建只要几微秒）。全量重建才预热整屏。
    warm_frame_glyphs(
        cell_data,
        &row_ranges,
        if incremental { dirty_rows } else { None },
        cursor,
        cell_width,
        cell_height,
        font_pipeline,
        &mut cluster_text,
    );
    if incremental && font_pipeline.atlas_generation() != generation_at_entry {
        // 预热把干净行的字形挤出了图集：缓存行的旧 UV 已失效，必须降级为全量
        // 重建；全量重建要整帧字形都在图集里，否则发射遍边建边光栅化会再次驱逐。
        incremental = false;
        warm_frame_glyphs(
            cell_data,
            &row_ranges,
            None,
            cursor,
            cell_width,
            cell_height,
            font_pipeline,
            &mut cluster_text,
        );
    }
    let mut row_ends: Vec<usize> = Vec::with_capacity(rows as usize);
    // 末遍是否在稳定代际下建完：只有稳定才允许把实例写回缓存。
    let mut built_stable = false;
    for _ in 0..MAX_STABLE_ATTEMPTS {
        instances.clear();
        row_ends.clear();
        let generation_before_emit = font_pipeline.atlas_generation();
        for (row, range) in row_ranges.iter().enumerate() {
            let is_clean = incremental && dirty_rows.is_some_and(|dirty| !dirty[row]);
            if is_clean {
                // 干净行：复用上一帧构建的实例（NFR-010）。
                // `incremental` 已蕴含 `cache.is_some()`，故这里必命中。
                let cache_ref = cache.as_ref().expect("incremental 蕴含缓存存在");
                let (cs, ce) = cache_ref.row_slice(row);
                instances.extend_from_slice(&cache_ref.instances()[cs..ce]);
            } else {
                append_row_instances(
                    cell_width,
                    cell_height,
                    ascent_pixels,
                    raster_scale,
                    atlas_width,
                    atlas_height,
                    cursor,
                    &highlights_by_row,
                    default_background,
                    font_pipeline,
                    instances,
                    &cell_data[range.clone()],
                    &mut cluster_text,
                );
            }
            row_ends.push(instances.len());
        }
        if font_pipeline.atlas_generation() == generation_before_emit {
            built_stable = true;
            break;
        }
        // 发射中代际变更：已建实例与缓存 clean 行部分 stale，
        // 降级为全量再建一遍（缓存已热，收敛）。
        incremental = false;
    }
    // 只有末遍全程未推进代际才写回缓存：否则实例里的 UV 属于被驱逐前的位置，
    // 却会被打上推进后的代际——下一帧增量判定误判为「同代际」，
    // 干净行于是照着已搬迁的 UV 采样（空白/错字）。
    // 留空缓存让下一帧整体重建：代际不匹配本就会触发它。
    if let Some(c) = cache.as_mut()
        && built_stable
    {
        c.update(
            rows,
            cols,
            &instances[..],
            row_ends,
            font_pipeline.atlas_generation(),
        );
    } else if !built_stable && ATLAS_THRASH_THROTTLE.should_log() {
        // 遍数用尽仍不稳定 = 本帧工作集在图集里装不下：每次查询都把另一字形挤出去，
        // 增遍数亦不可能收敛。此时必须出声——画面上的字会在同一个字符之间出现
        // 不同的字形轮廓（stale UV 的典型症状），而归因所需的唯一线索就是「图集
        // 抖动」。不出声则现象只会由用户上报，毫无现场信息。
        let (atlas_width, atlas_height) = (atlas_width as u32, atlas_height as u32);
        log::warn!(
            "cell_builder: atlas {atlas_width}x{atlas_height} thrashes at {rows}x{cols} cells \
             (workingset exceeds the atlas; raise the atlas size or lower the raster scale)"
        );
    }
    Some(())
}

/// 预热遍的去重键：码位 + 粗斜样式位。
///
/// Unicode 标量值占低 21 位（上限 U+10FFFF），粗/斜各占 1 位。
type WarmKey = u32;

/// 预热键的粗体位。
const WARM_BOLD_BIT: u32 = 1 << 21;

/// 预热键的斜体位。
const WARM_ITALIC_BIT: u32 = 1 << 22;

/// 预热遍的簇去重键：主体字符 + 附加码位 + 粗斜。
///
/// 用原始附加码位数组而非拼好的簇文本做键：簇文本要拼 `String`（每单元一次
/// 分配），而附加码位数组是 POD，可直接哈希。
type WarmClusterKey = (
    char,
    [u32; crate::terminal::ghostty_terminal::MAX_GRAPHEME_EXTRA],
    bool,
    bool,
);

/// 预热遍的计数初值（仅测试用）。
///
/// 刻意走非常量函数：clippy 在 aarch64-linux-android 目标上把 `thread_local!`
/// 的 const 初始化误判为「可改成 const」（host 目标不报），而本仓禁止在生产
/// 代码里写 `#[allow]`——非常量初始化即让该 lint 不再插手。
#[cfg(test)]
fn initial_warm_count() -> u64 {
    0
}

#[cfg(test)]
thread_local! {
    /// 预热遍访问过的单元数（仅测试用）。
    ///
    /// 线程局部而非全局：Rust 的测试用例并行跑在一个进程里，全局计数会被别的
    /// 用例的构建搅脏，断言就成了随机失败。
    static WARM_CELLS_VISITED: std::cell::Cell<u64> =
        std::cell::Cell::new(initial_warm_count());

    /// 预热遍向字体管线发出的字形请求数（仅测试用）。
    static WARM_GLYPH_REQUESTS: std::cell::Cell<u64> =
        std::cell::Cell::new(initial_warm_count());
}

/// 预热遍累计访问过的单元数（仅测试用）。
///
/// 回归钉要能「数」它：只断言构建输出相等只能证明「结果对」，证不了
/// 「没白扫整屏」——增量帧的预热范围退化成整屏时，输出照样全对。
#[cfg(test)]
pub(crate) fn warm_cells_visited() -> u64 {
    WARM_CELLS_VISITED.with(|counter| counter.get())
}

/// 预热遍累计发出的字形请求数（仅测试用）。
///
/// 计数的是**请求**而非光栅化：请求本身要走图集分配器的查找与 LRU 记账，
/// 一屏 1920 单元里同一个字形出现几十次时，逐格请求就是白做的查表。
#[cfg(test)]
pub(crate) fn warm_glyph_requests() -> u64 {
    WARM_GLYPH_REQUESTS.with(|counter| counter.get())
}

/// 预热本帧**要重建的行**里的全部字形（结果丢弃），保证发射遍为纯查表、UV 稳定。
/// 字形附加项走与发射遍完全相同的 overlay/shaping 入口，
/// 使回退链光栅化也在发射前完成。
///
/// 只覆盖本帧会重建的行（`dirty_rows` 为 `None` 时是整屏）：增量帧只重建脏行，
/// 干净行的字形早已在图集里——缓存代际一致即证明它们没被驱逐、UV 也没搬迁。
///
/// 同一字形在一帧里出现几十次（1920 单元通常只含几十种字形），预热只需做一次：
/// 去重表按（码位, 粗, 斜）记账，重复出现直接跳过。
fn warm_frame_glyphs(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    row_ranges: &[std::ops::Range<usize>],
    dirty_rows: Option<&[bool]>,
    cursor: CellCursor,
    cell_width: f32,
    cell_height: f32,
    font_pipeline: &mut crate::render::font::FontPipeline,
    cluster_text: &mut String,
) {
    if cursor.visible {
        // 空单元格光标路径的参考字形（'M' 优先，'0' 兜底），与发射遍一致。
        let _ = font_pipeline.glyph_information('M');
        let _ = font_pipeline.glyph_information('0');
    }
    let mut warmed: HashSet<WarmKey, RandomState> = HashSet::with_hasher(RandomState::default());
    let mut warmed_clusters: HashSet<WarmClusterKey, RandomState> =
        HashSet::with_hasher(RandomState::default());
    for (row, range) in row_ranges.iter().enumerate() {
        // 掩码短于行数时按脏处理：宁可多预热，不可漏预热——漏了就是帧内光栅化，
        // 即同帧早建实例的 UV 失效。
        if dirty_rows.is_some_and(|dirty| !dirty.get(row).copied().unwrap_or(true)) {
            continue;
        }
        for cell_data_entry in &cell_data[range.clone()] {
            #[cfg(test)]
            WARM_CELLS_VISITED.with(|counter| counter.set(counter.get() + 1));
            if (cell_data_entry.flags >> cell_flags::CONCEAL) & 1 == 1 {
                continue;
            }
            let character = codepoint_to_char(cell_data_entry.codepoint);
            if character == ' '
                || character == '\0'
                || cell_data_entry.codepoint == 0
                || cell_data_entry.codepoint == KITTY_PLACEHOLDER
            {
                continue;
            }
            let bold = (cell_data_entry.flags >> cell_flags::BOLD) & 1 == 1;
            let italic = (cell_data_entry.flags >> cell_flags::ITALIC) & 1 == 1;
            let key = character as u32
                | if bold { WARM_BOLD_BIT } else { 0 }
                | if italic { WARM_ITALIC_BIT } else { 0 };
            if warmed.insert(key) {
                #[cfg(test)]
                WARM_GLYPH_REQUESTS.with(|counter| counter.set(counter.get() + 1));
                if bold || italic {
                    let _ = font_pipeline.glyph_information_styled(character, bold, italic);
                } else {
                    let _ = font_pipeline.glyph_information(character);
                }
            }
            let has_cluster = cell_data_entry
                .grapheme_extra
                .iter()
                .any(|&codepoint| codepoint != 0);
            if !has_cluster {
                continue;
            }
            let cluster_key = (character, cell_data_entry.grapheme_extra, bold, italic);
            if !warmed_clusters.insert(cluster_key) {
                continue;
            }
            cluster_text_into(cluster_text, character, &cell_data_entry.grapheme_extra);
            if cluster_text.chars().count() <= 1 {
                continue;
            }
            #[cfg(test)]
            WARM_GLYPH_REQUESTS.with(|counter| counter.set(counter.get() + 1));
            let shaped = font_pipeline.shape_run_styled(cluster_text, bold, italic);
            match shaped.len() {
                1 => {
                    let glyph = &shaped[0];
                    let _ =
                        font_pipeline.glyph_information_for_glyph(glyph.font_id, glyph.glyph_id);
                }
                len if len > 1 => {
                    for glyph in shaped.iter().skip(1) {
                        let _ = font_pipeline
                            .glyph_information_for_glyph(glyph.font_id, glyph.glyph_id);
                    }
                }
                _ => {
                    let dummy_quad = || crate::render::font::OverlayQuad {
                        origin: [0.0; 2],
                        size: [cell_width, cell_height],
                        foreground: [0.0; 4],
                        background: [0.0; 4],
                        deco: [0.0; 4],
                        flags: 0.0,
                    };
                    for codepoint in &cell_data_entry.grapheme_extra {
                        if *codepoint == 0 {
                            continue;
                        }
                        let _ = font_pipeline.overlay_glyph_instance(
                            *codepoint,
                            dummy_quad(),
                            cell_height,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
