//! Android FFI boundary for torvox (boltffi + JNA).
//!
//! [`bridge`] is the **single export location** for the Rust↔Kotlin data
//! bridge (per AGENTS.md); all `torvox-core` types crossing the FFI must be
//! kept in sync with `TorvoxBridge.kt`. [`jni_bridge`] (Android only) wraps
//! NDK functions such as `ANativeWindow`; [`surface`] drives the render
//! pipeline against the Android surface. Depends on `torvox-core`,
//! `torvox-terminal`, and `torvox-renderer`.

pub mod bridge;
#[cfg(target_os = "android")]
pub mod jni_bridge;
pub mod surface;

#[cfg(not(target_os = "android"))]
pub mod mock_surface;

pub use bridge::{BridgeCell, TerminalConfig, TerminalError, TerminalEvent, TorvoxBridge};
pub use surface::AndroidSurface;
