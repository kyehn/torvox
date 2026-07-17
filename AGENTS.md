# AGENTS.md

## Project Context

Torvox is a GPU-accelerated Android terminal emulator using wgpu (Vulkan) for rendering, Ghostty VT parsing, and a Kotlin+Compose UI. Crate dependency order is strictly one-way.

## Setup and Commands

One-time setup:
```bash
git config core.hooksPath .githooks
```

Commands:
```bash
cargo test --workspace # All Rust tests
cargo clippy --all -- --deny warnings # Lint
cargo fmt --check # Format check
cd android && ./gradlew assembleDebug # Android debug APK
cd android && ./gradlew spotlessCheck detekt # Kotlin lint
nu scripts/check-rust.nu # Rust CI script
nu scripts/test-android-gradle.nu # Android CI script
cargo test --package torvox-core
cargo test run --package torvox-core --test property_tests
```

---

## Before Commit

Checklist (run `git config core.hooksPath .githooks` once before first commit):



1. `cargo test --workspace` exits 0
2. `cargo clippy --all -- --deny warnings` exits 0
3. `cargo fmt --check` exits 0
4. `cd android && ./gradlew spotlessCheck detekt` exits 0
5. `cargo geiger --package torvox-core` shows no new `unsafe`
6. Bridge type sync: if `torvox-core` types changed, `bridge.rs` + `TorvoxBridge.kt` updated

---

## Hooks

Install once:
```bash
git config core.hooksPath .githooks
```

- `.githooks/commit-msg` — Advisory conventional commit check. Warns on bad format, blocks "changes"/"wip" messages.
- `.githooks/pre-push` — Runs `cargo fmt --check`, `cargo clippy --all -- --deny warnings`, `./gradlew spotlessCheck` before push (30s timeout per command).

Use `git push --no-verify` to bypass hooks in emergencies (e.g., broken toolchain).

---

## Boundaries

### Must

- Read `docs/standards/` before changing a crate
- Anchor new `unsafe` blocks with a safety comment (`// SAFETY: ...`)

### Never

- Java files, portable-pty, bincode, rust-android-gradle
- `unsafe` in `torvox-core`
- `setup_scaffolding!()` in multiple crates
- `Canvas.drawText` per cell, raw bytes across FFI, `/proc/self/exe`
- `anyhow` in library crates — use `thiserror 2`
- boltffi Error `message` field — conflicts with Kotlin `Throwable.message`

---

## Standards (read before writing code)

- `docs/standards/STYLE.md` — Shell/Nix/GHA/General style
- `docs/standards/TESTING.md` — Test locations, commands
- `docs/standards/QUALITY-GATE.md` — Pre-commit checks, bridge change, E2E

---

## Architecture — Summary

See `docs/architecture.md` for the full architecture document including module boundaries, data flow, thread model, design decisions with requirement traceability, and error handling strategy.

### Crate Direction (strict one-way)

```text
libghostty-vt / libghostty-vt-sys ← torvox-core ← torvox-terminal ←
torvox-renderer ← torvox-gui-android ← android/app
```

Each crate depends only on the crate directly below it in the chain. Violations break the build. Verify with `cargo metadata --no-deps --format-version 1`.

### Thread Count

6-7 threads per session (PTY Reader, Input Writer, Process Waiter, RenderThread, plus shared bridge/MCP threads). See `docs/architecture.md#4-thread-model`.

### Render Path

GPU-only via wgpu (Vulkan). No GL, no CPU software fallback. Emulators use SwiftShader; Linux uses Lavapipe. See `docs/architecture.md#3-data-flow` for the full pipeline.

---

## When Writing Code

- Read `docs/standards/STYLE.md` before writing any file
- `torvox-core` is `#![no_std]`: no `std::`, no `alloc::` unless behind `#[cfg(feature = "std")]`
- `torvox-core` has zero `unsafe`: verify with `cargo geiger --package torvox-core`
- Sync `torvox-gui-android/src/bridge.rs` types when changing `torvox-core` types
- Run `TorvoxBridge.kt` JNA bindings check when modifying bridge types
- Lint after every file change: `cargo clippy --all -- --deny warnings`
- No magic numbers: use named constants with descriptive names
- No abbreviations: `config` not `cfg`, `background` not `bg`, `application` not `app`, `terminal` not `term`
- No `#[allow]` in production source code (test helpers excepted)
- No hardcoded `/data/.*/files` paths for app data — use `filesDir`
- No icons in Toast messages
- No `||` in Nushell scripts (invalid syntax)
- No bash/sh — Nushell only
- Rust: use `std::hint::black_box` not deprecated `criterion::black_box`
- Kotlin: use `SharingStarted.WhileSubscribed(TIMEOUT_MILLIS)` with named constant

---

## When Testing

- Read `docs/standards/TESTING.md` before writing tests
- Test public API only, not internal implementation
- One test = one behavior
- Run full suite before commit: `cargo test --workspace`

## Long Output Handling

- Commands that generate large output (dependency trees, full-stack traces) must save output to a temp file instead of dumping inline in panic messages.
- Use `std::env::temp_dir()` for the dump path and reference it in the error message.
- Retry operations only with a bounded maximum (e.g., emulator boot wait, max 7 min).
- `cargo-machete` must use `--skip-target-dir` to avoid IO errors on cached build artifacts. Do NOT use `--with-metadata` unless dependency renaming is present — it causes false positives with proc-macro deps like quickcheck.

---

## When Blocked

- If tests fail: fix the failing test
- If a dependency is missing: check `flake.nix` first, then ask
- If you encounter merge conflicts: stop and show the conflicting files
- Prefer fixing root causes: avoid deleting files, skipping tests, or adding `#[allow(...)]` to suppress real issues
- Plan every non-trivial change: exploration, planning, implementation, and acceptance review.

## Known Pitfalls

| # | Pitfall | Lesson |
|---|---------|--------|
| 1 | `Shell::Custom(u8)` | u8 too small → `String`, lost Copy |
| 2 | `DirtyLine` enum | Changed to `DirtyMask { partitions: Vec<u64> }` |
| 3 | thiserror 2.x + no_std | Set optional, std feature enables |
| 4 | boltffi multi-crate export | Only one export location allowed |
| 5 | boltffi `message` field | Conflicts with Kotlin `Throwable.message` |
| 6 | ghostty uses zig_0_15 | Ensure `zig_0_15` is first in PATH via `shellHook` so ghostty finds the correct version. No `CARGO_TARGET_*_LINKER` needed. |
| 7 | boltffi CLI no bridge gen | Use JNA manual binding (TorvoxBridge.kt) |
| 8 | libghostty-vt API | `scrollback_rows()` not `history_size()`; `resize(rows, cols)` two params |
| 9 | Ghostty Android linking | Dynamic (dylib) + build.rs SONAME strip; static fails (Zig install archive has only lib_vt.o) |
| 10 | Ghostty SONAME | `libghostty-vt.so.0` NEEDED in ELF; build.rs strips versioned SONAME — if skipped, Gradle filters versioned .so |
| 11 | Zig C++ namespace | Zig uses `std::__1`, NDK `libc++_shared.so` uses `std::__ndk1` — must bundle matching libc++ |
| 12 | TextureView/SurfaceView z-order | 当前项目使用 `TextureView`，无需 `setZOrderOnTop`。旧版 SurfaceView 方案在 SwiftShader 模拟器上 `setZOrderOnTop(true)` 导致 overlay alpha=0 画面不可见。|
| 13 | Render thread death | After 100 consecutive errors (~10s), thread exits permanently; must restart on new surface |
| 14 | ProGuard R8 | `-dontoptimize` required for JNA reflection-based binding on release builds |
| 15 | ADB on phone emulator | Use emulator device test. |
| 16 | `applicationId = "com.termux"` | Intentional design — do NOT change. `test-emulator.nu` runs `pm uninstall --user 0 com.termux` before Gradle to avoid `INSTALL_FAILED_UPDATE_INCOMPATIBLE`. |
| 17 | APK testkey | Must download from AOSP (`android.googlesource.com/platform/build`). Self-signing (`openssl req -x509`, `keytool -genkey`) is forbidden anywhere in the codebase. |
| 18 | rapidocr CLI not Python module | All OCR code must use `rapidocr` CLI command, NOT `from rapidocr import RapidOCR`. Model path is patched in `flake.nix`. Use `oldAttrs` not `old`, `postPatch` for patch source code. |
| 19 | Mesa Lavapipe for Vulkan | GPU renderer uses Vulkan via wgpu. Mesa's Lavapipe (`lvp_icd.x86_64.json`) provides software Vulkan when no physical GPU is available. Do NOT claim "no GPU" — Lavapipe IS the Vulkan implementation. Configured in `flake.nix` via `VK_ICD_FILENAMES`. Emulator provide SwiftShader for the guest GPU. |

---

## Key Files

| File | Purpose |
|------|---------|
| `torvox-core/src/cell.rs` | Cell, Attrs, Color, DirtyMask (no_std) |
| `torvox-core/src/grid.rs` | Grid, Scrollback |
| `torvox-core/src/config.rs` | ThemeConfig, ShellConfig, TerminalConfig |
| `torvox-core/src/selection.rs` | Selection modes (char/word/line/block) |
| `torvox-core/src/event.rs` | TerminalEvent, FocusEvent, CwdEvent |
| `torvox-core/src/snapshot.rs` | rkyv serialization for Android bridge |
| `torvox-terminal/src/pty.rs` | PtyPair — only allowed fork unsafe |
| `torvox-terminal/src/session.rs` | Session orchestrator, clipboard, shell integration |
| `torvox-terminal/src/ghostty_terminal.rs` | GhosttyTerminal (VT engine wrapper) |
| `torvox-terminal/src/keyboard.rs` | Input encoding (Kitty keyboard protocol) |
| `torvox-terminal/src/shell_env.rs` | ShellEnv (pre-exec environment setup) |
| `torvox-renderer/src/gpu.rs` | wgpu render pipeline, atlas, instance management |
| `torvox-renderer/src/font.rs` | cosmic-text shaping, swash glyph rasterization |
| `torvox-gui-android/src/bridge.rs` | boltffi data bridge — only export location |
| `torvox-gui-android/src/jni_bridge.rs` | JNI bridge for NDK functions (ANativeWindow) |
| `torvox-gui-android/src/surface.rs` | AndroidSurface, render pipeline |
| `torvox-mcp/src/main.rs` | MCP server (JSON-RPC over Unix socket) |
| `fuzz/fuzz_targets/` | cargo-fuzz targets (VT, OSC, grid, keyboard, selection, attrs, wire) |

---

## Protected Files (Read-Only Unless Explicitly Requested)

- `.config/`, `.cargo/`, `.github/`, `assets/`, `scripts/` (directories, set read-only)
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
