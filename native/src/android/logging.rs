//! Custom `log::Log` implementation for Android that writes to logcat
//! AND to an optional file simultaneously.
//!
//! Initialised from Kotlin via JNI (`Java_..._initLogger`). The file
//! path is set via [`set_log_file_path`] — until it is called, logs go
//! only to logcat.
//!
//! Replaces the previous `android_logger::init_once()` call in `bridge.rs`.
//!
//! # Requirements
//! - NFR-025 — Unified logging infrastructure (logcat + rotating file)

#![cfg(target_os = "android")]

use core::ffi::c_char;
use log::{Level, LevelFilter, Log, Metadata, Record};
use parking_lot::Mutex;
use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::Write;

// ── Android log priorities (from <android/log.h>) ──────────────────────

const ANDROID_LOG_VERBOSE: i32 = 2;
const ANDROID_LOG_DEBUG: i32 = 3;
const ANDROID_LOG_INFO: i32 = 4;
const ANDROID_LOG_WARN: i32 = 5;
const ANDROID_LOG_ERROR: i32 = 6;

// SAFETY: `__android_log_write` is the public NDK logging function from
// liblog.so. The `tag` and `text` pointers must be NUL-terminated C strings
// valid for the duration of the call; all call sites pass CString/str::as_ptr
// into strings that outlive the call (see `log` below).
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

// ── Logger ──────────────────────────────────────────────────────────────

struct AndroidLogger {
    log_file: Mutex<Option<std::fs::File>>,
}

impl Log for AndroidLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug && crate::android::module_filtered(metadata)
    }

    fn log(&self, record: &Record) {
        // Always write to logcat
        let tag = record.target();
        let msg = format!("{}", record.args());
        let prio = level_to_android(record.level());
        let tag_c = CString::new(tag).unwrap_or_else(|_| {
            // SAFETY: "Rust" has no interior NUL bytes
            CString::new("Rust").expect("hardcoded string without NUL")
        });
        let msg_c = CString::new(msg.as_str()).unwrap_or_else(|_| {
            // SAFETY: Vec::<u8>::new() contains no NUL bytes
            CString::new(Vec::<u8>::new()).expect("empty vec has no NUL")
        });
        // SAFETY: __android_log_write is a public NDK function; the pointers
        // point to valid NUL-terminated C strings.
        unsafe {
            __android_log_write(prio, tag_c.as_ptr(), msg_c.as_ptr());
        }

        // Write to file if a log file was configured
        let mut guard = self.log_file.lock();
        if let Some(ref mut file) = *guard {
            let _ = writeln!(
                file,
                "D {} {}:{}: {}",
                record.level(),
                tag,
                record.line().unwrap_or(0),
                msg,
            );
        }
    }

    fn flush(&self) {
        let mut guard = self.log_file.lock();
        if let Some(ref mut file) = *guard {
            let _ = file.flush();
        }
    }
}

static LOGGER: AndroidLogger = AndroidLogger {
    log_file: Mutex::new(None),
};

/// Must be called exactly once (idempotent via [`std::sync::Once`]).
/// Replaces the `android_logger::init_once()` call that was previously in
/// [`NativeBridge::new`](crate::bridge::NativeBridge::new).
pub(crate) fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER).expect("Logger already set");
        log::set_max_level(LevelFilter::Debug);
        install_panic_hook();
    });
}

/// Route panics from any Rust thread into the logging system (logcat +
/// optional file) with a captured backtrace.
///
/// Without this hook a panic in a non-JNI thread — e.g. the PTY reader
/// thread spawned in `session.rs`, which has no `catch_unwind` — prints
/// to stderr only, which is invisible on Android: the thread dies
/// silently and the crash site is lost. Kotlin's
/// `Thread.setDefaultUncaughtExceptionHandler` does not cover Rust
/// threads.
///
/// `Backtrace::force_capture()` works in release builds without
/// `RUST_BACKTRACE` (Rust 1.65+).
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("panic: {info}\n{backtrace}");
    }));
}

/// Open (or re-open) the file backing the log-file side of [`LOGGER`].
/// The file is opened in append mode; it is created if it does not exist.
/// Kotlin calls this after figuring out the correct log directory.
pub(crate) fn set_log_file_path(path: &str) {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_or_else(|e| {
            // If we cannot open the file, log to logcat and carry on.
            let msg = CString::new(format!("Log: failed to open log file {path}: {e}"))
                .unwrap_or_default();
            // SAFETY: `__android_log_write` is a documented NDK function from
            // `liblog.so`. Both `c"Rust"` and `msg.as_ptr()` are valid NUL-terminated
            // CString pointers, guaranteed by the `CString::new()` constructor above.
            unsafe {
                __android_log_write(ANDROID_LOG_ERROR, c"Rust".as_ptr(), msg.as_ptr());
            }
            OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .expect("cannot open /dev/null")
        });
    *LOGGER.log_file.lock() = Some(file);
}

// ── JNI exports ─────────────────────────────────────────────────────────
