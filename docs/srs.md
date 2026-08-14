# Software Requirements Specification

> **✅ Rewritten (Aug 2026).** Single `native/` crate architecture, current
> technologies. References to old crate names (`terminal-core`, `terminal-engine`,
> `gpu-renderer`, `android-gui`, `mcp-server`) and removed technologies
> (boltffi, rkyv, JNA) have been eliminated except where historical context is
> needed. See `docs/architecture.md` for current module layout.

## 1. Introduction

### 1.1 Purpose

This Software Requirements Specification (SRS) describes the functional and
non-functional requirements for **Terminal**, a GPU-accelerated terminal emulator
for Android. It uses wgpu (Vulkan) for GPU-accelerated rendering, the
Ghostty VT parser for xterm-compatible terminal emulation, and provides a
Kotlin+Compose UI backed by a Rust native library.

The document is intended for developers, testers, and maintainers of the
project. Requirements are derived exclusively from the existing codebase,
documentation, and build infrastructure. No speculative or unimplemented
features are included.

### 1.2 Scope

The application is a terminal emulator for Android devices. It supports:

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
| **JNA** | Java Native Access — historically used for Kotlin-to-Rust binding (removed in Phase 2, replaced by direct JNI) |
| **JSON-RPC** | JSON Remote Procedure Call protocol used by the MCP server |
| **Kitty KBP** | Kitty Keyboard Protocol — extended keyboard event encoding |
| **MCP** | Model Context Protocol — a protocol for AI-agent-to-tool communication |
| **NDK** | Android Native Development Kit |
| **OSC** | Operating System Command — escape sequences beginning with `ESC ]` |
| **PTY** | Pseudo-terminal — the kernel device pairing a master and slave endpoint |
| **rkyv** | Zero-copy serialization framework (removed in Phase 2) |
| **SSH** | Secure Shell protocol |
| **VT** | Video Terminal — the family of escape-sequence standards |
| **wgpu** | A cross-platform GPU abstraction layer (Rust implementation of WebGPU) |

### 1.4 References

| Reference | File / Location |
|-----------|-----------------|
| Architecture & Thread Model | [`AGENTS.md`](../AGENTS.md) |
| Project Standards (Style) | [`docs/standards/STYLE.md`](standards/STYLE.md) |
| Project Standards (Testing) | [`docs/standards/TESTING.md`](standards/TESTING.md) |
| Project Standards (Quality Gate) | [`docs/standards/QUALITY-GATE.md`](standards/QUALITY-GATE.md) |
| Build System | [`flake.nix`](../flake.nix), [`Cargo.toml`](../Cargo.toml) |
| Core Data Model | [`native/src/`](../native/src) |
| VT / Ghostty Integration | [`native/src/`](../native/src) |
| Renderer (wgpu Pipeline) | [`native/src/`](../native/src) |
| Android Bridge | [`native/src/`](../native/src) |
| MCP Server | [`native/src/`](../native/src) |
| SSH/Mosh Executable | [`exec-bin/src/`](../exec-bin/src) |
| CI Scripts | [`scripts/`](../scripts) |

---

## 2. Overall Description

### 2.1 Product Perspective

It is an Android application (package name `com.termux`) that provides a
full-featured terminal emulator. It replaces the CPU-based software rendering
path employed by traditional Android terminal emulators with a GPU-accelerated
pipeline via wgpu (Vulkan). The system is decomposed into a set of Rust crates
with strict one-way dependency ordering:

```text
libghostty-vt / libghostty-vt-sys
    ↑
native (merged: terminal engine + GPU renderer + JNI bridge + MCP)
    ↑
android/app (Kotlin + Compose)
```

The codebase is a single `native/` crate (see `docs/architecture.md`). All terminal,
rendering, and Android bridge code lives in one crate, eliminating cross-crate
boundary complexity. The old 5-crate architecture (terminal-core, terminal-engine,
gpu-renderer, android-gui, mcp-server) was merged in Phase 4.

### 2.2 Product Functions

The following high-level functions are provided:

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
- **Scrollback Buffer**: Maintain a bounded scrollback history (10,000 lines by
  default) with search capability.
- **OSC Sequence Handling**: Intercept and handle OSC 7 (CWD), OSC 8
  (hyperlinks), OSC 9/777 (notifications), and OSC 52 (clipboard).
- **SSH/Mosh Connectivity**: Launch SSH and Mosh sessions via the
  `exec-bin` crate.
- **MCP Server**: Expose terminal session state and control to AI agents via
  JSON-RPC over a Unix socket.
- **Android Bridge**: Synchronize Rust-side terminal state to the Kotlin UI
  layer via direct JNI calls.
- **Session Lifecycle**: Create, resize, and terminate terminal sessions with
  proper cleanup of threads, PTYs, and GPU resources.
- **Render Thread Recovery**: Automatically recover the render thread after
  Android surface destruction (configuration change, activity restart).
- **Color Themes**: Support 16 built-in color themes (Kotlin `BuiltInThemes`) and user-defined
  custom themes persisted via DataStore.
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
- **`no_std` no longer required**: the terminal module uses `std` directly
- **Zero `unsafe` in terminal path**: the terminal module must contain zero `unsafe` blocks.
- **Android API level**: Must target the Android NDK with minimum API level
  compatible with SurfaceView and Vulkan.
- **Bounded threads**: Each session is limited to 4 threads (PTY reader,
  input writer, process waiter, render thread).
- **No `anyhow` in library crates**: Library crates must use `thiserror` for
  error types.
- **No `Canvas.drawText` per cell**: Rendering must not use per-cell software
  text drawing; the GPU pipeline is mandatory.

### 2.5 Project Charter（项目章程，吸收自 `docs/charter.md`，原文已删除）

**Mission**: Build a GPU-accelerated Android terminal emulator using Vulkan (via
wgpu), Ghostty's vendored VT parser, and Kotlin + Compose UI.

**Core Goals**:

1. **GPU-accelerated rendering** — no CPU software fallback, only wgpu/Vulkan.
   Mesa Lavapipe for headless Linux, SwiftShader for emulator guest GPU.
2. **Full VT5xx+ compliance** — vendored Ghostty parser handles all escape
   sequences, scrollback, SGR, DEC modes, Kitty keyboard protocol, OSC.
3. **Low-latency input** — separate PTY reader and input writer threads,
   Kitty keyboard protocol, IME pixel-stable layout.
4. **Android-first** — Kotlin + Compose, JNI bridge, package name `com.termux`
   (Termux add-on compatibility).
5. **AI agent integration** — MCP server over Unix socket + stdio (tower-mcp).

**Target Users**: Android developers needing a native terminal for debugging,
Git, adb; Termux users relying on the existing Termux ecosystem; SSH/Mosh users
connecting to remote servers from Android; AI-assisted developers using AI
coding agents (Codex CLI, OpenCode, etc.) that consume MCP services.

**Design Philosophy**:

- GPU-accelerated everywhere (no `Canvas.drawText`, no CPU fallback)
- Deterministic Nix builds (no `sdkmanager`, no runtime discovery)
- Test closest to source (Rust-side over Android-side, state over pixels)
- Keep it simple: one `native/` crate, JNI direct bridge (no boltffi/JNA),
  embedded MCP (no standalone server)

**Out of Scope**（补充 §1.2）: Java files (Kotlin only on Android side);
`portable-pty` / `bincode` / `rust-android-gradle` packages; CPU/Canvas
rendering fallback; desktop builds (Linux builds for CI/testing only); bundled
fonts (uses system fonts).

**Key Technical Constraints**（补充 §2.4）: zero `unsafe` in the terminal
module（仅 `pty.rs` 与 FFI 允许，且须 `// SAFETY:`）；`anyhow` forbidden in
library code (`thiserror 2`)；naming = full words only（无缩写）；
scripts = Nushell only；MCP embedded in `native/`；AOSP testkey only
（self-signing forbidden）；package name `com.termux` — do not change。

---

## 3. Functional Requirements

### 3.1 Terminal Emulation

| ID | Requirement | Source |
|----|-------------|--------|
| FR-001 | The system SHALL process VT/xterm escape sequences using the Ghostty parser (`libghostty-vt`). | `AGENTS.md`, `native/src/terminal/ghostty_terminal/` |
| FR-002 | The system SHALL maintain a terminal grid data model (`Grid`) consisting of rows of cells, each with a character code, foreground/background color, and text attributes (`Attrs`). | `native/src/terminal/ghostty_terminal/types.rs` |
| FR-003 | The system SHALL support SGR (Select Graphic Rendition) parameters: bold, dim, italic, underline, double underline, blink, reverse, hidden, strikethrough, overline, and protected. | `native/src/terminal/sgr_parser.rs` |
| FR-004 | The system SHALL support 16 ANSI color palette indices plus 256-color and truecolor (24-bit RGB) foreground/background specifications. | `native/src/terminal/ghostty_terminal/types.rs` |
| FR-005 | The system SHALL support alternate screen buffer mode (SM/RM 1049) for full-screen applications (e.g., vim, less). | `native/src/terminal/ghostty_terminal/internal.rs` |
| FR-006 | The system SHALL support cursor positioning and movement (CUU, CUD, CUF, CUB, CUP, HVP, etc.) and cursor style (block, bar, underline, beam) with visible/hidden state. | `native/src/terminal/ghostty_terminal/types.rs`, `native/src/terminal/cursor_cmds.rs` |
| FR-007 | The system SHALL support scrolling regions (`scroll_up`, `scroll_down`, `insert_lines`, `delete_lines`) with configurable top/bottom boundaries. | `native/src/terminal/ghostty_terminal/internal.rs` |
| FR-008 | The system SHALL support tab stops (set, clear, move). | `native/src/terminal/action_parser.rs` |
| FR-009 | The system SHALL report terminal size changes via `SIGWINCH` to the child process. | `native/src/terminal/session.rs` |

### 3.2 Rendering Pipeline

| ID | Requirement | Source |
|----|-------------|--------|
| FR-010 | The system SHALL render the terminal grid using wgpu (Vulkan) as the sole graphics backend. OpenGL and CPU software paths are not supported. | `AGENTS.md`, `native/src/render/` |
| FR-011 | The system SHALL shape text runs using `cosmic-text` and rasterize glyphs using `swash`, caching results in a GPU atlas. | `native/src/render/font/` |
| FR-012 | The system SHALL pack glyph bitmaps into a GPU texture atlas using `guillotiere` for dynamic rectangle allocation and eviction. | `native/src/render/font/atlas.rs`, `native/src/render/font/` |
| FR-013 | The system SHALL track which rows of the grid changed since the last frame and limit rendering to those rows. | `native/src/render/cell_builder.rs`, `native/src/render/invalidation.rs` |
| FR-014 | The system SHALL render a cell cursor (block, bar, underline, beam) with configurable color and blink behavior. | `native/src/render/pass.rs` |
| FR-015 | The system SHALL render text selection highlights (character, word, line, block modes) as colored overlays on the affected cells. | `native/src/render/pass.rs` |
| FR-016 | The system SHALL support font configuration: family, size, and line spacing, with fallback to the system monospace font (resolved via fonts.xml / fontdb). | `native/src/render/font/font_db.rs` |
| FR-017 | The system SHALL render the terminal background, foreground, and 16-color ANSI palette from the active theme configuration. | `native/src/render/pass.rs` |
| FR-018 | The system SHALL recover from GPU surface destruction (e.g., Android activity restart) by recreating the render pipeline and continuing without data loss. | `AGENTS.md` (Pitfall #15), `native/src/render/context.rs` |
| FR-019 | The system SHALL support the Kitty Graphics Protocol (KGP) for rendering inline images as textured quads. | `native/src/render/pass.rs` |

### 3.3 Input Handling

| ID | Requirement | Source |
|----|-------------|--------|
| FR-020 | The system SHALL encode physical keyboard input using the Kitty Keyboard Protocol (KBP) for extended modifier and key reporting. | `AGENTS.md`, `native/src/terminal/ghostty_terminal/keymap.rs` |
| FR-021 | The system SHALL support IME (Input Method Editor) text input for composing CJK and other complex characters, with `Composing` state management. | `native/src/terminal/ghostty_terminal/internal.rs` |
| FR-022 | The system SHALL support terminal selection in four modes: character (`Char`), word (`Word`), line (`Line`), and block (`Block`). | `native/src/terminal/ghostty_terminal/types.rs` |
| FR-023 | The system SHALL automatically expand word-mode selections to word boundaries and detect URLs (`http://`, `https://`, `ftp://`, `www.`) for URL-aware selection expansion. | `native/src/terminal/ghostty_terminal/public_api.rs` |
| FR-024 | The system SHALL support touch input gestures: tap to place cursor, long-press for selection handles, and swipe for scrollback navigation. | `native/src/render/context.rs` |
| FR-025 | The system SHALL send DEL (`0x7F`) for the backspace key and encode modifier keys (Ctrl/Shift/Alt/Super) per the Kitty keyboard protocol. | `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt`, `native/src/terminal/ghostty_terminal/keymap.rs` |

### 3.4 Session Management

| ID | Requirement | Source |
|----|-------------|--------|
| FR-026 | The system SHALL spawn a child process (shell or custom executable) connected to a pseudo-terminal (PTY) via `fork/exec`. | `native/src/terminal/pty.rs` |
| FR-027 | The system SHALL read PTY output on a dedicated reader thread and forward parsed output to the grid update pipeline via a `flume` channel. | `native/src/terminal/session.rs`, `AGENTS.md` |
| FR-028 | The system SHALL wait for child process exit on a dedicated waiter thread and emit a `ProcessExited` event on termination. | `native/src/terminal/session.rs`, `AGENTS.md` |
| FR-029 | The system SHALL support resizing a terminal session (changing rows and columns) and forwarding the new size to the child process via `SIGWINCH`. | `native/src/terminal/session.rs` |
| FR-030 | The system SHALL maintain a bounded scrollback buffer with a configurable maximum (default 10,000 lines), evicting oldest entries when the limit is exceeded. | `native/src/terminal/ghostty_terminal/types.rs` |
| FR-031 | The system SHALL support a scrollback search feature that finds text matching a pattern (regex or literal) within the scrollback history. | `native/src/terminal/session.rs` |
| FR-032 | The system SHALL clear the scrollback buffer when entering the alternate screen and restore it on exit. | `native/src/terminal/ghostty_terminal/internal.rs` |

### 3.5 OSC Sequence Handling

| ID | Requirement | Source |
|----|-------------|--------|
| FR-033 | The system SHALL intercept OSC 7 sequences (`ESC ] 7 ; <uri> ST`) and extract the current working directory path as a `CwdEvent`. | `native/src/terminal/osc_handler.rs` |
| FR-034 | The system SHALL intercept OSC 8 sequences (`ESC ] 8 ; <params> ; <url> ST`) and extract hyperlink open/close events as `HyperlinkEvent`. | `native/src/terminal/osc_handler.rs` |
| FR-035 | The system SHALL intercept OSC 52 sequences (`ESC ] 52 ; <selection> ; <base64> ST`) and decode clipboard content as a `ClipboardEvent`. | `native/src/terminal/osc_handler.rs` |
| FR-036 | The system SHALL intercept OSC 9 (iTerm2) and OSC 777 (rxvt) sequences and extract notification title/body as `NotificationEvent`. | `native/src/terminal/osc_handler.rs` |
| FR-037 | The system SHALL pass through unrecognised OSC sequences (e.g., OSC 0 for title, OSC 4 for palette change) to the VT parser unchanged. | `native/src/terminal/osc_handler.rs` |
| FR-038 | The system SHALL handle partial OSC sequences that arrive split across multiple input chunks, accumulating state across `process()` calls. | `native/src/terminal/osc_handler.rs` |

### 3.6 Clipboard and Notifications

| ID | Requirement | Source |
|----|-------------|--------|
| FR-039 | The system SHALL copy selected text to the system clipboard on user request (e.g., copy action from selection). | `native/src/terminal/session.rs`, `native/src/android/ffi.rs` |
| FR-040 | The system SHALL read clipboard content when requested by terminal applications via OSC 52 (paste). | `native/src/terminal/osc_handler.rs` |
| FR-041 | The system SHALL display Android notifications for terminal-emitted OSC 9/777 notification sequences. | `native/src/terminal/osc_handler.rs` |

### 3.7 SSH/Mosh Connectivity

| ID | Requirement | Source |
|----|-------------|--------|
| FR-042 | The system SHALL provide an executable (`exec-bin`) capable of establishing SSH and Mosh connections. | `exec-bin/src/main.rs` |
| FR-043 | The system SHALL integrate SSH/Mosh sessions with the terminal session lifecycle (PTY management, resize forwarding). | `exec-bin/src/`, `native/src/terminal/session.rs` |

### 3.8 MCP Server Integration

| ID | Requirement | Source |
|----|-------------|--------|
| FR-044 | The system SHALL run an MCP (Model Context Protocol) server over a Unix domain socket, communicating via JSON-RPC 2.0 with newline-delimited JSON. | `native/src/mcp/` |
| FR-045 | The MCP server SHALL expose tools for listing sessions, reading grid state, reading scrollback, reading cursor position, and reading selected text. | `native/src/mcp/` |
| FR-046 | The MCP server SHALL expose tools for writing to the PTY, sending signals, resizing the terminal, and setting clipboard content (gated behind `--mcp-allow-write`). | `native/src/mcp/` |
| FR-047 | The MCP server SHALL expose a scrollback search tool that matches a regex pattern and returns matching line numbers, text, and column ranges. | `native/src/mcp/` |
| FR-048 | The MCP server SHALL expose an input queue mechanism that watches for a prompt pattern in scrollback and automatically injects queued text (AI agent automation). | `native/src/mcp/` |

### 3.9 Android Bridge

| ID | Requirement | Source |
|----|-------------|--------|
| FR-049 | The system SHALL use JNI for NDK-level functions (ANativeWindow lifecycle, surface creation/destruction) via `ffi.rs`. | `native/src/android/ffi.rs` |
| FR-050 | The system SHALL handle Android surface creation and destruction events, recreating the wgpu surface and render pipeline as needed. | `native/src/render/context.rs` |
| FR-051 | The system SHALL support ProGuard/R8 obfuscation with `-dontoptimize` to preserve direct JNI (no JNA). | `AGENTS.md` (Pitfall #8) |

### 3.10 Configuration and Themes

| ID | Requirement | Source |
|----|-------------|--------|
| FR-052 | The system SHALL provide 16 built-in color themes: Catppuccin Mocha, Catppuccin Latte, Dracula Plus, Nord, Tokyo Night, Rose Pine, Gruvbox Dark/Light, Everforest Dark, One Dark/Light, Monokai, Ayu Dark/Light, Kanagawa Wave, and Night Owl. | `android/app/src/main/java/terminal/emulator/ui/theme/TerminalTheme.kt` |
| FR-053 | The system SHALL support user-defined custom themes persisted in DataStore with fields for name, background, foreground, cursor, selection background, and 16 ANSI color slots. | `android/app/src/main/java/terminal/emulator/ui/theme/UserThemeStore.kt` |
| FR-054 | The system SHALL support configuration of terminal dimensions (rows, cols), scrollback size, shell path, and font size via `TerminalConfig`. | `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt` |
| FR-055 | The repository SHALL NOT contain golden images (reference PNG screenshots used for pixel-by-pixel comparison). All rendering verification SHALL use logical assertions (pixel-coordinate checks, OCR text detection) instead of image comparison. | `docs/standards/QUALITY-GATE.md`, `.gitignore` |
| FR-056 | The crate SHALL provide an off-screen render-verification path — a procedural geometry generator (`SceneData` + floor/sphere/box/torus + hand-written matrix helpers) and an infinite LOD grid shader rendered through a `Depth32Float` attachment — used by crate tests to verify GPU rendering and depth attachment behavior. This path SHALL NOT add depth attachments to the production terminal render path. | `native/src/render/procedural_geometry.rs`, `native/shaders/grid.wgsl`, `native/src/render/pipeline.rs` |
| FR-057 | The search overlay input SHALL debounce query changes by 150 ms so the native scrollback search only runs after the user pauses typing. | `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`, `android/app/src/main/java/terminal/emulator/ui/SearchDebouncer.kt` |
| FR-058 | Dependent settings controls SHALL be grayed out (disabled) when their prerequisite is off — the cursor-speed slider when cursor blink is off, and the user-theme edit entry when no user themes exist. | `android/app/src/main/java/terminal/emulator/ui/SettingsScreen.kt`, `SettingsComponents.kt` |
| FR-059 | The terminal surface SHALL expose accessibility custom actions (read previous line, read next line, read all) and update its content description to the current screen text (throttled) so TalkBack can navigate terminal content line by line. | `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt` |
| FR-060 | The bootstrap installer SHALL support both HTTP download and local-file install (SAF), and SHALL offer an offline path reading the bootstrap archive from `assets/bootstrap/` when present (no network). Embedded assets SHALL be detected at runtime; the APK SHALL NOT be forced to carry the archive. | `android/app/src/main/java/terminal/emulator/installer/BootstrapOrchestrator.kt` |
| FR-061 | Toolbar keys SHALL support a width (flex weight) and an optional secondary key (long-press action); the layout editor SHALL allow editing both; serialization SHALL be backward compatible with existing layouts. | `android/app/src/main/java/terminal/emulator/ui/ToolbarPreferences.kt`, `ModifierBar.kt` |

---

## 4. Non-Functional Requirements

### 4.1 Safety

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-001 | The native terminal module SHALL contain zero `unsafe` blocks, verified by audit. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-002 | All `unsafe` blocks in the codebase (confined to `native/src/terminal/pty.rs` for `fork/exec` and FFI boundary code) SHALL be preceded by a `// SAFETY:` comment explaining the invariants. | `AGENTS.md` |
| NFR-003 | The system SHALL not panic in error paths. Library functions SHALL return `Result` or `Option` rather than panicking. | `AGENTS.md` |
| NFR-004 | The system SHALL use `thiserror 2` (not `anyhow`) for error types in library crates. | `AGENTS.md` |
| NFR-005 | The system SHALL handle thread panics gracefully: the PTY reader thread, process waiter thread, and render thread SHALL NOT bring down the entire process on panic. | `native/src/terminal/session.rs` |

### 4.2 Performance

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-006 | The render thread SHALL use wgpu (Vulkan) for GPU-accelerated rendering. Software rendering via CPU text drawing (`Canvas.drawText`) is forbidden. | `AGENTS.md`, `native/src/render/` |
| NFR-007 | The glyph atlas SHALL be managed by `guillotiere` with a cache capacity of at least 10,000 glyph entries and eviction when full. | `native/src/render/font/atlas.rs` |
| NFR-008 | The scrollback buffer SHALL be bounded to a configurable maximum (default 10,000 lines) with automatic eviction of oldest entries. SHALL NOT exhibit unbounded memory growth. | `native/src/terminal/ghostty_terminal/types.rs` |
| NFR-009 | Each terminal session SHALL use a bounded number of threads (4): PTY reader, input writer, process waiter, and render thread. | `AGENTS.md` |
| NFR-010 | The frame pipeline SHALL only repaint dirty rows as tracked by row-level diffing, avoiding full-grid redraws on every frame. | `native/src/render/cell_builder.rs`, `native/src/render/invalidation.rs` |
| NFR-011 | The glyph cache SHALL be capped at 10,000 entries (`GLYPH_CACHE_CAPACITY`) to avoid unbounded memory growth. | `native/src/render/font/mod.rs` |

### 4.3 Maintainability

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-012 | The crate dependency graph SHALL be strictly one-way with no circular dependencies. The build SHALL fail on cycle detection. | `AGENTS.md` |
| NFR-013 | The codebase SHALL pass `cargo clippy --all -- --deny warnings` with zero warnings. No `#[allow]` attributes in production source code. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-014 | The codebase SHALL pass `cargo fmt --check` with consistent formatting. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-015 | The Kotlin codebase SHALL pass `./gradlew spotlessCheck detekt` with zero violations. | `AGENTS.md`, `docs/standards/QUALITY-GATE.md` |
| NFR-016 | When native terminal types change, the JNI bridge in `native/src/android/ffi.rs` and `NativeBridge.kt` SHALL be updated correspondingly. | `AGENTS.md` |

### 4.4 Compatibility

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-017 | The system SHALL target Android as the primary platform, using Kotlin + Compose for the UI layer. | `AGENTS.md` |
| NFR-018 | The system SHALL use Vulkan via wgpu for rendering. On systems without a physical GPU, Mesa's Lavapipe (software Vulkan) SHALL be used as the Vulkan implementation. On Android emulators, SwiftShader SHALL be used. | `AGENTS.md` (Pitfall #13) |
| NFR-019 | The build SHALL be deterministic via Nix flake, pinning all dependencies including the Zig compiler (for Ghostty), Rust toolchain, and Android SDK. | `flake.nix`, `AGENTS.md` |
| NFR-020 | The Ghostty library (libghostty-vt) SHALL be linked as a dynamic library (dylib) with the SONAME versioned suffix stripped for Android compatibility. | `AGENTS.md` (Pitfall #4) |
| NFR-021 | The APK SHALL use the application ID `com.termux` and SHALL be signed with the AOSP testkey (not self-signed certificates). | `AGENTS.md` (Pitfalls #16, #17) |

### 4.5 Reliability

| ID | Requirement | Source |
|----|-------------|--------|
| NFR-022 | The render thread SHALL detect GPU surface loss (Android configuration change, activity restart) and recreate the wgpu pipeline automatically. After 100 consecutive errors (~10 seconds), the thread SHALL exit permanently and require a new surface to restart. | `AGENTS.md` (Pitfall #7), `native/src/render/context.rs` |
| NFR-023 | The OSC handler SHALL cap payload size at 1 MB (`MAX_PAYLOAD_BYTES`) to prevent denial-of-service via oversized OSC sequences. | `native/src/terminal/osc_handler.rs` |
| NFR-024 | The system SHALL recover from PTY read errors without crashing the session. The reader thread SHALL log errors and continue reading. | `native/src/terminal/session.rs` |
| NFR-025 | The system SHALL provide unified logging infrastructure that writes to both logcat and a rotating file, with log levels configurable independently for each output. | `native/src/android/logging.rs` |

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
| Android Bridge | FR-049 — FR-051 | NFR-016 |
| Configuration & Themes | FR-052 — FR-055 | — |
| Safety | — | NFR-001 — NFR-005 |
| Performance | — | NFR-006 — NFR-011 |
| Maintainability | — | NFR-012 — NFR-016 |
| Compatibility | — | NFR-017 — NFR-021 |
| Reliability | — | NFR-022 — NFR-024 |
| Infrastructure | — | NFR-025 |

### B. Thread Model

Each terminal session SHALL use exactly 4 threads (NFR-009):

1. **PTY Reader** — poll/read the PTY master, run the VT parser inline, feed
   the grid via `flume`.
2. **Input Writer** — write keyboard input to the PTY master (separate write
   path avoids reader contention).
3. **Process Waiter** — block on `waitpid` for the child; emit `ProcessExited`.
4. **Render Loop** — driven by the Kotlin `TerminalRuntime` calling
   `NativeBridge.render` per frame (wgpu submit via the Rust `gpu-acquire`
   worker); see `docs/architecture.md` §6.3 for ownership details.

Shared infrastructure (MCP listener) is per-process, not per-session.

### C. Render Pipeline

```text
PTY → flume channel → GhosttyTerminal → diff_dirty_rows → RenderThread
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
| `native/src/terminal/ghostty_terminal/types.rs` | GridSnapshot, CellData, CellSnapshot, CursorStyle, SelectionMode, Attrs |
| `native/src/terminal/ghostty_terminal/mod.rs` | GhosttyTerminal engine wrapper |
| `native/src/terminal/ghostty_terminal/internal.rs` | Grid internals, scrollback, alt screen |
| `native/src/terminal/ghostty_terminal/public_api.rs` | Public API (expand_word, expand_url, etc.) |
| `native/src/terminal/ghostty_terminal/commands.rs` | Command enum for terminal communication |
| `native/src/terminal/ghostty_terminal/keymap.rs` | Kitty keyboard protocol encoding |
| `native/src/terminal/session.rs` | Session orchestrator |
| `native/src/terminal/pty.rs` | PTY pair creation (fork/exec) |
| `native/src/terminal/osc_handler.rs` | OSC 7/8/9/52/777 interceptor |
| `native/src/terminal/sgr_parser.rs` | SGR attribute parser |
| `native/src/terminal/cursor_cmds.rs` | Cursor movement commands |
| `native/src/terminal/action_parser.rs` | Tab stops, other control actions |
| `native/src/render/context.rs` | GpuContext, wgpu state |
| `native/src/render/pass.rs` | Per-frame rendering (cursor, selection, kgp) |
| `native/src/render/font/atlas.rs` | Glyph atlas (guillotiere packing) |
| `native/src/render/font/font_db.rs` | Font database loading and lookup |
| `native/src/render/font/shaping.rs` | cosmic-text shaping |
| `native/src/render/font/rasterization.rs` | swash glyph rasterization |
| `native/src/android/ffi.rs` | JNI exports and NDK bridge |
| `native/src/android/logging.rs` | Unified logging (logcat + file) |
| `native/src/mcp/` | MCP server (JSON-RPC over Unix socket) |
| `exec-bin/src/main.rs` | SSH/Mosh executable |

### E. Requirements Verification Matrix

Each requirement is linked to its verification method, test command, and acceptance criteria section.

| ID | Requirement | Verification Method | Verification Command | Acceptance Section |
|----|-------------|-------------------|---------------------|-------------------|
| FR-001 | Process VT/xterm escape sequences using the Ghostty parser (`libghostty-vt`). | Automated Test | `cargo test --package native` | §FR-001§ (VT/xterm Escape Sequence Processing) |
| FR-002 | Maintain a terminal grid data model (`Grid`) consisting of rows of cells, each with a character code, foreground/background color, and text attributes (`Attrs`). | Automated Test | `cargo test --package native` | §FR-002§ (Terminal Grid Data Model) |
| FR-003 | Support SGR (Select Graphic Rendition) parameters: bold, dim, italic, underline, double underline, blink, reverse, hidden, strikethrough, overline, and protected. | Automated Test | `cargo test --package native` | §FR-003§ (SGR (Select Graphic Rendition) Attributes) |
| FR-004 | Support 16 ANSI color palette indices plus 256-color and truecolor (24-bit RGB) foreground/background specifications. | Automated Test | `cargo test --package native` | §FR-004§ (Color Support (ANSI, 256-Color, Truecolor)) |
| FR-005 | Support alternate screen buffer mode (SM/RM 1049) for full-screen applications (e.g., vim, less). | Automated Test | `cargo test --package native` | §FR-005§ (Alternate Screen Buffer) |
| FR-006 | Support cursor positioning and movement (CUU, CUD, CUF, CUB, CUP, HVP, etc.) and cursor style (block, bar, underline, beam) with visible/hidden state. | Automated Test | `cargo test --package native` | §FR-006§ (Cursor Positioning and Style) |
| FR-007 | Support scrolling regions (`scroll_up`, `scroll_down`, `insert_lines`, `delete_lines`) with configurable top/bottom boundaries. | Automated Test | `cargo test --package native` | §FR-007§ (Scrolling Regions) |
| FR-008 | Support tab stops (set, clear, move). | Automated Test | `cargo test --package native` | §FR-008§ (Tab Stops) |
| FR-009 | Report terminal size changes via `SIGWINCH` to the child process. | Automated Test | `cargo test --package native` | §FR-009§ (SIGWINCH on Terminal Resize) |
| FR-010 | Render the terminal grid using wgpu (Vulkan) as the sole graphics backend. OpenGL and CPU software paths are not supported. | Automated Test | `cargo test --package native` | §FR-010§ (GPU-Accelerated Terminal Rendering) |
| FR-011 | Shape text runs using `cosmic-text` and rasterize glyphs using `swash`, caching results in a GPU atlas. | Automated Test | `cargo test --package native` | §FR-011§ (Text Shaping and Glyph Rasterization) |
| FR-012 | Pack glyph bitmaps into a GPU texture atlas using `guillotiere` for dynamic rectangle allocation and eviction. | Automated Test | `cargo test --package native` | §FR-012§ (GPU Texture Atlas Management) |
| FR-013 | Track which rows of the grid changed since the last frame and limit rendering to those rows. | Automated Test | `cargo test --package native -- dirty_rows_collected` | §FR-013§ (Dirty Mask for Incremental Rendering) |
| FR-014 | Render a cell cursor (block, bar, underline, beam) with configurable color and blink behavior. | Automated Test | `cargo test --package native` | §FR-014§ (Cursor Rendering) |
| FR-015 | Render text selection highlights (character, word, line, block modes) as colored overlays on the affected cells. | Automated Test | `cargo test --package native` | §FR-015§ (Selection Rendering) |
| FR-016 | Support font configuration: family, size, line spacing, and fallback to preferred monospace fonts (Roboto Mono, JetBrains Mono, etc.). | Automated Test | `cargo test --package native` | §FR-016§ (Font Configuration) |
| FR-017 | Render the terminal background, foreground, and 16-color ANSI palette from the active theme configuration. | Automated Test | `cargo test --package native` | §FR-017§ (Theme-Based Color Rendering) |
| FR-018 | Recover from GPU surface destruction (e.g., Android activity restart) by recreating the render pipeline and continuing without data loss. | Automated Test | `cargo test --package native` | §FR-018§ (GPU Surface Recovery) |
| FR-019 | Support the Kitty Graphics Protocol (KGP) for rendering inline images as textured quads. | Automated Test | `cargo test --package native` | §FR-019§ (Kitty Graphics Protocol (KGP)) |
| FR-020 | Encode physical keyboard input using the Kitty Keyboard Protocol (KBP) for extended modifier and key reporting. | Automated Test | `cargo test --package native` | §FR-020§ (Kitty Keyboard Protocol) |
| FR-021 | Support IME (Input Method Editor) text input for composing CJK and other complex characters, with `Composing` state management. | Automated Test | `cargo test --package native` | §FR-021§ (IME Text Input (CJK)) |
| FR-022 | Support terminal selection in four modes: character (`Char`), word (`Word`), line (`Line`), and block (`Block`). | Automated Test | `cargo test --package native` | §FR-022§ (Selection Modes (Character, Word, Line, Block)) |
| FR-023 | Automatically expand word-mode selections to word boundaries and detect URLs (`http://`, `https://`, `ftp://`, `www.`) for URL-aware selection expansion. | Automated Test | `cargo test --package native` | §FR-023§ (Word Boundary and URL Detection) |
| FR-024 | Support touch input gestures: tap to place cursor, long-press for selection handles, and swipe for scrollback navigation. | Automated Test | `cd android && ./gradlew testDebugUnitTest` | §FR-024§ (Touch Input Gestures) |
| FR-025 | Send DEL (`0x7F`) for the backspace key and encode modifier keys (Ctrl/Shift/Alt/Super) per the Kitty keyboard protocol. | Automated Test | `cargo test --package native -- key_encode_shift_a_uses_utf8_char` | §FR-025§ (Backspace and Modifier Key Encoding) |
| FR-026 | Spawn a child process (shell or custom executable) connected to a pseudo-terminal (PTY) via `fork/exec`. | Automated Test | `cargo test --package native` | §FR-026§ (PTY Child Process Spawn) |
| FR-027 | Read PTY output on a dedicated reader thread and forward parsed output to the grid update pipeline via a `flume` channel. | Automated Test | `cargo test --package native` | §FR-027§ (Dedicated PTY Reader Thread) |
| FR-028 | Wait for child process exit on a dedicated waiter thread and emit a `ProcessExited` event on termination. | Automated Test | `cargo test --package native` | §FR-028§ (Process Waiter Thread) |
| FR-029 | Support resizing a terminal session (changing rows and columns) and forwarding the new size to the child process via `SIGWINCH`. | Automated Test | `cargo test --package native` | §FR-029§ (Session Resize) |
| FR-030 | Maintain a bounded scrollback buffer with a configurable maximum (default 10,000 lines), evicting oldest entries when the limit is exceeded. | Automated Test | `cargo test --package native` | §FR-030§ (Bounded Scrollback Buffer) |
| FR-031 | Support a scrollback search feature that finds text matching a pattern (regex or literal) within the scrollback history. | Automated Test | `cargo test --package native` | §FR-031§ (Scrollback Search) |
| FR-032 | Clear the scrollback buffer when entering the alternate screen and restore it on exit. | Automated Test | `cargo test --package native` | §FR-032§ (Alternate Screen Scrollback Management) |
| FR-033 | Intercept OSC 7 sequences (`ESC ] 7 ; <uri> ST`) and extract the current working directory path as a `CwdEvent`. | Automated Test | `cargo test --package native` | §FR-033§ (OSC 7 — Current Working Directory) |
| FR-034 | Intercept OSC 8 sequences (`ESC ] 8 ; <params> ; <url> ST`) and extract hyperlink open/close events as `HyperlinkEvent`. | Automated Test | `cargo test --package native` | §FR-034§ (OSC 8 — Hyperlinks) |
| FR-035 | Intercept OSC 52 sequences (`ESC ] 52 ; <selection> ; <base64> ST`) and decode clipboard content as a `ClipboardEvent`. | Automated Test | `cargo test --package native` | §FR-035§ (OSC 52 — Clipboard Access) |
| FR-036 | Intercept OSC 9 (iTerm2) and OSC 777 (rxvt) sequences and extract notification title/body as `NotificationEvent`. | Automated Test | `cargo test --package native` | §FR-036§ (OSC 9 / OSC 777 — Notifications) |
| FR-037 | Pass through unrecognised OSC sequences (e.g., OSC 0 for title, OSC 4 for palette change) to the VT parser unchanged. | Automated Test | `cargo test --package native` | §FR-037§ (Unrecognised OSC Passthrough) |
| FR-038 | Handle partial OSC sequences that arrive split across multiple input chunks, accumulating state across `process()` calls. | Automated Test | `cargo test --package native` | §FR-038§ (Partial OSC Sequence Handling) |
| FR-039 | Copy selected text to the system clipboard on user request (e.g., copy action from selection). | Automated Test | `cargo test --package native` | §FR-039§ (Copy to System Clipboard) |
| FR-040 | Read clipboard content when requested by terminal applications via OSC 52 (paste). | Automated Test | `cargo test --package native` | §FR-040§ (OSC 52 Paste (Clipboard Read)) |
| FR-041 | Display Android notifications for terminal-emitted OSC 9/777 notification sequences. | Automated Test | `cargo test --package native` | §FR-041§ (Android Notifications via OSC) |
| FR-042 | Provide an executable (`exec-bin`) capable of establishing SSH and Mosh connections. | Automated Test | `cargo test --package exec-bin` | §FR-042§ (SSH/Mosh Executable) |
| FR-043 | Integrate SSH/Mosh sessions with the terminal session lifecycle (PTY management, resize forwarding). | Automated Test | `cargo test --package exec-bin` | §FR-043§ (SSH/Mosh Session Integration) |
| FR-044 | Run an MCP (Model Context Protocol) server over a Unix domain socket, communicating via JSON-RPC 2.0 with newline-delimited JSON. | Automated Test | `cargo test --package native` | §FR-044§ (MCP Server over Unix Socket) |
| FR-045 | Expose tools for listing sessions, reading grid state, reading scrollback, reading cursor position, and reading selected text. | Automated Test | `cargo test --package native` | §FR-045§ (Read-Only MCP Tools) |
| FR-046 | Expose tools for writing to the PTY, sending signals, resizing the terminal, and setting clipboard content (gated behind `--mcp-allow-write`). | Automated Test | `cargo test --package native` | §FR-046§ (Write MCP Tools (Gated)) |
| FR-047 | Expose a scrollback search tool that matches a regex pattern and returns matching line numbers, text, and column ranges. | Automated Test | `cargo test --package native` | §FR-047§ (Scrollback Search MCP Tool) |
| FR-048 | Expose an input queue mechanism that watches for a prompt pattern in scrollback and automatically injects queued text (AI agent automation). | Automated Test | `cargo test --package native` | §FR-048§ (Input Queue Automation) |
| FR-049 | Use JNI for NDK-level functions (ANativeWindow lifecycle, surface creation/destruction) via `ffi.rs`. | Automated Test | `cargo test -p integration-tests --test jni_bridge_test` | §FR-049§ (JNI NDK Bridge) |
| FR-050 | Handle Android surface creation and destruction events, recreating the wgpu surface and render pipeline as needed. | Automated Test | `cargo test --package native` | §FR-050§ (Surface Lifecycle) |
| FR-051 | Support ProGuard/R8 obfuscation with `-dontoptimize` to preserve direct JNI (no JNA). | Automated Test | `cd android && ./gradlew assembleDebug` | §FR-051§ (ProGuard/R8 Compatibility) |
| FR-052 | Provide 16 built-in color themes defined in Kotlin `BuiltInThemes`. | Code Review | `android/app/src/main/java/terminal/emulator/ui/theme/TerminalTheme.kt` | §FR-052§ (16 Built-In Color Themes) |
| FR-053 | Support user-defined custom themes persisted in DataStore with fields for name, background, foreground, cursor, selection background, and 16 ANSI color slots. | Kotlin Test | `cd android && ./gradlew testDebugUnitTest` | §FR-053§ (Custom Theme via DataStore) |
| FR-054 | Support configuration of terminal dimensions (rows, cols), scrollback size, shell path, and font size via `TerminalConfig`. | Code Review | `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt` | §FR-054§ (Terminal Configuration) |
| FR-055 | Repository SHALL NOT contain golden images; rendering verification SHALL use logical assertions or OCR. | Tool Inspection | `git ls-files '*.png' \| grep -E 'screenshots\|golden\|roborazzi' \| wc -l` | §FR-055§ (Repository Standards — Banned Binary Artifacts) |
| NFR-001 | The native terminal module SHALL contain zero `unsafe` blocks, verified by audit. | Tool Inspection | `cargo audit` | §NFR-001§ (No Unsafe in Production Code) |
| NFR-002 | All `unsafe` blocks in the codebase SHALL be preceded by a `// SAFETY:` comment explaining the invariants. | Tool Inspection | `grep -r '^unsafe' native/src/ --include='*.rs'` | §NFR-002§ (FFI Safety — extern "C" Validation) |
| NFR-003 | The system SHALL not panic in error paths. Library functions SHALL return `Result` or `Option` rather than panicking. | Automated Test | `cargo test --workspace` | §NFR-003§ (No Panics in Library Code) |
| NFR-004 | The system SHALL use `thiserror 2` (not `anyhow`) for error types in library crates. | Automated Test | `cargo test --workspace` | §NFR-004§ (thiserror, Not anyhow) |
| NFR-005 | The system SHALL handle thread panics gracefully: the PTY reader thread, process waiter thread, and render thread SHALL NOT bring down the entire process on panic. | Automated Test | `cargo test --package native` | §NFR-005§ (Thread Panic Containment) |
| NFR-006 | The render thread SHALL use wgpu (Vulkan) for GPU-accelerated rendering. Software rendering via CPU text drawing (`Canvas.drawText`) is forbidden. | Automated Test | `cargo test --package native` | §NFR-006§ (GPU-Only Rendering) |
| NFR-007 | The glyph atlas SHALL be managed by `guillotiere` with a cache capacity of at least 10,000 glyph entries and eviction when full. | Automated Test | `cargo test --package native` | §NFR-007§ (Glyph Atlas Capacity) |
| NFR-008 | The scrollback buffer SHALL be bounded to a configurable maximum (default 10,000 lines) with automatic eviction of oldest entries. SHALL NOT exhibit unbounded memory growth. | Automated Test | `cargo test --package native` | §NFR-008§ (Bounded Scrollback Memory) |
| NFR-009 | Each terminal session SHALL use a bounded number of threads (4): PTY reader, input writer, process waiter, and render thread. | Automated Test | `cargo test --package native` | §NFR-009§ (Bounded Thread Count) |
| NFR-010 | The frame pipeline SHALL only repaint dirty rows as tracked by row-level diffing, avoiding full-grid redraws on every frame. | Automated Test | `cargo test --package native -- dirty_rows_collected` | §NFR-010§ (Dirty Row-Only Repaint) |
| NFR-011 | The glyph cache SHALL be capped at 10,000 entries (`GLYPH_CACHE_CAPACITY`) to avoid unbounded memory growth. | Code Review | `native/src/render/font/mod.rs` | §NFR-011§ (Glyph Cache Cap) |
| NFR-012 | The crate dependency graph SHALL be strictly one-way with no circular dependencies. The build SHALL fail on cycle detection. | Automated Test | `cargo test --workspace` | §NFR-012§ (One-Way Crate Dependencies) |
| NFR-013 | The codebase SHALL pass `cargo clippy --all -- --deny warnings` with zero warnings. No `#[allow]` attributes in production source code. | Tool Inspection | `cargo clippy --all -- --deny warnings` | §NFR-013§ (Clippy Clean) |
| NFR-014 | The codebase SHALL pass `cargo fmt --check` with consistent formatting. | Tool Inspection | `cargo fmt --check` | §NFR-014§ (Formatting Consistency) |
| NFR-015 | The Kotlin codebase SHALL pass `./gradlew spotlessCheck detekt` with zero violations. | Tool Inspection | `cd android && ./gradlew spotlessCheck detekt` | §NFR-015§ (Kotlin Lint and Format) |
| NFR-016 | When native terminal types change, the JNI bridge in `native/src/android/ffi.rs` and `NativeBridge.kt` SHALL be updated correspondingly. | Automated Test | `cargo test --package native` | §NFR-016§ (Bridge Type Synchronization) |
| NFR-017 | The system SHALL target Android as the primary platform, using Kotlin + Compose for the UI layer. | Automated Test | `cd android && ./gradlew assembleDebug` | §NFR-017§ (Android Platform Target) |
| NFR-018 | The system SHALL use Vulkan via wgpu for rendering. On systems without a physical GPU, Mesa's Lavapipe (software Vulkan) SHALL be used as the Vulkan implementation. On Android emulators, SwiftShader SHALL be used. | Automated Test | `cargo test --package native` | §NFR-018§ (Vulkan via wgpu (Software Fallback)) |
| NFR-019 | The build SHALL be deterministic via Nix flake, pinning all dependencies including the Zig compiler (for Ghostty), Rust toolchain, and Android SDK. | Tool Inspection | `nix flake check` | §NFR-019§ (Deterministic Nix Build) |
| NFR-020 | The Ghostty library (libghostty-vt) SHALL be linked as a dynamic library (dylib) with the SONAME versioned suffix stripped for Android compatibility. | Automated Test | `nu scripts/check-rust.nu` | §NFR-020§ (Ghostty Dynamic Library Linking) |
| NFR-021 | The APK SHALL use the application ID `com.termux` and SHALL be signed with the AOSP testkey (not self-signed certificates). | Automated Test | `cd android && ./gradlew assembleDebug` | §NFR-021§ (Application ID and Signing) |
| NFR-022 | The render thread SHALL detect GPU surface loss (Android configuration change, activity restart) and recreate the wgpu pipeline automatically. After 100 consecutive errors (~10 seconds), the thread SHALL exit permanently and require a new surface to restart. | Automated Test | `cargo test --package native` | §NFR-022§ (Render Thread Surface Recovery) |
| NFR-023 | The OSC handler SHALL cap payload size at 1 MB (`MAX_PAYLOAD_BYTES`) to prevent denial-of-service via oversized OSC sequences. | Automated Test | `cargo test --package native` | §NFR-023§ (OSC Payload Size Limit) |
| NFR-024 | The system SHALL recover from PTY read errors without crashing the session. The reader thread SHALL log errors and continue reading. | Automated Test | `cargo test --package native` | §NFR-024§ (PTY Read Error Recovery) |
| NFR-025 | The system SHALL provide unified logging infrastructure that writes to both logcat and a rotating file, with log levels configurable independently for each output. | Tool Inspection | `ls native/src/android/logging.rs` | §NFR-025§ (Unified Logging) |
