//! Android JNI 桥接与 NDK 集成。

pub mod ffi;
pub(crate) mod keymap;
pub(crate) mod text_utils;

#[cfg(target_os = "android")]
pub mod logging;

/// 日志等级：与 Kotlin 侧 `LogLevel` 一一对应（数值即 Android 优先级）。
///
/// `OFF` 用 `i32::MAX` 表示「什么都不输出」——Android 没有这一级，
/// 但 `log::LevelFilter::Off` 有。
fn level_filter(priority: i32) -> log::LevelFilter {
    match priority {
        i32::MAX => log::LevelFilter::Off,
        6 => log::LevelFilter::Error,
        5 => log::LevelFilter::Warn,
        4 => log::LevelFilter::Info,
        3 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    }
}

/// 设置日志等级（宿主按设置项调用）。
///
/// 与 Kotlin 侧的 `LogUtil` 共用同一个等级值：两边各自过滤时，
/// 原生日志与 Kotlin 日志才会一致地出现在同一个等级下。
pub(crate) fn set_log_level(priority: i32) {
    let filter = level_filter(priority);
    log::set_max_level(filter);
    log::info!("log level set to {filter}");
}

/// 参照 wgpu-in-app 的 `init_logger()` 模式：将 `wgpu_hal` / naga 降到 Error，
/// 避免逐帧后端日志淹没 logcat；其余模块仍为 Debug。
#[cfg(any(target_os = "android", test))]
pub(crate) fn module_filtered(metadata: &log::Metadata<'_>) -> bool {
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
        // wgpu_hal 降到 Error（逐帧噪声）。
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
        // LevelFilter::Info 意为 Info 及以上；wgpu_core 的 Debug/Trace 被抑制。
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
