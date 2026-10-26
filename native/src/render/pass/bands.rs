//! 脏带：部分渲染的判定、清除实例与上传布局。
use crate::render::Renderer;
use crate::render::context::MIN_VERTEX_BUFFER_SIZE;

/// 本帧可用的部分渲染脏带；`None` 即强制全量重绘（判定表见 `pass::tests`）。
///
/// 脏带掩码必须与本帧的构建掩码同源：本帧走了全量重建（换字体、缩放、缓存失效）
/// 时每个单元的图集 UV 与几何都换了，而 `should_render_partial` 只看脏带数量，
/// 于是 partial 路径只重绘脏带、其余行沿用累加器里的旧像素——换字体后整屏仍是
/// 旧字，直到下一次输出把各行碰巧重新变脏。
///
/// 另两项同前：缓存尺寸不符（缩放）或图集代际变化（UV 已搬迁）时稀疏带会让
/// 干净行残留陈旧 UV，同样强制全量。
pub(super) fn partial_frame_bands(
    rebuilt_all_rows: bool,
    cache: Option<&crate::render::cell_builder::CachedInstances>,
    rows: u32,
    cols: u32,
    atlas_generation: u64,
    dirty_rows: Option<&[bool]>,
) -> Option<Vec<crate::render::cell_builder::DirtyBand>> {
    if rebuilt_all_rows {
        return None;
    }
    let cache = cache?;
    // 掩码必须与行数对齐：`compute_dirty_bands` 产出的行号会直达
    // `cache.band_slice` → `row_ends[row]`，而 `row_ends` 的长度只由
    // `is_compatible(rows, cols)` 保证与 `rows` 相等，**不**由掩码长度保证。
    // 与 `build_row_instances_into` 的判定同口径（cell_builder 里那处
    // `dirty_rows.is_some_and(|dirty_flags| dirty_flags.len() >= rows as usize)`）：超长只取前
    // `rows` 项（越界的位本就无所谓），过短则放弃分帧、整屏重画。
    // 少了这一步，一旦有调用方传入更长的掩码，release 构建就会在
    // `row_ends[row]` 上越界 panic，把渲染线程整个带走。
    let mask = dirty_rows?.get(..rows as usize)?;
    if !cache.is_compatible(rows, cols) {
        return None;
    }
    if cache.built_atlas_generation() != atlas_generation {
        return None;
    }
    Some(
        crate::render::cell_builder::compute_dirty_bands(mask)
            .into_iter()
            .map(|(start_row, end_row_exclusive)| {
                let (instance_start, instance_end) = cache.band_slice(start_row, end_row_exclusive);
                crate::render::cell_builder::DirtyBand {
                    start_row,
                    end_row_exclusive,
                    instance_start,
                    instance_end,
                }
            })
            .collect::<Vec<_>>(),
    )
}

/// 清除实例与真实单元实例在同一顶点缓冲中的字节边界 `(clear_end, draw_end)`：
/// 清除段占 `[0, clear_end)`，真实例占 `[clear_end, draw_end)`。
///
/// 纯函数，判定表见 `pass::tests`。容量必须按 `draw_end` 一次算足：先写清除段、
/// 再为真实例扩容重建缓冲，会把刚落盘的清除段丢掉。
pub(super) fn instance_upload_layout(clear_count: usize, draw_count: usize) -> (u64, u64) {
    let stride = std::mem::size_of::<crate::render::CellInstance>() as u64;
    let clear_end = clear_count as u64 * stride;
    (clear_end, clear_end + draw_count as u64 * stride)
}

/// 顶点缓冲容量不足 `required_size` 时按需重建，否则原样保留。
pub(super) fn ensure_instance_capacity(
    device: &wgpu::Device,
    instance_buffer: &mut Option<wgpu::Buffer>,
    required_size: u64,
    label: &str,
) {
    if required_size == 0 {
        return;
    }
    if instance_buffer
        .as_ref()
        .is_some_and(|buffer| buffer.size() >= required_size)
    {
        return;
    }
    *instance_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: required_size.max(MIN_VERTEX_BUFFER_SIZE),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }));
}

impl Renderer {
    /// 脏带部分渲染的判定（纯函数，表驱动单测）。仅当本帧唯一变化是被标记
    /// 脏带的单元实例时成立；累加器失效、或 kitty 叠加层本帧发生变化
    /// （新增/替换/删除图像）都强制全量重绘。
    ///
    /// kitty 图像**存在**本身不再强制全量：图像层是独立的图集，每帧整层重画在
    /// 累加器之上，文本层的脏带互不影响。此前把「有图像」当作必须全量的条件，
    /// 等于让一张静止的图片把整个终端钉在全量帧的代价上（SwiftShader 实测
    /// 全量帧 ~500ms，终端直接掉到 1-2fps，yazi 这类全屏 TUI 更是几乎不可用）。
    pub(super) fn should_render_partial(
        accumulator_ready: bool,
        frame_invalidated: bool,
        band_count: usize,
        kgp_overlay_dirty: bool,
    ) -> bool {
        accumulator_ready && !frame_invalidated && band_count > 0 && !kgp_overlay_dirty
    }

    /// 滚动一致性门控：视口像素偏移非零，或自上一呈现帧发生变化时为 true。
    /// 纹理拷贝无法亚像素平移累加器，这两种情况下脏带部分路径会把旧像素
    /// 与平移后的新几何混叠（滚动残留/撕裂），必须强制全量重绘。
    pub(super) fn is_scroll_offset_active(
        viewport_scroll_px: f32,
        last_drawn_viewport_scroll_px: f32,
    ) -> bool {
        // 仅当本帧亚行余量**发生变化**时才强制全量：滚动进行中余量每帧都在变，
        // 必须全量避免累加器旧像素与新几何混叠；余量稳定（含手指停住、空闲重绘）
        // 时部分渲染是安全的——内容未动，脏带为空，累加器保持自洽。
        (viewport_scroll_px - last_drawn_viewport_scroll_px).abs() > f32::EPSILON
    }
    /// 部分/全量合成门控：纯函数——脏带部分路径候选与滚动一致性门控的合成。
    /// 滚动激活时一律全量（判定见 is_scroll_offset_active），偏移归零且稳定后恢复部分渲染。
    pub(super) fn should_render_partial_frame(
        accumulator_ready: bool,
        frame_invalidated: bool,
        band_count: usize,
        kgp_overlay_dirty: bool,
        scroll_active: bool,
    ) -> bool {
        Self::should_render_partial(
            accumulator_ready,
            frame_invalidated,
            band_count,
            kgp_overlay_dirty,
        ) && !scroll_active
    }

    /// 脏带是否触及 kitty 图像所在的行。
    ///
    /// 图像四边形按像素定位，脏带按整行重画：只要两者在行上相交，该处的
    /// 图像像素就被清掉（脏带先以背景色清除，再画单元），必须重画图像。
    /// 单元格高度未知（几何尚未就绪）时保守返回 true——宁可多画一次，
    /// 也不留下被擦掉一半的图像。
    pub(super) fn kgp_touched_by_band(
        kgp_instances: &[crate::render::KittyGraphicsInstance],
        band: &crate::render::cell_builder::DirtyBand,
        cell_height_px: f32,
    ) -> bool {
        if cell_height_px <= 0.0 {
            return true;
        }
        kgp_instances.iter().any(|instance| {
            let first_row = (instance.quad_origin[1] / cell_height_px).floor();
            let last_row =
                ((instance.quad_origin[1] + instance.quad_size[1]) / cell_height_px).ceil();
            (band.start_row as f32) < last_row && (band.end_row_exclusive as f32) > first_row
        })
    }

    /// 脏带对应的像素行范围：与单元四边形完全相同的光栅化规则——只覆盖
    /// **像素中心**落在 `[start_row, end_row_exclusive) * cell_height_px` 内的行。
    ///
    /// 向外取整（`floor(start)` / `ceil(end)`）会多覆盖相邻行的边界像素，
    /// 而相邻行在部分帧里通常不在脏带内、本帧不重绘，于是那一像素保留清除色
    /// （主题背景），在选区/搜索高亮这类连续整行着色里表现为行与行之间的
    /// 1px 缝隙（用户报告的「文本选择 不同行间存在空隙」，模拟器实测：
    /// cell_height=44.625 时缝隙出现在 44.625 的分数部分 > 0.5 的行边界）。
    pub(super) fn band_clear_pixel_range(
        start_row: usize,
        end_row_exclusive: usize,
        cell_height_px: f32,
        config_height: u32,
    ) -> (u32, u32) {
        let y0 = (start_row as f32 * cell_height_px - 0.5).ceil().max(0.0) as u32;
        let y1 = ((end_row_exclusive as f32 * cell_height_px - 0.5)
            .ceil()
            .max(0.0) as u32)
            .min(config_height);
        (y0, y1)
    }

    /// 每个脏带构造一个纯色实例：has_glyph=0 的四边形，单元着色器直接以
    /// `background` 原样填充，覆盖该带的像素矩形（全部列 × 该带的行跨度，
    /// 边界见 [`Self::band_clear_pixel_range`]）。
    pub(super) fn band_clear_instances(
        &self,
        dirty_bands: &[crate::render::cell_builder::DirtyBand],
        cell_height_px: f32,
        config_width: u32,
        config_height: u32,
    ) -> Vec<crate::render::CellInstance> {
        if cell_height_px <= 0.0 || config_width == 0 || config_height == 0 {
            return Vec::new();
        }
        let background = [
            self.background.r as f32,
            self.background.g as f32,
            self.background.b as f32,
            self.background.a as f32,
        ];
        let mut instances = Vec::with_capacity(dirty_bands.len());
        for band in dirty_bands {
            let (y0, y1) = Self::band_clear_pixel_range(
                band.start_row,
                band.end_row_exclusive,
                cell_height_px,
                config_height,
            );
            if y1 > y0 {
                instances.push(crate::render::CellInstance {
                    quad_origin: [0.0, y0 as f32],
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    foreground: background,
                    background,
                    underline_color: background,
                    quad_size: [config_width as f32, (y1 - y0) as f32],
                    flags: 0.0,
                    bearing: [0.0; 2],
                });
            }
        }
        instances
    }
}
