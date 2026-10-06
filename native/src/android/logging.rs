//! 自定义 `log::Log` 实现，写入 logcat，由 `JNI_OnLoad` 初始化。

#![cfg(target_os = "android")]

use core::ffi::{c_char, c_void};
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

/// 错误策略的「输出日志并崩溃退出」的日志安装保证：`JNI_OnLoad` 由 VM 在加载本库时
/// 调用，早于任何 JNI 方法，因此 logger 在第一个 `log::*` 调用之前即已就位。
///
/// 曾经的 `NativeBridge.initLogger` 由 Kotlin 的独立线程异步调用，启动数秒内的致命
/// 退出会抢在该线程之前，经 `log` 门面的原因被静默丢弃，现场只剩无符号 tombstone
/// （2026-10-02 真机启动崩溃即为此）。`#[unsafe(no_mangle)]` 是 JNI 的 ABI 约定。
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(_vm: *mut c_void, _reserved: *mut c_void) -> i32 {
    init();
    JNI_VERSION_1_6
}

/// `JNI_OnLoad` 返回的 JNI 版本（`jni.h` 的 `JNI_VERSION_1_6`）：返回任何有效版本即
/// 使 VM 为本库缓存 JNIEnv，不得返回 `JNI_ERR`，那会拒绝加载本库。
const JNI_VERSION_1_6: i32 = 0x0001_0006;

// ── Logger ─────────────────────────────────────────────────────────────

struct AndroidLogger;

impl Log for AndroidLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug && crate::android::module_filtered(metadata)
    }

    fn log(&self, record: &Record) {
        let tag = record.target();
        let message = format!("{}", record.args());
        let prio = level_to_android(record.level());
        let tag_c = CString::new(tag).unwrap_or_else(|_| {
            // SAFETY: "Rust" 不含内部 NUL 字节
            CString::new("Rust").expect("hardcoded string without NUL")
        });
        let message_c = CString::new(message).unwrap_or_else(|_| {
            // SAFETY: `Vec::<u8>::new()` 不含 NUL 字节
            CString::new(Vec::<u8>::new()).expect("empty vec has no NUL")
        });
        // SAFETY: `__android_log_write` 是公开 NDK 函数，指针指向有效的 NUL 结尾 C 字符串。
        unsafe {
            __android_log_write(prio, tag_c.as_ptr(), message_c.as_ptr());
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
