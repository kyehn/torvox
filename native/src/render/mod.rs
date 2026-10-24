//! GPU 渲染管线：wgpu 实例管理、图集与字形渲染，无 CPU/Canvas 回退路径。
//! [`font`] 负责整形与光栅化，`pipeline` 构建管线，`pass` 驱动逐帧渲染。
//!
//! 图集的覆盖率纹理用 `Rgba8Unorm`，且覆盖率被写进**全部四个通道**
//! （`font/atlas` 的 Mask 分支写 R=G=B=A=alpha；有色字形分支写
//! R=G=B=alpha、A=255）。形式上仍是 `Rgba8Unorm`，语义上只有 `.r` 被消费——
//! 着色器按 `texel.r` 解析覆盖率，其余通道不参与任何判定。
//! 且它是**线性**非 sRGB 格式：覆盖率数据本就在线性空间，GPU 采样时不做 gamma 校正。
//! （此前此处写「R 通道为覆盖率，GBA 为 0」，与实际写入不符——照它改图集会静默
//! 改变每个抗锯齿字形边缘的颜色。）

// ── 子模块 ──────────────────────────────────────────────────────────────
pub mod font;
pub mod kitty;

pub mod cell_builder;
pub mod context;
mod pass;
mod pipeline;
// 离屏渲染验证路径：程序化几何与带深度附件的
// LOD 网格仅供 crate 内测试，2D 终端渲染不需要深度附件，故不进正常构建，
// 也不泄漏到启用了 `test-util` 的原生集成测试。
pub(crate) mod wgpu_backend;

#[cfg(test)]
mod tests;

// ── 再导出 ──────────────────────────────────────────────────────────────
pub use cell_builder::{CellCursor, build_instances_from_cell_data};
#[cfg(test)]
pub(crate) use cell_builder::{SearchHighlight, blend_highlight, cell_highlight};
pub use context::Renderer;
pub use context::apply_scroll_px_offset;
pub use context::orthographic_projection;
pub use pipeline::GpuUniforms;
#[cfg(test)]
pub(crate) use pipeline::QUAD_CORNERS;

/// 串行化 GPU 基准：软件 Vulkan（Mesa Lavapipe）下每个测试各自建设备，
/// 并行争抢 CPU 会使吞吐阈值抖动，故整段基准持有锁，保证一次只跑一个。
#[cfg(test)]
pub(crate) static GPU_BENCH_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

// ── 公开常量 ────────────────────────────────────────────────────────────
pub const RENDER_SCALE: f32 = 1.0;

// ── 错误类型 ────────────────────────────────────────────────────────────
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("wgpu request adapter failed")]
    NoAdapter,
    #[error("wgpu request device failed: {0}")]
    DeviceRequest(String),
    #[error("surface creation failed: {0}")]
    Surface(String),
    /// 本帧没有可呈现的纹理：渲染暂停、surface 尚未挂载、平台收走了原生窗口
    /// （BufferQueue 被遗弃），或交换链尺寸需要重配。
    ///
    /// 属**可恢复**路径：宿主换新窗口重挂载后即恢复（见 `render-stability`
    /// 规范「渲染 surface 失效后自愈」），故日志级别低于 error——
    /// 详见 [GpuError::is_recoverable]。
    #[error("surface unavailable: {0}")]
    SurfaceUnavailable(String),
    /// 仅测试回读脚手架（`render_to_buffer`）构造，生产路径不产生。
    #[cfg(test)]
    #[error("buffer readback failed: {0}")]
    Readback(String),
}

impl GpuError {
    /// 该错误是否属于「本帧没画出来、但下一帧或换窗口后即可恢复」的一类。
    ///
    /// 判据是**类型**而非文案：只有 [GpuError::SurfaceUnavailable] 是可恢复的，
    /// 其余（管线缺失、单元转换失败、attach 失败……）都是真异常。
    ///
    /// 用途：日志分级。可恢复的一类按 debug 记（逐帧可能重复，不能淹掉
    /// W/E 闸门），真异常仍按 error 记。宿主侧另有「瞬时渲染错误」的限频汇总，
    /// 两者分工：本处给细节，宿主给频率。
    pub fn is_recoverable(&self) -> bool {
        matches!(self, GpuError::SurfaceUnavailable(_))
    }
}

#[cfg(test)]
mod gpu_error_recoverability {
    use super::GpuError;

    /// 本帧没有可呈现的纹理属可恢复一类：宿主换新窗口重挂载后即恢复，
    /// 不得按 error 记（否则每次切后台都刷一条 E 级，淹没真缺陷）。
    #[test]
    fn surface_unavailable_is_recoverable() {
        assert!(GpuError::SurfaceUnavailable("begin_frame failed".into()).is_recoverable());
    }

    /// 其余错误都是真异常：管线缺失、单元转换失败、attach 失败、适配器缺失。
    #[test]
    fn other_errors_are_not_recoverable() {
        assert!(!GpuError::Surface("No render pipeline".into()).is_recoverable());
        assert!(!GpuError::NoAdapter.is_recoverable());
        assert!(!GpuError::DeviceRequest("no device".into()).is_recoverable());
    }
}

// ── GPU 实例类型 ────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    pub quad_origin: [f32; 2],
    pub atlas_offset: [f32; 2],
    pub atlas_size: [f32; 2],
    pub foreground: [f32; 4],
    pub background: [f32; 4],
    /// SGR 58 下划线/装饰色，上游回退到前景色。
    pub underline_color: [f32; 4],
    pub quad_size: [f32; 2],
    pub flags: f32,
    pub bearing: [f32; 2],
}

impl CellInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x4,
        5 => Float32x4,
        10 => Float32x4,
        6 => Float32x2,
        7 => Float32,
        8 => Float32x2,
    ];

    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct KittyGraphicsInstance {
    pub quad_origin: [f32; 2],
    pub quad_size: [f32; 2],
    pub atlas_offset: [f32; 2],
    pub atlas_region: [f32; 2],
    pub alpha: f32,
    pub _padding: f32,
}

impl KittyGraphicsInstance {
    pub fn new(
        quad_origin: [f32; 2],
        quad_size: [f32; 2],
        atlas_offset: [f32; 2],
        atlas_region: [f32; 2],
        alpha: f32,
    ) -> Self {
        Self {
            quad_origin,
            quad_size,
            atlas_offset,
            atlas_region,
            alpha,
            _padding: 0.0,
        }
    }

    pub const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x2,
        5 => Float32,
    ];

    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<KittyGraphicsInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}
