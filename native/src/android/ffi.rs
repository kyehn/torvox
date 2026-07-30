//! JNI FFI bridge — replaces boltffi/JNA with direct JNI.
//!
//! This module exports `extern "system"` JNI functions called from Kotlin.
//! Each function follows the JNI naming convention:
//! `Java_terminal_emulator_bridge_NativeBridge_<methodName>`
//!
//! Session lifecycle:
//! - `initSession()` creates a `Session` and registers it globally
//! - `destroySession()` shuts down the session and removes it
//! - The active session is set via `switchSession()`
//!
//! Events (bell, title, clipboard, exit) are pushed into a global queue
//! and drained by Kotlin via `pollEvent()`.
//!
//! # Threading model (JNI call site assumptions)
//!
//! All JNI exports are called from Kotlin's main UI thread
//! (Dispatchers.Main) unless otherwise noted.  The Kotlin side serialises
//! calls to `initSession`, `destroySession`, `switchSession`, `resize`,
//! `feedPty`, `writeKey`, and `dialogResult` on a
//! single coroutine context.
//!
//! `pollEvent` is called at frame rate (~16ms intervals) from a
//! `LaunchedEffect` — also on the main thread.
//!
//! `setMcpEnabled` may be called from settings or during initialisation.
//! It is safe from any thread.
//!
//! # Concurrency model
//!
//! - `SESSION_REGISTRY` is an `RwLock`; reads dominate writes.
//! - `EVENT_QUEUE` is a `Mutex`; push/pop are fast.
//! - Lock order: `SESSION_REGISTRY` → `Session` → `exit_code`.
//!   `EVENT_QUEUE` is locked independently and never while holding a
//!   `Session` lock.
//! - `ACTIVE_SESSION_ID` is an `AtomicU64` with `Acquire`/`Release` ordering.
//!   ID 0 means "no active session".

// Style: nested-if is an error-handling idiom for JNI; unused-mut is a
// false positive with jni crate (new_string needs &mut self on some configs).
#![allow(clippy::collapsible_if, unused_mut, clippy::type_complexity)]

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
#[cfg(feature = "mcp")]
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex, RwLock};

use crate::event::Event;
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
#[cfg(target_os = "android")]
use jni::sys::jobject;

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
}

static SESSION_REGISTRY: LazyLock<RwLock<HashMap<u64, SessionEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Lock SESSION_REGISTRY for reading, recovering from poison.
fn rlock_session_registry() -> std::sync::RwLockReadGuard<'static, HashMap<u64, SessionEntry>> {
    match SESSION_REGISTRY.read() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("SESSION_REGISTRY: read lock poisoned, recovered");
            poisoned.into_inner()
        }
    }
}

/// Lock SESSION_REGISTRY for writing, recovering from poison.
fn wlock_session_registry() -> std::sync::RwLockWriteGuard<'static, HashMap<u64, SessionEntry>> {
    match SESSION_REGISTRY.write() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("SESSION_REGISTRY: write lock poisoned, recovered");
            poisoned.into_inner()
        }
    }
}

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

static ACTIVE_SESSION_ID: AtomicU64 = AtomicU64::new(0);

fn next_session_id() -> u64 {
    NEXT_SESSION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ══════════════════════════════════════════════════════════════════════════
// Event Queue
// ══════════════════════════════════════════════════════════════════════════

static EVENT_QUEUE: crate::event::EventQueue = crate::event::EventQueue::new();

/// Monotonic counter for user-input request IDs.
#[cfg(feature = "mcp")]
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(feature = "mcp")]
static REQUEST_REGISTRY: LazyLock<
    Mutex<HashMap<(u64, u64), tokio::sync::oneshot::Sender<String>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[allow(dead_code)]
fn encode_modifiers(input: &[u8], mods: i32) -> Vec<u8> {
    let ctrl = (mods & 4) != 0;
    let alt_or_meta = (mods & (2 | 8)) != 0;

    let mut output = Vec::with_capacity(input.len() + 2);

    if alt_or_meta {
        output.push(0x1B); // ESC prefix for Alt/Meta
    }

    if ctrl && input.len() == 1 {
        let c = input[0];
        // For printable ASCII (0x20-0x7E), apply the standard Ctrl
        // formula c & 0x1F. Pre-existing control chars and non-ASCII
        // bytes pass through unchanged.
        if (0x20..=0x7E).contains(&c) {
            output.push(c & 0x1F);
            return output;
        }
    }

    output.extend_from_slice(input);
    output
}

pub(crate) fn push_event(event: Event) {
    EVENT_QUEUE.push(event);
}

#[cfg(feature = "mcp")]
pub(crate) fn register_request(session_id: u64) -> (u64, tokio::sync::oneshot::Receiver<String>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    lock_or_recover(&REQUEST_REGISTRY, "ffi: register_request")
        .insert((session_id, request_id), tx);
    (request_id, rx)
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: initSession
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_initSession(
    mut env: JNIEnv,
    _class: JClass,
    rows: jint,
    cols: jint,
) -> jlong {
    let rows = match u32::try_from(rows) {
        Ok(r) => r,
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "initSession: rows must be non-negative",
            );
            return 0;
        }
    };
    let cols = match u32::try_from(cols) {
        Ok(c) => c,
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "initSession: cols must be non-negative",
            );
            return 0;
        }
    };

    match Session::spawn("", rows, cols, &ShellEnv::default()) {
        Ok(mut session) => {
            let id = next_session_id();
            let entry = SessionEntry {
                session: Arc::new(Mutex::new(session)),
            };

            let mut registry = wlock_session_registry();
            registry.insert(id, entry);
            if ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire) == 0 {
                ACTIVE_SESSION_ID.store(id, std::sync::atomic::Ordering::Release);
                #[cfg(feature = "mcp")]
                crate::mcp::global_state().set_active_session_id(id);
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
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_destroySession(
    _env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    let id = session_id as u64;
    let removed = wlock_session_registry().remove(&id).is_some();

    if removed {
        // If we removed the active session, clear the active ID
        ACTIVE_SESSION_ID
            .compare_exchange(
                id,
                0,
                std::sync::atomic::Ordering::Acquire,
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
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_switchSession(
    _env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    let id = session_id as u64;
    let exists = rlock_session_registry().contains_key(&id);

    if exists {
        ACTIVE_SESSION_ID.store(id, std::sync::atomic::Ordering::Release);
        #[cfg(feature = "mcp")]
        crate::mcp::global_state().set_active_session_id(id);
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
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionCount(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    rlock_session_registry().len() as i32
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: resize
// ══════════════════════════════════════════════════════════════════════════

/// Resize the specified session. Throws RuntimeException if session not found.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_resize(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    rows: jint,
    cols: jint,
) {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    if let Some(entry) = registry.get(&id) {
        match entry.session.lock() {
            Ok(mut session) => {
                let rows = match u32::try_from(rows) {
                    Ok(r) => r,
                    Err(_) => {
                        let _ = env.throw_new(
                            "java/lang/IllegalArgumentException",
                            "resize: rows must be non-negative",
                        );
                        return;
                    }
                };
                let cols = match u32::try_from(cols) {
                    Ok(c) => c,
                    Err(_) => {
                        let _ = env.throw_new(
                            "java/lang/IllegalArgumentException",
                            "resize: cols must be non-negative",
                        );
                        return;
                    }
                };
                if let Err(e) = session.resize(rows, cols) {
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_feedPty(
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

    let registry = rlock_session_registry();
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_writeKey(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    key: JString,
    mods: jint,
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

    let registry = rlock_session_registry();
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
                    // Encode modifiers into the key byte sequence.
                    // This is a basic encoder; the modern Ghostty key
                    // encoder path (internal.rs key::Encoder + key::Event)
                    // should be used for full Kitty keyboard protocol support.
                    let bytes = encode_modifiers(key_str.as_bytes(), mods);
                    session.write(&bytes)
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_pollEvent<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    // Step 1: Poll the active session for new events.
    // Collect events first, then push them after dropping the session lock
    // to maintain the lock order: SESSION_REGISTRY → Session, then EVENT_QUEUE.
    // Never lock EVENT_QUEUE while holding a Session lock.
    let mut pending_events: Vec<Event> = Vec::new();
    let active_id = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
    if active_id != 0 {
        if let Some(entry) = rlock_session_registry().get(&active_id) {
            let mut session = match entry.session.lock() {
                Ok(guard) => guard,
                Err(_poisoned) => {
                    log::error!("pollEvent: session lock poisoned for session {active_id}");
                    let _ = env.throw_new(
                        "java/lang/RuntimeException",
                        "pollEvent: session lock poisoned",
                    );
                    return std::ptr::null_mut();
                }
            };
            // Process VT output from the PTY reader thread. This is the
            // critical path that drives all terminal state updates: it
            // reads from the output_rx channel, feeds data into Ghostty's
            // VT parser, and populates event flags (bell, clipboard, etc.)
            // that are polled below. Without this call the terminal will
            // never process output and the output channel deadlocks.
            session.process_output();
            // Check bell
            if session.poll_bel() {
                pending_events.push(Event::Bell {
                    session_id: active_id,
                });
            }
            // Check clipboard
            if let Some(text) = session.poll_clipboard() {
                pending_events.push(Event::Clipboard {
                    session_id: active_id,
                    text,
                });
            }
            // Check exit
            if session.is_exited() {
                let code = *lock_or_recover(&session.exit_code, "ffi: pollEvent exit_code")
                    .as_ref()
                    .unwrap_or(&0);
                pending_events.push(Event::Exit {
                    session_id: active_id,
                    code,
                });
            }
            // Session lock and registry read lock are dropped here.
        }
        // Push collected events — no Session or SESSION_REGISTRY locks held.
        for event in pending_events {
            EVENT_QUEUE.push(event);
        }
    }

    // Step 2: Drain one event from the queue.
    let event = EVENT_QUEUE.pop();

    match event {
        Some(e) => {
            let json = serde_json::to_string(&e).unwrap_or_else(|err| {
                log::error!("pollEvent: event serialization failed: {err}");
                String::new()
            });
            match env.new_string(&json) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        None => std::ptr::null_mut(),
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: initLogger
// ══════════════════════════════════════════════════════════════════════════

/// Initialise Rust-side logging (logcat + optional file).
/// Called once from Kotlin on app startup.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_initLogger(
    _env: JNIEnv,
    _class: JClass,
) {
    // Logging is already initialised by JNI_OnLoad; this is a no-op
    // placeholder for future configuration.
    log::info!("NativeBridge::initLogger called");
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: setLogFilePath
// ══════════════════════════════════════════════════════════════════════════

/// Set the file path for Rust-side log output.
/// Kotlin calls this after computing the log directory.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setLogFilePath<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) {
    #[cfg(target_os = "android")]
    {
        let path_str: String = match env.get_string(&path) {
            Ok(s) => s.into(),
            Err(_) => {
                log::error!("setLogFilePath: failed to read path string");
                return;
            }
        };
        crate::android::logging::set_log_file_path(&path_str);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (&mut env, path);
        log::info!("setLogFilePath: not supported on this platform");
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: attachWindow
// ══════════════════════════════════════════════════════════════════════════

/// Attach an Android Surface — Android only.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_attachWindow(
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
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_detachWindow(
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
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setMcpEnabled(
    _env: JNIEnv,
    _class: JClass,
    enabled: jboolean,
) {
    #[cfg(feature = "mcp")]
    {
        // Register dialog / pick_file callbacks once (they bridge
        // from the MCP thread into the JNI event queue).
        static MCP_HANDLERS: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        MCP_HANDLERS.get_or_init(|| {
            let state = crate::mcp::global_state();
            state.set_dialog_handler(
                |session_id: u64,
                 dialog_type: String,
                 title: String,
                 message: String,
                 options: Vec<String>|
                 -> tokio::sync::oneshot::Receiver<String> {
                    let (request_id, rx) = register_request(session_id);
                    push_event(Event::ShowDialog {
                        session_id,
                        request_id,
                        dialog_type,
                        title,
                        message,
                        options,
                    });
                    rx
                },
            );
            state.set_pick_file_handler(
                |session_id: u64,
                 starting_path: String,
                 filter: String|
                 -> tokio::sync::oneshot::Receiver<String> {
                    let (request_id, rx) = register_request(session_id);
                    push_event(Event::PickFile {
                        session_id,
                        request_id,
                        starting_path,
                        filter,
                    });
                    rx
                },
            );
            state.set_send_signal_handler(|session_id: u64, signum: i32| -> String {
                let guard = match SESSION_REGISTRY.read() {
                    Ok(g) => g,
                    Err(e) => return format!("SESSION_REGISTRY poisoned: {e}"),
                };
                match guard.get(&session_id) {
                    Some(entry) => match entry.session.lock() {
                        Ok(session) => match session.send_signal(signum) {
                            Ok(()) => format!("Signal {signum} sent to session {session_id}"),
                            Err(e) => format!("send_signal failed: {e}"),
                        },
                        Err(e) => format!("Session lock poisoned: {e}"),
                    },
                    None => format!("Session {session_id} not found"),
                }
            });
        });
        crate::mcp::set_enabled(enabled == JNI_TRUE);
    }
    #[cfg(not(feature = "mcp"))]
    let _ = enabled;
}

// ══════════════════════════════════════════════════════════════════════════
// ══════════════════════════════════════════════════════════════════════════
// JNI Export: dialogResult — Kotlin responds to a dialog request
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
#[cfg(feature = "mcp")]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_dialogResult<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    result: JString<'local>,
) {
    let session_id = session_id as u64;
    let request_id = request_id as u64;

    // Look up the pending request sender and send the result
    if let Some(tx) =
        lock_or_recover(&REQUEST_REGISTRY, "ffi: dialogResult").remove(&(session_id, request_id))
    {
        let result_str: String = env
            .get_string(&result)
            .map(|s| s.into())
            .unwrap_or_default();
        let _ = tx.send(result_str);
    }
}
// ══════════════════════════════════════════════════════════════════════════

/// Returns a JSON array of active session IDs.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listSessions<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let ids: Vec<u64> = rlock_session_registry().keys().copied().collect();

    let json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    match env.new_string(&json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
