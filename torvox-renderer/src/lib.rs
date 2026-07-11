//! GPU renderer for torvox (wgpu / Vulkan).
//!
//! The only rendering path — there is no CPU/Canvas fallback. [`font`]
//! performs text shaping (cosmic-text), glyph rasterization (swash), and atlas
//! packing (guillotiere); [`gpu`] owns the wgpu pipeline, atlas texture, and
//! per-glyph instance submission. Depends on `torvox-core` and `torvox-terminal`.
//!
//! The atlas alpha-coverage texture uses `R8Unorm`, a **linear** (non-sRGB)
//! format; glyph coverage data is already in linear space, so the GPU applies
//! no gamma correction on sampling.

pub mod font;
pub mod gpu;
