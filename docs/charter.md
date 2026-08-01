# Project Charter

## Mission

Build a GPU-accelerated Android terminal emulator using Vulkan (via wgpu),
Ghostty's vendored VT parser, and Kotlin + Compose UI.

## Core Goals

1. **GPU-accelerated rendering** — no CPU software fallback, only wgpu/Vulkan.
   Mesa Lavapipe for headless Linux, SwiftShader for emulator guest GPU.
2. **Full VT5xx+ compliance** — vendored Ghostty parser handles all escape
   sequences, scrollback, SGR, DEC modes, Kitty keyboard protocol, OSC.
3. **Low-latency input** — separate PTY reader and input writer threads,
   Kitty keyboard protocol, IME pixel-stable layout.
4. **Android-first** — Kotlin + Compose, JNI bridge, package name `com.termux`
   (Termux add-on compatibility).
5. **AI agent integration** — MCP server over Unix socket + stdio (tower-mcp).

## Target Users

- **Android developers** needing a native terminal for debugging, Git, adb
- **Termux users** who rely on the existing Termux ecosystem
- **SSH/Mosh users** connecting to remote servers from Android
- **AI-assisted developers** using AI coding agents (Codex CLI, OpenCode, etc.)
  that consume MCP services

## Design Philosophy

- GPU-accelerated everywhere (no `Canvas.drawText`, no CPU fallback)
- Deterministic Nix builds (no `sdkmanager`, no runtime discovery)
- Test closest to source (Rust-side over Android-side, state over pixels)
- Keep it simple: one `native/` crate, JNI direct bridge (no boltffi/JNA),
  embedded MCP (no standalone server)

## Out of Scope

- Java files (Kotlin only on Android side)
- `portable-pty` / `bincode` / `rust-android-gradle` packages
- CPU/Canvas rendering fallback
- Desktop builds (Linux builds for CI/testing only)
- Bundled fonts (uses system fonts)

## Key Technical Constraints

| Constraint | Rule |
|------------|------|
| **Unsafe** | Zero `unsafe` in terminal module; allowed only in `pty.rs` and FFI with `// SAFETY:` |
| **Error handling** | `anyhow` forbidden in library code; use `thiserror 2` |
| **Naming** | Full words only; no abbreviations (`config` not `cfg`, `terminal` not `term`) |
| **Scripts** | Nushell only; no bash/sh |
| **MCP** | Embedded in `native/`; no standalone server |
| **Test key** | AOSP testkey only; self-signing forbidden |
| **Package name** | `com.termux` — do not change |

> This charter consolidates the project brief and product context.
