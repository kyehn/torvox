//! JNI bridge for calling NDK functions that boltffi/JNA cannot provide.
//! Provides a reliable way to obtain an ANativeWindow pointer from a Surface object.
//!
//! # Requirements
//! - [FR-051](crate) — Surface: JNI/NDK ANativeWindow

use core::ffi::c_void;

// JNI opaque types
type JNIEnvPtr = *mut c_void;
type JClassPtr = *mut c_void;
type JObjectPtr = *mut c_void;

// SAFETY: FFI function declaration from the Android NDK. Safe to declare — the
// unsafety is in calling it, which is annotated at the call site. The signature
// matches the NDK `ANativeWindow_fromSurface` header definition.
#[cfg(target_os = "android")]
#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: JNIEnvPtr, surface: JObjectPtr) -> *mut c_void;
}

/// JNI export: io.torvox.bridge.NativeWindow.getNativeWindowPtr(Surface) -> Long
/// Returns ANativeWindow pointer as jlong (i64).
/// This is the NDK-recommended way to obtain ANativeWindow from a Surface.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_torvox_bridge_NativeWindow_getNativeWindowPtr(
    env: JNIEnvPtr,
    _class: JClassPtr,
    surface: JObjectPtr,
) -> i64 {
    if env.is_null() || surface.is_null() {
        log::error!("JNI getNativeWindowPtr: null env or surface pointer");
        return 0;
    }
    // SAFETY: ANativeWindow_fromSurface is a public NDK function that
    // returns a new reference to the ANativeWindow. env and surface are
    // valid JNI pointers (checked non-null above). The returned pointer
    // is valid until released via ANativeWindow_release.
    let ptr = unsafe { ANativeWindow_fromSurface(env, surface) };
    log::debug!("JNI ANativeWindow_fromSurface returned: {:p}", ptr);
    ptr as i64
}
