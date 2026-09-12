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
//! ├── terminal/       — Ghostty VT parsing, PTY management, Session
//! ├── render/         — wgpu pipeline, cosmic-text shaping, swash glyphs
//! ├── android/        — JNI FFI exports, NDK bridge, logging
//! ├── test/           — cross-module integration tests (cfg(test) only)
//! └── (unit tests live beside the code under `#[cfg(test)]`)
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

/// Cross-module integration tests (moved in from `native/tests/`).
#[cfg(test)]
mod test;
