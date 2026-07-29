//! Merged native crate — terminal engine, GPU renderer, JNI bridge.
//!
//! This crate consolidates the former terminal-engine, gpu-renderer, and
//! android-gui crates into a single unit for faster compilation and simpler
//! cross-module refactoring.
//!
//! ## Feature flags
//!
//! | Feature   | Deps pulled | Purpose | Default |
//! |-----------|-------------|---------|---------|
//! | `mcp`     | tower-mcp, axum, tokio, schemars | Embed MCP server (dialog/pickfile/clipboard tools) | **on** (implied by `test-util`) |
//! | `test-util` | `mcp` + bytemuck | Enable test-only types (FlatGrid, SearchHighlight) | off |
//!
//! ```ignore
//! # Dev / CI (run tests with MCP support)
//! cargo test --features test-util
//!
//! # Production (Android release — no MCP, smaller binary)
//! cargo build --release
//! ```
//!
//! ## Module hierarchy
//!
//! ```text
//! native/
//! ├── terminal/       — Ghostty VT parsing, PTY management, Session
//! ├── render/         — wgpu pipeline, cosmic-text shaping, swash glyphs
//! ├── android/        — JNI FFI exports, NDK bridge, logging
//! ├── mcp/            — MCP server (feature-gated, tower-mcp)
//! ├── lock_util       — poison recovery helpers
//! └── screenshot_tests — included into render::tests
//! ```

pub mod event;
mod lock_util;

// ── Terminal engine (ex terminal-engine) ─────────────────────────────────
pub mod terminal;

// ── GPU renderer (ex gpu-renderer) ───────────────────────────────────────
pub mod render;

// ── Android JNI bridge (ex android-gui) ──────────────────────────────────
pub mod android;

// ── MCP (JSON-RPC over Unix socket for local IPC) ────────────────────────
#[cfg(feature = "mcp")]
pub mod mcp;

// ── Re-exports for backward compatibility ────────────────────────────────
//
// These match the public API the original crate-pair provided so that
// consumers (integration-tests, exec-bin) can migrate incrementally.
// Eventually the path-qualified imports (native::terminal::session::Session)
// are preferred.

// Re-exports from terminal module (matching terminal_engine crate's public API)
pub use terminal::ghostty_terminal::{CursorStyle, SelectionMode, is_wide};
pub use terminal::mock_pty::{MockPty, MockPtyHandle};
pub use terminal::pty::{Pty, PtyError, PtyPair};
pub use terminal::shell_env::ShellEnv;

// Re-exports from render module (matching gpu_renderer crate's public API)
pub use render::font::*;
