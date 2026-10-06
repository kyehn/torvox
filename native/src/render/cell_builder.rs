//! 单元格实例构建器——把终端网格状态转换为 GPU 实例数据。
//!
//! 要求：
//! - FR-050——surface 生命周期：resize / surface 重建时重建单元格实例
use crate::render::CellInstance;

use crate::terminal::CursorStyle;
use crate::terminal::ghostty_terminal::cell_flags;

use foldhash::fast::RandomState;

/// 搜索高亮 alpha 超过该阈值时交换前景/背景色
/// （高 alpha = 不透明高亮，交换后视觉更清晰）。
/// 低于该阈值只做混合（淡着色）。
const SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD: u8 = 128;
/// 竖线光标宽度占单元格宽度比例（DECSCUSR 竖线样式）。
const BAR_CURSOR_WIDTH_FRACTION: f32 = 0.25;
/// 下划线光标高度占单元格高度比例（DECSCUSR 下划线样式）。
const UNDERLINE_CURSOR_HEIGHT_FRACTION: f32 = 0.15;
/// 光标标记最小厚度像素，保证低分辨率下仍然可见。
const CURSOR_MARKER_MINIMUM_THICKNESS: f32 = 1.0;
/// 方块光标背景透明度系数（半透明覆盖保证原文可读）。
const BLOCK_CURSOR_BACKGROUND_ALPHA_SCALE: f32 = 0.7;
/// 未指定光标颜色时的默认光标颜色（不透明白色）。
const DEFAULT_CURSOR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
use std::collections::HashMap;

/// 供 build_instances_from_cell_data() 渲染光标用的光标状态。
#[derive(Debug, Clone, Copy, Default)]
pub struct CellCursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub style: CursorStyle,
    pub color: Option<[f32; 4]>,
}

impl CellCursor {
    /// 解析后的光标颜色，未指定时使用默认白色。
    fn resolved_color(self) -> [f32; 4] {
        self.color.unwrap_or(DEFAULT_CURSOR_COLOR)
    }

    /// 半透明光标标记背景，保证原文可读。
    fn marker_background(self) -> [f32; 4] {
        let cursor_color = self.resolved_color();
        [
            cursor_color[0],
            cursor_color[1],
            cursor_color[2],
            cursor_color[3] * BLOCK_CURSOR_BACKGROUND_ALPHA_SCALE,
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
    pub rows: u32,
    pub cols: u32,
    pub grid_cell_width: f32,
    pub grid_cell_height: f32,
    pub cursor: CellCursor,
    pub atlas_width: f32,
    pub atlas_height: f32,
    pub search_highlights: &'a [SearchHighlight],
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// 搜索结果高亮的一段行范围。
pub struct SearchHighlight {
    pub row: i32,
    pub start_col: i32,
    pub end_col_exclusive: i32,
    pub color: [u8; 4],
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

/// 把高亮 RGBA 混合进浮点颜色。
pub(crate) fn blend_highlight(base: [f32; 4], hl_rgba: [u8; 4]) -> [f32; 4] {
    let alpha = hl_rgba[3] as f32 / 255.0;
    if alpha <= 0.0 {
        return base;
    }
    let hr = hl_rgba[0] as f32 / 255.0;
    let hg = hl_rgba[1] as f32 / 255.0;
    let hb = hl_rgba[2] as f32 / 255.0;
    [
        base[0] * (1.0 - alpha) + hr * alpha,
        base[1] * (1.0 - alpha) + hg * alpha,
        base[2] * (1.0 - alpha) + hb * alpha,
        1.0,
    ]
}

#[inline]
/// 把搜索高亮 RGBA 施加到单元格前景/背景。
pub(crate) fn apply_search_highlight(
    foreground: &mut [f32; 4],
    background: &mut [f32; 4],
    hl: [u8; 4],
) {
    if hl[3] >= SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD {
        std::mem::swap(foreground, background);
    }
    *background = blend_highlight(*background, hl);
}

/// 连续的一段脏网格行，并解析出其实例切片。
///
/// 由 [`compute_dirty_bands`] + [`CachedInstances::band_slice`] 产出，
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
/// 纯函数——表驱动单测（compute_dirty_bands_tests）。
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
    !matches!(
        char::from_u32(codepoint),
        None | Some(' ') | Some('\t') | Some('\0')
    )
}

/// 增量渲染的行级实例缓存（FR-013 / NFR-010）。
///
/// 与测试专用参照实现 `snapshot_reference::build_cell_instances_into`
/// （row_ends + 每行的实例切片）保持一致：构建后 `row_ends[r]` 即
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
    pub fn instances(&self) -> &[CellInstance] {
        &self.instances
    }

    /// 缓存是否仍匹配当前网格尺寸（resize 会
    /// 使行布局失效并强制全量重建）且持有上次构建的行
    /// 数据。空的、从未构建过的缓存不可用于
    /// 增量提供。
    pub fn is_compatible(&self, rows: u32, cols: u32) -> bool {
        self.built && self.rows == rows && self.cols == cols && self.row_ends.len() == rows as usize
    }

    /// 缓存实例构建时所依据的图集代际。
    /// 脏带切片还额外要求代际相等：
    /// 重建/驱逐会搬迁 UV，陈旧代际下的稀疏带会让
    /// 干净行采样到已搬迁的区域（空白/错字）。
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
        debug_assert!(end_row >= start_row && end_row <= self.row_ends.len());
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

    /// 仅测试：直接写入 row_ends（band_slice 解析测试无需
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

/// 依 `CellData.row` 把扁平行主序的 `cell_data` 切片划分为每行范围。
/// 宽字符/间隔单元被终端省略，故各行长度不同；
/// 无单元的行映射为空范围。存在超出 `rows` 的
/// 单元（或输入非行主序）时返回 `None`——
/// 属陈旧/失配数据，不可用于增量构建。
pub(crate) fn build_row_ranges(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    rows: u32,
) -> Option<Vec<std::ops::Range<usize>>> {
    let mut ranges: Vec<std::ops::Range<usize>> = (0..rows as usize).map(|_| 0..0).collect();
    let mut current = 0usize;
    for (r, range) in ranges.iter_mut().enumerate() {
        let start = current;
        while current < cell_data.len() && cell_data[current].row as usize == r {
            current += 1;
        }
        *range = start..current;
    }
    if current < cell_data.len() {
        return None;
    }
    Some(ranges)
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
    debug_assert!(dirty.len() >= rows as usize);
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
/// 携带搜索高亮参数，使渲染器无需完整 GridSnapshot
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
    let _ = (rows, cols); // 调用方用于投影；四边形网格覆盖全部
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
    let incremental = dirty_rows.is_some_and(|dirty_flags| dirty_flags.len() >= rows as usize)
        && cache.as_ref().is_some_and(|instance_cache| {
            instance_cache.is_compatible(rows, cols)
                && instance_cache.atlas_generation == font_pipeline.atlas_generation()
        });

    // 发射前预热：本帧所有字形先光栅化（结果丢弃）。atlas 驱逐搬迁 UV，
    // 帧内 rasterize 会使同帧早建实例失效（同字不同区渲染不一致）。
    // 预热后发射遍为纯查表，UV 稳定；若代际仍变化（防御），缓存已热，
    // 重建一遍即稳定，最多两遍。
    let generation_at_entry = font_pipeline.atlas_generation();
    warm_frame_glyphs(cell_data, cursor, cell_width, cell_height, font_pipeline);
    // 预热本身可能驱逐并搬迁 UV：此时缓存行的旧 UV 已失效，
    // 必须降级为全量重建（预热后为纯查表，代价低且正确）。
    let mut incremental = incremental && font_pipeline.atlas_generation() == generation_at_entry;
    let mut row_ends: Vec<usize> = Vec::with_capacity(rows as usize);
    // 末遍是否在稳定代际下建完：只有稳定才允许把实例写回缓存。
    let mut built_stable = false;
    for _ in 0..2 {
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
                    font_pipeline,
                    instances,
                    &cell_data[range.clone()],
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
    }
    Some(())
}

/// 预热本帧全部字形（结果丢弃），保证发射遍为纯查表、UV 稳定。
/// 字形附加项走与发射遍完全相同的 overlay/shaping 入口，
/// 使回退链光栅化也在发射前完成。
fn warm_frame_glyphs(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    cursor: CellCursor,
    cell_width: f32,
    cell_height: f32,
    font_pipeline: &mut crate::render::font::FontPipeline,
) {
    if cursor.visible {
        // 空单元格光标路径的参考字形（'M' 优先，'0' 兜底），与发射遍一致。
        let _ = font_pipeline.glyph_information('M');
        let _ = font_pipeline.glyph_information('0');
    }
    for cd in cell_data {
        let ch = char::from_u32(cd.codepoint).unwrap_or('�');
        if ch == ' ' || ch == '\0' || cd.codepoint == 0 {
            continue;
        }
        let bold = (cd.flags >> cell_flags::BOLD) & 1 == 1;
        let italic = (cd.flags >> cell_flags::ITALIC) & 1 == 1;
        if bold || italic {
            let _ = font_pipeline.glyph_information_styled(ch, bold, italic);
        } else {
            let _ = font_pipeline.glyph_information(ch);
        }
        let has_cluster = cd.grapheme_extra.iter().any(|&codepoint| codepoint != 0);
        if !has_cluster {
            continue;
        }
        let mut cluster_text = String::new();
        cluster_text.push(ch);
        for codepoint in &cd.grapheme_extra {
            if *codepoint == 0 {
                continue;
            }
            if let Some(mark) = char::from_u32(*codepoint) {
                cluster_text.push(mark);
            }
        }
        if cluster_text.chars().count() <= 1 {
            continue;
        }
        let shaped = font_pipeline.shape_run(&cluster_text);
        if shaped.len() == 1 {
            let glyph = &shaped[0];
            let _ = font_pipeline.glyph_information_for_glyph(glyph.font_id, glyph.glyph_id);
        } else if shaped.len() > 1 {
            for glyph in shaped.iter().skip(1) {
                let _ = font_pipeline.glyph_information_for_glyph(glyph.font_id, glyph.glyph_id);
            }
        } else {
            let dummy_quad = || crate::render::font::OverlayQuad {
                origin: [0.0; 2],
                size: [cell_width, cell_height],
                foreground: [0.0; 4],
                background: [0.0; 4],
                deco: [0.0; 4],
                flags: 0.0,
            };
            for codepoint in &cd.grapheme_extra {
                if *codepoint == 0 {
                    continue;
                }
                let _ = font_pipeline.overlay_glyph_instance(*codepoint, dummy_quad(), cell_height);
            }
        }
    }
}

/// 构建单个网格行的实例。全量与增量构建器共用，
/// 使两条路径的单元级逻辑保持一致。
// 渲染热路径：参数由双构建路径共享调用，成组改结构体只增间接无收益。
fn append_row_instances(
    cell_width: f32,
    cell_height: f32,
    ascent_pixels: f32,
    raster_scale: f32,
    atlas_width: f32,
    atlas_height: f32,
    cursor: CellCursor,
    highlights_by_row: &HashMap<i32, Vec<&SearchHighlight>, RandomState>,
    font_pipeline: &mut crate::render::font::FontPipeline,
    instances: &mut Vec<CellInstance>,
    cell_row: &[crate::terminal::ghostty_terminal::CellData],
) {
    for cd in cell_row {
        // 转换前校验码点——非法值应当
        // 在 debug 构建中被捕获，避免终端内容缺陷被掩盖。
        debug_assert!(
            char::from_u32(cd.codepoint).is_some(),
            "cell_builder: invalid codepoint: {}",
            cd.codepoint
        );
        let ch = char::from_u32(cd.codepoint).unwrap_or('�');
        let cell_span = cd.width.max(1) as f32;
        let quad_origin = [cd.col as f32 * cell_width, cd.row as f32 * cell_height];
        let mut foreground = cd.foreground;
        let mut background = cd.background;
        // SGR 7 反显：交换前景与背景色
        // 检查反显属性（新布局中为第 2 位，与旧路径
        // pack_style_flags → shader 所用的 `cell.reverse` 位位置一致）。
        // 与 termux TerminalRenderer.java:182-187 一致（选区与
        // reverseVideo 归入同一处前景/背景交换），也与 Ghostty 的
        // 渲染器反显处理一致。
        if (cd.flags >> cell_flags::REVERSE) & 1 == 1 {
            std::mem::swap(&mut foreground, &mut background);
        }

        // 搜索高亮叠加（施加在终端烘焙的
        // 选区反显之上）。
        if let Some(hl) = cell_highlight(cd.row, cd.col, highlights_by_row) {
            apply_search_highlight(&mut foreground, &mut background, *hl);
        }
        // (spec cursor-rendering "宽字符光标几何"): 宽字符
        // 光标必须覆盖整个字形。build_cell_data 为每个宽字符只发出一条
        // width=2 的 CellData（尾部网格列没有自己的
        // CellData——review-1 已验证 internal.rs 会消费两列），故
        // 首条记录的 `cell_span` 已跨整个字形：Bar 标记按 cell_span
        // 缩放正是让光标覆盖两列的手段。
        // 不存在需要另行标记的间隔单元记录。
        let is_cursor = cursor.visible && cd.row == cursor.row && cd.col == cursor.col;
        let effective_foreground = foreground;
        let mut effective_background = background;
        // 默认四边形尺寸（方块光标与空单元格共用）
        let quad_size = [cell_width * cell_span, cell_height];
        // 方块光标高度跟随字形（ascent+descent 以物理
        // 像素计），而非整个网格单元——420dpi 下整格高的光标块
        // 看着就是包住 ~66px 字形的巨大实心矩形，而单元只有
        // 79px 高。

        if is_cursor && matches!(cursor.style, CursorStyle::Block) {
            // 方块光标（独占样式）：保留原文前景保证可读，仅把背景
            // 替换为光标色半透明覆盖。
            effective_background = cursor.marker_background();
        }

        // 整尺寸字形四边形（避免组合记号等被 Bar/Underline
        // 光标标记尺寸裁掉）。
        let glyph_quad_size = [cell_width * cell_span, cell_height];
        let glyph_quad_origin = [cd.col as f32 * cell_width, cd.row as f32 * cell_height];
        if ch == ' ' || ch == '\0' || cd.codepoint == 0 {
            // 空单元格：不发背景四边形，只有下面的光标块。
            {
                let mut origin = quad_origin;
                let mut size = quad_size;
                // 空单元格上的方块光标：像下面的非空路径那样，
                // 让光标块与字形盒精确对齐——顶边 = 基线 −
                // 参考字形的 placement.top，高度 = 该参考字形的
                // 位图高度。quad_origin 是单元顶边（shader
                // 原样占据 [origin, origin+size]；bearing 只在框内
                // 移动位图），故在此加上完整 ascent 会把光标块
                // 压低一行——即「光标块比文字低一行」
                // 的报告（，模拟器实测：
                // VT 光标 (0,38)，光标块像素落在第 1 行）。
                if is_cursor {
                    let marker_background = cursor.marker_background();
                    let reference = font_pipeline
                        .glyph_information('M')
                        .or_else(|| font_pipeline.glyph_information('0'));
                    // 参考字形盒：顶边与高度与非空路径一致，保证空
                    // 单元格光标与文本行对齐。
                    let (glyph_top, glyph_height) = match reference {
                        Some(info) => (
                            ascent_pixels * raster_scale - info.placement.top as f32,
                            (info.height as f32).max(1.0),
                        ),
                        None => (
                            0.0,
                            ((ascent_pixels + font_pipeline.descent_pixels()) * raster_scale)
                                .max(1.0),
                        ),
                    };
                    match cursor.style {
                        CursorStyle::Block => {
                            origin[1] += glyph_top;
                            size[1] = glyph_height;
                        }
                        CursorStyle::Bar => {
                            // 竖线光标：单元格左侧细竖条，高度与字形盒一致。
                            origin[1] += glyph_top;
                            size[0] = (cell_width * BAR_CURSOR_WIDTH_FRACTION)
                                .max(CURSOR_MARKER_MINIMUM_THICKNESS);
                            size[1] = glyph_height;
                            effective_background = marker_background;
                        }
                        CursorStyle::Underline => {
                            // 下划线光标：字形盒底部细横条，宽度覆盖整格。
                            let marker_height = (cell_height * UNDERLINE_CURSOR_HEIGHT_FRACTION)
                                .max(CURSOR_MARKER_MINIMUM_THICKNESS);
                            origin[1] += glyph_top + glyph_height - marker_height;
                            size[1] = marker_height;
                            effective_background = marker_background;
                        }
                    }
                }
                instances.push(CellInstance {
                    quad_origin: origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    foreground: effective_foreground,
                    background: effective_background,
                    underline_color: cd.underline_color,
                    quad_size: size,
                    flags: cd.flags as f32,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
            }
            continue;
        }

        // 非空单元格：先发字形四边形，再在其上追加光标标记
        // （Bar/Underline 时才有），使细竖线/下划线
        // 在字形之上可见。
        // 主体字形——单元带 bold/italic 标志时使用同族
        // styled 字面，否则合成）。
        let cell_bold = (cd.flags >> cell_flags::BOLD) & 1 == 1;
        let cell_italic = (cd.flags >> cell_flags::ITALIC) & 1 == 1;
        // 组合字形延续符的簇整形（组合记号、emoji ZWJ
        // 序列）：整簇只整形一次，使每个记号落到字体
        // 指定的位置。无附加符的单元完全跳过整形，
        // 常见情形零开销。
        let has_cluster = cd.grapheme_extra.iter().any(|&codepoint| codepoint != 0);
        let mut cluster_text = String::new();
        if has_cluster {
            cluster_text.push(ch);
            for codepoint in &cd.grapheme_extra {
                if *codepoint == 0 {
                    continue;
                }
                if let Some(mark) = char::from_u32(*codepoint) {
                    cluster_text.push(mark);
                }
            }
        }
        let cluster_shaped: Vec<crate::render::font::ShapedGlyphInfo> =
            if cluster_text.chars().count() > 1 {
                font_pipeline.shape_run(&cluster_text)
            } else {
                Vec::new()
            };
        // 整簇整形成单个字形（ZWJ emoji）时，它取代主体字形
        // 成为主体四边形；带位置的记号交由下面的 overlay
        // 循环处理。
        let merged_cluster_glyph = if cluster_shaped.len() == 1 {
            let glyph = &cluster_shaped[0];
            font_pipeline.glyph_information_for_glyph(glyph.font_id, glyph.glyph_id)
        } else {
            None
        };
        let has_merged_glyph = merged_cluster_glyph.is_some();
        if let Some(info) = merged_cluster_glyph
            .or_else(|| font_pipeline.glyph_information_styled(ch, cell_bold, cell_italic))
        {
            let uv_x = info.atlas_x as f32 / atlas_width;
            let uv_y = info.atlas_y as f32 / atlas_height;
            let uv_w = info.width as f32 / atlas_width;
            let uv_h = info.height as f32 / atlas_height;
            let bearing_x = info.placement.left as f32;
            // info.height 是光栅化位图高度，单位为物理像素
            // （已乘 raster_scale）。与之比较的是物理网格单元高度；
            // 居中回退同样按物理像素给出——
            // 单位不可混用）。
            let glyph_h_px = info.height as f32;
            let raw_bearing_y = ascent_pixels * raster_scale - info.placement.top as f32;
            let bearing_y = if glyph_h_px > cell_height {
                (cell_height - glyph_h_px) / 2.0
            } else {
                raw_bearing_y
            };
            let mut origin = glyph_quad_origin;
            let mut size = glyph_quad_size;
            // 方块光标与字形位图同高同顶边：字形坐在基线上而非
            // 单元格中心，顶边即基线减放置顶部偏移。
            if is_cursor && matches!(cursor.style, CursorStyle::Block) {
                let cursor_h = glyph_h_px.max(1.0);
                origin[1] += raw_bearing_y;
                size[1] = cursor_h;
            }

            instances.push(CellInstance {
                quad_origin: origin,
                atlas_offset: [uv_x, uv_y],
                atlas_size: [uv_w, uv_h],
                foreground: effective_foreground,
                background: effective_background,
                underline_color: cd.underline_color,
                quad_size: size,
                flags: cd.flags as f32,
                bearing: [bearing_x, bearing_y],
                glyph_advance_width: info.advance_width,
            });

            // 竖线/下划线光标：在字形之上追加细标记（原文颜色不动，
            // 标记盖在上层保证可见）。
            if is_cursor && !matches!(cursor.style, CursorStyle::Block) {
                let marker_background = cursor.marker_background();
                let glyph_top = glyph_quad_origin[1] + raw_bearing_y;
                let glyph_height = glyph_h_px.max(1.0);
                let (marker_origin, marker_size) = match cursor.style {
                    CursorStyle::Bar => (
                        [glyph_quad_origin[0], glyph_top],
                        [
                            (cell_width * BAR_CURSOR_WIDTH_FRACTION)
                                .max(CURSOR_MARKER_MINIMUM_THICKNESS),
                            glyph_height,
                        ],
                    ),
                    _ => {
                        let marker_height = (cell_height * UNDERLINE_CURSOR_HEIGHT_FRACTION)
                            .max(CURSOR_MARKER_MINIMUM_THICKNESS);
                        (
                            [
                                glyph_quad_origin[0],
                                glyph_top + glyph_height - marker_height,
                            ],
                            [cell_width * cell_span, marker_height],
                        )
                    }
                };
                instances.push(CellInstance {
                    quad_origin: marker_origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    foreground: effective_foreground,
                    background: marker_background,
                    underline_color: cd.underline_color,
                    quad_size: marker_size,
                    flags: cd.flags as f32,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
            }

            // 组合字形延续码点（组合记号、emoji ZWJ 等），
            // 作为 overlay 实例渲染在主体字形之上，位置由上面的
            // 簇整形给出；整形无可用结果时按码点回退
            // 渲染。
            if cluster_shaped.len() > 1 {
                for glyph in cluster_shaped.iter().skip(1) {
                    if let Some(info) =
                        font_pipeline.glyph_information_for_glyph(glyph.font_id, glyph.glyph_id)
                    {
                        let shaped_origin = [
                            glyph_quad_origin[0] + glyph.x_offset,
                            glyph_quad_origin[1] + glyph.y_offset,
                        ];
                        if let Some(overlay) = font_pipeline.shaped_overlay_instance(
                            &info,
                            crate::render::font::OverlayQuad {
                                origin: shaped_origin,
                                size: glyph_quad_size,
                                foreground: effective_foreground,
                                background: effective_background,
                                deco: cd.underline_color,
                                flags: cd.flags as f32,
                            },
                            cell_height,
                        ) {
                            instances.push(overlay);
                        }
                    }
                }
            } else if !has_merged_glyph {
                for codepoint in &cd.grapheme_extra {
                    if *codepoint == 0 {
                        continue;
                    }
                    if let Some(overlay) = font_pipeline.overlay_glyph_instance(
                        *codepoint,
                        crate::render::font::OverlayQuad {
                            origin: glyph_quad_origin,
                            size: glyph_quad_size,
                            foreground: effective_foreground,
                            background: effective_background,
                            deco: cd.underline_color,
                            flags: cd.flags as f32,
                        },
                        cell_height,
                    ) {
                        instances.push(overlay);
                    }
                }
            }
        } else {
            // 图集中找不到字形——推入一个空背景四边形，使
            // 单元格背景（含选区/高亮）可见。
            // 字体图集重建后字形即出现。
            instances.push(CellInstance {
                quad_origin,
                atlas_offset: [0.0; 2],
                atlas_size: [0.0; 2],
                foreground: effective_foreground,
                background: effective_background,
                underline_color: cd.underline_color,
                quad_size,
                flags: cd.flags as f32,
                bearing: [0.0; 2],
                glyph_advance_width: 0.0,
            });
        }
    }
}

// ── 测试 ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::CellData;

    fn cell_data(
        row: u32,
        col: u32,
        ch: char,
        foreground: [f32; 4],
        background: [f32; 4],
        flags: u32,
    ) -> CellData {
        CellData {
            codepoint: ch as u32,
            width: 1,
            grapheme_extra: [0; 7],
            foreground,
            background,
            underline_color: foreground,
            flags,
            row,
            col,
        }
    }

    fn build(
        cells: &[CellData],
        cursor: CellCursor,
        highlights: &[SearchHighlight],
    ) -> Vec<CellInstance> {
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let mut instances = Vec::new();
        let result = build_instances_from_cell_data(
            cells,
            CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_width: 1024.0 / 80.0,
                grid_cell_height: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                search_highlights: highlights,
            },
            &mut font_pipeline,
            &mut instances,
        );
        assert!(
            result.is_some(),
            "build should succeed with a font pipeline"
        );
        instances
    }

    /// 无反显/高亮时，普通单元保持自身颜色。
    #[test]
    fn plain_cell_keeps_colors() {
        let cells = vec![cell_data(
            0,
            0,
            'A',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            0,
        )];
        let instances = build(&cells, CellCursor::default(), &[]);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].foreground, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(instances[0].background, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(instances[0].flags, 0.0);
    }

    /// SGR 7 反显交换前景与背景。
    #[test]
    fn reverse_swaps_foreground_background() {
        let cells = vec![cell_data(
            0,
            0,
            'A',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            1 << cell_flags::REVERSE,
        )];
        let instances = build(&cells, CellCursor::default(), &[]);
        assert_eq!(instances[0].foreground, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(instances[0].background, [1.0, 0.0, 0.0, 1.0]);
    }

    /// SGR 1/3 bold+italic 标志传到 GPU 实例（shader 样式位）。
    #[test]
    fn bold_italic_flags_reach_instance() {
        let flags = (1 << cell_flags::BOLD) | (1 << cell_flags::ITALIC);
        let cells = vec![cell_data(
            0,
            0,
            'I',
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            flags,
        )];
        let instances = build(&cells, CellCursor::default(), &[]);
        assert_eq!(
            instances[0].flags, flags as f32,
            "bold+italic flags must reach the instance"
        );
        assert!(
            instances[0].atlas_size[0] > 0.0 && instances[0].atlas_size[1] > 0.0,
            "styled cell must carry a real atlas quad, not a blank fallback"
        );
    }

    /// SGR 58 下划线颜色传到 GPU 实例，供 shader 装饰遍使用。
    #[test]
    fn underline_color_reaches_instance() {
        let mut decorated = cell_data(
            0,
            0,
            'U',
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            1 << cell_flags::UNDERLINE,
        );
        decorated.underline_color = [1.0, 0.0, 0.0, 1.0];
        let instances = build(&[decorated], CellCursor::default(), &[]);
        assert_eq!(
            instances[0].underline_color,
            [1.0, 0.0, 0.0, 1.0],
            "SGR 58 color must reach the instance deco channel"
        );
    }

    /// alpha >= 128 的搜索高亮先交换前景/背景，再混合背景。
    #[test]
    fn search_highlight_swaps_and_blends() {
        let cells = vec![cell_data(
            2,
            3,
            'D',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            0,
        )];
        let hl = SearchHighlight {
            row: 2,
            start_col: 3,
            end_col_exclusive: 4,
            color: [0xFF, 0xFF, 0x00, 0xFF], // 不透明黄
        };
        let instances = build(&cells, CellCursor::default(), &[hl]);
        // alpha >= 128 → 交换前景/背景，随后 background = blend(background, yellow, alpha=1) = yellow。
        assert_eq!(
            instances[0].foreground,
            [0.0, 0.0, 1.0, 1.0],
            "highlight swaps foreground→background"
        );
        assert_eq!(
            instances[0].background,
            [1.0, 1.0, 0.0, 1.0],
            "blend with opaque yellow"
        );
    }

    /// alpha 低于 128 的高亮只混合、不交换。
    #[test]
    fn search_highlight_blends_without_swap() {
        let cells = vec![cell_data(
            0,
            0,
            'E',
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            0,
        )];
        let hl = SearchHighlight {
            row: 0,
            start_col: 0,
            end_col_exclusive: 1,
            color: [0xFF, 0x00, 0x00, 0x7F], // alpha ~0.5 的红（低于 128 交换阈值）
        };
        let instances = build(&cells, CellCursor::default(), &[hl]);
        assert_eq!(
            instances[0].foreground,
            [0.0, 0.0, 0.0, 1.0],
            "foreground unchanged below alpha 128"
        );
        // background = white * (1-a) + red * a，其中 a = 0x7F/255
        let alpha = 0x7F as f32 / 255.0;
        let expected = 1.0 - alpha;
        assert!(
            (instances[0].background[0] - 1.0).abs() < 1e-5,
            "red channel keeps base white"
        );
        assert!((instances[0].background[1] - expected).abs() < 1e-5);
        assert!((instances[0].background[2] - expected).abs() < 1e-5);
    }

    /// 方块光标把背景换成光标色（半透明），
    /// 并保留原前景色，使字形仍可读。
    #[test]
    fn block_cursor_paints_background_keeps_foreground() {
        let cells = vec![cell_data(
            5,
            5,
            'F',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            0,
        )];
        let cursor = CellCursor {
            row: 5,
            col: 5,
            visible: true,
            style: CursorStyle::Block,
            color: Some([0.0, 1.0, 0.0, 1.0]),
        };
        let instances = build(&cells, cursor, &[]);
        assert_eq!(
            instances[0].foreground,
            [1.0, 0.0, 0.0, 1.0],
            "block cursor keeps foreground"
        );
        assert_eq!(
            instances[0].background,
            [0.0, 1.0, 0.0, 0.7],
            "background = cursor color * 0.7"
        );
    }

    /// 空单元格（空格）只发背景四边形，绝不发字形实例。
    #[test]
    fn empty_cell_emits_background_quad() {
        let cells = vec![cell_data(
            0,
            0,
            ' ',
            [0.9, 0.9, 0.9, 1.0],
            [0.1, 0.1, 0.1, 1.0],
            0,
        )];
        let instances = build(&cells, CellCursor::default(), &[]);
        assert_eq!(instances.len(), 1);
        assert_eq!(
            instances[0].atlas_size, [0.0; 2],
            "no glyph UVs for a space"
        );
        assert_eq!(instances[0].background, [0.1, 0.1, 0.1, 1.0]);
    }

    // 逐字段比较（CellInstance 未派生 PartialEq）。

    const TEST_GRID_ROWS: u32 = 24;
    const TEST_GRID_COLS: u32 = 80;

    /// 缓存构建测试所用的标准 24x80 配置。
    fn test_config(cursor: CellCursor) -> CellInstanceConfig<'static> {
        CellInstanceConfig {
            rows: TEST_GRID_ROWS,
            cols: TEST_GRID_COLS,
            grid_cell_width: 1024.0 / TEST_GRID_COLS as f32,
            grid_cell_height: 1024.0 / TEST_GRID_ROWS as f32,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: &[],
        }
    }

    /// 以标准测试配置执行一次缓存构建。
    fn run_cached_build(
        cells: &[CellData],
        cursor: CellCursor,
        mask: &[bool],
        font_pipeline: &mut crate::render::font::FontPipeline,
        cache: &mut CachedInstances,
        instances: &mut Vec<CellInstance>,
    ) -> Option<()> {
        build_instances_cached(
            cells,
            test_config(cursor),
            font_pipeline,
            mask,
            cache,
            instances,
        )
    }

    fn instances_equal(a: &[CellInstance], b: &[CellInstance]) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| {
                x.quad_origin == y.quad_origin
                    && x.atlas_offset == y.atlas_offset
                    && x.atlas_size == y.atlas_size
                    && x.foreground == y.foreground
                    && x.background == y.background
                    && x.quad_size == y.quad_size
                    && x.flags == y.flags
                    && x.bearing == y.bearing
                    && x.glyph_advance_width == y.glyph_advance_width
            })
    }

    /// 增量路径（build_instances_cached）：只让第 2 行变脏的一帧
    /// 必须产出与完整重建完全一致的实例，其中干净行
    /// 从缓存提供（NFR-010：只重绘脏行）。
    #[test]
    fn cached_incremental_rebuild_matches_full_build() {
        let mk = |row: u32, ch: char| {
            cell_data(row, 0, ch, [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0], 0)
        };
        let cells: Vec<CellData> = (0..24).map(|row| mk(row, 'a')).collect();
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };

        // 第 1 帧：全脏一遍以播种缓存。
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let mut instances = Vec::new();
        let mut cache = CachedInstances::new(24, 80);
        let all_dirty = vec![true; 24];
        let ok = run_cached_build(
            &cells,
            cursor,
            &all_dirty,
            &mut font_pipeline,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "initial full build should succeed");
        let full_frame1 = build(&cells, cursor, &[]);
        assert!(
            instances_equal(&instances, &full_frame1),
            "initial build equals a full build"
        );

        // 第 2 帧：仅第 2 行变化；其余各行必须从
        // 缓存提供，结果仍须等于完整重建。
        let mut cells2 = cells.clone();
        cells2[2] = mk(2, 'z');
        let mut dirty = vec![false; 24];
        dirty[2] = true;
        instances.clear();
        let ok = run_cached_build(
            &cells2,
            cursor,
            &dirty,
            &mut font_pipeline,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "incremental build should succeed");
        let full_frame2 = build(&cells2, cursor, &[]);
        assert!(
            instances_equal(&instances, &full_frame2),
            "incremental result must match a full rebuild"
        );
        // 缓存此刻保存第 2 帧的实例供下一帧复用。
        assert!(
            instances_equal(cache.instances(), &instances),
            "cache must be refreshed with the latest instances"
        );
    }

    /// 图集陈旧（重建或字形驱逐会搬迁 UV）：按更旧图集代际
    /// 构建的缓存绝不可提供干净行——该帧必须等于完整
    /// 重建，否则陈旧 UV 会渲染出错或空白，
    /// 直到各行碰巧重新变脏。
    #[test]
    fn cached_stale_atlas_generation_forces_full_rebuild() {
        let mk = |row: u32, ch: char| {
            cell_data(row, 0, ch, [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0], 0)
        };
        let cells: Vec<CellData> = (0..24).map(|row| mk(row, 'a')).collect();
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };

        // 第 1 帧：全脏一遍以播种缓存。
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let mut instances = Vec::new();
        let mut cache = CachedInstances::new(24, 80);
        let all_dirty = vec![true; 24];
        let ok = run_cached_build(
            &cells,
            cursor,
            &all_dirty,
            &mut font_pipeline,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "initial full build should succeed");

        // 模拟缓存填充之后的图集重建/驱逐。
        cache.atlas_generation = cache.atlas_generation.wrapping_add(1);

        // 第 2 帧改了第 2 行却声称无脏行：代际为当前时那行陈旧
        // 内容会从缓存被提供；代际陈旧时则必须整帧
        // 改为重建。
        let mut cells2 = cells.clone();
        cells2[2] = mk(2, 'z');
        let clean = vec![false; 24];
        instances.clear();
        let ok = run_cached_build(
            &cells2,
            cursor,
            &clean,
            &mut font_pipeline,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "rebuild after atlas change should succeed");
        let full_frame2 = build(&cells2, cursor, &[]);
        assert!(
            instances_equal(&instances, &full_frame2),
            "stale atlas generation must force a full rebuild, not serve cached rows"
        );
    }

    /// 帧内驱逐回归：小 atlas 上混合旧字形（clean 行）与大量新字形
    /// （dirty 行，迫使驱逐）的一帧，增量结果中每个实例的 UV 必须与
    /// 当前缓存查询一致。驱逐前构建/服务的旧 UV 即 stale（同字不同区
    /// 渲染不一致，如 "d 部分区域像 a"），必须被预热 + 全量降级消除。
    #[test]
    fn incremental_frame_with_midframe_eviction_has_current_uvs() {
        const ATLAS: f32 = 128.0;
        const ROWS: u32 = 24;
        const COLS_PER_ROW: u32 = 8;
        let pool: Vec<char> = ('A'..='Z')
            .chain('a'..='z')
            .chain('0'..='9')
            .chain("!@#$%^&*()_+-=[]{}|;:,.<>?/~`".chars())
            .collect();
        // 帧工作集（95 ASCII）适配 atlas；帧间 churn 逐出旧条目，
        // 第二帧预热/发射中必再次驱逐，覆盖“帧内驱逐致 stale”路径。
        assert!(pool.len() >= 90, "frame working set must be substantial");
        let mk = |row: u32, col: u32, ch: char, bold: bool| {
            cell_data(
                row,
                col,
                ch,
                [1.0, 1.0, 1.0, 1.0],
                [0.0, 0.0, 0.0, 1.0],
                if bold { 1 << cell_flags::BOLD } else { 0 },
            )
        };
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };
        let config = CellInstanceConfig {
            rows: ROWS,
            cols: 80,
            grid_cell_width: 1024.0 / 80.0,
            grid_cell_height: 1024.0 / 24.0,
            cursor,
            atlas_width: ATLAS,
            atlas_height: ATLAS,
            search_highlights: &[],
        };
        let mut font_pipeline =
            crate::render::font::FontPipeline::new(ATLAS as i32, ATLAS as i32, 14.0);
        let mut instances = Vec::new();
        let mut cache = CachedInstances::new(ROWS, 80);
        // Frame 1：全 dirty 播种缓存。
        let mut frame1 = Vec::new();
        for row in 0..ROWS {
            for column in 0..COLS_PER_ROW {
                let ch = pool[((row * COLS_PER_ROW + column) as usize) % pool.len()];
                frame1.push(mk(row, column, ch, (row + column) % 7 == 0));
            }
        }
        let all_dirty = vec![true; ROWS as usize];
        let ok = build_instances_cached(
            &frame1,
            config,
            &mut font_pipeline,
            &all_dirty,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "seed build should succeed");
        // 帧间 churn：直接光栅化 300+ 帧外字形，逐出第一帧条目
        // （DejaVu Sans Mono 覆盖希腊/西里尔/拉丁扩展-A/箭头，
        // 主字体直命中，无慢速全库扫描）。
        // 下一帧预热必须重新载入它们，再次驱逐——stale 场景。
        let churn: Vec<char> = ('\u{0391}'..='\u{03C9}')
            .chain('\u{0410}'..='\u{044F}')
            .chain('\u{0100}'..='\u{017F}')
            .chain('\u{2190}'..='\u{21FF}')
            .collect();
        assert!(churn.len() > 300, "churn set must exceed atlas headroom");
        for &ch in &churn {
            let _ = font_pipeline.glyph_information(ch);
        }
        // Frame 2：偶数行不变（clean），奇数行换新字形（dirty，迫使驱逐）。
        let mut frame2 = Vec::new();
        for row in 0..ROWS {
            for column in 0..COLS_PER_ROW {
                let idx = (row * COLS_PER_ROW + column) as usize;
                let ch = if row % 2 == 0 {
                    pool[idx % pool.len()]
                } else {
                    pool[(idx + 45) % pool.len()]
                };
                frame2.push(mk(row, column, ch, (row + column) % 7 == 0));
            }
        }
        let mut dirty = vec![false; ROWS as usize];
        for (row, dirty_entry) in dirty.iter_mut().enumerate() {
            *dirty_entry = row % 2 == 1;
        }
        instances.clear();
        let ok = build_instances_cached(
            &frame2,
            config,
            &mut font_pipeline,
            &dirty,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "incremental build should succeed");
        assert_eq!(
            instances.len(),
            frame2.len(),
            "each non-empty cell yields exactly one instance (no cursor/highlights)"
        );
        // 逐实例断言 UV 与当前查询一致；验证查询本身必须零变更
        // （纯命中），否则断言失去意义。
        let generation_before_verify = font_pipeline.atlas_generation();
        for (cell, instance) in frame2.iter().zip(instances.iter()) {
            let ch = char::from_u32(cell.codepoint).expect("test cells are valid");
            let bold = (cell.flags >> cell_flags::BOLD) & 1 == 1;
            let info = font_pipeline.glyph_information_styled(ch, bold, false);
            let info = info.expect("frame glyphs must resolve");
            assert!(
                info.width > 0 && info.height > 0,
                "glyph {ch:?} must have a bitmap"
            );
            assert_eq!(
                instance.atlas_offset,
                [info.atlas_x as f32 / ATLAS, info.atlas_y as f32 / ATLAS],
                "stale UV for {ch:?} (row {}, col {})",
                cell.row,
                cell.col
            );
            assert_eq!(
                instance.atlas_size,
                [info.width as f32 / ATLAS, info.height as f32 / ATLAS],
                "stale UV size for {ch:?} (row {}, col {})",
                cell.row,
                cell.col
            );
        }
        assert_eq!(
            font_pipeline.atlas_generation(),
            generation_before_verify,
            "verification lookups must be pure cache hits"
        );
    }

    /// 退化输入（缓存网格尺寸陈旧，或脏标记短于网格）
    /// 必须回退为完整重建，而不是
    /// 提供陈旧行。
    #[test]
    fn cached_degraded_input_falls_back_to_full_rebuild() {
        let cells: Vec<CellData> = (0..24)
            .map(|row| cell_data(row, 0, 'x', [1.0; 4], [0.0, 0.0, 0.0, 1.0], 0))
            .collect();
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let full = build(&cells, cursor, &[]);

        // 陈旧缓存：按 12 行网格构建，而网格已有 24 行。
        let mut cache = CachedInstances::new(12, 80);
        let mut instances = Vec::new();
        let ok = run_cached_build(
            &cells,
            cursor,
            &[true; 24],
            &mut font_pipeline,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some());
        assert!(
            instances_equal(&instances, &full),
            "stale cache must force a full rebuild"
        );

        // 脏标记短于网格（仅 10 行）。
        let mut cache2 = CachedInstances::new(24, 80);
        instances.clear();
        let ok = run_cached_build(
            &cells,
            cursor,
            &[true; 10],
            &mut font_pipeline,
            &mut cache2,
            &mut instances,
        );
        assert!(ok.is_some());
        assert!(
            instances_equal(&instances, &full),
            "a too-short dirty mask must force a full rebuild"
        );
    }

    /// 回归：与网格不再匹配的缓存（如仅列数变化的 resize，行数
    /// 不变）叠加部分脏标记时，仍须产出完整重建。新建的空缓存
    /// 看起来「兼容」（rows/cols 字段相同），从中提供「干净」行
    /// 会复制 0 个实例，于是把这些行从本帧丢掉。调用方
    /// （pass.rs）负责在重建缓存时换成全 true 的脏标记；本测试
    /// 钉住这一退化组合，使其永远不会静默通过。
    #[test]
    fn stale_cache_with_partial_mask_must_not_drop_rows() {
        let cells: Vec<CellData> = (0..24)
            .map(|row| cell_data(row, 0, 'x', [1.0; 4], [0.0, 0.0, 0.0, 1.0], 0))
            .collect();
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let full = build(&cells, cursor, &[]);

        // 模拟 resize 帧：缓存按 24x40 构建，而网格
        // 现为 24x80（仅列数变化）。为 24x80 重建的空缓存
        // 在尺寸上「兼容」，却不含任何行数据。
        let mut cache = CachedInstances::new(24, 80);
        let mut instances = Vec::new();
        // 部分标记：只有第 2 行脏（仅一行字节变化时单元 diff
        // 路径的产出）。
        let mut mask = vec![false; 24];
        mask[2] = true;
        let ok = run_cached_build(
            &cells,
            cursor,
            &mask,
            &mut font_pipeline,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some());
        // 每一行都必须在场：空缓存提供「干净」行时
        // 23 个干净行只会产出 0 个实例。
        assert_eq!(
            instances.len(),
            full.len(),
            "partial mask on an empty cache must not drop rows (got {} vs {})",
            instances.len(),
            full.len()
        );
        assert!(
            instances_equal(&instances, &full),
            "result must equal a full rebuild on cache mismatch"
        );
    }
    /// 空单元格上的方块光标必须留在光标自身所在行内：
    /// quad_origin 是单元顶边（shader 原样占据
    /// [origin, origin+size]），故顶边绝不可按完整 ascent
    /// 平移——那会把光标块压到文字下一行（模拟器实测：
    /// VT 光标 (0,38)，光标块像素落在第 1 行）。
    #[test]
    fn block_cursor_on_empty_cell_stays_in_its_row() {
        let cursor = CellCursor {
            row: 0,
            col: 38,
            visible: true,
            style: crate::terminal::ghostty_terminal::CursorStyle::Block,
            color: Some([1.0, 1.0, 1.0, 1.0]),
        };
        // 光标下仅一个空单元格（码点 0）。
        let cells = vec![cell_data(0, 38, '\0', [1.0; 4], [0.0; 4], 0)];
        let instances = build(&cells, cursor, &[]);
        assert_eq!(instances.len(), 1, "empty cursor cell emits one quad");
        let cell_height = 1024.0 / 24.0;
        let origin_y = instances[0].quad_origin[1];
        let bottom_y = origin_y + instances[0].quad_size[1];
        assert!(
            origin_y >= 0.0 && bottom_y <= cell_height + 0.5,
            "block quad spans y [{origin_y}, {bottom_y}] but row 0 ends at {cell_height}"
        );
        // 光标块必须与参考字形的字形盒对齐，与非空
        // 路径一致（顶边 = 基线 − placement.top）。
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        if let Some(reference) = font_pipeline.glyph_information('M') {
            let expected_top = font_pipeline.ascent_pixels() * font_pipeline.get_raster_scale()
                - reference.placement.top as f32;
            assert!(
                (origin_y - expected_top).abs() <= 1.5,
                "empty-cell block top {origin_y} != reference glyph top {expected_top}"
            );
        }
    }
    /// 竖线光标在空单元格必须是左侧细竖条，而非整格方块。
    #[test]
    fn bar_cursor_on_empty_cell_is_thin_vertical() {
        let cursor = CellCursor {
            row: 0,
            col: 5,
            visible: true,
            style: crate::terminal::ghostty_terminal::CursorStyle::Bar,
            color: Some([1.0, 1.0, 1.0, 1.0]),
        };
        let cells = vec![cell_data(0, 5, '\0', [1.0; 4], [0.0; 4], 0)];
        let instances = build(&cells, cursor, &[]);
        assert_eq!(instances.len(), 1, "empty bar cursor emits one quad");
        let cell_width = 1024.0 / 80.0;
        let cell_height = 1024.0 / 24.0;
        let width = instances[0].quad_size[0];
        let height = instances[0].quad_size[1];
        let origin_y = instances[0].quad_origin[1];
        assert!(
            width <= cell_width * 0.5,
            "bar width {width} must stay thin within cell {cell_width}"
        );
        assert!(
            width >= CURSOR_MARKER_MINIMUM_THICKNESS - 0.01,
            "bar width {width} must stay visible"
        );
        assert!(
            height >= CURSOR_MARKER_MINIMUM_THICKNESS - 0.01,
            "bar height {height} must stay visible"
        );
        assert!(
            origin_y >= 0.0 && origin_y + height <= cell_height + 0.5,
            "bar spans y [{origin_y}, {}] but row 0 ends at {cell_height}",
            origin_y + height
        );
        assert!(
            (instances[0].quad_origin[0] - 5.0 * cell_width).abs() <= 0.5,
            "bar must sit at the cell left edge"
        );
    }
    /// 下划线光标在空单元格必须是底部细横条，而非整格方块。
    #[test]
    fn underline_cursor_on_empty_cell_is_thin_horizontal() {
        let cursor = CellCursor {
            row: 0,
            col: 7,
            visible: true,
            style: crate::terminal::ghostty_terminal::CursorStyle::Underline,
            color: Some([1.0, 1.0, 1.0, 1.0]),
        };
        let cells = vec![cell_data(0, 7, '\0', [1.0; 4], [0.0; 4], 0)];
        let instances = build(&cells, cursor, &[]);
        assert_eq!(instances.len(), 1, "empty underline cursor emits one quad");
        let cell_width = 1024.0 / 80.0;
        let cell_height = 1024.0 / 24.0;
        let width = instances[0].quad_size[0];
        let height = instances[0].quad_size[1];
        assert!(
            (width - cell_width).abs() <= 0.5,
            "underline width {width} must span the cell {cell_width}"
        );
        assert!(
            height <= cell_height * 0.5,
            "underline height {height} must stay thin within cell {cell_height}"
        );
        let bottom = instances[0].quad_origin[1] + height;
        assert!(
            bottom <= cell_height + 0.5,
            "underline bottom {bottom} must stay inside row 0 ({cell_height})"
        );
    }
    /// 非空单元格竖线光标：字形保持原文色并追加一枚标记。
    #[test]
    fn bar_cursor_on_glyph_cell_emits_glyph_plus_marker() {
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: true,
            style: crate::terminal::ghostty_terminal::CursorStyle::Bar,
            color: Some([1.0, 1.0, 1.0, 1.0]),
        };
        let cells = vec![cell_data(
            0,
            0,
            'A',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            0,
        )];
        let instances = build(&cells, cursor, &[]);
        assert_eq!(
            instances.len(),
            2,
            "bar cursor on a glyph emits the glyph plus one marker"
        );
        assert_eq!(
            instances[0].foreground,
            [1.0, 0.0, 0.0, 1.0],
            "glyph keeps its own foreground under a bar cursor"
        );
        assert_eq!(
            instances[0].background,
            [0.0, 0.0, 0.0, 1.0],
            "glyph keeps its own background under a bar cursor"
        );
        assert_eq!(
            instances[1].atlas_size,
            [0.0, 0.0],
            "marker is a solid color quad without glyph content"
        );
        assert!(
            instances[1].quad_size[0] <= instances[0].quad_size[0] * 0.5,
            "marker must stay thin relative to the glyph quad"
        );
    }
}
