# 0008 — Rendering Pipeline

- **Status**: Accepted
- **Date**: 2026-07-27
- **Requirement IDs**: FR-02, NFR-01

## Context

The original `gpu-renderer/` crate (~9.6 KLOC) used:
- **wgpu** for Vulkan GPU access
- **cosmic-text** for Unicode text shaping (ligatures, CJK fallback, complex
  scripts)
- **swash** for glyph rasterization (converting font outlines to pixels)
- **guillotiere** for atlas packing (efficiently packing glyphs into a
  shared texture)
- **fontdb** for font discovery
- **lru** for shape cache

This pipeline was embedded in a separate crate with its own module hierarchy
(font/, gpu/, cell_builder, etc.) and significant abstraction overhead.

The adversarial review confirmed the pipeline is correct for the target use
case — wgpu gives unique control (shaders, zero-copy textures, Kitty
Graphics) not available via Android Canvas. The key insight was: the
complexity comes from the abstraction layers, not the tools themselves.

warp-mobile-android uses the same tools (cosmic-text + Vulkan via `ash`)
and achieves stable 60 fps on Adreno 750 with a 7.4 MB APK.

This ADR depends on ADR-0002: the render pipeline's input changes from
torvox's `GridSnapshot` to Ghostty's `CellIterator`, which drives the
simplification of cell_builder and related modules.

## Decision

Keep wgpu, cosmic-text, swash, guillotiere, and fontdb — but **flatten
the module hierarchy** and **remove abstraction layers** that add no value:

### What stays (with simplification)

| Component | Use | Lines | Simplification |
|-----------|-----|-------|---------------|
| wgpu | Vulkan surface, device, queue, swapchain | ~1,200 | Remove `GpuContext` — inline into `Renderer` |
| cosmic-text | Unicode shaping for non-ASCII text | ~400 | Keep — essential for ligatures, CJK |
| swash | Glyph rasterization | ~250 | Keep — only way to get glyph pixels |
| guillotiere | Atlas texture packing | ~100 | Keep — risk avoidance per design review |
| fontdb | Font discovery | ~100 | Fold into `FontSystem` |
| WGSL shaders | Cell rendering + post-processing | ~200 | 3 active (was 5) |
| Glue + types | Structs, traits, JNI wire-up | ~200 | |
| **Total** | | **~2,450** | |

### What is removed

| Removed | KLOC | Reason |
|---------|------|--------|
| `font/pipeline.rs` | ~600 | Abstracted glyph cache — inline directly into `Renderer` |
| `font/rasterization.rs` | ~126 | Swash calls inlined into font module |
| `font/cjk.rs` | ~266 | Folded into font discovery (cosmic-text handles CJK) |
| `gpu/cell_builder.rs` | ~762 | Cell data now comes from Ghostty `CellIterator` — not from `GridSnapshot` |
| `gpu/pipeline.rs` → half | ~723 | Split into focused render functions, not pipeline abstraction |
| WGSL shaders | 5→3 active | Merge text+background+selection+kgp into focused shaders |

### Shader count

Reduce from 5 WGSL shaders to 3 active:

1. **`cell.wgsl`** — Vertex+Fragment for terminal cells: textured quad
   (glyph atlas) with foreground/background color and selection highlight
2. **`post.wgsl`** — Optional post-processing (bloom, color correction,
   CRT effect) — only if needed for features

## Alternatives Considered

### Replace wgpu with raw Vulkan via `ash`
- **Rejected**: wgpu provides surface creation, swapchain management, and
  safety wrappers over Vulkan that are valuable. warp-mobile-android uses
  `ash` and has substantially more Vulkan boilerplate than wgpu.

### Replace wgpu with Android Canvas + SurfaceView
- **Rejected**: Canvas rendering goes through Android HWUI which has
  unpredictable latency (p95 spikes >50 ms for complex text layouts).
  GPU control via wgpu provides consistent frame times and enables future
  features (Kitty Graphics protocol, GPU compositing).

### Replace cosmic-text with simple ab_glyph
- **Rejected**: ab_glyph does not support ligatures, CJK fallback, or
  complex script shaping. Since the user specified "不影响显示即可,"
  cosmic-text's full Unicode support must be retained.

## Consequences

### Positive

- Same GPU capabilities with ~75% less code (~2.5 KLOC vs ~9.6 KLOC)
- Simplified shader pipeline (3 active instead of 5)
- Cell data comes from Ghostty `CellIterator` — no `GridSnapshot` intermediary
- Easier to debug (flatter structure, fewer abstraction layers)

### Negative

- Loses the theoretical separation between "renderer" and "application" —
  but this was never a real separation (there's only one consumer of the
  renderer)
- Merging the pipeline with the main code means full-rebuild on any shader
  or rendering change (acceptable for single-developer project)

## Status Note (Aug 2026)

Key decisions implemented:

- ✅ wgpu + cosmic-text + swash + guillotiere **retained** as core render dependencies
- ✅ `CellData`/`build_instances_from_cell_data` **implemented** as primary render path
- ✅ GridSnapshot → `CellIterator` transition **done** for render path (GridSnapshot retained for queries)
- ✅ Module hierarchy **flattened**: `gpu-renderer/` → `native/src/render/`

Decisions **not implemented** (scope evolved or deferred):

- ✅ Shader count: 3 **active** WGSL files (cell, background, kgp) —
  2 dead files (background_blur_h, background_blur_v) deleted Aug 2026
- ✅ `GpuContext` → `Renderer` rename completed Aug 2026 (20+ files, type alias for compat)
- ❌ `pipeline.rs` still a centralized file (~604 lines), not split into per-function modules
- ❌ Font module files (`font/pipeline.rs`, `font/rasterization.rs`, `font/cjk.rs`) **relocated** to `native/src/render/font/` but not inlined
- ❌ KLOC estimate (~2,450) under-estimated — actual core render code is ~6,500 lines (features like background blur, KGP added)

- Frame time budget: <16 ms per frame at 60 fps on Adreno 6xx+
- `ATrace_beginSection`/`_endSection` markers in all render functions
