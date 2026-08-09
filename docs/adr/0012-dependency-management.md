# 0012 — Dependency Management

- **Status**: Accepted
- **Date**: 2026-09-05
- **Requirement IDs**: NFR-011, NFR-012, NFR-013

## Context

Torvox's `native/` crate has a deliberately minimal dependency profile.
The project targets Android NDK r27 with static linking via `cargo-ndk`,
and each dependency must satisfy strict constraints:

- **Android NDK compatibility**: Must build for `aarch64-linux-android`
  with NDK r27 (API 35). Dependencies that assume glibc or desktop-only
  syscalls are excluded.
- **Static linking**: All Rust dependencies are statically linked into
  `libtorvox.so`. Dynamic dependencies must be Android NDK system libraries
  (libc, liblog, libandroid, libEGL, libvulkan, libjnigraphics).
- **Binary size**: The APK must remain ≤ 12 MB. Each dependency adds to
  the final `.so` size.
- **Build reproducibility**: All dependencies are pinned via `Cargo.lock`.
  The `deny.toml` configuration blocks crates with license or advisory
  issues.

The workspace is structured as:

```
Cargo.toml              — workspace root (virtual)
native/Cargo.toml       — libtorvox (the only library crate)
exec-bin/Cargo.toml     — standalone binary for host-side debugging
integration-tests/Cargo.toml — Android instrumented tests
```

Only `native/` ships in the APK. `exec-bin/` and `integration-tests/`
are development-only.

## Decision

### 1. Dependency inventory (native crate)

The `native/Cargo.toml` declares these direct dependencies:

| Crate | Purpose | Justification |
|-------|---------|---------------|
| `jni` | JNI FFI bridge | Kotlin ↔ Rust boundary |
| `ndk` | Android NDK bindings | `ANativeWindow`, logging |
| `ndk-sys` | Raw NDK symbols | Low-level NDK access |
| `wgpu` | GPU rendering | Vulkan/WebGPU abstraction |
| `wgpu-core` | GPU state management | Required by wgpu |
| `wgpu-hal` | Hardware abstraction | Required by wgpu |
| `naga` | Shader compilation | WGSL → SPIR-V |
| `bytemuck` | Safe transmutation | Pod casts for GPU buffers |
| `swash` | Font shaping/text | Glyph lookup and shaping |
| `skrifa` | Font parsing | OpenType font table access |
| `log` | Log facade | Structured logging |
| `android_logger` | Logcat routing | Android log output |
| `tokio` | Async runtime | MCP server async I/O |
| `tower-mcp` | MCP protocol | Model Context Protocol |
| `serde` / `serde_json` | Serialization | MCP JSON-RPC, config |
| `tempfile` | Temp files | Test fixtures only |
| `image-compare` | Snapshot comparison | GPU test assertions |
| `image` | PNG encoding | Snapshot output |
| `pollster` | Block on async | wgpu initialization |

Dependencies are categorized:

- **Core runtime**: `jni`, `ndk`, `ndk-sys`, `wgpu`, `wgpu-core`,
  `wgpu-hal`, `naga`, `bytemuck`, `swash`, `skrifa`, `log`, `tokio`,
  `tower-mcp`, `serde`, `serde_json`
- **Development/test only**: `tempfile`, `image-compare`, `image`,
  `pollster` (also used at runtime for wgpu init)

### 2. Version pinning

All dependency versions are pinned in `Cargo.lock`, which is committed to
the repository. Workspace `Cargo.toml` specifies version ranges in
`[dependencies]` sections; `Cargo.lock` resolves to exact versions.

Policy:
- Patch versions (`0.x.Y`) may be updated freely via `cargo update`
- Minor versions (`0.X.0`) require explicit review — run `cargo update`
  and verify the build still passes
- Major versions (`X.0.0`) require an ADR update documenting the change

### 3. License compliance

`deny.toml` enforces:

- **Allowed licenses**: MIT, Apache-2.0, Unicode-DFS-2016, BSD-2-Clause,
  BSD-3-Clause, ISC, Zlib, Open.font-licensed fonts
- **Denied licenses**: GPL-family (incompatible with proprietary app
  distribution), CC-BY (not suitable for binary inclusion)
- **Advisory database**: `cargo deny` checks against the RustSec advisory
  database for known vulnerabilities

The CI pipeline (`rust-checks.yml`) runs `cargo deny check` on every PR.

### 4. Android NDK version pinning

The project uses **NDK r27** (API 35), pinned via:

- `ANDROID_NDK_HOME` environment variable in CI
- `rust-toolchain.toml` specifies the target triple
- `Cargo.toml` `[target.aarch64-linux-android]` section configures the
  linker

NDK upgrades require testing across all ABIs and verification that the
linker flags remain compatible.

### 5. Rust toolchain versioning

The Rust toolchain is pinned via `rust-toolchain.toml` (currently nightly
for `asm_goto` and `thread_local` features). The specific nightly date is
locked — upgrades are manual and require:

1. Update `rust-toolchain.toml`
2. Run full CI (clippy, tests, Android build)
3. Verify no new warnings or errors
4. Document the upgrade in commit message

### 6. Ghostty source tracking

Ghostty-derived code (forked from v1.1.3, commit 007e1d23e) is vendored
in `native/src/terminal/ghostty_terminal/`. It is not managed via
`Cargo.lock` — it is copied source code. Updates follow ADR-0002:

1. Pull changes from Ghostty's upstream `src/terminal/` directory
2. Adapt to torvox's rendering interface (`CellBuilder` trait)
3. Run Ghostty's VT test suite to verify no regressions
4. Document divergences

### 7. Dependency audit cadence

- **Every CI run**: `cargo deny check` (licenses, advisories, bans)
- **Monthly**: `cargo update` + full test suite to pick up patch updates
- **Quarterly**: Review dependency tree with `cargo tree` — check for
  unused or replaceable dependencies
- **Before releases**: Full `cargo audit` scan, manual review of
  dependency changelogs

## Alternatives Considered

### Vendoring all dependencies
- **Rejected**: Rust's `Cargo.lock` already provides reproducibility.
  Vendoring adds maintenance burden (manual updates) without meaningful
  security benefit when `Cargo.lock` is committed.

### Workspace-level dependency unification
- **Not needed**: Only `native/` ships in the APK. The workspace
  structure (`exec-bin/`, `integration-tests/`) uses separate dependency
  trees that do not affect the release artifact.

### `cargo-vet` for dependency verification
- **Deferred**: `cargo deny` provides license and advisory checking.
  `cargo-vet` adds cryptographic verification of dependency builds, which
  is valuable for supply-chain security but adds operational complexity.
  Can be adopted when the project has more external contributors.

## Consequences

### Positive

- Minimal dependency surface — each crate is justified and auditable
- License compliance enforced in CI — no GPL contamination
- NDK version pinned — reproducible Android builds
- Ghostty source is vendored and tracked — no surprise upstream changes
- `Cargo.lock` committed — deterministic builds for all contributors

### Negative

- NDK upgrades require manual testing across ABIs
- Ghostty updates require manual adaptation (not a `cargo update` away)
- Nightly Rust toolchain means occasional breakage from upstream changes
- Dependency tree review is quarterly — vulnerabilities between audits
  may go unnoticed (mitigated by `cargo deny` advisory checks on every
  CI run)
