# 0006 — Build and Dependency Simplification

- **Status**: Accepted
- **Date**: 2026-07-27
- **Requirement IDs**: NFR-02, NFR-05

## Context

The original torvox accumulated significant build complexity:

1. **no_std in terminal-core**: Required feature gates on `thiserror`,
   `hashbrown` instead of `std::collections`, no `std::error::Error`, no
   `std::sync`. This added complexity with no practical benefit — Android
   always has `std` available.

2. **boltffi**: Added `boltffi_bindgen` as a build dependency, required
   `setup_scaffolding!()` macro, and produced an external wire-format
   binary. Removed per ADR-0003.

3. **rkyv (zero-copy serialization)**: Used for `SessionSnapshot` binary
   persistence. With Ghostty as source of truth (ADR-0002), `GridSnapshot`
   is removed. For process-death recovery, the Ghostty Formatter API
   (`GHOSTTY_FORMATTER_FORMAT_VT`) produces VT escape sequences that can
   be replayed into a fresh terminal — no rkyv needed.

4. **Nix flake complexity**: The Nix configuration handled NDK cross-
   compilation, Zig version pinning for Ghostty, etc. This was necessary
   with the old crate structure but can be simplified for a single crate.

## Decision

### Remove

| Component | Replaced by | KLOC saved |
|-----------|-------------|------------|
| no_std feature gates | Normal `std` | ~500 lines of conditional code |
| boltffi | Direct JNI (`jni` crate) | 4.6 KLOC bridge + build dep |
| rkyv | Ghostty Formatter API for session persistence | ~300 lines + dep tree |
| `hashbrown` | `std::collections::HashMap` | 1 dep removed |
| `thiserror` no_std feature | `thiserror` (std, no feature gate) | 1 feature gate removed |

### Simplify Nix

The Nix flake still handles NDK toolchain setup but the build invocation
simplifies to:

```bash
# Build native library
cargo build --lib --release --target aarch64-linux-android

# Or via Zig (future)
zig build jni

# Build APK
cd android && ./gradlew assembleDebug
```

No `cargo metadata` validation, no boltffi code generation, no rkyv
serialization step.

## Alternatives Considered

### Keep no_std for "portability"
- **Rejected**: No realistic scenario requires torvox to run on embedded
  targets without std. The constraint was self-imposed and unenforced by
  the actual build targets.

## Consequences

### Positive

- Standard Rust — no feature gates, no conditional imports
- Smaller dependency tree (~15 fewer transitive deps)
- Faster builds (no boltffi code generation, no rkyv)
- Simpler Nix configuration

### Negative

- `std` is slightly larger on disk but negligible in a 7+ MB APK
- Session persistence changes from binary rkyv to text VT sequences
  (Ghostty Formatter output is ~2x larger but still <100 KB per session)
- Must revert to `std::collections::HashMap` which is marginally slower
  than `hashbrown` (not measurable at terminal data volumes)

## Compliance

- No `#[cfg(not(feature = "std"))]` in any Rust file
- No `boltffi` or `rkyv` in workspace dependencies
- `Cargo.toml` has no feature gates related to no_std

## Status Note (Jul 2026, updated Aug 2026)

This decision has been fully implemented. `#![no_std]`, `rkyv`, and `boltffi` have all been removed from the workspace. `bytemuck` is used for GPU data transport (CellData struct). The workspace now has 3 crates (`native`, `exec-bin`, `integration-tests`) and builds with zero errors. The `Cargo.toml` dependency tree was further simplified in Phase 4 (crate consolidation).
