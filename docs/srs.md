# Software Requirements Specification — Torvox

## 1. Introduction

### 1.1 Purpose

This Software Requirements Specification (SRS) describes the functional and
non-functional requirements for **Torvox**, a GPU-accelerated terminal emulator
for Android. Torvox uses wgpu (Vulkan) for GPU-accelerated rendering, the
Ghostty VT parser for xterm-compatible terminal emulation, and provides a
Kotlin+Compose UI backed by a Rust native library.

The document is intended for developers, testers, and maintainers of the Torvox
project. Requirements are derived exclusively from the existing codebase,
documentation, and build infrastructure. No speculative or unimplemented
features are included.

### 1.2 Scope

Torvox is a terminal emulator application for Android devices. It supports:

- Full xterm–compatible VT escape sequence processing via the Ghostty parser.
- GPU-accelerated rendering via wgpu (Vulkan) with cosmic-text shaping and
  swash glyph rasterization.
- PTY-based process management for running shells and command-line programs.
- Keyboard input encoding including the Kitty keyboard protocol.
- Touch-based interaction (tap, swipe, long-press, selection).
- Clipboard integration (copy/paste via OSC 52).
- SSH/Mosh connectivity via an integrated executable.
- An MCP (Model Context Protocol) server for AI agent integration.
- Session lifecycle management with multiple concurrent sessions.
- Configurable color themes and terminal settings.

The following are out of scope: Java files (except the Kotlin UI), portable-pty
library, bincode serialization, and the rust-android-gradle plugin.

### 1.3 Definitions, Acronyms, and Abbreviations

| Term | Definition |
|------|------------|
| **CSI** | Control Sequence Introducer — escape sequences beginning with `ESC [` |
| **CWD** | Current Working Directory (as reported by the shell via OSC 7) |
| **FFI** | Foreign Function Interface — Rust-to-Kotlin bridging layer |
| **GPU** | Graphics Processing Unit |
| **IME** | Input Method Editor — for composing CJK and other complex text |
| **JNA** | Java Native Access — used for Kotlin-to-Rust binding |
| **JSON-RPC** | JSON Remote Procedure Call protocol used by the MCP server |
| **Kitty KBP** | Kitty Keyboard Protocol — extended keyboard event encoding |
| **MCP** | Model Context Protocol — a protocol for AI-agent-to-tool communication |
| **NDK** | Android Native Development Kit |
| **OSC** | Operating System Command — escape sequences beginning with `ESC ]` |
| **PTY** | Pseudo-terminal — the kernel device pairing a master and slave endpoint |
| **rkyv** | A zero-copy serialization framework used for the Android bridge |
| **SSH** | Secure Shell protocol |
| **VT** | Video Terminal — the family of escape-sequence standards |
| **wgpu** | A cross-platform GPU abstraction layer (Rust implementation of WebGPU) |

### 1.4 References

| Reference | File / Location |
|-----------|-----------------|
| Architecture & Thread Model | [`AGENTS.md`](AGENTS.md) |
| Project Standards (Style) | [`docs/standards/STYLE.md`](docs/standards/STYLE.md) |
| Project Standards (Testing) | [`docs/standards/TESTING.md`](docs/standards/TESTING.md) |
| Project Standards (Quality Gate) | [`docs/standards/QUALITY-GATE.md`](docs/standards/QUALITY-GATE.md) |
| Build System | [`flake.nix`](flake.nix), [`Cargo.toml`](Cargo.toml) |
| Core Data Model | [`torvox-core/src/`](torvox-core/src/) |
| VT / Ghostty Integration | [`torvox-terminal/src/`](torvox-terminal/src/) |
| Renderer (wgpu Pipeline) | [`torvox-renderer/src/`](torvox-renderer/src/) |
| Android Bridge | [`torvox-gui-android/src/`](torvox-gui-android/src/) |
| MCP Server | [`torvox-mcp/src/`](torvox-mcp/src/) |
| SSH/Mosh Executable | [`torvox-exec/src/`](torvox-exec/src/) |
| CI Scripts | [`scripts/`](scripts/) |

---

## 2. Overall Description

### 2.1 Product Perspective

Torvox is an Android application (package name `com.termux`) that provides a
full-featured terminal emulator. It replaces the CPU-based software rendering
path employed by traditional Android terminal emulators with a GPU-accelerated
pipeline via wgpu (Vulkan). The system is decomposed into a set of Rust crates
with strict one-way dependency ordering:

```
libghostty-vt / libghostty-vt-sys
    ↑
torvox-core (no_std, serde + unicode-width)
    ↑
torvox-terminal (libghostty-vt + nix + flume)
    ↑
torvox-renderer (wgpu + cosmic-text + swash + guillotiere)
    ↑
torvox-gui-android (boltffi + JNA)
    ↑
android/app (Kotlin + Compose)
```

Each crate builds on the abstractions of the crate below it. The `torvox-core`
crate is `#![no_std]` and contains the data model (cell, grid, configuration,
selection, events). The terminal crate adds PTY management and VT parsing. The
renderer crate provides the wgpu-based drawing pipeline. The Android bridge
crate exposes Rust functionality to Kotlin via boltffi data types and JNA.

### 2.2 Product Functions

The following high-level functions are provided by Torvox:

- **Terminal Emulation**: Process VT/xterm escape sequences (CSI, OSC, SGR)
  and maintain an in-memory grid of character cells with attributes.
- **PTY Process Management**: Spawn, interact with, and terminate child
  processes (shells, editors, REPLs) via a pseudo-terminal.
- **GPU-Accelerated Rendering**: Render glyphs, backgrounds, cursor, and
  selection highlights using wgpu (Vulkan) with cosmic-text shaping and swash
  rasterization.
- **Keyboard Input**: Encode physical keyboard input using the Kitty Keyboard
  Protocol and route it to the PTY.
- **IME Text Input**: Compose and commit text via Android's Input Method
  Framework, including CJK support.
- **Terminal Selection**: Select text in character, word, line, or block mode;
  copy to clipboard; URL detection and expansion.
- **Scrollback Buffer**: Maintain a bounded scrollback history (50,000 lines by
  default) with search capability.
- **OSC Sequence Handling**: Intercept and handle OSC 7 (CWD), OSC 8
  (hyperlinks), OSC 9/777 (notifications), and OSC 52 (clipboard).
- **SSH/Mosh Connectivity**: Launch SSH and Mosh sessions via the
  `torvox-exec` crate.
- **MCP Server**: Expose terminal session state and control to AI agents via
  JSON-RPC over a Unix socket.
- **Android Bridge**: Synchronize Rust-side terminal state to the Kotlin UI
  layer via boltffi data types and JNA.
- **Session Lifecycle**: Create, resize, and terminate terminal sessions with
  proper cleanup of threads, PTYs, and GPU resources.
- **Render Thread Recovery**: Automatically recover the render thread after
  Android surface destruction (configuration change, activity restart).
- **Color Themes**: Support 16 built-in color themes and custom theme
  definition via TOML.
- **Clipboard Integration**: Read and write the system clipboard from terminal
  sequences (OSC 52) and user interactions.

### 2.3 User Characteristics

The primary users are:

- **Developers and system administrators** who require a capable terminal
  emulator on Android with SSH, Mosh, and full VT escape support.
- **AI agents** (secondary user) that interact with the terminal through the
  MCP server protocol to read state, send input, and inspect output.

### 2.4 Constraints

- **One-way crate dependencies**: The crate dependency graph must be acyclic
  and strictly layered. Violations break the build.
- **`no_std` core**: `torvox-core` must compile in `#![no_std]` environments;
  `std` and `alloc` are gated behind feature flags.
- **Zero `unsafe` in core**: `torvox-core` must contain zero `unsafe` blocks,
  enforced by `cargo geiger`.
- **Android API level**: Must target the Android NDK with minimum API level
  compatible with SurfaceView and Vulkan.
- **Bounded threads**: Each session is limited to 6–7 threads (PTY reader,
  process waiter, render thread, etc.).
- **No `anyhow` in library crates**: Library crates must use `thiserror` for
  error types.
- **No `Canvas.drawText` per cell**: Rendering must not use per-cell software
  text drawing; the GPU pipeline is mandatory.

---

## 3. Functional Requirements

### 3.1 Terminal Emulation

| ID | Requirement | Source |
|----|-------------|--------|
| FR-001 | The system SHALL process VT/xterm escape sequences using the Ghostty parser (`libghostty-vt`). | `AGENTS.md`, `torvox-terminal/src/ghostty_terminal.rs` |
| FR-002 | The system SHALL maintain a terminal grid data model (`Grid`) consisting of rows of cells, each with a character code, foreground/background color, and text attributes (`Attrs`). | `torvox-core/src/grid.rs`, `torvox-core/src/cell.rs` |
| FR-003 | The system SHALL support SGR (Select Graphic Rendition) parameters: bold, dim, italic, underline, double underline, blink, reverse, hidden, strikethrough, overline, and protected. | `torvox-core/src/sgr.rs`, `torvox-core/src/cell.rs` |
| FR-004 | The system SHALL support 16 ANSI color palette indices plus 256-color and truecolor (24-bit RGB) foreground/background specifications. | `torvox-core/src/color.rs`, `torvox-core/src/sgr.rs` |
| FR-005 | The system SHALL support alternate screen buffer mode (SM/RM 1049) for full-screen applications (e.g., vim, less). | `torvox-core/src/grid.rs` (`alt_screen`) |
| FR-006 | The system SHALL support cursor positioning and movement (CUU, CUD, CUF, CUB, CUP, HVP, etc.) and cursor style (block, bar, underline, beam) with visible/hidden state. | `torvox-core/src/cursor.rs`, `torvox-core/src/csi.rs` |
| FR-007 | The system SHALL support scrolling regions (`scroll_up`, `scroll_down`, `insert_lines`, `delete_lines`) with configurable top/bottom boundaries. | `torvox-core/src/grid.rs` |
| FR-008 | The system SHALL support tab stops (set, clear, move). | `torvox-core/src/control.rs` |
| FR-009 | The system SHALL report terminal size changes via `SIGWINCH` to the child process. | `torvox-terminal/src/session.rs` |

### 3.2 Rendering Pipeline

| ID | Requirement | Source |
|----|-------------|--------|
| FR-010 | The system SHALL render the terminal grid using wgpu (Vulkan) as the sole graphics backend. OpenGL and CPU software paths are not supported. | `AGENTS.md`, `torvox-renderer/src/gpu.rs` |
| FR-011 | The system SHALL shape text runs using `cosmic-text` and rasterize glyphs using `swash`, caching results in a GPU atlas. | `torvox-renderer/src/font.rs` |
| FR-012 | The system SHALL pack glyph bitmaps into a GPU texture atlas using `guillotiere` for dynamic rectangle allocation and eviction. | `torvox-renderer/src/font.rs`, `torvox-renderer/src/gpu.rs` |
| FR-013 | The system SHALL maintain a dirty mask (`DirtyMask`) that tracks which rows of the grid have changed and limit rendering to those rows. | `torvox-core/src/cell.rs`, `torvox-core/src/grid.rs` |
| FR-014 | The system SHALL render a cell cursor (block, bar, underline, beam) with configurable color and blink behavior. | `torvox-renderer/src/gpu.rs` |
| FR-015 | The system SHALL render text selection highlights (character, word, line, block modes) as colored overlays on the affected cells. | `torvox-renderer/src/gpu.rs`, `torvox-core/src/selection.rs` |
| FR-016 | The system SHALL support font configuration: family, size, line spacing, and fallback to preferred monospace fonts (Roboto Mono, JetBrains Mono, etc.). | `torvox-core/src/config.rs` (`FontConfig`), `torvox-renderer/src/font.rs` |
| FR-017 | The system SHALL render the terminal background, foreground, and 16-color ANSI palette from the active theme configuration. | `torvox-renderer/src/gpu.rs`, `torvox-core/src/config.rs` (`Theme`) |
| FR-018 | The system SHALL recover from GPU surface destruction (e.g., Android activity restart) by recreating the render pipeline and continuing without data loss. | `AGENTS.md` (Pitfall #13), `torvox-renderer/src/gpu.rs` |
| FR-019 | The system SHALL support the Kitty Graphics Protocol (KGP) for rendering inline images as textured quads. | `torvox-renderer/src/gpu.rs` (`KgpInstance`) |

### 3.3 Input Handling

| ID | Requirement | Source |
|----|-------------|--------|
| FR-020 | The system SHALL encode physical keyboard input using the Kitty Keyboard Protocol (KBP) for extended modifier and key reporting. | `AGENTS.md`, `torvox-terminal/src/keyboard.rs` |
| FR-021 | The system SHALL support IME (Input Method Editor) text input for composing CJK and other complex characters, with `Composing` state management. | `torvox-core/src/terminal.rs` |
| FR-022 | The system SHALL support terminal selection in four modes: character (`Char`), word (`Word`), line (`Line`), and block (`Block`). | `torvox-core/src/selection.rs` (`SelectionMode`) |
| FR-023 | The system SHALL automatically expand word-mode selections to word boundaries and detect URLs (`http://`, `https://`, `ftp://`, `www.`) for URL-aware selection expansion. | `torvox-core/src/selection.rs` (`expand_word`, `expand_url`) |
| FR-024 | The system SHALL support touch input gestures: tap to place cursor, long-press for selection handles, and swipe for scrollback navigation. | `torvox-gui-android/src/surface.rs` |
| FR-025 | The system SHALL support configurable backspace mode (DEL `0x7f` or BS `0x08`) and right-Alt mode (character modifier or meta). | `torvox-core/src/config.rs` (`BackspaceMode`, `RightAltMode`) |

### 3.4 Session Management

| ID | Requirement | Source |
|----|-------------|--------|
| FR-026 | The system SHALL spawn a child process (shell or custom executable) connected to a pseudo-terminal (PTY) via `fork/exec`. | `torvox-terminal/src/pty.rs` |
| FR-027 | The system SHALL read PTY output on a dedicated reader thread and forward parsed output to the grid update pipeline via a `flume` channel. | `torvox-terminal/src/session.rs`, `AGENTS.md` |
| FR-028 | The system SHALL wait for child process exit on a dedicated waiter thread and emit a `ProcessExited` event on termination. | `torvox-terminal/src/session.rs`, `AGENTS.md` |
| FR-029 | The system SHALL support resizing a terminal session (changing rows and columns) and forwarding the new size to the child process via `SIGWINCH`. | `torvox-core/src/grid.rs` (`resize`), `torvox-terminal/src/session.rs` |
| FR-030 | The system SHALL maintain a bounded scrollback buffer with a configurable maximum (default 50,000 lines), evicting oldest entries when the limit is exceeded. | `torvox-core/src/grid.rs` (`max_scrollback`, `scrollback`) |
| FR-031 | The system SHALL support a scrollback search feature that finds text matching a pattern (regex or literal) within the scrollback history. | `torvox-mcp/src/lib.rs` (`ScrollbackSearch`), `torvox-core/src/terminal.rs` |
| FR-032 | The system SHALL clear the scrollback buffer when entering the alternate screen and restore it on exit. | `torvox-core/src/grid.rs` (`set_alt_screen`) |

### 3.5 OSC Sequence Handling

| ID | Requirement | Source |
|----|-------------|--------|
| FR-033 | The system SHALL intercept OSC 7 sequences (`ESC ] 7 ; <uri> ST`) and extract the current working directory path as a `CwdEvent`. | `torvox-terminal/src/osc_handler.rs` |
| FR-034 | The system SHALL intercept OSC 8 sequences (`ESC ] 8 ; <params> ; <url> ST`) and extract hyperlink open/close events as `HyperlinkEvent`. | `torvox-terminal/src/osc_handler.rs` |
| FR-035 | The system SHALL intercept OSC 52 sequences (`ESC ] 52 ; <selection> ; <base64> ST`) and decode clipboard content as a `ClipboardEvent`. | `torvox-terminal/src/osc_handler.rs` |
| FR-036 | The system SHALL intercept OSC 9 (iTerm2) and OSC 777 (rxvt) sequences and extract notification title/body as `NotificationEvent`. | `torvox-terminal/src/osc_handler.rs` |
| FR-037 | The system SHALL pass through unrecognised OSC sequences (e.g., OSC 0 for title, OSC 4 for palette change) to the VT parser unchanged. | `torvox-terminal/src/osc_handler.rs` |
| FR-038 | The system SHALL handle partial OSC sequences that arrive split across multiple input chunks, accumulating state across `process()` calls. | `torvox-terminal/src/osc_handler.rs` |

### 3.6 Clipboard and Notifications

| ID | Requirement | Source |
|----|-------------|--------|
| FR-039 | The system SHALL copy selected text to the system clipboard on user request (e.g., copy action from selection). | `torvox-terminal/src/session.rs`, `torvox-gui-android/src/bridge.rs` |
| FR-040 | The system SHALL read clipboard content when requested by terminal applications via OSC 52 (paste). | `torvox-terminal/src/osc_handler.rs` |
| FR-041 | The system SHALL display Android notifications for terminal-emitted OSC 9/777 notification sequences. | `torvox-terminal/src/osc_handler.rs` (`NotificationEvent`) |

### 3.7 SSH/Mosh Connectivity

| ID | Requirement | Source |
|----|-------------|--------|
| FR-042 | The system SHALL provide an executable (`torvox-exec`) capable of establishing SSH and Mosh connections. | `torvox-exec/src/main.rs` |
| FR-043 | The system SHALL integrate SSH/Mosh sessions with the terminal session lifecycle (PTY management, resize forwarding). | `torvox-exec/src/`, `torvox-terminal/src/session.rs` |

### 3.8 MCP Server Integration

| ID | Requirement | Source |
|----|-------------|--------|
| FR-044 | The system SHALL run an MCP (Model Context Protocol) server over a Unix domain socket, communicating via JSON-RPC 2.0 with newline-delimited JSON. | `torvox-mcp/src/lib.rs` |
| FR-045 | The MCP server SHALL expose tools for listing sessions, reading grid state, reading scrollback, reading cursor position, and reading selected text. | `torvox-mcp/src/lib.rs` (`list_tools`) |
| FR-046 | The MCP server SHALL expose tools for writing to the PTY, sending signals, resizing the terminal, and setting clipboard content (gated behind `--mcp-allow-write`). | `torvox-mcp/src/lib.rs` |
| FR-047 | The MCP server SHALL expose a scrollback search tool that matches a regex pattern and returns matching line numbers, text, and column ranges. | `torvox-mcp/src/lib.rs` (`SearchMatch`) |
| FR-048 | The MCP server SHALL expose an input queue mechanism that watches for a prompt pattern in scrollback and automatically injects queued text (AI agent automation). | `torvox-mcp/src/lib.rs` (`InputQueue`) |

### 3.9 Android Bridge

| ID | Requirement | Source |
|----|-------------|--------|
| FR-049 | The system SHALL bridge Rust terminal state to Kotlin using boltffi data types (`BridgeCell`, `BridgeAttrs`, `BridgeGrid`) mapped over JNA. | `torvox-gui-android/src/bridge.rs` |
| FR-050 | The system SHALL synchronize the terminal grid, cursor, selection, and scrollback to the Kotlin UI layer via serialized snapshots (rkyv format). | `torvox-core/src/snapshot.rs`, `torvox-gui-android/src/bridge.rs` |
| FR-051 | The system SHALL use JNI for NDK-level functions (ANativeWindow lifecycle, surface creation/destruction) via `jni_bridge.rs`. | `torvox-gui-android/src/jni_bridge.rs` |
| FR-052 | The system SHALL handle Android surface creation and destruction events, recreating the wgpu surface and render pipeline as needed. | `torvox-gui-android/src/surface.rs` |
| FR-053 | The system SHALL support ProGuard/R8 obfuscation with `-dontoptimize` to preserve JNA reflection-based binding. | `AGENTS.md` (Pitfall #14) |

### 3.10 Configuration and Themes

| ID | Requirement | Source |
|----|-------------|--------|
| FR-054 | The system SHALL provide 16 built-in color themes: Catppuccin Mocha, Catppuccin Latte, Dracula+, Nord, Tokyo Night, Rose Pine, Gruvbox Dark, Gruvbox Light, Everforest Dark, One Dark, One Light, Monokai, Ayu Dark, Ayu Light, Kanagawa Wave, and Night Owl. | `torvox-core/src/config.rs` (`Theme::all_built_in()`) |
| FR-055 | The system SHALL support custom theme definition via TOML with fields for name, background, foreground, cursor, selection background, and 16 ANSI color slots. | `torvox-core/src/config.rs` (`Theme::parse_custom()`) |
| FR-056 | The system SHALL support configuration of terminal dimensions (rows, cols), scrollback size, shell path, font size, backspace mode, and right-Alt mode via `TerminalConfig`. | `torvox-core/src/config.rs` |
| FR-057 | The repository SHALL NOT contain golden images (reference PNG screenshots used for pixel-by-pixel comparison). All rendering verification SHALL use logical assertions (pixel-coordinate checks, OCR text detection) instead of image comparison. | `docs/standards/QUALITY-GATE.md`, `.gitignore` |

---

## 4. Non-Functional Requirements

### 4.1 Safety

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-001 | `torvox-core` SHALL contain zero `unsafe` blocks. The build MUST fail if `cargo geiger --package torvox-core` reports any `unsafe` usage. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-002 | All `unsafe` blocks in the codebase (confined to `torvox-terminal/src/pty.rs` for `fork/exec` and FFI boundary code) SHALL be preceded by a `// SAFETY:` comment explaining the invariants. | `AGENTS.md` |
| NFR-003 | The system SHALL not panic in error paths. Library functions SHALL return `Result` or `Option` rather than panicking. | `AGENTS.md` |
| NFR-004 | The system SHALL use `thiserror 2` (not `anyhow`) for error types in library crates. | `AGENTS.md` |
| NFR-005 | The system SHALL handle thread panics gracefully: the PTY reader thread, process waiter thread, and render thread SHALL NOT bring down the entire process on panic. | `torvox-terminal/src/session.rs` |

### 4.2 Performance

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-006 | The render thread SHALL use wgpu (Vulkan) for GPU-accelerated rendering. Software rendering via CPU text drawing (`Canvas.drawText`) is forbidden. | `AGENTS.md`, `torvox-renderer/src/gpu.rs` |
| NFR-007 | The glyph atlas SHALL be managed by `guillotiere` with a cache capacity of at least 10,000 glyph entries and eviction when full. | `torvox-renderer/src/font.rs` |
| NFR-008 | The scrollback buffer SHALL be bounded to a configurable maximum (default 50,000 lines) with automatic eviction of oldest entries. SHALL NOT exhibit unbounded memory growth. | `torvox-core/src/grid.rs` (`max_scrollback`) |
| NFR-009 | Each terminal session SHALL use a bounded number of threads (6–7): PTY reader, process waiter, render thread, plus shared threads. | `AGENTS.md` |
| NFR-010 | The frame pipeline SHALL only repaint dirty rows as tracked by the `DirtyMask` bitfield, avoiding full-grid redraws on every frame. | `torvox-core/src/cell.rs` (`DirtyMask`) |
| NFR-011 | The shaped text cache SHALL be capped at 4,096 entries to avoid unbounded memory growth from repeated shaping of different text runs. | `torvox-renderer/src/font.rs` |

### 4.3 Maintainability

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-012 | The crate dependency graph SHALL be strictly one-way with no circular dependencies. The build SHALL fail on cycle detection. | `AGENTS.md` |
| NFR-013 | The codebase SHALL pass `cargo clippy --all -- --deny warnings` with zero warnings. No `#[allow]` attributes in production source code. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-014 | The codebase SHALL pass `cargo fmt --check` with consistent formatting. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-015 | The Kotlin codebase SHALL pass `./gradlew spotlessCheck detekt` with zero violations. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-016 | When `torvox-core` types change, the bridge types in `torvox-gui-android/src/bridge.rs` and `TorvoxBridge.kt` SHALL be updated correspondingly. | `AGENTS.md` |

### 4.4 Compatibility

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-017 | The system SHALL target Android as the primary platform, using Kotlin + Compose for the UI layer. | `AGENTS.md` |
| NFR-018 | The system SHALL use Vulkan via wgpu for rendering. On systems without a physical GPU, Mesa's Lavapipe (software Vulkan) SHALL be used as the Vulkan implementation. On Android emulators, SwiftShader SHALL be used. | `AGENTS.md` (Pitfall #19) |
| NFR-019 | The build SHALL be deterministic via Nix flake, pinning all dependencies including the Zig compiler (for Ghostty), Rust toolchain, and Android SDK. | `flake.nix`, `AGENTS.md` |
| NFR-020 | The Ghostty library (libghostty-vt) SHALL be linked as a dynamic library (dylib) with the SONAME versioned suffix stripped for Android compatibility. | `AGENTS.md` (Pitfall #10) |
| NFR-021 | The APK SHALL use the application ID `com.termux` and SHALL be signed with the AOSP testkey (not self-signed certificates). | `AGENTS.md` (Pitfalls #16, #17) |

### 4.5 Reliability

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-022 | The render thread SHALL detect GPU surface loss (Android configuration change, activity restart) and recreate the wgpu pipeline automatically. After 100 consecutive errors (~10 seconds), the thread SHALL exit permanently and require a new surface to restart. | `AGENTS.md` (Pitfall #13), `torvox-renderer/src/gpu.rs` |
| NFR-023 | The OSC handler SHALL cap payload size at 1 MB (`MAX_PAYLOAD_BYTES`) to prevent denial-of-service via oversized OSC sequences. | `torvox-terminal/src/osc_handler.rs` |
| NFR-024 | The system SHALL recover from PTY read errors without crashing the session. The reader thread SHALL log errors and continue reading. | `torvox-terminal/src/session.rs` |

---

## 5. Appendix

### A. Requirement Traceability

| Feature Area | Functional Requirements | Non-Functional Requirements |
|--------------|------------------------|-----------------------------|
| Terminal Emulation | FR-001 — FR-009 | NFR-001, NFR-003 |
| Rendering Pipeline | FR-010 — FR-019 | NFR-006, NFR-007, NFR-010, NFR-011, NFR-022 |
| Input Handling | FR-020 — FR-025 | — |
| Session Management | FR-026 — FR-032 | NFR-008, NFR-009, NFR-022 |
| OSC Sequences | FR-033 — FR-038 | NFR-023 |
| Clipboard & Notifications | FR-039 — FR-041 | — |
| SSH/Mosh Connectivity | FR-042 — FR-043 | — |
| MCP Server | FR-044 — FR-048 | — |
| Android Bridge | FR-049 — FR-053 | NFR-016 |
| Configuration & Themes | FR-054 — FR-056 | — |
| Safety | — | NFR-001 — NFR-005 |
| Performance | — | NFR-006 — NFR-011 |
| Maintainability | — | NFR-012 — NFR-016 |
| Compatibility | — | NFR-017 — NFR-021 |
| Reliability | — | NFR-022 — NFR-024 |

### B. Thread Model

Each terminal session operates with the following threads:

1. **PTY Reader Thread** — reads raw bytes from the PTY master fd, feeds them
   through the `OscHandler` interceptor and then to the `GhosttyTerminal` parser.
   Communicates parsed output to the session via a `flume` channel.
2. **Process Waiter Thread** — blocks on `waitpid` for the child process to
   exit. Emits `ProcessExited` event on termination.
3. **Render Thread** — waits on a `CountDownLatch` wake signal, reads the
   current grid snapshot, shapes text via `cosmic-text`, rasterizes glyphs via
   `swash`, packs into a `guillotiere` atlas, and issues wgpu draw commands.

Total: 3 session-specific threads plus shared infrastructure (MCP server I/O
thread, etc.), staying within the 6–7 threads per session limit.

### C. Render Pipeline

```
PTY → flume channel → GhosttyTerminal → DirtyMask → RenderThread
  → cosmic-text shape + swash glyph rasterize
  → guillotiere pack into atlas
  → wgpu atlas upload
  → Instance[] vertex buffer
  → wgpu render_frame → SurfaceView
```

All stages run on the GPU after atlas upload. The CPU does not perform
per-pixel rendering.

### D. Key File Index

| File | Description |
|------|-------------|
| `torvox-core/src/cell.rs` | Cell, Attrs, Color, DirtyMask |
| `torvox-core/src/grid.rs` | Grid, scrollback buffer, dirty tracking |
| `torvox-core/src/config.rs` | TerminalConfig, Theme, FontConfig |
| `torvox-core/src/selection.rs` | Selection modes (char/word/line/block) |
| `torvox-core/src/event.rs` | TerminalEvent types |
| `torvox-core/src/snapshot.rs` | rkyv serialization for Android bridge |
| `torvox-terminal/src/pty.rs` | PTY pair creation (fork/exec) |
| `torvox-terminal/src/session.rs` | Session orchestrator |
| `torvox-terminal/src/ghostty_terminal.rs` | Ghostty VT engine wrapper |
| `torvox-terminal/src/keyboard.rs` | Kitty keyboard protocol encoding |
| `torvox-terminal/src/osc_handler.rs` | OSC 7/8/9/52/777 interceptor |
| `torvox-renderer/src/gpu.rs` | wgpu render pipeline, atlas, instances |
| `torvox-renderer/src/font.rs` | cosmic-text shaping, swash rasterization |
| `torvox-gui-android/src/bridge.rs` | boltffi bridge types |
| `torvox-gui-android/src/jni_bridge.rs` | JNI NDK functions |
| `torvox-gui-android/src/surface.rs` | Android surface management |
| `torvox-mcp/src/lib.rs` | MCP server |
| `torvox-exec/src/main.rs` | SSH/Mosh executable |

### E. Requirements Verification Matrix

Each requirement is linked to its verification method, test command, and acceptance criteria section.

| ID | Requirement | Verification Method | Verification Command | Acceptance Section |
|----|-------------|-------------------|---------------------|-------------------|
| FR-001 | Process VT/xterm escape sequences using the Ghostty parser (`libghostty-vt`). | Automated Test | `cargo test --package torvox-terminal` | §FR-001§ (VT/xterm Escape Sequence Processing) |
| FR-002 | Maintain a terminal grid data model (`Grid`) consisting of rows of cells, each with a character code, foreground/background color, and text attributes (`Attrs`). | Automated Test | `cargo test --package torvox-core && cargo test --package torvox-terminal` | §FR-002§ (Terminal Grid Data Model) |
| FR-003 | Support SGR (Select Graphic Rendition) parameters: bold, dim, italic, underline, double underline, blink, reverse, hidden, strikethrough, overline, and protected. | Automated Test | `cargo test --package torvox-terminal` | §FR-003§ (SGR Attributes) |
| FR-004 | Support 16 ANSI color palette indices plus 256-color and truecolor (24-bit RGB) foreground/background specifications. | Automated Test | `cargo test --package torvox-core && cargo test --package torvox-terminal` | §FR-004§ (Color Support) |
| FR-005 | Support alternate screen buffer mode (SM/RM 1049) for full-screen applications (e.g., vim, less). | Automated Test | `cargo test --package torvox-terminal` | §FR-005§ (Alternate Screen Buffer) |
| FR-006 | Support cursor positioning and movement (CUU, CUD, CUF, CUB, CUP, HVP, etc.) and cursor style (block, bar, underline, beam) with visible/hidden state. | Automated Test | `cargo test --package torvox-terminal` | §FR-006§ (Cursor Positioning and Style) |
| FR-007 | Support scrolling regions (`scroll_up`, `scroll_down`, `insert_lines`, `delete_lines`) with configurable top/bottom boundaries. | Automated Test | `cargo test --package torvox-core && cargo test --package torvox-terminal` | §FR-007§ (Scrolling Regions) |
| FR-008 | Support tab stops (set, clear, move). | Automated Test | `cargo test --package torvox-terminal` | §FR-008§ (Tab Stops) |
| FR-009 | Report terminal size changes via `SIGWINCH` to the child process. | Automated Test | `cargo test --package torvox-terminal` | §FR-009§ (SIGWINCH Reporting) |
| FR-010 | Render the terminal grid using wgpu (Vulkan) as the sole graphics backend. OpenGL and CPU software paths are not supported. | Automated Test | `cargo test --package torvox-renderer` | §FR-010§ (wgpu Rendering) |
| FR-011 | Shape text runs using `cosmic-text` and rasterize glyphs using `swash`, caching results in a GPU atlas. | Automated Test | `cargo test --package torvox-renderer` | §FR-011§ (Text Shaping and Glyph Rasterization) |
| FR-012 | Pack glyph bitmaps into a GPU texture atlas using `guillotiere` for dynamic rectangle allocation and eviction. | Automated Test | `cargo test --package torvox-renderer` | §FR-012§ (Glyph Atlas Management) |
| FR-013 | Maintain a dirty mask (`DirtyMask`) that tracks which rows of the grid have changed and limit rendering to those rows. | Automated Test | `cargo test --package torvox-core` | §FR-013§ (Dirty Mask Tracking) |
| FR-014 | Render a cell cursor (block, bar, underline, beam) with configurable color and blink behavior. | Automated Test | `cargo test --package torvox-renderer` | §FR-014§ (Cursor Rendering) |
| FR-015 | Render text selection highlights (character, word, line, block modes) as colored overlays on the affected cells. | Automated Test | `cargo test --package torvox-renderer` | §FR-015§ (Selection Rendering) |
| FR-016 | Support font configuration: family, size, line spacing, and fallback to preferred monospace fonts (Roboto Mono, JetBrains Mono, etc.). | Automated Test | `cargo test --package torvox-renderer && cargo test --package torvox-terminal` | §FR-016§ (Font Configuration) |
| FR-017 | Render the terminal background, foreground, and 16-color ANSI palette from the active theme configuration. | Automated Test | `cargo test --package torvox-renderer && cargo test --package torvox-core` | §FR-017§ (Theme Rendering) |
| FR-018 | Recover from GPU surface destruction (e.g., Android activity restart) by recreating the render pipeline and continuing without data loss. | Automated Test | `cargo test --package torvox-gui-android` | §FR-018§ (GPU Surface Recovery) |
| FR-019 | Support the Kitty Graphics Protocol (KGP) for rendering inline images as textured quads. | Automated Test | `cargo test --package torvox-renderer` | §FR-019§ (Kitty Graphics Protocol) |
| FR-020 | Encode physical keyboard input using the Kitty Keyboard Protocol (KBP) for extended modifier and key reporting. | Automated Test | `cargo test --package torvox-terminal` | §FR-020§ (Keyboard Input) |
| FR-021 | Support IME (Input Method Editor) text input for composing CJK and other complex characters, with `Composing` state management. | Automated Test | `cargo test --package torvox-core` | §FR-021§ (IME Text Input) |
| FR-022 | Support terminal selection in four modes: character (`Char`), word (`Word`), line (`Line`), and block (`Block`). | Automated Test | `cargo test --package torvox-core && cargo test --package torvox-terminal` | §FR-022§ (Selection Modes) |
| FR-023 | Automatically expand word-mode selections to word boundaries and detect URLs (`http://`, `https://`, `ftp://`, `www.`) for URL-aware selection expansion. | Automated Test | `cargo test --package torvox-core` | §FR-023§ (Selection Expansion) |
| FR-024 | Support touch input gestures: tap to place cursor, long-press for selection handles, and swipe for scrollback navigation. | Automated Test | `cd android && ./gradlew testDebugUnitTest` | §FR-024§ (Touch Input Handling) |
| FR-025 | Support configurable backspace mode (DEL `0x7f` or BS `0x08`) and right-Alt mode (character modifier or meta). | Automated Test | `cargo test --package torvox-core` | §FR-025§ (Input Configuration) |
| FR-026 | Spawn a child process (shell or custom executable) connected to a pseudo-terminal (PTY) via `fork/exec`. | Automated Test | `cargo test --package torvox-terminal` | §FR-026§ (PTY Process Spawning) |
| FR-027 | Read PTY output on a dedicated reader thread and forward parsed output to the grid update pipeline via a `flume` channel. | Automated Test | `cargo test --package torvox-terminal` | §FR-027§ (PTY Reader Thread) |
| FR-028 | Wait for child process exit on a dedicated waiter thread and emit a `ProcessExited` event on termination. | Automated Test | `cargo test --package torvox-terminal` | §FR-028§ (Process Waiter Thread) |
| FR-029 | Support resizing a terminal session (changing rows and columns) and forwarding the new size to the child process via `SIGWINCH`. | Automated Test | `cargo test --package torvox-core && cargo test --package torvox-terminal` | §FR-029§ (Session Resizing) |
| FR-030 | Maintain a bounded scrollback buffer with a configurable maximum (default 50,000 lines), evicting oldest entries when the limit is exceeded. | Automated Test | `cargo test --package torvox-core && cargo test --package torvox-terminal` | §FR-030§ (Scrollback Buffer) |
| FR-031 | Support a scrollback search feature that finds text matching a pattern (regex or literal) within the scrollback history. | Automated Test | `cargo test --package torvox-mcp` | §FR-031§ (Scrollback Search) |
| FR-032 | Clear the scrollback buffer when entering the alternate screen and restore it on exit. | Automated Test | `cargo test --package torvox-terminal` | §FR-032§ (Alternate Screen Scrollback) |
| FR-033 | Intercept OSC 7 sequences (`ESC ] 7 ; <uri> ST`) and extract the current working directory path as a `CwdEvent`. | Automated Test | `cargo test --package torvox-terminal` | §FR-033§ (OSC 7 — Current Working Directory) |
| FR-034 | Intercept OSC 8 sequences (`ESC ] 8 ; <params> ; <url> ST`) and extract hyperlink open/close events as `HyperlinkEvent`. | Automated Test | `cargo test --package torvox-terminal` | §FR-034§ (OSC 8 — Hyperlinks) |
| FR-035 | Intercept OSC 52 sequences (`ESC ] 52 ; <selection> ; <base64> ST`) and decode clipboard content as a `ClipboardEvent`. | Automated Test | `cargo test --package torvox-terminal` | §FR-035§ (OSC 52 — Clipboard Access) |
| FR-036 | Intercept OSC 9 (iTerm2) and OSC 777 (rxvt) sequences and extract notification title/body as `NotificationEvent`. | Automated Test | `cargo test --package torvox-terminal` | §FR-036§ (OSC 9/777 — Notifications) |
| FR-037 | Pass through unrecognised OSC sequences (e.g., OSC 0 for title, OSC 4 for palette change) to the VT parser unchanged. | Automated Test | `cargo test --package torvox-terminal` | §FR-037§ (OSC Passthrough) |
| FR-038 | Handle partial OSC sequences that arrive split across multiple input chunks, accumulating state across `process()` calls. | Automated Test | `cargo test --package torvox-terminal` | §FR-038§ (OSC Split Handling) |
| FR-039 | Copy selected text to the system clipboard on user request (e.g., copy action from selection). | Automated Test | `cargo test --package torvox-terminal && cargo test --package torvox-gui-android` | §FR-039§ (Copy to Clipboard) |
| FR-040 | Read clipboard content when requested by terminal applications via OSC 52 (paste). | Automated Test | `cargo test --package torvox-terminal` | §FR-040§ (Paste from Clipboard) |
| FR-041 | Display Android notifications for terminal-emitted OSC 9/777 notification sequences. | Automated Test | `cargo test --package torvox-terminal` | §FR-041§ (Notifications) |
| FR-042 | Provide an executable (`torvox-exec`) capable of establishing SSH and Mosh connections. | Automated Test | `cargo test --package torvox-exec` | §FR-042§ (SSH/Mosh Executable) |
| FR-043 | Integrate SSH/Mosh sessions with the terminal session lifecycle (PTY management, resize forwarding). | Automated Test | `cargo test --package torvox-exec` | §FR-043§ (SSH/Mosh Integration) |
| FR-044 | Run an MCP (Model Context Protocol) server over a Unix domain socket, communicating via JSON-RPC 2.0 with newline-delimited JSON. | Automated Test | `cargo test --package torvox-mcp` | §FR-044§ (MCP Server) |
| FR-045 | Expose tools for listing sessions, reading grid state, reading scrollback, reading cursor position, and reading selected text. | Automated Test | `cargo test --package torvox-mcp` | §FR-045§ (Read-Only MCP Tools) |
| FR-046 | Expose tools for writing to the PTY, sending signals, resizing the terminal, and setting clipboard content (gated behind `--mcp-allow-write`). | Automated Test | `cargo test --package torvox-mcp` | §FR-046§ (Write-Enabled MCP Tools) |
| FR-047 | Expose a scrollback search tool that matches a regex pattern and returns matching line numbers, text, and column ranges. | Automated Test | `cargo test --package torvox-mcp` | §FR-047§ (MCP Scrollback Search) |
| FR-048 | Expose an input queue mechanism that watches for a prompt pattern in scrollback and automatically injects queued text (AI agent automation). | Automated Test | `cargo test --package torvox-mcp` | §FR-048§ (MCP Input Queue) |
| FR-049 | Bridge Rust terminal state to Kotlin using boltffi data types (`BridgeCell`, `BridgeAttrs`, `BridgeGrid`) mapped over JNA. | Automated Test | `cargo test --package torvox-gui-android && cargo test --package torvox-terminal` | §FR-049§ (boltffi Bridge Types) |
| FR-050 | Synchronize the terminal grid, cursor, selection, and scrollback to the Kotlin UI layer via serialized snapshots (rkyv format). | Automated Test | `cargo test --package torvox-gui-android && cargo test --package torvox-core` | §FR-050§ (rkyv Snapshots) |
| FR-051 | Use JNI for NDK-level functions (ANativeWindow lifecycle, surface creation/destruction) via `jni_bridge.rs`. | Automated Test | `cargo test --package torvox-gui-android` | §FR-051§ (JNI NDK Bridge) |
| FR-052 | Handle Android surface creation and destruction events, recreating the wgpu surface and render pipeline as needed. | Automated Test | `cargo test --package torvox-gui-android` | §FR-052§ (Surface Lifecycle) |
| FR-053 | Support ProGuard/R8 obfuscation with `-dontoptimize` to preserve JNA reflection-based binding. | Automated Test | `cd android && ./gradlew assembleDebug` | §FR-053§ (ProGuard/R8 Compatibility) |
| FR-054 | Provide 16 built-in color themes: Catppuccin Mocha, Catppuccin Latte, Dracula+, Nord, Tokyo Night, Rose Pine, Gruvbox Dark, Gruvbox Light, Everforest Dark, One Dark, One Light, Monokai, Ayu Dark, Ayu Light, Kanagawa Wave, and Night Owl. | Automated Test | `cargo test --package torvox-core && cd android && ./gradlew testDebugUnitTest` | §FR-054§ (Built-in Themes) |
| FR-055 | Support custom theme definition via TOML with fields for name, background, foreground, cursor, selection background, and 16 ANSI color slots. | Automated Test | `cargo test --package torvox-core` | §FR-055§ (Custom Theme Definition) |
| FR-056 | Support configuration of terminal dimensions (rows, cols), scrollback size, shell path, font size, backspace mode, and right-Alt mode via `TerminalConfig`. | Automated Test | `cargo test --package torvox-core` | §FR-056§ (Terminal Configuration) |
| FR-057 | Repository SHALL NOT contain golden images; rendering verification SHALL use logical assertions or OCR. | Tool Inspection | `git ls-files '*.png' | grep -E 'screenshots|golden|roborazzi' | wc -l` | §FR-057§ (Golden Image Ban) |
| NFR-001 | `torvox-core` SHALL contain zero `unsafe` blocks. The build MUST fail if `cargo geiger --package torvox-core` reports any `unsafe` usage. | Tool Inspection | `cargo geiger --package torvox-core` | §NFR-001§ (Zero Unsafe in Core) |
| NFR-002 | All `unsafe` blocks in the codebase SHALL be preceded by a `// SAFETY:` comment explaining the invariants. | Tool Inspection | `cargo geiger --all` | §NFR-002§ (SAFETY Comments) |
| NFR-003 | The system SHALL not panic in error paths. Library functions SHALL return `Result` or `Option` rather than panicking. | Automated Test | `cargo test --workspace` | §NFR-003§ (No Panic in Error Paths) |
| NFR-004 | The system SHALL use `thiserror 2` (not `anyhow`) for error types in library crates. | Automated Test | `cargo test --workspace` | §NFR-004§ (Error Type Convention) |
| NFR-005 | The system SHALL handle thread panics gracefully: the PTY reader thread, process waiter thread, and render thread SHALL NOT bring down the entire process on panic. | Automated Test | `cargo test --package torvox-terminal` | §NFR-005§ (Thread Panic Isolation) |
| NFR-006 | The render thread SHALL use wgpu (Vulkan) for GPU-accelerated rendering. Software rendering via CPU text drawing (`Canvas.drawText`) is forbidden. | Automated Test | `cargo test --package torvox-renderer` | §NFR-006§ (GPU Rendering Only) |
| NFR-007 | The glyph atlas SHALL be managed by `guillotiere` with a cache capacity of at least 10,000 glyph entries and eviction when full. | Automated Test | `cargo test --package torvox-renderer` | §NFR-007§ (Glyph Atlas Capacity) |
| NFR-008 | The scrollback buffer SHALL be bounded to a configurable maximum (default 50,000 lines) with automatic eviction of oldest entries. SHALL NOT exhibit unbounded memory growth. | Automated Test | `cargo test --package torvox-terminal` | §NFR-008§ (Scrollback Bounding) |
| NFR-009 | Each terminal session SHALL use a bounded number of threads (6–7): PTY reader, process waiter, render thread, plus shared threads. | Automated Test | `cargo test --package torvox-terminal` | §NFR-009§ (Bounded Thread Count) |
| NFR-010 | The frame pipeline SHALL only repaint dirty rows as tracked by the `DirtyMask` bitfield, avoiding full-grid redraws on every frame. | Automated Test | `cargo test --package torvox-core` | §NFR-010§ (Dirty Row Repaint) |
| NFR-011 | The shaped text cache SHALL be capped at 4,096 entries to avoid unbounded memory growth from repeated shaping of different text runs. | Automated Test | `cargo test --package torvox-renderer` | §NFR-011§ (Text Cache Capacity) |
| NFR-012 | The crate dependency graph SHALL be strictly one-way with no circular dependencies. The build SHALL fail on cycle detection. | Automated Test | `cargo test --workspace` | §NFR-012§ (Acyclic Dependencies) |
| NFR-013 | The codebase SHALL pass `cargo clippy --all -- --deny warnings` with zero warnings. No `#[allow]` attributes in production source code. | Tool Inspection | `cargo clippy --all -- --deny warnings` | §NFR-013§ (Clippy Cleanliness) |
| NFR-014 | The codebase SHALL pass `cargo fmt --check` with consistent formatting. | Tool Inspection | `cargo fmt --check` | §NFR-014§ (Formatting Consistency) |
| NFR-015 | The Kotlin codebase SHALL pass `./gradlew spotlessCheck detekt` with zero violations. | Tool Inspection | `cd android && ./gradlew spotlessCheck detekt` | §NFR-015§ (Kotlin Lint Compliance) |
| NFR-016 | When `torvox-core` types change, the bridge types in `torvox-gui-android/src/bridge.rs` and `TorvoxBridge.kt` SHALL be updated correspondingly. | Automated Test | `cargo test --package torvox-gui-android` | §NFR-016§ (Bridge Type Sync) |
| NFR-017 | The system SHALL target Android as the primary platform, using Kotlin + Compose for the UI layer. | Automated Test | `cd android && ./gradlew assembleDebug` | §NFR-017§ (Android Platform) |
| NFR-018 | The system SHALL use Vulkan via wgpu for rendering. On systems without a physical GPU, Mesa's Lavapipe (software Vulkan) SHALL be used as the Vulkan implementation. On Android emulators, SwiftShader SHALL be used. | Automated Test | `cargo test --package torvox-renderer` | §NFR-018§ (Vulkan Rendering Backend) |
| NFR-019 | The build SHALL be deterministic via Nix flake, pinning all dependencies including the Zig compiler (for Ghostty), Rust toolchain, and Android SDK. | Tool Inspection | `nix flake check` | §NFR-019§ (Deterministic Build) |
| NFR-020 | The Ghostty library (libghostty-vt) SHALL be linked as a dynamic library (dylib) with the SONAME versioned suffix stripped for Android compatibility. | Automated Test | `nu scripts/check-rust.nu` | §NFR-020§ (Ghostty Dynamic Linking) |
| NFR-021 | The APK SHALL use the application ID `com.termux` and SHALL be signed with the AOSP testkey (not self-signed certificates). | Automated Test | `cd android && ./gradlew assembleDebug` | §NFR-021§ (APK Identity) |
| NFR-022 | The render thread SHALL detect GPU surface loss (Android configuration change, activity restart) and recreate the wgpu pipeline automatically. After 100 consecutive errors (~10 seconds), the thread SHALL exit permanently and require a new surface to restart. | Automated Test | `cargo test --package torvox-gui-android` | §NFR-022§ (Render Thread Recovery) |
| NFR-023 | The OSC handler SHALL cap payload size at 1 MB (`MAX_PAYLOAD_BYTES`) to prevent denial-of-service via oversized OSC sequences. | Automated Test | `cargo test --package torvox-terminal` | §NFR-023§ (OSC Payload Limit) |
| NFR-024 | The system SHALL recover from PTY read errors without crashing the session. The reader thread SHALL log errors and continue reading. | Automated Test | `cargo test --package torvox-terminal` | §NFR-024§ (PTY Read Error Recovery) |
