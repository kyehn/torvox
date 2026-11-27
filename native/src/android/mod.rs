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
        2 => log::LevelFilter::Trace,
        // 未知取值退回信息级：Trace 是**最宽松**的一级，把它当默认会让一次
        // 笔误（或版本错配）直接变成满屏日志。未知取值本不该出现，故出声。
        _ => {
            log::warn!("unknown log level priority {priority}; falling back to info");
            log::LevelFilter::Info
        }
    }
}

/// 设置日志等级（宿主按设置项调用）。
///
/// 与 Kotlin 侧的 `LogUtil` 共用同一个等级值：两边各自过滤时，
/// 原生日志与 Kotlin 日志才会一致地出现在同一个等级下。
pub(crate) fn set_log_level(priority: i32) {
    let filter = level_filter(priority);
    // 先记后设：等级切到「关闭」后，`log::info!` 会被**自己设的等级**过滤掉，
    // 于是「已切到关闭」这条确认永远发不出去。确认必须在切换前落。
    log::info!("log level set to {filter}");
    log::set_max_level(filter);
}

/// 一条诊断是否应写入 logcat：先按宿主设定的等级门槛过滤，再按模块噪声门槛过滤。
///
/// 等级门槛必须取宿主当前设定（`LevelFilter`），不能硬编码为 Debug：否则宿主在设置里
/// 选「详细」也放不出本仓的 trace 行，两侧等级语义便不再一致。
#[cfg(any(target_os = "android", test))]
pub(crate) fn log_enabled(metadata: &log::Metadata<'_>, filter: log::LevelFilter) -> bool {
    metadata.level() <= filter && module_filtered(metadata)
}

/// 参照 wgpu-in-app 的 `init_logger()` 模式：将 `wgpu_hal` / naga 降到 Error，
/// `wgpu_core` 降到 Info，避免逐帧后端日志淹没 logcat；其余模块只受宿主等级门槛约束。
/// 噪声门槛 MUST 对包括 Trace 在内的每一级生效，否则宿主选「详细」时上游噪声会趁机放行。
#[cfg(any(target_os = "android", test))]
pub(crate) fn module_filtered(metadata: &log::Metadata<'_>) -> bool {
    let target = metadata.target();
    if target.starts_with("wgpu_hal") || target.starts_with("naga") {
        return metadata.level() <= log::Level::Error;
    }
    if target.starts_with("wgpu_core") {
        return metadata.level() <= log::Level::Info;
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

    /// 宿主把等级设为「详细」时，原生侧的 trace 级诊断 MUST 放行。
    ///
    /// 判别力：写入门限一旦硬编码为 `Debug`，宿主选「详细」也放不出任何
    /// trace 行，两侧的等级语义便不再一致（`logcat-hygiene` 的两侧同步条）。
    #[test]
    fn verbose_level_releases_native_trace_diagnostics() {
        assert!(log_enabled(
            &metadata(log::Level::Trace, "native::render"),
            log::LevelFilter::Trace
        ));
        assert!(!log_enabled(
            &metadata(log::Level::Trace, "native::render"),
            log::LevelFilter::Debug
        ));
    }

    #[test]
    fn off_level_blocks_every_level() {
        for level in [
            log::Level::Error,
            log::Level::Warn,
            log::Level::Info,
            log::Level::Debug,
            log::Level::Trace,
        ] {
            assert!(!log_enabled(
                &metadata(level, "native::render"),
                log::LevelFilter::Off
            ));
        }
    }

    /// 上游噪声门限 MUST 不因「详细」档位而放开：wgpu_hal 与 naga 只放行
    /// 错误级，wgpu_core 只放行信息级及以上；放开的只是本仓诊断。
    #[test]
    fn upstream_trace_stays_filtered() {
        assert!(!module_filtered(&metadata(
            log::Level::Trace,
            "wgpu_hal::vulkan"
        )));
        assert!(!module_filtered(&metadata(
            log::Level::Trace,
            "naga::front::wgsl"
        )));
        assert!(!module_filtered(&metadata(
            log::Level::Trace,
            "wgpu_core::device"
        )));
        assert!(module_filtered(&metadata(
            log::Level::Trace,
            "native::render"
        )));
    }

    /// 等级映射：Android 优先级与 `log::LevelFilter` 一一对应。
    ///
    /// 判别力：`Trace` 是**最宽松**的一级，未知取值若映射到它，一次笔误就把
    /// 日志全开；OFF 必须真正关掉一切。
    #[test]
    fn level_filter_maps_android_priorities() {
        assert_eq!(level_filter(i32::MAX), log::LevelFilter::Off);
        assert_eq!(level_filter(6), log::LevelFilter::Error);
        assert_eq!(level_filter(5), log::LevelFilter::Warn);
        assert_eq!(level_filter(4), log::LevelFilter::Info);
        assert_eq!(level_filter(3), log::LevelFilter::Debug);
        assert_eq!(level_filter(2), log::LevelFilter::Trace);
        // 未知取值退回信息级，不退回最宽松的一级。
        assert_eq!(level_filter(0), log::LevelFilter::Info);
        assert_eq!(level_filter(99), log::LevelFilter::Info);
    }
}
