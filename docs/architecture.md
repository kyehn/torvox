# Architecture

GPU-accelerated Android terminal emulator using wgpu (Vulkan) for
rendering, Ghostty's vendored VT parser for terminal emulation, and a
Kotlin + Compose UI. Crate dependencies flow strictly one-way bottom-to-top;
violations break the build.

---

## 1. Overview

The system converts PTY output into a rendered terminal display on Android.
Data flows through four layers:

1. **PTY I/O** — reads child process output, writes keyboard input
2. **VT Parsing** — transforms escape-sequence bytes into a structured grid
3. **GPU Rendering** — shapes text, rasterizes glyphs, packs atlases, submits draw
   instances via wgpu/Vulkan
4. **Android Bridge** — transmits terminal snapshots to Kotlin via boltffi,
   receives UI commands via JNA

A sixth crate (`mcp-server`) exposes terminal state to external AI agents over a
JSON-RPC Unix socket (Model Context Protocol).

---

## 2. Module Architecture

### 2.1 Crate Dependency Graph

```
libghostty-vt / libghostty-vt-sys         ← Ghostty VT parser (vendored Zig)
    ↑
terminal-core (no_std, serde + unicode-width)  ← Data model, Grid, Cell, Event
    ↑
terminal-engine (libghostty-vt + nix + flume) ← PTY, VT parse, Session
    ↑
gpu-renderer (wgpu + cosmic-text + swash + guillotiere) ← GPU render
    ↑
android-gui (boltffi + JNA)           ← Kotlin↔Rust bridge
    ↑
android/app (Kotlin + Compose)               ← Android UI
```

This one-way constraint (enforced by `cargo metadata` verification) prevents
circular dependencies and forces clean layering: lower crates know nothing about
higher crates. Each crate uses only types from crates below it in the chain.

| Crate | Location | Dependencies | Key Traits/Structs |
|-------|----------|-------------|-------------------|
| `libghostty-vt-sys` | Vendored Zig | None | Raw FFI bindings to Ghostty VT |
| `libghostty-vt` | Vendored Zig | `libghostty-vt-sys` | `Terminal`, `TerminalOptions`, `key::Encoder` |
| `terminal-core` | `terminal-core/` | None (no_std) | `Cell`, `Grid`, `TerminalConfig`, `TerminalEvent`, `Selection`, `SessionSnapshot` |
| `terminal-engine` | `terminal-engine/` | `terminal-core`, `libghostty-vt` | `Session`, `GhosttyTerminal`, `PtyPair`, `ShellEnv` |
| `gpu-renderer` | `gpu-renderer/` | `terminal-core`, `terminal-engine` | `GpuContext`, `FontPipeline`, `GlyphKey` |
| `android-gui` | `android-gui/` | `terminal-core`, `terminal-engine`, `gpu-renderer` | `BridgeCell`, `TorvoxBridge`, `AndroidSurface` |
| `mcp-server` | `mcp-server/` | `terminal-core` | `SessionStore`, `McpCommand`, `McpServer` |

*Rationale (NFR-012): The chain prevents dependency cycles that would
otherwise appear between session management and rendering. The renderer reads
grid snapshots; the session manager never imports renderer types. See
`docs/srs.md` section 4.3.*

---

### 2.2 terminal-core (Data Model)

**Location:** `terminal-core/src/`

**Attribute:** `#![no_std]`, `#![forbid(unsafe_code)]` (`terminal-core/src/lib.rs`)

The data model crate defines all terminal state types. It has zero runtime
dependencies — only `serde` (optional), `unicode-width`, and `rkyv` (optional)
for serialization.

| Module | File | Primary Types | Purpose |
|--------|------|--------------|---------|
| `cell` | `cell.rs` | `Cell`, `Color`, `Attrs`, `DirtyMask` | Atomic display unit and per-cell attributes |
| `grid` | `grid.rs` | `Grid`, `GridSnapshot` trait | Row-based buffer with scrollback (VecDeque), dirty tracking |
| `line` | `line.rs` | `Line` | A single row of cells |
| `cursor` | `cursor.rs` | `CursorState`, `CursorStyle` | Cursor position, visibility, style (Block/Underline/Bar) |
| `config` | `config.rs` | `TerminalConfig`, `Shell`, `Theme`, `RenderConfig`, `FontConfig` | Runtime configuration |
| `event` | `event.rs` | `TerminalEvent`, `DirtyRegion` | Events emitted by the terminal to the UI |
| `selection` | `selection.rs` | `Selection`, `SelectionMode` (Char/Word/Line/Block) | Text selection with four modes |
| `snapshot` | `snapshot.rs` | `SessionSnapshot` | rkyv-serializable bridge snapshot |
| `terminal` | `terminal.rs` | `TerminalState` | VT protocol state (modes, tab stops, cursor) |
| `vt_types` | `vt_types.rs` | VT-level type aliases | Shared VT enumerations |
| `sgr` | `sgr.rs` | `SgrAttribute` | SGR parameter parsing |
| `csi` | `csi.rs` | CSI parameter types | CSI sequence helpers |
| `ansi` | `ansi.rs` | ANSI constants | Escape code constants |
| `unicode` | `unicode.rs` | Unicode width helpers | CJK width detection |

**Design decision — no_std** (NFR-001): `terminal-core` uses `#![no_std]` with
`extern crate alloc` to remain embeddable in resource-constrained environments
and to enforce discipline about heap allocation. The `std` feature gate enables
`std::error::Error` impls via `thiserror 2` for non-embedded builds. The no_std
attribute reinforces `#![forbid(unsafe_code)]` by limiting the available standard
library surface.

**Design decision — zero unsafe** (NFR-001): `terminal-core` is `#![forbid(unsafe_code)]`.
Every `cargo geiger --package terminal-core` run must report zero unsafe blocks.
This ensures the data model cannot introduce memory corruption.

**Requirement trace:**
- `cell.rs` — `FR-005`
- `grid.rs` — `FR-006`
- `terminal.rs` — `FR-003`
- `event.rs` — `FR-004`
- `snapshot.rs` — `FR-042`
- `config.rs` — `FR-052`
- `cursor.rs` — `FR-007`
- `selection.rs` — `FR-008`

---

### 2.3 terminal-engine (PTY + VT)

**Location:** `terminal-engine/src/`

This crate owns PTY lifecycle, VT parsing, and session orchestration.

| Module | File | Primary Types | Purpose |
|--------|------|--------------|---------|
| `pty` | `pty.rs` | `Pty` trait, `PtyPair`, `PtyError` | PTY master/slave creation; only allowed `fork` unsafe |
| `session` | `session.rs` | `Session` | Wires PTY reader, VT parser, process waiter together |
| `ghostty_terminal` | `ghostty_terminal.rs` | `GhosttyTerminal`, `GridSnapshot`, `SearchMatch` | VT engine wrapper |
| `osc_handler` | `osc_handler.rs` | `OscHandler`, `OscEvent` | OSC sequence interpreter (clipboard, cwd, title) |
| `shell_env` | `shell_env.rs` | `ShellEnv` | Pre-exec environment setup (TERM, PATH, locale) |
| `action_parser` | `action_parser.rs` | Action parser types | Post-VT action dispatch |
| `sgr_parser` | `sgr_parser.rs` | SGR parser | SGR sequence parser |
| `cursor_cmds` | `cursor_cmds.rs` | Cursor command types | Cursor movement commands |
| `snapshot_test` | `snapshot_test.rs` | Test helpers | Snapshot-based integration tests |
| `vt_conformance` | `vt_conformance.rs` | VT conformance tests | xterm conformance verification |

**Session structure** (`session.rs`):
`Session` holds:
- `pty: Box<dyn Pty>` — abstracted PTY (real or mock)
- `terminal: GhosttyTerminal` — VT engine wrapper
- `osc_handler: OscHandler` — OSC sequence handler
- `output_tx` / `output_rx` — flume channel for PTY→renderer data
- `exited: AtomicBool` — child process exit flag
- `reader_handle` / `wait_handle` — spawned thread join handles

**Requirement trace:**
- `session.rs` — `FR-010`, `FR-041`
- `ghostty_terminal.rs` — `FR-002`, `FR-033`

**Design decision — Ghostty VT parser** (FR-001): Using the vendored Ghostty VT
parser instead of a custom one avoids reimplementing decades of VT
escape-sequence behavior. Ghostty's parser is battle-tested, supports the full
VT5xx+ specification, and provides scrollback access, SGR attributes, DEC mode
control, and keyboard encoding. On Android it links dynamically (`dylib`) with
SONAME stripping in `build.rs` per pitfall #9 in AGENTS.md.

---

### 2.4 gpu-renderer (GPU Pipeline)

**Location:** `gpu-renderer/src/`

**Attribute:** The only rendering path — no CPU/Canvas fallback (`lib.rs`).

| Module | File | Primary Types | Purpose |
|--------|------|--------------|---------|
| `gpu` | `gpu.rs` | `GpuContext`, `CellInstance`, `KgpInstance`, `SearchHighlight` | wgpu pipeline, atlas, instance management |
| `font` | `font.rs` | `FontPipeline`, `GlyphKey`, `GlyphInfo`, `ShapedGlyphInfo`, `FontError` | Text shaping, glyph rasterization, atlas packing |

**Render pipeline sequence:**

```
PTY → flume → GhosttyTerminal → DirtyMask → RenderThread
  → cosmic-text shape + swash glyph rasterize → guillotiere pack
  → wgpu atlas upload → Instance[] → wgpu render_frame → SurfaceView
```

**Pipeline stages in detail:**

1. **Grid snapshot** — `GhosttyTerminal` produces `GridSnapshot` with cell data
   and dirty flags. The `DirtyMask` (`cell.rs` with `partitions: Vec<u64>`)
   identifies changed rows (pitfall #2 in AGENTS.md).

2. **Text shaping** — `cosmic-text` (`FontPipeline`) shapes each visible line's
   characters into positioned glyphs. Cosmic-text handles Unicode, ligatures,
   and complex script shaping via an embedded `FontSystem`.

3. **Glyph rasterization** — `swash` rasterizes each shaped glyph into coverage
   data. Atlas texture uses `R8Unorm` (linear format, no sRGB gamma) because
   glyph coverage data is already in linear space (`gpu-renderer/src/lib.rs`).

4. **Atlas packing** — `guillotiere` packs glyph bitmaps into a 2048×2048 atlas
   texture. Allocation IDs are tracked per `GlyphKey` for eviction and
   deallocation.

5. **Instance construction** — Each visible glyph becomes a `CellInstance`
   (`repr(C)`, `bytemuck::Pod`) containing quad geometry and atlas UV
   coordinates. These are uploaded to a wgpu storage buffer.

6. **Render pass** — wgpu executes the vertex/fragment shader, sampling the
   atlas texture and writing RGBA output. KGP (Kitty Graphics Protocol) images
   use a separate `KgpInstance` path with full RGBA quads.

7. **Presentation** — The rendered frame is submitted to the Vulkan surface
   backed by `ANativeWindow` on Android.

**Key constants** (`gpu.rs`):
- `DESIRED_FRAME_LATENCY: 2` (desktop), `1` (Android)
- `MIN_ATLAS_BUFFER_SIZE: 64`
- `DEFAULT_BG_ALPHA: 0.8`
- `SURFACE_RELEASE_POLL_MS: 50` (Android only)

**Search highlight rendering**: `SearchHighlight` instances are submitted as
overlay quads with configurable background color (`gpu.rs`).

**Design decision — GPU-only (Vulkan via wgpu)** (FR-010, NFR-006, NFR-018): No GL
fallback, no CPU software path. Vulkan is the only rendering API. On Linux,
Mesa Lavapipe provides software Vulkan (`lvp_icd.x86_64.json` configured in
`flake.nix` via `VK_ICD_FILENAMES`). On Android emulators, SwiftShader provides
GPU emulation. This avoids maintaining multiple rendering backends and ensures
consistent behavior across targets.

**Requirement trace:**
- `gpu.rs` — `FR-001`, `FR-015`, `FR-019`
- `font.rs` — `FR-015`

---

### 2.5 android-gui (Android Bridge)

**Location:** `android-gui/src/`

This crate is the **single FFI export location** for the Rust↔Kotlin boundary
(per AGENTS.md — only one `setup_scaffolding!()` call allowed).

| Module | File | Primary Types | Purpose |
|--------|------|--------------|---------|
| `bridge` | `bridge.rs` | `TorvoxBridge`, `BridgeCell`, `BridgeAttrs`, `TerminalConfig`, `TerminalError` | boltffi data bridge — singular export location |
| `jni_bridge` | `jni_bridge.rs` | `Java_io_torvox_bridge_NativeWindow_getNativeWindowPtr` | JNI for ANativeWindow acquisition |
| `surface` | `surface.rs` | `AndroidSurface`, `SurfaceError` | Render pipeline lifecycle against Android Surface |

**Two-way bridge pattern:**

1. **Rust → Kotlin (boltffi)**: Terminal state (grid snapshots, events) flows
   from Rust to Kotlin via `boltffi::data` annotated structs
   (`BridgeCell`, `BridgeAttrs`, etc.). boltffi generates a binary wire format
   that Kotlin deserializes using hand-written `WireReader`/`WireWriter`
   (`TorvoxBridge.kt`). Boltffi has no CLI bridge code generator (pitfall #7),
   so JNA is used for the reverse direction.

2. **Kotlin → Rust (JNA)**: UI commands (input, resize, session management) flow
   from Kotlin to Rust via JNA calls in `TorvoxBridge.kt`. JNA reflection-based
   binding requires ProGuard R8 `-dontoptimize` for release builds (pitfall #14).

**Surface management** (`surface.rs`):
- `ANativeWindow_setBuffersGeometry` configures buffer dimensions/format
- `ANativeWindow_release` releases window on surface destroy
- Generation counter tracks render thread "generations" to prevent stale thread
  interference (pitfall #13)
- After 100 consecutive render errors (~10 seconds), thread exits permanently

**JNI bridge** (`jni_bridge.rs`):
- `Java_io_torvox_bridge_NativeWindow_getNativeWindowPtr` calls the NDK's
  `ANativeWindow_fromSurface` to obtain a native window pointer from a
  `Surface` Java object.

**Design decision — TextureView over SurfaceView** (FR-052):
Current project uses `TextureView` which does not need `setZOrderOnTop`.
The previous SurfaceView approach required `setZOrderOnTop(true)` on SwiftShader
emulators, causing overlay alpha=0 rendering (invisible output). See pitfall #12
in AGENTS.md. TextureView integrates naturally with Compose and handles Android
surface lifecycle recreation events (configuration changes, activity restart).

**Design decision — boltffi + JNA two-way bridge** (FR-049, FR-050):
boltffi generates efficient Rust→Kotlin serialization for bulk grid data.
Kotlin→Rust calls use JNA because boltffi lacks a CLI bridge generator.
The `message` field on boltffi errors is avoided because it conflicts with
Kotlin's `Throwable.message` (pitfall #5).

**Bridge sync discipline** (per AGENTS.md pre-commit checklist):
When `terminal-core` types change, both `android-gui/src/bridge.rs` and
`android/app/src/main/java/io/torvox/bridge/TorvoxBridge.kt` must be updated.
boltffi wire format is position-sensitive with no length prefix or checksum,
so field order and count must match exactly (lesson from memory-bank #01).

**Requirement trace:**
- `bridge.rs` — `FR-042`, `FR-043`, `FR-044`
- `surface.rs` — `FR-044`

---

### 2.6 mcp-server (MCP Server)

**Location:** `mcp-server/src/`

**Attribute:** `#![forbid(unsafe_code)]`

Model Context Protocol server exposing terminal sessions to AI agents via
JSON-RPC 2.0 over a Unix domain socket.

**Architecture:**

```
AI Agent  <--stdio/JSON-RPC-->  mcp-server  <--Unix socket-->  android-gui
```

| Type | Location | Purpose |
|------|----------|---------|
| `SessionStore` trait | `lib.rs:265` | Abstraction over session storage |
| `McpCommand` enum | `lib.rs:226` | 6 command variants: Read, Write, Signal, SetTerminalSize, WriteClipboard, RaiseNotification |
| `ReadRequest` enum | `lib.rs:145` | 11 read types: Sessions, Grid, Scrollback, Cursor, Selection, Title, Search, Cwd, DirEntry, FileContent, ProcessInfo |
| `ReadResponse` enum | `lib.rs:183` | Response variants matching read types |
| `McpServer` | `lib.rs:464` | Request handler, tool dispatch |
| `SignalKind` | `lib.rs:256` | Interrupt, Terminate, Hangup, Quit |
| `serve_unix()` | `lib.rs:1121` | Creates listener, accepts connections, dispatches |

**Exposed tools (~21):**
- `list_sessions`, `read_grid`, `read_scrollback`, `read_cursor`, `read_selection`,
  `read_title`, `send_input`, `send_signal`, `set_terminal_size`, etc.

**Design** (FR-044, FR-045): The MCP server enables AI coding agents to inspect
terminal state during development sessions. The `SessionStore` trait allows a `NoOpStore`
implementation (returns empty data) when no GUI is connected, or a real
implementation backed by `android-gui`'s session registry.

---

### 2.7 Android App (Kotlin + Compose)

**Location:** `android/app/`

**Package:** `io.term` (`applicationId = "com.termux"` — intentional, pitfall #16)

Kotlin + Jetpack Compose UI. Key files:

| File | Purpose |
|------|---------|
| `TorvoxBridge.kt` | JNA bindings + boltffi wire format reader/writer |
| Compose UI layer | Terminal screen, settings, session management |

The `TorvoxBridge.kt` file is the Kotlin side of the bridge, containing:
- `WireReader` / `WireWriter` — manual boltffi wire format parsing
- JNA interface declarations for Rust functions
- Hand-rolled serialization for `BridgeCell`, `BridgeAttrs`, etc.

---

## 3. Data Flow

### 3.1 Terminal Output Path

```
┌──────────┐   poll read    ┌──────────────┐  flume  ┌───────────────────┐
│ PTY      │ ──────────────►│ PTY Reader   │ ──────► │ GhosttyTerminal   │
│ Master   │    read(8192)  │ (dedicated   │         │ (VT parser, same  │
│ (child)  │               │  thread)     │         │  thread)          │
└──────────┘               └──────────────┘         └────────┬──────────┘
                                                             │
                                                             │ Grid
                                                             ▼
┌──────────────────────────────────────────────────────────────────────┐
│ GhosttyTerminal                                                      │
│ 1. feed() — pushes bytes into Ghostty VT parser                     │
│ 2. Parser writes to internal grid (rows × cols)                     │
│ 3. GhosttyTerminal.grid() returns snapshot                          │
│ 4. DirtyMask tracks changed rows via Vec<u64> partitions            │
└──────────────────────────────────────────────────────────────────────┘
                                                             │
                                                    GridSnapshot
                                                             │
                                                             ▼
┌──────────────────────────────────────────────────────────────────────┐
│ RenderThread (CountDownLatch wake)                                   │
│ 1. Read GridSnapshot from GhosttyTerminal                           │
│ 2. cosmic-text: shape each visible line → positioned glyphs         │
│ 3. swash: rasterize each glyph → coverage data                      │
│ 4. guillotiere: pack into 2048×2048 R8Unorm atlas                  │
│ 5. Upload atlas texture (+ dirty regions only) to GPU               │
│ 6. Build CellInstance[] buffer for visible glyphs                   │
│ 7. wgpu render pass → Vulkan → ANativeWindow → SurfaceView         │
└──────────────────────────────────────────────────────────────────────┘
```

*Reference: AGENTS.md "Render Pipeline" section, `ghostty_terminal.rs`,
`gpu-renderer/src/lib.rs`.*

### 3.2 Input Path

```
┌──────────┐  JNA call   ┌────────────────┐  write(pty_fd)  ┌──────────┐
│ Kotlin   │ ──────────► │ Input Writer   │ ──────────────► │ PTY      │
│ (IME)    │             │ (separate      │                 │ Master   │
│          │             │  write path)   │                 │ (child)  │
└──────────┘             └────────────────┘                 └──────────┘
```

Keyboard input follows the Kitty keyboard protocol for modifier encoding.
The input path is separate from the PTY reader thread to avoid contention.
Key events flow:
- Android IME → JNA bridge (`TorvoxBridge.kt`) → Rust `Session::write_input()`
- Terminal resize → JNA bridge → Rust `Session::resize()`
- Signal (SIGINT, etc.) → JNA bridge → Rust `Session::signal()`

### 3.3 Bridge Data Flow

```
┌───────────────┐   boltffi serialization   ┌───────────────────────┐
│ Rust: bridge  │ ────────────────────────► │ Kotlin: TorvoxBridge  │
│ GridSnapshot  │    binary wire format     │ .kt deserialization    │
│ TerminalEvent │                           │ (WireReader/WireWriter)│
└───────────────┘                           └───────────────────────┘
       ◄─────────────────────────────────────────
       JNA calls (input, resize, config)

┌──────────────────────────────────────────────────────────────────┐
│ ANativeWindow (Vulkan surface)                                    │
│  JNI: Java_io_torvox_bridge_NativeWindow_getNativeWindowPtr      │
│  → ANativeWindow_fromSurface → raw pointer → Rust GpuContext      │
└──────────────────────────────────────────────────────────────────┘
```

*Reference: `android-gui/src/bridge.rs`, `jni_bridge.rs`,
`android/app/src/main/java/io/torvox/bridge/TorvoxBridge.kt`.*

---

## 4. Thread Model

Each terminal session creates 6–7 threads:

| Thread | Source | Lifespan | Purpose |
|--------|--------|----------|---------|
| **PTY Reader** | `session.rs` | Entire session | Polls PTY with `poll()` (100ms timeout), reads output, feeds GhosttyTerminal |
| **Input Writer** | `session.rs` | Entire session | Writes keyboard input to PTY master (separate write path avoids reader contention) |
| **Process Waiter** | `session.rs` | Until child exits | `waitpid()` on child process; exits after child terminates |
| **RenderThread** | `surface.rs` | While surface alive | CountDownLatch-woken loop: reads grid snapshot, shapes, rasterizes, submits GPU frame |
| **VT Parser** | `ghostty_terminal.rs` | Entire session | Same thread as PTY Reader — Ghostty parser runs inline |
| **MCP Listener** | `mcp-server` | Server lifetime | Accepts Unix socket connections, dispatches JSON-RPC requests |
| **MCP Worker** | `mcp-server` | Per connection | Handles an individual MCP client session |

```
┌─────────────────────────────────────────────────────────────────┐
│                     Session                                     │
│                                                                 │
│  ┌──────────────┐    flume     ┌──────────────────┐            │
│  │ PTY Reader   │──────────────► GhosttyTerminal  │            │
│  │ (poll/read)  │              │ (VT parser,      │            │
│  │              │              │  same thread)     │            │
│  └──────┬───────┘              └────────┬─────────┘            │
│         │ writes to PTY                │ GridSnapshot          │
│         ▼                              ▼                       │
│  ┌──────────────┐            ┌──────────────────┐             │
│  │ Input Writer │            │ RenderThread     │             │
│  │ (JNA calls)  │            │ (CountDownLatch  │             │
│  └──────────────┘            │  → wgpu → Surface)             │
│                              └──────────────────┘             │
│  ┌──────────────┐                                             │
│  │ Process      │  waitpid() → sets exited flag               │
│  │ Waiter       │  (exits after child terminates)              │
│  └──────────────┘                                             │
└─────────────────────────────────────────────────────────────────┘
```

**Thread lifecycle rules:**
- PTY Reader and Render Thread are always active during a session
- Process Waiter exits when child process terminates
- Render Thread exits after 100 consecutive errors (~10 seconds, pitfall #13);
  must be restarted on new surface via generation counter
- MCP listener thread is per-server, not per-session

**Synchronization primitives:**
- `flume::bounded` channel: PTY Reader → Render Thread (grid snapshots)
- `CountDownLatch`: Render Thread wake from Kotlin
- `AtomicBool`: exit flags, notification triggers
- `Arc<Mutex<Option<String>>>`: clipboard, cwd, notification text
- `Arc<(Mutex<bool>, Condvar)>`: output notification

*Reference: AGENTS.md "Thread Model", `session.rs`, `surface.rs`.*

---

## 5. Design Decisions

### 5.1 GPU-only with wgpu (Vulkan)

**Decision:** No GL fallback, no CPU software rendering path. Vulkan everywhere.
**Rationale** (FR-010, NFR-006, NFR-018):
- Single rendering backend simplifies maintenance across Linux, Android, and
  emulator targets.
- Vulkan provides explicit control over GPU resources (memory, barriers,
  command buffers) essential for low-latency terminal rendering.
- Mesa Lavapipe provides Vulkan on headless/dev Linux environments.
- Android emulators use SwiftShader for Vulkan GPU emulation.
- Avoids the complexity of maintaining multiple rendering paths (GL vs Vulkan).
**Implementation:** `gpu-renderer/src/gpu.rs` — wgpu `Instance`, `Surface`,
`Device`, `Queue`, `RenderPipeline`.

### 5.2 Ghostty VT Parser

**Decision:** Vendored Ghostty VT parser (`libghostty-vt` / `libghostty-vt-sys`)
instead of a custom VT parser.
**Rationale** (FR-001):
- Ghostty's parser implements the full VT5xx+ specification with extensive
  real-world testing.
- Avoids reimplementing hundreds of escape sequences, DEC modes, OSC commands,
  and keyboard protocols (Kitty keyboard protocol).
- Provides a clean C API via Zig compilation, wrapped in Rust.
- Android linking uses dynamic (`dylib`) with SONAME stripping in `build.rs`;
  static linking fails because the Zig install archive contains only `lib_vt.o`
  (pitfall #9).
**Implementation:** `terminal-engine/src/ghostty_terminal.rs` wraps
`libghostty_vt::Terminal` with Rust-safe access.

### 5.3 no_std for terminal-core

**Decision:** `#![no_std]` with `extern crate alloc`.
**Rationale** (NFR-001):
- Enforces heap allocation discipline — all allocs go through explicit `Vec`,
  `String`, `VecDeque` from `alloc`.
- Keeps the data model embeddable in constrained environments (e.g., kernel
  debugging, bare-metal).
- `thiserror 2` with optional `std` feature enables `Error` trait impls when
  needed.
- Zero `unsafe` (`#![forbid(unsafe_code)]`) is reinforced by the limited no_std
  surface, which excludes potentially unsafe std APIs.
**Implementation:** `terminal-core/src/lib.rs` line 1: `#![no_std]`

### 5.4 One-way Crate Dependency

**Decision:** Strict one-way dependency chain (lower crates never import higher
ones). Violations break the build.
**Rationale** (NFR-012):
- Prevents circular dependencies between session management and rendering.
- Forces clean layering: `terminal-core` (data model) → `terminal-engine` (PTY) →
  `gpu-renderer` (GPU) → `android-gui` (bridge).
- Each crate has a single responsibility with clear boundaries.
- Verified via `cargo metadata --no-deps --format-version 1`.
**Implementation:** Cargo.toml dependency declarations; `AGENTS.md` documents
the graph; build CI enforces it.

### 5.5 boltffi + JNA Bridge

**Decision:** Two-way bridge: boltffi for Rust→Kotlin (grid data), JNA for
Kotlin→Rust (commands).
**Rationale** (FR-049, FR-050):
- boltffi efficiently serializes bulk terminal grid data (thousands of cells)
  into a compact binary format for Kotlin consumption (FR-049).
- JNA handles Kotlin→Rust calls because boltffi lacks a CLI bridge generator
  (pitfall #7).
- rkyv serialization provides zero-copy snapshots for grid/cursor/selection
  state synchronization to the Kotlin layer (FR-050).
- ProGuard R8 needs `-dontoptimize` for JNA reflection-based binding on release
  builds (pitfall #14).
- The `message` field on boltffi Error types is avoided — it conflicts with
  Kotlin `Throwable.message` (pitfall #5).
**Implementation:** `android-gui/src/bridge.rs` (single export location),
`TorvoxBridge.kt` (JNA bindings + wire format reader/writer).

### 5.6 6–7 Thread Model Per Session

**Decision:** Dedicated threads for PTY reading, VT parsing, input writing,
process waiting, and rendering.
**Rationale** (NFR-009, FR-027, FR-028):
- PTY reader and VT parser on the same thread avoids cross-thread state
  synchronization for the terminal grid.
- Input writer on a separate thread prevents keyboard input from being blocked
  by PTY output processing.
- Process waiter is isolated — exits cleanly after child terminates without
  affecting other threads.
- Render thread has its own lifecycle managed via CountDownLatch, generation
  counter, and error thresholds.
**Implementation:** `session.rs` (spawns reader, waiter), `surface.rs` (render
thread).

### 5.7 cargo-audit over cargo-deny

**Decision:** Use `cargo-audit` for security vulnerability scanning; do not use
`cargo-deny`.
**Rationale** (NFR-019): Existing project infrastructure and CI scripts use
`cargo-audit`. `cargo-deny` was not chosen because license/duplicate checking is
handled by other tools. Build determinism via Nix flake pinning ensures audit
consistency across environments.
**Implementation:** Rust CI script (`check-rust.nu`) runs `cargo audit`.

### 5.8 TextureView over SurfaceView

**Decision:** Use `TextureView` for the terminal display surface.
**Rationale** (FR-052): TextureView does not require `setZOrderOnTop`. The
previous SurfaceView approach needed `setZOrderOnTop(true)` on SwiftShader
emulators, causing overlay alpha=0 and invisible output (pitfall #12).
TextureView integrates naturally with Compose's layout system and handles
Android surface lifecycle events (configuration changes, activity restart)
for wgpu pipeline recreation.
**Implementation:** `android/app` Kotlin UI layer uses `TextureView` as the
rendering surface target.

---

## 6. Error Handling Strategy

### 6.1 Error Type Convention

- **Library crates** (`terminal-core`, `terminal-engine`, `gpu-renderer`): use
  `thiserror 2` for `Error`/`Display` derives. `anyhow` is forbidden in library
  crates (AGENTS.md "Never" section).
- **Binary crates** (`mcp-server`): may use `anyhow` in `main.rs` but library
  modules use `thiserror`.
- **terminal-core**: errors use no_std-compatible patterns. The `std` feature
  enables `std::error::Error` impls.

### 6.2 Error Propagation

```
┌─────────────────────────────────────────────────────────────┐
│ Layer           │ Error Type        │ Handling              │
├─────────────────┼───────────────────┼───────────────────────┤
│ terminal-core     │ No custom error   │ Returns Option/Result │
│                 │ (no_std)          │ from public API       │
├─────────────────┼───────────────────┼───────────────────────┤
│ terminal-engine │ SessionError      │ Propagated to session │
│                 │ PtyError          │ orchestrator          │
├─────────────────┼───────────────────┼───────────────────────┤
│ gpu-renderer │ GpuError          │ Logged; render thread │
│                 │ FontError         │ retries (up to 100    │
│                 │                   │ consecutive failures) │
├─────────────────┼───────────────────┼───────────────────────┤
│ torvox-gui-and  │ SurfaceError      │ Maps to Kotlin string │
│ roid            │ TerminalError     │ via boltffi           │
├─────────────────┼───────────────────┼───────────────────────┤
│ mcp-server      │ McpError          │ Returns JSON-RPC      │
│                 │                   │ error response        │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Recovery Strategies

| Failure Mode | Detection | Recovery |
|-------------|-----------|----------|
| Render thread error | 100-consecutive-error counter | Thread exits; generation counter triggers restart on new surface |
| GPU surface lost | wgpu error callback (`log_gpu_error`) | Surface recreation via Android lifecycle callback |
| PTY read error | `poll()`/`read()` returns error | Session terminates; process exited event sent to UI |
| PTY write error | `write()` returns error | Logged; input silently dropped |
| MCP invalid request | JSON parse error | Returns JSON-RPC error response, continues serving |

### 6.4 Safety

- **terminal-core**: zero `unsafe` (`#![forbid(unsafe_code)]`)
- **mcp-server**: zero `unsafe` (`#![forbid(unsafe_code)]`)
- All other crates: `unsafe` blocks annotated with `// SAFETY:` comments
  explaining invariants (AGENTS.md requirement)
- `PtyPair` in `pty.rs` is the only location where `fork()` is called with
  explicit safety documentation

---

## 7. Testing Strategy

*Full details in `docs/standards/TESTING.md`.*

### Unit Tests
- Crate-level tests in each source file (`#[cfg(test)] mod tests { ... }`)
- Property tests: `terminal-core` property tests in `tests/property_tests.rs`

### Integration Tests
- `terminal-engine/tests/xterm_conformance.rs` — xterm conformance spec
- `terminal-engine/src/vt_conformance.rs` — VT protocol conformance
- `terminal-engine/src/snapshot_test.rs` — snapshot-based integration tests
- `android-gui` tests via `cargo test --package android-gui`

### Fuzzing
- 7 cargo-fuzz targets in `fuzz/fuzz_targets/`:
  `fuzz_attrs`, `fuzz_grid_ops`, `fuzz_grid_resize`, `fuzz_osc_handler`,
  `fuzz_osc_parse`, `fuzz_selection`, `fuzz_vt_parser`

### Pre-commit Quality Gate
```bash
cargo nextest run --workspace --profile ci
cargo clippy --all -- --deny warnings
cargo fmt --check
cargo geiger --package terminal-core  # zero unsafe
nu scripts/check-rust.nu
```

Bridge changes additionally require `TorvoxBridge.kt` sync verification
and `cargo test --package android-gui`.

*Reference: `docs/standards/QUALITY-GATE.md`.*
