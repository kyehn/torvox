# Reference Project Analysis

Comparative architecture analysis with peer terminal emulator projects,
conducted August 2026 to validate torvox design decisions.

---

## 1. Peer Projects

| Project | Language | Render | JNI Bridge | SSH | MCP | Notes |
|---------|----------|--------|------------|-----|-----|-------|
| **torvox** (us) | Rust + Kotlin | wgpu (Vulkan) | Direct JNI | External exec-bin | ✅ 8 tools | Ghostty VT engine |
| **chuchu/Ghossh** | Zig + Kotlin | None (Canvas) | Zig JNI | ✅ libssh2 | ❌ | Ghostty VT; 400+ themes; Tailscale |
| **mightty** | Rust (Windows) | GPUI | N/A | ❌ | ❌ | Ghostty VT; build.rs bindgen |
| **Haven** | Kotlin + Rust | None | UniFFI | ✅ JSch | ✅ 130+ tools | PRoot Linux; multi-protocol |
| **Termux** | Kotlin | None (Canvas) | N/A | External ssh | ❌ | apt/pkg; 6 plugins |
| **ghostling** | Zig+Swift | Metal | Zig | ❌ | ❌ | iOS/macOS Ghostty |
| **ghostty-android-terminal** | Kotlin | None | N/A | ❌ | ❌ | arm64chroot; rootfs tarball |

---

## 2. Architecture Comparison

### 2.1 VT Engine

All modern projects use **Ghostty** (`libghostty-vt`):
- **torvox** and **mightty**: Rust wrapper via `libghostty-rs` git dependency
- **chuchu/Ghossh**: Zig wrapper (direct C ABI)
- **ghostling**: Swift wrapper (iOS)

**torvox advantage**: Single source of truth — no parallel data model.
All terminal state comes from Ghostty C API; CellData is extracted for rendering.

### 2.2 GPU Rendering

**torvox** is unique among Android terminals in using full GPU rendering (wgpu/Vulkan).
Others use `Canvas.drawText` per cell (CPU-bound, poor CJK performance).

| Project | Renderer | Performance | CJK Support |
|---------|----------|-------------|-------------|
| **torvox** | wgpu (Vulkan) | 0.2ms typing, 5107 fps | ✅ Font fallback + glyph cache |
| **chuchu** | Canvas.drawText | CPU-bound, ~5 fps scrolling | ❌ Basic |
| **Haven** | Canvas.drawText | CPU-bound | ✅ Through PRoot |
| **Termux** | Canvas.drawText | CPU-bound | ✅ Through Android |

### 2.3 JNI Bridge

| Project | Method | Safety | Complexity |
|---------|--------|--------|------------|
| **torvox** | `jni` crate (Rust) | ✅ SAFETY comments on all unsafe | Low — direct JNI |
| **chuchu** | Zig `export fn` | ✅ Zig ABI compatible | Low — Zig exports C ABI |
| **Haven** | UniFFI | ✅ Auto-generated bindings | Medium — codegen |
| **Termux** | N/A (pure Kotlin) | N/A | N/A |

**torvox choice (direct JNI via `jni` crate)**: Good tradeoff. The `jni` crate
handles JNI environment management, type conversion, and exception throwing.
The only unsafe code is for NDK functions (ANativeWindow, logging).

### 2.4 Thread Model

| Project | Threads per Session | Notes |
|---------|--------------------|-------|
| **torvox** | 4 (Reader, Writer, Waiter, Renderer) + 1 shared MCP | CellData channel decouples VT from render |
| **chuchu** | 2 (VT + UI) | Simpler but UI blocks on VT |
| **mightty** | PtyWorker (Input/Output/Control parts) | Clean worker separation |

### 2.5 Build System

| Project | Build | Deterministic | Complexity |
|---------|-------|--------------|------------|
| **torvox** | Nix + Cargo + Gradle | ✅ | High (Nix learning curve) |
| **chuchu/Ghossh** | Gradle + NDK + Zig | ⚠️ | Medium |
| **mightty** | Cargo (build.rs compiles Zig) | ✅ | Medium |
| **Termux** | Gradle | ⚠️ | Low |

---

## 3. Features to Consider

### 3.1 High Priority

- **Native SSH integration** (libssh2): Replace external exec-bin binary.
  chuchu/Ghossh show it's feasible; would enable session management,
  key storage, Tailscale integration.

- **Tailscale SSH**: chuchu/Ghossh native support via Tailscale API socket.

### 3.2 Medium Priority

- **Extended MCP toolset**: Haven has 130+ tools. File browsing,
  session control, configuration management.

- **PRoot / user-space Linux**: arm64chroot from ghostty-android-terminal
  or PRoot from Haven. Enables apt/dpkg in Termux-compatible userland.

- **Theme system expansion**: chuchu syncs 400+ Ghostty themes.

- **Backup/restore**: chuchu AES-256-GCM encrypted export.

### 3.3 Low Priority (Future)

- **Plugin architecture**: Termux-style Intent-based plugins.
- **Multi-channel distribution**: F-Droid, GitHub Releases, Google Play.
- **Binding generator**: mightty-style automation for Ghostty C API.

---

## 4. Design Validation

| torvox Decision | Validated By | Result |
|----------------|--------------|--------|
| Ghostty as single source of truth | chuchu, mightty, ghostling all use Ghostty | ✅ Correct |
| wgpu GPU rendering | Unique among Android terminals; necessary for performance | ✅ Correct |
| Direct JNI (no boltffi/JNA) | chuchu uses Zig JNI; both avoid JNA | ✅ Correct |
| embedded MCP | Haven proves MCP value for terminal apps | ✅ Correct |
| Single crate (no multi-crate) | No peer project uses multi-crate for terminal engine | ✅ Correct |
| Nix build | Unique but proven correct | ⚠️ Acceptable |
