# Dependencies & Technology Stack

## Core Stack

| Layer | Technology | Version | Purpose |
|-------|-----------|---------|---------|
| Rust edition | Rust 2024 | 1.97+ | Main language for terminal, render, bridge |
| Android UI | Kotlin + Jetpack Compose | minSdk 33 / targetSdk 34 / compileSdk 37 | UI layer, TextureView, JNI client |
| GPU | wgpu 30 | Vulkan-only | Graphics API — Vulkan via wgpu, Lavapipe (Linux) / SwiftShader (emulator) |
| VT engine | libghostty-vt | vendored Zig (0.16) | Ghostty's VT5xx+ parser, vendored as dynamic lib |
| Text shaping | cosmic-text 0.19 | | Unicode text shaping and line layout |
| Rasterization | swash 0.2 | | Font rasterization and glyph caching |
| Atlas packing | guillotiere 0.7 | | GPU atlas rectangle packing |
| Font discovery | fontdb 0.23 | | System font database |
| Bridge | JNI (jni crate) | | Kotlin ←→ Rust (no boltffi/JNA) |
| IPC | tower-mcp 0.14 | | MCP protocol over Unix socket + stdio |
| Async runtime | tokio 1 | | MCP listener (axum + tower) |
| HTTP | axum 0.8 | | HTTP transport for MCP |
| PTY I/O | nix 0.31 | | PTY master/slave, poll/read/write |
| IPC channels | flume 0.12 | | Lock-free channels: PTY → CellData, events |
| Serialization | bytemuck | | Zero-copy CellData (80B Pod) between Rust → GPU |
| Error handling | thiserror 2 | | Error type derivation |
| Schema derivation | schemars 1 | | JSON Schema for MCP tools |

## Build & CI

| Tool | Use |
|------|-----|
| Nix (flakes) | Reproducible dev environment, NDK/SDK, Lavapipe |
| Cargo | Rust build |
| Gradle (AGP) | Android APK build |
| cargo ndk | Cross-compilation for Android ABIs |
| cargo test | All Rust tests |
| cargo clippy | Lint (deny warnings) |
| cargo fmt | Format check |
| cargo machete | Unused dependency detection |
| cargo audit | Vulnerability scanning |
| cargo geiger | Unsafe block auditing |
| jscpd | Markdown duplicate detection |

## Key Constraints

- **Rust**: no `anyhow` in libraries; `#![deny(unsafe_code)]` in terminal module
- **Kotlin**: `SharingStarted.WhileSubscribed(TIMEOUT_MILLIS)` with named constant
- **Android**: `applicationId = "com.termux"`; TextureView; `adjustNothing` for IME
- **Scripts**: Nushell only — no bash/sh
- **Build**: `nix develop` only; no `sdkmanager`, no `cargo zigbuild`
- **Signing**: AOSP testkey (`android/app/aosp-testkey.p12`); self-signing forbidden

---

## 1. Dependency Management

### 1.1 Rust Dependencies

- Managed via Cargo workspace (`Cargo.toml` — `[workspace.dependencies]`)
- All shared dependencies declared in `[workspace.dependencies]` with consistent versions
- Pinned via `Cargo.lock` (committed to git)
- Dependency order: single `native` crate (see `docs/architecture.md#51-crate-structure`):

  ```text
  libghostty-vt / libghostty-vt-sys
      ↑
  native (merged: terminal + render + android + mcp)
      ↑
  exec-bin / integration-tests
      ↑
  android/app (Kotlin + Compose)
  ```

- No crate boundary violations possible — all code in one `native/` crate
- Upstream `libghostty-vt` / `libghostty-vt-sys` pinned at crates.io `0.2.1` in `[workspace.dependencies]` (no git patches)
- Key native crate (`Cargo.toml`) dependencies:
  - **GPU**: `wgpu 30` (Vulkan/GLES), `guillotiere 0.7` (atlas packing)
  - **Font**: `cosmic-text 0.19` (text shaping/layout), `swash 0.2` (glyph rasterization), `fontdb 0.23` (font discovery)
  - **Bridge**: `jni 0.21` (direct JNI), `bytemuck 1` (zero-copy CellData)
  - **IPC**: `tower-mcp 0.14` (MCP protocol), `axum 0.8` (HTTP), `tokio 1` (async), `schemars 1` (JSON Schema)
  - **Terminal**: `libc 0.2` (PTY), `nix 0.31` (POSIX), `flume 0.12` (cell channel)
  - **Utilities**: `serde 1`, `thiserror 2`, `lru 0.18`, `strsim 0.11`（fuzzy search 编辑距离，替换手写 levenshtein；修饰键位标志直接使用 libghostty-vt 的 `Mods` 类型，未引入 `bitflags` crate）
  - **Dev/test**: `proptest 1.11`

### 1.2 Nix Dependencies

- Build environment and all tools declared via `flake.nix` devShell
- `flake.lock` pinned (committed to git) for reproducible development environments
- Inputs: `nixpkgs`, `flake-parts`, `fenix` (Rust toolchain)
- All lint and audit tools (cargo-audit, cargo-machete, clippy, etc.) declared as devShell packages

### 1.3 Android Dependencies

- Gradle-managed via `android/build.gradle.kts` (root) and `android/app/build.gradle.kts` (app module)
- Kotlin + Jetpack Compose UI with standard AndroidX libraries:
  - `androidx.compose:compose-bom:2026.06.01` (Compose Bill of Materials)
  - `androidx.core:core-ktx`, `lifecycle-runtime-ktx`, `activity-compose`
  - `androidx.compose.ui`, `ui-graphics`, `material3`, `material-icons-extended`
  - `androidx.navigation:navigation-compose`, `androidx.datastore:datastore-preferences`
- JSON: `kotlinx-serialization-json:1.11.0`（JNI 结构化载荷唯一方案；`FontInfoDto`/`PollEvent` 判别式对齐 Rust serde）
  - Moshi 曾短暂引入（FontMetadata 脚手架）后移除：生产代码零引用，JSON 全走 kotlinx-serialization，删除两行依赖减少 KSP 编译
- Dependency injection: `com.google.dagger:hilt-android:2.60.1` with KSP compiler
- Direct JNI (no JNA or boltffi)
- MCP / IPC: `tower-mcp 0.14` (MCP protocol), `axum 0.8` (HTTP), `tokio 1` (async), `schemars 1` (JSON Schema)
- Test frameworks: JUnit 4/5, MockK, Turbine, Robolectric, Roborazzi, Cucumber, Espresso, UI Automator, ArchUnit, Stove

## 2. Vulnerability Scanning

- [`cargo-audit`] scans Rust crate dependencies for known security vulnerabilities
- Runs in CI via `integration-tests/tests/tool_lint.rs` (`cargo_audit_finds_no_vulnerabilities` test)
- Also invoked in `scripts/check-rust.nu` as part of the full CI pipeline
- `cargo-deny` is intentionally **not** configured for this project:
  - The `deny_toml_must_not_exist` test in `tool_lint.rs` asserts that no `deny.toml` file exists in the repository
  - Per project policy documented in `docs/architecture.md#9-architecture-decisions`: existing CI infrastructure uses `cargo-audit`, and build determinism via Nix flake pinning ensures audit consistency across environments
  - `cargo-deny` is present in `flake.nix` devShell packages (for ad-hoc use) but has no configuration file

## 3. License Compliance

- All dependencies must use OSI-approved open-source licenses
- License checking is done via **manual review** (no automated license scanning tool is configured)
- The project policy explicitly excludes `cargo-deny` configuration, which means no automated `allow-list` or `deny-list` license enforcement
- Rust crate licenses are verified during dependency upgrades by maintainers
- Kotlin/Android library licenses are reviewed via Gradle dependency metadata

## 4. Unused Dependency Detection

- [`cargo-machete`] scans Rust workspaces for declared but unused dependencies
- Runs in CI via `integration-tests/tests/tool_lint.rs` (`cargo_machete_finds_no_unused_deps` test)
- Uses `--skip-target-dir` flag per project convention (avoids false positives from cached build artifacts)
- Do NOT use `--with-metadata` flag — it causes false positives with proc-macro dependencies like `quickcheck` (see AGENTS.md pitfalls)

## 5. Supply Chain

- **Upstream libghostty-vt**: crates.io `0.2.1` pinned in `Cargo.toml` `[workspace.dependencies]`:

  ```toml
  libghostty-vt = "0.2.1"
  libghostty-vt-sys = "0.2.1"
  ```

  Exact versions are locked in `Cargo.lock` for reproducible builds. The
  `libghostty-vt-sys` build script fetches the pinned Ghostty source commit
  (`GHOSTTY_COMMIT` in its `build.rs`) into its `OUT_DIR` cache — no local
  checkout or patching is involved.

- **No vendored crates in tree**: The `vendor/` directory is not used; Ghostty
  source is fetched by the `libghostty-vt-sys` build script into the cargo
  build output directory.

- **Nix flake pinning**: `flake.lock` pins all Nix inputs (`nixpkgs`, `flake-parts`, `fenix`) to specific revisions, providing reproducible development environments across machines.
