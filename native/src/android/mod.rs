//! Android JNI bridge and NDK integration.
//!
//! [`ffi`] exports the JNI functions called from Kotlin. [`jni_bridge`]
//! wraps NDK functions such as `ANativeWindow_fromSurface`.

pub mod ffi;
#[cfg(target_os = "android")]
pub mod jni_bridge;
#[cfg(target_os = "android")]
pub mod logging;
