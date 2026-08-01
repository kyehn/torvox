//! Android JNI bridge and NDK integration.
//!
//! # Requirements
//! - FR-049 — JNI NDK bridge: direct JNI exports for the Android platform

pub mod ffi;

#[cfg(target_os = "android")]
pub mod logging;
