use super::Renderer;

#[cfg(any(target_os = "android", test))]
use crate::render::GpuError;

// ── 表面配置、格式与投影 ────────────────────────────────────────────
/// 为给定视口尺寸创建正交投影矩阵。
pub fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

/// 按视口 Y 像素偏移平移正交投影（正值 = 内容下移）。屏幕 Y 向下增长而 NDC Y
/// 向上增长，故偏移从平移行中减去；高度或偏移为 0 时矩阵不变。
pub fn apply_scroll_px_offset(
    mut proj: [[f32; 4]; 4],
    scroll_px: f32,
    height: f32,
) -> [[f32; 4]; 4] {
    if height > 0.0 && scroll_px != 0.0 {
        proj[3][1] -= scroll_px * 2.0 / height;
    }
    proj
}

/// 顶点缓冲的下限字节数。wgpu 拒绝 0 字节的缓冲，而调用方的空列表提前返回
/// 并不构成该下限的来源——留一个最小值让契约与 wgpu 的要求对齐。
pub const MIN_VERTEX_BUFFER_SIZE: u64 = 64;

/// 交换链尺寸的唯一计算点：按跨渲染器固定的 `RENDER_SCALE` 缩放，再钳制到
/// `[1, max_texture_dimension_2d]`。
///
/// `attach_surface` 与 `reconfigure_swapchain` 两条挂载路径 MUST 共用本函数。
/// 超限的 surface 交给 `configure` 时，wgpu 30 的 `configure` 无返回值，失败只经
/// `get_current_texture` 的 Lost 状态异步上报——表现为永久黑屏且没有直接错误；
/// 缩放与钳制各写一份则补了钳制的那条路径会给另一条留下这个缺口。
/// 与 `select_surface_format` 同理：唯一调用方仅 Android 存在，故限定为
/// Android 生产构建 + 测试，而非加 `allow(dead_code)`。
#[cfg(any(target_os = "android", test))]
pub(crate) fn surface_extent(width: u32, height: u32, max_texture_dimension_2d: u32) -> (u32, u32) {
    let limit = max_texture_dimension_2d.max(1);
    let scaled = |dim: u32| ((dim as f32 * crate::render::RENDER_SCALE) as u32).clamp(1, limit);
    (scaled(width), scaled(height))
}

impl Renderer {
    /// 从适配器报告的格式里**指名**挑一个 8 位 RGBA 族的交换链格式。
    ///
    /// 此前是「取第一个非 sRGB 格式」——那等于让设备决定我们会去申请哪种窗口缓冲。
    /// 真机（Mali GPU）在 `attachWindow` 之后立刻报告 R8 格式不被分配器支持。0x38 正是
    /// `AHARDWAREBUFFER_FORMAT_R8_UNORM`——即驱动把单通道格式排在了候选首位，我们照单
    /// 申请，于是缓冲区队列分配失败。这不只是日志噪音：渲染进 R8 目标也只有灰度一个
    /// 通道，即便侥幸分配成功，颜色也是错的。
    ///
    /// 取舍顺序：Rgba8Unorm（与 SurfaceFlinger 默认 RGBA_8888 原生对应，无需 sRGB 视图）
    /// → 其它非 sRGB 的 RGBA/BGRA 8 位 → 任意非 sRGB → 任意格式。每一档退化都记一条
    /// 日志，避免「用了个奇怪格式」这件事再次无声发生。
    /// 与 `select_present_mode` 同理：唯一调用方 `attach_surface` 仅 Android 存在，
    /// 故限定为 Android 生产构建 + 测试，而非加 `allow(dead_code)`。
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn select_surface_format(
        caps: &wgpu::SurfaceCapabilities,
    ) -> Result<wgpu::TextureFormat, GpuError> {
        let rgba_family = |candidate: &wgpu::TextureFormat| {
            matches!(
                candidate,
                wgpu::TextureFormat::Rgba8Unorm
                    | wgpu::TextureFormat::Bgra8Unorm
                    | wgpu::TextureFormat::Rgba8UnormSrgb
                    | wgpu::TextureFormat::Bgra8UnormSrgb
            )
        };
        if let Some(format) = caps
            .formats
            .iter()
            .copied()
            .find(|candidate| *candidate == wgpu::TextureFormat::Rgba8Unorm)
        {
            return Ok(format);
        }
        if let Some(format) = caps
            .formats
            .iter()
            .copied()
            .find(|candidate| rgba_family(candidate) && !candidate.is_srgb())
        {
            log::warn!("attach_surface: 设备未提供 Rgba8Unorm，退到交换链格式 {format:?}");
            return Ok(format);
        }
        // 宁可要 sRGB 的 RGBA8，也不要非 RGBA 族：单通道目标渲染出来只有灰度，
        // gamma 不对只是偏亮/偏暗，颜色正确性远比 gamma 重要。
        if let Some(format) = caps
            .formats
            .iter()
            .copied()
            .find(|candidate| rgba_family(candidate))
        {
            log::warn!(
                "attach_surface: 只有 sRGB 的 RGBA8 可用，交换链退到 {format:?}（gamma 会偏）"
            );
            return Ok(format);
        }
        if let Some(format) = caps
            .formats
            .iter()
            .copied()
            .find(|candidate| !candidate.is_srgb())
        {
            log::error!(
                "attach_surface: 适配器只提供非 RGBA8 族格式，交换链退到 {format:?}；\
                 若该格式的窗口缓冲不可分配（Mali 的 R8_UNORM 即是），画面将无法呈现"
            );
            return Ok(format);
        }
        caps.formats
            .first()
            .copied()
            .ok_or_else(|| GpuError::Surface("attach_surface: no supported surface formats".into()))
    }

    /// 宿主构建排除了仅 Android 的 `attach_surface` 调用方，本函数在宿主上会成为死代码，
    /// 故限定为 Android 生产构建 + 测试，而非加 `allow(dead_code)`。
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn select_present_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::PresentMode {
        // 渲染稳定性 spec §3「滚动不得有可见撕裂」：Immediate 无 vsync 直通
        // 扫描线，滚动时必然撕裂，且让渲染线程无界冲刷（真机实测 166fps，
        // 多数帧被显示端丢弃）。优先 Mailbox（vsync 节拍 + 新帧覆盖旧帧、
        // 无背压——滚动终端里「最新帧永远胜出」正是所需取舍，见下方
        // attach_surface 对 desired_maximum_frame_latency=2 的注释），其次
        // Fifo（标准 vsync），再 AutoVsync；Immediate 仅作驱动只支持它时的
        // 最后兜底。
        if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else if caps.present_modes.contains(&wgpu::PresentMode::AutoVsync) {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::Immediate
        }
    }

    #[cfg(target_os = "android")]
    pub fn reconfigure_swapchain(&mut self, width: u32, height: u32) {
        let (surface, config) = match (self.surface.as_ref(), self.surface_config.as_mut()) {
            (Some(surface), Some(config)) => (surface, config),
            _ => return,
        };
        let (scaled_width, scaled_height) =
            surface_extent(width, height, self.device.limits().max_texture_dimension_2d);
        if config.width == scaled_width && config.height == scaled_height {
            return;
        }
        config.width = scaled_width;
        config.height = scaled_height;
        surface.configure(&self.device, config);

        // 累加器内容属于旧尺寸，reconfigure 后强制全量重绘（同 attach 慢路径）。
        // 否则空闲 shell 会永远持有重建后为空的累加器——空闲门控只在
        // `frame_invalidated` 时重绘，导致输入法 resize / 切后台且 surface 保留时黑屏或闪烁。
        self.frame_texture = None;
        self.frame_view = None;
        self.frame_invalidated = true;

        self.projection_width = scaled_width;
        self.projection_height = scaled_height;

        if let Some(buf) = &self.cell_uniform_buffer {
            let atlas_width = self
                .atlas_texture
                .as_ref()
                .map_or(0, |atlas_texture| atlas_texture.width());
            let atlas_height = self
                .atlas_texture
                .as_ref()
                .map_or(0, |atlas_texture| atlas_texture.height());
            let uniforms = self.cell_uniforms(
                scaled_width as f32,
                scaled_height as f32,
                atlas_width as f32,
                atlas_height as f32,
            );
            self.queue
                .write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
        }

        log::info!(
            "RECONFIGURE_SWAPCHAIN: {}x{} (projection updated)",
            width,
            height
        );
    }
}
