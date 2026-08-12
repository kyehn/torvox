# 11. Testing Strategy

## Status

Accepted

Date: 2026-09-05

## Requirement IDs

NFR-020, NFR-026, NFR-027, NFR-028, NFR-029, FR-017

## Context

Torvox is a Rust-native Android terminal emulator with a rendering
pipeline that requires GPU hardware for pixel-perfect validation. The
testing strategy must account for:

- **GPU-dependent code**: wgpu rendering pipelines, cell builder raster,
  font atlas allocation — all produce GPU-dependent output.
- **Cross-compilation**: The `native/` crate targets `aarch64-linux-android`
  for production, but tests run on the host (x86_64 Linux/macOS).
- **Ghostty dependency**: The VT parser and terminal state are forked from
  Ghostty and evolve independently. Changes upstream can break downstream
  assumptions.
- **FFI boundary**: JNI functions are only callable from Android — they
  cannot be unit-tested on the host.
- **Integration with Android**: Session lifecycle, surface management, and
  process death recovery require an Android device or emulator.

The project currently has three CI workflows:

1. `rust-checks.yml` — host-side Rust checks (clippy, unit tests, GPU
   snapshot tests via lavapipe/SwiftShader)
2. `android-tests.yml` — Android emulator instrumented tests
3. `release.yml` — APK build and release

## Decision

### 1. Three-tier test architecture

```
┌─────────────────────────────────────────────────┐
│  Tier 3: Android Instrumented Tests             │
│  (real device / emulator — CI android-tests.yml)│
│  FR-014, FR-015, FR-016, FR-017, NFR-034       │
├─────────────────────────────────────────────────┤
│  Tier 2: Host GPU Snapshot Tests                │
│  (lavapipe/SwiftShader — CI rust-checks.yml)    │
│  NFR-030, NFR-031, NFR-032                      │
├─────────────────────────────────────────────────┤
│  Tier 1: Host Unit Tests                        │
│  (no GPU — cargo test on host)                  │
│  Ghostty VT tests, log_chunk, peer_uid,        │
│  theme, cursor, cell builder geometry           │
└─────────────────────────────────────────────────┘
```

### 2. Tier 1 — Host unit tests (no GPU)

Pure-logic tests that run on any host without GPU access:

- **Ghostty VT parser tests**: Forked from upstream with the same test
  suite. Run with `cargo test` on host. Validates VT sequence parsing,
  cursor movement, scrolling, OSC handling. These tests are the primary
  regression gate for Ghostty-derived code.
- **Log chunking**: `log_chunk::chunk_message` unit tests verify
  message splitting logic.
- **MCP peer UID**: `peer_uid_allowed` logic tests.
- **Cell builder geometry**: Pixel-space cell positioning tests that
  mock GPU texture dimensions.
- **Theme conversion**: Ghostty → wgpu uniform buffer conversion tests.
- **Cursor style mapping**: Ghostty cursor → wgpu atlas index tests.

These tests have no feature gates and run in every CI workflow.

### 3. Tier 2 — Host GPU snapshot tests (lavapipe/SwiftShader)

Tests that require a GPU device but run on CI via software rendering:

- **Full render pipeline snapshot**: `render_to_buffer()` produces a
  pixel buffer from a known terminal state. Output is compared against
  golden PNG files using `image-compare` crate (< 2% RMS threshold).
  Updated with `UPDATE_GPU_SNAPSHOTS=1`.
- **Cell builder raster**: Verifies glyph atlas population and texture
  coordinates for ASCII and CJK characters.
- **Font atlas allocation**: Tests atlas page splitting and compaction.

These tests are feature-gated behind `gpu-tests` and require lavapipe
(`VK_ICD_FILENAMES=lvp_icd.x86_64.json`) or SwiftShader. CI runs them
in `rust-checks.yml` with the lavapipe ICD pre-configured.

### 4. Tier 3 — Android instrumented tests

Tests that run on an Android emulator in CI (`android-tests.yml`):

- **JNI lifecycle**: `initSession`, `attachWindow`, `detachWindow`,
  `destroySession` call sequences.
- **PTY spawn + echo**: Verifies shell session creation and basic I/O.
- **Surface handoff**: `SurfaceCommand` queue delivery to render thread.
- **Session switching**: Multiple session create/switch/destroy cycles.
- **Process death recovery**: Kill app process, verify state restoration.

These tests use `androidx.test` and require a running emulator with GPU
emulation (SwiftShader).

### 5. Snapshot update protocol

GPU snapshot tests use golden files stored alongside the test source.
The update workflow:

1. Developer runs `UPDATE_GPU_SNAPSHOTS=1 cargo test --features gpu-tests`
2. New PNG files are written to the snapshot directory
3. Developer inspects the new snapshots visually
4. Updated snapshots are committed with the code change

Snapshot format: PNG files compared with < 2% RMS error via
`image-compare`. The `assert` module provides
`compare_images_equal_with_tolerance()`.

### 6. Ghostty divergence tracking

Ghostty-derived code (`terminal/ghostty_terminal/`) carries a test suite
forked from upstream. When updating from Ghostty:

1. Pull upstream VT tests into `terminal/ghostty_terminal/`
2. Run the test suite — any failures indicate behavioral divergence
3. Document divergences in the relevant ADR or code comments
4. The divergence documentation requirement (per ADR-0002) ensures
   future developers understand what changed

### 7. CI quality gates

All three CI workflows enforce:

- `cargo fmt -- --check` — formatting
- `cargo clippy --all-targets` — lint (warnings denied)
- `cargo test` — host unit tests
- `cargo test --features gpu-tests` — GPU snapshot tests (lavapipe)
- Android emulator instrumented tests (Tier 3)

No PR merges without all gates passing.

## Alternatives Considered

### Property-based testing (proptest/quickcheck)
- **Deferred**: The VT parser and rendering pipeline are deterministic —
  property testing would add complexity without proportional benefit for
  v1. Could be added later for fuzzing VT sequence edge cases.

### Visual regression via screenshots (Percy/BackstopJS)
- **Rejected**: These tools are designed for web UI testing. The wgpu
  rendering pipeline produces pixel buffers, not DOM — the `image-compare`
  approach is simpler and more direct.

### Mock GPU for all tests
- **Rejected**: Mocking the GPU would lose the primary value of Tier 2
  tests — verifying that the rendering pipeline produces correct pixels.
  Software rendering (lavapipe/SwiftShader) provides real GPU execution
  at acceptable CI cost.

## Consequences

### Positive

- Clear separation of test concerns — no GPU needed for Tier 1 tests
- GPU snapshot tests catch rendering regressions that unit tests cannot
- Ghostty divergence is tracked through forked test suites
- CI gates prevent regressions from reaching main

### Negative

- Tier 3 tests are slow (Android emulator startup) — run only on CI, not
  locally by default
- GPU snapshot golden files must be regenerated when font loading or
  atlas layout changes — requires manual visual inspection
- lavapipe/SwiftShader output may differ slightly from real GPU output —
  tolerance thresholds need occasional tuning

## Compliance

- `cargo test --workspace` must pass before every commit
- `cargo clippy --all -- --deny warnings` enforced in CI
- GPU snapshot tests require Lavapipe (VK_ICD_FILENAMES) or real GPU
- Tier 3 emulator tests run in CI only (`scripts/test-emulator.nu`)
- Test count tracked in `docs/traceability.yml`
