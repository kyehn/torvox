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

use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU64;
#[cfg(feature = "mcp")]
use std::sync::atomic::Ordering;

use crate::event::Event;
use jni::JNIEnv;
use jni::objects::JObject;
use jni::objects::{JClass, JString};
use jni::sys::{
    JNI_FALSE, JNI_TRUE, jboolean, jbyte, jbyteArray, jfloat, jint, jlong, jobjectArray, jsize,
    jstring,
};

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
    pub(crate) fn ANativeWindow_setBuffersGeometry(
        window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
        format: i32,
    ) -> i32;
}

// ══════════════════════════════════════════════════════════════════════════
// Session Registry
// ══════════════════════════════════════════════════════════════════════════

/// A registered session with its ID and thread-safe handle.
struct SessionEntry {
    session: Arc<Mutex<Session>>,
    /// Last scroll offset applied for THIS session (round-209: previously
    /// the delta was computed against a single global value, so switching
    /// sessions could move the old session's viewport when its render
    /// thread resumed with the other session's offset).
    last_scroll_offset: i64,
}

static SESSION_REGISTRY: LazyLock<RwLock<HashMap<u64, SessionEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Global render state (ADR-0007): the wgpu renderer + font pipeline used
/// by the Android render thread. Created lazily on the first
/// `attachWindow`, owned by the JNI render thread (Kotlin's render loop
/// calls `render` from exactly one thread). `Renderer` is `Send + Sync`
/// (verified by a compile-time test in render::context), so a Mutex is
/// sound; contention is negligible (one `render` call per frame).
static RENDER_STATE: std::sync::Mutex<Option<RenderState>> = std::sync::Mutex::new(None);

/// Session id whose surface is currently attached to the (global) render
/// state, 0 = none. `detachWindow` only drops the surface when the caller
/// is the owning session — `switchSession` attaches the new session's
/// surface before releasing the old one, and an unconditional detach would
/// wipe the just-attached surface (black screen for every session after
/// the first).
#[cfg(target_os = "android")]
static ATTACHED_SESSION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct RenderState {
    renderer: crate::render::context::Renderer,
    font_pipeline: crate::render::font::FontPipeline,
    /// Search highlight ranges for the next frame. Set by
    /// `setSearchHighlights` (byte-packed rows/cols), consumed by
    /// `render_inner`. Only the latest input matters — the render loop
    /// runs at ~60 fps and highlights are re-set on every keystroke.
    /// Stored as parsed structs so `render_inner` passes them by slice.
    /// Cleared by `clearSearchHighlights`.
    search_highlights: Vec<crate::render::cell_builder::SearchHighlight>,
    /// Background image pending upload, set by `setBackgroundImage` from
    /// any thread and consumed by the render thread at the start of the
    /// next `render_inner` (same deferred-consume pattern as
    /// `search_highlights`; wgpu texture creation happens on the render
    /// thread). `None` = no pending change.
    pending_bg_image: Option<(Vec<u8>, u32, u32)>,
    /// Set by `clearBackgroundImage`; consumed by the render thread.
    pending_bg_image_clear: bool,
    /// App-level cursor blink (user setting, distinct from the VT cursor
    /// visibility the terminal itself controls). `enabled` + `speed_ms`
    /// come from `setCursorBlink`; `phase_reset_ms` is updated by
    /// `resetCursorBlink` so a user interaction restarts the blink phase
    /// with the cursor visible (round-204).
    cursor_blink_enabled: bool,
    cursor_blink_speed_ms: u64,
    cursor_blink_phase_reset_ms: u64,
    /// Active text selection for the next frame. Set by `setSelection`
    /// (row/col bounds in visible-grid coordinates), consumed by
    /// `render_inner`; same deferred-consume pattern as
    /// `search_highlights`. `selection_bg` is the theme's selection
    /// background color (ARGB from Kotlin, converted to linear f32).
    selection: Option<crate::render::cell_builder::SelectionRange>,
    selection_bg: Option<[f32; 4]>,
    /// Last rendered frame (cells + cursor + dims). Needed for app-level
    /// cursor blink: `render()` only draws when the terminal produced new
    /// CellData, so an idle terminal would never repaint the cursor phase.
    /// When blink is enabled and the phase flips, the cached frame is
    /// redrawn with the cursor visibility toggled (round-204).
    last_frame: Option<(
        Vec<crate::terminal::ghostty_terminal::CellData>,
        crate::terminal::ghostty_terminal::CursorInfo,
        u32,
        u32,
    )>,
    /// Blink phase (0 = visible half, 1 = hidden half) of the last drawn
    /// frame; used to detect phase flips while idle.
    last_blink_phase: Option<u64>,
    /// Selection state at last draw — used by the idle repaint gate to
    /// detect selection changes and force a redraw.
    last_drawn_selection: Option<crate::render::cell_builder::SelectionRange>,
    /// App-level cursor style override (user setting), applied on top of
    /// the terminal's own cursor style. `None` = follow the terminal.
    cursor_style_override: Option<crate::terminal::CursorStyle>,
    /// Bumped by `setCursorStyle`; idle repaint happens when it changes
    /// (same gate as the blink phase).
    cursor_style_version: u64,
    last_drawn_style_version: u64,
}

/// Ensure the render state exists, creating the renderer + font pipeline
/// on first use. Panics on GPU init failure (fatal — no graceful
/// degradation, per project policy: a terminal without rendering is
/// broken and must not limp along).
fn render_state_mut() -> std::sync::MutexGuard<'static, Option<RenderState>> {
    let mut guard = RENDER_STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_none() {
        let renderer = crate::render::context::Renderer::new_with_no_surface();
        let font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        *guard = Some(RenderState {
            renderer,
            font_pipeline,
            search_highlights: Vec::new(),
            selection: None,
            selection_bg: None,
            pending_bg_image: None,
            pending_bg_image_clear: false,
            cursor_blink_enabled: true,
            cursor_blink_speed_ms: 600,
            cursor_blink_phase_reset_ms: 0,
            last_frame: None,
            last_blink_phase: None,
            last_drawn_selection: None,
            cursor_style_override: None,
            cursor_style_version: 0,
            last_drawn_style_version: 0,
        });
        log::info!("render state initialized (renderer + font pipeline)");
    }
    guard
}

/// Lock SESSION_REGISTRY for reading, recovering from poison.
fn rlock_session_registry() -> parking_lot::RwLockReadGuard<'static, HashMap<u64, SessionEntry>> {
    // parking_lot has no poisoning; panic safety is jni_export_guard's job.
    SESSION_REGISTRY.read()
}

/// Lock SESSION_REGISTRY for writing, recovering from poison.
fn wlock_session_registry() -> parking_lot::RwLockWriteGuard<'static, HashMap<u64, SessionEntry>> {
    SESSION_REGISTRY.write()
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
/// Wait for a host-app clipboard answer with a bounded timeout.
///
/// Kotlin always answers `clipboardResult` (even with an empty string on
/// failure), so the only ways to reach the deadline are a dead process or
/// a misbehaving client; an empty answer is the xterm-compatible "empty
/// clipboard" response.
const CLIPBOARD_ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

fn wait_for_clipboard_answer(mut rx: tokio::sync::oneshot::Receiver<String>) -> String {
    let deadline = std::time::Instant::now() + CLIPBOARD_ANSWER_TIMEOUT;
    loop {
        match rx.try_recv() {
            Ok(text) => return text,
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => return String::new(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                if std::time::Instant::now() >= deadline {
                    return String::new();
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}

pub(crate) fn register_request(session_id: u64) -> (u64, tokio::sync::oneshot::Receiver<String>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    REQUEST_REGISTRY.lock().insert((session_id, request_id), tx);
    (request_id, rx)
}

/// Remove a pending dialog/pick_file request without answering it. Called
/// by the MCP tools when their 300s timeout expires so a never-answered
/// request cannot leak one oneshot Sender in REQUEST_REGISTRY per call.
#[cfg(feature = "mcp")]
pub(crate) fn cancel_request(session_id: u64, request_id: u64) {
    REQUEST_REGISTRY.lock().remove(&(session_id, request_id));
    // Round-210 P2-14: tell Kotlin to dismiss the still-visible dialog
    // (the MCP tool call has given up; without this the dialog hangs on
    // screen unresponsive until the process dies).
    push_event(crate::event::Event::DialogCancel {
        session_id,
        request_id,
    });
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
        // Round-217: without TERM, bash's readline treats the terminal as
        // "dumb" and disables input echo entirely — typed characters reach
        // the shell (commands execute) but are never shown. xterm-256color
        // is the standard value for terminal emulators (Termux uses it).
        extra: vec![("TERM".to_string(), "xterm-256color".to_string())],
    };

    match Session::spawn(&shell_path, rows, cols, &shell_env) {
        Ok(mut session) => {
            let id = next_session_id();
            let entry = SessionEntry {
                session: Arc::new(Mutex::new(session)),
                last_scroll_offset: 0,
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
            let session_guard = guard[&id].session.lock();
            let (rows, cols) = session_guard.grid_size();
            mcp.set_terminal_dims(rows, cols);
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
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new("java/lang/RuntimeException", "resize: session not found");
        return;
    };
    let mut session = entry.session.lock();
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
            if let Err(e) =
                env.throw_new("java/lang/RuntimeException", format!("resize: failed: {e}"))
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
    _env: &mut JNIEnv,
    _class: JClass,
    session_id: jlong,
    focused: jboolean,
) -> jboolean {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    if let Some(entry) = registry.get(&id) {
        let mut session = entry.session.lock();
        session.focus_event(focused == JNI_TRUE);
        return JNI_TRUE;
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
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new("java/lang/RuntimeException", "feedPty: session not found");
        return;
    };
    let mut session = entry.session.lock();
    if let Err(e) = session.write(&input) {
        // The master fd is O_NONBLOCK (set in Session::spawn): a full PTY
        // buffer (child not reading) surfaces as EAGAIN. Dropping the
        // input matches xterm behavior; surfacing it as an error would
        // spam the log on every keystroke of a huge paste.
        if e.is_would_block() {
            return;
        }
        let _ = env.throw_new(
            "java/lang/RuntimeException",
            format!("feedPty: write failed: {e}"),
        );
    }
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
        let mut session = entry.session.lock();
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
            let guard = session.as_ref().lock();
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
    let mut pending_clipboard_reads: Vec<(u64, String)> = Vec::new();
    let active_id = ACTIVE_SESSION_ID.load(std::sync::atomic::Ordering::Acquire);
    {
        let registry = rlock_session_registry();
        if active_id != 0 {
            if let Some(entry) = registry.get(&active_id) {
                let mut session = entry.session.lock();
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
                // Check OSC 52 clipboard read request (`ESC ] 52 ; c ; ?`).
                // Collect the selection here (inside the session lock); the
                // one-shot slot + responder thread are set up after the
                // registry/session locks are released (see below) so the
                // lock order stays single-directional.
                if let Some(selection) = session.poll_clipboard_read() {
                    pending_clipboard_reads.push((active_id, selection));
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
            let mut session = entry.session.lock();
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
    // Process pending OSC 52 clipboard read requests (outside the
    // registry/session locks): register a one-shot answer slot, push the
    // event, and spawn a short-lived responder thread that writes the
    // host-app answer back to the PTY. The VT thread must never block on
    // the host app, and clipboardResult may arrive on any thread.
    for (session_id, selection) in pending_clipboard_reads {
        let (request_id, rx) = register_request(session_id);
        pending_events.push(Event::ClipboardRead {
            session_id,
            request_id,
            selection: selection.clone(),
        });
        let registry = wlock_session_registry();
        let session = registry.get(&session_id).map(|entry| entry.session.clone());
        drop(registry);
        if let Some(session) = session {
            std::thread::spawn(move || {
                let text = wait_for_clipboard_answer(rx);
                let mut session = session.lock();
                if let Err(error) = session.answer_clipboard_read(&selection, &text) {
                    log::warn!("osc52: clipboard read answer write-back failed: {error}");
                }
            });
        } else {
            // Session vanished before the responder started; drop the
            // one-shot slot so no Sender leaks in REQUEST_REGISTRY.
            cancel_request(session_id, request_id);
        }
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

    // ADR-0007: hand the ANativeWindow to the renderer. wgpu takes its own
    // reference when the surface is created, so the caller-owned reference
    // from ANativeWindow_fromSurface is released here (the wgpu surface
    // keeps the window alive until detachWindow drops it).
    //
    // Explicitly match the swapchain configuration on the BufferQueue:
    // TextureView-backed surfaces can otherwise stay at a stale/default
    // geometry, and SwiftShader-on-emulator refuses to dequeue buffers
    // whose format/geometry disagree with the queue (dequeueBuffer
    // timeout). WINDOW_FORMAT_RGBA_8888 = 1.
    // SAFETY: `ptr` is a valid ANativeWindow* (checked non-null above);
    // ANativeWindow_setBuffersGeometry is a documented NDK function.
    unsafe {
        ANativeWindow_setBuffersGeometry(ptr, width, height, 1);
    }
    let mut state = render_state_mut();
    let renderer = &mut state
        .as_mut()
        .expect("render_state_mut always initializes")
        .renderer;
    let session_id = _session_id as u64;
    match renderer.attach_surface(ptr.cast(), width.max(0) as u32, height.max(0) as u32) {
        Ok(()) => {
            ATTACHED_SESSION_ID.store(session_id, std::sync::atomic::Ordering::Release);
            log::info!("FFI: attachWindow surface attached (session {session_id})");
        }
        Err(error) => {
            log::error!("FFI: attachWindow surface attach failed: {error}");
        }
    }
    // SAFETY: `ptr` is a valid ANativeWindow* from
    // `ANativeWindow_fromSurface`. Whether attach succeeded or failed, the
    // caller-owned reference must be released: wgpu acquired its own
    // reference when the surface was created (success path), and nobody
    // adopted it otherwise (failure path). `ANativeWindow_release` is a
    // documented NDK function that decrements the window's refcount.
    unsafe { ANativeWindow_release(ptr) };
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: render — render one frame from the CellData fast path
// ══════════════════════════════════════════════════════════════════════════

/// Render one frame for the active session. Returns:
/// - 1 if output was available and a frame was presented,
/// - 0 if the session had no pending cell data (idle),
/// - -1 on error (surface missing, GPU failure, unknown session).
///
/// Called from Kotlin's render loop (one thread). `width`/`height` are
/// advisory (the attached surface config owns the real dimensions) but
/// kept so a future resize path can reconfigure the swapchain here.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_render<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    _width: jint,
    _height: jint,
) -> jint {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, -1, { render_inner(session_id as u64) })
}

fn render_inner(session_id: u64) -> jint {
    // Consume any pending background-image change before rendering:
    // `setBackgroundImage`/`clearBackgroundImage` JNI calls store their
    // payload here (any thread); the wgpu texture upload happens on the
    // render thread where `Renderer` is used.
    {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            if render_state.pending_bg_image_clear {
                render_state.renderer.clear_bg_image();
                render_state.pending_bg_image_clear = false;
            }
            if let Some((data, w, h)) = render_state.pending_bg_image.take() {
                render_state.renderer.set_bg_image(&data, w, h);
                log::info!(
                    "render_inner: consumed bg image {w}x{h}, view={}",
                    render_state.renderer.bg_image_view.is_some()
                );
            }
        }
    }
    // Check surface readiness first (cheap lock, no session lock held):
    // when no surface is attached there is nothing to present, and
    // draining the CellData channel in that state would drop the latest
    // snapshot for no benefit. Keeps the two locks disjoint.
    {
        let state = RENDER_STATE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ready = state.as_ref().is_some_and(|s| s.renderer.surface.is_some());
        if !ready {
            return 0;
        }
    }
    // Upload glyph atlas dirty regions first, unconditionally (even on
    // idle frames): glyph rasterization happens inside render_cell_data,
    // so its dirty rect must be consumed on the NEXT render call. If we
    // returned early on idle frames without uploading, a newly rasterized
    // glyph would never reach the GPU texture until more output arrives.
    {
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            log::error!("render: render state missing");
            return -1;
        };
        if render_state.renderer.surface.is_none() {
            // No surface attached yet (attachWindow not called / surface lost).
            return 0;
        }
        // Lazy one-time pipeline creation (mirrors the test helpers): the
        // cell pipeline is created from the attached surface's format on
        // first render.
        if render_state.renderer.cell_pipeline.is_none() {
            let (w, h) = render_state
                .renderer
                .surface_config
                .as_ref()
                .map_or((0, 0), |c| (c.width, c.height));
            if w == 0 || h == 0 {
                return 0;
            }
            render_state
                .renderer
                .initialize_pipeline_and_bind_group(ATLAS_SIZE, ATLAS_SIZE, w, h);
        }
        if let Some(rect) = render_state.font_pipeline.take_dirty_rect() {
            let (aw, ah) = render_state.font_pipeline.atlas_dimensions();
            render_state.renderer.upload_atlas(
                render_state.font_pipeline.atlas_bitmap(),
                aw,
                ah,
                Some(rect),
            );
        }
    }
    // Collect cell data (session lock), then render (render-state lock).
    // NOTE (round-209, P2-4): the idle branch below DOES hold the session
    // lock while touching render state — the lock order is strictly
    // SESSION_REGISTRY → session → render_state everywhere (never the
    // reverse), so there is no deadlock cycle, but the session lock's
    // critical section is widened by the cached-frame clone. This is
    // accepted: the clone is a bounded 80-byte-per-cell Vec and the
    // render-state lock is never contended by another lock holder.
    let (cells, cursor_info, rows, cols, is_new_data, scrollback) = {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&session_id) else {
            log::warn!("render: unknown session {session_id}");
            return -1;
        };
        let session = entry.session.lock();
        let scrollback = session.terminal().scrollback_length();
        match session.terminal().receive_cell_data() {
            Some((cells, cursor_info)) => {
                let (rows, cols) = session.grid_size();
                (cells, cursor_info, rows, cols, true, scrollback)
            }
            None => {
                // Idle. Blink repaint decision is made below under the
                // render-state lock; here we only need to know whether a
                // cached frame exists (checked there).
                let state = render_state_mut();
                let Some(render_state) = (*state).as_ref() else {
                    return 0;
                };
                match &render_state.last_frame {
                    Some((cached_cells, cached_cursor, cached_rows, cached_cols)) => (
                        cached_cells.clone(),
                        *cached_cursor,
                        *cached_rows,
                        *cached_cols,
                        false,
                        scrollback,
                    ),
                    None => return 0,
                }
            }
        }
    };

    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::error!("render: render state missing");
        return -1;
    };
    if render_state.renderer.surface.is_none() {
        // No surface attached yet (attachWindow not called / surface lost).
        return 0;
    }
    let mut cursor = crate::render::CellCursor {
        row: cursor_info.row,
        col: cursor_info.col,
        visible: cursor_info.visible,
        style: render_state
            .cursor_style_override
            .unwrap_or(cursor_info.style),
        color: None,
    };
    // App-level cursor blink (user setting): when enabled, hide the
    // cursor during the second half of each blink cycle, measured from
    // the last phase reset. The terminal's own cursor visibility still
    // gates it (`cursor_info.visible`).
    let mut blink_phase = 0u64;
    if render_state.cursor_blink_enabled {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let speed = render_state.cursor_blink_speed_ms.max(50);
        let phase = now_ms.saturating_sub(render_state.cursor_blink_phase_reset_ms);
        blink_phase = (phase / speed) % 2;
        if blink_phase == 1 {
            cursor.visible = false;
        }
    }
    // Idle repaint gate: only redraw the cached frame when the blink
    // phase actually flipped (otherwise every render() call would repaint
    // at full rate and burn CPU on an idle terminal).
    if !is_new_data {
        let phase_changed = render_state.last_blink_phase != Some(blink_phase);
        let style_changed =
            render_state.last_drawn_style_version != render_state.cursor_style_version;
        let selection_changed = render_state.selection != render_state.last_drawn_selection;
        if !render_state.cursor_blink_enabled && !style_changed && !selection_changed {
            return 0;
        }
        if render_state.cursor_blink_enabled
            && !phase_changed
            && !style_changed
            && !selection_changed
        {
            return 0;
        }
    }
    // Round-216: selection rows are stored in GRID coordinates (the Kotlin
    // side uses scrollbackLine(gridRow)), but CellData rows are VISIBLE
    // rows (0..rows-1 of the viewport). Translate here each frame so the
    // highlight follows the text as the user scrolls. Done under the
    // render-state lock only (scrollback was captured under the session
    // lock above, preserving SESSION_REGISTRY → session → render_state).
    let render_selection = render_state.selection.map(|mut sel| {
        sel.start_row -= scrollback as i32;
        sel.end_row -= scrollback as i32;
        sel
    });
    let result = render_state.renderer.render_cell_data(
        &cells,
        rows,
        cols,
        cursor,
        &mut render_state.font_pipeline,
        ATLAS_SIZE as f32,
        ATLAS_SIZE as f32,
        render_selection,
        render_state.selection_bg,
        &render_state.search_highlights,
    );
    if result.is_ok() {
        render_state.last_frame = Some((cells, cursor_info, rows, cols));
        render_state.last_blink_phase = Some(blink_phase);
        render_state.last_drawn_selection = render_state.selection;
        render_state.last_drawn_style_version = render_state.cursor_style_version;
    }
    match result {
        Ok(()) => 1,
        Err(error) => {
            log::error!("render: frame failed: {error}");
            -1
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: detachWindow
// ══════════════════════════════════════════════════════════════════════════

/// Detach the current surface — Android only.
///
/// Real implementation: owner-checked (ATTACHED_SESSION_ID CAS) release
/// of the wgpu surface; the session's RenderState is torn down so the
/// next attach recreates it.
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
    let session_id = _session_id as u64;
    // Only the session that owns the attached surface may drop it:
    // switchSession attaches the new session's surface BEFORE detaching
    // the old one, so an unconditional detach would wipe the just-attached
    // surface (black screen for every session after the first).
    //
    // The ownership check and the surface drop happen under the SAME
    // RENDER_STATE lock (attach_window_inner also takes it): an
    // owner-check-then-drop split would let a concurrent attach (new owner
    // stored) slip in between and have its fresh surface dropped.
    let Ok(mut state) = RENDER_STATE.lock() else {
        log::error!("detachWindow: render state lock poisoned");
        return;
    };
    if ATTACHED_SESSION_ID
        .compare_exchange(
            session_id,
            0,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_err()
    {
        log::debug!(
            "detachWindow: session {session_id} does not own the attached surface, ignoring",
        );
        return;
    }
    log::info!("FFI: detachWindow (session {session_id})");
    // ADR-0007: drop the wgpu surface; the next attachWindow recreates it.
    // The ANativeWindow reference wgpu held is released when the Surface
    // is dropped.
    if let Some(render_state) = state.as_mut() {
        render_state.renderer.release_surface();
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: setMcpEnabled
// ══════════════════════════════════════════════════════════════════════════

/// Override the MCP Unix socket path (round-210 P2-13). Kotlin derives it
/// from `context.filesDir` so it follows the real `applicationId` instead
/// of the hardcoded `/data/data/com.termux` default.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setMcpSocketPath(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) {
    jni_export_guard!(&mut env, (), {
        #[cfg(feature = "mcp")]
        {
            if let Ok(s) = env.get_string(&path) {
                crate::mcp::set_socket_path(s.into());
                log::info!("setMcpSocketPath: {}", crate::mcp::socket_path());
            }
        }
    })
}

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
                    Some(entry) => {
                        let session = entry.session.lock();
                        match session.send_signal(signum) {
                            Ok(()) => format!("Signal {signum} sent to session {session_id}"),
                            Err(e) => format!("send_signal failed: {e}"),
                        }
                    }
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

    if let Some(tx) = REQUEST_REGISTRY.lock().remove(&(session_id, request_id)) {
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
    let result_str: String = env
        .get_string(&result)
        .map(|s| s.into())
        .unwrap_or_default();
    answer_request(session_id, request_id, result_str);
}

/// Answer a pending dialog/pick_file/clipboard request, resolving the
/// MCP tool's oneshot receiver. No-op for unknown (already-answered or
/// expired) request ids. Shared by the JNI exports and unit tests.
#[cfg(feature = "mcp")]
pub(crate) fn answer_request(session_id: u64, request_id: u64, result: String) {
    if let Some(tx) = REQUEST_REGISTRY.lock().remove(&(session_id, request_id)) {
        let _ = tx.send(result);
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

/// Atlas size shared by pipeline init, render_cell_data and the font
/// pipeline. Kept in one place: changing it requires all three call sites
/// to agree or glyph UVs misalign.
const ATLAS_SIZE: u32 = 1024;

// ══════════════════════════════════════════════════════════════════════════
// JNI Exports: TerminalQueryPort (search/scrollback/text/font)
//
// Native side of the Kotlin TerminalQueryPort seam (audit P1 #6). Each
// export obeys the resize_inner pattern: registry rlock -> session lock ->
// engine query; unknown session id throws IllegalArgumentException. Font
// queries need no session id; they read the global RENDER_STATE.
// ══════════════════════════════════════════════════════════════════════════

/// Returns the terminal title (OSC 0/2) for a session, or null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTitle<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "getTitle: session not found",
            );
            return std::ptr::null_mut();
        };
        let session = entry.session.lock();
        let title = session.terminal().title();
        drop(session);
        drop(registry);
        match env.new_string(&title) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Returns the number of scrollback rows for a session.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_scrollbackLength(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut env, 0, {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "scrollbackLength: session not found",
            );
            return 0;
        };
        let session = entry.session.lock();
        session.terminal().scrollback_length() as jint
    })
}

/// Returns a single scrollback row's trimmed text, or null for an empty row.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_scrollbackLine<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "scrollbackLine: session not found",
            );
            return std::ptr::null_mut();
        };
        let Ok(row) = u32::try_from(row) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "scrollbackLine: row must be non-negative",
            );
            return std::ptr::null_mut();
        };
        let session = entry.session.lock();
        // Kotlin passes an absolute row (scrollback + viewport offset via
        // `scrollbackLength - scrollOffset + row`), so pass it straight
        // through to read_line_text (which expects absolute rows).
        let text = session.terminal().read_line_text(row);
        drop(session);
        drop(registry);
        match text {
            Some(text) => match env.new_string(&text) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            None => std::ptr::null_mut(),
        }
    })
}

/// Returns visible + scrollback text joined by newlines (dump_grid path).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getTerminalText<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "getTerminalText: session not found",
            );
            return std::ptr::null_mut();
        };
        let session = entry.session.lock();
        let grid = session.terminal().dump_grid();
        drop(session);
        drop(registry);

        let mut lines: Vec<String> =
            Vec::with_capacity((grid.scrollback.len() + grid.rows as usize) as usize);
        for row_cells in &grid.scrollback {
            let line: String = row_cells
                .iter()
                .filter_map(|c| char::from_u32(c.codepoint).filter(|ch| *ch != '\0'))
                .collect();
            lines.push(line.trim_end().to_string());
        }
        for row in 0..grid.rows as usize {
            let start = row * grid.cols as usize;
            let end = start
                .saturating_add(grid.cols as usize)
                .min(grid.visible.len());
            if start >= end {
                continue;
            }
            let line: String = grid.visible[start..end]
                .iter()
                .filter_map(|c| char::from_u32(c.codepoint).filter(|&c| c != '\0'))
                .collect::<String>()
                .trim_end()
                .to_string();
            lines.push(line);
        }
        let text = lines.join("\n");
        match env.new_string(&text) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Returns a JSON array of `{row,start_col,end_col}` search matches, or
/// `[]` on timeout/disconnect. Column indices are character columns.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_searchAllInScrollback<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    query: JString<'local>,
    case_sensitive: jboolean,
    fuzzy: jboolean,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let id = session_id as u64;
        let Ok(query) = env.get_string(&query) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "searchAllInScrollback: bad query string",
            );
            return std::ptr::null_mut();
        };
        let query: String = query.into();
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "searchAllInScrollback: session not found",
            );
            return std::ptr::null_mut();
        };
        let session = entry.session.lock();
        let matches =
            session
                .terminal()
                .search_all_in_scrollback(&query, case_sensitive != 0, fuzzy != 0);
        log::info!(
            "searchAllInScrollback: query={query:?} matches={}",
            matches.len(),
        );
        drop(session);
        drop(registry);

        let json = serde_json::to_string(
            &matches
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "row": m.row,
                        "start_col": m.start_col,
                        "end_col": m.end_col,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".into());
        match env.new_string(&json) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Returns true when the cell has no printable codepoint.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_isCellEmpty(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jboolean {
    jni_export_guard!(&mut env, JNI_TRUE, {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "isCellEmpty: session not found",
            );
            return JNI_TRUE;
        };
        let Ok(row) = u32::try_from(row) else {
            return JNI_TRUE;
        };
        let session = entry.session.lock();
        // gridRow from Kotlin is already an absolute row number
        // (scrollback + visibleOffset - scrollOffset). Do NOT add
        // scrollback again — that would double-count it.
        let absolute = row as usize;
        let visible_rows = session.terminal().rows();
        let scrollback = session.terminal().scrollback_length();
        let mut empty = true;
        if (absolute as u32) < visible_rows + scrollback {
            if let Some(line) = session.terminal().read_line_text(row) {
                // Round-209 P2-5: `col` is a CHARACTER column, but the raw
                // line is UTF-8 — comparing against line.len() (bytes)
                // misjudged multi-byte cells (CJK/emoji) as empty. Count
                // code points instead.
                let char_col = col.max(0) as usize;
                let char_len = line.chars().count();
                log::debug!(
                    "isCellEmpty({row},{col}): scrollback={scrollback} rows={visible_rows} absolute={absolute} line={line:?} char_len={char_len}"
                );
                empty = char_col >= char_len;
            } else {
                log::debug!(
                    "isCellEmpty({row},{col}): read_line_text({absolute}) returned None (scrollback={scrollback})"
                );
            }
        } else {
            log::debug!(
                "isCellEmpty({row},{col}): absolute={absolute} out of range rows+scrollback={}",
                visible_rows + scrollback
            );
        }
        if empty { JNI_TRUE } else { JNI_FALSE }
    })
}

/// Returns the list of monospace font families the pipeline knows.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_listFontFamilies<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jobjectArray {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return std::ptr::null_mut();
        };
        let families = render_state.font_pipeline.list_monospace_fonts();
        drop(state);

        let string_class = env.find_class("java/lang/String");
        let Ok(string_class) = string_class else {
            return std::ptr::null_mut();
        };
        let array = env.new_object_array(families.len() as jsize, string_class, JObject::null());
        let Ok(array) = array else {
            return std::ptr::null_mut();
        };
        for (i, family) in families.iter().enumerate() {
            if let Ok(s) = env.new_string(family) {
                let _ = env.set_object_array_element(&array, i as jsize, s);
            }
        }
        array.into_raw()
    })
}

/// Returns the default font family name.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getDefaultFontName<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return std::ptr::null_mut();
        };
        let name = render_state.font_pipeline.default_font_name();
        match env.new_string(&name) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Returns font information string (active + CJK fallback).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getFontInfo<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return std::ptr::null_mut();
        };
        let info = render_state.font_pipeline.font_information();
        drop(state);
        match env.new_string(&info) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Clears the renderer's search highlight ranges for the next frame.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clearSearchHighlights(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.search_highlights.clear();
        }
    })
}

/// Set search highlight ranges. `data` is byte-packed: a 4-byte match
/// count (i32 LE) followed by that many 16-byte records:
///
///   [0..4]    match count (i32 LE), authoritative
///   [4..]     16-byte records: row(i32) start(i32) end(i32) RGBA(u8x4)
///
/// Kotlin's TerminalSurface packs matches this way and calls
/// `bridge.setSearchHighlights(data.copyOf())`; the render loop consumes
/// the parsed list on the next frame.
#[unsafe(no_mangle)]
// JNI exports receive raw handles (jbyteArray/jstring are pointer types)
// whose validity is the JVM's contract, not a Rust lifetime guarantee;
// the SAFETY comment inside documents the contract (feedPty pattern).
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSearchHighlights(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    data: jbyteArray,
) {
    jni_export_guard!(&mut env, (), {
        // SAFETY: `data` is a JNI method argument, guaranteed valid by the
        // JVM runtime for the duration of this call. `from_raw` wraps the
        // pointer without taking ownership; the local ref is released when
        // this native method returns (same pattern as feed_pty_inner).
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(data) };
        let Some(bytes) = env.convert_byte_array(&byte_array).ok() else {
            return;
        };
        // Wire format (see Kotlin TerminalScreen.search results packing):
        //   [0..4]   match count (i32 LE)  -- the count prefix IS the
        //            authoritative length; trailing bytes (a last partial
        //            16-byte record) are ignored defensively.
        //   [4..]    16-byte records: row(i32) start(i32) end(i32) RGBA(u8x4)
        let Some(prefix) = bytes.get(0..4) else {
            return;
        };
        let count =
            i32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]).max(0) as usize;
        // Cap the claimed count at the number of complete records actually
        // present: a corrupt/huge count must not force an unbounded
        // Vec::with_capacity allocation.
        let count = count.min(bytes.len().saturating_sub(4) / 16);
        let mut highlights = Vec::with_capacity(count);
        for chunk in bytes[4..].chunks_exact(16).take(count) {
            let row = i32::from_le_bytes(chunk[0..4].try_into().unwrap());
            let start_col = i32::from_le_bytes(chunk[4..8].try_into().unwrap());
            let end_col_exclusive = i32::from_le_bytes(chunk[8..12].try_into().unwrap());
            let color = [chunk[12], chunk[13], chunk[14], chunk[15]];
            highlights.push(crate::render::cell_builder::SearchHighlight {
                row,
                start_col,
                end_col_exclusive,
                color,
            });
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.search_highlights = highlights;
        }
    });
}

/// Set the active text selection for the next rendered frame.
///
/// Coordinates are visible-grid rows/cols (as produced by `build_cell_data`
/// — row 0 is the top visible line), which is what Kotlin's long-press /
/// drag handle logic works in. `hasSelection=false` clears the selection.
/// `mode` follows the Kotlin `SelectionMode` ordinal (Char=0, Word=1,
/// Line=2, Block=3, Semantic=4 — note Block/Semantic are swapped relative
/// to the Rust enum, mapped below). `selectionBgArgb` is the theme's
/// selection background color (ARGB packed, e.g. 0xFF45475A), converted
/// to linear f32 for the shader.
#[unsafe(no_mangle)]
// JNI exports receive raw handles (jstring/jbyteArray are pointer types)
// whose validity is the JVM's contract, not a Rust lifetime guarantee.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSelection(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    start_row: jint,
    start_col: jint,
    end_row: jint,
    end_col: jint,
    has_selection: jboolean,
    mode: jbyte,
    selection_bg_argb: jint,
) {
    jni_export_guard!(&mut env, (), {
        // Kotlin SelectionMode ordinal → Rust SelectionMode.
        let mode = match mode {
            0 => crate::terminal::SelectionMode::Char,
            1 => crate::terminal::SelectionMode::Word,
            2 => crate::terminal::SelectionMode::Line,
            3 => crate::terminal::SelectionMode::Block,
            4 => crate::terminal::SelectionMode::Semantic,
            _ => crate::terminal::SelectionMode::Char,
        };
        let argb = selection_bg_argb as u32;
        let selection_bg = [
            ((argb >> 16) & 0xFF) as f32 / 255.0,
            ((argb >> 8) & 0xFF) as f32 / 255.0,
            (argb & 0xFF) as f32 / 255.0,
            ((argb >> 24) & 0xFF) as f32 / 255.0,
        ];
        let selection = if has_selection == jni::sys::JNI_TRUE {
            Some(crate::render::cell_builder::SelectionRange {
                start_row,
                start_col,
                end_row,
                end_col,
                active: true,
                mode,
                origin: None,
                is_empty: false,
            })
        } else {
            None
        };
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.selection = selection;
            render_state.selection_bg = if selection.is_some() {
                Some(selection_bg)
            } else {
                None
            };
        }
    });
}

/// Apply a theme: 54 bytes = background RGB (3) + foreground RGB (3) +
/// 16 ANSI palette colors (48). Mirrors `GhosttyTerminal::set_theme`.
#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)] // JNI signatures contain raw pointers by design
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setTheme(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) {
    jni_export_guard!(&mut env, (), {
        let id = session_id as u64;
        // SAFETY: `data` is the JNI `jbyteArray` argument validated by the
        // JVM before this export is called; the jni crate's `from_raw` only
        // wraps the pointer, and `convert_byte_array` performs the bounds
        // checks against the actual array length.
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(data) };
        let bytes = match env.convert_byte_array(&byte_array) {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = env.throw_new(
                    "java/lang/IllegalArgumentException",
                    "setTheme: cannot read byte array",
                );
                return;
            }
        };
        if bytes.len() != 54 {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "setTheme: expected exactly 54 bytes (bg3 fg3 ansi48)",
            );
            return;
        }
        let background = [bytes[0], bytes[1], bytes[2]];
        let foreground = [bytes[3], bytes[4], bytes[5]];
        let mut ansi = [[0u8; 3]; 16];
        for (i, color) in ansi.iter_mut().enumerate() {
            let base = 6 + i * 3;
            *color = [bytes[base], bytes[base + 1], bytes[base + 2]];
        }
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "setTheme: session not found",
            );
            return;
        };
        let session = entry.session.lock();
        session.terminal().set_theme(background, foreground, ansi);
        // The cell shader's Fix F transparency check compares each cell's
        // background against `uniforms.default_bg`, which is sourced from
        // `Renderer::bg_color`. Without syncing it here, the terminal
        // theme (e.g. #151515) never matches the renderer default
        // (#1E1E2E Catppuccin), `is_default_bg` stays false, and the
        // wallpaper is hidden behind opaque cell backgrounds
        // (emulator-verified, round-203: checkerboard probe proved the bg
        // pass and cell transparency both work; only the default_bg
        // comparison failed).
        {
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                render_state.renderer.set_bg_color(background);
            }
        }
        log::info!("setTheme: session {id} bg={background:02X?} fg={foreground:02X?}");
    })
}

/// Set the terminal background image. RGBA bytes, decoded on the Kotlin
/// side (TerminalViewModel). The upload is deferred to the render thread
/// via `RenderState::pending_bg_image` — the same pattern as
/// `setSearchHighlights` — so wgpu texture creation happens where the
/// renderer is used.
#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)] // JNI signatures contain raw pointers by design
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setBackgroundImage(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    data: jbyteArray,
    width: jint,
    height: jint,
) {
    jni_export_guard!(&mut env, (), {
        let (Some(w), Some(h)) = (u32::try_from(width).ok(), u32::try_from(height).ok()) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "setBackgroundImage: width/height must be non-negative",
            );
            return;
        };
        if w == 0 || h == 0 {
            // Zero-sized image: treat as clear. Avoids a degenerate
            // 0-byte texture below.
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                render_state.pending_bg_image_clear = true;
            }
            return;
        }
        // SAFETY: `data` is a JNI method argument, guaranteed valid by the
        // JVM runtime for the duration of this call (same pattern as
        // feed_pty_inner).
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(data) };
        let Some(bytes) = env.convert_byte_array(&byte_array).ok() else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "setBackgroundImage: cannot read byte array",
            );
            return;
        };
        let expected = w as usize * h as usize * 4;
        if bytes.len() < expected {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                format!(
                    "setBackgroundImage: expected {expected} bytes (RGBA {w}x{h}), got {}",
                    bytes.len()
                ),
            );
            return;
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.pending_bg_image = Some((bytes, w, h));
        }
        log::info!("setBackgroundImage: {w}x{h} queued for render thread");
    })
}

/// Clear the terminal background image. Deferred to the render thread
/// like `setBackgroundImage`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_clearBackgroundImage(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.pending_bg_image = None;
            render_state.pending_bg_image_clear = true;
        }
        log::info!("clearBackgroundImage: queued for render thread");
    })
}

/// Set the background-image blur radius and opacity. Deferred to the
/// render thread state (the renderer is owned by RENDER_STATE); the next
/// `begin_frame` picks up the new values. `alpha` arrives scaled by 10
/// from Kotlin (`settings.backgroundAlpha * 10`), so it is divided here.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setBackgroundParams(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    blur_radius: jint,
    alpha_tenths: jint,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            let blur = blur_radius as f32;
            let alpha = alpha_tenths as f32 / 10.0;
            render_state.renderer.set_background_params(blur, alpha);
            log::info!("setBackgroundParams: blur={blur} alpha={alpha}");
        }
    })
}

/// Set the app-level cursor blink (user setting). `enabled=false` forces
/// the cursor to follow only the terminal's own visibility; `enabled=true`
/// blinks at `speed_ms` (clamped 50..=2000) from the last phase reset.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setCursorBlink(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    enabled: jboolean,
    speed_ms: jint,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.cursor_blink_enabled = enabled != 0;
            render_state.cursor_blink_speed_ms = (speed_ms as u64).clamp(50, 2000);
            log::info!(
                "setCursorBlink: enabled={} speed={}ms",
                render_state.cursor_blink_enabled,
                render_state.cursor_blink_speed_ms
            );
        }
    })
}

/// Restart the app-level blink phase with the cursor visible (called on
/// user interaction so the cursor reappears immediately).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_resetCursorBlink(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.cursor_blink_phase_reset_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            log::info!("resetCursorBlink: phase reset");
        }
    })
}

/// Pause/resume the renderer (e.g. while the settings screen is open or
/// the surface is destroyed). The Rust renderer already checks
/// `render_paused` in render_frame; this JNI export is the missing wire.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRenderPaused(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    paused: jboolean,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.renderer.set_render_paused(paused != 0);
            log::info!("setRenderPaused: paused={}", paused != 0);
        }
    })
}

/// App-level cursor style override ("block" | "bar" | "underline").
/// Any other value clears the override (follow the terminal).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setCursorStyle(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    style: JString,
) {
    jni_export_guard!(&mut env, (), {
        let style_str = match env.get_string(&style) {
            Ok(s) => s.to_string_lossy().into_owned(),
            Err(_) => return,
        };
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.cursor_style_override = match style_str.as_str() {
                "block" => Some(crate::terminal::CursorStyle::Block),
                "bar" => Some(crate::terminal::CursorStyle::Bar),
                "underline" => Some(crate::terminal::CursorStyle::Underline),
                _ => None,
            };
            render_state.cursor_style_version = render_state.cursor_style_version.wrapping_add(1);
            log::info!("setCursorStyle: {style_str}");
        }
    })
}

/// Set the font family of the renderer's font pipeline. Returns true if
/// the family was found.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontFamily(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    family: JString,
) -> jboolean {
    jni_export_guard!(&mut env, JNI_FALSE, {
        let family_str = match env.get_string(&family) {
            Ok(s) => s.to_string_lossy().into_owned(),
            Err(_) => return JNI_FALSE,
        };
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return JNI_FALSE;
        };
        let found = render_state.font_pipeline.set_font_family(&family_str);
        log::info!("setFontFamily: {family_str} found={found}");
        if found { JNI_TRUE } else { JNI_FALSE }
    })
}

/// Set the font size (in tenths of a pixel, matching the Kotlin slider).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontSizeInPlace(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    size_tenths: jint,
) {
    jni_export_guard!(&mut env, (), {
        let size = (size_tenths as f32) / 10.0;
        if !(4.0..=100.0).contains(&size) {
            return;
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            let (cw, ch) = render_state.font_pipeline.set_font_size_in_place(size);
            log::info!(
                "setFontSizeInPlace: {} -> cell {cw:.1}x{ch:.1}",
                size_tenths
            );
        }
    })
}

/// Set the font rasterization scale (device pixel density). Glyph bitmaps
/// are rasterized at font_size * raster_scale so text stays crisp on
/// high-density screens (round-215).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setRasterScale(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    scale: jfloat,
) {
    jni_export_guard!(&mut env, (), {
        if !(0.5..=8.0).contains(&scale) {
            return;
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            // Round-215: BOTH pipelines must agree on raster_scale — the
            // font pipeline rasterizes glyph bitmaps at font_size*scale,
            // while the renderer feeds the same scale to the cell shader
            // uniform. Desync made the shader sample glyph bitmaps at 1x
            // while cell_builder computed bearings at Nx, drawing glyphs
            // as distorted corner triangles.
            render_state.font_pipeline.set_raster_scale(scale);
            render_state.renderer.set_raster_scale(scale);
        }
    })
}

/// Load a custom font file into the renderer's font database. Returns the
/// first family name from the file, or null on failure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_loadFontFile<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _session_id: jlong,
    path: JString<'local>,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let path_str = match env.get_string(&path) {
            Ok(s) => s.to_string_lossy().into_owned(),
            Err(_) => return std::ptr::null_mut(),
        };
        // Custom font loading is an Android feature (the font database
        // with extra paths is android-only; the desktop build uses system
        // fonts). Reject the load on other targets.
        #[cfg(target_os = "android")]
        let family = {
            // Probe the file with a fresh fontdb to discover the family name.
            let mut db = crate::render::font::font_db::load_font_database();
            let ids =
                db.load_font_source(fontdb::Source::File(std::path::PathBuf::from(&path_str)));
            let Some(family) = ids
                .iter()
                .filter_map(|id| db.face(*id))
                .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
                .next()
            else {
                log::warn!("loadFontFile: no family name in {path_str}");
                return std::ptr::null_mut();
            };
            // Register the file with the renderer and rebuild its pipeline
            // so the new family is selectable.
            crate::render::font::font_db::set_extra_font_paths(vec![std::path::PathBuf::from(
                &path_str,
            )]);
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                let (aw, ah) = render_state.font_pipeline.atlas_dimensions();
                let font_size = render_state.font_pipeline.font_size();
                let (aw, ah) = (aw as i32, ah as i32);
                render_state.font_pipeline =
                    crate::render::font::FontPipeline::new(aw, ah, font_size);
                let _ = render_state.font_pipeline.set_font_family(&family);
            }
            family
        };
        #[cfg(not(target_os = "android"))]
        let family: String = {
            log::warn!("loadFontFile: unsupported on this target");
            String::new()
        };
        if family.is_empty() {
            return std::ptr::null_mut();
        }
        log::info!("loadFontFile: {} -> family {family}", path_str);
        match env.new_string(&family) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Set the renderer's system locale (used for font fallback ordering).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setSystemLocale(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    locale: JString,
) {
    jni_export_guard!(&mut env, (), {
        let locale_str = match env.get_string(&locale) {
            Ok(s) => s.to_string_lossy().into_owned(),
            Err(_) => return,
        };
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.font_pipeline.set_system_locale(&locale_str);
            log::info!("setSystemLocale: {locale_str}");
        }
    })
}

/// Register extra font directories/files (the app-private fonts dir).
/// The renderer's pipeline reads these when created; if it already
/// exists it is rebuilt so the new fonts become selectable.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setExtraFontPaths(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    paths: jobjectArray,
) {
    jni_export_guard!(&mut env, (), {
        #[cfg(target_os = "android")]
        {
            // Parse a String[] from Java.
            let mut path_list: Vec<std::path::PathBuf> = Vec::new();
            // SAFETY: `paths` is a JNI method argument, guaranteed valid by
            // the JVM runtime for the duration of this call (same pattern
            // as feed_pty_inner / setBackgroundImage).
            let array = unsafe { jni::objects::JObjectArray::from_raw(paths) };
            let len = env.get_array_length(&array).unwrap_or(0);
            for i in 0..len {
                let item = env.get_object_array_element(&array, i).ok();
                if let Some(item) = item {
                    let s = JString::from(item);
                    if let Ok(s) = env.get_string(&s) {
                        path_list.push(std::path::PathBuf::from(s.to_string_lossy().into_owned()));
                    }
                }
            }
            crate::render::font::font_db::set_extra_font_paths(path_list);
            // Rebuild the pipeline if it already exists so the fonts are
            // immediately selectable.
            let mut state = render_state_mut();
            if let Some(render_state) = state.as_mut() {
                let (aw, ah) = render_state.font_pipeline.atlas_dimensions();
                let font_size = render_state.font_pipeline.font_size();
                let (aw, ah) = (aw as i32, ah as i32);
                render_state.font_pipeline =
                    crate::render::font::FontPipeline::new(aw, ah, font_size);
            }
            log::info!("setExtraFontPaths: registered extra font paths");
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (&mut env, paths);
            log::warn!("setExtraFontPaths: unsupported on this target");
        }
    })
}

/// Current cell width in pixels (from the renderer's font pipeline).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCellWidth(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) -> jfloat {
    jni_export_guard!(&mut env, 0.0, {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return 0.0;
        };
        render_state.font_pipeline.cell_metrics().0
    })
}

/// Current cell height in pixels (from the renderer's font pipeline).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCellHeight(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) -> jfloat {
    jni_export_guard!(&mut env, 0.0, {
        let state = render_state_mut();
        let Some(render_state) = state.as_ref() else {
            return 0.0;
        };
        render_state.font_pipeline.cell_metrics().1
    })
}

/// Current grid dimensions as (rows << 32) | cols, or 0 when the session
/// is unknown. Backs `Bridge.getGridRowsColsPacked` (round-204: the
/// Kotlin stub returned 0 forever, so syncGridDimensions could never
/// converge on the native grid after a dropped resize).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getGridRowsColsPacked(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut env, 0, {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(_session_id as u64)) else {
            return 0;
        };
        let session = entry.session.lock();
        let (rows, cols) = session.grid_size();
        ((rows as i64) << 32) | (cols as i64)
    })
}

/// Set the viewport scroll offset (rows into scrollback; 0 = active
/// screen). The difference from the previous offset is applied on the VT
/// thread via `scroll_viewport(Delta)`, so the next CellData push carries
/// the scrolled view (round-205: previously a Kotlin-side no-op).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setScrollOffset(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    offset: jint,
) {
    jni_export_guard!(&mut env, (), {
        let target = offset.max(0) as i64;
        let mut registry = wlock_session_registry();
        let Some(entry) = registry.get_mut(&(_session_id as u64)) else {
            return;
        };
        // Per-session delta (round-209): the previous code computed the
        // delta against a single global `render_state.scroll_offset`, so
        // switching sessions polluted the resumed session's viewport.
        let delta = target - entry.last_scroll_offset;
        if delta == 0 {
            return;
        }
        let session = entry.session.lock();
        // scroll_viewport delta semantics (verified on host + emulator):
        // NEGATIVE = scroll up into history, POSITIVE = back toward the
        // bottom. Kotlin's scrollOffset grows when the user swipes up
        // (into history), so the delta must be negated here (round-207:
        // previously the sign was wrong — swiping down to the bottom sent
        // a negative delta that scrolled INTO history instead).
        if session.terminal().scroll_viewport(-(delta as isize)) {
            entry.last_scroll_offset = target;
            log::debug!("setScrollOffset: target={target} delta={delta}");
        } else {
            // Command channel full or VT thread gone: do NOT advance
            // last_scroll_offset, so the next call with the same target
            // retries the delta instead of silently dropping it (round-209,
            // P1-2: previously the global offset was advanced first and a
            // failed send left the viewport permanently stale).
            log::warn!(
                "setScrollOffset: scroll_viewport send failed (target={target} delta={delta}); will retry"
            );
        }
    })
}
