# Technology Stack

## Core Stack

| Layer | Technology | Version | Purpose |
|-------|-----------|---------|---------|
| Rust edition | Rust 2024 | 1.97+ | Main language for terminal, render, bridge |
| Android UI | Kotlin + Jetpack Compose | Android 14+ (API 34+) | UI layer, TextureView, JNI client |
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

> This document consolidates the technology context.
