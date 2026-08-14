# Architecture

> **arc42.** This document follows the [arc42](https://arc42.org) architecture
> documentation template (12 sections). It supersedes the earlier free-form
> architecture document; all content was migrated, and cross-references in
> `docs/traceability.yml` and `docs/dependencies.md` were updated to the new
> section anchors. Supporting material: ADRs in `docs/adr/`, requirements in
> `docs/requirements/*.sdoc` (StrictDoc), acceptance criteria in
> `docs/acceptance.md`. Terminology: `docs/glossary.md`.

---

## 1. Introduction and Goals

GPU-accelerated Android terminal emulator using wgpu (Vulkan) for rendering,
Ghostty's vendored VT parser for terminal emulation, and a Kotlin + Compose UI.
All Rust code lives in a single `native` crate; the Kotlin side communicates
via JNI (no boltffi/JNA).

The system converts PTY output into a rendered terminal display on Android.
Data flows through four layers:

1. **PTY I/O** — reads child process output, writes keyboard input
2. **VT Parsing** — transforms escape-sequence bytes into a structured grid
3. **GPU Rendering** — shapes text, rasterizes glyphs, packs atlases, submits draw
   instances via wgpu/Vulkan
4. **Android Bridge** — JNI functions expose terminal state and commands to Kotlin

An embedded **MCP module** provides JSON-RPC 2.0 over Unix socket or stdio for
AI agent integration.

### 1.1 Requirements Overview

Functional requirements are maintained in StrictDoc
(`docs/requirements/functional_requirements.sdoc`, prefix FR-, 63 requirements)
with prose in `docs/srs.md`. Non-functional requirements live in
`docs/requirements/non_functional_requirements.sdoc` (prefix NFR-, 25
requirements). The traceability matrix is in `docs/traceability.yml`, and
per-requirement acceptance criteria in `docs/acceptance.md`.

Requirement families: terminal emulation (FR-001..FR-009), rendering pipeline,
input handling, session management, OSC handling, clipboard and notifications,
SSH/Mosh connectivity, MCP server integration, Android bridge, configuration
and themes.

### 1.2 Quality Goals

Top quality goals, in priority order (see §10 for measurable scenarios):

| Goal | Priority | Rationale |
|------|----------|-----------|
| **Correctness** | 1 | Ghostty parser as single source of truth; zero `unsafe` in terminal path; deterministic behavior across platforms |
| **Performance** | 2 | GPU-only rendering (no CPU fallback), bounded threads, low-latency input path |
| **Reliability** | 3 | Thread panics must not take down the process; graceful surface loss recovery |
| **Maintainability** | 4 | Single `native/` crate, named constants, no abbreviations, `thiserror 2` error types |
| **Compatibility** | 5 | Termux ecosystem (`com.termux` package name), xterm-compatible VT, Android NDK/Compose baseline |

### 1.3 Stakeholders

| Role/Name | Contact | Expectations |
|-----------|---------|--------------|
| End user | Termux/SSH users | Works like the Termux terminal: fast, correct VT behavior, GPU-rendered text |
| AI coding agent | Codex CLI, OpenCode, etc. | MCP server over stdio/Unix socket with terminal state tools |
| Maintainer | `jane` (single-owner project) | Clean architecture, deterministic Nix builds, low maintenance burden |
| CI | GitHub Actions | Fast, deterministic checks (Rust + Android Gradle), no flaky GPU tests |

---

## 2. Architecture Constraints

| # | Constraint | Origin |
|---|------------|--------|
| 1 | Zero `unsafe` in the terminal data path (ghostty_terminal internals); `pty.rs` and FFI may use `unsafe`, each block anchored with a `// SAFETY:` comment | NFR-001/002 |
| 2 | No `anyhow` in library crates — `thiserror 2` only | NFR-004 |
| 3 | GPU-only rendering: no `Canvas.drawText` per cell, no CPU/software fallback | NFR-006 |
| 4 | One-way, acyclic crate dependency graph, strictly layered | srs §2.4 |
| 5 | Bounded threads: 4 threads per session (PTY reader, input writer, process waiter, render thread) | srs §2.4 |
| 6 | Android NDK minimum API level compatible with SurfaceView and Vulkan | srs §2.4 |
| 7 | Package name `com.termux` — Termux add-on compatibility, do not change | charter |
| 8 | AOSP testkey only (self-signing forbidden); no JNA reflection binding; no boltffi | charter |
| 9 | Naming: full words only, no abbreviations; no magic numbers — named constants | STYLE.md |
| 10 | Scripts are Nushell only (no bash/sh); `.github/` and `scripts/` are read-only, bounded file sets | AGENTS.md |
| 11 | No Java files (Kotlin only); no `portable-pty`, `rust-android-gradle`, `bincode` | AGENTS.md |
| 12 | MCP embedded in `native/` (no standalone server crate) | ADR-0005 |
| 13 | Deterministic Nix builds (no `sdkmanager`, no runtime discovery) | charter |

---

## 3. Context and Scope

### 3.1 Business Context

The system is a standalone Android app that emulates a terminal attached to a
child shell process. It is also a service provider for AI coding agents and a
drop-in companion for the Termux ecosystem.

| Communication partner | Inputs (to system) | Outputs (from system) |
|-----------------------|--------------------|------------------------|
| User | Touch gestures, keyboard/IME input, clipboard paste | Rendered terminal display, clipboard copy, toasts, notifications |
| Child process (bash/zsh, ssh/mosh) | PTY output (escape sequences) | PTY input (keystrokes, paste, OSC 52) |
| AI coding agent | MCP JSON-RPC requests over Unix socket/stdio | MCP tool results (terminal info, grid snapshot, clipboard, open_url) |
| Android system | Lifecycle events, surface changes, intent (e.g. open URL) | Activity/surface interaction, notifications |
| Filesystem | SSH keys, config, shell env | Terminal background image, session state |

### 3.2 Technical Context

| Channel | Direction | Protocol / Technology | Notes |
|---------|-----------|-----------------------|-------|
| JNI bridge | Kotlin ↔ Rust | `jni` crate, 14 `#[no_mangle]` exports in `native/src/android/ffi.rs` | Synchronous calls; `pollEvent()` returns JSON event batches |
| PTY master | Rust ↔ child | POSIX PTY (`pty.rs`, `nix` crate) | Reader polls with 100ms timeout; separate writer thread |
| GPU surface | Rust ↔ Android | `ANativeWindow` → Vulkan via wgpu | Surface handle extracted on JNI thread, sent to render thread |
| MCP | Rust ↔ agent | JSON-RPC 2.0 over Unix socket or stdio (tower-mcp, axum, tokio) | Per-server listener thread, not per-session |
| Clipboard | Kotlin ↔ Android | Android ClipboardManager + OSC 52 | OSC 52 verbatim passthrough via JNI |
| Logging | Rust ↔ system | `log` + Android logcat / panic hook | See §8.4 |

---

## 4. Solution Strategy

The architecture rests on five pillars:

1. **Ghostty as the single source of terminal state.** No parallel data model:
   grid, cells, selection, cursor all come from the vendored Ghostty parser.
   The Rust side never re-implements terminal semantics (ADR-0002).
2. **Direct JNI bridge with zero-copy CellData.** An 80-byte `bytemuck` Pod
   struct crosses FFI without per-cell calls; `GridSnapshot` serves the slow
   query path (ADR-0003).
3. **Single crate.** All Rust in `native/` (cdylib + lib); two thin workspace
   members (`exec-bin`, `integration-tests`) avoid cross-crate boundaries
   (ADR-0001).
4. **GPU-only wgpu/Vulkan rendering.** cosmic-text shaping + swash
   rasterization + guillotiere atlas; software fallbacks (Lavapipe/SwiftShader)
   exist only as *drivers*, not code paths (ADR-0008).
5. **Embedded MCP.** tower-mcp server compiled into the crate serves stdio
   (AI coding agents) and Unix socket (external tools) (ADR-0005).

---

## 5. Building Block View

### 5.1 Crate Structure

All Rust code is a single crate (`native/`):

```text
native/                          ← single cdylib + lib crate
├── src/
│   ├── terminal/                ← PTY, VT parsing (libghostty-vt), Session
│   ├── render/                  ← wgpu pipeline, cosmic-text, swash, guillotiere
│   │   ├── font/                ← font loading, shaping, atlas
│   │   ├── context.rs           ← GpuContext (device, queue, surface config)
│   │   ├── pipeline.rs          ← wgpu shader pipelines (WGSL)
│   │   ├── pass.rs              ← per-frame render pass
│   │   ├── cell_builder.rs      ← CellData → CellInstance → GPU instance construction
│   │   └── surface.rs           ← ANativeWindow surface management
│   ├── android/                 ← JNI FFI exports (no boltffi)
│   ├── mcp.rs                   ← tower-mcp server (Unix socket + stdio)
│   └── lock_util.rs             ← poison recovery
├── shaders/                     ← WGSL shader sources (3 files)
└── Cargo.toml
```

**Split binaries / test harnesses** live in sibling workspace crates:

- `exec-bin/` — standalone terminal for integration testing
- `integration-tests/` — end-to-end render and PTY tests

### 5.2 Module Responsibilities

| Module | What | Key Dependencies |
|--------|------|------------------|
| `terminal/` | PTY master/slave, Ghostty VT wrapper, Session orchestration, keyboard encoding, shell env setup | `libghostty-vt` (vendored Zig), `nix`, `flume` |
| `render/` | wgpu device/surface, cosmic-text shaping, swash rasterization, guillotiere atlas, WGSL pipelines, CellInstance construction | `wgpu`, `cosmic-text`, `swash`, `guillotiere`, `bytemuck` |
| `android/` | 14 JNI `#[no_mangle]` functions: session lifecycle, surface attach/detach, input, polling, dialog result, MCP toggle, persistence | `jni` crate |
| `mcp.rs` | 7 MCP tools via tower-mcp (terminal_info, clipboard_get, clipboard_set, notify, toast, open_url, pick_file) | `tower-mcp`, `axum`, `tokio`, `schemars` |
| `lock_util.rs` | `lock_or_recover()`, `write_or_recover()` — mutex/poison recovery helpers | None |

### 5.3 Dependency Graph

```text
libghostty-vt                    ← Ghostty VT parser (vendored Zig, dynamic link on Android)
    ↑
native                             ← terminal, render, android, mcp (single cdylib)
    ↑
exec-bin / integration-tests     ← thin wrappers
    ↑
android/app                      ← Kotlin + Compose UI
```

Each consumer depends only on crates directly below it. No circular dependencies.
The single-crate design eliminates cross-crate boundary violations entirely.

---

## 6. Runtime View

### 6.1 Render Path (Fast Path)

```text
PTY output
    → poll()/read() [PTY Reader thread]
    → GhosttyTerminal::try_write_to_terminal()
    → CellData (80B bytemuck Pod struct, 0 FFI calls/cell) via flume channel (bounded 256)
    → RenderThread:
        1. build_instances_from_cell_data() → Vec<CellInstance>
        2. wgpu write_buffer (storage)
        3. render_pass()
        4. wgpu submit(render_pass)
        5. surface present
    → ANativeWindow → Vulkan
```

This is the only path for rendering. GridSnapshot is used **only** in the command
path (selection, scrollback queries, OSC handlers).

### 6.2 Query Path (Slow Path)

```text
Kotlin → JNI call (feedPty, writeKey, etc.)
    → SessionRegistry → Session
    → GhosttyTerminal command queue → TakeSnapshot
    → GridSnapshot (CellIterator iteration)
    → String/JSON result → pollEvent response
```

### 6.3 Thread Model

Each terminal session creates 4 threads:

| Thread | Source | Lifespan | Purpose |
|--------|--------|----------|---------|
| **PTY Reader** | `terminal/ghostty_terminal/` | Session | Polls PTY with `poll()` (100ms timeout), reads output, feeds GhosttyTerminal; VT parser runs inline on same thread |
| **Input Writer** | `terminal/ghostty_terminal/` | Session | Writes keyboard input to PTY master (separate write path avoids reader contention) |
| **Process Waiter** | `terminal/ghostty_terminal/` | Until child exits | `waitpid()` on child process; exits after child terminates |
| **Render Thread** | `render/context.rs` | While surface alive | flume-channel woken loop: receives CellData, shapes, rasterizes, submits GPU frame |

The **MCP Listener** is a per-server thread (not per-session) that accepts Unix
socket or stdio connections via axum+tokio.

```text
Session
  PTY Reader ──flume──► GhosttyTerminal ──flume──► Render Thread ──► wgpu
  Input Writer ◄──JNI── Kotlin
  Process Waiter ──waitpid()──► exited flag

MCP Listener (tokio runtime, one per process)
  Unix socket / stdio ──► tower-mcp dispatch ──► snapshot channel
```

**Synchronization:**

- `flume::bounded` channel: PTY Reader → Render Thread (CellData snapshots)
- `Arc<AtomicBool>`: exit flags, notification triggers
- `Arc<Mutex<Vec<Event>>>`: event queue consumed by Kotlin via `pollEvent()`
- `Arc<Mutex<HashMap<u64, Session>>>`: session registry

---

## 7. Deployment View

### 7.1 Infrastructure Level 1

```text
┌────────────────────────────────────────────────────────────────┐
│                      Android App Process                       │
│                                                                │
│  ┌──────────────────┐   JNI calls    ┌──────────────────────┐  │
│  │ Kotlin UI         │◄──────────────►│ Rust native.so       │  │
│  │ (Compose +        │  pollEvent()   │                      │  │
│  │  TextureView)     │  JSON events   │ terminal/  render/  │  │
│  └──────────────────┘                │ android/  mcp.rs     │  │
│         │                            └──────────┬───────────┘  │
│         │ ANativeWindow (Vulkan surface)          │ PTY master  │
│         ▼                                        ▼              │
│  ┌────────────┐                         ┌─────────────┐       │
│  │ GPU (wgpu) │                         │ child proc  │       │
│  └────────────┘                         │ (bash/zsh)  │       │
│                                         └─────────────┘       │
└────────────────────────────────────────────────────────────────┘
```

### 7.2 Infrastructure Level 2

| Element | Description |
|---------|-------------|
| **APK** | Built by Gradle (`android/`), signed with AOSP testkey, package `com.termux` |
| **`libnative.so`** | Rust cdylib; ghostty-vt linked dynamically (SONAME stripped in build.rs); matching `libc++_shared.so` bundled (Zig `std::__1` vs NDK `std::__ndk1`) |
| **`assets/bin/<abi>/exec-bin`** | Compiled standalone binary for SSH/Mosh integration |
| **GPU driver** | Physical Vulkan (device) or SwiftShader (emulator guest); Lavapipe for headless Linux tests via `VK_ICD_FILENAMES` |
| **MCP endpoint** | Unix socket (external tools) or stdio (AI coding agent) |

### 7.3 Mapping

| Artifact | Build command | Deployed to |
|----------|---------------|-------------|
| Rust crate | `nu scripts/build-android-libs.nu` | `android/app/src/main/jniLibs/`, `assets/bin/` |
| APK | `nu scripts/build-apk.nu` (`./gradlew assembleDebug`) | device/emulator via adb |
| Emulator | `nu scripts/setup-emulator.nu` | AVD with SwiftShader GPU |
| E2E suite | `nu scripts/test-emulator.nu` | Emulator (`pm uninstall --user 0 com.termux` first) |

---

## 8. Cross-cutting Concepts

### 8.1 Error Handling

| Failure Mode | Detection | Recovery |
|-------------|-----------|----------|
| Render thread crash | 100-consecutive-error counter | Thread exits; generation counter triggers restart on new surface |
| GPU surface lost | wgpu error callback | Surface recreation via Android lifecycle callback |
| PTY read/write error | `poll()`/`read()`/`write()` returns error | Session terminates; `Event::Terminated` sent to UI |
| MCP invalid request | JSON parse error | Returns JSON-RPC error response, continues serving |

All Rust code uses `thiserror 2` for error types. `anyhow` is forbidden in
library code (see §2 constraint 2). Mutex poisoning is recovered via
`lock_or_recover()` / `write_or_recover()` in `lock_util.rs`.

### 8.2 Concurrency and Synchronization

- 4 threads per session, ownership boundaries fixed by ADR-0004.
- Channels (`flume::bounded`) for streaming; atomics for flags; mutexes for
  shared collections (event queue, session registry).
- Render thread communicates with the rest via a command queue; NDK surface
  functions are never called on the render thread — the surface handle is
  extracted on the JNI thread (`ANativeWindow_fromSurface`) and sent over the
  command queue (pitfall #15).

### 8.3 FFI Bridge

Direct JNI (`jni` crate), no boltffi/JNA (ADR-0003). `CellData` (80B bytemuck)
is the fast-path transfer unit; string/JSON results serve the query path.
JNI signatures in `native/src/android/ffi.rs` must stay in sync with
`NativeBridge.kt` (bridge type sync checklist).

### 8.4 Logging

ADR-0010. Rust uses `log`; panic hook (`log_panics`) reports panics to the
logging backend; Android side surfaces logcat. MCP session logging is
chunked/size-bounded.

### 8.5 Security Model

ADR-0009. Session lifecycle isolation (PDEATHSIG), clipboard proxying via OSC
52 verbatim, `filesDir`-based private storage (no hardcoded `/data/.../files`
paths), AOSP testkey for APK signing, no raw bytes across FFI without bounds.

### 8.6 Build and Dependencies

ADR-0006/0012. Deterministic Nix builds; dependency audit via `cargo-audit`
over `cargo-deny`; `cargo-machete --skip-target-dir` for unused deps; license
review documented in `docs/dependencies.md`.

---

## 9. Architecture Decisions

| # | Decision | Rationale | ADR | Status |
|---|----------|-----------|-----|--------|
| 1 | **Ghostty as single source of truth** | No parallel data model; grid/cell/selection from Ghostty C API | ADR-0002 | ✅ |
| 2 | **JNI direct bridge** (no boltffi/JNA) | 80B CellData via bytemuck = zero-copy FFI; no ProGuard issues | ADR-0003 | ✅ |
| 3 | **Single crate** (no cross-crate boundaries) | Faster compilation, simpler refactoring after boltffi/terminal-core removal | ADR-0001 | ✅ |
| 4 | **GPU-only wgpu/Vulkan** (no GL/CPU fallback) | Consistent across Linux, Android, emulator; Lavapipe/SwiftShader provide SW driver fallback | ADR-0008 | ✅ |
| 5 | **4+1 thread model** | PTY reader + VT parser on one thread avoids grid sync; separate input writer | ADR-0004 | ✅ |
| 6 | **Embedded MCP** (tower-mcp) | ~400 LOC replaces ~2K standalone crate; stdio (AI coding agent) + Unix socket | ADR-0005 | ✅ |
| 7 | **TextureView over SurfaceView** | No `setZOrderOnTop` needed; natural Compose integration | ADR-0003 | ✅ |
| 8 | **cargo-audit over cargo-deny** | Existing infra; license checking handled elsewhere (ADR-0006) | ADR-0006 | ✅ |

See individual ADRs in `docs/adr/` for the full decision context and alternatives
considered. Deferred/rejected alternatives are recorded in
`docs/rejected-technologies.md` (§1 decision registry).

---

## 10. Quality Requirements

### 10.1 Quality Requirements Overview

Full NFRs: `docs/requirements/non_functional_requirements.sdoc` (25
requirements). Summary by category (srs §4):

| Category | Key NFRs | Verification |
|----------|----------|--------------|
| Safety | NFR-001 zero `unsafe` in terminal path; NFR-002 `// SAFETY:` anchors; NFR-003 no panics in error paths | `cargo audit` + code review; tests |
| Performance | NFR-006 GPU-only rendering; NFR-007 glyph atlas ≥ 10,000 entries with eviction | Benchmarks in TESTING.md; emulator baselines |
| Maintainability | NFR-004 `thiserror 2`; naming/lint rules | `cargo clippy --deny warnings`, detekt, spotless |
| Compatibility | Android NDK/Compose baseline; `com.termux` package | Instrumented tests on emulator |
| Reliability | NFR-005 thread panic isolation | Panic-injection tests, session lifecycle tests |

### 10.2 Quality Scenarios

- **Cold start to first frame**: bounded by surface attach + first `pollEvent`
  cycle; measured on emulator baseline in `docs/standards/TESTING.md`
  (Simulator performance baseline).
- **Scrollback of 10k lines**: GridSnapshot query path returns structured
  snapshot; covered by `scrollback_rows` tests.
- **Render thread death**: after 100 consecutive errors (~10s) thread exits;
  restart on new surface (generation counter) — covered by lifecycle tests.
- **GPU loss**: surface recreation via Android lifecycle callback; no crash.

---

## 11. Risks and Technical Debts

Known unresolved items (full registry with evidence in
`docs/rejected-technologies.md` §2.1):

| Item | Status | Notes |
|------|--------|-------|
| MCP `terminal_info` transcript field (D3) | P2 | `screenshot` tool is the substitute |
| `$TMPDIR` cleanup policy (D6) | P2 | TMPDIR set; daily cleanup not urgent |
| Dependency boundary check script (D18) | P2 | Single-crate design keeps boundaries trivial |
| DocumentProvider full CRUD chain (D29) | Partial | delete/create/write need instrumented coverage |
| 480×854/360dp small-screen matrix (D30) | Partial | Layout adapted; full matrix not all run |
| Handle vs ActionMode menu timing (D31) | Partial | `TYPE_FLOATING` + `onGetContentRect` done |
| maestro `suites/` 3 files unreferenced (D27) | Cleanup | Zero references; non-blocking |

Operational pitfalls are documented in AGENTS.md "Known Pitfalls" (e.g. Zig
version first in PATH, ghostty-vt API names, dynamic linking, libc++ namespace,
TextureView z-order, render-thread restart, R8 `-dontoptimize`, emulator-only
device tests, `com.termux` applicationId, AOSP testkey, rapidocr CLI,
Lavapipe config, JNI naming, ANativeWindow thread rule).

---

## 12. Glossary

| Term | Definition |
|------|------------|
| **CSI** | Control Sequence Introducer — escape sequences beginning with `ESC [` |
| **CWD** | Current Working Directory (as reported by the shell via OSC 7) |
| **FFI** | Foreign Function Interface — Rust-to-Kotlin bridging layer |
| **GPU** | Graphics Processing Unit |
| **IME** | Input Method Editor — for composing CJK and other complex text |
| **JNA** | Java Native Access — historically used for Kotlin-to-Rust binding (removed, replaced by direct JNI) |
| **JSON-RPC** | JSON Remote Procedure Call protocol used by the MCP server |
| **Kitty KBP** | Kitty Keyboard Protocol — extended keyboard event encoding |
| **MCP** | Model Context Protocol — a protocol for AI-agent-to-tool communication |
| **NDK** | Android Native Development Kit |
| **OSC** | Operating System Command — escape sequences beginning with `ESC ]` |
| **PTY** | Pseudo-terminal — the kernel device pairing a master and slave endpoint |
| **SSH** | Secure Shell protocol |
| **VT** | Video Terminal — the family of escape-sequence standards |
| **wgpu** | A cross-platform GPU abstraction layer (Rust implementation of WebGPU) |

---

## Appendix: Testing Quick Reference

See `docs/standards/TESTING.md` (full guide) and `docs/standards/QUALITY-GATE.md`
(pre-commit checks).

```text
cargo test --workspace              # all tests
cargo clippy --all -- --deny warnings  # lint
cargo fmt --check                   # format
cargo machete --skip-target-dir     # unused deps
nu scripts/check-rust.nu            # full CI check
```
