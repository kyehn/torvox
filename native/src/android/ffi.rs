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
//! never frozen for long  documents this in the session focus_event
//! docs too).
//!
//! EXCEPTION B: `dialogResult` and `clipboardResult` arrive from
//! AlertDialog / ActivityResult callbacks (main thread) and from the render
//! thread (empty replies on session exit). All call sites only take the
//! REQUEST_REGISTRY mutex briefly (no session locks, no blocking waits), so
//! the multi-thread entry is safe.
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
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::event::Event;
use jni::JNIEnv;
use jni::objects::JObject;
use jni::objects::JObjectArray;
use jni::objects::{JClass, JString};
use jni::sys::{
    JNI_FALSE, JNI_TRUE, jboolean, jbyte, jbyteArray, jfloat, jint, jlong, jobjectArray, jsize,
    jstring,
};

use super::text_utils::termux_env_vars;
use super::text_utils::{encode_modifiers, plain_text_url_at};
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
    /// Last scroll offset applied for THIS session: previously
    /// the delta was computed against a single global value, so switching
    /// sessions could move the old session's viewport when its render
    /// thread resumed with the other session's offset).
    last_scroll_offset: i64,
}

static SESSION_REGISTRY: LazyLock<RwLock<HashMap<u64, SessionEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Read the exit code of a registered session, if the wait thread already
/// captured one (spec d4: terminal_info exposes the session exit code).
/// Lock order: SESSION_REGISTRY → Session → exit_code (see module docs).
/// `None` for unknown sessions and for sessions still running.
#[cfg(feature = "mcp")]
pub(crate) fn session_exit_code(session_id: u64) -> Option<i32> {
    let registry = SESSION_REGISTRY.read();
    let entry = registry.get(&session_id)?;
    let session = entry.session.lock();
    session.exit_code_now()
}

/// Read the current working directory of a registered session (OSC 7
/// shell-tracked first, then the terminal's /proc-derived fallback —
/// session.rs cwd()). Registered as the MCP `terminal_info` cwd handler
/// so the MCP thread never holds the session lock.
/// Lock order: SESSION_REGISTRY → Session → cwd (see module docs).
#[cfg(feature = "mcp")]
pub(crate) fn session_cwd(session_id: u64) -> Option<String> {
    let registry = SESSION_REGISTRY.read();
    let entry = registry.get(&session_id)?;
    let session = entry.session.lock();
    Some(session.cwd())
}

/// Drains the OSC 133 last-command-output buffer of a registered session
/// (termlib getLastCommandOutput equivalent, research-supplement-4.md
/// §1.2). The buffer is cleared on read, so each query sees fresh data.
/// Lock order: SESSION_REGISTRY → Session (see module docs).
#[cfg(feature = "mcp")]
pub(crate) fn session_last_command_output(session_id: u64) -> Option<String> {
    let registry = SESSION_REGISTRY.read();
    let entry = registry.get(&session_id)?;
    let mut session = entry.session.lock();
    let output = session.take_last_command_output();
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// Test-only: register a session entry directly (host tests cannot go
/// through the JNI spawn path). Dead in non-test lib builds by design.
#[cfg(any(test, feature = "test-util"))]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn register_session_for_test(
    session_id: u64,
    session: std::sync::Arc<parking_lot::Mutex<crate::terminal::session::Session>>,
) {
    SESSION_REGISTRY.write().insert(
        session_id,
        SessionEntry {
            session,
            last_scroll_offset: 0,
        },
    );
}

/// Test-only: drop every registered session so a stale entry cannot leak
/// into a later test. Dead in non-test lib builds by design.
#[cfg(any(test, feature = "test-util"))]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn clear_registry_for_test() {
    SESSION_REGISTRY.write().clear();
}

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
    /// Bell-flash overlay phase pending for the next frame: `Some(phase)`
    /// set by `setFlashState` (any thread), consumed by `render_inner`
    /// (same deferred pattern as the background image). `0.0` turns the
    /// flash off. Kotlin drives the decay animation.
    pending_flash_phase: Option<f32>,
    /// App-level cursor blink (user setting, distinct from the VT cursor
    /// visibility the terminal itself controls). `enabled` + `speed_ms`
    /// come from `setCursorBlink`; `phase_reset_ms` is updated by
    /// `resetCursorBlink` so a user interaction restarts the blink phase
    /// with the cursor visible.
    ///
    /// These three are atomics (not plain fields under the render-state
    /// lock) on purpose: they are written by UI-thread JNI calls
    /// (`setCursorBlink`/`resetCursorBlink`) and read by the render
    /// thread, and the render thread holds the render-state lock for the
    /// whole frame (incl. `render_cell_data`, ~0.5 s/frame on software
    /// renderers). Plain fields would make each UI call block behind the
    /// render thread's lock; bursts of setting calls (e.g. the test
    /// battery) then accumulate past the 5 s ANR window. Atomics keep the
    /// UI path lock-free; Relaxed ordering is fine (single-writer burst +
    /// render-thread reader, staleness bounded by the next frame).
    cursor_blink_enabled: AtomicBool,
    cursor_blink_speed_ms: AtomicU64,
    cursor_blink_phase_reset_ms: AtomicU64,
    /// Active text selection for the next frame. Set by `setSelection`
    /// (row/col bounds in visible-grid coordinates), consumed by
    /// `render_inner`; same deferred-consume pattern as
    /// `search_highlights`.
    selection: Option<crate::render::cell_builder::SelectionRange>,
    /// Last rendered frame (cells + cursor + dims). Needed for app-level
    /// cursor blink: `render()` only draws when the terminal produced new
    /// CellData, so an idle terminal would never repaint the cursor phase.
    /// When blink is enabled and the phase flips, the cached frame is
    /// redrawn with the cursor visibility toggled.
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
    /// Search highlights at last draw (viewport row space). Dirty-band
    /// rendering must mark their rows when they change or are cleared,
    /// otherwise stale highlight pixels would persist in the accumulator.
    last_drawn_search_highlights: Vec<crate::render::cell_builder::SearchHighlight>,
    /// App-level cursor style override (user setting), applied on top of
    /// the terminal's own cursor style. `None` = follow the terminal.
    cursor_style_override: Option<crate::terminal::CursorStyle>,
    /// Bumped by `setCursorStyle`; idle repaint happens when it changes
    /// (same gate as the blink phase).
    cursor_style_version: u64,
    last_drawn_style_version: u64,
    /// App-level cursor color override (user theme), applied on top of the
    /// terminal's own cursor color. `None` = follow the terminal (white).
    /// Set by `setCursorColor` (any thread); read by the render thread
    /// when building `CellCursor` — no idle-repaint gate needed because
    /// theme application always coincides with a repaint.
    cursor_color: Option<[f32; 4]>,
    /// Pre-allocated dirty row mask, reused across frames to avoid
    /// per-frame `Vec<bool>` allocation (~100-300 bytes × 120fps).
    dirty_mask: Vec<bool>,
    /// Cached scrollback length — updated on FrameData::New, reused on
    /// Idle to avoid the synchronous `scrollback_length()` RPC that blocks
    /// the render thread for up to 50 ms when the VT thread is busy.
    cached_scrollback: u32,
    /// P2-1 content-dirty flag (dual-flag protocol — see
    /// docs/reference/dual-flag-protocol.md): raised by the JNI entry
    /// points that mutate deferred render inputs (`setSelection`,
    /// `setSearchHighlights`/`clearSearchHighlights`,
    /// `setFontSizeInPlace`, `setFlashState`, `setBackgroundImage`/
    /// `clearBackgroundImage`) and consumed by the render thread with a
    /// single `getAndSet(false)` swap in `render_inner`. Independent from
    /// the P1-1 per-session `new_output` flag (PTY ingest):
    /// selection/highlight/font-size/flash changes must repaint but never
    /// reset the viewport. It is BOTH a wake signal for the Kotlin render
    /// loop (UI callers' notifyRender() + the safety-net latch cadence)
    /// and one of the idle-gate pass conditions — NEVER an outer
    /// short-circuit (the blink phase has no JNI set-point and is decided
    /// inside the gate; an outer short-circuit would freeze cursor blink).
    dirty: AtomicBool,
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
        let font_pipeline =
            crate::render::font::FontPipeline::new(ATLAS_SIZE as i32, ATLAS_SIZE as i32, 14.0);
        *guard = Some(RenderState {
            renderer,
            font_pipeline,
            search_highlights: Vec::new(),
            selection: None,
            pending_bg_image: None,
            pending_bg_image_clear: false,
            pending_flash_phase: None,
            cursor_blink_enabled: AtomicBool::new(true),
            cursor_blink_speed_ms: AtomicU64::new(600),
            cursor_blink_phase_reset_ms: AtomicU64::new(0),
            last_frame: None,
            last_blink_phase: None,
            last_drawn_selection: None,
            last_drawn_search_highlights: Vec::new(),
            cursor_style_override: None,
            cursor_style_version: 0,
            last_drawn_style_version: 0,
            cursor_color: None,
            dirty_mask: Vec::new(),
            cached_scrollback: 0,
            dirty: AtomicBool::new(false),
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

/// Separate registry for screenshot requests which return
/// `(width, height, rgba_bytes)` instead of a plain String.
#[cfg(feature = "mcp")]
static SCREENSHOT_REQUEST_REGISTRY: LazyLock<
    Mutex<HashMap<(u64, u64), tokio::sync::oneshot::Sender<(u32, u32, Vec<u8>)>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

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

#[cfg(feature = "mcp")]
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

#[cfg(feature = "mcp")]
pub(crate) fn register_request(session_id: u64) -> (u64, tokio::sync::oneshot::Receiver<String>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    REQUEST_REGISTRY.lock().insert((session_id, request_id), tx);
    (request_id, rx)
}

/// Register a screenshot request that returns `(width, height, rgba_bytes)`.
#[cfg(feature = "mcp")]
pub(crate) fn register_screenshot_request(
    session_id: u64,
) -> (u64, tokio::sync::oneshot::Receiver<(u32, u32, Vec<u8>)>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    SCREENSHOT_REQUEST_REGISTRY
        .lock()
        .insert((session_id, request_id), tx);
    (request_id, rx)
}

/// Answer a screenshot request with RGBA pixel data.
#[cfg(feature = "mcp")]
pub(crate) fn answer_screenshot_request(
    session_id: u64,
    request_id: u64,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) {
    if let Some(tx) = SCREENSHOT_REQUEST_REGISTRY
        .lock()
        .remove(&(session_id, request_id))
    {
        let _ = tx.send((width, height, pixels));
    }
}

/// Remove a pending dialog/pick_file request without answering it. Called
/// by the MCP tools when their 300s timeout expires so a never-answered
/// request cannot leak one oneshot Sender in REQUEST_REGISTRY per call.
#[cfg(feature = "mcp")]
pub(crate) fn cancel_request(session_id: u64, request_id: u64) {
    REQUEST_REGISTRY.lock().remove(&(session_id, request_id));
    SCREENSHOT_REQUEST_REGISTRY
        .lock()
        .remove(&(session_id, request_id));
    // tell Kotlin to dismiss the still-visible dialog
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

// ── Session 生命周期 ──────────────────────────────────────────────
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
    scrollback_lines: jint,
    env_array: jobjectArray,
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
            scrollback_lines,
            env_array,
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
    scrollback_lines: jint,
    env_array: jobjectArray,
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

    // Reference (zed-android-port util/env.rs EnvOp 分层 — "user config"
    // layer above the system/base layers): parse the optional String[] of
    // "KEY=VALUE" user environment overrides passed by Kotlin (Settings >
    // Environment variables). Malformed entries and JNI read failures are
    // skipped best-effort; `parse_env_entries` keeps the first '=' split
    // so values may contain '='.
    let user_env: Vec<(String, String)> = if env_array.is_null() {
        Vec::new()
    } else {
        // SAFETY: `env_array` is a JNI method argument, guaranteed valid
        // by the JVM runtime for the duration of this call (same pattern
        // as setExtraFontPaths below).
        let array = unsafe { JObjectArray::from_raw(env_array) };
        let len = env.get_array_length(&array).unwrap_or(0);
        let mut entries = Vec::new();
        for i in 0..len {
            if let Ok(item) = env.get_object_array_element(&array, i) {
                if let Ok(s) = env.get_string(&JString::from(item)) {
                    entries.push(s.into());
                }
            }
        }
        crate::terminal::shell_env::parse_env_entries(&entries)
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
            Some(prefix.clone())
        },
        extra: {
            // without TERM, bash's readline treats the terminal
            // as "dumb" and disables input echo entirely — typed characters
            // reach the shell (commands execute) but are never shown.
            // xterm-256color is the standard value for terminal emulators
            // (Termux uses it).
            let mut extra = vec![("TERM".to_string(), "xterm-256color".to_string())];
            if !prefix.is_empty() {
                // termux-exec's execve hook only forwards
                // app-data executables to the system linker when the path is
                // under TERMUX_APP__DATA_DIR / TERMUX_APP__LEGACY_DATA_DIR.
                // The nix-on-droid bootstrap is compiled with the built-in
                // package name `com.termux.nix`, so without these variables
                // every execve of a $PREFIX binary fails with EACCES
                // (SELinux execute_no_trans) — `cat: Permission denied`.
                // Derive the Termux paths from the prefix
                // (`.../files/usr` → files dir → app data dir) instead of
                // adding a new JNI parameter.
                extra.extend(termux_env_vars(&prefix));
            }
            // User-defined overrides land last: they shadow TERM /
            // TERMUX_* defaults, and duplicate user keys collapse onto the
            // last occurrence so build_env emits each key exactly once.
            for (key, _) in &user_env {
                extra.retain(|(k, _)| k != key);
            }
            extra.extend(user_env);
            extra
        },
    };

    // Parse scrollback_lines: Kotlin Settings → JNI → native.
    // Non-positive values fall back to the default (50 000).
    let scrollback_lines = u32::try_from(scrollback_lines)
        .unwrap_or(crate::terminal::session::DEFAULT_SCROLLBACK_LINES)
        .max(1);

    let theme = crate::terminal::session::ThemeConfig {
        scrollback_lines,
        ..Default::default()
    };

    match Session::spawn_with_theme(&shell_path, rows, cols, &shell_env, None, theme) {
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
// JNI Export: getScrollbackRows
// ══════════════════════════════════════════════════════════════════════════

/// Returns the scrollback row count for a session (0 for an unknown or
/// empty session). Lightweight read following the TerminalQueryPort
/// pattern: lock the session, query Ghostty, unlock. Feeds the Kotlin
/// frame-timing memory gauge — a monotonically growing row count across
/// windows would indicate an unbounded scrollback.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getScrollbackRows(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jint {
    jni_export_guard!(&mut env, 0, {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return 0;
        };
        let session = entry.session.lock();
        session.terminal().scrollback_length() as jint
    })
}

// ── 输入、调整与键鼠编码 ────────────────────────────────────────
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
            //
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
// JNI Export: setPixelSize
// ══════════════════════════════════════════════════════════════════════════

/// Update the PTY winsize pixel fields (ws_xpixel/ws_ypixel) for the
/// specified session. Throws RuntimeException if session not found.
///
/// ghostty-android `pty_jni.c:84-87`: pixel-aware programs (`icat`,
/// fullscreen TUIs) read the pixel size from TIOCGWINSZ; a 0 pixel field
/// makes them fall back to a wrong default cell size. The Kotlin host calls
/// this alongside every grid resize with the surface's pixel dimensions.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setPixelSize(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    width_px: jint,
    height_px: jint,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        set_pixel_size_inner(&mut env, _class, session_id, width_px, height_px)
    })
}

fn set_pixel_size_inner(
    env: &mut JNIEnv,
    _class: JClass,
    session_id: jlong,
    width_px: jint,
    height_px: jint,
) {
    let id = session_id as u64;
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new(
            "java/lang/RuntimeException",
            "setPixelSize: session not found",
        );
        return;
    };
    let (Ok(width), Ok(height)) = (u16::try_from(width_px), u16::try_from(height_px)) else {
        let _ = env.throw_new(
            "java/lang/IllegalArgumentException",
            "setPixelSize: pixel dimensions must be in 0..=65535",
        );
        return;
    };
    let session = entry.session.lock();
    if let Err(error) = session.set_pixel_size(width, height) {
        if let Err(e) = env.throw_new(
            "java/lang/RuntimeException",
            format!("setPixelSize failed: {error}"),
        ) {
            log::error!("setPixelSize: throw_new failed: {e}");
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

// JNI Export: feedTerminal
// ══════════════════════════════════════════════════════════════════════════
// Feeds bytes directly to the VT parser (terminal.vt_write) instead of the
// PTY. Used by tests to inject escape sequences (OSC 8 links, DECSET) that
// must be parsed by the terminal, not echoed by the shell.

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_feedTerminal(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    data: jbyteArray,
) {
    jni_export_guard!(&mut env, (), {
        feed_terminal_inner(&mut env, session_id, data)
    })
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
fn feed_terminal_inner(env: &mut JNIEnv, session_id: jlong, data: jbyteArray) {
    let id = session_id as u64;
    // SAFETY: `data` is a JNI method argument, guaranteed valid by the JVM
    // runtime for the duration of this call.
    let byte_array = unsafe { jni::objects::JByteArray::from_raw(data) };
    let input: Vec<u8> = match env.convert_byte_array(&byte_array) {
        Ok(bytes) => bytes,
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                "feedTerminal: failed to read input bytes",
            );
            return;
        }
    };
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        let _ = env.throw_new(
            "java/lang/RuntimeException",
            "feedTerminal: session not found",
        );
        return;
    };
    let mut session = entry.session.lock();
    session.terminal_mut().vt_write(&input);
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

// JNI Export: encodeMouseEvent
// ══════════════════════════════════════════════════════════════════════════
// Encodes a mouse event into terminal escape sequences using the Ghostty
// mouse encoder (SGR/X10/UTF-8 per the application's DECSET selection).
// position is in surface pixels; cellW/cellH are the renderer's live cell
// dimensions. Returns an empty byte array when mouse reporting is off or
// encoding fails (the event is dropped — zelland renderer/mod.rs pattern).

#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_encodeMouseEvent(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    x_px: jfloat,
    y_px: jfloat,
    action: jint,
    button: jint,
    cell_w: jfloat,
    cell_h: jfloat,
) -> jbyteArray {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        encode_mouse_event_inner(
            &mut env, session_id, x_px, y_px, action, button, cell_w, cell_h,
        )
    })
}

// JNI export bodies may legitimately take many arguments: the parameter
// list is dictated by the Kotlin `NativeBridge` declaration, not by design
// choice. The argument count is fixed by the ABI and cannot be reduced
// without a coordinated Kotlin change.
#[allow(clippy::too_many_arguments)]
fn encode_mouse_event_inner(
    env: &mut JNIEnv,
    session_id: jlong,
    x_px: jfloat,
    y_px: jfloat,
    action: jint,
    button: jint,
    cell_w: jfloat,
    cell_h: jfloat,
) -> jbyteArray {
    let id = session_id as u64;
    let empty = || {
        env.byte_array_from_slice(&[])
            .map(|arr| arr.into_raw())
            .unwrap_or(std::ptr::null_mut())
    };

    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&id) else {
        return empty();
    };
    let session = entry.session.lock();
    let Some(bytes) = session.terminal().encode_mouse_event(
        (x_px, y_px),
        action as u8,
        button as u8,
        cell_w,
        cell_h,
    ) else {
        return empty();
    };
    if bytes.is_empty() {
        return empty();
    }
    env.byte_array_from_slice(&bytes)
        .map(|arr| arr.into_raw())
        .unwrap_or_else(|_| empty())
}

// ── 事件轮询 ────────────────────────────────────────────────────
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

/// Read the child's recorded lifetime (fork → waitpid, ms).
/// the wait thread writes it at exit, so this returns immediately; 0 is
/// the fallback for an exotic session without the field populated yet.
fn wait_exit_alive_ms(session: &Arc<Mutex<Session>>) -> u64 {
    let guard = session.as_ref().lock();
    let alive = guard.exit_alive_ms.lock();
    (*alive).unwrap_or(0)
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
    // Bell/clipboard/notification/exit polling is identical for the active
    // session and every background session; single shared implementation.
    let mut collect_session_events =
        |session_id: u64,
         session: &mut Session,
         handle: &Arc<Mutex<Session>>,
         events: &mut Vec<Event>,
         exits: &mut Vec<(u64, Arc<Mutex<Session>>)>| {
            if session.poll_bel() {
                events.push(Event::Bell { session_id });
            }
            if let Some(text) = session.poll_clipboard() {
                events.push(Event::Clipboard { session_id, text });
            }
            if let Some((title, body)) = session.poll_notification() {
                events.push(Event::Notification {
                    session_id,
                    title,
                    body,
                });
            }
            // Only the first poll after the process exits reports it
            // (mark_exit_reported); the sweep branch uses the same dedup so a
            // slow consumer can never see duplicate Exit events for the same
            // session. The exit code is read after both locks are released
            // (see pending_exits below).
            if session.is_exited() && session.mark_exit_reported() {
                exits.push((session_id, handle.clone()));
            }
        };
    #[cfg(feature = "mcp")]
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
                // Check OSC 52 clipboard read request (`ESC ] 52 ; c ; ?`).
                // Collect the selection here (inside the session lock); the
                // one-shot slot + responder thread are set up after the
                // registry/session locks are released (see below) so the
                // lock order stays single-directional.
                #[cfg(feature = "mcp")]
                if let Some(selection) = session.poll_clipboard_read() {
                    pending_clipboard_reads.push((active_id, selection));
                }
                // Check progress (OSC 9;4 ConEmu)
                if let Some((state, value)) = session.poll_progress() {
                    pending_events.push(Event::Progress {
                        session_id: active_id,
                        state,
                        value,
                    });
                }
                collect_session_events(
                    active_id,
                    &mut session,
                    &entry.session,
                    &mut pending_events,
                    &mut pending_exits,
                );
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
            collect_session_events(
                *id,
                &mut session,
                &entry.session,
                &mut pending_events,
                &mut pending_exits,
            );
        }
        // Registry read lock is released here.
    }
    // Process pending OSC 52 clipboard read requests (outside the
    // registry/session locks): register a one-shot answer slot, push the
    // event, and spawn a short-lived responder thread that writes the
    // host-app answer back to the PTY. The VT thread must never block on
    // the host app, and clipboardResult may arrive on any thread.
    #[cfg(feature = "mcp")]
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
            alive_ms: wait_exit_alive_ms(&session),
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

// ── P1-1 new_output 旁路标志 ───────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI Export: consumeNewOutput
// ══════════════════════════════════════════════════════════════════════════

/// Take and clear the per-session `new_output` flag (P1-1 scroll-reset
/// signal, dual-flag protocol — see docs/reference/dual-flag-protocol.md).
///
/// The flag is raised by the PTY ingest path (`Session::process_output` /
/// `poll_pty_output` → `OutputProcessor::process`) and read-and-cleared here
/// by the render thread once per frame, as a BYPASS read alongside the
/// `pollAll()` loop — deliberately NOT a queued `Event` variant: under
/// sustained output (tail -f) an event variant would compete for the
/// `MAX_EVENTS_PER_POLL` budget and starve bell/dialog/exit events.
/// Independent from the P2-1 `dirty` flag (selection/highlight/font-size
/// changes must repaint but never reset the viewport).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_consumeNewOutput(
    _env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    // No panic risk inside (no JNI calls, no unwraps); still keep the guard
    // pattern consistent with neighbouring exports.
    let registry = rlock_session_registry();
    let Some(entry) = registry.get(&(session_id as u64)) else {
        return JNI_FALSE;
    };
    let session = entry.session.lock();
    if session.take_new_output() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

// ── 日志与渲染生命周期 ──────────────────────────────────────────
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
// JNI Export: attachWindow
// ══════════════════════════════════════════════════════════════════════════

/// Attach an Android Surface — Android only.
///
/// Called from Bridge.kt on surface attach (ADR-0007):
/// TerminalRuntime hands the Android Surface over the JNI boundary and
/// the render thread consumes it via the native window. The surface is
/// detached again by `detachWindow`.
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

/// Compute the cursor blink phase and apply it to the cursor visibility.
/// Returns the blink phase (0 = visible, 1 = hidden).
fn compute_cursor_blink(render_state: &RenderState, cursor: &mut crate::render::CellCursor) -> u64 {
    let cursor_blink_enabled = render_state.cursor_blink_enabled.load(Ordering::Relaxed);
    if !cursor_blink_enabled {
        return 0;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let speed = render_state
        .cursor_blink_speed_ms
        .load(Ordering::Relaxed)
        .max(50);
    let phase_reset_ms = render_state
        .cursor_blink_phase_reset_ms
        .load(Ordering::Relaxed);
    let phase = now_ms.saturating_sub(phase_reset_ms);
    let blink_phase = (phase / speed) % 2;
    if blink_phase == 1 {
        cursor.visible = false;
    }
    blink_phase
}

/// Build a cursor from a `CursorInfo` snapshot, applying the style override
/// and cursor color from render state.
fn build_cursor(
    render_state: &RenderState,
    cursor_info: &crate::terminal::ghostty_terminal::CursorInfo,
) -> crate::render::CellCursor {
    crate::render::CellCursor {
        row: cursor_info.row,
        col: cursor_info.col,
        visible: cursor_info.visible,
        style: render_state
            .cursor_style_override
            .unwrap_or(cursor_info.style),
        color: render_state.cursor_color,
    }
}

/// Compute the selection range adjusted for scrollback offset.
fn compute_render_selection(
    selection: Option<crate::render::cell_builder::SelectionRange>,
    scrollback: u32,
) -> Option<crate::render::cell_builder::SelectionRange> {
    selection.map(|mut sel| {
        sel.start_row -= scrollback as i32;
        sel.end_row -= scrollback as i32;
        sel
    })
}

/// Mark overlay rows (current selection, previous selection, search
/// highlights) as dirty in the mask. These are per-row visual overlays
/// whose addition/removal changes pixels without touching cell content.
fn mark_overlay_dirty_rows(
    dirty_mask: &mut Vec<bool>,
    rows_usize: usize,
    render_selection: &Option<crate::render::cell_builder::SelectionRange>,
    previous_selection: Option<crate::render::cell_builder::SelectionRange>,
    highlight_rows: &[i32],
) {
    if let Some(sel) = render_selection {
        let start = sel.start_row.max(0) as usize;
        let end = (sel.end_row + 1).max(0) as usize;
        for slot in dirty_mask.iter_mut().take(end.min(rows_usize)).skip(start) {
            *slot = true;
        }
    }
    // Previous selection: clearing/shrinking a selection must
    // repaint the rows it used to cover.
    if let Some(old_sel) = previous_selection {
        let start = old_sel.start_row.max(0) as usize;
        let end = (old_sel.end_row + 1).max(0) as usize;
        for slot in dirty_mask.iter_mut().take(end.min(rows_usize)).skip(start) {
            *slot = true;
        }
    }
    // Search highlight rows, current AND last drawn: highlights are
    // per-row overlays; adding/removing/moving them changes pixels
    // without changing cell content.
    for r in highlight_rows {
        if *r >= 0 && (*r as usize) < rows_usize {
            dirty_mask[*r as usize] = true;
        }
    }
}

/// Collect highlight row numbers from current and last-drawn highlights.
fn collect_highlight_rows(render_state: &RenderState) -> Vec<i32> {
    render_state
        .search_highlights
        .iter()
        .chain(render_state.last_drawn_search_highlights.iter())
        .map(|hl| hl.row)
        .collect()
}

fn render_inner(session_id: u64) -> jint {
    // ── Phase 1: Pre-render housekeeping (single RENDER_STATE lock) ───────
    // Consume pending bg image / flash phase, check surface readiness,
    // upload atlas dirty rect, and lazily create the pipeline — all under
    // ONE lock acquisition instead of the previous three. This reduces
    // RENDER_STATE lock round-trips per frame from 3 to 2 (the second
    // acquisition is in Phase 3 for the actual render).
    {
        let mut state = render_state_mut();
        let Some(render_state) = state.as_mut() else {
            return 0;
        };
        // Surface must be attached before any rendering work.
        if render_state.renderer.surface.is_none() {
            return 0;
        }
        // Consume pending background-image / flash-phase changes.
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
        if let Some(phase) = render_state.pending_flash_phase.take() {
            render_state.renderer.set_flash_phase(phase);
            log::trace!("render_inner: flash phase={phase}");
        }
        // Lazy one-time pipeline creation.
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
        // Upload glyph atlas dirty regions (even on idle frames: newly
        // rasterized glyphs must reach the GPU texture before next draw).
        if let Some(rect) = render_state.font_pipeline.take_dirty_rect() {
            let (aw, ah) = render_state.font_pipeline.atlas_dimensions();
            render_state.renderer.upload_atlas(
                render_state.font_pipeline.atlas_bitmap(),
                aw,
                ah,
                Some(rect),
            );
        }
    } // ── render_state lock released ──────────────────────────────────────

    // ── Phase 2: Collect cell data (session lock only) ────────────────────
    // On new data: receive owned CellData from the channel (zero-copy move).
    // On idle: record the fact — Phase 3 will reference last_frame directly,
    // avoiding the 32KB clone that previously happened here.
    enum FrameData {
        New {
            cells: Vec<crate::terminal::ghostty_terminal::CellData>,
            cursor_info: crate::terminal::ghostty_terminal::CursorInfo,
            rows: u32,
            cols: u32,
        },
        Idle {},
    }
    let frame_data = {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&session_id) else {
            log::warn!("render: unknown session {session_id}");
            return -1;
        };
        let session = entry.session.lock();
        // CRITICAL: Do NOT call session.terminal().scrollback_length()
        // here — it's a synchronous RPC to the VT thread that blocks the
        // render thread for up to 50 ms when the VT thread is busy.
        // Scrollback length is piggy-backed on CursorInfo via the cell
        // data channel (see push_cell_data → CursorInfo.scrollback_length).
        match session.terminal().receive_cell_data() {
            Some((cells, cursor_info)) => {
                let (rows, cols) = session.grid_size();
                FrameData::New {
                    cells,
                    cursor_info,
                    rows,
                    cols,
                }
            }
            None => FrameData::Idle {},
        }
    }; // ── session lock released ──────────────────────────────────────────

    // ── Phase 3: Render (render_state lock) ───────────────────────────────
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::error!("render: render state missing");
        return -1;
    };
    if render_state.renderer.surface.is_none() {
        return 0;
    }
    // P2-1: single-point read-clear of the content-dirty flag.
    let content_dirty = render_state.dirty.swap(false, Ordering::AcqRel);

    // Branch on new data vs idle — the idle path uses a reference to
    // last_frame instead of cloning, eliminating ~32KB per idle frame.
    match frame_data {
        FrameData::New {
            cells,
            cursor_info,
            rows,
            cols,
        } => {
            // Use scrollback_length from the cell data channel — no
            // synchronous RPC needed. The VT thread piggybacks it on
            // every CursorInfo push.
            let scrollback = cursor_info.scrollback_length;
            render_state.cached_scrollback = scrollback;
            let mut cursor = build_cursor(render_state, &cursor_info);
            let blink_phase = compute_cursor_blink(render_state, &mut cursor);
            let render_selection = compute_render_selection(render_state.selection, scrollback);
            // Build dirty mask using pre-allocated buffer.
            let rows_usize = rows as usize;
            // Immutable snapshots first: the mask is a &mut borrow of
            // render_state.dirty_mask and cannot coexist with reads of the
            // other render_state fields below.
            let scroll_up_rows: Option<u32> = None;
            let previous_cursor_row = render_state
                .last_frame
                .as_ref()
                .map(|(_, old_cursor_info, _, _)| old_cursor_info.row);
            let previous_selection_rows =
                compute_render_selection(render_state.last_drawn_selection, scrollback);
            let highlight_rows = collect_highlight_rows(render_state);
            let dirty_mask = &mut render_state.dirty_mask;
            dirty_mask.clear();
            dirty_mask.resize(rows_usize, false);
            match &render_state.last_frame {
                Some((old_cells, _, old_rows, old_cols))
                    if *old_rows == rows && *old_cols == cols =>
                {
                    if let Some(s) = scroll_up_rows {
                        // Pure scroll: only the freshly revealed bottom rows
                        // carry new content; everything else arrives via the
                        // accumulator blit.
                        let start = rows_usize - s as usize;
                        dirty_mask[start..rows_usize].fill(true);
                    } else {
                        crate::render::cell_builder::diff_dirty_rows_into(
                            old_cells,
                            &cells,
                            rows,
                            &mut dirty_mask[..rows_usize],
                        );
                    }
                }
                _ => {
                    dirty_mask[..rows_usize].fill(true);
                }
            }
            // Cursor row(s): mark BOTH the current and previous cursor rows
            // regardless of visibility — the cursor is an overlay on the
            // instances (blink/style/position all change pixels without
            // changing cell content), and dirty-band rendering needs every
            // affected row repainted or stale cursor pixels persist.
            if (cursor.row as usize) < rows_usize {
                dirty_mask[cursor.row as usize] = true;
            }
            if let Some(prev_row) = previous_cursor_row
                && (prev_row as usize) < rows_usize
            {
                dirty_mask[prev_row as usize] = true;
            }
            mark_overlay_dirty_rows(
                dirty_mask,
                rows_usize,
                &render_selection,
                previous_selection_rows,
                &highlight_rows,
            );
            let result = render_state.renderer.render_cell_data(
                &cells,
                rows,
                cols,
                cursor,
                &mut render_state.font_pipeline,
                ATLAS_SIZE as f32,
                ATLAS_SIZE as f32,
                render_selection,
                &render_state.search_highlights,
                Some(&render_state.dirty_mask),
                scroll_up_rows,
            );
            if result.is_ok() {
                render_state.last_frame = Some((cells, cursor_info, rows, cols));
                render_state.last_blink_phase = Some(blink_phase);
                render_state.last_drawn_selection = render_state.selection;
                render_state.last_drawn_search_highlights = render_state.search_highlights.clone();
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
        FrameData::Idle {} => {
            // Idle path: use reference to last_frame — zero clone.
            let Some((ref cached_cells, cached_cursor, cached_rows, cached_cols)) =
                render_state.last_frame
            else {
                return 0;
            };
            let mut cursor = build_cursor(render_state, &cached_cursor);
            let blink_phase = compute_cursor_blink(render_state, &mut cursor);
            // Idle repaint gate (P2-1): only repaint when something actually
            // changed — blink phase flip, style change, selection change,
            // search highlights active, or content-dirty flag raised.
            let cursor_blink_enabled = render_state.cursor_blink_enabled.load(Ordering::Relaxed);
            let phase_changed = render_state.last_blink_phase != Some(blink_phase);
            let style_changed =
                render_state.last_drawn_style_version != render_state.cursor_style_version;
            let selection_changed = render_state.selection != render_state.last_drawn_selection;
            let highlights_changed =
                render_state.search_highlights != render_state.last_drawn_search_highlights;
            let bg_image_pending = render_state.pending_bg_image.is_some();
            let flash_phase_pending = render_state.pending_flash_phase.is_some();
            let needs_repaint = (!cursor_blink_enabled
                && (style_changed
                    || selection_changed
                    || highlights_changed
                    || bg_image_pending
                    || flash_phase_pending
                    || content_dirty))
                || (cursor_blink_enabled
                    && (phase_changed
                        || style_changed
                        || selection_changed
                        || highlights_changed
                        || bg_image_pending
                        || flash_phase_pending
                        || content_dirty));
            if !needs_repaint {
                return 0;
            }
            let scrollback = render_state.cached_scrollback;
            let render_selection = compute_render_selection(render_state.selection, scrollback);
            let rows_usize = cached_rows as usize;
            let previous_selection_rows =
                compute_render_selection(render_state.last_drawn_selection, scrollback);
            let highlight_rows = collect_highlight_rows(render_state);
            let dirty_mask = &mut render_state.dirty_mask;
            dirty_mask.clear();
            dirty_mask.resize(rows_usize, false);
            // Cursor row regardless of visibility: blink/style toggles
            // pixels without touching cell content.
            if (cursor.row as usize) < rows_usize {
                dirty_mask[cursor.row as usize] = true;
            }
            mark_overlay_dirty_rows(
                dirty_mask,
                rows_usize,
                &render_selection,
                previous_selection_rows,
                &highlight_rows,
            );
            let result = render_state.renderer.render_cell_data(
                cached_cells,
                cached_rows,
                cached_cols,
                cursor,
                &mut render_state.font_pipeline,
                ATLAS_SIZE as f32,
                ATLAS_SIZE as f32,
                render_selection,
                &render_state.search_highlights,
                Some(&render_state.dirty_mask),
                None,
            );
            if result.is_ok() {
                render_state.last_blink_phase = Some(blink_phase);
                render_state.last_drawn_selection = render_state.selection;
                render_state.last_drawn_search_highlights = render_state.search_highlights.clone();
                render_state.last_drawn_style_version = render_state.cursor_style_version;
                // NOTE: last_frame NOT updated on idle — cells unchanged.
            }
            match result {
                Ok(()) => 1,
                Err(error) => {
                    log::error!("render: frame failed: {error}");
                    -1
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: renderWithNewOutput
// ══════════════════════════════════════════════════════════════════════════

/// Combined render + consumeNewOutput: renders one frame AND reads the
/// per-session new_output flag in a single JNI crossing, saving ~0.1-0.3ms
/// per frame vs two separate calls (render + consumeNewOutput).
///
/// Returns a packed `jlong`:
///   - bits 0..31  = render count (same semantics as `render()`)
///   - bit  32     = new_output flag (1 = PTY output was ingested, 0 = idle)
///   - bits 33..63 = 0 (reserved)
///
/// On error the render count is negative and new_output is 0.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_renderWithNewOutput<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    _width: jint,
    _height: jint,
) -> jlong {
    let count = jni_export_guard!(&mut env, -1i32, { render_inner(session_id as u64) });
    let mut new_output: i32 = 0;
    if count > 0 {
        // Consume the new_output flag inline (same logic as
        // consumeNewOutput but without a second JNI crossing).
        let registry = rlock_session_registry();
        if let Some(entry) = registry.get(&(session_id as u64)) {
            let session = entry.session.lock();
            if session.take_new_output() {
                new_output = 1;
            }
        }
    }
    ((new_output as i64) << 32) | (count as i64 & 0xFFFF_FFFF)
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

// ── MCP 桥接与异步结果 ──────────────────────────────────────────
// ══════════════════════════════════════════════════════════════════════════
// JNI Export: setMcpEnabled
// ══════════════════════════════════════════════════════════════════════════

/// Override the MCP Unix socket path. Kotlin derives it
/// from `context.filesDir` so it follows the real `applicationId` instead
/// of the hardcoded `/data/data/com.termux` default.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setMcpSocketPath(
    mut env: JNIEnv,
    _class: JClass,
    _path: JString,
) {
    jni_export_guard!(&mut env, (), {
        #[cfg(feature = "mcp")]
        {
            if let Ok(s) = env.get_string(&_path) {
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
                // registry read in this file.
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
            state.set_session_cwd_handler(|session_id: u64| -> Option<String> {
                // Spec: terminal_info reports the session cwd. Resolved on
                // the MCP worker thread WITHOUT holding the session lock:
                // the JNI bridge reads OSC 7 tracked cwd (fallback /proc)
                // via the registry, mirroring session_exit_code.
                session_cwd(session_id)
            });
            state.set_session_exit_code_handler(session_exit_code);
            state.set_session_last_command_output_handler(session_last_command_output);
            state.set_cancel_request_handler(|session_id: u64, request_id: u64| {
                cancel_request(session_id, request_id);
            });
            state.set_run_command_handler(
                |session_id: u64,
                 command: String|
                 -> (u64, tokio::sync::oneshot::Receiver<String>) {
                    let (request_id, rx) = register_request(session_id);
                    push_event(Event::RunCommand {
                        session_id,
                        request_id,
                        command,
                    });
                    (request_id, rx)
                },
            );
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
            state.set_screenshot_handler(
                |session_id: u64| -> (u64, tokio::sync::oneshot::Receiver<(u32, u32, Vec<u8>)>) {
                    let (request_id, rx) = register_screenshot_request(session_id);
                    push_event(Event::Screenshot {
                        session_id,
                        request_id,
                    });
                    (request_id, rx)
                },
            );
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
// JNI Export: runCommandResult — Kotlin responds to an MCP run_command
// ══════════════════════════════════════════════════════════════════════════

/// Reply to an MCP `run_command` request. The `result` is the JSON payload
/// `{"exit_code":N,"err_code":M,"stdout":...,"stderr":...}` produced by the
/// Kotlin command runner (err_code: 0=ok, 1=timeout, 2=exception). Same
/// routing as `dialogResult` (shared registry).
#[unsafe(no_mangle)]
#[cfg(feature = "mcp")]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_runCommandResult<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    result: JString<'local>,
) {
    // A panic escaping this JNI export would abort the whole process.
    // Convert it into a Java exception instead.
    jni_export_guard!(&mut env, (), {
        run_command_result_inner(&mut env, _class, session_id, request_id, result)
    })
}

#[cfg(feature = "mcp")]
fn run_command_result_inner<'local>(
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

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: screenshotResult — Kotlin responds to an MCP screenshot
// ══════════════════════════════════════════════════════════════════════════

/// Reply to an MCP `screenshot` request. Kotlin captures RGBA pixels via
/// `captureFrame()` and sends them back through this export.
#[unsafe(no_mangle)]
#[cfg(feature = "mcp")]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_screenshotResult<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    width: jint,
    height: jint,
    pixels: jbyteArray,
) {
    jni_export_guard!(&mut env, (), {
        screenshot_result_inner(
            &mut env, _class, session_id, request_id, width, height, pixels,
        )
    })
}

#[cfg(feature = "mcp")]
fn screenshot_result_inner<'local>(
    env: &mut JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    request_id: jlong,
    width: jint,
    height: jint,
    pixels: jbyteArray,
) {
    let session_id = session_id as u64;
    let request_id = request_id as u64;
    let w = width as u32;
    let h = height as u32;
    let pixel_data: Vec<u8> = {
        // SAFETY: `pixels` is a JNI method argument, guaranteed valid by the
        // JVM runtime for the duration of this call. `from_raw` wraps the
        // pointer without taking ownership; the local ref is released by
        // the JVM when this native method returns.
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(pixels) };
        env.convert_byte_array(&byte_array).unwrap_or_default()
    };
    answer_screenshot_request(session_id, request_id, w, h, pixel_data);
}

// ══════════════════════════════════════════════════════════════════════════
// JNI Export: captureFrame — render thread captures current frame pixels
// ══════════════════════════════════════════════════════════════════════════

/// Capture the current terminal frame as RGBA pixels via GPU readback.
/// Called from the render thread (which owns the wgpu context) when an
/// MCP `screenshot` event is dispatched.
///
/// Returns a `byte[]` with the first 8 bytes being width (u32 LE) +
/// height (u32 LE), followed by `width * height * 4` RGBA bytes.
/// Returns null if no frame is available.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_captureFrame<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jbyteArray {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        capture_frame_inner(&mut env, _class, session_id as u64)
    })
}

fn capture_frame_inner<'local>(
    env: &mut JNIEnv<'local>,
    _class: JClass<'local>,
    _session_id: u64,
) -> jbyteArray {
    let mut state = render_state_mut();
    let Some(render_state) = state.as_mut() else {
        log::warn!("captureFrame: render state not initialized");
        return std::ptr::null_mut();
    };

    // Check that the renderer has a surface config (dimensions).
    let (w, h) = render_state
        .renderer
        .surface_config
        .as_ref()
        .map_or((0, 0), |c| (c.width, c.height));
    if w == 0 || h == 0 {
        log::warn!("captureFrame: no surface config (w={w}, h={h})");
        return std::ptr::null_mut();
    }

    // Clone instances from the last render (render_to_buffer needs &mut self,
    // so we can't borrow cpu_instances while calling it).
    let instances = render_state.renderer.cpu_instances.clone();
    if instances.is_empty() {
        log::warn!("captureFrame: no instances to render (frame not yet rendered)");
        return std::ptr::null_mut();
    }

    // GPU readback: render to offscreen buffer.
    match render_state.renderer.render_to_buffer(&instances, &[]) {
        Ok(pixels) => {
            let pixel_len = pixels.len();
            let expected = (w * h * 4) as usize;
            if pixel_len != expected {
                log::warn!(
                    "captureFrame: pixel count mismatch (got {pixel_len}, expected {expected})"
                );
                return std::ptr::null_mut();
            }
            // Build output: [width:u32 LE][height:u32 LE][RGBA pixels]
            let mut output = Vec::with_capacity(8 + pixel_len);
            output.extend_from_slice(&w.to_le_bytes());
            output.extend_from_slice(&h.to_le_bytes());
            output.extend_from_slice(&pixels);
            match env.byte_array_from_slice(&output) {
                Ok(arr) => arr.into_raw(),
                Err(e) => {
                    log::warn!("captureFrame: byte_array_from_slice failed: {e}");
                    std::ptr::null_mut()
                }
            }
        }
        Err(e) => {
            log::warn!("captureFrame: render_to_buffer failed: {e}");
            std::ptr::null_mut()
        }
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
///
/// 2048² (was 1024²): on a real device at ~3x display density a 14sp glyph
/// rasterizes to ~40px, so the old atlas held only ~700 glyphs — a single
/// scrolling log screen with a few hundred distinct characters thrashed the
/// LRU, evicting and re-rasterizing the same glyphs frame after frame
/// (CPU-bound). 2048² quadruples the capacity (~2800 glyphs) with a bounded
/// memory cost (16MB device + 16MB staging bitmap).
const ATLAS_SIZE: u32 = 2048;

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

// ── 文本与滚动查询 ──────────────────────────────────────────────
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

/// Returns the terminal cursor's viewport position packed as `(y << 32) | x`,
/// or `-1` when the cursor is hidden/off-viewport.
///
///  observability (spec cursor-rendering): reads through
/// `build_cell_data` — the EXACT source the render thread consumes — so the
/// instrumentation layer sees the same coordinates the GPU draws. Values are
/// 0-based viewport rows/cols.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getCursorViewportPacked(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jlong {
    jni_export_guard!(&mut env, -1, {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "getCursorViewportPacked: session not found",
            );
            return -1;
        };
        let session = entry.session.lock();
        let terminal = session.terminal();
        let Some((row, col)) = terminal.render_cursor() else {
            return -1;
        };
        ((row as jlong) << 32) | (col as jlong)
    })
}

/// Returns a single scrollback row's trimmed text, or null for an empty row.
///
/// **Lazy-access semantics** — Ghostty only retains lines that have been
/// explicitly read; most indices return null.  Do NOT use for full-text
/// iteration — use `dump_grid` (via `getTerminalText`) instead.
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

/// Extract selection text with Ghostty's native formatter (wrap-aware,
/// wide-char safe — termux TerminalBuffer.getSelectedText semantics).
/// startRow/startCol/endRow/endCol are grid rows/cols (absolute: row 0 is
/// the top of scrollback, matching scrollbackLine). Returns "" on error.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_selectionText<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    start_row: jint,
    start_col: jint,
    end_row: jint,
    end_col: jint,
    rectangle: jboolean,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return std::ptr::null_mut();
        };
        let session = entry.session.lock();
        let text = session.terminal().selection_text(
            (start_row.max(0) as u32, start_col.max(0) as u32),
            (end_row.max(0) as u32, end_col.max(0) as u32),
            rectangle == JNI_TRUE,
        );
        drop(session);
        drop(registry);
        match env.new_string(&text) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Query the OSC 8 hyperlink URI at a grid cell (row 0 = top of
/// scrollback, matching scrollbackLine). Returns null when no link.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_hyperlinkAt<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
    row: jint,
    col: jint,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return std::ptr::null_mut();
        };
        let session = entry.session.lock();
        let url = session
            .terminal()
            .hyperlink_at(row.max(0) as u32, col.max(0) as u32);
        if url.is_none() {
            // Plain-text URL fallback (zed-port URL_REGEX pattern): OSC 8
            // covers hyperlinks only; a tap on a bare URL in terminal output
            // must still open the browser. Reuse the DumpGrid text path
            // (same data as getTerminalText) and scan the tapped row's
            // column span.
            if let Some(fallback) =
                plain_text_url_at(&session, row.max(0) as u32, col.max(0) as u32)
            {
                drop(session);
                drop(registry);
                return match env.new_string(&fallback) {
                    Ok(s) => s.into_raw(),
                    Err(_) => std::ptr::null_mut(),
                };
            }
        }
        drop(session);
        drop(registry);
        match url {
            Some(url) => match env.new_string(&url) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            None => std::ptr::null_mut(),
        }
    })
}

// ── 搜索与选择 ──────────────────────────────────────────────────
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
                // `col` is a CHARACTER column, but the raw
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

// ── 字体与主题 ──────────────────────────────────────────────────
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
        let info = render_state.font_pipeline.font_info();
        drop(state);
        match serde_json::to_string(&info) {
            Ok(json) => match env.new_string(&json) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
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
            // P2-1 dirty: highlight clearing must reach the screen even on
            // an idle terminal (otherwise stale highlights linger until the
            // next PTY output — #5 regression class).
            render_state.dirty.store(true, Ordering::Relaxed);
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
        let payload = &bytes[4..];
        let mut offset = 0;
        for _ in 0..count {
            if offset + 16 > payload.len() {
                break;
            }
            let row = i32::from_le_bytes(
                payload[offset..offset + 4]
                    .try_into()
                    .expect("4-byte slice"),
            );
            let start_col = i32::from_le_bytes(
                payload[offset + 4..offset + 8]
                    .try_into()
                    .expect("4-byte slice"),
            );
            let end_col_exclusive = i32::from_le_bytes(
                payload[offset + 8..offset + 12]
                    .try_into()
                    .expect("4-byte slice"),
            );
            let color = [
                payload[offset + 12],
                payload[offset + 13],
                payload[offset + 14],
                payload[offset + 15],
            ];
            highlights.push(crate::render::cell_builder::SearchHighlight {
                row,
                start_col,
                end_col_exclusive,
                color,
            });
            offset += 16;
        }
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.search_highlights = highlights;
            // P2-1 dirty: new highlights must paint on the next frame even
            // with blink off and no PTY traffic (#5 idle-screen regression).
            render_state.dirty.store(true, Ordering::Relaxed);
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
    _selection_bg_argb: jint,
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
        // `selection_bg_argb` is accepted for Kotlin contract stability but
        // no longer stored: classic inverse video (fg<->bg swap) renders
        // the selection highlight (see rejected-technologies §1.7 #33).
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
            // P2-1 dirty: selection changes must repaint even on an idle
            // terminal (deferred field otherwise waits for PTY output).
            render_state.dirty.store(true, Ordering::Relaxed);
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
        // (emulator-verified: checkerboard probe proved the bg
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
                // P2-1 dirty: zero-sized image treated as clear — must still
                // reach the screen on an idle terminal.
                render_state.dirty.store(true, Ordering::Relaxed);
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
            // P2-1 dirty: background-image changes must repaint even on an
            // idle terminal (pending_bg_image is consumed at the top of
            // render_inner BEFORE the idle gate — without this raise the
            // upload happens but the gate can skip presenting it).
            render_state.dirty.store(true, Ordering::Relaxed);
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
            // P2-1 dirty: see setBackgroundImage.
            render_state.dirty.store(true, Ordering::Relaxed);
        }
        log::info!("clearBackgroundImage: queued for render thread");
    })
}

/// Set the bell-flash overlay phase for the next frame. `phase` is the
/// decaying flash strength in 0..=1 (0 = no flash) — the Kotlin side
/// animates it down after a bell; the native side just composites a white
/// full-screen quad whose alpha scales with the phase. Deferred to the
/// render thread like `setBackgroundImage`, so the value is consumed by
/// the next `render_inner`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFlashState(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    phase: jfloat,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.pending_flash_phase = Some(phase.max(0.0));
            // P2-1 dirty: bell-flash phase changes must repaint even on an
            // idle terminal — without this raise SCREEN_FLASH bells go
            // silently invisible once vsync alignment removed the old
            // full-rate poll cadence that used to flush deferred fields.
            render_state.dirty.store(true, Ordering::Relaxed);
        }
        log::info!("setFlashState: phase={phase} queued for render thread");
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
            let enabled = enabled != 0;
            let speed = (speed_ms as u64).clamp(50, 2000);
            render_state
                .cursor_blink_enabled
                .store(enabled, Ordering::Relaxed);
            render_state
                .cursor_blink_speed_ms
                .store(speed, Ordering::Relaxed);
            log::info!("setCursorBlink: enabled={enabled} speed={speed}ms");
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
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            render_state
                .cursor_blink_phase_reset_ms
                .store(now_ms, Ordering::Relaxed);
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

/// App-level cursor color override ("r" | "g" | "b" in 0..1 linear RGB).
/// Applied on top of the theme's cursor color so the user theme's cursor
/// reaches the renderer (the 54-byte `setTheme` payload has no slot for
/// it). `None` clears the override (follow the terminal).
#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)] // JNI signatures contain raw pointers by design
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setCursorColor(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    r: f32,
    g: f32,
    b: f32,
) {
    jni_export_guard!(&mut env, (), {
        let mut state = render_state_mut();
        if let Some(render_state) = state.as_mut() {
            render_state.cursor_color =
                Some([r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), 1.0]);
            log::info!("setCursorColor: ({r}, {g}, {b})");
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

/// Set the independent family for one style slot — 0=bold, 1=italic,
/// 2=bold-italic (ghostty-android TerminalFontStore 4-slot design,
/// research-ghostty-android-extra.md:80). Empty family clears the slot
/// (falls back to same-family lookup + synthesis).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_setFontFamilyForStyle(
    mut env: JNIEnv,
    _class: JClass,
    _session_id: jlong,
    family: JString,
    slot: jint,
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
        let found = render_state
            .font_pipeline
            .set_font_family_for_style(&family_str, slot.max(0) as u8);
        log::info!("setFontFamilyForStyle(slot={slot}): {family_str} found={found}");
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
            // P2-1 dirty: font-size changes must repaint even on an idle
            // terminal (glyph metrics changed → cached frame is stale).
            render_state.dirty.store(true, Ordering::Relaxed);
            log::info!(
                "setFontSizeInPlace: {} -> cell {cw:.1}x{ch:.1}",
                size_tenths
            );
        }
    })
}

/// Set the font rasterization scale (device pixel density). Glyph bitmaps
/// are rasterized at font_size * raster_scale so text stays crisp on
/// high-density screens.
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
            // BOTH pipelines must agree on raster_scale — the
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

// ── 网格尺寸查询 ────────────────────────────────────────────────
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
/// is unknown. Backs `Bridge.getGridRowsColsPacked`: the
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

// ── 滚动、模式与状态查询 ────────────────────────────────────────
/// Set the viewport scroll offset (rows into scrollback; 0 = active
/// screen). The difference from the previous offset is applied on the VT
/// thread via `scroll_viewport(Delta)`, so the next CellData push carries
/// the scrolled view: previously a Kotlin-side no-op).
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
        // Per-session delta: the previous code computed the
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
        // (into history), so the delta must be negated here:
        // previously the sign was wrong — swiping down to the bottom sent
        // a negative delta that scrolled INTO history instead).
        if session.terminal().scroll_viewport(-(delta as isize)) {
            entry.last_scroll_offset = target;
            log::debug!("setScrollOffset: target={target} delta={delta}");
        } else {
            // Command channel full or VT thread gone: do NOT advance
            // last_scroll_offset, so the next call with the same target
            // retries the delta instead of silently dropping it,
            // previously the global offset was advanced first and a
            // failed send left the viewport permanently stale).
            log::warn!(
                "setScrollOffset: scroll_viewport send failed (target={target} delta={delta}); will retry"
            );
        }
    })
}

/// Whether the remote is currently on the alternate screen buffer
/// (vim/less/htop). Lock-free read of the mirror maintained by the VT thread
/// on every `Query::AltScreen` query — safe to call from the Android input
/// path on every touch-scroll event without blocking the UI thread.
///
/// Backs `Bridge.isAltScreenActive` so touch-scroll gestures on the alternate
/// screen can be forwarded to the remote as mouse-wheel escapes instead of
/// scrolling local scrollback (Haven research: altScreen wheel consumption).
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getAltScreenState(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
) -> jboolean {
    jni_export_guard!(&mut env, 0, {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(session_id as u64)) else {
            return 0;
        };
        let session = entry.session.lock();
        if session.terminal().alt_screen_active_atomic() {
            1
        } else {
            0
        }
    })
}

/// Queries a terminal mode (ghostty `mode_get`). `kind` selects the mode
/// namespace: 0 = DEC private modes, non-zero = ANSI modes. Backs the
/// DECCKM (application cursor keys, DEC private mode 1) lookup the Kotlin
/// key encoder needs to switch arrow keys between SS3 (`ESC OA`) and CSI
/// (`ESC [ A`) — research-haven.md:141, research-zed-port.md:252.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getMode(
    mut env: JNIEnv,
    _class: JClass,
    session_id: jlong,
    mode_num: jint,
    kind: jint,
) -> jboolean {
    jni_export_guard!(&mut env, 0, {
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&(session_id as u64)) else {
            return 0;
        };
        let session = entry.session.lock();
        if session
            .terminal()
            .mode_get(mode_num.max(0) as u16, kind.max(0) as u8)
        {
            1
        } else {
            0
        }
    })
}

/// Drains the OSC 133 `last_command_output` buffer of a session (termlib
/// getLastCommandOutput equivalent, research-supplement-4.md §1.2) and
/// returns it as a string; null when the buffer is empty or the session is
/// gone. Reading clears the buffer, so each call sees fresh data.
#[unsafe(no_mangle)]
pub extern "system" fn Java_terminal_emulator_bridge_NativeBridge_getLastCommandOutput<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: jlong,
) -> jstring {
    jni_export_guard!(&mut env, std::ptr::null_mut(), {
        let id = session_id as u64;
        let registry = rlock_session_registry();
        let Some(entry) = registry.get(&id) else {
            return std::ptr::null_mut();
        };
        let mut session = entry.session.lock();
        let output = session.take_last_command_output();
        drop(session);
        drop(registry);
        if output.is_empty() {
            return std::ptr::null_mut();
        }
        match env.new_string(&output) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    })
}
