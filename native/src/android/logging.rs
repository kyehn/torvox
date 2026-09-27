//! 自定义 `log::Log` 实现，写入 logcat，由 Kotlin 经 JNI 初始化。

#![cfg(target_os = "android")]

use core::ffi::c_char;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::ffi::CString;

// ── Android 日志优先级（来自 <android/log.h>） ─────────────────────────

const ANDROID_LOG_VERBOSE: i32 = 2;
const ANDROID_LOG_DEBUG: i32 = 3;
const ANDROID_LOG_INFO: i32 = 4;
const ANDROID_LOG_WARN: i32 = 5;
const ANDROID_LOG_ERROR: i32 = 6;

// SAFETY: `__android_log_write` 是 liblog.so 公开的 NDK 日志函数。`tag` 与 `text`
// 指针须为 NUL 结尾的 C 字符串且在调用期间有效；所有调用点传入的 CString/str::as_ptr
// 均指向生命周期长于本次调用的字符串（见下方 `log`）。
#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(prio: i32, tag: *const c_char, text: *const c_char) -> i32;
}

fn level_to_android(level: Level) -> i32 {
    match level {
        Level::Error => ANDROID_LOG_ERROR,
        Level::Warn => ANDROID_LOG_WARN,
        Level::Info => ANDROID_LOG_INFO,
        Level::Debug => ANDROID_LOG_DEBUG,
        Level::Trace => ANDROID_LOG_VERBOSE,
    }
}

// ── Logger ─────────────────────────────────────────────────────────────

struct AndroidLogger;

impl Log for AndroidLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug && crate::android::module_filtered(metadata)
    }

    fn log(&self, record: &Record) {
        let tag = record.target();
        let msg = format!("{}", record.args());
        let prio = level_to_android(record.level());
        let tag_c = CString::new(tag).unwrap_or_else(|_| {
            // SAFETY: "Rust" has no interior NUL bytes
            CString::new("Rust").expect("hardcoded string without NUL")
        });
        let msg_c = CString::new(msg).unwrap_or_else(|_| {
            // SAFETY: Vec::<u8>::new() contains no NUL bytes
            CString::new(Vec::<u8>::new()).expect("empty vec has no NUL")
        });
        // SAFETY: `__android_log_write` 是公开 NDK 函数，指针指向有效的 NUL 结尾 C 字符串。
        unsafe {
            __android_log_write(prio, tag_c.as_ptr(), msg_c.as_ptr());
        }
    }

    fn flush(&self) {}
}

static LOGGER: AndroidLogger = AndroidLogger;

/// 必须且只需调用一次（经 [`std::sync::Once`] 幂等）。
pub(crate) fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER).expect("Logger already set");
        log::set_max_level(LevelFilter::Debug);
        install_panic_hook();
    });
}

/// 将任意 Rust 线程的 panic 连同 backtrace 导入日志系统。
///
/// 无此钩子时，非 JNI 线程（如 `session.rs` 起的 PTY 读取线程，无 `catch_unwind`）
/// 的 panic 只进 stderr，在 Android 上不可见：线程静默死亡且丢失崩溃现场；
/// Kotlin 的 `Thread.setDefaultUncaughtExceptionHandler` 不覆盖 Rust 线程。
/// `Backtrace::force_capture()` 在 release 构建下无需 `RUST_BACKTRACE` 亦生效。
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("panic: {info}\n{backtrace}");
    }));
}
