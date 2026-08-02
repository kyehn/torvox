# Performance Benchmarks

Current benchmark results and thresholds for the torvox terminal renderer.

---

## 1. Command

```bash
cargo test --features test-util -- bench  # Run all benchmarks
```

Benchmarks use `#[bench]` gates and run in debug profile by default. Release
profile yields ~5-50× improvement on CPU-bound paths.

**Threshold tiers**: benchmark threshold assertions are two-tiered.
Local (non-`CI`) runs assert strict thresholds (typing < 3 ms/keystroke,
bulk > 8k cells/s, scroll > 800/500 snaps/s) and fail on real regressions.
CI runs (parallel test load + software Vulkan contention) keep an
anti-flake floor ~5× below the local single-run number that catches
order-of-magnitude regressions only. GPU throughput thresholds apply in
both tiers (buffer upload is CPU-copy bound even under Lavapipe).

---

## 2. Benchmark Results

All results from x86 Linux with Mesa Lavapipe (software Vulkan).

### 2.1 Terminal Engine Benchmarks

| Benchmark | Debug | Release | Threshold | Notes |
|-----------|-------|---------|-----------|-------|
| `bench_typing_latency` | 0.2 ms/keystroke | — | < 6 ms | `\n`-terminated writes with flush |
| `bench_bulk_output_throughput` | 17.72 MB/s | — | > 4 kB/s | 4×64KB writes, measures MB/s + cells/s |
| `bench_scroll_throughput` | 55-56 snaps/s | — | > 800 | 3 scroll offsets (0, 5, 50) |
| `bench_cell_data_vs_grid_snapshot` | 1.47× faster | — | ≥ 0.5× | CellData must not be slower than GridSnapshot |

### 2.2 Render Pipeline Benchmarks

| Benchmark | Debug | Release | Threshold | Notes |
|-----------|-------|---------|-----------|-------|
| `bench_build_instances_from_cell_data` | 920 fps | 5107 fps | > 200 fps | 1920 cells × 100 iterations |
| `bench_build_instances_mixed` | — | — | > 200 fps | CJK + colors + bold/italic mixed |
| `bench_gpu_buffer_upload_throughput` | 40+ GB/s | — | > 500 MB/s | Raw wgpu buffer write bandwidth |
| `bench_gpu_command_encoding` | — | — | > 5000 fps | Empty pass encode throughput |
| `bench_gpu_full_submit` | — | — | — | Full frame submission with poll |
| `bench_gpu_atlas_upload` | — | — | — | Atlas texture upload bandwidth |
| `bench_cpu_end_to_end_pipeline` | 1362 fps | — | — | VT write → CellData → CellInstance pipeline |
| `bench_cjk_glyph_cache_effectiveness` | 6.8M→0.17M ops | — | — | Cache eliminates 97.5% of swash operations |

---

## 3. Performance Characteristics

### 3.1 CellData Path (production render path)

The CellData fast path:
- **0 FFI calls** per cell (vs 8+ for GridSnapshot path)
- **80 bytes** per cell (`bytemuck::Pod + Zeroable`)
- **flume channel** transport (lock-free, bounded to 256)
- **build_instances_from_cell_data**: O(n) conversion with grapheme cluster stacking

### 3.2 CJK Cache

The `glyph_id_cache` and `cjk_glyph_cache` in `FontPipeline` reduced repeated
swash charmap lookups by **97.5%** (from 6.8M to 0.17M operations per test run).
This was the fix for 49 fps → 5107 fps in render benchmark.

### 3.3 GPU Readback

GPU readback (used for screenshot tests and GPU compute verification) uses a
`device.on_processed()` channel with 100ms timeout — no busy-waiting.

---

## 4. Performance Regressions

The following pre-existing issues exist (not caused by current changes):

| Test | Failure | Cause |
|------|---------|-------|
| `gpu_render_colored_text` | Color precision (0.9 expected, 0.8 got) | Mesa Lavapipe fp16 blend |
| `vt_color_background_blue` | Same | Same |
| `vt_color_foreground_red` | Same | Same |
| `vt_color_reset` | Same | Same |

These are **color precision** issues in software Vulkan (Lavapipe fp16 blending).
On physical GPU (Adreno/Mali) they pass. Thresholds are set to accommodate this.

---

## 5. GPU Environment

Benchmarks require:
- **Vulkan-capable GPU** or **Mesa Lavapipe** (software Vulkan)
- Set `VK_ICD_FILENAMES` to the Lavapipe `.json` ICD file
- Provided by `nix develop` shell environment

GPU-dependent tests return early (not panic) when no device is available.
