//! Custom `log::Log` implementation for Android that writes to logcat.
//!
//! Initialised from Kotlin via JNI (`Java_..._initLogger`).
//!
//! Messages longer than logcat's per-entry payload cap (4068 bytes) are
//! split into chunks (round-227 T2) — logcat silently truncates any entry
//! past that limit, so the tail of a long log line would otherwise be
//! lost. Chunking math lives in [`crate::log_chunk`] and is unit-tested
//! on the host.
//!
//! Replaces the previous `android_logger::init_once()` call in `bridge.rs`.
//!
//! # Requirements
//! - NFR-025 — Unified logging infrastructure (logcat; round-227 T2:
//!   logcat-only, no file sink — mirrors termux-kotlin Logger)

#![cfg(target_os = "android")]

use core::ffi::c_char;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::ffi::CString;

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
        // Split long messages so logcat does not truncate them (T2).
        for chunk in crate::log_chunk::chunk_message(tag, &msg) {
            let msg_c = CString::new(chunk.as_str()).unwrap_or_else(|_| {
                // SAFETY: Vec::<u8>::new() contains no NUL bytes
                CString::new(Vec::<u8>::new()).expect("empty vec has no NUL")
            });
            // SAFETY: __android_log_write is a public NDK function; the
            // pointers point to valid NUL-terminated C strings.
            unsafe {
                __android_log_write(prio, tag_c.as_ptr(), msg_c.as_ptr());
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: AndroidLogger = AndroidLogger;

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

/// Route panics from any Rust thread into the logging system (logcat)
/// with a captured backtrace.
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

// ── JNI exports ─────────────────────────────────────────────────────────
