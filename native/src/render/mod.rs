//! GPU render pipeline — wgpu instance management, atlas, glyph rendering.
//!
//! The only rendering path — there is no CPU/Canvas fallback. The [`font`]
//! sub-module performs text shaping (cosmic-text), glyph rasterization (swash),
//! and atlas packing (guillotiere); [`context`] owns the `Renderer` struct;
//! `pipeline` builds wgpu pipelines; `pass` drives per-frame rendering;
//! `surface` manages Android surface lifecycle.
//!
//! The atlas alpha-coverage texture uses `Rgba8Unorm` (R channel = coverage,
//! GBA = 0), a **linear** (non-sRGB) format; glyph coverage data is already in
//! linear space, so the GPU applies no gamma correction on sampling.

//! # Requirements
//! - FR-050 — Android surface lifecycle (attach/detach) recreates the wgpu surface and pipeline

// ── Sub-modules ──────────────────────────────────────────────────────────
pub mod cpu_frame;
pub mod font;
pub mod invalidation;
#[cfg(not(target_os = "android"))]
pub mod renderdoc_capture;

pub(crate) mod cell_builder;
pub mod context;
mod pass;
mod pipeline;
pub(crate) mod snapshot_reference;
// Off-screen render-verification path (research-wgpu-example §6.1/§6.2):
// procedural geometry + depth-attached LOD grid are crate-test-only — the
// production `Renderer` keeps zero depth attachments (2D terminal rendering
// needs none), so this module must not ship in the normal build or leak into
// `integration-tests` (which enables `test-util`).
#[cfg(test)]
pub(crate) mod procedural_geometry;
pub(crate) mod wgpu_backend;

#[cfg(test)]
mod tests;

// ── Re-exports ───────────────────────────────────────────────────────────
pub use cell_builder::{CellCursor, build_instances_from_cell_data};
#[cfg(any(test, feature = "test-util"))]
#[allow(unused_imports)]
pub(crate) use cell_builder::{SearchHighlight, SelectionRange, blend_highlight, cell_highlight};
pub use context::FrameContext;
pub use context::Renderer;
pub use context::orthographic_projection;
pub use cpu_frame::{CpuCell, CpuCursor, CpuFrame, TextHit, TextItem};
pub use invalidation::{FrameInvalidation, InvalidationLevel};
#[cfg(any(test, feature = "test-util"))]
#[allow(unused_imports)]
pub(crate) use pipeline::{DEFAULT_BG_ALPHA, QUAD_CORNERS};
pub use pipeline::{GpuUniforms, image_active_value};
#[cfg(any(test, feature = "test-util"))]
#[allow(unused_imports)]
pub(crate) use snapshot_reference::{
    FlatGrid, SnapshotConfig, build_cell_instances_from_flat, build_cell_instances_from_snapshot,
    build_cell_instances_into, color_f32x4_eq,
};

/// Serialises GPU/CPU benchmarks: under software Vulkan (Mesa Lavapipe)
/// each test creates its own wgpu device, and parallel benchmarks contend
/// for CPU so hard throughput thresholds become flaky. The lock is held
/// for the whole benchmark body, guaranteeing one benchmark at a time
/// scroll bench joined the lock for the same reason).
#[cfg(any(test, feature = "test-util"))]
#[cfg_attr(not(test), allow(dead_code))]
// Only referenced from #[cfg(test)] benches; the lib build with
// `--features test-util` (clippy) has no callers.
pub(crate) static GPU_BENCH_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

// ── Public Constants ─────────────────────────────────────────────────────
pub const RENDER_SCALE: f32 = 1.0;

pub const CATPPUCCIN_MOCHA_BG: wgpu::Color = wgpu::Color {
    r: 30.0 / 255.0,
    g: 30.0 / 255.0,
    b: 46.0 / 255.0,
    a: 1.0,
};

// ── Error Type ───────────────────────────────────────────────────────────
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("wgpu request adapter failed")]
    NoAdapter,
    #[error("wgpu request device failed: {0}")]
    DeviceRequest(String),
    #[error("surface creation failed: {0}")]
    Surface(String),
    #[error("shader compilation failed: {0}")]
    Shader(String),
    #[error("buffer creation failed: {0}")]
    Buffer(String),
    #[error("buffer readback failed: {0}")]
    Readback(String),
}

// ── GPU Instance Types ───────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    pub quad_origin: [f32; 2],
    pub atlas_offset: [f32; 2],
    pub atlas_size: [f32; 2],
    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
    pub quad_size: [f32; 2],
    pub flags: f32,
    pub bearing: [f32; 2],
    pub glyph_advance_width: f32,
}

impl CellInstance {
    pub const ATTRIBS: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x4,
        5 => Float32x4,
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

// ── Test-util surface for integration-tests ──────────────────────────────
// integration-tests enables `test-util` and reaches the render internals
// (reference snapshot path + instance types) through this single module.
pub mod gpu {
    pub use super::cell_builder::{CellCursor, CellInstanceConfig, build_instances_from_cell_data};
    pub use super::cell_builder::{SearchHighlight, SelectionRange};
    pub use super::context::{Renderer, orthographic_projection};
    pub use super::pipeline::{GpuUniforms, image_active_value};
    #[cfg(any(test, feature = "test-util"))]
    pub use super::snapshot_reference::{
        FlatGrid, SnapshotConfig, build_cell_instances_from_flat,
        build_cell_instances_from_snapshot, build_cell_instances_into,
    };
    pub use super::{
        CATPPUCCIN_MOCHA_BG, CellInstance, GpuError, KittyGraphicsInstance, RENDER_SCALE,
    };
}
