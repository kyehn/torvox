# AGENTS.md

## Project Context

GPU-accelerated Android terminal emulator using wgpu (Vulkan) for rendering, Ghostty VT parsing (vendored via `libghostty-vt-sys`), and a Kotlin+Compose UI. Ghostty is the single source of truth for all terminal state — no separate data-model crate.

Single crate (`native/`) with 2 thin workspace members, ~29.5k LOC Rust, JNI direct bridge (no boltffi/JNA).

## Setup and Commands

Commands:

```bash
cargo test --workspace        # All Rust tests
cargo clippy --all -- --deny warnings  # Lint
cargo fmt --check             # Format check
cd android && ./gradlew assembleDebug  # Android debug APK
cd android && ./gradlew spotlessCheck detekt  # Kotlin lint
nu scripts/check-rust.nu     # Rust CI script
nu scripts/test-android-gradle.nu  # Android CI script
```

---

## Before Commit

Checklist:

1. `cargo test --workspace` exits 0
2. `cargo clippy --all -- --deny warnings` exits 0
3. `cargo fmt --check` exits 0
4. `cd android && ./gradlew spotlessCheck detekt` exits 0
5. Bridge type sync: if `ffi.rs` JNI signatures changed, update `NativeBridge.kt`

---

## Hooks

No pre-installed hooks. Run checks manually before commit.

---

## Boundaries

### Must

- Read `docs/standards/` before changing a crate
- Anchor new `unsafe` blocks with a safety comment (`// SAFETY: ...`)

### Never

- Java files, portable-pty, rust-android-gradle
- `Canvas.drawText` per cell, raw bytes across FFI, `/proc/self/exe`
- `anyhow` in library crates — use `thiserror 2`
- `unsafe` in the core terminal data path (ghostty_terminal internals)
- JNA reflection-based binding — delete any remaining JNA code if found
- bash/sh scripts — Nushell only

---

## Standards (read before writing code)

- `docs/standards/STYLE.md` — Shell/Nix/GHA/General style
- `docs/standards/TESTING.md` — Test locations, commands
- `docs/standards/QUALITY-GATE.md` — Pre-commit checks, bridge change, E2E

---

## Architecture — Summary

See `docs/architecture.md` for the full architecture document.

---

## When Writing Code

- Read `docs/standards/STYLE.md` before writing any file
- `native/src/android/ffi.rs` is the single JNI export location — keep `NativeBridge.kt` in sync
- Lint after every file change: `cargo clippy --all -- --deny warnings`
- No magic numbers: use named constants with descriptive names
- No abbreviations: `config` not `cfg`, `background` not `bg`, `terminal` not `term`
- No `#[allow]` in production source code (test helpers excepted)
- No hardcoded `/data/.*/files` paths for app data — use `filesDir`
- No icons in Toast messages
- No `||` in Nushell scripts (invalid syntax)
- Rust: use `std::hint::black_box` not deprecated `criterion::black_box`
- Kotlin: use `SharingStarted.WhileSubscribed(TIMEOUT_MILLIS)` with named constant

---

## When Testing

- Read `docs/standards/TESTING.md` before writing tests
- Test public API only, not internal implementation
- One test = one behavior
- Run full suite before commit: `cargo test --workspace`

## Long Output Handling

- Commands that generate large output must save to temp file instead of dumping inline.
- Use `std::env::temp_dir()` for the dump path.
- Retry operations only with a bounded maximum (e.g., emulator boot wait, max 7 min).
- `cargo-machete` must use `--skip-target-dir` to avoid IO errors. Do NOT use `--with-metadata` unless dependency renaming is present.

---

## When Blocked

- If tests fail: fix the failing test
- If a dependency is missing: check `flake.nix` first, then ask
- If you encounter merge conflicts: stop and show the conflicting files
- Prefer fixing root causes: avoid deleting files, skipping tests, or adding `#[allow(...)]` to suppress real issues
- Plan every non-trivial change: exploration, planning, implementation, and acceptance review.

---

## Known Pitfalls

| # | Pitfall | Lesson |
|---|---------|--------|
| 1 | Ghostty Zig version | Uses `zig_0_16` — ensure it's first in PATH via `shellHook`. No `CARGO_TARGET_*_LINKER` needed. |
| 2 | libghostty-vt API | `scrollback_rows()` not `history_size()`; `resize(rows, cols)` two params |
| 3 | Ghostty Android linking | Dynamic (dylib) + build.rs SONAME strip; static fails (Zig install archive has only lib_vt.o) |
| 4 | Ghostty SONAME | `libghostty-vt.so.0` NEEDED in ELF; build.rs strips versioned SONAME — if skipped, Gradle filters |
| 5 | Zig C++ namespace | Zig uses `std::__1`, NDK `libc++_shared.so` uses `std::__ndk1` — must bundle matching libc++ |
| 6 | TextureView z-order | Use `TextureView`, no `setZOrderOnTop`. Old SurfaceView approach on SwiftShader made overlay invisible. |
| 7 | Render thread death | After 100 consecutive errors (~10s), thread exits permanently; must restart on new surface |
| 8 | ProGuard R8 | `-dontoptimize` required for JNI on release builds (prevents native method stripping) |
| 9 | ADB on emulator | Use emulator device test, not phone/tablet. |
| 10 | `applicationId = "com.termux"` | Intentional — do NOT change. `test-emulator.nu` runs `pm uninstall --user 0 com.termux` first. |
| 11 | APK testkey | Must download from AOSP. Self-signing is forbidden. |
| 12 | rapidocr CLI | All OCR code must use `rapidocr` CLI command, NOT Python module. |
| 13 | Mesa Lavapipe | GPU renderer uses Mesa Lavapipe for software Vulkan when no physical GPU. Configured via `VK_ICD_FILENAMES`. |
| 14 | JNI function naming | Java_io_term_bridge_NativeBridge_* naming — must match Kotlin `external fun` declarations exactly. |
| 15 | ANativeWindow ptr | Surface handle must be extracted on JNI thread via `ANativeWindow_fromSurface` and sent to render thread via command queue. Do NOT call NDK functions on the render thread. |

---

## Key Files

| File | Purpose |
|------|---------|
| `native/src/terminal/ghostty_terminal/types.rs` | CellData, CursorStyle, SelectionMode, is_wide |
| `native/src/terminal/ghostty_terminal/internal.rs` | CellIterator loop, build_cell_data(), VT thread run loop |
| `native/src/terminal/ghostty_terminal/commands.rs` | Command enum, RunConfig, cell_data_tx channel |
| `native/src/terminal/ghostty_terminal/public_api.rs` | spawn(), Session handle |
| `native/src/terminal/pty.rs` | PtyPair — only allowed fork unsafe |
| `native/src/render/cell_builder.rs` | CellInstanceConfig, build_instances_from_cell_data() |
| `native/src/render/context.rs` | Renderer (wgpu device/queue/surface) |
| `native/src/render/pipeline.rs` | wgpu shader pipelines (WGSL) |
| `native/src/render/pass.rs` | render_cell_data(), render_frame() |
| `native/src/render/surface.rs` | ANativeWindow surface management |
| `native/src/android/ffi.rs` | JNI FFI exports (14 JNI functions) |
| `native/src/mcp.rs` | MCP server (tower-mcp, CLI-compatible protocol) |
| `native/src/lock_util.rs` | poison recovery helpers |
| `android/app/.../NativeBridge.kt` | JNI native method declarations |
| `native/src/render/font/mod.rs` | cosmic-text shaping, swash glyph rasterization |

---

## Protected Files (Read-Only Unless Explicitly Requested)

- `.github/`, `scripts/` (directories, set read-only)
- `flake.nix`

These files and directories are set read-only. Wait for the user to ask before modifying them.

---

## scripts/ Directory

Only these 9 files allowed. No new files — merge into existing.

1. `bootstrap-libghostty.nu`
2. `build-android-libs.nu`
3. `build-apk.nu`
4. `check-rust.nu`
5. `download-rapidocr-models.nu`
6. `fetch-aosp-testkey.nu`
7. `setup-emulator.nu`
8. `test-android-gradle.nu`
9. `test-emulator.nu`

## .github/workflows

Only these 3 files, each with 1 job max. No new files.

1. `rust-checks.yml`
2. `release.yml`
3. `android-tests.yml`

Prefer `scripts/` over workflows. Only modify workflows when scripts cannot solve the problem.

- `check-rust.nu` → `rust-checks.yml`
- `build-android-libs.nu` / `build-apk.nu` / `test-emulator.nu` → `release.yml`
- `test-android-gradle.nu` → `android-tests.yml`
- `bootstrap-libghostty.nu` / `download-rapidocr-models.nu` / `setup-emulator.nu` → auxiliary tools

---

## docs/standards/ Reference

| File | When to Read |
|------|-------------|
| `docs/standards/STYLE.md` | Before writing any file |
| `docs/standards/TESTING.md` | Before writing tests |
| `docs/standards/QUALITY-GATE.md` | Before review or commit |
