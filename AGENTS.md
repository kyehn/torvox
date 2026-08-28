# AGENTS.md

GPU-accelerated Android terminal emulator using wgpu (Vulkan) for rendering, Ghostty VT parsing (via `libghostty-vt-sys`), and Kotlin+Compose UI.

### Must

- Read `docs/specification/` before changing

### Never

- Java files, portable-pty, rust-android-gradle
- `Canvas.drawText` per cell, raw bytes across FFI, `/proc/self/exe`
- `anyhow` in library crates — use `thiserror 2`
- `unsafe` in the core terminal data path
- JNA reflection-based binding — delete any remaining JNA code if found
- bash/sh scripts — Nushell only

---

## When Writing Code

- Read `docs/specification/` before writing any file
- No magic numbers: use named constants with descriptive names
- No abbreviations: `config` not `cfg`, `background` not `bg`, `terminal` not `term`
- No `#[allow]` in production source code (test helpers excepted)
- No hardcoded `/data/.*/files` paths for app data
- No `||` in Nushell scripts (invalid syntax)
- Rust: use `std::hint::black_box` not deprecated `criterion::black_box`
- Kotlin: use `SharingStarted.WhileSubscribed(TIMEOUT_MILLIS)` with named constant

## When Blocked

- If a dependency is missing: check `flake.nix` first, then ask
- If you encounter merge conflicts: stop and show the conflicting files
- Prefer fixing root causes: avoid deleting files, skipping tests, or adding `#[allow(...)]` to suppress real issues

## Protected Files

- `.github/`, `scripts/`, `flake.nix`, `rust-toolchain.toml`, `README.md`

Wait for the user to ask before modifying them.

- `AGENTS.md` `docs/specification/`

如果需要修改，这些文件可以添加简洁的注释。允许修改拼写/语法错误，支持调整格式/排版。
