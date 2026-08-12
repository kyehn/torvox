# Architecture

GPU-accelerated Android terminal emulator using wgpu (Vulkan) for
rendering, Ghostty's vendored VT parser for terminal emulation, and a
Kotlin + Compose UI. All Rust code lives in a single `native` crate;
the Kotlin side communicates via JNI (no boltffi/JNA).

---

## 1. Overview

The system converts PTY output into a rendered terminal display on Android.
Data flows through four layers:

1. **PTY I/O** — reads child process output, writes keyboard input
2. **VT Parsing** — transforms escape-sequence bytes into a structured grid
3. **GPU Rendering** — shapes text, rasterizes glyphs, packs atlases, submits draw
   instances via wgpu/Vulkan
4. **Android Bridge** — JNI functions expose terminal state and commands to Kotlin

An embedded **MCP module** provides JSON-RPC 2.0 over Unix socket or stdio for
AI agent integration.

---

## 2. Module Architecture

### 2.1 Crate Structure

All Rust code is a single crate (`native/`):

```
native/                          ← single cdylib + lib crate
├── src/
│   ├── terminal/                ← PTY, VT parsing (libghostty-vt), Session
│   ├── render/                  ← wgpu pipeline, cosmic-text, swash, guillotiere
│   │   ├── font/                ← font loading, shaping, atlas
│   │   ├── context.rs           ← GpuContext (device, queue, surface config)
│   │   ├── pipeline.rs          ← wgpu shader pipelines (WGSL)
│   │   ├── pass.rs              ← per-frame render pass
│   │   ├── cell_builder.rs      ← CellData → CellInstance → GPU instance construction
│   │   └── context.rs          ← ANativeWindow surface management (surface.rs inlined)
│   ├── android/                 ← JNI FFI exports (no boltffi)
│   ├── mcp.rs                   ← tower-mcp server (Unix socket + stdio)
│   └── lock_util.rs             ← poison recovery
├── shaders/                     ← WGSL shader sources (3 files)
└── Cargo.toml
```

**Split binaries / test harnesses** live in sibling workspace crates:
- `exec-bin/` — standalone terminal for integration testing
- `integration-tests/` — end-to-end render and PTY tests

### 2.2 Module Responsibilities

| Module | What | Key Dependencies |
|--------|------|------------------|
| `terminal/` | PTY master/slave, Ghostty VT wrapper, Session orchestration, keyboard encoding, shell env setup | `libghostty-vt` (vendored Zig), `nix`, `flume` |
| `render/` | wgpu device/surface, cosmic-text shaping, swash rasterization, guillotiere atlas, WGSL pipelines, CellInstance construction | `wgpu`, `cosmic-text`, `swash`, `guillotiere`, `bytemuck` |
| `android/` | 14 JNI `#[no_mangle]` functions: session lifecycle, surface attach/detach, input, polling, dialog result, MCP toggle, persistence | `jni` crate |
| `mcp.rs` | 7 MCP tools via tower-mcp (terminal_info, clipboard_get, clipboard_set, notify, toast, open_url, pick_file) | `tower-mcp`, `axum`, `tokio`, `schemars` |
| `lock_util.rs` | `lock_or_recover()`, `write_or_recover()` — mutex/poison recovery helpers | None |

### 2.3 Dependency Graph

```
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

## 3. Data Flow

### 3.1 Render Path (fast path — per frame)

```
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

### 3.2 Query Path (slow path — per command)

```
Kotlin → JNI call (feedPty, writeKey, etc.)
    → SessionRegistry → Session
    → GhosttyTerminal command queue → TakeSnapshot
    → GridSnapshot (CellIterator iteration)
    → String/JSON result → pollEvent response
```

### 3.3 Flow diagram

```
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

---

## 4. Thread Model

Each terminal session creates 4 threads:

| Thread | Source | Lifespan | Purpose |
|--------|--------|----------|---------|
| **PTY Reader** | `terminal/ghostty_terminal/` | Session | Polls PTY with `poll()` (100ms timeout), reads output, feeds GhosttyTerminal; VT parser runs inline on same thread |
| **Input Writer** | `terminal/ghostty_terminal/` | Session | Writes keyboard input to PTY master (separate write path avoids reader contention) |
| **Process Waiter** | `terminal/ghostty_terminal/` | Until child exits | `waitpid()` on child process; exits after child terminates |
| **Render Thread** | `render/context.rs` (surface inlined) | While surface alive | flume-channel woken loop: receives CellData, shapes, rasterizes, submits GPU frame |

The **MCP Listener** is a per-server thread (not per-session) that accepts Unix
socket or stdio connections via axum+tokio.

```
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

## 5. Key Design Decisions

| # | Decision | Rationale | ADR | Status |
|---|----------|-----------|-----|--------|
| 1 | **Ghostty as single source of truth** | No parallel data model; grid/cell/selection from Ghostty C API | ADR-0002 | ✅ |
| 2 | **JNI direct bridge** (no boltffi/JNA) | 80B CellData via bytemuck = zero-copy FFI; no ProGuard issues | ADR-0003 | ✅ |
| 3 | **Single crate** (no cross-crate boundaries) | Faster compilation, simpler refactoring after boltffi/terminal-core removal | ADR-0001 | ✅ |
| 4 | **GPU-only wgpu/Vulkan** (no GL/CPU fallback) | Consistent across Linux, Android, emulator; Lavapipe/SwiftShader provide SW fallback | ADR-0008 | ✅ |
| 5 | **4+1 thread model** | PTY reader + VT parser on one thread avoids grid sync; separate input writer | ADR-0004 | ✅ |
| 6 | **Embedded MCP** (tower-mcp) | ~400 LOC replaces ~2K standalone crate; stdio (AI coding agent) + Unix socket | ADR-0005 | ✅ |
| 7 | **TextureView over SurfaceView** | No `setZOrderOnTop` needed; natural Compose integration | ADR-0003, pitfall #12 | ✅ |
| 8 | **cargo-audit over cargo-deny** | Existing infra; license checking handled elsewhere | ADR-0006 | ✅ |

See individual ADRs in `docs/adr/` for the full decision context and alternatives
considered.

---

## 6. Error Handling

| Failure Mode | Detection | Recovery |
|-------------|-----------|----------|
| Render thread crash | 100-consecutive-error counter | Thread exits; generation counter triggers restart on new surface |
| GPU surface lost | wgpu error callback | Surface recreation via Android lifecycle callback |
| PTY read/write error | `poll()`/`read()`/`write()` returns error | Session terminates; `Event::Terminated` sent to UI |
| MCP invalid request | JSON parse error | Returns JSON-RPC error response, continues serving |

All Rust code uses `thiserror 2` for error types. `anyhow` is forbidden in
library code (see AGENTS.md "Never" rules).

---

## 7. Testing

See `docs/standards/TESTING.md` (full guide) and `docs/standards/QUALITY-GATE.md`
(pre-commit checks).

Quick reference:
```
cargo test --workspace              # all tests
cargo clippy --all -- --deny warnings  # lint
cargo fmt --check                   # format
cargo machete --skip-target-dir     # unused deps
nu scripts/check-rust.nu            # full CI check
```
