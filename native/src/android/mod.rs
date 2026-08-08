//! Android JNI bridge and NDK integration.
//!
//! # Requirements
//! - FR-049 — JNI NDK bridge: direct JNI exports for the Android platform

pub mod ffi;

#[cfg(target_os = "android")]
pub mod logging;

/// wgpu-in-app init_logger() pattern (wgpu-in-app/src/lib.rs:15-40): keep
/// wgpu_hal / naga at Error so the logcat is not flooded by per-frame
/// backend noise, while the rest of the app stays at Debug.
#[cfg(any(target_os = "android", test))]
pub(crate) fn module_filtered(metadata: &log::Metadata) -> bool {
    if metadata.level() <= log::Level::Debug {
        let target = metadata.target();
        if target.starts_with("wgpu_hal") || target.starts_with("naga") {
            return metadata.level() <= log::Level::Error;
        }
        if target.starts_with("wgpu_core") {
            return metadata.level() <= log::Level::Info;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(level: log::Level, target: &'static str) -> log::Metadata<'static> {
        log::Metadata::builder().level(level).target(target).build()
    }

    #[test]
    fn wgpu_hal_debug_is_filtered() {
        // wgpu-in-app keeps wgpu_hal at Error (per-frame noise).
        assert!(!module_filtered(&metadata(
            log::Level::Debug,
            "wgpu_hal::gles::egl"
        )));
        assert!(!module_filtered(&metadata(log::Level::Info, "wgpu_hal")));
        assert!(module_filtered(&metadata(log::Level::Error, "wgpu_hal")));
    }

    #[test]
    fn naga_debug_is_filtered() {
        assert!(!module_filtered(&metadata(
            log::Level::Debug,
            "naga::front::wgsl"
        )));
        assert!(module_filtered(&metadata(log::Level::Error, "naga")));
    }

    #[test]
    fn wgpu_core_info_is_allowed() {
        // LevelFilter::Info means Info and above (Error/Warn/Info); Debug
        // and Trace from wgpu_core are suppressed.
        assert!(module_filtered(&metadata(
            log::Level::Info,
            "wgpu_core::device"
        )));
        assert!(!module_filtered(&metadata(
            log::Level::Debug,
            "wgpu_core::device"
        )));
        assert!(module_filtered(&metadata(log::Level::Error, "wgpu_core")));
    }

    #[test]
    fn app_modules_stay_debug() {
        assert!(module_filtered(&metadata(
            log::Level::Debug,
            "native::render::context"
        )));
        assert!(module_filtered(&metadata(
            log::Level::Debug,
            "ghostty_terminal"
        )));
    }
}
