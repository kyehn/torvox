//! Custom `log::Log` implementation for Android that writes to logcat
//! AND to an optional file simultaneously.
//!
//! Initialised by [`torvox_init_logger`] (called from Kotlin via JNA). The
//! file path is set via [`torvox_set_log_file_path`] — until it is called,
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

struct TorvoxLogger {
    log_file: Mutex<Option<std::fs::File>>,
}

impl Log for TorvoxLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        // Always write to logcat
        let tag = record.target();
        let msg = format!("{}", record.args());
        let prio = level_to_android(record.level());
        let tag_c = CString::new(tag).unwrap_or(CString::new("TorvoxRust").unwrap());
        let msg_c =
            CString::new(msg.as_str()).unwrap_or_else(|_| CString::new(Vec::<u8>::new()).unwrap());
        // SAFETY: __android_log_write is a public NDK function; the pointers
        // point to valid NUL-terminated C strings.
        unsafe {
            __android_log_write(prio, tag_c.as_ptr(), msg_c.as_ptr());
        }

        // Write to file if a log file was configured
        if let Ok(mut guard) = self.log_file.lock() {
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
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.log_file.lock() {
            if let Some(ref mut file) = *guard {
                let _ = file.flush();
            }
        }
    }
}

static LOGGER: TorvoxLogger = TorvoxLogger {
    log_file: Mutex::new(None),
};

/// Must be called exactly once (idempotent via [`std::sync::Once`]).
/// Replaces the `android_logger::init_once()` call that was previously in
/// [`TorvoxBridge::new`](crate::bridge::TorvoxBridge::new).
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER).expect("TorvoxLogger already set");
        log::set_max_level(LevelFilter::Debug);
    });
}

/// Open (or re-open) the file backing the log-file side of [`LOGGER`].
/// The file is opened in append mode; it is created if it does not exist.
/// Kotlin calls this after figuring out the correct log directory.
pub fn set_log_file_path(path: &str) {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_or_else(|e| {
            // If we cannot open the file, log to logcat and carry on.
            let msg = CString::new(format!("TorvoxLogger: failed to open log file {path}: {e}"))
                .unwrap_or_default();
            unsafe {
                __android_log_write(ANDROID_LOG_ERROR, c"TorvoxRust".as_ptr(), msg.as_ptr());
            }
            // Return a /dev/null equivalent so writes are no-ops
            OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .expect("cannot open /dev/null")
        });
    if let Ok(mut guard) = LOGGER.log_file.lock() {
        *guard = Some(file);
    }
}

// ── JNA exports ─────────────────────────────────────────────────────────

/// JNA export: initialise the logger once. Safe to call multiple times
/// (idempotent via Once). Must be called before any log macro.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_init_logger() {
    init();
}

/// JNA export: set the log file path. The file is opened in append mode.
/// Safe to call before or after `torvox_init_logger`.
///
/// # Safety
/// `path_ptr` must point to a valid UTF-8 byte array of length `path_len`,
/// or be null when `path_len` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torvox_set_log_file_path(path_ptr: *const u8, path_len: i32) {
    if path_ptr.is_null() || path_len <= 0 {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(path_ptr, path_len as usize) };
    let path = String::from_utf8_lossy(slice);
    set_log_file_path(&path);
}
