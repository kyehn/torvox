//! GPU 渲染管线：wgpu 实例管理、图集与字形渲染，无 CPU/Canvas 回退路径。
//! [`font`] 负责整形与光栅化，`pipeline` 构建管线，`pass` 驱动逐帧渲染。
//!
//! 图集的 alpha 覆盖纹理用 `Rgba8Unorm`（R 通道为覆盖率，GBA 为 0）且是
//! **线性**非 sRGB 格式：覆盖率数据本就在线性空间，GPU 采样时不做 gamma 校正。

// ── 子模块 ──────────────────────────────────────────────────────────────
pub mod font;
pub mod kitty;

pub(crate) mod cell_builder;
pub mod context;
mod pass;
mod pipeline;
// 离屏渲染验证路径（见 docs/specification/REFERENCE.md）：程序化几何与带深度附件的
// LOD 网格仅供 crate 内测试，2D 终端渲染不需要深度附件，故不进正常构建，
// 也不泄漏到启用了 `test-util` 的原生集成测试。
pub(crate) mod wgpu_backend;

#[cfg(test)]
mod tests;

// ── 再导出 ──────────────────────────────────────────────────────────────
pub use cell_builder::{CellCursor, build_instances_from_cell_data};
#[cfg(test)]
pub(crate) use cell_builder::{SearchHighlight, blend_highlight, cell_highlight};
pub use context::FrameContext;
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

pub const CATPPUCCIN_MOCHA_BACKGROUND: wgpu::Color = wgpu::Color {
    r: 30.0 / 255.0,
    g: 30.0 / 255.0,
    b: 46.0 / 255.0,
    a: 1.0,
};

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
    #[error("buffer readback failed: {0}")]
    Readback(String),
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
    pub glyph_advance_width: f32,
}

impl CellInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x4,
        5 => Float32x4,
        10 => Float32x4,
        6 => Float32x2,
        7 => Float32,
        8 => Float32x2,
        9 => Float32,
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

// ── 供基准与渲染测试使用的再导出面 ─────────────────────────────────────────
pub mod gpu {
    pub use super::cell_builder::{
        CellCursor, CellInstanceConfig, SearchHighlight, build_instances_from_cell_data,
    };
    pub use super::context::{Renderer, orthographic_projection};
    pub use super::pipeline::GpuUniforms;
    pub use super::{
        CATPPUCCIN_MOCHA_BACKGROUND, CellInstance, GpuError, KittyGraphicsInstance, RENDER_SCALE,
    };
}
