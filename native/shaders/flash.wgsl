//! Bell-flash overlay shader — full-screen white quad with a decaying alpha.
//!
//! # Requirements
//! - FR-010 — Vulkan-only: this is a wgpu/WGSL path (no GL/CPU fallback).
//!
//! The alpha is computed on the CPU side from the phase provided by Kotlin
//! (see `BELL_FLASH_ALPHA_255`) and passed via a single uniform float.
//! `phase = 0` makes the pass a no-op (uniform alpha 0).

struct Uniforms {
    alpha: f32,
    _padding: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // White flash; alpha blending (SrcAlpha / OneMinusSrcAlpha) is
    // configured on the pipeline so `uniforms.alpha` controls opacity.
    return vec4<f32>(1.0, 1.0, 1.0, uniforms.alpha);
}
