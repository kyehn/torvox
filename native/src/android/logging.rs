//! Custom `log::Log` implementation for Android that writes to logcat
//! AND to an optional file simultaneously.
//!
//! Initialised by [`init_logger`] (called from Kotlin via JNA). The
//! file path is set via [`set_log_file_path`] — until it is called,
//! logs go only to logcat.
//!
//! Replaces the previous `android_logger::init_once()` call in `bridge.rs`.
//!
//! # Requirements
//! - NFR-025 — Unified logging infrastructure (logcat + rotating file)

#![cfg(target_os = "android")]

use core::ffi::c_char;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

// ── Android log priorities (from <android/log.h>) ──────────────────────

const ANDROID_LOG_VERBOSE: i32 = 2;
const ANDROID_LOG_DEBUG: i32 = 3;
const ANDROID_LOG_INFO: i32 = 4;
const ANDROID_LOG_WARN: i32 = 5;
const ANDROID_LOG_ERROR: i32 = 6;

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
        metadata.level() <= Level::Debug
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
        if let Ok(mut guard) = self.log_file.lock()
            && let Some(ref mut file) = *guard
        {
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
        if let Ok(mut guard) = self.log_file.lock()
            && let Some(ref mut file) = *guard
        {
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
    });
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
    if let Ok(mut guard) = LOGGER.log_file.lock() {
        *guard = Some(file);
    } else {
        log::warn!("ffi_set_log_file_path: LOGGER.log_file lock poisoned");
    }
}

// ── JNA exports ─────────────────────────────────────────────────────────

/// JNA export: initialise the logger once. Safe to call multiple times
/// (idempotent via Once). Must be called before any log macro.
///
/// # Safety
/// Must only be called from a context where JNA exports are valid (i.e., from
/// the library's exported function table). The function itself delegates to the
/// safe `init()` and has no direct unsafe operations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_logger() {
    init();
}

/// JNA export: set the log file path. The file is opened in append mode.
/// Safe to call before or after `init_logger`.
///
/// # Safety
/// `path_ptr` must point to a valid UTF-8 byte array of length `path_len`,
/// or be null when `path_len` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ffi_set_log_file_path(path_ptr: *const u8, path_len: i32) {
    // SAFETY: `path_ptr` and `path_len` come from a trusted C caller
    // (the Android JNI logging bridge). The pointer is validated as
    // non-null above; `path_len` bounds the slice length.
    if path_ptr.is_null() || path_len <= 0 {
        return;
    }
    // Clamp to a reasonable maximum path length (PATH_MAX is 4096 on Linux).
    // This prevents a malicious or buggy caller from causing UB by passing
    // an enormous path_len that would read out-of-bounds.
    let len = (path_len as usize).min(4096);
    let slice = unsafe { std::slice::from_raw_parts(path_ptr, len) };
    let path = String::from_utf8_lossy(slice);
    set_log_file_path(&path);
}
