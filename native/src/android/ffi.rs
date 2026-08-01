//! JNI FFI bridge — replaces boltffi/JNA with direct JNI.
//!
//! # Requirements
//! - FR-049 — JNI NDK bridge: session lifecycle exported to Kotlin
//! - FR-050 — surface lifecycle: attach/detach window events
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
//! Session lifecycle calls (`initSession`, `destroySession`, `switchSession`,
//! `resize`, `feedPty`, `writeKey`) are called from Kotlin coroutines on
//! `Dispatchers.IO` (TerminalRuntime) — never from the main UI thread. They
//! must never block indefinitely: the VT command channel uses `try_send`
//! and query RPCs use bounded timeouts so a wedged VT thread cannot freeze
//! the caller.
//!
//! EXCEPTION A: `focusEvent` runs on the main UI thread (window focus
//! change). Its mode query is bounded to 50ms (FOCUS_MODE_QUERY_TIMEOUT_MS)
//! and the session lock is held only for that window, so the UI thread is
//! never frozen for long (round-98 documents this in the session focus_event
//! docs too).
//!
//! EXCEPTION B: `dialogResult` and `clipboardResult` arrive from
//! AlertDialog / ActivityResult callbacks (main thread) and from the render
//! thread (empty replies on session exit). All call sites only take the
//! REQUEST_REGISTRY mutex briefly (no session locks, no blocking waits), so
//! the multi-thread entry is safe (round-116).
//!
//! `pollEvent` is called at frame rate from ONE render thread per active
//! session (TerminalRuntime render loop) — never on the main thread.
//! Multiple render threads may call it transiently while a session
//! replacement is starting (the old thread is stopped first; the window is
//! bounded by the 1s join timeout). Background sessions have no render
//! thread under the single-active-session design.
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
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jbyteArray, jint, jlong, jstring};

use crate::terminal::ShellEnv;
use crate::terminal::session::Session;
use std::sync::Arc;

/// Catches panics escaping a JNI export body. A panic crossing the
/// `extern "system"` boundary is undefined behaviour (process abort) — every
/// session dies instantly and no crash handler runs. The guard converts the
/// panic into a Java RuntimeException and returns `$default` to Kotlin.
macro_rules! jni_export_guard {
    ($env:expr, $default:expr, $call:expr) => {{
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $call)) {
            Ok(value) => value,
            Err(payload) => {
                let message = payload
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_string());
                log::error!("JNI export panicked: {message}");
                let _ = $env.throw_new(
                    "java/lang/RuntimeException",
                    format!("native panic: {message}"),
                );
                $default
            }
        }
    }};
}

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

/// Max VT chunks drained per frame per background session in the pollEvent
/// sweep. Keeps a background session's reader thread from blocking on a
/// full output channel (which would fill the PTY kernel buffer and freeze
/// the child process) without starving the active session's frame budget.
const BACKGROUND_CHUNKS_PER_FRAME: u32 = 2;

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

#[cfg(feature = "mcp")]
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

/// Remove a pending dialog/pick_file request without answering it. Called
/// by the MCP tools when their 300s timeout expires so a never-answered
/// request cannot leak one oneshot Sender in REQUEST_REGISTRY per call.
#[cfg(feature = "mcp")]
pub(crate) fn cancel_request(session_id: u64, request_id: u64) {
    lock_or_recover(&REQUEST_REGISTRY, "ffi: cancel_request").remove(&(session_id, request_id));
}

/// Current active session id (0 = none). Read helper for MCP event
/// bridging where the callback has no session parameter.
#[cfg(feature = "mcp")]
fn active_session_id() -> u64 {
    ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire)
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
    shell: JString,
    home: JString,
    user: JString,
    path: JString,
    working_directory: JString,
    prefix: JString,
) -> jlong {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, 0, {
        init_session_inner(
            &mut env,
            _class,
            rows,
            cols,
            shell,
            home,
            user,
            path,
            working_directory,
            prefix,
        )
    })
}

// JNI export bodies may legitimately take many arguments: the parameter
// list is dictated by the Kotlin `NativeBridge` declaration, not by design
// choice. The argument count is fixed by the ABI and cannot be reduced
// without a coordinated Kotlin change.
#[allow(clippy::too_many_arguments)]
fn init_session_inner(
    env: &mut JNIEnv,
    _class: JClass,
    rows: jint,
    cols: jint,
    shell: JString,
    home: JString,
    user: JString,
    path: JString,
    working_directory: JString,
    prefix: JString,
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

    let shell_path: String = match env.get_string(&shell) {
        Ok(s) => s.into(),
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                "initSession: failed to read shell path",
            );
            return 0;
        }
    };
    // The Kotlin side resolves the effective shell (Termux bash when the
    // bootstrap is installed, otherwise the system shell). Defend against an
    // empty value anyway: execve("") would fail and the session would exit
    // immediately.
    let shell_path = if shell_path.is_empty() {
        "/system/bin/sh".to_string()
    } else {
        shell_path
    };

    // Read the environment the Kotlin side resolved from the bootstrap
    // (home/user/path/working directory/prefix). Empty strings mean
    // "not known" and fall back to the process environment.
    let read_env_string = |env: &mut JNIEnv, value: &JString, name: &str| -> Option<String> {
        match env.get_string(value) {
            Ok(s) => {
                let text: String = s.into();
                Some(text)
            }
            Err(_) => {
                let _ = env.throw_new(
                    "java/lang/RuntimeException",
                    format!("initSession: failed to read {name}"),
                );
                None
            }
        }
    };
    let home = match read_env_string(env, &home, "home") {
        Some(value) => value,
        None => return 0,
    };
    let user = match read_env_string(env, &user, "user") {
        Some(value) => value,
        None => return 0,
    };
    let path = match read_env_string(env, &path, "path") {
        Some(value) => value,
        None => return 0,
    };
    let working_directory = match read_env_string(env, &working_directory, "workingDirectory") {
        Some(value) => value,
        None => return 0,
    };
    let prefix = match read_env_string(env, &prefix, "prefix") {
        Some(value) => value,
        None => return 0,
    };
    let default = ShellEnv::default();
    let home = if home.is_empty() {
        default.home.clone()
    } else {
        home
    };
    let user = if user.is_empty() {
        default.user.clone()
    } else {
        user
    };
    let path = if path.is_empty() {
        default.path.clone()
    } else {
        path
    };
    let working_directory = if working_directory.is_empty() {
        home.clone()
    } else {
        working_directory
    };
    let shell_env = ShellEnv {
        home,
        user,
        path,
        working_directory,
        prefix: if prefix.is_empty() {
            None
        } else {
            Some(prefix)
        },
        extra: vec![],
    };

    match Session::spawn(&shell_path, rows, cols, &shell_env) {
        Ok(mut session) => {
            let id = next_session_id();
            let entry = SessionEntry {
                session: Arc::new(Mutex::new(session)),
            };

            let mut registry = wlock_session_registry();
            registry.insert(id, entry);
            // Atomic check-and-set: concurrent initSession calls (start()
            // and createSession spawn outside the Kotlin lock) must not
            // double-write the active id. The dims are only mirrored for
            // the session that WON the active slot, symmetric with resize's
            // active-only update.
            if ACTIVE_SESSION_ID
                .compare_exchange(
                    0,
                    id,
                    std::sync::atomic::Ordering::Acquire,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                #[cfg(feature = "mcp")]
                crate::mcp::global_state().set_active_session_id(id);
                #[cfg(feature = "mcp")]
                crate::mcp::global_state().set_terminal_dims(rows, cols);
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
    mut _env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut _env, JNI_FALSE, {
        destroy_session_inner(&mut _env, _class, session_id)
    })
}

fn destroy_session_inner(_env: &mut JNIEnv, _class: JClass, session_id: jlong) -> jboolean {
    let id = session_id as u64;
    // Take the entry out of the registry and release the write lock BEFORE
    // the entry is dropped: Session::drop kills the child and joins its
    // reader/wait threads (tens to hundreds of ms), and holding the global
    // registry lock during that would block every pollEvent/feedPty/writeKey
    // on all sessions. The active-id fix-up and the MCP mirror update run
    // INSIDE the write-lock critical section so a concurrent switchSession
    // can never interleave a stale mirror value (mirror = lock-free
    // AtomicU64, no deadlock risk).
    let removed_entry = {
        let mut guard = wlock_session_registry();
        let removed = guard.remove(&id);
        if removed.is_some() {
            // If we removed the active session, clear the active ID.
            ACTIVE_SESSION_ID
                .compare_exchange(
                    id,
                    0,
                    std::sync::atomic::Ordering::Acquire,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .ok();
            // Mirror the ACTUAL current value into the MCP layer — NOT
            // unconditionally 0: when a non-active session was destroyed
            // the compare_exchange above failed and the active id is
            // unchanged. Setting 0 unconditionally would desync the mirror
            // and break send_signal/dialog/pick_file/clipboard_get until
            // the next switchSession/initSession.
            #[cfg(feature = "mcp")]
            crate::mcp::global_state().set_active_session_id(
                ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire),
            );
        }
        removed
    };
    let removed = removed_entry.is_some();
    // removed_entry is dropped HERE, outside the write lock (Session::drop
    // kills and joins the child's threads).

    if removed {
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
    mut _env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut _env, JNI_FALSE, {
        switch_session_inner(&mut _env, _class, session_id)
    })
}

fn switch_session_inner(_env: &mut JNIEnv, _class: JClass, session_id: jlong) -> jboolean {
    let id = session_id as u64;
    // Check-and-store atomically under the WRITE lock: a plain read-lock
    // check followed by an unlocked store leaves a TOCTOU window where a
    // concurrent destroySession removes the id between the two, storing a
    // stale active id. destroy/switch are both low-frequency, so write-lock
    // contention is a non-issue. The MCP mirror is updated INSIDE the same
    // critical section so a concurrent destroySession can never interleave
    // a stale mirror value after the switch (mirror = lock-free AtomicU64,
    // no deadlock risk).
    {
        let mut guard = wlock_session_registry();
        if !guard.contains_key(&id) {
            log::warn!("FFI: switchSession id={} not found", id);
            return 0;
        }
        ACTIVE_SESSION_ID.store(id, std::sync::atomic::Ordering::Release);
        #[cfg(feature = "mcp")]
        {
            let mcp = crate::mcp::global_state();
            mcp.set_active_session_id(id);
            // Refresh terminal_info dims from the newly active session's
            // CACHED grid size: init mirrors dims only for the CAS winner
            // and resize only for the active session, so a switch between
            // sessions with different grids must re-sync here. The cache
            // itself is lock-free — a query RPC on the VT thread inside
            // the registry write lock would freeze every session operation
            // for up to 2×QUERY_TIMEOUT_MS. The short session.lock() below
            // may still wait up to a pollEvent frame or a 50ms focus-event
            // query while the UI thread holds it, which is acceptable
            // (dims refresh is best-effort).
            match guard[&id].session.lock() {
                Ok(session_guard) => {
                    let (rows, cols) = session_guard.grid_size();
                    mcp.set_terminal_dims(rows, cols);
                }
                Err(poisoned) => {
                    log::error!("switchSession: session lock poisoned for session {id}");
                    // Recover and still refresh dims (best-effort, same as
                    // the pollEvent sweep branch): a stale terminal_info
                    // would mislead MCP agents about the grid size.
                    let (rows, cols) = poisoned.into_inner().grid_size();
                    mcp.set_terminal_dims(rows, cols);
                }
            }
        }
    }
    JNI_TRUE
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: getSessionCount
// ══════════════════════════════════════════════════════════════════════════

/// Returns the number of active sessions.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getSessionCount(
    mut _env: JNIEnv,
    _class: JClass,
) -> jint {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut _env, 0, { get_session_count_inner(&mut _env, _class) })
}

fn get_session_count_inner(_env: &mut JNIEnv, _class: JClass) -> jint {
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
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        resize_inner(&mut env, _class, session_id, rows, cols)
    })
}

fn resize_inner(env: &mut JNIEnv, _class: JClass, session_id: jlong, rows: jint, cols: jint) {
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
                match session.resize(rows, cols) {
                    Err(e) => {
                        if let Err(e) = env
                            .throw_new("java/lang/RuntimeException", format!("resize: failed: {e}"))
                        {
                            log::error!("resize: throw_new failed: {e}");
                        }
                    }
                    Ok(outcome) => {
                        // Only the ACTIVE session's size is reflected in the
                        // global MCP dims: background sessions may
                        // legitimately have a different grid (e.g. deferred
                        // resize), and terminal_info must report the visible
                        // one. When the grid command was dropped the PTY and
                        // grid disagree, so the dims stay at the cached (old)
                        // values — publishing the new ones would make MCP
                        // dims flip-flop between resize and switch paths
                        // (round-113).
                        if matches!(outcome, crate::terminal::session::ResizeOutcome::Applied)
                            && id == ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire)
                        {
                            #[cfg(feature = "mcp")]
                            crate::mcp::global_state().set_terminal_dims(rows, cols);
                        }
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
// JNI Export: focusEvent
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_focusEvent(
    mut env: JNIEnv,
    class: JClass,
    session_id: jlong,
    focused: jboolean,
) -> jboolean {
    jni_export_guard!(&mut env, JNI_FALSE, {
        focus_event_inner(&mut env, class, session_id, focused)
    })
}

fn focus_event_inner(
    env: &mut JNIEnv,
    _class: JClass,
    session_id: jlong,
    focused: jboolean,
) -> jboolean {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    if let Some(entry) = registry.get(&id) {
        match entry.session.lock() {
            Ok(mut session) => {
                session.focus_event(focused == JNI_TRUE);
                return JNI_TRUE;
            }
            Err(_poisoned) => {
                // Same policy as resize/feedPty (round-98): a poisoned
                // session lock means a panic happened inside the VT thread;
                // silence would mask the crash — throw and let Kotlin log it.
                log::error!("focusEvent: session lock poisoned for session {id}");
                let _ = env.throw_new(
                    "java/lang/RuntimeException",
                    format!("focusEvent: session lock poisoned for session {id}"),
                );
                return JNI_FALSE;
            }
        }
    }
    JNI_FALSE
}

// JNI Export: feedPty
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
// JNI exports receive raw handles (jbyteArray/jstring are pointer types)
// whose validity is the JVM's contract, not a Rust lifetime guarantee;
// each unsafe block below carries its own SAFETY comment.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_feedPty(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        feed_pty_inner(&mut env, _class, session_id, data)
    })
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
fn feed_pty_inner(env: &mut JNIEnv, _class: JClass, session_id: jlong, data: jbyteArray) {
    let id = session_id as u64;

    // Raw bytes, not a String: PTY input may be arbitrary binary (pasted
    // GBK/ISO-8859-1 text, protocol data). Going through a Java String
    // would replace invalid UTF-8 sequences with U+FFFD, silently
    // corrupting the bytes the child process receives.
    let input: Vec<u8> = {
        // SAFETY: `data` is a JNI method argument, guaranteed valid by the
        // JVM runtime for the duration of this call. `from_raw` wraps the
        // pointer without taking ownership; the local ref is released by
        // the JVM when this native method returns.
        // `not_unsafe_ptr_arg_deref` is handled at the function level (JNI
        // handle validity is the VM's contract, not a Rust lifetime
        // guarantee); the SAFETY comment above documents the contract.
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(data) };
        match env.convert_byte_array(&byte_array) {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = env.throw_new(
                    "java/lang/RuntimeException",
                    "feedPty: failed to read input bytes",
                );
                return;
            }
        }
    };

    let registry = rlock_session_registry();
    if let Some(entry) = registry.get(&id) {
        match entry.session.lock() {
            Ok(mut session) => {
                if let Err(e) = session.write(&input) {
                    // The master fd is O_NONBLOCK (set in Session::spawn): a
                    // full PTY buffer (child not reading) surfaces as EAGAIN.
                    // Dropping the input matches xterm behaviour; surfacing it
                    // as an error would spam the log on every keystroke of a
                    // huge paste.
                    if e.is_would_block() {
                        return;
                    }
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
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        write_key_inner(&mut env, _class, session_id, key, mods, text)
    })
}

fn write_key_inner(
    env: &mut JNIEnv,
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
                    // EAGAIN (full PTY buffer) drops the input silently —
                    // same rationale as feedPty.
                    if e.is_would_block() {
                        return;
                    }
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
    class: JClass<'local>,
) -> jstring {
    // A panic here (e.g. inside ghostty's VT processing) would abort the
    // whole process. Convert it into a Java exception instead.
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        poll_event_inner(&mut env, class)
    })
}

/// Wait (up to 100ms) for the wait thread to write the session's exit code,
/// polling with short session locks so no other thread is blocked for the
/// whole wait. The reader thread can set `exited` (EOF) just before the
/// wait thread writes `exit_code`; this closes that window so a real exit
/// code (e.g. 137) is never reported as 0. MUST be called without holding
/// the session lock.
///
/// On timeout the code is reported as 0 — indistinguishable from a clean
/// exit. This is a documented tradeoff: the alternative (blocking longer)
/// would stall pollEvent frames, and the exit event cannot be re-sent
/// (mark_exit_reported is already set). The warn log is the only signal.
fn wait_exit_code(session: &Arc<Mutex<Session>>) -> i32 {
    for _ in 0..10 {
        {
            let guard = lock_or_recover(session.as_ref(), "ffi: wait_exit_code");
            if let Some(code) = guard.exit_code_now() {
                return code;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    log::warn!("ffi: exit code not written within 100ms of exit");
    0
}

fn poll_event_inner<'local>(env: &mut JNIEnv<'local>, _class: JClass<'local>) -> jstring {
    // Step 1: Poll the active session for new events.
    // Collect events first, then push them after dropping the session lock
    // to maintain the lock order: SESSION_REGISTRY → Session, then EVENT_QUEUE.
    // Never lock EVENT_QUEUE while holding a Session lock.
    let mut pending_events: Vec<Event> = Vec::new();
    // Sessions whose Exit event must be reported this frame, with the Arc
    // cloned so the exit code can be read AFTER the SESSION_REGISTRY read
    // lock is released: wait_exit_code may busy-wait up to 100ms, and
    // holding the registry read lock that long would block destroySession/
    // initSession write locks (RwLock writer starvation).
    let mut pending_exits: Vec<(u64, Arc<Mutex<Session>>)> = Vec::new();
    let active_id = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
    {
        let registry = rlock_session_registry();
        if active_id != 0 {
            if let Some(entry) = registry.get(&active_id) {
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
                // Check notification (OSC 9)
                if let Some((title, body)) = session.poll_notification() {
                    pending_events.push(Event::Notification {
                        session_id: active_id,
                        title,
                        body,
                    });
                }
                // Check exit. Only the first poll after the process exits
                // reports it (mark_exit_reported); the sweep branch below uses
                // the same dedup so a slow consumer can never see duplicate
                // Exit events for the same session. The exit code is read
                // after both locks are released (see pending_exits below).
                if session.is_exited() && session.mark_exit_reported() {
                    pending_exits.push((active_id, entry.session.clone()));
                }
                // Session lock is dropped here (end of the if-let block).
            }
        }
        // Sweep background sessions: report exits once (exit_reported flag)
        // AND drain their PTY output. A background session whose output
        // channel fills up (reader thread blocks on bounded send → PTY
        // kernel buffer fills → child process write blocks) would freeze
        // the background job; draining a few chunks per frame keeps the
        // pipe moving without starving the active session's frame budget.
        for (id, entry) in registry.iter() {
            if active_id != 0 && *id == active_id {
                continue;
            }
            let mut session = match entry.session.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    log::error!("pollEvent: sweep session lock poisoned for session {id}");
                    poisoned.into_inner()
                }
            };
            // 2 chunks per frame per background session: enough to keep
            // the reader thread unblocked under sustained output.
            session.poll_pty_output(BACKGROUND_CHUNKS_PER_FRAME);
            // Consume stale event flags immediately and push them with the
            // correct session_id. If left set, they would be replayed when
            // the session becomes active again minutes later (stale replay).
            if session.poll_bel() {
                pending_events.push(Event::Bell { session_id: *id });
            }
            if let Some(text) = session.poll_clipboard() {
                pending_events.push(Event::Clipboard {
                    session_id: *id,
                    text,
                });
            }
            if let Some((title, body)) = session.poll_notification() {
                pending_events.push(Event::Notification {
                    session_id: *id,
                    title,
                    body,
                });
            }
            if session.is_exited() && session.mark_exit_reported() {
                pending_exits.push((*id, entry.session.clone()));
            }
        }
        // Registry read lock is released here.
    }
    // Read exit codes AFTER the registry read lock is released: the
    // wait may take up to 100ms, and holding a read lock that long
    // would starve destroySession/initSession write locks (RwLock
    // writer starvation). The cloned Arc keeps the session alive
    // regardless of registry changes.
    for (id, session) in pending_exits {
        pending_events.push(Event::Exit {
            session_id: id,
            code: wait_exit_code(&session),
        });
    }
    // Push collected events — no Session or SESSION_REGISTRY locks held.
    for event in pending_events {
        EVENT_QUEUE.push(event);
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
    mut _env: JNIEnv,
    _class: JClass,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut _env, (), { init_logger_inner(&mut _env, _class) })
}

fn init_logger_inner(_env: &mut JNIEnv, _class: JClass) {
    // There is no JNI_OnLoad hook in this crate, so this JNI export is the
    // only place logging can be initialised. Without it every log::* call
    // in production (including GPU errors, lock poisoning, VT thread
    // panics) is silently dropped, making crash diagnosis impossible.
    #[cfg(target_os = "android")]
    crate::android::logging::init();
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
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        set_log_file_path_inner(&mut env, _class, path)
    })
}

fn set_log_file_path_inner<'local>(
    env: &mut JNIEnv<'local>,
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
        let _ = (&env, path);
        log::info!("setLogFilePath: not supported on this platform");
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: attachWindow
// ══════════════════════════════════════════════════════════════════════════

/// Attach an Android Surface — Android only.
///
/// NOTE: kept as an ADR-0007 placeholder. Kotlin currently never calls
/// this export (surface integration is deferred; the render thread does
/// not consume a native window yet).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_attachWindow(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    surface: jobject,
    width: jint,
    height: jint,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        attach_window_inner(&mut env, _class, _session_id, surface, width, height)
    })
}

#[cfg(target_os = "android")]
fn attach_window_inner(
    env: &mut JNIEnv,
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
///
/// NOTE: kept as an ADR-0007 placeholder (currently a log-only no-op),
/// symmetric with attachWindow.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_detachWindow(
    mut _env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut _env, (), {
        detach_window_inner(&mut _env, _class, _session_id)
    })
}

#[cfg(target_os = "android")]
fn detach_window_inner(_env: &mut JNIEnv, _class: JClass, _session_id: jlong) {
    log::info!("FFI: detachWindow");
    // NOTE: Surface lifecycle integration is pending (ADR-0007).
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: setMcpEnabled
// ══════════════════════════════════════════════════════════════════════════

/// Enable or disable the MCP server (starts/stops it as needed).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setMcpEnabled(
    mut _env: JNIEnv,
    _class: JClass,
    enabled: jboolean,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut _env, (), {
        set_mcp_enabled_inner(&mut _env, _class, enabled)
    })
}

fn set_mcp_enabled_inner(_env: &mut JNIEnv, _class: JClass, enabled: jboolean) {
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
                 -> (u64, tokio::sync::oneshot::Receiver<String>) {
                    let (request_id, rx) = register_request(session_id);
                    push_event(Event::ShowDialog {
                        session_id,
                        request_id,
                        dialog_type,
                        title,
                        message,
                        options,
                    });
                    (request_id, rx)
                },
            );
            state.set_pick_file_handler(
                |session_id: u64,
                 starting_path: String,
                 filter: String|
                 -> (u64, tokio::sync::oneshot::Receiver<String>) {
                    let (request_id, rx) = register_request(session_id);
                    push_event(Event::PickFile {
                        session_id,
                        request_id,
                        starting_path,
                        filter,
                    });
                    (request_id, rx)
                },
            );
            state.set_send_signal_handler(|session_id: u64, signum: i32| -> String {
                // Recover-on-poison helper, same policy as every other
                // registry read in this file (round-99).
                let guard = rlock_session_registry();
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
            // Remaining tools are bridged through the event queue into
            // Kotlin (which owns the system clipboard, toasts, and the
            // browser), mirroring the dialog/pick_file seam.
            state.set_clipboard_get_handler(|| {
                let session_id = active_session_id();
                let (request_id, rx) = register_request(session_id);
                push_event(Event::GetClipboard {
                    session_id,
                    request_id,
                });
                (request_id, rx)
            });
            state.set_clipboard_set_handler(|text: String| {
                let session_id = active_session_id();
                push_event(Event::Clipboard { session_id, text });
            });
            state.set_notify_handler(|message: String| {
                let (title, body) = match message.split_once('\n') {
                    Some((t, b)) => (t.to_string(), b.to_string()),
                    None => (message, String::new()),
                };
                push_event(Event::Notification {
                    session_id: active_session_id(),
                    title,
                    body,
                });
            });
            state.set_toast_handler(|text: String| {
                push_event(Event::Toast { text });
            });
            state.set_open_url_handler(|url: String| {
                push_event(Event::OpenUrl { url });
            });
        });
        crate::mcp::set_enabled(enabled == JNI_TRUE);
    }
    #[cfg(not(feature = "mcp"))]
    let _ = enabled;
}

// ══════════════════════════════════════════════════════════════════════════
// ══════════════════════════════════════════════════════════════════════════
// JNI Export: clipboardResult — Kotlin responds to an MCP clipboard_get
// ══════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
#[cfg(feature = "mcp")]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clipboardResult<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    text: JString<'local>,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        clipboard_result_inner(&mut env, _class, session_id, request_id, text)
    })
}

#[cfg(feature = "mcp")]
fn clipboard_result_inner<'local>(
    env: &mut JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    text: JString<'local>,
) {
    let session_id = session_id as u64;
    let request_id = request_id as u64;

    if let Some(tx) =
        lock_or_recover(&REQUEST_REGISTRY, "ffi: clipboardResult").remove(&(session_id, request_id))
    {
        let text_str: String = env.get_string(&text).map(|s| s.into()).unwrap_or_default();
        let _ = tx.send(text_str);
    }
}

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
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        dialog_result_inner(&mut env, _class, session_id, request_id, result)
    })
}

#[cfg(feature = "mcp")]
fn dialog_result_inner<'local>(
    env: &mut JNIEnv<'local>,
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
///
/// NOTE: kept for API completeness; Kotlin currently never calls this
/// export (session ids are tracked in TerminalRuntime.sessionIds).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listSessions<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        list_sessions_inner(&mut env, _class)
    })
}

fn list_sessions_inner<'local>(env: &mut JNIEnv<'local>, _class: JClass<'local>) -> jstring {
    let ids: Vec<u64> = rlock_session_registry().keys().copied().collect();

    let json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    match env.new_string(&json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
