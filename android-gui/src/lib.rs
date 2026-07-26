//! Android FFI boundary (boltffi + JNA).
//!
//! [`bridge`] is the **single export location** for the Rust↔Kotlin data
//! bridge (per AGENTS.md); all `terminal-core` types crossing the FFI must be
//! kept in sync with `NativeBridge.kt`. [`jni_bridge`] (Android only) wraps
//! NDK functions such as `ANativeWindow`; [`surface`] drives the render
//! pipeline against the Android surface. Depends on `terminal-core`,
//! `terminal-engine`, and `gpu-renderer`.

pub mod bridge;
#[cfg(target_os = "android")]
pub mod jni_bridge;
mod lock_util;
#[cfg(target_os = "android")]
pub mod logging;
pub mod surface;

#[cfg(not(target_os = "android"))]
pub mod mock_surface;

pub use bridge::{BridgeCell, NativeBridge, TerminalConfig, TerminalError, TerminalEvent};
pub use surface::AndroidSurface;
