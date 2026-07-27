//! Android JNI bridge and NDK integration.

pub mod ffi;

#[cfg(target_os = "android")]
pub mod logging;
