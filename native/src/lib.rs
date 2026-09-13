//! Merged native crate — terminal engine, GPU renderer, JNI bridge.
//!
//! This crate consolidates the former terminal-engine, gpu-renderer, and
//! android-gui crates into a single unit for faster compilation and simpler
//! cross-module refactoring.
//!
//! No feature flags — every capability is always compiled in.
//!
//! ## Module hierarchy
//!
//! ```text
//! native/
//! ├── terminal/       — Ghostty VT 解析、PTY 管理、Session
//! ├── render/         — wgpu 管线、cosmic-text 整形、swash 字形
//! ├── android/        — JNI FFI 导出、NDK 桥接、日志
//! └── (单元测试就近存放于 `#[cfg(test)]`，集成行为由 `tests/features/*.feature` 统一管理)
//! ```

pub mod event;

// ── Terminal engine (ex terminal-engine) ─────────────────────────────────
pub mod terminal;

// ── GPU renderer (ex gpu-renderer) ───────────────────────────────────────
pub mod render;

// ── Android JNI bridge (ex android-gui) ──────────────────────────────────
pub mod android;

/// Platform-independent logcat chunking. Kept outside the
/// `android` module so the exact algorithm is unit-testable on the host.
pub mod log_chunk;

#[cfg(test)]
mod prop_tests;
