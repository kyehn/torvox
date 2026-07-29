//! JNI FFI bridge — replaces boltffi/JNA with direct JNI.
//!
//! This module exports `extern "system"` JNI functions called from Kotlin.
//! Each function follows the JNI naming convention:
//! `Java_io_term_bridge_NativeBridge_<methodName>`
//!
//! Session lifecycle:
//! - `initSession()` creates a `Session` and registers it globally
//! - `destroySession()` shuts down the session and removes it
//! - The active session is set via `switchSession()`
//!
//! Events (bell, title, clipboard, exit) are pushed into a global queue
//! and drained by Kotlin via `pollEvent()`.

// Style: nested-if is an error-handling idiom for JNI; unused-mut is a
// false positive with jni crate (new_string needs &mut self on some configs).
#![allow(clippy::collapsible_if, unused_mut, clippy::type_complexity)]

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
#[cfg(feature = "mcp")]
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::lock_util::lock_or_recover;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{JNI_TRUE, jboolean, jint, jlong, jstring};

use crate::terminal::ShellEnv;
use crate::terminal::session::Session;
use std::sync::Arc;

// ══════════════════════════════════════════════════════════════════════════
// NDK FFI declarations
// ══════════════════════════════════════════════════════════════════════════

/// NDK raw-pointer type for JNI invocation API.
#[cfg(target_os = "android")]
type JNIEnvPtr = *mut std::ffi::c_void;
#[cfg(target_os = "android")]
type JObjectPtr = *mut std::ffi::c_void;

// SAFETY: These are publicly documented Android NDK functions obtained from
// `libandroid.so`. The pointer arguments must be valid JNI environment and
// jobject references, which callers guarantee by deriving them from JNI entry
// points that receive valid arguments from the Kotlin/Java runtime.
#[cfg(target_os = "android")]
#[link(name = "android")]
unsafe extern "C" {
    pub(crate) fn ANativeWindow_fromSurface(
        env: JNIEnvPtr,
        surface: JObjectPtr,
    ) -> *mut std::ffi::c_void;
    pub(crate) fn ANativeWindow_release(window: *mut std::ffi::c_void);
}

// ══════════════════════════════════════════════════════════════════════════
// Session Registry
// ══════════════════════════════════════════════════════════════════════════

/// A registered session with its ID and thread-safe handle.
struct SessionEntry {
    session: Arc<Mutex<Session>>,
    /// Last time the session state was saved (for periodic persistence).
    last_save: Mutex<Instant>,
}

/// Global session registry. Thread-safe via Mutex.
static SESSION_REGISTRY: LazyLock<Mutex<HashMap<u64, SessionEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

static ACTIVE_SESSION_ID: AtomicU64 = AtomicU64::new(0);

fn next_session_id() -> u64 {
    NEXT_SESSION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ══════════════════════════════════════════════════════════════════════════
// Event Queue
// ══════════════════════════════════════════════════════════════════════════

/// Events emitted by sessions and consumed by Kotlin via `pollEvent()`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Event {
    Title {
        session_id: u64,
        title: String,
    },
    Bell {
        session_id: u64,
    },
    Clipboard {
        session_id: u64,
        text: String,
    },
    Exit {
        session_id: u64,
        code: i32,
    },
    /// Request Kotlin to show a dialog (input/confirm/select).
    /// Kotlin responds by calling `dialogResult()` JNI.
    #[cfg(feature = "mcp")]
    ShowDialog {
        session_id: u64,
        request_id: u64,
        dialog_type: String,
        title: String,
        message: String,
    },
    /// Request Kotlin to show a file picker (Android SAF / desktop).
    /// Kotlin responds by calling `filePicked()` JNI.
    #[cfg(feature = "mcp")]
    PickFile {
        session_id: u64,
        request_id: u64,
        starting_path: String,
        filter: String,
    },
}

/// Global event queue. Events are rare (<1 Hz), so Mutex contention is fine.
static EVENT_QUEUE: LazyLock<Mutex<Vec<Event>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Monotonic counter for user-input request IDs.
#[cfg(feature = "mcp")]
#[allow(dead_code)]
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Registry for pending user-input requests (dialog / file picker).
/// Keyed by (session_id, request_id) to support multiple concurrent dialogs.
#[cfg(feature = "mcp")]
#[allow(dead_code)]
static REQUEST_REGISTRY: LazyLock<
    Mutex<HashMap<(u64, u64), tokio::sync::oneshot::Sender<String>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Push an event into the global queue (called from Session polling).
pub(crate) fn push_event(event: Event) {
    lock_or_recover(&EVENT_QUEUE, "ffi: push_event").push(event);
}

/// Save interval for periodic persistence.
const SAVE_INTERVAL_SECS: u64 = 30;

/// Periodically save session state if enough time has elapsed.
/// Called after feedPty / process_output.
pub(crate) fn maybe_save_session(
    session: &crate::terminal::session::Session,
    last_save: &Mutex<Instant>,
) {
    let elapsed = last_save
        .lock()
        .map(|t| t.elapsed())
        .unwrap_or(Duration::MAX);
    if elapsed >= Duration::from_secs(SAVE_INTERVAL_SECS) {
        if session.save_session() {
            if let Ok(mut t) = last_save.lock() {
                *t = Instant::now();
            }
        }
    }
}

/// Register a pending user-input request and return the receiver.
/// Kotlin will send the response via `dialogResult` JNI.
#[cfg(feature = "mcp")]
#[allow(dead_code)]
pub(crate) fn register_request(session_id: u64) -> (u64, tokio::sync::oneshot::Receiver<String>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut registry) = REQUEST_REGISTRY.lock() {
        registry.insert((session_id, request_id), tx);
    }
    (request_id, rx)
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: initSession
// ══════════════════════════════════════════════════════════════════════════

/// Create a new terminal session with the default shell.
/// Returns the session ID (jlong) on success, or 0 on failure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_initSession(
    mut env: JNIEnv,
    _class: JClass,
    rows: jint,
    cols: jint,
) -> jlong {
    let rows = rows as u32;
    let cols = cols as u32;

    match Session::spawn("", rows, cols, &ShellEnv::default()) {
        Ok(mut session) => {
            let id = next_session_id();
            session.restore_session();
            let entry = SessionEntry {
                session: Arc::new(Mutex::new(session)),
                last_save: Mutex::new(Instant::now()),
            };

            let mut registry = lock_or_recover(&SESSION_REGISTRY, "ffi: initSession");
            registry.insert(id, entry);
            if ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                ACTIVE_SESSION_ID.store(id, std::sync::atomic::Ordering::Relaxed);
            }

            log::info!("FFI: initSession -> id={}", id);
            id as jlong
        }
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("initSession failed: {e}"),
            );
            0
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: destroySession
// ══════════════════════════════════════════════════════════════════════════

/// Destroy a session by ID. Returns true on success.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_destroySession(
    _env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    let id = session_id as u64;
    let removed = lock_or_recover(&SESSION_REGISTRY, "ffi: destroySession")
        .remove(&id)
        .is_some();

    if removed {
        // If we removed the active session, clear the active ID
        ACTIVE_SESSION_ID
            .compare_exchange(
                id,
                0,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .ok();
        log::info!("FFI: destroySession id={}", id);
        JNI_TRUE
    } else {
        log::warn!("FFI: destroySession id={} not found", id);
        0
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: switchSession
// ══════════════════════════════════════════════════════════════════════════

/// Switch the active session. Returns true if the session exists.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_switchSession(
    _env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    let id = session_id as u64;
    let exists = lock_or_recover(&SESSION_REGISTRY, "ffi: switchSession").contains_key(&id);

    if exists {
        ACTIVE_SESSION_ID.store(id, std::sync::atomic::Ordering::Relaxed);
        JNI_TRUE
    } else {
        log::warn!("FFI: switchSession id={} not found", id);
        0
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: getSessionCount
// ══════════════════════════════════════════════════════════════════════════

/// Returns the number of active sessions.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_getSessionCount(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    SESSION_REGISTRY.lock().map(|r| r.len() as i32).unwrap_or(0)
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: resize
// ══════════════════════════════════════════════════════════════════════════

/// Resize the specified session. Throws RuntimeException if session not found.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_resize(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    rows: jint,
    cols: jint,
) {
    let id = session_id as u64;
    let registry = lock_or_recover(&SESSION_REGISTRY, "ffi: resize");
    if let Some(entry) = registry.get(&id) {
        match entry.session.lock() {
            Ok(mut session) => {
                if let Err(e) = session.resize(rows as u32, cols as u32) {
                    if let Err(e) =
                        env.throw_new("java/lang/RuntimeException", format!("resize: failed: {e}"))
                    {
                        log::error!("resize: throw_new failed: {e}");
                    }
                }
            }
            Err(_poisoned) => {
                let msg = "resize: session lock poisoned";
                log::error!("{msg}");
                let _ = env.throw_new("java/lang/RuntimeException", msg);
            }
        }
        return;
    }
    let _ = env.throw_new("java/lang/RuntimeException", "resize: session not found");
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: feedPty
// ══════════════════════════════════════════════════════════════════════════

/// Write raw bytes to the PTY of the specified session.
/// Throws RuntimeException if session not found or write fails.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_feedPty(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    data: JString,
) {
    let id = session_id as u64;

    let input: String = match env.get_string(&data) {
        Ok(s) => s.into(),
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                "feedPty: failed to read input string",
            );
            return;
        }
    };

    let registry = lock_or_recover(&SESSION_REGISTRY, "ffi: feedPty");
    if let Some(entry) = registry.get(&id) {
        match entry.session.lock() {
            Ok(mut session) => {
                if let Err(e) = session.write(input.as_bytes()) {
                    if let Err(e) = env.throw_new(
                        "java/lang/RuntimeException",
                        format!("feedPty: write failed: {e}"),
                    ) {
                        log::error!("feedPty: throw_new failed: {e}");
                    }
                } else {
                    maybe_save_session(&session, &entry.last_save);
                }
                return;
            }
            Err(_poisoned) => {
                let msg = "feedPty: session lock poisoned";
                log::error!("{msg}");
                if let Err(e) = env.throw_new("java/lang/RuntimeException", msg) {
                    log::error!("feedPty: throw_new failed: {e}");
                }
                return;
            }
        }
    }
    let _ = env.throw_new("java/lang/RuntimeException", "feedPty: session not found");
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: writeKey
// ══════════════════════════════════════════════════════════════════════════

/// Encode and submit a key event to the session.
/// `key` is the key name (e.g., "a", "Enter", "Escape").
/// `mods` is a bitmask of modifiers (1=shift, 2=alt, 4=ctrl, 8=meta, 16=super).
/// `text` is optional composed text (for IME input).
/// Throws RuntimeException if the session is not found or write fails.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_writeKey(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    key: JString,
    _mods: jint,
    text: JString,
) {
    let id = session_id as u64;

    let key_str: String = match env.get_string(&key) {
        Ok(s) => s.into(),
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                "writeKey: failed to read key string",
            );
            return;
        }
    };
    let has_text = !text.is_null();

    let registry = lock_or_recover(&SESSION_REGISTRY, "ffi: writeKey");
    if let Some(entry) = registry.get(&id) {
        match entry.session.lock() {
            Ok(mut session) => {
                let result = if has_text {
                    match env.get_string(&text) {
                        Ok(t) => session.write(t.to_bytes()),
                        Err(_) => {
                            let _ = env.throw_new(
                                "java/lang/RuntimeException",
                                "writeKey: failed to read text string",
                            );
                            return;
                        }
                    }
                } else {
                    session.write(key_str.as_bytes())
                };
                if let Err(e) = result {
                    if let Err(e) = env.throw_new(
                        "java/lang/RuntimeException",
                        format!("writeKey: write failed: {e}"),
                    ) {
                        log::error!("writeKey: throw_new failed: {e}");
                    }
                }
            }
            Err(_poisoned) => {
                let msg = "writeKey: session lock poisoned";
                log::error!("{msg}");
                let _ = env.throw_new("java/lang/RuntimeException", msg);
            }
        }
        return;
    }
    let _ = env.throw_new("java/lang/RuntimeException", "writeKey: session not found");
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: pollEvent
// ══════════════════════════════════════════════════════════════════════════

/// Poll the global event queue. Returns the oldest pending event as a
/// JSON string, or null if no events are pending.
///
/// Before draining the queue, this function polls the active session for
/// new events (bell, clipboard, exit, title change) and pushes them in.
///
/// Each call drains one event. Kotlin should call this at frame rate
/// (every ~16ms) in a LaunchedEffect.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_pollEvent<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    // Step 1: Poll the active session for new events.
    let active_id = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Relaxed);
    if active_id != 0 {
        let registry = lock_or_recover(&SESSION_REGISTRY, "ffi: pollEvent");
        if let Some(entry) = registry.get(&active_id) {
            if let Ok(session) = entry.session.lock() {
                // Check bell
                if session.poll_bel() {
                    push_event(Event::Bell {
                        session_id: active_id,
                    });
                }
                // Check clipboard
                if let Some(text) = session.poll_clipboard() {
                    push_event(Event::Clipboard {
                        session_id: active_id,
                        text,
                    });
                }
                // Check exit
                if session.is_exited() {
                    let code = *lock_or_recover(&session.exit_code, "ffi: pollEvent exit_code")
                        .as_ref()
                        .unwrap_or(&0);
                    push_event(Event::Exit {
                        session_id: active_id,
                        code,
                    });
                }
            }
        }
    }

    // Step 2: Drain one event from the queue.
    let event = lock_or_recover(&EVENT_QUEUE, "ffi: pollEvent drain").pop();

    match event {
        Some(e) => {
            let json = serde_json::to_string(&e).unwrap_or_default();
            match env.new_string(&json) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        None => std::ptr::null_mut(),
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: attachWindow
// ══════════════════════════════════════════════════════════════════════════

/// Attach an Android Surface — Android only.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_attachWindow(
    env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    surface: jobject,
    width: jint,
    height: jint,
) {
    if surface.is_null() {
        log::error!("FFI: attachWindow called with null surface");
        return;
    }

    // Get the raw ANativeWindow pointer from the Surface object.
    let raw_env = env.get_native_interface();
    // SAFETY: `raw_env` comes from `get_native_interface()` which returns a valid
    // JNIEnv pointer; `surface` is a JNI method argument guaranteed valid by the
    // JVM runtime. `ANativeWindow_fromSurface` is a documented NDK function from
    // `libandroid.so`.
    let ptr = unsafe { ANativeWindow_fromSurface(raw_env as *mut _, surface as *mut _) };

    if ptr.is_null() {
        log::error!("FFI: attachWindow — ANativeWindow_fromSurface returned NULL");
        return;
    }

    log::info!("FFI: attachWindow ptr={:p} {}x{}", ptr, width, height);

    // NOTE: Surface lifecycle integration is pending (ADR-0007). We do not
    // yet use the ANativeWindow pointer in the render thread. Release it
    // immediately to avoid leaking a native window resource on every
    // attachWindow call. When surface integration is implemented, the
    // pointer should be wrapped in the RAII `NativeWindow` type and
    // released after the wgpu surface that references it.
    // SAFETY: `ptr` is a valid ANativeWindow* from `ANativeWindow_fromSurface`
    // and has not been used elsewhere. `ANativeWindow_release` is a documented
    // NDK function that decrements the window's reference count.
    unsafe { ANativeWindow_release(ptr) };
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: detachWindow
// ══════════════════════════════════════════════════════════════════════════

/// Detach the current surface — Android only.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_detachWindow(
    _env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) {
    log::info!("FFI: detachWindow");
    // NOTE: Surface lifecycle integration is pending (ADR-0007).
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: setMcpEnabled
// ══════════════════════════════════════════════════════════════════════════

/// Enable or disable the MCP server (starts/stops it as needed).
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_setMcpEnabled(
    _env: JNIEnv,
    _class: JClass,
    enabled: jboolean,
) {
    #[cfg(feature = "mcp")]
    crate::mcp::set_enabled(enabled == JNI_TRUE);
    #[cfg(not(feature = "mcp"))]
    let _ = enabled;
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: setSessionSavePath
// ══════════════════════════════════════════════════════════════════════════

/// Set the persistence save path for a session. Pass empty string to disable.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_setSessionSavePath<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    path: JString<'local>,
) {
    let id = session_id as u64;
    let path_str: String = env.get_string(&path).map(|s| s.into()).unwrap_or_default();
    let registry = lock_or_recover(&SESSION_REGISTRY, "ffi: setSessionSavePath");
    if let Some(entry) = registry.get(&id) {
        if let Ok(mut session) = entry.session.lock() {
            session.set_save_path(&path_str);
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: dialogResult — Kotlin responds to a dialog request
// ══════════════════════════════════════════════════════════════════════════

/// Called by Kotlin after the user interacts with a dialog or file picker.
/// `result` is the user's input (text for input, "confirmed"/"cancelled"
/// for confirm, selected option for select, file path for pick_file).
#[unsafe(no_mangle)]
#[cfg(feature = "mcp")]
pub extern "system" fn Java_io_term_bridge_NativeBridge_dialogResult<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    result: JString<'local>,
) {
    let session_id = session_id as u64;
    let request_id = request_id as u64;

    // Look up the pending request sender and send the result
    if let Ok(mut registry) = REQUEST_REGISTRY.lock() {
        if let Some(tx) = registry.remove(&(session_id, request_id)) {
            let result_str: String = env
                .get_string(&result)
                .map(|s| s.into())
                .unwrap_or_default();
            let _ = tx.send(result_str);
        }
    }
}
// ══════════════════════════════════════════════════════════════════════════

/// Returns a JSON array of active session IDs.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_term_bridge_NativeBridge_listSessions<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let ids: Vec<u64> = SESSION_REGISTRY
        .lock()
        .map(|r| r.keys().copied().collect())
        .unwrap_or_default();

    let json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    match env.new_string(&json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
