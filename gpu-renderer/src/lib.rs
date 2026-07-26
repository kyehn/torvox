//! GPU renderer (wgpu / Vulkan).
//!
//! The only rendering path — there is no CPU/Canvas fallback. [`font`]
//! performs text shaping (cosmic-text), glyph rasterization (swash), and atlas
//! packing (guillotiere); [`gpu`] owns the wgpu pipeline, atlas texture, and
//! per-glyph instance submission. Depends on `terminal-core` and `terminal-engine`.
//!
//! The atlas alpha-coverage texture uses `Rgba8Unorm` (R channel = coverage,
//! GBA = 0), a **linear** (non-sRGB) format; glyph coverage data is already in
//! linear space, so the GPU applies no gamma correction on sampling.

pub mod font;
pub mod gpu;
mod lock_util;
pub mod renderdoc_capture;
