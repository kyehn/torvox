package terminal.emulator.runtime

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.os.SystemClock
import android.view.Surface
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.core.net.toUri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import terminal.emulator.bell.BellHandler
import terminal.emulator.bell.BellMode
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.BridgeTheme
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.Shell
import terminal.emulator.bridge.TerminalConfig
import terminal.emulator.bridge.createBridge
import terminal.emulator.monitor.RenderWatchDog
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.util.ArgumentTokenizer
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton

data class RuntimeState(
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val rows: Int = 24,
    val cols: Int = 80,
    val activeSessionId: Long = 0L,
    val sessionIds: List<Long> = emptyList(),
)

/**
 * Window after a user scroll gesture during which new output must NOT yank the viewport back to the
 * bottom (P1-1 recentlyScrolled guard; branch-B semantics, kept off in the default termux-parity
 * path by feeding a zero timestamp — see [shouldResetScroll]).
 */
const val RECENT_SCROLL_WINDOW_NANOS: Long = 100_000_000L

/**
 * P1-1 scroll-reset decision (termux `onScreenUpdated` parity, pure and side-effect free so it can
 * be table-driven tested).
 *
 * termux semantics (TerminalView.java onScreenUpdated, verified against termux-app master): new
 * output scrolls the viewport back to the bottom UNLESS text selection is active or auto-scroll is
 * explicitly disabled. Verified against source: termux's `isAutoScrollDisabled()` is an explicit
 * host-app toggle (`toggleAutoScrollDisabled()`), NOT raised by user scrolling — so the default
 * here matches termux branch A (recentlyScrolled stays false); branch B may pass a
 * gesture-time-window predicate later.
 *
 * @param scrollActive SCROLL-button explicit lock (user wants to stay browsing; maps to termux's
 *   explicit auto-scroll disable).
 * @param hasSelectionOrDrag selection active or handle drag in progress (maps to termux's
 *   `isSelectingText()` / skipScrolling parameter).
 * @param newOutput native PTY-ingest flag consumed this frame (NOT the render() count, which also
 *   counts cursor-blink repaints).
 * @param recentlyScrolled true within [RECENT_SCROLL_WINDOW_NANOS] of a UI scroll gesture; the
 *   caller passes the real window check so a brief in-flight scroll is not yanked by an output
 *   burst landing mid-gesture (100 ms guard — a deliberate improvement over raw termux parity).
 */
internal fun shouldResetScroll(
    scrollActive: Boolean,
    hasSelectionOrDrag: Boolean,
    newOutput: Boolean,
    recentlyScrolled: Boolean = false,
): Boolean = newOutput && !hasSelectionOrDrag && !scrollActive && !recentlyScrolled

/**
 * Session state is encoded in two booleans:
 * - running=true, renderThreadExited=false → alive
 * - running=true, renderThreadExited=true → dead (needs cleanup)
 * - running=false, renderThreadExited=* → stopped (stale entry; skip) renderThreadExited is set by
 *   the render thread after loop exit; always read under sessionLock alongside running.
 */
internal data class SessionEntry(
    val id: Long,
    // Invariant: bridge is never null while the entry lives (created non-
    // null in createSession, never reassigned). The scattered
    // `entry.bridge == null` checks are defensive-only and always false.
    var bridge: Bridge?,
    var renderThreadRef: Thread?,
    @Volatile var running: Boolean,
    @Volatile var renderThreadExited: Boolean = false,
    @Volatile var restartAttempts: Int = 0,
    // Same initial value as TerminalRuntime.INITIAL_RESTART_DELAY_MS (100L);
    // decayRestartCounts/confirmRestartGrace reset to that constant after a
    // healthy period, so a fresh session must start from the same delay.
    @Volatile var nextRestartDelayMs: Long = 100L,
    // True while a dead-render-thread restart is scheduled (handleDeadRenderThread
    // dispatched and waiting out the backoff delay). Guards against the render
    // monitor re-dispatching the same dead entry every 500ms tick while the
    // backoff delay (up to 1000ms) is still pending — without it restartAttempts
    // would be double-counted and the session closed prematurely.
    @Volatile var restartScheduled: Boolean = false,
    // True when a render thread failed to die within the join timeout and
    // may still be executing native render code against this session's
    // bridge/surface. While set, closeSession/stop/closeDeadSession must
    // NOT call releaseGpuSurface() or bridge.close() — destroying the
    // native session under a resumed thread is a use-after-free. Cleared
    // only when a subsequent join confirms the thread exited. The leaked
    // session is reclaimed when the process dies.
    @Volatile var renderThreadPossiblyAlive: Boolean = false,
    // The thread that failed to die within a join timeout (GPU hang). Kept
    // so a later exit path can join it once more: if it finally exited, the
    // session is safe to close; if it is still hung, close must be skipped.
    @Volatile var hungRenderThread: Thread? = null,
    // Set under sessionLock when closeSession begins. startRenderThread
    // checks it and refuses to start a fresh thread, closing the
    // close-vs-restart TOCTOU window: without it, a concurrent
    // resumeRendering/switchSession could start a new render thread after
    // the close decision but before bridge.close(), leaving an orphaned
    // thread polling a destroyed native session (global event queue
    // double-consumer, native UAF risk).
    @Volatile var closing: Boolean = false,
    //  fast-death recovery (warp WarpTerminalService.kt:906-915):
    // spawn timestamp (elapsedRealtime) so a shell that dies within
    // FAST_DEATH_THRESHOLD_MS can be detected and retried with
    // /system/bin/sh. Reset on every (re)spawn.
    @Volatile var spawnedAtRealtimeMs: Long = SystemClock.elapsedRealtime(),
    // Consecutive fast-death retries for this session (bounded by
    // MAX_FAST_DEATH_RETRIES; never reset — a session that fast-dies
    // repeatedly stays dead after the budget is exhausted).
    @Volatile var fastDeathCount: Int = 0,
    // True once the user typed anything into this session; a fast exit
    // AFTER user input is a legitimate quick exit (e.g. `exit`), not a
    // broken shell, so fast-death recovery is skipped.
    @Volatile var userTypedSinceSpawn: Boolean = false,
    // True between the fast-death detection and the respawn completing.
    // The render monitor must NOT restart the render thread in this
    // window: the old thread's exit event is consumed, polling the dead
    // session would re-trigger fast-death (double respawn race) — the
    // respawn thread restarts the render thread itself.
    // when true (SCROLL button active), new output should NOT
    // auto-reset scroll — the user intentionally wants to stay browsing.
    @Volatile var scrollActive: Boolean = false,
    // P1-1: timestamp (System.nanoTime) of the last UI-thread scroll
    // gesture (written in setScrollOffset). The render thread reads it to
    // compute the recentlyScrolled guard; 0L means "never scrolled".
    @Volatile var lastScrollNanos: Long = 0L,
    // Input→echo latency probe (emulator-performance-verification): input
    // stamps land in writeToPty AND in Bridge.onPtyWrite (hardware-key path
    // bypasses writeToPty), echo pairing happens in the render loop when a
    // frame consumes the native new_output flag.
    val latencyProbe: LatencyProbe = LatencyProbe(),
    @Volatile var fastDeathRetryScheduled: Boolean = false,
    // the shell exited and the [Process completed (code X)]
    // prompt was fed to the terminal (see feedProcessCompletedPrompt).
    // The session stays visible and running until the user presses Enter.
    @Volatile var waitingForProcessCompleted: Boolean = false,
    // Exit code captured when the [Process completed] prompt was shown;
    // reused when Enter confirms the close.
    @Volatile var processExitCode: Int = 0,
    // Set by writeToPty when the user presses Enter on the prompt. The
    // render loop detects it and re-dispatches handleSessionExit so the
    // bridge close stays on the render thread (no UAF against a live
    // render loop).
    @Volatile var processCompletedConfirmed: Boolean = false,
) {
    // renderSignaled replaced a per-frame `CountDownLatch`, which had a
    // lost-wakeup race: after `bridge.render()` the loop published a fresh
    // latch and waited, but a producer `countDown()` on the stale latch
    // during the render left the new latch unsignaled, so the thread waited
    // the full timeout. A coalescing flag under a lock/condition avoids
    // both the race and the per-frame allocation.
    //
    // Remaining accepted window: a notifyRender() landing between the
    // `get()` check and the `set(false)` after waitOutput is coalesced into
    // the flag and can be cleared by that set(false), costing one idle
    // timeout (max ~500ms) — never a lost frame (the next signal or the
    // periodic frame tick re-renders). Coalescing signals inherently
    // allows this; the alternative (per-signal queue) is overkill here.

    val renderSignaled = java.util.concurrent.atomic.AtomicBoolean(false)

    @Volatile var forceRenderRequested: Boolean = false

    // P2-1 vsync alignment (warp semantics): raised by the Choreographer
    // frame callback on the MAIN thread every display frame. The callback is
    // SIGNAL-ONLY — it never touches the surface Mutex and never calls
    // bridge.render() directly (mutex starvation precedent: a non-render-
    // thread render call hung on the surface Mutex and permanently blocked
    // the real render thread). Consumed by the render loop's wake gate.
    @Volatile var vsyncRequested: Boolean = false

    @Volatile var lastRenderStart: Long = 0L

    @Volatile var lastRenderDone: Long = 0L
    var renderWatchDog: RenderWatchDog? = null

    @Volatile var lastSignalNanos: Long = System.nanoTime()

    fun notifyRender() {
        lastSignalNanos = System.nanoTime()
        renderSignaled.set(true)
        renderThreadRef?.let {
            java.util.concurrent.locks.LockSupport.unpark(it)
        }
    }

    @Volatile var scrollOffset: Int = 0
}

// ═══════════════════════════════════════════════════════════════════════════
// SECTION 1: Fields & injected dependencies
// ═══════════════════════════════════════════════════════════════════════════

@Singleton
class TerminalRuntime
@Inject
constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepository: SettingsRepository,
) {
    // ADR-0007: surface handed over by TerminalSurface; attached once the
    // session's Bridge exists (attach can only run after spawn).
    @Volatile private var pendingSurface: android.view.Surface? = null

    @Volatile private var pendingSurfaceWidth: Int = 0

    @Volatile private var pendingSurfaceHeight: Int = 0

    /** Render-thread lifecycle supervision (C6). */
    val renderSupervisor = RenderSupervisor()

    private val clipboardAccess = ClipboardAccess(context, tag = "Runtime")

    private val eventDispatcher = EventDispatcher()

    /** BellHandler with 4-mode support (SOUND/VIBRATE/SCREEN_FLASH/SILENT) and 150ms debounce. */
    private val bellHandler = BellHandler(context)

    /**
     * invoked from the render loop after every presented frame render thread). Lets the SurfaceView
     * refresh its accessibility contentDescription — the SurfaceView is self-drawn and has no text
     * nodes, so the render loop is the only content-changed signal. Must return quickly; the callback
     * may post to the main thread.
     */
    @Volatile var onFrameRendered: (() -> Unit)? = null

    init {
        // Sync bell handler mode from persisted setting on startup.
        // Uses a local scope since the class-level `scope` is not yet initialized.
        CoroutineScope(SupervisorJob() + Dispatchers.IO).launch {
            val modeId = settingsRepository.bellMode.first()
            bellHandler.setMode(BellMode.fromId(modeId))
        }
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    // In-flight native bell-flash animation; cancelled and restarted on each
    // new bell so a burst always restarts from phase 1.0.
    @Volatile private var bellFlashJob: Job? = null

    // Keep bell handler mode in sync with persisted setting at runtime  fix).
    init {
        scope.launch {
            settingsRepository.bellMode.collect { modeId ->
                bellHandler.setMode(BellMode.fromId(modeId))
            }
        }
    }

    private val _state = MutableStateFlow(RuntimeState())
    val state: StateFlow<RuntimeState> = _state.asStateFlow()

    private val sessions = ConcurrentHashMap<Long, SessionEntry>()

    // ── P2-1 vsync frame-callback chain (warp semantics) ─────────────
    // Posts UI work to the main looper (Choreographer.getInstance()
    // requires a Looper; TerminalRuntime itself may be built on any
    // thread via DI).
    private val mainHandler = android.os.Handler(android.os.Looper.getMainLooper())

    /**
     * Idempotence guard: exactly one self-rescheduling callback chain per process (CAS-guarded; reset
     * on registration failure to allow retry).
     */
    private val vsyncChainStarted = java.util.concurrent.atomic.AtomicBoolean(false)

    /**
     * The single self-rescheduling Choreographer frame callback: each display frame IS the vsync
     * signal. It only raises [SessionEntry.vsyncRequested] + unparks the render thread via
     * [SessionEntry.notifyRender] — the render thread owns all rendering (design D3: main thread
     * never touches the surface Mutex).
     *
     * The chain re-posts itself unconditionally at the end of every frame, whether or not a frame is
     * pushed: RenderState's deferred fields (search_highlights / selection / pending_flash_phase)
     * rely on per-frame consumption, so a broken chain would silently freeze them.
     */
    private val vsyncFrameCallback =
        object : android.view.Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                // Lock-free read: sessions is a ConcurrentHashMap and
                // activeSessionId is @Volatile.
                val entry = sessions[activeSessionId]
                if (entry != null) {
                    entry.vsyncRequested = true
                    entry.notifyRender()
                }
                // Self-reschedule (warp pattern): keep the chain alive at
                // display refresh rate regardless of whether this frame
                // produced a paint.
                android.view.Choreographer.getInstance().postFrameCallback(this)
            }
        }

    /**
     * Starts the vsync frame-callback chain on the main looper. Idempotent: CAS guard ensures a
     * single chain for the process.
     */
    private fun ensureVsyncChainStarted() {
        if (!vsyncChainStarted.compareAndSet(false, true)) return
        mainHandler.post {
            try {
                android.view.Choreographer.getInstance().postFrameCallback(vsyncFrameCallback)
                LogUtil.d("Runtime", "vsync frame callback chain started")
            } catch (exception: Exception) {
                // Allow a later session start to retry the registration.
                vsyncChainStarted.set(false)
                LogUtil.e("Runtime", "vsync frame callback registration failed", exception)
            }
        }
    }

    // MCP dialog / file-pick requests from the embedded MCP server.
    // Wired by the UI layer (e.g. MainActivity) via setDialogRequestHandler;
    // responses are sent back through NativeBridge.dialogResult.
    @Volatile
    var dialogRequestHandler:
        (
            (
                sessionId: Long,
                requestId: Long,
                dialogType: String,
                title: String,
                message: String,
                options: List<String>,
            ) -> Unit
        )? =
        null

    @Volatile
    var pickFileRequestHandler:
        ((sessionId: Long, requestId: Long, startingPath: String, filter: String) -> Unit)? =
        null

    // called when the native MCP tool call times out
    // (300s) so the still-visible dialog is dismissed. Wired by the UI
    // layer alongside dialogRequestHandler.
    @Volatile var dialogCancelHandler: ((sessionId: Long, requestId: Long) -> Unit)? = null

    @Volatile var accentColor: Int = 0xFF2196F3.toInt()

    @Volatile var selectionBgColor: Int = 0xFF45475A.toInt()

    @Volatile var cellWidth: Float = 0f

    @Volatile var cellHeight: Float = 0f

    // ⑥ last font size pushed to native (tenths). Zoom gestures anchor to
    // this so the preview/finalize math starts from the actually rendered
    // size, not the raw settings value (which can differ when the size was
    // never explicitly set).
    @Volatile internal var appliedFontSizeTenths: Int = 0

    // Logical pixel cell dimensions (for grid row/col computation).
    // These are the raw native values WITHOUT density scaling.
    @Volatile var logicalCellWidth: Float = 0f

    @Volatile var logicalCellHeight: Float = 0f

    private val renderGeneration = java.util.concurrent.atomic.AtomicInteger(0)

    @Volatile private var activeSessionId: Long = 0L

    @Volatile private var starting = false
    private val sessionLock = Any()

    /**
     * Failsafe session request (termux-compatible app shortcut extra
     * `com.termux.app.failsafe_session`): the next session starts with the system shell and no prefix
     * bootstrap, so a broken bootstrap cannot brick terminal access. Consumed (not reset) by
     * buildConfig; a second shortcut tap while a session is already up is a no-op because torvox
     * keeps a single session (see start()'s `sessions.isNotEmpty()` guard).
     */
    @Volatile
    var failsafeRequested: Boolean = false
        private set

    fun requestFailsafeSession() {
        failsafeRequested = true
        LogUtil.w(
            "Runtime",
            "Failsafe session requested — next start uses /system/bin/sh without prefix",
        )
    }

    /**
     * Serialises surface lifecycle transitions (pause/resume). Both [pauseRendering] and
     * [resumeRendering] run on this single thread so that surface-destroy → surface-available
     * ordering is preserved; running them on different threads can leave a fresh surface without a
     * render thread (async pause stopping a just-started resume).
     */
    private val surfaceTransitionExecutor: java.util.concurrent.ExecutorService =
        java.util.concurrent.Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "SurfaceTransition").apply { isDaemon = true }
        }

    @Volatile private var foregroundServiceRunning = false

    @Volatile private var renderMonitorJob: Job? = null

    // Serializes startRenderMonitor's check-then-assign: start() (IO
    // coroutine) and resumeRendering (surfaceTransitionExecutor thread) can
    // both reach it, and an unsynchronized pair would launch two monitor
    // loops (stopRenderMonitor cancels only the referenced one, leaving the
    // other to spin until the scope cancels).
    private val monitorLock = Any()

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2: Render thread lifecycle
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Answer MCP clipboard_get / OSC 52 read requests: read the system clipboard and reply via
     * [NativeBridge.clipboardResult]. Empty text is a legitimate result; only exceptions produce an
     * empty fallback reply.
     */
    private fun dispatchClipboardRequests(
        requests: List<terminal.emulator.bridge.Bridge.ClipboardRequest>,
    ) {
        requests.forEach { request ->
            try {
                val text = clipboardAccess.clipboardText().orEmpty()
                NativeBridge.clipboardResult(request.sessionId, request.requestId, text)
            } catch (exception: Exception) {
                // Class only: exception messages can embed clipboard text.
                LogUtil.e("Runtime", "clipboard request dispatch failed: ${exception.javaClass.simpleName}")
                NativeBridge.clipboardResult(request.sessionId, request.requestId, "")
            }
        }
    }

    /**
     * print the [Process completed (code X)] - press Enter prompt directly into the VT parser (the
     * child is gone, so the PTY no longer carries writes; the screen must be updated in-band).
     */
    private fun feedProcessCompletedPrompt(entry: SessionEntry, exitCode: Int) {
        val text = PROCESS_COMPLETED_PROMPT_PREFIX + exitCode + PROCESS_COMPLETED_PROMPT_SUFFIX
        try {
            entry.bridge?.feedTerminal(text.encodeToByteArray())
        } catch (exception: Exception) {
            LogUtil.w("Runtime", "feedTerminal failed for [Process completed] prompt", exception)
        }
        entry.notifyRender()
    }

    /**
     * for the foreground session, keep it visible after the shell exits and show a
     * [Process completed] prompt instead of closing immediately (termux-app
     * TerminalSession.java:353-364 semantics). The entry stays in the session map with running=true
     * until the user presses Enter, which then closes it. Returns true when the prompt was shown
     * (caller should return early and NOT close the session).
     */
    private fun maybeShowProcessCompletedPrompt(entry: SessionEntry, exitCode: Int): Boolean {
        if (entry.id != activeSessionId || entry.waitingForProcessCompleted) return false
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return false
            entry.waitingForProcessCompleted = true
            entry.processExitCode = exitCode
        }
        LogUtil.i(
            "Runtime",
            "session ${entry.id} exited with code $exitCode; showing [Process completed] prompt",
        )
        feedProcessCompletedPrompt(entry, exitCode)
        return true
    }

    private fun handleSessionExit(
        entry: SessionEntry,
        exitCode: Int,
        // native-measured child lifetime (ms); 0 when the
        // event predates the field (or is a sweep). Fast-death uses this
        // instead of Kotlin event-latency timing.
        aliveMs: Long,
    ) {
        //  fast-death recovery (warp WarpTerminalService.kt:906-915):
        // a shell that dies within FAST_DEATH_THRESHOLD_MS of spawn with no
        // user input is almost certainly a broken bootstrap/prefix shell
        // (or a misconfigured login binary). Retry with /system/bin/sh and
        // exponential backoff before surfacing the exit. The respawn runs
        // on a separate thread so the backoff delay never blocks the render
        // thread (which owns this call site). Extracted to
        // [tryFastDeathRecovery] for the detekt complexity limit.
        // this function runs both for the initial shell exit
        // (from the poll.exit branch) and, after the user presses Enter on
        // the [Process completed] prompt, for the confirmed close (render
        // loop re-dispatch). Fast-death recovery was already ruled out when
        // the prompt was first shown, so it is skipped for confirmed closes.
        val confirmedClose = entry.processCompletedConfirmed
        if (!confirmedClose) {
            if (tryFastDeathRecovery(entry, exitCode, aliveMs)) return
            // Foreground session: keep it visible with a [Process completed]
            // prompt instead of closing immediately. The entry stays in
            // sessions with running=true until Enter, which then closes it.
            if (maybeShowProcessCompletedPrompt(entry, exitCode)) return
        }
        LogUtil.i("Runtime", "session ${entry.id} exited with code $exitCode")
        // Phase 1 (locked): capture the possibly-hung thread; the actual
        // join runs UNLOCKED below (up to THREAD_JOIN_TIMEOUT_MS) so a hung
        // GPU thread does not stall every session operation.
        val hungThreadToJoin: Thread?
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            entry.running = false
            hungThreadToJoin = if (entry.renderThreadPossiblyAlive) entry.hungRenderThread else null
        }

        // Phase 2 (UNLOCKED): one final chance — the previously-hung thread
        // may have resumed and exited by now (GPU unblocked). Join it with a
        // fresh timeout; only skip bridge close if it is STILL alive.
        // Guard against joining ourselves: this function runs on the render
        // thread (poll.exit), and a concurrent join-timeout path may have
        // recorded THIS thread as hung. Joining self always times out and
        // would leak the native session for nothing — the caller is about to
        // exit the loop anyway.
        val hungExited =
            if (hungThreadToJoin == null || hungThreadToJoin === Thread.currentThread()) {
                true
            } else if (hungThreadToJoin.isAlive) {
                hungThreadToJoin.interrupt()
                hungThreadToJoin.join(THREAD_JOIN_TIMEOUT_MS)
                !hungThreadToJoin.isAlive
            } else {
                true
            }

        // Decide close eligibility under the lock, then perform the actual
        // bridge.close() OUTSIDE it: Session::drop kills the child and joins
        // reader/wait threads (up to ~100ms+); holding sessionLock during
        // that would block every session operation (switch/create/close).
        val skipClose =
            synchronized(sessionLock) {
                if (!sessions.containsKey(entry.id)) return
                // The watchdog thread is per-session; stop it here (normal exit
                // path) or every exited session leaks a polling thread.
                entry.renderWatchDog?.stop()
                entry.renderWatchDog = null
                // Clear the flag only when the joined thread is still the
                // recorded hung thread (no other path replaced it meanwhile).
                if (hungExited && entry.hungRenderThread === hungThreadToJoin) {
                    entry.renderThreadPossiblyAlive = false
                    entry.hungRenderThread = null
                }
                entry.renderThreadPossiblyAlive
            }
        try {
            if (skipClose) {
                // A render thread may still be stuck in native render
                // code (GPU hang, join timeout). destroySession under it
                // is a use-after-free; leave the native session to be
                // reclaimed at process death, same rule as closeSession.
                LogUtil.e(
                    "Runtime",
                    "session ${entry.id} render thread possibly alive — skipping bridge close on exit",
                )
            } else {
                entry.bridge?.close()
            }
        } catch (e: Exception) {
            LogUtil.e("Runtime", "session ${entry.id} bridge close on exit failed", e)
        }
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            sessions.remove(entry.id)
            if (entry.id == activeSessionId) {
                // The foreground session just disappeared; the replacement
                // session has no render thread (only the active session
                // renders) so its output/events would never be polled and
                // the terminal would appear frozen. Activate it now.
                activeSessionId = sessions.keys.sorted().lastOrNull() ?: 0L
                if (activeSessionId != 0L) {
                    activateReplacementSession(
                        activeSessionId,
                        "handleSessionExit",
                        markRunning = false,
                        withRetry = true,
                        syncGrid = false,
                    )
                }
            }
            updateForegroundSessionCount(sessions.size)
            updateState()
        }
    }

    /**
     * fast-death detection: when the child's native lifetime is within [FAST_DEATH_THRESHOLD_MS], the
     * user typed nothing, and the retry budget is not exhausted, log + schedule the /system/bin/sh
     * respawn and return true (the exit is consumed). Extracted from handleSessionExit for the detekt
     * complexity limit.
     */
    private fun tryFastDeathRecovery(
        entry: SessionEntry,
        exitCode: Int,
        aliveMs: Long,
    ): Boolean {
        val effectiveAliveMs =
            resolveAliveMs(
                aliveMs,
                SystemClock.elapsedRealtime(),
                entry.spawnedAtRealtimeMs,
            )
        if (!shouldRetryFastDeath(effectiveAliveMs, entry.userTypedSinceSpawn, entry.fastDeathCount)) {
            return false
        }
        val attempt: Int
        synchronized(sessionLock) {
            if (
                !shouldScheduleFastDeathRetry(entry.closing, entry.fastDeathCount, MAX_FAST_DEATH_RETRIES)
            ) {
                return false
            }
            entry.fastDeathCount++
            attempt = entry.fastDeathCount
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            // Block the render monitor from restarting the render
            // thread while the respawn is pending (see field doc).
            entry.fastDeathRetryScheduled = true
        }
        val backoffMs = fastDeathBackoffMs(attempt)
        LogUtil.w(
            "Runtime",
            "Fast death detected for session ${entry.id} (attempt $attempt/$MAX_FAST_DEATH_RETRIES, alive ${effectiveAliveMs}ms, exitCode $exitCode); retrying in ${backoffMs}ms with /system/bin/sh",
        )
        scheduleFastDeathRetry(entry, backoffMs)
        return true
    }

    /**
     * fast-death recovery (warp WarpTerminalService.kt:906-915): after the backoff delay, clear the
     * grid, kill the dead child, respawn with /system/bin/sh and restart the render thread. Runs on
     * its own thread so the delay never blocks the render/poll loop. On respawn failure the session
     * is removed exactly like a normal exit.
     */
    private fun scheduleFastDeathRetry(entry: SessionEntry, backoffMs: Long) {
        Thread(
            {
                try {
                    Thread.sleep(backoffMs)
                    synchronized(sessionLock) {
                        if (!sessions.containsKey(entry.id) || entry.closing) return@Thread
                    }
                    // Clear the grid so the failed shell's stderr diagnostic
                    // does not bleed into the fallback session (warp clears
                    // with the same ESC[2J ESC[H sequence).
                    entry.bridge?.feedTerminal("\u001b[2J\u001b[H".toByteArray())
                    val bridge = entry.bridge ?: return@Thread
                    val grid = bridge.getGridRowsColsPacked()
                    val rows = ((grid shr 32) and 0xFFFF).toInt().coerceAtLeast(1)
                    val cols = (grid and 0xFFFF).toInt().coerceAtLeast(1)
                    // Close the dead child's native session first: the PTY
                    // child already exited, so this is fast; without it
                    // initSession would leak the old native Session.
                    try {
                        bridge.close()
                    } catch (closeException: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "fast-death: bridge close failed for session ${entry.id}",
                            closeException,
                        )
                    }
                    val respawned = bridge.spawnTerminal(rows, cols, "/system/bin/sh")
                    if (respawned <= 0L) {
                        LogUtil.e(
                            "Runtime",
                            "fast-death respawn failed for session ${entry.id}, surfacing exit",
                        )
                        synchronized(sessionLock) {
                            sessions.remove(entry.id)
                            entry.fastDeathRetryScheduled = false
                            updateForegroundSessionCount(sessions.size)
                            updateState()
                        }
                        return@Thread
                    }
                    synchronized(sessionLock) {
                        if (entry.closing) return@synchronized
                        entry.running = true
                        entry.renderThreadExited = false
                        entry.spawnedAtRealtimeMs = SystemClock.elapsedRealtime()
                        entry.userTypedSinceSpawn = false
                        entry.fastDeathRetryScheduled = false
                        renderSupervisor.startRenderThread(entry)
                    }
                    // The respawned native session has no window attached
                    // (spawnTerminal re-created it with a fresh session id).
                    // Re-attach the surface so the render thread has
                    // something to draw into — otherwise the fallback shell
                    // runs but renders black frames until the next
                    // surfaceCreated.
                    attachPendingSurface(bridge)
                    LogUtil.w(
                        "Runtime",
                        "fast-death respawn OK for session ${entry.id} (/system/bin/sh rows=$rows cols=$cols)",
                    )
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "fast-death retry failed for session ${entry.id}", exception)
                }
            },
            "FastDeath-${entry.id}",
        )
            .apply {
                isDaemon = true
                start()
            }
    }

    private fun closeDeadSession(entry: SessionEntry) {
        // Mark closing under the lock, symmetric with closeSession: the
        // replacement branches of handleSessionExit/closeDeadSession call
        // startRenderThread for the new active session, and without the
        // closing flag a racing closeDeadSession(entry) could have its
        // bridge closed while a replacement render thread starts on it —
        // orphaned thread double-consuming the global event queue.
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            entry.closing = true
            entry.running = false
        }
        // LogUtil.e already logs to logcat — no duplicate write.
        LogUtil.e("Runtime", "session ${entry.id} exceeded max restart attempts, closing session")
        try {
            if (entry.renderThreadPossiblyAlive) {
                // The hung thread may still be inside native render code;
                // destroying the session under it is a use-after-free.
                // Leave the native session to be reclaimed at process death.
                LogUtil.e(
                    "Runtime",
                    "session ${entry.id} render thread possibly alive — skipping bridge close",
                )
            } else {
                entry.bridge?.close()
            }
        } catch (e: Exception) {
            LogUtil.e("Runtime", "session ${entry.id} bridge close during cleanup failed", e)
        }
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            sessions.remove(entry.id)
            // Keep the foreground service count in sync, symmetric with
            // handleSessionExit and closeSession: closing the LAST session
            // via this path must stop the foreground service and clear its
            // stale notification.
            updateForegroundSessionCount(sessions.size)
            if (entry.id == activeSessionId) {
                // Same as handleSessionExit: the replacement session needs a
                // render thread or its output/events are never polled and the
                // terminal appears frozen.
                val remaining = sessions.keys.sorted()
                activeSessionId = remaining.lastOrNull() ?: 0L
                if (activeSessionId != 0L) {
                    activateReplacementSession(
                        activeSessionId,
                        "closeDeadSession",
                        markRunning = false,
                        withRetry = true,
                        syncGrid = false,
                    )
                }
            }
        } // synchronized(sessionLock)
        updateState()
    }

    private fun startForegroundServiceIfNeeded() {
        if (!foregroundServiceRunning) {
            terminal.emulator.service.TerminalForegroundService.start(context)
            foregroundServiceRunning = true
            LogUtil.d("Runtime", "foreground service started")
        }
    }

    private fun stopForegroundService() {
        // NO flag gate: MainActivity.onCreate starts the service
        // directly via the static TerminalForegroundService.start() without
        // setting this flag, so a flag-gated stop would leak the service and
        // its PARTIAL_WAKE_LOCK when the Activity is destroyed before the
        // runtime's bootstrap completes. stopService on a non-running
        // service is a harmless no-op (returns false), so stop unconditionally
        // and reset the flag.
        val stopped =
            try {
                terminal.emulator.service.TerminalForegroundService.stop(context)
            } catch (serviceException: Exception) {
                if (serviceException is kotlinx.coroutines.CancellationException) {
                    throw serviceException
                }
                // A stopService binder failure (rare) must not leave the flag
                // true: a stale flag makes the next startForegroundServiceIfNeeded
                // skip starting, leaving no notification and no wake lock for
                // live sessions. Reset the flag anyway — the service either
                // stopped or is about to be killed by the system.
                LogUtil.e("Runtime", "Failed to stop foreground service", serviceException)
                false
            }
        foregroundServiceRunning = false
        if (stopped) {
            LogUtil.d("Runtime", "foreground service stopped")
        }
    }

    /**
     * Update the foreground service session count, keeping the [foregroundServiceRunning] flag in
     * sync: the service stops itself when the count reaches 0
     * (TerminalForegroundService.updateSessionCount calls stop()), so the flag MUST be cleared here
     * or a later startForegroundServiceIfNeeded would skip restarting the service — no foreground
     * notification, no wake lock, and background sessions can be killed.
     *
     * Exception-safe: called from inside sessionLock on the close paths; a service exception must
     * never escape the lock (it would skip updateState and corrupt the session bookkeeping).
     */
    private fun updateForegroundSessionCount(count: Int) {
        if (count <= 0) {
            foregroundServiceRunning = false
        }
        try {
            terminal.emulator.service.TerminalForegroundService.updateSessionCount(context, count)
        } catch (exception: Exception) {
            if (exception is kotlinx.coroutines.CancellationException) {
                throw exception
            }
            LogUtil.e("Runtime", "updateSessionCount failed", exception)
        }
    }

    private data class SelectionStateSnapshot(
        val startRow: Int,
        val startCol: Int,
        val endRow: Int,
        val endCol: Int,
        val hasSelection: Boolean,
        val mode: Byte,
        // P1-1: true while a selection-handle drag is in progress (UI
        // thread, via setSelectionDragging). The render thread folds this
        // into hasSelectionOrDrag for the scroll-reset decision; it is
        // NOT forwarded to native setSelection — dragging only affects
        // scroll semantics, never the painted selection range.
        val dragging: Boolean = false,
    )

    private val selectionState =
        java.util.concurrent.atomic.AtomicReference(
            SelectionStateSnapshot(0, 0, 0, 0, false, 0),
        )

    /**
     * Active session's scroll offset: read by the surface on session switch to resync its local
     * selection-math offset.
     */
    fun activeSessionScrollOffset(): Int {
        synchronized(sessionLock) {
            return sessions[activeSessionId]?.scrollOffset ?: 0
        }
    }

    fun setScrollOffset(offset: Int) {
        val entry = sessions[activeSessionId] ?: return
        entry.scrollOffset = offset
        // P1-1: record the gesture time so the render thread's
        // recentlyScrolled guard can suppress the new-output scroll reset
        // for RECENT_SCROLL_WINDOW_NANOS after user scrolling.
        entry.lastScrollNanos = System.nanoTime()
        // The render thread already reads entry.scrollOffset and calls
        // bridge.setScrollOffset() under the surface lock, so calling it here
        // would be a redundant JNA round-trip + surface-lock acquisition on the
        // calling thread (often the UI thread during scroll). Just signal the
        // render thread to pick up the change.
        entry.notifyRender()
    }

    /**
     * sync scroll-active state from TerminalViewModel so the render thread knows whether to
     * auto-reset scroll on new output.
     */
    fun setScrollActive(active: Boolean) {
        val entry = sessions[activeSessionId] ?: return
        entry.scrollActive = active
    }

    /**
     * P1-1: mark the start/end of a selection-handle drag on the UI thread. Folds into
     * SelectionStateSnapshot.dragging via the existing selectionState channel; the render thread
     * treats dragging like an active selection for the scroll-reset decision (termux skipScrolling
     * parity). endSelection/clearSelection overwrite the snapshot with dragging=false through
     * setSelection, so no dedicated clear needed.
     */
    fun setSelectionDragging(dragging: Boolean) {
        val current = selectionState.get()
        if (current.dragging == dragging) return
        selectionState.set(current.copy(dragging = dragging))
        sessions[activeSessionId]?.notifyRender()
    }

    fun forceRender() {
        val entry = sessions[activeSessionId] ?: return
        entry.forceRenderRequested = true
        entry.notifyRender()
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 3: Session lifecycle
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Write `$HOME/.mkshrc` with a termux-parity prompt (self-healing).
     *
     * root cause: with no rc file, interactive mksh falls back to the AOSP `/system/etc/mkshrc`
     * prompt `:/data/.../home $ ` — 38 columns wide. Any typed command longer than the remaining ~10
     * columns forces mksh's horizontal line-scroll redraw (`\r` + scrolled window + `<` marker +
     * backspace run), which renders as the garbled echo users reported ("cho …" fragments and a stray
     * `<` at the right edge). Real Termux avoids this entirely with the short `PS1='$ '` prompt —
     * same parity rule as every other termux-behavior fix in this round.
     *
     * mksh reads `$ENV` when set, else `~/.mkshrc` for interactive shells; we source the system rc
     * first (keeps its PATH/alias setup) and then override PS1. Bash from a bootstrap never reads
     * this file. The file is overwritten when it does not already contain the parity marker so stale
     * installs self-heal.
     */
    private fun ensureMkshPromptRc(homeDir: String) {
        val mkshRcFile = java.io.File(homeDir, ".mkshrc")
        val parityMarker = "PS1='$ '"
        if (mkshRcFile.isFile) {
            try {
                if (mkshRcFile.readText().contains(parityMarker)) return
            } catch (_: Exception) {
                // unreadable — overwrite below
            }
        }
        try {
            mkshRcFile.parentFile?.mkdirs()
            mkshRcFile.writeText(
                "# torvox: termux-parity prompt (see TerminalRuntime.ensureMkshPromptRc)\n" +
                    ". /system/etc/mkshrc\n" +
                    "$parityMarker\n",
            )
        } catch (exception: Exception) {
            LogUtil.w("Runtime", "Failed to write .mkshrc: $exception")
        }
    }

    private fun withMkshEnvInjection(
        baseEnv: Map<String, String>,
        homeDir: String,
    ): Map<String, String> = if (!baseEnv.containsKey("ENV")) baseEnv + ("ENV" to "$homeDir/.mkshrc") else baseEnv

    private suspend fun buildConfig(
        rows: Int = DEFAULT_GRID_ROWS,
        cols: Int = DEFAULT_GRID_COLS,
    ): TerminalConfig {
        val configReads = coroutineScope {
            val shellDeferred = async { settingsRepository.shell.first() }
            val scrollbackDeferred = async { settingsRepository.scrollbackLines.first() }
            val fontDeferred = async { computeFontSizeTenths() }
            val themeDeferred = async { resolveThemeName() }
            val envDeferred = async { settingsRepository.environmentVariables.first() }
            ConfigReads(
                shellPath = shellDeferred.await(),
                scrollbackLines = scrollbackDeferred.await(),
                fontSizeTenths = fontDeferred.await(),
                themeName = themeDeferred.await(),
                environmentVariables = envDeferred.await(),
            )
        }
        val resolvedTheme = BuiltInThemes.byName(configReads.themeName)
        val shell = resolveShell(configReads.shellPath)
        val bridgeTheme = makeBridgeTheme(resolvedTheme)
        accentColor = bridgeTheme.ansi5
        selectionBgColor = bridgeTheme.selectionBg
        val prefixDir = java.io.File(context.filesDir, "usr").absolutePath
        val homeDir = java.io.File(context.filesDir, "home").absolutePath
        // Failsafe (termux app shortcut "New session (Failsafe)"): bypass
        // the prefix bootstrap entirely — system shell, system PATH, no
        // PREFIX — so a broken bootstrap cannot brick terminal access
        // (matches termux-app TermuxSession.java:95-113 isFailsafe path).
        if (failsafeRequested) {
            return buildFailsafeConfig(rows, cols, configReads, bridgeTheme, homeDir)
        }
        // Prefix shell resolution mirrors nix-on-droid-app
        // (UnixShellEnvironment.LOGIN_SHELL_BINARIES = login, bash, zsh,
        // fish, sh): nix-on-droid bootstraps expose bin/login (a static
        // proot entry point), termux bootstraps expose bin/bash. The first
        // existing candidate wins; completeness additionally requires the
        // bootstrap's own tree (termux: lib/; nix-on-droid: nix/).
        // Only ELF candidates are eligible: termux also ships a bin/login
        // *script* (motd + exec, shebang #!/data/.../usr/bin/sh) which the
        // linker-wrapper spawn path cannot load ("bad ELF magic" —
        // emulator-verified), so it must not be selected.
        val prefixShell = findPrefixShell(prefixDir)
        val prefixComplete =
            prefixShell != null &&
                (
                    java.io.File("$prefixDir/lib").isDirectory ||
                        java.io.File("$prefixDir/nix").isDirectory
                    ) &&
                java.io.File("$prefixDir/etc").isDirectory
        val effectivePrefix = if (prefixComplete) prefixDir else ""
        val effectiveShell = if (prefixComplete) Shell.Custom("$prefixDir/$prefixShell") else shell
        val effectiveHome =
            if (prefixComplete) {
                homeDir
            } else {
                java.io
                    .File(context.filesDir, "home")
                    .apply {
                        if (!exists() && !mkdirs()) {
                            LogUtil.w("Runtime", "Failed to create home directory: $this")
                        }
                    }
                    .absolutePath
            }
        val effectivePath: String =
            if (prefixComplete) {
                "$prefixDir/bin:${System.getenv("PATH").orEmpty().ifEmpty { "/system/bin:/system/xbin" }}"
            } else {
                System.getenv("PATH").orEmpty().ifEmpty { "/system/bin:/system/xbin" }
            }
        ensureMkshPromptRc(effectiveHome)
        val effectiveEnv =
            if (!prefixComplete) {
                withMkshEnvInjection(configReads.environmentVariables, effectiveHome)
            } else {
                configReads.environmentVariables
            }
        return TerminalConfig(
            shell = effectiveShell,
            rows = rows,
            cols = cols,
            scrollbackLines = configReads.scrollbackLines,
            font_size_tenths = configReads.fontSizeTenths,
            theme = bridgeTheme,
            home = effectiveHome,
            user = System.getProperty("user.name") ?: "shell",
            path = effectivePath,
            workingDirectory = effectiveHome,
            prefix = effectivePrefix,
            env = effectiveEnv,
        )
    }

    /**
     * Failsafe session config: system shell, system PATH, no PREFIX. Extracted from buildConfig so
     * the failsafe branch does not push buildConfig past the detekt LongMethod limit.
     */
    private fun buildFailsafeConfig(
        rows: Int,
        cols: Int,
        configReads: ConfigReads,
        bridgeTheme: BridgeTheme,
        homeDir: String,
    ): TerminalConfig {
        val home =
            java.io
                .File(context.filesDir, "home")
                .apply {
                    if (!exists() && !mkdirs()) {
                        LogUtil.w("Runtime", "Failed to create home directory: $this")
                    }
                }
                .absolutePath
        ensureMkshPromptRc(home)
        val failsafeEnv = withMkshEnvInjection(configReads.environmentVariables, home)
        return TerminalConfig(
            shell = Shell.SystemDefault,
            rows = rows,
            cols = cols,
            scrollbackLines = configReads.scrollbackLines,
            font_size_tenths = configReads.fontSizeTenths,
            theme = bridgeTheme,
            home = home,
            user = System.getProperty("user.name") ?: "shell",
            path = System.getenv("PATH").orEmpty().ifEmpty { "/system/bin:/system/xbin" },
            workingDirectory = homeDir,
            prefix = "",
            env = failsafeEnv,
        )
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2b: Render-thread supervision (extracted C6)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns render-thread lifecycle: monitor loop, dead-thread detection, restart/backoff, and thread
     * start/stop.
     *
     * Extracted from TerminalRuntime so the supervision logic has one home and the orchestrator stays
     * thin. Inner class: accesses TerminalRuntime's session registry/locks without threading them
     * through constructors (pure code move, zero behavior change).
     */
    inner class RenderSupervisor {
        internal fun startRenderMonitor() {
            synchronized(monitorLock) {
                if (renderMonitorJob?.isActive == true) return
                renderMonitorJob = scope.launch {
                    while (isActive) {
                        delay(RENDER_MONITOR_INTERVAL_MS)
                        checkSessions()
                    }
                }
            }
        }

        internal fun stopRenderMonitor() {
            synchronized(monitorLock) {
                renderMonitorJob?.cancel()
                renderMonitorJob = null
            }
        }

        internal fun checkSessions() {
            val deadSessions = mutableListOf<SessionEntry>()
            val healthySessions = mutableListOf<SessionEntry>()
            synchronized(sessionLock) {
                scanSessionsForDeath(deadSessions, healthySessions)
            }
            decayRestartCounts(healthySessions)
            for (entry in deadSessions) {
                scope.launch {
                    handleDeadRenderThread(entry)
                }
            }
        }

        internal fun scanSessionsForDeath(
            deadSessions: MutableList<SessionEntry>,
            healthySessions: MutableList<SessionEntry>,
        ) {
            for (entry in sessions.values) {
                if (!entry.running) continue
                if (entry.bridge == null) continue
                if (entry.renderThreadExited) {
                    deadSessions.add(entry)
                } else {
                    val thread = entry.renderThreadRef
                    if (thread != null && !thread.isAlive) {
                        deadSessions.add(entry)
                    }
                    if (entry.restartAttempts > 0) {
                        healthySessions.add(entry)
                    }
                }
            }
        }

        internal fun decayRestartCounts(healthySessions: MutableList<SessionEntry>) {
            for (entry in healthySessions) {
                synchronized(sessionLock) {
                    if (!entry.running || entry.renderThreadExited) continue
                    if (entry.restartAttempts > 0) {
                        entry.restartAttempts = 0
                        entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                    }
                }
            }
        }

        internal suspend fun handleDeadRenderThread(entry: SessionEntry) {
            LogUtil.w(
                "Runtime",
                "session ${entry.id} render thread exited, restart attempt ${entry.restartAttempts + 1}",
            )

            // Phase 1 (locked): quick state checks only. Phase 2 (UNLOCKED):
            // stopDeadRenderThreadResources joins the old thread (up to 1s) —
            // holding sessionLock across a join blocks every session operation.
            synchronized(sessionLock) {
                if (!entry.running) return
                // while a fast-death respawn is pending the
                // render thread must stay dead — the respawn thread starts
                // it with the fresh /system/bin/sh session. Restarting now
                // would poll the consumed-exit session and double-respawn.
                if (entry.fastDeathRetryScheduled) return
                if (!entry.renderThreadExited) {
                    val thread = entry.renderThreadRef
                    if (thread != null && thread.isAlive) return
                    entry.renderThreadExited = true
                }
                if (entry.bridge == null) {
                    entry.running = false
                    entry.renderThreadExited = false
                    return
                }
            }

            // Phase 2 (UNLOCKED): the join below may block up to
            // THREAD_JOIN_TIMEOUT_MS when the render thread is stuck in native
            // GPU code — exactly the case that would stall every session
            // operation if sessionLock were held. It is deliberately outside the
            // lock.
            stopDeadRenderThreadResources(entry)

            synchronized(sessionLock) {
                // running stays true after the join above (it is the session
                // intent flag; only pause/stop set it false). Re-check it here so
                // a concurrent pauseRendering() — which ran between Phase 1 and
                // now — cancels the restart instead of starting a render thread
                // on a destroyed surface.
                if (!entry.running) return
                if (entry.bridge == null || !sessions.containsKey(entry.id)) return
                if (entry.restartScheduled) return
                entry.restartScheduled = true
                entry.restartAttempts++
            }
            // closeDeadSession runs UNLOCKED: it closes the bridge (Session::drop
            // kills and joins threads, ~100ms+) and may start a replacement
            // render thread (join up to THREAD_JOIN_TIMEOUT_MS). Both must never
            // run while holding sessionLock. The attempt counter is monotonic
            // and closeDeadSession re-checks sessions.containsKey, so a racing
            // concurrent caller is harmless (second call returns immediately).
            if (shouldCloseDeadRender(entry.restartAttempts, RENDER_MAX_RESTART_ATTEMPTS)) {
                closeDeadSession(entry)
                return
            }
            val d =
                synchronized(sessionLock) {
                    val next = entry.nextRestartDelayMs
                    entry.nextRestartDelayMs =
                        nextRestartDelayMs(entry.nextRestartDelayMs, MAX_RESTART_DELAY_MS)
                    next
                }

            delay(d)
            restartRenderThreadAfterDelay(entry)
            delay(GRACE_PERIOD_AFTER_RESTART_MS)
            confirmRestartGrace(entry)
        }

        internal fun stopDeadRenderThreadResources(entry: SessionEntry) {
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            // NOTE: do NOT set entry.running = false here. running is the
            // session-intent flag (pause/stop set it false; start/resume set it
            // true). The dead-thread restart path relies on running staying true
            // so the delayed restart (restartRenderThreadAfterDelay) can still
            // run — and so a concurrent pauseRendering() (which sets running =
            // false) correctly cancels the pending restart.
            entry.renderThreadRef?.let { t ->
                t.interrupt()
                t.join(THREAD_JOIN_TIMEOUT_MS)
                if (t.isAlive) {
                    LogUtil.w("Runtime", "Render thread did not exit within timeout, continuing anyway")
                    entry.renderThreadPossiblyAlive = true
                    entry.hungRenderThread = t
                    return
                }
            }
            entry.renderThreadRef = null
            entry.renderSignaled.set(false)
            // A renderThreadRef join that SUCCEEDED does not prove the recorded
            // hung thread (from an earlier join timeout) is dead too — it may
            // still be inside native render code. Only clear the flag when no
            // hung thread is recorded, or give the hung thread one final join
            // first; unconditional clearing would let close paths destroy the
            // native session under a still-alive thread (use-after-free).
            val hung = entry.hungRenderThread
            if (entry.renderThreadPossiblyAlive && hung != null && hung.isAlive) {
                hung.interrupt()
                hung.join(THREAD_JOIN_TIMEOUT_MS)
            }
            if (entry.renderThreadPossiblyAlive && hung != null && !hung.isAlive) {
                entry.renderThreadPossiblyAlive = false
                entry.hungRenderThread = null
            }
            // Skip releaseGpuSurface here — the new render thread will
            // reconfigure the surface via attachSurface.
        }

        internal suspend fun restartRenderThreadAfterDelay(entry: SessionEntry) {
            synchronized(sessionLock) {
                // Consume the scheduling marker first: whether this restart runs
                // or is cancelled (paused/closed), a later monitor dispatch must
                // be able to schedule a fresh restart.
                entry.restartScheduled = false
                if (!sessions.containsKey(entry.id)) return
                // Only restart when the session still intends to run. A paused
                // session (surface destroyed) must not get a render thread — the
                // resume path starts it when the surface returns.
                if (!entry.running) return
                // Another thread may already have been started (e.g. resume
                // racing the delayed restart); do not start a second one.
                if (entry.renderThreadRef?.isAlive == true) return
                if (entry.bridge == null) return
                entry.renderThreadExited = false
                startRenderThread(entry)
                LogUtil.d(
                    "Runtime",
                    "session ${entry.id} render thread restarted (attempt ${entry.restartAttempts})",
                )
            }
        }

        internal suspend fun confirmRestartGrace(entry: SessionEntry) {
            synchronized(sessionLock) {
                if (!sessions.containsKey(entry.id)) return
                if (!entry.running) return
                if (entry.renderThreadExited) return
                val thread = entry.renderThreadRef
                if (thread != null && thread.isAlive) {
                    entry.restartAttempts = 0
                    entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                    LogUtil.d("Runtime", "session ${entry.id} render thread healthy after restart")
                }
            }
        }

        internal fun startRenderThread(entry: SessionEntry) {
            // Refuse to start a thread for a session that is being closed:
            // closeSession sets entry.closing under sessionLock before it
            // starts the unlocked stop/close sequence, and every caller of
            // this function holds sessionLock — so a closing entry can never
            // get a fresh render thread (orphaned-thread TOCTOU, see the
            // closing field doc). Also reset the running intent flag: callers
            // (resumeRendering/switchSession/closeSession Phase 3) set it
            // true BEFORE calling, and leaving it true would make
            // closeSession's locked re-check misclassify this entry as
            // running and skip bridge.close (native session + child process
            // leak).
            if (entry.closing) {
                entry.running = false
                LogUtil.d("Runtime", "session ${entry.id} closing — refusing to start render thread")
                return
            }
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            entry.renderThreadExited = false
            entry.running = false
            entry.renderSignaled.set(false)
            val oldThread = entry.renderThreadRef
            entry.renderThreadRef = null
            oldThread?.let { t ->
                if (t === Thread.currentThread()) {
                    // Self-join would time out and wrongly mark the current
                    // thread as hung; skip (the caller is the render thread
                    // itself, e.g. handleSessionExit replacement).
                    LogUtil.w(
                        "Runtime",
                        "session ${entry.id} startRenderThread called from its own render thread — skipping join",
                    )
                } else {
                    t.interrupt()
                    t.join(THREAD_JOIN_TIMEOUT_MS)
                    if (t.isAlive) {
                        LogUtil.w(
                            "Runtime",
                            "session ${entry.id} previous render thread still alive after join — forcing new thread anyway",
                        )
                        // The old thread may still be inside native render code.
                        // Record that so close paths skip releaseGpuSurface/close,
                        // and keep the reference for one final join at exit time.
                        entry.renderThreadPossiblyAlive = true
                        entry.hungRenderThread = t
                    } else {
                        // Clear the flag only when the joined thread IS the recorded
                        // hung thread (or none is recorded). A different hung thread
                        // from an earlier GPU stall may still be alive inside native
                        // code; clearing here would let close paths destroy the
                        // session under it (use-after-free).
                        if (entry.hungRenderThread == null || entry.hungRenderThread === t) {
                            entry.renderThreadPossiblyAlive = false
                            entry.hungRenderThread = null
                        }
                    }
                }
            }
            val generation = renderGeneration.incrementAndGet()
            entry.running = true
            val renderThread =
                Thread(
                    {
                        // Display-priority render thread: the frame pipeline
                        // competes with the UI thread for CPU when the IME is
                        // open or surfaces churn. THREAD_PRIORITY_DISPLAY puts
                        // frame production ahead of the UI thread's less
                        // time-critical work, cutting frame-time jitter on
                        // real devices (and SwiftShader emulators).
                        Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY)
                        var diagCount = 0
                        var consecutiveErrors = 0
                        var lastScrollOffset = Int.MAX_VALUE
                        var lastSelection = SelectionStateSnapshot(0, 0, 0, 0, false, 0)
                        // Per-thread frame-duration statistics: reset whenever the
                        // render thread restarts (fresh lifetime, no stale history).
                        val frameTiming = FrameTimingStats()
                        // Whole-loop period statistics (render + pollAll + event
                        // dispatch + waitOutput): `frameTiming` above covers only
                        // bridge.render(); the loop window's inverse is the actual
                        // frame rate and separates "native render is slow" from
                        // "something else in the loop is slow".
                        val loopTiming = FrameTimingStats()
                        // Baseline-adaptive degradation detector: learns the
                        // device's own frame-time baseline and alerts on windows
                        // that regress ~3x above it — works on the software
                        // emulator (~555ms/frame) and on real devices (~17ms)
                        // with one mechanism instead of fixed thresholds.
                        val frameTimingTrend = FrameTimingTrend()
                        LogUtil.d(
                            "Runtime",
                            "render thread started for session ${entry.id} generation=$generation",
                        )
                        while (entry.running && renderGeneration.get() == generation) {
                            // user pressed Enter on the
                            // [Process completed] prompt — writeToPty only
                            // signals; the close path runs here on the render
                            // thread so the bridge is not destroyed under a
                            // live render loop.
                            if (entry.processCompletedConfirmed) {
                                handleSessionExit(entry, entry.processExitCode, 0L)
                                break
                            }
                            try {
                                val loopFrameStart = System.nanoTime()
                                val bridge = entry.bridge ?: break
                                // ── P2-1 vsync wake gate (design D3) ──────────
                                // The loop renders ONLY when a wake source fired:
                                //   ① vsyncRequested — Choreographer frame callback
                                //     (the display-frame signal; the callback is
                                //     signal-only and never touches the surface
                                //     Mutex or calls render itself),
                                //   ② forceRenderRequested — immediate-feedback
                                //     bypass (forceRender(); semantics unchanged),
                                //   ③ renderSignaled — any notifyRender() producer
                                //     (scroll/selection/PTY-adjacent UI signals).
                                // Otherwise it parks on the output latch. The latch
                                // timeout (active 16ms / idle 500ms) doubles as the
                                // safety-net cadence (M-10): a timeout return still
                                // falls through to one render attempt so deferred-
                                // field consumption never stalls if the main thread
                                // blocks or a vsync signal is lost. PTY output note:
                                // waitOutput is a pure park — PTY arrival cannot wake
                                // it early (pre-existing behavior); new output is
                                // picked up on the next vsync attempt (~16.7ms) or
                                // by this timeout fallback, where receive_cell_data
                                // consumes the pending data. After waking, render()
                                // is called UNCONDITIONALLY — dirty/blink phase
                                // decisions live exclusively inside the native idle
                                // gate (an outer dirty short-circuit would freeze
                                // cursor blink: no JNI set-point exists for it).
                                if (
                                    !entry.vsyncRequested &&
                                    !entry.forceRenderRequested &&
                                    !entry.renderSignaled.get()
                                ) {
                                    val idleNanos = System.nanoTime() - entry.lastSignalNanos
                                    val timeoutNanos =
                                        if (idleNanos > RENDER_IDLE_THRESHOLD_NANOS) {
                                            RENDER_LATCH_IDLE_TIMEOUT_NANOS
                                        } else {
                                            RENDER_LATCH_TIMEOUT_NANOS
                                        }
                                    bridge.waitOutput(timeoutNanos / 1_000_000L)
                                    if (Thread.interrupted()) throw InterruptedException()
                                }
                                // Consume the wake flags before rendering. ALL THREE are
                                // cleared unconditionally: a signal landing between the
                                // gate check and this clear folds into the render below;
                                // one landing mid-render waits for the next vsync/timeout
                                // tick — at most one frame of extra latency, never a lost
                                // wake-up. Clearing renderSignaled here (not inside the
                                // park branch) fixes a busy-spin leak: when vsyncRequested
                                // won the gate check, renderSignaled stayed true forever,
                                // every subsequent iteration skipped the park, and the
                                // thread spun at full speed burning CPU between renders.
                                entry.vsyncRequested = false
                                entry.forceRenderRequested = false
                                entry.renderSignaled.set(false)
                                val selectionSnapshot = selectionState.get()
                                if (selectionSnapshot != lastSelection) {
                                    bridge.setSelection(
                                        selectionSnapshot.startRow,
                                        selectionSnapshot.startCol,
                                        selectionSnapshot.endRow,
                                        selectionSnapshot.endCol,
                                        selectionSnapshot.hasSelection,
                                        selectionSnapshot.mode,
                                        selectionBgColor,
                                    )
                                    lastSelection = selectionSnapshot
                                }
                                val currentScrollOffset = entry.scrollOffset
                                if (currentScrollOffset != lastScrollOffset) {
                                    bridge.setScrollOffset(currentScrollOffset)
                                    lastScrollOffset = currentScrollOffset
                                }
                                entry.lastRenderStart = System.nanoTime()
                                // Combined render + consumeNewOutput in a single JNI
                                // crossing (~0.1-0.3ms saved per frame).
                                val (count, newOutput) = bridge.renderWithNewOutput()
                                val frameMs = (System.nanoTime() - entry.lastRenderStart) / 1_000_000.0
                                if (frameMs > SLOW_FRAME_LOG_THRESHOLD_MS) {
                                    LogUtil.w(
                                        "Runtime",
                                        "SLOW_FRAME session=${entry.id} render=$frameMs count=$count newOutput=$newOutput scrollOffset=$currentScrollOffset",
                                    )
                                }
                                if (newOutput) {
                                    // Latency probe echo pairing: this frame
                                    // consumed PTY output; if an input stamp is
                                    // pending, one input→echo sample lands.
                                    entry.latencyProbe
                                        .onEchoFrame(
                                            SystemClock.elapsedRealtimeNanos(),
                                        )
                                        ?.let { latencyNanos ->
                                            LogUtil.d(
                                                "Runtime",
                                                "latency session=${entry.id} echo=${latencyNanos / 1_000_000.0}ms",
                                            )
                                            // Periodic p50/p95 summary into logcat
                                            // (LATENCY_REPORT marker is grep-stable
                                            // for offline percentile collection).
                                            val n = entry.latencyProbe.sampleCount
                                            if (n % LATENCY_REPORT_EVERY == 0) {
                                                LogUtil.i(
                                                    "Runtime",
                                                    "LATENCY_REPORT session=${entry.id} ${entry.latencyProbe.report()}",
                                                )
                                            }
                                        }
                                }
                                if (count > 0) {
                                    // New content was actually rendered. Refresh
                                    // the idle clock here: previously only UI
                                    // interactions updated lastSignalNanos, so a
                                    // sustained PTY output stream (tail -f, ping,
                                    // gradle, ...) with no interaction let
                                    // idleNanos grow past RENDER_IDLE_THRESHOLD
                                    // after ~5s and the loop fell into the 500ms
                                    // idle latch — rendering dropped to ~2 FPS
                                    // while the terminal was actively printing.
                                    entry.lastSignalNanos = System.nanoTime()
                                    // P1-1 scroll semantics (termux onScreenUpdated
                                    // parity): consume the native new_output flag
                                    // (PTY ingest → bypass flag, NOT the render()
                                    // count which also counts cursor-blink repaints)
                                    // and reset the viewport to the bottom only when
                                    // no selection/drag is active, the SCROLL lock is
                                    // off, and no scroll gesture happened within
                                    // RECENT_SCROLL_WINDOW_NANOS. Skipped resets are
                                    // dropped ("skip = give up"): the flag is already
                                    // read-clear; the next output arrival resets
                                    // again (termux: sustained output always wins).
                                    if (
                                        shouldResetScroll(
                                            scrollActive = entry.scrollActive,
                                            hasSelectionOrDrag =
                                            selectionSnapshot.hasSelection || selectionSnapshot.dragging,
                                            newOutput = newOutput,
                                            recentlyScrolled =
                                            System.nanoTime() - entry.lastScrollNanos <
                                                RECENT_SCROLL_WINDOW_NANOS,
                                        )
                                    ) {
                                        // Single-point reset write on the render
                                        // thread; lastScrollOffset stays untouched so
                                        // the existing diff-push forwards offset 0 to
                                        // native on the next frame.
                                        entry.scrollOffset = 0
                                    }
                                }
                                if (count < 0) {
                                    // Transient render error (surface not ready, snapshot unavailable, etc.)
                                    // These resolve on their own; don't count them toward the fatal limit.
                                    if (consecutiveErrors == 0) {
                                        LogUtil.w(
                                            "Runtime",
                                            "session ${entry.id} transient render error code=$count",
                                        )
                                    }
                                    consecutiveErrors++
                                    if (consecutiveErrors > RENDER_MAX_TRANSIENT_ERRORS) {
                                        LogUtil.e(
                                            "Runtime",
                                            "session ${entry.id} too many transient render errors ($consecutiveErrors), stopping render thread",
                                        )
                                        break
                                    }
                                    // Adaptive backoff: 50ms for first 10, then 200ms
                                    val sleepMs =
                                        if (consecutiveErrors > 10) {
                                            RENDER_ERROR_BACKOFF_MS
                                        } else {
                                            RENDER_ERROR_SLEEP_MS
                                        }
                                    Thread.sleep(sleepMs)
                                } else {
                                    if (consecutiveErrors > 0) {
                                        LogUtil.i(
                                            "Runtime",
                                            "session ${entry.id} recovered after $consecutiveErrors errors",
                                        )
                                    }
                                    consecutiveErrors = 0
                                    try {
                                        val poll = bridge.pollAll()
                                        // Exit is handled FIRST, in its own branch:
                                        // the event was already consumed from the
                                        // native queue and cannot be replayed, so an
                                        // exception in bell/notification/clipboard
                                        // handling below must never skip the cleanup.
                                        if (poll.exit) {
                                            // Reply empty FIRST, before any cleanup:
                                            // these MCP request events were already
                                            // consumed from the native queue and can
                                            // never be dispatched, so leaving them
                                            // unanswered would hang the MCP tool call
                                            // for 300s. Answering before
                                            // handleSessionExit (which may close the
                                            // bridge, ~100ms+) also minimizes the
                                            // MCP-side latency. Each reply is guarded
                                            // individually: a JNI failure here must
                                            // NEVER skip the session cleanup below
                                            // (the exit event is consumed and cannot
                                            // be replayed — leaking the entry, the
                                            // native session and the zombie child).
                                            try {
                                                poll.dialogs.forEach { request ->
                                                    NativeBridge.dialogResult(
                                                        request.sessionId,
                                                        request.requestId,
                                                        "",
                                                    )
                                                }
                                                poll.dialogCancels.forEach { (sessionId, requestId) ->
                                                    dialogCancelHandler?.invoke(sessionId, requestId)
                                                }
                                                poll.pickFiles.forEach { request ->
                                                    NativeBridge.dialogResult(
                                                        request.sessionId,
                                                        request.requestId,
                                                        "",
                                                    )
                                                }
                                                dispatchClipboardRequests(poll.clipboardReads)
                                                dispatchClipboardRequests(poll.clipboardGets)
                                                poll.screenshots.forEach { request ->
                                                    NativeBridge.screenshotResult(
                                                        request.sessionId,
                                                        request.requestId,
                                                        0,
                                                        0,
                                                        ByteArray(0),
                                                    )
                                                }
                                                poll.runCommands.forEach { request ->
                                                    // The command can never be dispatched
                                                    // now (the shell is gone); reply with
                                                    // an error payload instead of leaving
                                                    // the native oneshot hanging for 300s.
                                                    NativeBridge.runCommandResult(
                                                        request.sessionId,
                                                        request.requestId,
                                                        runCommandPayload(
                                                            exitCode = -1,
                                                            errCode = ERR_CODE_EXCEPTION,
                                                            stdout = "",
                                                            stderr = "session exited before run_command completed",
                                                        ),
                                                    )
                                                }
                                            } catch (replyException: Exception) {
                                                LogUtil.e(
                                                    "Runtime",
                                                    "exit branch: MCP empty-reply failed, continuing cleanup",
                                                    replyException,
                                                )
                                            }
                                            if (poll.sessionId != 0L && poll.sessionId != entry.id) {
                                                // A background (non-active) session's
                                                // shell exited. Its render thread is
                                                // stopped, so nobody else would ever
                                                // reap it — the native sweep reports
                                                // it once through this queue. Close it
                                                // here (handleSessionExit is safe for
                                                // non-active sessions: the replacement
                                                // branch is gated on entry.id ==
                                                // activeSessionId).
                                                val exitedEntry =
                                                    synchronized(sessionLock) { sessions[poll.sessionId] }
                                                if (exitedEntry != null) {
                                                    LogUtil.i(
                                                        "Runtime",
                                                        "reaping background session ${poll.sessionId} (exit ${poll.exitCode})",
                                                    )
                                                    handleSessionExit(exitedEntry, poll.exitCode, poll.exitAliveMs)
                                                }
                                            } else {
                                                // Full cleanup (bridge close, session removal,
                                                // state update) happens here; the render monitor
                                                // skips !running entries so it would never reap
                                                // an exited session.
                                                handleSessionExit(entry, poll.exitCode, poll.exitAliveMs)
                                            }
                                            // Shared by both branches: reap any
                                            // ADDITIONAL sessions that exited in the
                                            // same frame (the first one was handled
                                            // above). Their native exit_reported
                                            // flags are already set and never re-sent.
                                            // Only poll.sessionId is excluded — every
                                            // OTHER id in the list (including entry.id
                                            // when this is the background branch) must
                                            // be reaped or the Kotlin entry, native
                                            // session and zombie child leak forever.
                                            // handleSessionExit is idempotent
                                            // (containsKey re-check).
                                            poll.exits.forEach { exitInfo ->
                                                if (exitInfo.sessionId != poll.sessionId) {
                                                    val extra =
                                                        synchronized(sessionLock) { sessions[exitInfo.sessionId] }
                                                    if (extra != null) {
                                                        LogUtil.i(
                                                            "Runtime",
                                                            "reaping same-frame exited session ${exitInfo.sessionId} (exit ${exitInfo.exitCode})",
                                                        )
                                                        handleSessionExit(
                                                            extra,
                                                            exitInfo.exitCode,
                                                            exitInfo.exitAliveMs,
                                                        )
                                                    }
                                                }
                                            }
                                            if (!entry.waitingForProcessCompleted) {
                                                break
                                            }
                                            // the [Process completed]
                                            // prompt is showing — keep the session
                                            // visible (running stays true) until the
                                            // user presses Enter. Native exit_reported
                                            // is set so no further exit events arrive.
                                        }
                                        eventDispatcher.handle(poll)
                                    } catch (exception: Exception) {
                                        LogUtil.e(
                                            "Runtime",
                                            "pollAll failed for session ${entry.id}; deferred events dropped",
                                            exception,
                                        )
                                    }
                                    diagCount++
                                    if (diagCount == 1) {
                                        LogUtil.d("Runtime", "session ${entry.id} first render OK")
                                    }
                                    if (diagCount % RENDER_DIAGNOSTIC_FREQUENCY == 0) {
                                        val title =
                                            try {
                                                bridge.getActiveSessionTitle()
                                            } catch (exception: Exception) {
                                                LogUtil.e("Runtime", "title query failed", exception)
                                                ""
                                            }
                                        if (title.isNotEmpty() && title != _state.value.title) {
                                            // CAS update: the collector and the IO
                                            // session functions also write _state; a
                                            // non-atomic read-modify-write here could
                                            // clobber their session list.
                                            _state.update { current -> current.copy(title = title) }
                                        }
                                    }
                                    entry.lastRenderDone = System.nanoTime()
                                    frameTiming.record(entry.lastRenderDone - entry.lastRenderStart)
                                    frameTiming.takeReport()?.let { report ->
                                        // Memory gauge alongside the timing window: a
                                        // monotonically growing scrollback row count
                                        // across windows indicates unbounded history.
                                        val scrollbackRows =
                                            try {
                                                NativeBridge.getScrollbackRows(entry.id)
                                            } catch (exception: Exception) {
                                                LogUtil.w("Runtime", "scrollback query failed", exception)
                                                -1
                                            }
                                        val summary =
                                            "session ${entry.id} frame timing window (${report.frameCount} frames): " +
                                                "avg=${report.averageNanos / 1_000_000L}ms " +
                                                "p95=${report.p95Nanos / 1_000_000L}ms " +
                                                "max=${report.maxNanos / 1_000_000L}ms " +
                                                "scrollback=$scrollbackRows rows"
                                        val trendDegraded = frameTimingTrend.observe(report.averageNanos)
                                        when {
                                            // Absolute pathology: a stall beyond any
                                            // device's expectation (emulator baseline
                                            // ~555ms/frame; real devices ~17ms).
                                            report.p95Nanos >= FRAME_TIME_WARN_P95_NANOS ||
                                                report.maxNanos >= FRAME_TIME_WARN_MAX_NANOS ->
                                                LogUtil.w(
                                                    "Runtime",
                                                    "$summary — severe stall(s), investigate render cost",
                                                )

                                            // Baseline-relative regression (~3x the
                                            // device's own learned baseline, at least
                                            // 100ms average): catches gradual and
                                            // device-specific degradations that an
                                            // absolute threshold cannot.
                                            trendDegraded ->
                                                LogUtil.w(
                                                    "Runtime",
                                                    "$summary — degraded vs baseline (${frameTimingTrend.currentBaselineNanos()?.div(1_000_000L)}ms), investigate render cost",
                                                )

                                            // Normal window: Info (not Debug) so the
                                            // gauge survives release builds — LogUtil.d
                                            // is gated on BuildConfig.DEBUG and would
                                            // hide every window on a release APK,
                                            // leaving gradual issues invisible.
                                            // One line per 60 rendered frames (~1s on
                                            // a real device, ~33s on the emulator) is
                                            // a quiet but always-present signal.
                                            else -> LogUtil.i("Runtime", summary)
                                        }
                                    }
                                    // accessibility hook — the render loop
                                    // is the only signal that terminal content
                                    // changed, and the SurfaceView has no text nodes.
                                    // The listener runs on the render thread and must
                                    // return quickly (it may post to the main thread).
                                    try {
                                        onFrameRendered?.invoke()
                                    } catch (exception: Exception) {
                                        LogUtil.w("Runtime", "onFrameRendered callback failed", exception)
                                    }
                                    // P2-1: tail wait removed — parking now happens in
                                    // the loop-top wake gate above (same latch, same
                                    // active/idle timeouts). Falling through here goes
                                    // straight back to the gate.
                                }
                                // Whole-loop period: the inverse of the average is
                                // the ACTUAL frame rate (waitOutput + pollAll +
                                // event dispatch included). If render avg is ~16ms
                                // but this is ~50ms, the frame time goes elsewhere
                                // in the loop, not the native render path.
                                loopTiming.record(System.nanoTime() - loopFrameStart)
                                loopTiming.takeReport()?.let { loopReport ->
                                    val loopAvgMs = loopReport.averageNanos / 1_000_000L
                                    val fps = if (loopAvgMs > 0) 1_000L / loopAvgMs else 0L
                                    LogUtil.i(
                                        "Runtime",
                                        "session ${entry.id} loop timing window (${loopReport.frameCount} frames): " +
                                            "avg=${loopAvgMs}ms p95=${loopReport.p95Nanos / 1_000_000L}ms " +
                                            "max=${loopReport.maxNanos / 1_000_000L}ms ≈${fps}fps",
                                    )
                                }
                            } catch (exception: InterruptedException) {
                                // The render thread was interrupted during shutdown
                                // (session switch / runtime stop). This is an expected
                                // signal, not a render failure — exit the loop cleanly.
                                Thread.currentThread().interrupt()
                                break
                            } catch (exception: Exception) {
                                consecutiveErrors++
                                if (consecutiveErrors == 1) {
                                    LogUtil.e(
                                        "Runtime",
                                        "session ${entry.id} first render exception",
                                        exception,
                                    )
                                } else if (consecutiveErrors % RENDER_ERROR_LOG_FREQUENCY == 0) {
                                    LogUtil.e(
                                        "Runtime",
                                        "session ${entry.id} render exception (x$consecutiveErrors)",
                                        exception,
                                    )
                                }
                                if (consecutiveErrors > RENDER_MAX_CONSECUTIVE_ERRORS) {
                                    LogUtil.e(
                                        "Runtime",
                                        "session ${entry.id} too many render exceptions ($consecutiveErrors), stopping render thread",
                                        exception,
                                    )
                                    break
                                }
                                Thread.sleep(RENDER_ERROR_SLEEP_MS)
                            }
                        }
                        entry.renderThreadExited = true
                        LogUtil.d("Runtime", "render thread stopped for session ${entry.id}")
                    },
                    "Render-${entry.id}",
                )
                    .apply {
                        isDaemon = true
                    }
            entry.renderThreadRef = renderThread
            renderThread.start()
            // P2-1: drive the new render loop's wake gate from display vsync
            // (one self-rescheduling Choreographer chain per process).
            ensureVsyncChainStarted()
            entry.renderWatchDog =
                RenderWatchDog(
                    getStart = { entry.lastRenderStart },
                    getDone = { entry.lastRenderDone },
                    isRunning = {
                        entry.running && !entry.renderThreadExited && activeSessionId == entry.id
                    },
                    onHangDetected = {
                        LogUtil.e(
                            "Runtime",
                            "session ${entry.id} render thread hung (>${RENDER_HANG_TIMEOUT_NANOS / 1_000_000L}s) — marking thread for restart",
                        )
                        // Mark the thread as dead. The render monitor (checkSessions)
                        // will detect this and restart the thread with exponential backoff.
                        // This avoids killing the entire process for a GPU hang.
                        entry.renderThreadExited = true
                    },
                    hangTimeoutNanos = RENDER_HANG_TIMEOUT_NANOS,
                )
                    .also { it.start() }
        }

        internal fun stopRenderThread(entry: SessionEntry): Boolean {
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            entry.running = false
            val thread = entry.renderThreadRef
            entry.renderThreadRef = null
            entry.renderSignaled.set(false)
            thread?.let { t ->
                t.interrupt()
                t.join(THREAD_JOIN_TIMEOUT_MS)
                if (t.isAlive) {
                    LogUtil.e(
                        "Runtime",
                        "session ${entry.id} render thread still alive after join — possibly hung",
                    )
                    entry.renderThreadPossiblyAlive = true
                    entry.hungRenderThread = t
                    return false
                }
            }
            // Clear the possibly-alive flag only when the joined thread IS the
            // recorded hung thread (or none is recorded); a different hung
            // thread may still be alive inside native code, and clearing here
            // would let close paths destroy the native session under it.
            if (entry.hungRenderThread == null || entry.hungRenderThread === thread) {
                entry.renderThreadPossiblyAlive = false
                entry.hungRenderThread = null
            }
            return true
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4b: Event dispatch (extracted K3)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Dispatches non-exit poll events (bell, notification, clipboard, MCP dialogs/pick-file, toast,
     * open-url, clipboard_get replies).
     *
     * Extracted from the render loop so the loop body stays a tight poll → handle → wait cycle. Inner
     * class: accesses TerminalRuntime's handlers/context/clipboard without threading them through
     * constructors. Exit reaping stays in the loop it owns the break/cleanup control flow).
     */
    inner class EventDispatcher {
        /**
         * Handle all non-exit events in [poll]. Called from the render loop after exit handling.
         * Exceptions here must never skip the loop's per-frame bookkeeping (caller wraps us in the
         * outer try).
         */
        fun announceAccessibility(content: String) {
            // termlib AccessibilityOverlay live-region pattern: announce
            // via the platform accessibility manager so TalkBack reads
            // events (bell, notifications) even though the terminal is a
            // self-drawn SurfaceView with no text nodes.
            try {
                val am =
                    context.getSystemService(android.content.Context.ACCESSIBILITY_SERVICE)
                        as? android.view.accessibility.AccessibilityManager
                if (am != null && am.isEnabled) {
                    // Announcement events are deprecated without a modern
                    // replacement (system accessibility broadcast); the
                    // constructor form avoids the deprecated obtain() API.
                    @Suppress("DEPRECATION")
                    val event =
                        android.view.accessibility.AccessibilityEvent(
                            android.view.accessibility.AccessibilityEvent.TYPE_ANNOUNCEMENT,
                        )
                    event.text.add(content)
                    am.sendAccessibilityEvent(event)
                }
            } catch (exception: Exception) {
                LogUtil.w("Runtime", "announceAccessibility failed", exception)
            }
        }

        /**
         * Bell-flash overlay animation, BellMode.SCREEN_FLASH): animate the native flash phase 1.0 →
         * 0.0 over BELL_FLASH_DURATION_MS. A fresh bell cancels and restarts the animation so a burst
         * always re-peaks at phase 1.0 (BellHandler.debounce already coalesces sub-150ms bells before
         * we get here).
         */
        private fun triggerBellFlash() {
            if (bellHandler.currentMode.value != BellMode.SCREEN_FLASH) return
            bellFlashJob?.cancel()
            bellFlashJob = scope.launch {
                val bridge = bridge() ?: return@launch
                var remainingMs = BELL_FLASH_DURATION_MS
                while (remainingMs > 0 && isActive) {
                    bridge.setFlashState(remainingMs.toFloat() / BELL_FLASH_DURATION_MS)
                    delay(BELL_FLASH_TICK_MS)
                    remainingMs -= BELL_FLASH_TICK_MS
                }
                bridge.setFlashState(0f)
            }
        }

        fun handle(poll: terminal.emulator.bridge.Bridge.PollResult) {
            if (poll.bel) {
                bellHandler.fireBell(onAccessibility = { announceAccessibility(it) })
                triggerBellFlash()
            }
            if (poll.notification != null) {
                val (title, body) = poll.notification
                val toastText = if (title.isNotEmpty()) "$title: $body" else body
                Handler(Looper.getMainLooper()).post {
                    android.widget.Toast.makeText(context, toastText, android.widget.Toast.LENGTH_LONG).show()
                }
                terminal.emulator.ui.TerminalNotificationHelper(context).showNotification(title, body)
                announceAccessibility(if (title.isNotEmpty()) title else body)
            }
            // ConEmu progress (OSC 9;4). Log the event;
            // a progress bar UI can be added later if needed.
            poll.progress?.let { (state, value) ->
                LogUtil.d("Runtime", "OSC 9;4 progress: state=$state value=$value")
            }
            if (poll.clipboard != null) {
                clipboardAccess.setClipboardText(poll.clipboard)
            }
            poll.dialogs.forEach { request -> dispatchDialogRequest(request) }
            poll.dialogCancels.forEach { (sessionId, requestId) ->
                dialogCancelHandler?.invoke(sessionId, requestId)
            }
            poll.pickFiles.forEach { request -> dispatchPickFileRequest(request) }
            poll.toastText?.let { text ->
                Handler(Looper.getMainLooper()).post {
                    android.widget.Toast.makeText(context, text, android.widget.Toast.LENGTH_SHORT).show()
                }
            }
            poll.openUrl?.let { url ->
                try {
                    val intent =
                        android.content.Intent(
                            android.content.Intent.ACTION_VIEW,
                            url.toUri(),
                        )
                    intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
                    context.startActivity(intent)
                } catch (exception: Exception) {
                    // Never log the URL or the exception
                    // stack: both may carry token/query
                    // parameters and LogUtil writes the
                    // persistent log file unconditionally
                    //
                    LogUtil.e("Runtime", "open_url failed: ${exception.javaClass.simpleName}")
                }
            }
            dispatchClipboardRequests(poll.clipboardReads)
            dispatchClipboardRequests(poll.clipboardGets)
            poll.runCommands.forEach { request ->
                // run_command may take up to 30 s; run it on the IO scope
                // so the poll loop keeps servicing keyboard / clipboard /
                // signal requests meanwhile).
                scope.launch { dispatchRunCommandRequest(request) }
            }
            poll.screenshots.forEach { request ->
                dispatchScreenshotRequest(request)
            }
        }
    }

    /**
     * Dispatch one MCP `screenshot` request. Captures the current terminal frame as RGBA pixels via
     * GPU readback (render_to_buffer) and returns the result to the native MCP tool via
     * [NativeBridge.screenshotResult].
     *
     * Must run on the render thread (which owns the wgpu context). The render thread's event loop
     * calls this directly.
     */
    private fun dispatchScreenshotRequest(
        request: terminal.emulator.bridge.Bridge.ScreenshotRequest,
    ) {
        try {
            val data = NativeBridge.captureFrame(request.sessionId)
            if (data == null || data.size < 8) {
                LogUtil.w("Runtime", "screenshot: captureFrame returned null or insufficient data")
                NativeBridge.screenshotResult(request.sessionId, request.requestId, 0, 0, ByteArray(0))
                return
            }
            // First 8 bytes: width (u32 LE) + height (u32 LE)
            val width =
                (
                    (data[0].toInt() and 0xFF) or
                        ((data[1].toInt() and 0xFF) shl 8) or
                        ((data[2].toInt() and 0xFF) shl 16) or
                        ((data[3].toInt() and 0xFF) shl 24)
                    )
            val height =
                (
                    (data[4].toInt() and 0xFF) or
                        ((data[5].toInt() and 0xFF) shl 8) or
                        ((data[6].toInt() and 0xFF) shl 16) or
                        ((data[7].toInt() and 0xFF) shl 24)
                    )
            if (width <= 0 || height <= 0) {
                LogUtil.w("Runtime", "screenshot: invalid dimensions ${width}x$height")
                NativeBridge.screenshotResult(request.sessionId, request.requestId, 0, 0, ByteArray(0))
                return
            }
            val pixels = data.copyOfRange(8, data.size)
            NativeBridge.screenshotResult(request.sessionId, request.requestId, width, height, pixels)
        } catch (exception: Exception) {
            LogUtil.e("Runtime", "screenshot dispatch failed: ${exception.message}")
            NativeBridge.screenshotResult(request.sessionId, request.requestId, 0, 0, ByteArray(0))
        }
    }

    /**
     * Dispatch one MCP `run_command` request (same request/response routing as [dialogResult]). The raw command string is tokenized to argv with
     * [ArgumentTokenizer] (no shell, no metacharacter interpretation) and executed in the app's
     * process. The captured stdout/stderr and exit code are returned to the native MCP tool via
     * `NativeBridge.runCommandResult`.
     *
     * Runs on the IO scope: the poll loop launches it without awaiting so a 30 s command cannot
     * freeze keyboard / clipboard / signal polling.
     */
    private fun dispatchRunCommandRequest(
        request: terminal.emulator.bridge.Bridge.RunCommandRequest,
    ) {
        // Reply with an error payload instead of leaving the native
        // oneshot unresolved when anything below fails.
        fun fail(detail: String) {
            LogUtil.e("Runtime", "run_command request dispatch failed: $detail")
            val payload =
                runCommandPayload(
                    exitCode = -1,
                    errCode = ERR_CODE_EXCEPTION,
                    stdout = "",
                    stderr = detail,
                )
            NativeBridge.runCommandResult(request.sessionId, request.requestId, payload)
        }
        try {
            if (request.command.isBlank()) {
                fail("empty command")
                return
            }
            val argv = ArgumentTokenizer.tokenize(request.command)
            if (argv.isEmpty()) {
                fail("no argv produced")
                return
            }
            LogUtil.i("Runtime", "run_command argv=${argv.joinToString(" ")}")
            val result =
                executeRunCommand(
                    argv,
                    prefixDir = java.io.File(context.filesDir, "usr").absolutePath,
                )
            val errCode = if (result.timedOut) ERR_CODE_TIMEOUT else ERR_CODE_NONE
            val payload = runCommandPayload(result.exitCode, errCode, result.stdout, result.stderr)
            NativeBridge.runCommandResult(request.sessionId, request.requestId, payload)
        } catch (exception: Exception) {
            fail("${exception.javaClass.simpleName}: ${exception.message}")
        }
    }

    /**
     * Dispatch one MCP dialog request to the activity handler; replies empty when no handler is
     * attached (activity destroyed window) so the native tool call never hangs. Extracted from
     * handle() for the detekt CyclomaticComplexMethod limit.
     */
    private fun dispatchDialogRequest(request: terminal.emulator.bridge.Bridge.DialogRequest) {
        try {
            LogUtil.i(
                "Runtime",
                "MCP dialog request session=${request.sessionId} type=${request.dialogType}",
            )
            val handler = dialogRequestHandler
            if (handler != null) {
                handler(
                    request.sessionId,
                    request.requestId,
                    request.dialogType,
                    request.title,
                    request.message,
                    request.options,
                )
            } else {
                LogUtil.w(
                    "Runtime",
                    "MCP dialog request dropped (no handler), replying empty",
                )
                NativeBridge.dialogResult(request.sessionId, request.requestId, "")
            }
        } catch (exception: Exception) {
            LogUtil.e("Runtime", "dialog request dispatch failed", exception)
            NativeBridge.dialogResult(request.sessionId, request.requestId, "")
        }
    }

    /**
     * Dispatch one MCP pick_file request to the activity handler (same no-handler fallback as
     * [dispatchDialogRequest]).
     */
    private fun dispatchPickFileRequest(request: terminal.emulator.bridge.Bridge.PickFileRequest) {
        try {
            LogUtil.i("Runtime", "MCP pick_file request session=${request.sessionId}")
            val handler = pickFileRequestHandler
            if (handler != null) {
                handler(
                    request.sessionId,
                    request.requestId,
                    request.startingPath,
                    request.filter,
                )
            } else {
                LogUtil.w(
                    "Runtime",
                    "MCP pick_file request dropped (no handler), replying empty",
                )
                NativeBridge.dialogResult(request.sessionId, request.requestId, "")
            }
        } catch (exception: Exception) {
            LogUtil.e("Runtime", "pick_file request dispatch failed", exception)
            NativeBridge.dialogResult(request.sessionId, request.requestId, "")
        }
    }

    private companion object {
        const val DEFAULT_GRID_ROWS = 24
        const val DEFAULT_GRID_COLS = 80
        private const val TENTHS_PER_UNIT = 10

        /** Zoom-preview bounds in tenths — mirrors the native setFontSizeInPlace clamp (4.0..100.0). */
        private const val MIN_FONT_SIZE_TENTHS = 40
        private const val MAX_FONT_SIZE_TENTHS = 1000

        /** ModifierBar overlay height reserved when recomputing the grid from font metrics. */
        private const val MODIFIER_BAR_HEIGHT_DP = 80f
        private const val FONT_SIZE_DISPLAY_RATIO = 0.6f
        private const val FONT_SIZE_MIN_PX = 300
        private const val FONT_SIZE_MAX_PX = 600
        private const val FONT_SIZE_HEIGHT_RATIO = 0.5f
        private const val FONT_SIZE_HEIGHT_MIN_PX = 250
        private const val FONT_SIZE_HEIGHT_MAX_PX = 500
        private const val RENDER_ERROR_LOG_FREQUENCY = 60

        // [Process completed] prompt fed to the terminal when
        // a foreground session's shell exits (kept visible until Enter).
        private const val PROCESS_COMPLETED_PROMPT_PREFIX = "\r\n[Process completed (code "
        private const val PROCESS_COMPLETED_PROMPT_SUFFIX = ")] - press Enter"

        /** Bell-flash overlay animation (native render quad, BellMode.SCREEN_FLASH). */
        private const val BELL_FLASH_DURATION_MS = 400L
        private const val BELL_FLASH_TICK_MS = 16L

        /**
         * run_command err_code, termux ExecutionCommand dual-track): exitCode is the shell exit code;
         * errCode is the app-level failure classification. 0 = none (command ran), 1 = timeout, 2 =
         * internal exception. Destructive commands are refused by the native safety classifier before
         * they reach the host, so no blocked err_code exists on this side.
         */
        private const val ERR_CODE_NONE = 0
        private const val ERR_CODE_TIMEOUT = 1
        private const val ERR_CODE_EXCEPTION = 2

        private const val RENDER_MAX_CONSECUTIVE_ERRORS = 100
        private const val RENDER_MAX_TRANSIENT_ERRORS =
            50 // ~2.5s of transient errors before thread exit
        private const val RENDER_ERROR_SLEEP_MS = 50L
        private const val RENDER_ERROR_BACKOFF_MS =
            200L // Longer sleep after 10 consecutive transient errors

        // Logcat LATENCY_REPORT summary cadence (samples).
        private const val LATENCY_REPORT_EVERY = 50

        // 8ms active latch: halves worst-case echo wake quantization
        // (input→echo latency tail) at negligible cost — the native idle
        // gate turns extra wake-ups into ~0-cost count=0 JNI crossings.
        private const val RENDER_LATCH_TIMEOUT_NANOS = 8_000_000L

        // Slow-frame diagnostic: frames above this log a SLOW_FRAME line
        // (render-stage wall time) for offline breakdown.
        private const val SLOW_FRAME_LOG_THRESHOLD_MS = 30.0
        private const val RENDER_LATCH_IDLE_TIMEOUT_NANOS = 500_000_000L // 500ms for idle (~2 FPS)
        private const val RENDER_IDLE_THRESHOLD_NANOS = 5_000_000_000L // 5s idle → switch to low-freq
        private const val RENDER_DIAGNOSTIC_FREQUENCY = 60
        private const val THREAD_JOIN_TIMEOUT_MS = 1000L
        private const val RENDER_HANG_TIMEOUT_NANOS = 10_000_000_000L // 10 seconds

        // Frame-timing diagnostics (FrameTimingStats + FrameTimingTrend): the
        // absolute thresholds below cover stalls beyond any device's
        // expectation (measured emulator idle windows average single-digit
        // ms; a real device targets ~17ms). Gradual/device-specific
        // regressions are caught by the baseline-adaptive FrameTimingTrend
        // (~3x of the learned baseline, ≥100ms average) so a real
        // degradation surfaces in the logs on any hardware.
        private const val FRAME_TIME_WARN_P95_NANOS = 1_000_000_000L // 1s p95
        private const val FRAME_TIME_WARN_MAX_NANOS = 2_000_000_000L // 2s single frame
        private const val RENDER_INITIAL_RETRY_MAX = 5
        private const val RENDER_INITIAL_RETRY_DELAY_MS = 150L

        // Render monitor — proactive death detection
        private const val RENDER_MONITOR_INTERVAL_MS = 500L
        private const val RENDER_MAX_RESTART_ATTEMPTS = 5
        private const val INITIAL_RESTART_DELAY_MS = 100L
        private const val MAX_RESTART_DELAY_MS = 1000L
        private const val GRACE_PERIOD_AFTER_RESTART_MS = 300L
    }

    private data class ConfigReads(
        val shellPath: String,
        val scrollbackLines: Int,
        val fontSizeTenths: Int,
        val themeName: String,
        val environmentVariables: Map<String, String>,
    )

    private fun findPrefixShell(prefixDir: String): String? = listOf("bin/login", "bin/bash", "bin/zsh", "bin/fish", "bin/sh").firstOrNull { candidate ->
        val file = java.io.File("$prefixDir/$candidate")
        file.isFile && isElf(file)
    }

    internal suspend fun computeFontSizeTenths(): Int {
        val userFontSize = settingsRepository.fontSize.first()
        if (settingsRepository.fontSizeExplicitlySet.first()) {
            // fontSize is in sp (SettingsRepository default 10f). fontSizeTenths
            // is the same value in tenths of a sp (native font pipeline consumes
            // sp directly — the raster scale applies the density). Multiplying
            // by density here double-scaled the font (10sp → 225 tenths = 22.5sp)
            // and made the Settings slider disagree with the rendered size
            // "font size setting vs actual mismatch").
            return (userFontSize * TENTHS_PER_UNIT.toFloat()).toInt()
        }
        // Fresh install: derive a sensible default from the screen width
        // (single source of truth: SettingsRepository.defaultFontSizeFor) so a
        // phone (~360dp) and a tablet (~600dp) show the same column count.
        val widthDp = context.resources.configuration.screenWidthDp.toFloat()
        return (SettingsRepository.defaultFontSizeFor(widthDp) * TENTHS_PER_UNIT.toFloat()).toInt()
    }

    /**
     * ⑥ Zoom preview: push new font metrics to the native font pipeline and refresh the Kotlin cell
     * metrics WITHOUT resizing the grid. The renderer draws the next frame at the new cell size
     * (cell_builder reads the font metrics every frame), while ghostty keeps its rows/cols until the
     * gesture finalizes through [appliedFontSizeSp]/setFontSize, which runs the full apply (including
     * the grid reflow). Caller rate-limits this — it is cheap enough to run a few times per second
     * even on software-GPU emulators.
     */
    fun setFontSizePreview(sizeSp: Float) {
        val tenths = (sizeSp * TENTHS_PER_UNIT.toFloat()).toInt()
        if (tenths < MIN_FONT_SIZE_TENTHS || tenths > MAX_FONT_SIZE_TENTHS) return
        val entry = sessions[activeSessionId] ?: return
        entry.bridge?.setFontSizeInPlace(tenths)
        entry.bridge?.let { syncGridDimensions(it) }
    }

    /**
     * The font size (sp) currently rendered by the native pipeline (last value pushed via
     * setFontSizeInPlace). Falls back to the built-in default before the first application.
     */
    fun appliedFontSizeSp(): Float {
        val tenths = appliedFontSizeTenths
        return if (tenths > 0) {
            tenths / TENTHS_PER_UNIT.toFloat()
        } else {
            SettingsRepository.DEFAULT_FONT_SIZE
        }
    }

    internal suspend fun resolveThemeName(): String {
        val themeMode = settingsRepository.themeMode.first()
        val dayTheme = settingsRepository.dayThemeName.first()
        val nightTheme = settingsRepository.nightThemeName.first()
        val singleTheme = settingsRepository.themeName.first()
        val systemDark =
            (
                context.resources.configuration.uiMode and
                    android.content.res.Configuration.UI_MODE_NIGHT_MASK
                ) ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES
        val effectiveDark =
            when (settingsRepository.appThemeMode.first()) {
                "day" -> false
                "night" -> true
                else -> systemDark
            }
        return when (themeMode) {
            "day" -> dayTheme
            "night" -> nightTheme
            "fixed" -> singleTheme
            else -> if (effectiveDark) nightTheme else dayTheme
        }
    }

    private fun resolveShell(shellPath: String): Shell = if (shellPath == "/system/bin/sh" || shellPath.isEmpty()) {
        Shell.SystemDefault
    } else {
        Shell.Custom(shellPath)
    }

    private fun makeBridgeTheme(
        resolvedTheme: terminal.emulator.ui.theme.TerminalTheme,
    ): BridgeTheme {
        val backgroundColor = resolvedTheme.background.toArgb()
        val foregroundColor = resolvedTheme.foreground.toArgb()
        val cursor = resolvedTheme.cursor.toArgb()
        val ansiInts = resolvedTheme.ansi.map { it.toArgb() }
        val resolvedSelectionBg =
            if (resolvedTheme.selectionBg == Color.Transparent) {
                Color(0xFF45475A)
            } else {
                resolvedTheme.selectionBg
            }
        return BridgeTheme(
            name = resolvedTheme.name,
            bg = backgroundColor,
            fg = foregroundColor,
            cursor = cursor,
            selectionBg = resolvedSelectionBg.toArgb(),
            ansi0 = ansiInts[0],
            ansi1 = ansiInts[1],
            ansi2 = ansiInts[2],
            ansi3 = ansiInts[3],
            ansi4 = ansiInts[4],
            ansi5 = ansiInts[5],
            ansi6 = ansiInts[6],
            ansi7 = ansiInts[7],
            ansi8 = ansiInts[8],
            ansi9 = ansiInts[9],
            ansi10 = ansiInts[10],
            ansi11 = ansiInts[11],
            ansi12 = ansiInts[12],
            ansi13 = ansiInts[13],
            ansi14 = ansiInts[14],
            ansi15 = ansiInts[15],
        )
    }

    suspend fun start(
        surface: Surface?,
        width: Int,
        height: Int,
    ) {
        synchronized(sessionLock) {
            if (sessions.isNotEmpty() || starting) return
            starting = true
        }
        // LogUtil.d already logs to logcat — no duplicate write.
        LogUtil.d("Runtime", "start() called: surface=$surface width=$width height=$height")
        // ADR-0007: remember the surface handed in by startRuntime so the
        // renderer can attach it once the session bridge exists.
        if (surface != null) {
            pendingSurface = surface
            pendingSurfaceWidth = width
            pendingSurfaceHeight = height
        }
        if (!NativeBridge.isNativeLoaded()) {
            // Mirror createSession's guard. Without it, bridge.ping() throws
            // RuntimeException, the rollback's destroySession throws
            // UnsatisfiedLinkError (an Error — not caught by catch(Exception))
            // and the app crashes when the native lib is missing/ABI-mismatched.
            LogUtil.e("Runtime", "start: native library not loaded, aborting start")
            synchronized(sessionLock) {
                starting = false
            }
            return
        }
        // point the MCP socket at our real data dir (the
        // native default hardcodes /data/data/com.termux, which breaks if
        // the package is ever renamed).
        runCatching {
            NativeBridge.setMcpSocketPath(
                context.filesDir.resolve("run/mcp.sock").absolutePath,
            )
        }
        // restore the persisted MCP server toggle. The switch in
        // SettingsScreen only writes the DataStore flag; nothing replayed it
        // on startup, so the server never came back after an app restart
        // even though the setting said enabled (emulator-verified: socket
        // absent after force-stop/relaunch). Read the first value from the
        // DataStore-backed Flow (first emission requires an async disk read;
        // a failure here must not abort startup — it degrades to the
        // pre-fix behavior of a stopped server) and mirror it to the native
        // side, which is idempotent (start() no-ops when running).
        runCatching {
            val enabled = settingsRepository.mcpServerEnabled.first()
            NativeBridge.setMcpEnabled(enabled)
            LogUtil.d("Runtime", "start: restored MCP server enabled=$enabled")
        }
            .onFailure { error ->
                // Restore failure means MCP is silently unavailable — exactly the
                // bug this code fixes — so it must be visible in logcat.
                LogUtil.e("Runtime", "start: failed to restore MCP server toggle", error)
            }
        val displayW = context.resources.displayMetrics.widthPixels
        val displayH = context.resources.displayMetrics.heightPixels
        val density = context.resources.displayMetrics.density
        LogUtil.d(
            "Runtime",
            "displayMetrics: w=$displayW h=$displayH density=$density",
        )

        val bypassMinSurface = System.getProperty("test.minSurface") != null

        if (!bypassMinSurface && (width <= 0 || height <= 0)) {
            LogUtil.e(
                "Runtime",
                "start() called with non-positive dimensions, waiting for surfaceChanged",
            )
            starting = false
            return
        }

        val minWidth =
            (displayW * FONT_SIZE_DISPLAY_RATIO).toInt().coerceIn(FONT_SIZE_MIN_PX, FONT_SIZE_MAX_PX)
        val minHeight =
            (displayH * FONT_SIZE_HEIGHT_RATIO)
                .toInt()
                .coerceIn(FONT_SIZE_HEIGHT_MIN_PX, FONT_SIZE_HEIGHT_MAX_PX)
        if (!bypassMinSurface && (width < minWidth || height < minHeight)) {
            LogUtil.w(
                "Runtime",
                "start() called with small surface ${width}x$height (display=${displayW}x$displayH min=${minWidth}x$minHeight), waiting for correct surfaceChanged",
            )
            starting = false
            return
        }

        // Hoisted so the failure path can close it; stays null when
        // start() bails out before createBridge().
        var startedBridge: terminal.emulator.bridge.Bridge? = null
        if (surface != null) {
            // ADR-0007: surface integration is deferred — the native side
            // receives the Surface via attachWindow(JNI) and Kotlin never
            // hands a raw ANativeWindow pointer across the bridge; do NOT
            // abort startup when the pointer is 0, or the terminal cannot
            // start at all.
            LogUtil.d("Runtime", "surface present — render integration pending (ADR-0007)")
        } else {
            LogUtil.d("Runtime", "no surface — using GPU offscreen rendering path")
        }

        try {
            // Bootstrap is strictly opt-in: a fresh app runs the system
            // shell and downloads nothing unless the user explicitly
            // configured a bootstrap URL in settings. Auto-downloading a
            // Termux bootstrap (~150 MB) on first launch would be
            // intrusive and unbounded. Test override via system property
            // (no DataStore dependency).
            val testUrl = System.getProperty("test.bootstrapUrl")
            val bootstrapUrl = if (testUrl != null) testUrl else settingsRepository.bootstrapUrl.first()
            if (bootstrapUrl.isNotEmpty()) {
                // Log only the origin (scheme://host), never the full URL:
                // private bootstrap URLs can carry token/query parameters,
                // and LogUtil writes the persistent log file unconditionally
                //
                val origin =
                    runCatching {
                        val uri = bootstrapUrl.toUri()
                        val scheme = uri.scheme ?: return@runCatching "<no-scheme>"
                        val host = uri.host
                        if (host.isNullOrBlank()) return@runCatching "<no-host>"
                        "$scheme://$host"
                    }
                        .getOrNull() ?: "<unparsable>"
                LogUtil.d("Runtime", "Bootstrap URL set: $origin")
                val downloader = terminal.emulator.installer.BootstrapDownloader(context)
                val installer =
                    terminal.emulator.installer.BootstrapInstaller(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        stagingDir = java.io.File(context.filesDir, "usr-staging"),
                    )
                val secondStage =
                    terminal.emulator.installer.SecondStageRunner(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                    )
                val installOrchestrator =
                    terminal.emulator.installer.BootstrapOrchestrator(downloader, installer, secondStage)
                when (installOrchestrator.getInstallStatus()) {
                    terminal.emulator.installer.BootstrapOrchestrator.Status.NOT_INSTALLED -> {
                        // Never auto-download: a Termux bootstrap (~150 MB)
                        // must be installed explicitly from Settings — the
                        // app must not download or install it on its own.
                        // Log state only so a missing install stays
                        // diagnosable; the Settings bootstrap button is the
                        // single entry point.
                        LogUtil.d("Runtime", "Bootstrap not installed — install manually from Settings")
                    }

                    terminal.emulator.installer.BootstrapOrchestrator.Status.INSTALLED -> {
                        LogUtil.d("Runtime", "Bootstrap already installed")
                    }

                    else -> {}
                }
            }
            val configStartNs = System.nanoTime()
            val config = buildConfig()
            LogUtil.d(
                "Runtime",
                "buildConfig: fontSizeTenths=${config.font_size_tenths} rows=${config.rows} cols=${config.cols} theme=${config.theme.name} elapsed=${(System.nanoTime() - configStartNs) / 1_000_000}ms",
            )
            val bridgeStartNs = System.nanoTime()
            val bridge = createBridge(config)
            startedBridge = bridge
            LogUtil.d(
                "Runtime",
                "bridge created: ${bridge.ping()} elapsed=${(System.nanoTime() - bridgeStartNs) / 1_000_000}ms",
            )

            bridge.setSystemLocale(
                java.util.Locale.getDefault().toLanguageTag(),
            )
            LogUtil.d("Runtime", "setSystemLocale: ${java.util.Locale.getDefault().toLanguageTag()}")

            val fontsDir = context.filesDir.resolve("fonts")
            fontsDir.apply {
                if (!exists() && !mkdirs()) {
                    LogUtil.w("Runtime", "Failed to create fonts directory: $this")
                }
            }
            // NOTE: bridge.setExtraFontPaths is intentionally NOT called
            // here — Bridge skips it while sessionId == 0 (before
            // spawnTerminal), which silently dropped the extra font paths
            // Nerd Font in filesDir/fonts never loaded,
            // NERD_FALLBACK found 0). It is called again right after
            // spawnTerminal below.

            // The bootstrap download/install above can take minutes. The
            // surface passed into start() may have been destroyed during
            // that window (rotation, split-screen); its ANativeWindow
            // pointer is dangling. Spawning on it would render into a dead
            // window (black screen / hang). Abort and let the next
            // surface-available event retry start() with a fresh surface.
            if (surface != null && !surface.isValid) {
                LogUtil.e(
                    "Runtime",
                    "start(): surface became invalid during bootstrap, aborting (will retry on next surface)",
                )
                // The bridge (native engine) was already created above;
                // close it or every invalid-surface retry leaks one.
                // (Flow analysis guarantees startedBridge is non-null here:
                // the assignment happened earlier in this same try block.)
                try {
                    startedBridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e("Runtime", "Failed to close bridge on invalid surface", closeException)
                }
                startedBridge = null
                starting = false
                return
            }

            val spawnStartNs = System.nanoTime()
            val spawnResult = bridge.spawnTerminal(config.rows, config.cols, bridge.shellPath())
            val spawnElapsedMs = (System.nanoTime() - spawnStartNs) / 1_000_000
            LogUtil.d(
                "Runtime",
                "spawnTerminal: rows=${config.rows} cols=${config.cols} result=$spawnResult elapsed=${spawnElapsedMs}ms",
            )
            if (spawnResult <= 0L) {
                LogUtil.e(
                    "Runtime",
                    "spawnTerminal returned $spawnResult — native session init failed, aborting start",
                )
                bridge.close()
                return
            }

            // sessionId is now non-zero — extra font paths
            // (filesDir/fonts, e.g. user-installed Nerd Fonts) actually
            // reach the native font database here. Called before
            // spawnTerminal it was a silent no-op.
            bridge.setExtraFontPaths(listOf(fontsDir.absolutePath))

            try {
                val initialFontFamily = settingsRepository.fontFamily.first()
                val effectiveFont = terminal.emulator.resolveEffectiveFontFamily(initialFontFamily)
                bridge.setFontFamily(effectiveFont)
                // the native renderer starts with a hardcoded
                // 14.0px font; without this the user's font-size setting
                // never reached the GPU path — glyphs stayed tiny and
                // "setting did nothing / got worse after restart".
                bridge.setFontSizeInPlace(config.font_size_tenths)
                // rasterize glyphs at device density so text is
                // crisp on high-density screens (swash bitmaps are scaled by
                // raster_scale; the shader samples the atlas at that scale).
                val density = context.resources.displayMetrics.density
                // raster_scale must cover the full sp→px mapping: font size
                // is stored in sp, and sp scales with BOTH display density
                // and the user's system font scale. Rasterizing at density
                // alone under-rasterizes when fontScale > 1 (e.g. "font
                // size" accessibility), and the shader then upscales the
                // atlas bitmap — the "blurry text" reports.
                bridge.setRasterScale(
                    (density * context.resources.configuration.fontScale).coerceIn(0.5f, 4f),
                )
                // Refresh cellWidth/cellHeight from the new font metrics and
                // recompute the grid so the first rendered frame matches the
                // configured size. Without this the renderer draws
                // new-sized cells at the spawn-time grid — the
                // "font-size setting vs actual mismatch" flash in the logs
                // (cell_builder logged new cell metrics against the old
                // grid for ~60-160ms until the next insets/surface event).
                syncGridDimensions(bridge)
                recomputeGridFromFontMetrics()
                appliedFontSizeTenths = config.font_size_tenths
                bridge.setTheme(config.theme)
                val cursorStyle = settingsRepository.cursorStyle.first()
                bridge.setCursorStyle(cursorStyle)
                val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
                bridge.setCursorBlinkEnabled(cursorBlinkEnabled)
                val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
                bridge.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
                LogUtil.d(
                    "Runtime",
                    "settings applied: fontFamily=$effectiveFont fontSizeTenths=${config.font_size_tenths} theme=${config.theme.name} cursorStyle=$cursorStyle cursorBlink=$cursorBlinkEnabled cursorSpeed=$cursorBlinkSpeedMs",
                )
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.e(
                    "Runtime",
                    "Failed to apply initial settings (continuing with defaults)",
                    exception,
                )
            }

            // The native spawn result is the authoritative session ID (the
            // native and Kotlin sequences can drift when createSession runs
            // concurrently with this slower bootstrap path). Insert under the
            // lock with that ID.
            var finalSessionId = 0L
            var entry: SessionEntry? = null
            var abandonedReason: String? = null
            synchronized(sessionLock) {
                if (sessions.isNotEmpty()) {
                    // A session was created (or start() re-entered) while the
                    // bootstrap download above was running. Inserting a second
                    // active entry would start a second render thread on the
                    // single global event queue and clobber the existing
                    // session's UI state — destroy the just-spawned native
                    // session instead and keep the existing one active.
                    LogUtil.w("Runtime", "start: sessions already exist, aborting own insertion")
                    starting = false
                    abandonedReason = "duplicate"
                } else {
                    finalSessionId = spawnResult
                    entry =
                        SessionEntry(
                            id = finalSessionId,
                            bridge = bridge,
                            renderThreadRef = null,
                            running = true,
                        )
                    sessions[finalSessionId] = entry
                    activeSessionId = finalSessionId
                    bridge.onPtyWrite = { nanos ->
                        entry.latencyProbe.onInputWritten(nanos)
                    }
                }
            }
            if (abandonedReason != null) {
                // Close the bridge OUTSIDE the lock (Session::drop joins the
                // PTY reader thread for hundreds of ms; holding sessionLock
                // across it would freeze every session operation). Mirrors
                // createSessionInner's abandonedByStart/abandonedByStop
                // rollback.
                try {
                    bridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e("Runtime", "start: failed to close $abandonedReason bridge", closeException)
                }
                return
            }
            // entry is assigned in every branch above; the nullability is
            // only invisible to smart-cast across the early return.
            val startedEntry = requireNotNull(entry) { "start: session entry must exist after spawn" }
            // First render: problematic GPUs (Mali-G57 w/ missing SURFACE_VIEW_FORMATS)
            // can hang get_current_texture() indefinitely. The previous approach of
            // spawning a daemon thread to call bridge.render() caused mutex starvation —
            // the daemon would acquire the surface Mutex and hang, permanently blocking
            // the real render thread. Instead, signal the render loop to produce the
            // first frame via forceRenderRequested, which will be picked up by the
            // real render thread once it starts.
            // ADR-0007: attach the surface now that the bridge exists.
            attachPendingSurface(bridge)
            //  (warp WarpTerminalService.kt:797-808): the first
            // resize with grid dims must be issued right after spawn —
            // attachPendingSurface → recomputeGridFromFontMetrics does it;
            // anchor the grid dims in the spawn sequence for verification.
            LogUtil.d(
                "Runtime",
                "first resize after spawn: grid=${_state.value.rows}x${_state.value.cols}",
            )
            startedEntry.forceRenderRequested = true
            // Start the render thread, publish the UI state, and start the
            // foreground service + monitor under ONE sessionLock critical
            // section: the last writer wins against concurrent close
            // paths. A check-then-publish split leaves a window where a
            // close completes and start() still publishes a ghost
            // RuntimeState / resurrects the service.
            synchronized(sessionLock) {
                val stillActive = sessions[finalSessionId] === startedEntry
                if (!stillActive) {
                    // A concurrent close landed after our insertion: the
                    // entry is already being cleaned up (closeSession
                    // removed the entry). Publishing a RuntimeState or
                    // starting the foreground service now would resurrect a
                    // ghost UI state and the notification underneath
                    // teardown.
                    LogUtil.w(
                        "Runtime",
                        "start: session $finalSessionId closed/stopped during startup, skipping render start",
                    )
                    // Defensive: keep the monitor alive for surviving
                    // sessions when this was a concurrent close rather than
                    // a full stop. Unreachable today (start() only inserts
                    // into an empty map and starting=true blocks concurrent
                    // creates, so no OTHER session can exist), but cheap to
                    // honor if that invariant ever changes.
                    if (sessions.isNotEmpty()) {
                        renderSupervisor.startRenderMonitor()
                    }
                    return
                }

                // Publish the state BEFORE starting the thread: the render
                // thread's title CAS  would otherwise be overwritten
                // by this direct assignment on its first frame.
                _state.value =
                    RuntimeState(
                        isRunning = true,
                        rows = config.rows,
                        cols = config.cols,
                        activeSessionId = finalSessionId,
                        sessionIds = listOf(finalSessionId),
                    )
                LogUtil.d(
                    "Runtime",
                    "session $finalSessionId config: rows=${config.rows} cols=${config.cols} fontSizeTenths=${config.font_size_tenths}",
                )
                LogUtil.d("Runtime", "session $finalSessionId started")
                try {
                    startForegroundServiceIfNeeded()
                    updateForegroundSessionCount(sessions.size)
                } catch (serviceException: Exception) {
                    if (serviceException is kotlinx.coroutines.CancellationException) {
                        // Unreachable in practice (no suspend points in this
                        // locked block; service calls are synchronous IPC)
                        // but rethrown per project convention. If it ever
                        // fires, the entry stays inserted and the caller's
                        // scope teardown owns the cleanup.
                        throw serviceException
                    }
                    // Same guard as createSession: a service-start failure
                    // must not roll back the already-created session.
                    LogUtil.e(
                        "Runtime",
                        "Failed to start foreground service for session $finalSessionId",
                        serviceException,
                    )
                }
                renderSupervisor.startRenderThread(startedEntry)
                renderSupervisor.startRenderMonitor()
            }
            // recompute the grid AFTER the session is registered
            // — the earlier attempt (pre-insertion) was a silent no-op, so
            // the 24x80 startup grid stayed even though the native font was
            // 47px, leaving huge vertical gaps (92px rows vs 36px glyphs).
            try {
                startedEntry.bridge?.let { syncGridDimensions(it) }
                // Grid stays at initial 24x80 — font metrics don't determine
                // grid dimensions; the terminal scrolls when content overflows.
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "initial grid recompute failed", exception)
            }
        } catch (exception: Exception) {
            if (exception is kotlinx.coroutines.CancellationException) {
                // Re-throw cancellation: swallowing it breaks structured
                // concurrency (same convention as createSessionInner). The
                // finally block still resets starting; any spawned bridge
                // leaks are closed by the caller's scope teardown path.
                throw exception
            }
            LogUtil.e("Runtime", "Failed to start terminal", exception)
            // The full stack trace reaches logcat via LogUtil (chunked if
            // needed), with a stable FAILED grep anchor.
            // Any failure after createBridge() (settings, attachSurface,
            // spawnTerminal throwing instead of returning 0) would otherwise
            // leak the native session and its PTY child forever.
            try {
                startedBridge?.close()
            } catch (closeException: Exception) {
                LogUtil.e("Runtime", "Failed to close bridge during start rollback", closeException)
            }
            // If the failure happened AFTER the entry was inserted (e.g.
            // startRenderThread threw inside the lock), the map still holds
            // the entry with a null renderThreadRef: checkSessions' alive
            // logic never marks it dead (null thread reference) and the
            // ghost tab would persist forever. Remove it under the lock —
            // the bridge is already closed above and close is idempotent.
            // Restore the UI state too: a RuntimeState published before the
            // failure would otherwise keep isRunning=true with a dead id.
            synchronized(sessionLock) {
                startedBridge?.let { bridgeToRemove ->
                    sessions.entries.removeIf { it.value.bridge === bridgeToRemove }
                }
                if (sessions.isEmpty()) {
                    activeSessionId = 0L
                    _state.value = RuntimeState()
                    // The service may have been started inside the lock
                    // before the failure; bring the count back to 0 so the
                    // notification and wake lock do not outlive the empty
                    // session map (updateForegroundSessionCount is
                    // exception-safe and clears the running flag at 0).
                    updateForegroundSessionCount(0)
                }
            }
        } finally {
            starting = false
        }
    }

    // Architecture Note: each session currently creates its own bridge with a
    // separate GPU surface (surface.rs owns the wgpu pipeline per ANativeWindow).
    // Sharing a single pre-initialized GPU pipeline across sessions is a possible
    // future optimization (could cut session-creation time) but is not yet
    // implemented — do not assume a shared pipeline exists.
    private val createSessionMutex = kotlinx.coroutines.sync.Mutex()

    /**
     * Creates a new terminal session. Serialized against concurrent calls double-tap on the
     * new-session button) and against [start]: two concurrent creations would each spawn a native
     * session and start a render thread, and two render threads consuming the single global event
     * queue misroute events (exit events dropped, sessions leaked).
     */
    suspend fun createSession(
        surface: Surface,
        width: Int,
        height: Int,
    ): Long = createSessionMutex.withLock {
        createSessionInner(surface, width, height)
    }

    private suspend fun createSessionInner(
        surface: Surface,
        width: Int,
        height: Int,
    ): Long {
        if (starting) {
            LogUtil.w(
                "Runtime",
                "createSession: start() in progress (bootstrap), refusing concurrent creation",
            )
            return -1L
        }
        if (width <= 0 || height <= 0) {
            LogUtil.e("Runtime", "createSession: invalid dimensions ${width}x$height")
            return -1L
        }
        if (!surface.isValid) {
            LogUtil.e("Runtime", "createSession: surface is not valid")
            return -1L
        }
        if (!NativeBridge.isNativeLoaded()) {
            // UnsatisfiedLinkError is an Error, not an Exception — it would
            // escape the catch below and crash the app when the native
            // library is missing/corrupt (e.g. ABI mismatch on x86
            // emulators or 32-bit devices). Fail soft and reuse the
            // existing rollback path.
            LogUtil.e("Runtime", "createSession: native library not loaded, refusing to spawn")
            return -1L
        }
        var nextId = 0L
        var createdBridge: terminal.emulator.bridge.Bridge? = null
        try {
            val configStartNs = System.nanoTime()
            val config = buildConfig()
            val bridgeStartNs = System.nanoTime()
            val bridge = createBridge(config).also { createdBridge = it }
            LogUtil.d(
                "Runtime",
                "createSession bridgeElapsed=${(System.nanoTime() - bridgeStartNs) / 1_000_000}ms",
            )
            bridge.setSystemLocale(
                java.util.Locale.getDefault().toLanguageTag(),
            )

            try {
                val initialFontFamily = settingsRepository.fontFamily.first()
                val effectiveFont = terminal.emulator.resolveEffectiveFontFamily(initialFontFamily)
                bridge.setFontFamily(effectiveFont)
                val cursorStyle = settingsRepository.cursorStyle.first()
                bridge.setCursorStyle(cursorStyle)
                val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
                bridge.setCursorBlinkEnabled(cursorBlinkEnabled)
                val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
                bridge.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.e(
                    "Runtime",
                    "Failed to apply settings to new session (continuing with defaults)",
                    exception,
                )
            }

            // Spawn FIRST (outside the lock) so the native session ID
            // becomes the authoritative map key. Kotlin's max+1 sequence
            // and the native sequence can drift apart when start() (the
            // slow bootstrap path, which also spawns outside its lock) and
            // createSession run concurrently; routing switchSession /
            // handleSessionExit by the native ID keeps them in sync.
            val spawnStartNs = System.nanoTime()
            val spawnResult = bridge.spawnTerminal(config.rows, config.cols, bridge.shellPath())
            val spawnElapsedMs = (System.nanoTime() - spawnStartNs) / 1_000_000
            LogUtil.d(
                "Runtime",
                "createSession spawnTerminal result=$spawnResult elapsed=${spawnElapsedMs}ms",
            )
            if (spawnResult <= 0L) {
                throw RuntimeException("native spawn failed (result=$spawnResult)")
            }
            nextId = spawnResult

            // Apply the theme AFTER spawn: Bridge.setTheme no-ops while
            // sessionId == 0, and calling it before spawnTerminal would
            // silently drop the user's theme (the native session would
            // keep the default palette). Mirrors the start() ordering.
            bridge.setTheme(config.theme)

            val entry: SessionEntry
            val abandonedByStart: Boolean
            synchronized(sessionLock) {
                if (starting) {
                    // start() (bootstrap slow path) may have begun after our
                    // earlier check and inserted its own session while we
                    // were suspended on the spawn call. Inserting a second
                    // entry would leave one of the two sessions without a
                    // render thread (zombie shell process) and desync the UI
                    // session list. Refuse the insertion; the caller
                    // (onSurfaceTextureAvailable fallback) will retry via
                    // start() once bootstrap completes.
                    LogUtil.w("Runtime", "createSession: start() began during spawn, abandoning insertion")
                    abandonedByStart = true
                } else {
                    entry =
                        SessionEntry(
                            id = nextId,
                            bridge = bridge,
                            renderThreadRef = null,
                            running = false,
                        )
                    sessions[nextId] = entry
                    abandonedByStart = false
                    bridge.onPtyWrite = { nanos ->
                        entry.latencyProbe.onInputWritten(nanos)
                    }
                }
            }
            if (abandonedByStart) {
                // Close the bridge outside the lock (Session::drop joins the
                // PTY reader thread) so the just-spawned native session and
                // its shell child are not leaked.
                try {
                    bridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e("Runtime", "createSession: failed to close abandoned bridge", closeException)
                }
                return -1L
            }

            try {
                // Outside the lock: switchSessionInternal performs a
                // synchronous first-frame render that can block on a hung
                // GPU; holding sessionLock across it would freeze every
                // session operation. needsSpawn=false: the session was
                // spawned above — spawning again here would create a second
                // native session whose ID diverges from the map key
                // (split input/output + a leaked shell process).
                switchSessionInternal(
                    nextId,
                    surface,
                    width,
                    height,
                )
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "Failed to switch to new session $nextId, rolling back", exception)
                // Structural map change under the lock  invariant);
                // the bridge close below stays OUTSIDE the lock (Session::drop
                // joins threads).
                synchronized(sessionLock) {
                    sessions.remove(nextId)
                }
                // Close the native bridge so the Rust-side session and
                // its PTY child are not leaked.
                try {
                    bridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e(
                        "Runtime",
                        "Failed to close bridge during session $nextId rollback",
                        closeException,
                    )
                }
                throw exception
            }

            updateState()
            // A session created outside the bootstrap path (e.g. the "+"
            // button after all sessions were closed) must bring the
            // foreground service back: the close paths stop it at count 0,
            // and without it there is no foreground notification and no
            // PARTIAL_WAKE_LOCK — background sessions can be killed.
            // Service start + liveness re-check happen in ONE sessionLock
            // section, symmetric with start(): serializes with
            // concurrent close paths, so a close can neither land between
            // the service start and the re-check (resurrecting the
            // notification underneath teardown) nor clear the map
            // mid-block (ghost id).
            // Guarded individually: a ForegroundServiceStartNotAllowedException
            // (API 31+ background start) or ROM SecurityException here must
            // NOT leak the already-inserted session through the generic
            // catch below (which would skip the bridge close and return -1
            // for a live session).
            val stillPresent: Boolean
            synchronized(sessionLock) {
                stillPresent = sessions.containsKey(nextId)
                if (stillPresent) {
                    try {
                        startForegroundServiceIfNeeded()
                        updateForegroundSessionCount(sessions.size)
                    } catch (serviceException: Exception) {
                        if (serviceException is kotlinx.coroutines.CancellationException) {
                            // Unreachable in practice (no suspend points in
                            // this locked block), rethrown per convention;
                            // scope teardown owns any cleanup.
                            throw serviceException
                        }
                        LogUtil.e(
                            "Runtime",
                            "Failed to start foreground service for session $nextId",
                            serviceException,
                        )
                    }
                }
            }
            if (!stillPresent) {
                LogUtil.w(
                    "Runtime",
                    "createSession: session $nextId removed concurrently (exit/close), rolling back",
                )
                // NOTE: this close may race the lock-outside close of the
                // path that removed the entry (handleSessionExit /
                // closeSession). Safe by construction: Bridge.close() is
                // idempotent via its sessionId!=0 guard, and native
                // destroySession is idempotent via registry remove.
                try {
                    bridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e("Runtime", "createSession: failed to close rolled-back bridge", closeException)
                }
                return -1L
            }
            LogUtil.d("Runtime", "session $nextId created and activated")
            return nextId
        } catch (exception: Exception) {
            if (exception is kotlinx.coroutines.CancellationException) {
                // Re-throw cancellation: swallowing it breaks structured
                // concurrency and can leak the session if the caller's
                // scope is cancelled mid-spawn.
                throw exception
            }
            LogUtil.e("Runtime", "Failed to create session $nextId", exception)
            // The full stack trace reaches logcat via LogUtil (chunked if
            // needed), with a stable FAILED grep anchor.
            // If the failure happened before the entry was inserted (settings
            // application, spawnTerminal throwing), the bridge was never
            // rolled back above — close it to avoid leaking the native
            // session and PTY child.
            createdBridge?.let { leaked ->
                if (leaked !== sessions[nextId]?.bridge) {
                    try {
                        leaked.close()
                    } catch (closeException: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "Failed to close leaked bridge during createSession rollback",
                            closeException,
                        )
                    }
                }
            }
            return -1L
        }
    }

    fun switchSession(
        id: Long,
        surface: Surface,
        width: Int,
        height: Int,
    ) {
        switchSessionInternal(id, surface, width, height)
        updateState()
    }

    private fun switchSessionInternal(
        id: Long,
        surface: Surface,
        width: Int,
        height: Int,
    ) {
        // Phase 1 (locked): validate, stop the previous render thread,
        // (re)configure the target bridge. Phase 2 (UNLOCKED): the
        // synchronous first-frame render retry — bridge.render() can block
        // forever on a hung GPU, and holding sessionLock across it would
        // freeze every session operation (close/switch/stop, render
        // monitor). Phase 3 (locked): start the new render thread and
        // publish the new active session.
        val target: SessionEntry
        val previousActiveId: Long
        synchronized(sessionLock) {
            target =
                sessions[id]
                    ?: run {
                        LogUtil.e("Runtime", "switchSession: session $id not found")
                        return
                    }
            // Capture before any stopping happens; used to restore the
            // previous session if the switch/spawn fails.
            previousActiveId = activeSessionId
            if (id == activeSessionId) return
            // ADR-0007: hand the Surface to the renderer (attachWindow JNI
            // extracts the ANativeWindow inside Rust). This is LAZY: it only
            // stores the reference — the wgpu surface is created on the
            // first render frame, which happens after the old session's
            // thread is stopped and its surface released below
            // the release order is therefore attach-stored → stop
            // old thread → release old surface → new surface created on
            // first frame; the same ANativeWindow is never held by two
            // live wgpu surfaces).
            target.bridge?.attachSurface(surface, width, height)

            if (!surface.isValid) {
                LogUtil.e("Runtime", "switchSession: surface is no longer valid, aborting")
                return
            }

            val current = sessions[activeSessionId]
            if (current != null) {
                // NOTE /93): stopRenderThread joins the OLD render
                // thread while holding sessionLock (up to THREAD_JOIN_TIMEOUT_MS
                // = 1s). switchSession runs on an IO-dispatcher coroutine
                // (TerminalViewModel), so this does NOT ANR the UI thread —
                // but it stalls every sessionLock-mediated operation
                // (close/switch/create/stopForegroundServiceIfIdle, including
                // MainActivity.onDestroy). ACCEPTED tradeoff: the join only
                // blocks when the old thread is stuck in native code (GPU
                // hang — rendering is already degraded), and a lock-free
                // stop would let closeSession race this entry's teardown
                // (use-after-free on the bridge).
                try {
                    val stopped = renderSupervisor.stopRenderThread(current)
                    if (stopped) {
                        // Release the old bridge's GPU surface before the new
                        // bridge creates its own on the same ANativeWindow.
                        // This avoids VK_ERROR_NATIVE_WINDOW_IN_USE_KHR from
                        // the Vulkan driver when two wgpu surfaces share the
                        // same ANativeWindow. Skipped when the render thread
                        // hung (join timeout) — releasing would be a
                        // use-after-free once the thread resumes.
                        current.bridge?.releaseGpuSurface()
                    } else {
                        LogUtil.e(
                            "Runtime",
                            "switchSession: session ${current.id} render thread hung — skipping surface release",
                        )
                    }
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "switchSession: error stopping current session", exception)
                }
            }

            try {
                // ADR-0007: the surface is handed to the renderer via
                // attachSurface above; nothing else is needed here.
                target.running = true
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "switchSession: attachSurface failed for session $id", exception)
                // Spawn failure: the previous active session was stopped and
                // its GPU surface released above. Restore it so the terminal
                // is not left frozen (running=false, no render thread, and
                // the monitor skips !running entries forever).
                val previous = sessions[previousActiveId]
                if (previous != null && shouldRestorePreviousSession(previous.id, id)) {
                    LogUtil.w(
                        "Runtime",
                        "switchSession: restoring previous session ${previous.id} after failure",
                    )
                    previous.running = true
                    previous.renderThreadExited = false
                    previous.restartAttempts = 0
                    try {
                        renderSupervisor.startRenderThread(previous)
                        activeSessionId = previous.id
                    } catch (restoreException: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "switchSession: failed to restore previous session ${previous.id}",
                            restoreException,
                        )
                    }
                }
                return
            }
        }

        // Phase 2 (UNLOCKED): render the new session's first frame
        // SYNCHRONOUSLY before the event-driven render thread starts, so
        // the reconfigured swapchain shows real content immediately instead
        // of a brief blank/clear frame. Reconfiguring the swapchain
        // (above) discards the previous session's backbuffer, and the
        // render thread's first frame is only presented after OS thread
        // scheduling — that gap is exactly the blank flash. Presenting
        // here closes it. The render thread takes over right after (it
        // re-renders once, then latches on the RENDER_LATCH_IDLE_TIMEOUT_NANOS
        // cadence, so the event-driven model is preserved).
        //
        // NOTE: deliberately outside sessionLock — bridge.render() can hang
        // indefinitely on a GPU fault (Mali-G57 get_current_texture).
        try {
            // The GPU surface may not be fully configured immediately after
            // spawnTerminal/attachSurface (a transient race on shared
            // ANativeWindow). Retry the first synchronous render a few times
            // with a short delay so we present real content instead of
            // starting a render thread on a not-yet-ready surface (which would
            // block and trip the hang watchdog).
            var initialRender = target.bridge?.render() ?: 0
            var attempts = 1
            while (initialRenderRetryNeeded(initialRender, attempts, RENDER_INITIAL_RETRY_MAX)) {
                Thread.sleep(RENDER_INITIAL_RETRY_DELAY_MS)
                initialRender = target.bridge?.render() ?: 0
                attempts++
            }
            LogUtil.d(
                "Runtime",
                "switchSession: initial render for session $id result=$initialRender (attempts=$attempts)",
            )
            target.forceRenderRequested = true
            target.notifyRender()
        } catch (exception: Exception) {
            LogUtil.e(
                "Runtime",
                "switchSession: initial render failed for session $id",
                exception,
            )
        }

        // Phase 3 (locked): publish the switch.
        synchronized(sessionLock) {
            // Re-validate: the session may have been closed while the first
            // frame was rendering (user closed it, or the monitor reaped it).
            if (sessions[id] !== target) {
                LogUtil.w(
                    "Runtime",
                    "switchSession: session $id was closed during first frame, aborting switch",
                )
                return
            }
            // A concurrent switchSession may have published a different
            // active session while we were rendering the first frame. Its
            // render thread is running; stop it before starting ours so only
            // one render thread ever consumes the shared event queue (two
            // consumers misroute bell/clipboard/notification events).
            val concurrentToStop =
                concurrentRenderThreadToStop(
                    activeSessionIdAfterRender = activeSessionId,
                    previousActiveId = previousActiveId,
                    targetId = id,
                    concurrentSessionId = sessions[activeSessionId]?.id,
                )
            if (concurrentToStop != null) {
                val concurrent = sessions[concurrentToStop]
                if (concurrent != null) {
                    try {
                        renderSupervisor.stopRenderThread(concurrent)
                        LogUtil.w(
                            "Runtime",
                            "switchSession: stopped concurrent session $concurrentToStop (active changed during first frame)",
                        )
                    } catch (exception: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "switchSession: failed to stop concurrent session $concurrentToStop",
                            exception,
                        )
                    }
                }
            }
            try {
                renderSupervisor.startRenderThread(target)
                activeSessionId = id
                // Sync the native ACTIVE_SESSION_ID so pollEvent/process_output
                // operate on the new session. Without this, all sessions except
                // the first share one native-active session and multi-session
                // output/exit detection silently breaks.
                try {
                    NativeBridge.switchSession(id)
                } catch (exception: Exception) {
                    LogUtil.e(
                        "Runtime",
                        "switchSession: native switchSession failed for session $id",
                        exception,
                    )
                }
                target.bridge?.let { syncGridDimensions(it) }
                //  stopped applySettings from resizing background
                // sessions, so the newly active session must be aligned to the
                // current window size here: syncGridDimensions reads the
                // native grid (an ADR-0007 stub returning 0) and can't tell us
                // the real dims, so resize unconditionally with the latest UI
                // state.
                target.bridge?.resize(
                    _state.value.rows.coerceAtLeast(1),
                    _state.value.cols.coerceAtLeast(1),
                )
                LogUtil.d("Runtime", "switched to session $id")
                // DECSET 1004 focus reporting is per-window and the new
                // active session never received a focus-in while backgrounded.
                // Re-send the last known window focus state so a focused
                // TUI (vim/fzf) resumes its FocusGained behaviour.
                if (lastWindowFocus) {
                    target.bridge?.focusEvent(true)
                }
            } catch (exception: Exception) {
                LogUtil.e(
                    "Runtime",
                    "switchSession: failed to start render thread for session $id",
                    exception,
                )
            }
        }
    }

    /**
     * Single entry point for the Activity teardown path (onDestroy): stop the foreground service when
     * no session is running, or refresh the notification count when sessions survive. Centralizes the
     * service lifecycle here instead of MainActivity calling the static methods directly (which
     * bypassed the runtime's foregroundServiceRunning flag and its exception protection).
     *
     * The count read and the stop decision both happen inside sessionLock : a createSession landing
     * between the snapshot and the stop would otherwise leave a live session without its foreground
     * service.
     *
     * note: if onDestroy runs while runtime.start() is mid-bootstrap (no session inserted yet), this
     * stops the service and start() later re-inserts a session and re-starts the service. That is the
     * intended background-session semantics (Termux-style: sessions outlive the Activity), not a leak
     * — the final state is one live session + one foreground service. A leak would only occur if
     * start() FAILED after the service restart, which its rollback (updateForegroundSessionCount(0))
     * already handles.
     */
    fun stopForegroundServiceIfIdle() {
        synchronized(sessionLock) {
            if (sessions.isEmpty()) {
                stopForegroundService()
            } else {
                updateForegroundSessionCount(sessions.size)
            }
        }
    }

    fun closeSession(id: Long) {
        // Phase 1 (locked): capture close eligibility AND clear the running
        // intent flag immediately, symmetric with handleSessionExit — a
        // delayed restart (restartRenderThreadAfterDelay) checks running
        // under the lock and must see false to cancel. Reading running
        // UNLOCKED later could catch startRenderThread's transient
        // running=false→true window and misclassify the close as safe while
        // a brand-new render thread is starting (orphaned thread, global
        // event queue double-consumer).
        var wasRunning: Boolean = false
        val entry =
            synchronized(sessionLock) {
                val e = sessions[id] ?: return
                wasRunning = e.running
                e.running = false
                e.closing = true
                e
            }
        LogUtil.d("Runtime", "closeSession($id)")

        // Stop the render thread for any session, active or not.
        // running and renderThreadPossiblyAlive are @Volatile; the entry
        // reference stays valid outside the lock (the object is only
        // dropped when our last reference goes away).
        var renderThreadStopped = true
        if (wasRunning) {
            renderThreadStopped = renderSupervisor.stopRenderThread(entry)
        } else if (entry.renderThreadPossiblyAlive) {
            // A previous join timed out (hung GPU) while running was
            // still true; the flag survives the pause path where
            // running is set false. The thread may still be inside
            // native render code, so this is NOT a safe-to-close state.
            renderThreadStopped = false
        }
        // Locked re-check before closing (pure state read, NO join — a
        // join here would hold sessionLock for up to THREAD_JOIN_TIMEOUT_MS
        // and block every session operation). Any concurrent
        // startRenderThread is rejected by entry.closing (and resets
        // running=false), so only a thread that predates the close can be
        // alive here; mark the session un-closable instead (native session
        // reclaimed at process death) — safe and non-blocking.
        synchronized(sessionLock) {
            if (entry.renderThreadRef?.isAlive == true) {
                renderThreadStopped = false
            }
        }
        if (renderThreadStopped && !entry.renderThreadPossiblyAlive) {
            // Only release the GPU surface when the render thread is
            // confirmed dead. stopRenderThread's join may have succeeded
            // on the CURRENT thread while an EARLIER hung thread (from a
            // previous join timeout, recorded in hungRenderThread) is
            // still alive inside native code — renderThreadPossiblyAlive
            // covers that case. Releasing/closing here would be a
            // use-after-free when that thread resumes.
            entry.bridge?.releaseGpuSurface()
            // Same reasoning applies to close(): destroySession tears
            // down the native session/PTY/wgpu context that the hung
            // thread may still be touching. The native session is
            // reclaimed when the process dies; destroying it here
            // would be a use-after-free the moment the thread resumes.
            entry.bridge?.close()
        } else {
            LogUtil.e(
                "Runtime",
                "session $id render thread hung — skipping surface release AND bridge close to avoid use-after-free",
            )
        }

        // Phase 3 (locked): remove the session and, if it was active,
        // switch to a replacement. The replacement's hung-thread join and
        // render restart stay inside the lock, symmetric with
        // handleSessionExit (they only run on the close-active path).
        synchronized(sessionLock) {
            if (!sessions.containsKey(id)) return
            sessions.remove(id)
            updateForegroundSessionCount(sessions.size)

            // If we closed the active session, switch to another
            if (id == activeSessionId) {
                val remaining = sessions.keys.sorted()
                if (remaining.isNotEmpty()) {
                    val newId = remaining.last()
                    activeSessionId = newId
                    activateReplacementSession(
                        newId,
                        "closeSession",
                        markRunning = true,
                        withRetry = false,
                        syncGrid = true,
                    )
                } else {
                    activeSessionId = 0L
                }
            }
            updateState()
        }
    }

    suspend fun applySettings() {
        val config = buildConfig()
        val fontFamily = settingsRepository.fontFamily.first()
        val effectiveFontFamily = terminal.emulator.resolveEffectiveFontFamily(fontFamily)
        val cursorStyle = settingsRepository.cursorStyle.first()
        val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
        val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
        // buildConfig() defaults to 24x80; resizing every session to that
        // on ANY settings change would shrink live PTYs (vim/htop get a
        // spurious SIGWINCH and reflow). Keep each session's current grid
        // size instead.
        // Floor of 1, not 24: with the IME open the visible grid can
        // legitimately be smaller, and inflating it fires a spurious
        // SIGWINCH + shell reflow.
        val currentRows = _state.value.rows.coerceAtLeast(1)
        val currentCols = _state.value.cols.coerceAtLeast(1)
        sessions.values.forEach { entry ->
            entry.bridge?.setFontSize(config.font_size_tenths)
            entry.bridge?.setFontFamily(effectiveFontFamily)
            entry.bridge?.setTheme(config.theme)
            entry.bridge?.setCursorStyle(cursorStyle)
            entry.bridge?.setCursorBlinkEnabled(cursorBlinkEnabled)
            entry.bridge?.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
            entry.notifyRender()
        }
        // Font metrics changed — but grid dimensions stay fixed.
        // The terminal scrolls when content overflows the visible area.
    }

    /**
     * Recompute the active session's grid from the CURRENT native font metrics: rows = (surface -
     * ModifierBar) / cell_height, cols = surface / cell_width. No-op when the surface size or metrics
     * aren't known yet (fonts applied pre-attach). Called after every font-size change and after the
     * initial font application.
     */
    private fun recomputeGridFromFontMetrics() {
        val density = context.resources.displayMetrics.density
        val barHeightPx = (MODIFIER_BAR_HEIGHT_DP * density + 0.5f).toInt()
        // Surface size: the pending surface (set by startRuntime) is the
        // authoritative source before the first attachSurface lands.
        val surfaceW = pendingSurfaceWidth
        val surfaceH = pendingSurfaceHeight
        val currentRows = _state.value.rows.coerceAtLeast(1)
        val currentCols = _state.value.cols.coerceAtLeast(1)
        if (surfaceW <= 0 || surfaceH <= 0 || cellWidth <= 0f || cellHeight <= 0f) return
        // Compute grid dimensions from physical surface / physical cell metrics.
        // Both surface and cell are in physical pixels (density-scaled). The
        // ModifierBar overlays the bottom of the surface, so its height is
        // subtracted before computing rows.
        val (newRows, newCols) =
            computeGridDimensions(
                surfaceWidth = surfaceW,
                surfaceHeight = surfaceH - barHeightPx,
                cellWidth = cellWidth,
                cellHeight = cellHeight,
            )
        if (newRows == 0 || newCols == 0) {
            // Degenerate geometry (usable height ≤ 0, e.g. a surface no
            // taller than the ModifierBar): keep the current grid instead
            // of collapsing it to one row.
            return
        }
        if (newRows == currentRows && newCols == currentCols) return
        LogUtil.d(
            "Runtime",
            "recomputeGridFromFontMetrics: ${currentRows}x$currentCols -> ${newRows}x$newCols " +
                "(cell ${cellWidth}x$cellHeight, surface ${surfaceW}x$surfaceH)",
        )
        // Resize only the active session: background sessions keep their own
        // grid dimensions; resizing every session with the active one's size
        // would fire spurious SIGWINCH + reflow on them.
        sessions[activeSessionId]?.bridge?.resize(newRows, newCols)
        _state.update { it.copy(rows = newRows, cols = newCols) }
    }

    suspend fun applyFontSettings() {
        val fontSizeTenths = computeFontSizeTenths()
        appliedFontSizeTenths = fontSizeTenths
        val fontFamily = settingsRepository.fontFamily.first()
        val effectiveFontFamily = terminal.emulator.resolveEffectiveFontFamily(fontFamily)
        LogUtil.d(
            "Runtime",
            "applyFontSettings: fontFamily='$fontFamily' effective='$effectiveFontFamily' fontSizeTenths=$fontSizeTenths sessions=${sessions.size}",
        )
        sessions.values.forEach { entry ->
            try {
                val familyResult = entry.bridge?.setFontFamily(effectiveFontFamily)
                LogUtil.d("Runtime", "setFontFamily result: $familyResult")
                // Independent bold/italic families (ghostty-android 4-slot
                // design): empty = clear the slot (same-family fallback).
                val boldFamily =
                    terminal.emulator.resolveEffectiveFontFamily(settingsRepository.boldFontFamily.first())
                val italicFamily =
                    terminal.emulator.resolveEffectiveFontFamily(
                        settingsRepository.italicFontFamily.first(),
                    )
                entry.bridge?.setFontFamilyForStyle(boldFamily, terminal.emulator.FONT_SLOT_BOLD)
                entry.bridge?.setFontFamilyForStyle(italicFamily, terminal.emulator.FONT_SLOT_ITALIC)
                entry.bridge?.setFontSizeInPlace(fontSizeTenths)
                entry.bridge?.let { syncGridDimensions(it) }
                // the grid must follow the font. syncGridDimensions
                // only reads the existing native grid (still the default
                // 80x24 after a restart); without recomputing rows/cols from
                // the surface and the new font metrics the renderer lays the
                // grid at surface/80 x surface/24 (13.5x92) while glyphs are
                // rasterized for 41.2x81.4 cells — fonts look huge and rows
                // overlap. Recompute and resize the active session.
                recomputeGridFromFontMetrics()
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "applyFontSettings failed for session", exception)
            }
        }
    }

    fun loadFontFile(path: String): String? {
        val entry = sessions.values.firstOrNull() ?: return null
        return entry.bridge?.loadFontFile(path)
    }

    fun writeToPty(data: ByteArray): Boolean {
        val entry = sessions[activeSessionId]
        if (entry != null && entry.running) {
            // the shell exited and [Process completed] is
            // showing — the only accepted input is Enter, which confirms
            // the prompt and lets the render loop run the close path.
            if (entry.waitingForProcessCompleted) {
                if (data.any { it == '\r'.code.toByte() || it == '\n'.code.toByte() }) {
                    synchronized(sessionLock) {
                        entry.processCompletedConfirmed = true
                    }
                    entry.renderSignaled.set(true)
                    entry.notifyRender()
                }
                return true
            }
            // any user input marks the session as interactive —
            // a later fast exit is a legitimate quick exit (e.g. `exit`),
            // so fast-death recovery must NOT fire for it.
            entry.userTypedSinceSpawn = true
            val written = entry.bridge?.writeToPty(data) ?: false
            if (written) {
                // Latency probe input stamp (elapsed-realtime clock: it
                // survives deep sleep, unlike nanoTime's monotonic base).
                entry.latencyProbe.onInputWritten(SystemClock.elapsedRealtimeNanos())
            }
            entry.notifyRender()
            return written
        }
        LogUtil.w("Runtime", "writeToPty: no active running session to receive write")
        return false
    }

    /** Feed bytes directly to the VT parser (test path for escape sequences). */
    fun feedTerminal(data: ByteArray): Boolean {
        val entry = sessions[activeSessionId] ?: return false
        return entry.bridge?.feedTerminal(data) ?: false
    }

    fun bridge(): Bridge? = sessions[activeSessionId]?.bridge

    /**
     * Input→echo latency summary of the active session (emulator-performance-verification). `NOT
     * MEASURED` below N=30 — callers must surface that verbatim instead of inventing a number.
     */
    fun latencyReport(): String = sessions[activeSessionId]?.latencyProbe?.report() ?: "latency NOT MEASURED n=0"

    @Volatile private var lastWindowFocus: Boolean = false

    fun focusChange(focused: Boolean) {
        lastWindowFocus = focused
        // Focus reporting (DECSET 1004) is per-window: only the active
        // session receives it. Broadcasting to every session would perform
        // N synchronous JNI RPCs (each holding the session lock) on the UI
        // thread for a single window focus change.
        val entry = sessions[activeSessionId] ?: return
        entry.bridge?.focusEvent(focused)
    }

    fun pauseRendering() {
        // stopRenderThread joins the render thread (up to 1s per session);
        // on the main thread (surface destroy) with 3+ sessions this can
        // exceed the 5s ANR threshold, so run it off the main thread.
        // surfaceDestroyed returns immediately; rendering simply stops.
        //
        // Both pause and resume go through the same single-thread executor
        // so surface-destroy → surface-available ordering is preserved:
        // otherwise an async pause could stop the render thread that a
        // synchronous resume just started (fixed-size device rotation).
        //
        //  note: the per-session join happens INSIDE sessionLock
        // (up to 1s each), so with several sessions every sessionLock
        // operation (close/switch/stopForegroundServiceIfIdle) stalls for
        // the sum — same accepted tradeoff as switchSession's locked join
        // (GPU-hang-only in practice, executor thread so no ANR).
        surfaceTransitionExecutor.execute {
            synchronized(sessionLock) {
                sessions.values.forEach { entry ->
                    if (entry.running) {
                        renderSupervisor.stopRenderThread(entry)
                        entry.running = false
                        LogUtil.d("Runtime", "pauseRendering: session ${entry.id} stopped")
                    }
                }
            }
        }
    }

    fun resumeRendering() {
        surfaceTransitionExecutor.execute {
            synchronized(sessionLock) {
                // Only the active session renders (see switchSessionInternal);
                // starting threads for every session would create multiple
                // consumers of the single global native event queue, and an
                // exit event could then be handled by the wrong session's
                // thread (closing an innocent session).
                val activeEntry = sessions[activeSessionId]
                if (activeEntry != null && !activeEntry.running && activeEntry.bridge != null) {
                    try {
                        activeEntry.running = true
                        renderSupervisor.startRenderThread(activeEntry)
                        LogUtil.d("Runtime", "resumeRendering: session ${activeEntry.id} restarted")
                    } catch (exception: Exception) {
                        LogUtil.e("Runtime", "resumeRendering failed for session ${activeEntry.id}", exception)
                    }
                }
                // Inside the lock: serializing here means the
                // monitor starts before any concurrent close path's
                // stopRenderMonitor and gets cancelled by it — never a
                // monitor resurrected after teardown completed.
                renderSupervisor.startRenderMonitor()
            }
        }
    }

    fun setSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean,
        mode: Byte = 0,
        selectionBgArgb: Int = selectionBgColor,
    ) {
        LogUtil.d(
            "Runtime",
            "setSelection: start=($startRow,$startCol) end=($endRow,$endCol) active=$hasSelection mode=$mode",
        )
        // Full-snapshot overwrite: dragging resets to false here — every
        // commit path (endSelection/clearSelection/syncSelectionToNative)
        // funnels through this call, so a finished drag always clears the
        // P1-1 dragging guard.
        selectionState.set(
            SelectionStateSnapshot(startRow, startCol, endRow, endCol, hasSelection, mode),
        )
        val entry = sessions[activeSessionId]
        entry
            ?.bridge
            ?.setSelection(startRow, startCol, endRow, endCol, hasSelection, mode, selectionBgArgb)
        entry?.notifyRender()
    }

    fun expandAndSetSelection(
        row: Int,
        col: Int,
        mode: Byte = 0,
    ): Pair<Pair<Int, Int>, Pair<Int, Int>>? {
        LogUtil.d("Runtime", "expandAndSetSelection: row=$row col=$col mode=$mode")
        val entry = sessions[activeSessionId] ?: return null
        val bounds = entry.bridge?.expandAndSetSelection(row, col, mode) ?: return null
        val (start, end) = bounds
        selectionState.set(
            SelectionStateSnapshot(start.first, start.second, end.first, end.second, true, mode),
        )
        entry.notifyRender()
        return bounds
    }

    /**
     * Resize the active session's terminal grid. Values are clamped to the native u16 range
     * (1..=65535) BEFORE the bridge call so the PTY/MCP dimensions and the UI state agree.
     */
    fun resize(
        rows: Int,
        cols: Int,
    ) {
        val entry = sessions[activeSessionId] ?: return
        // Clamp BEFORE the bridge call so native (PTY/MCP dims) and UI state
        // can never diverge: 0 is a legal native size but would break grid
        // math elsewhere, and native resize rejects anything >u16 (the
        // Kotlin state would otherwise carry a value the PTY silently
        // refused). This clamp is the single end-to-end guard. When the
        // native grid command is dropped (ResizeOutcome::Dropped) this state
        // holds the REQUESTED size while native keeps the old cached size;
        // the divergence self-heals on the next resize event, and once
        // getGridRowsColsPacked is real, syncGridDimensions would rewrite
        // this state back to the native value. Note the
        // upper bound is the u16 protocol limit, NOT a display-size sanity
        // limit: the UI paths (window insets, applySettings) supply real
        // grid sizes, so a 65535×65535 grid can only be requested by a
        // direct API caller; native would attempt the allocation.
        val clampedRows = rows.coerceIn(1, 0xFFFF)
        val clampedCols = cols.coerceIn(1, 0xFFFF)
        entry.bridge?.resize(clampedRows, clampedCols)
        // CAS: a plain copy would overwrite a title update the
        // render thread published between read and write.
        _state.update { it.copy(rows = clampedRows, cols = clampedCols) }
        entry.notifyRender()
    }

    /**
     * Update the PTY winsize pixel fields (ws_xpixel/ws_ypixel) for the active session, preserving
     * rows/cols. Called alongside every grid resize with the terminal surface's pixel dimensions, so
     * pixel-aware programs (`icat`, fullscreen TUIs) read real pixels from TIOCGWINSZ instead of 0
     * (ghostty-android pty_jni.c:84-87). Values are clamped to the native u16 range like [resize]; a
     * 0 both is legal (clears the fields) and the ioctl succeeds, so no lower-bound guard is needed.
     */
    fun setPixelSize(widthPx: Int, heightPx: Int) {
        val entry = sessions[activeSessionId] ?: return
        entry.bridge?.setPixelSize(widthPx.coerceIn(0, 0xFFFF), heightPx.coerceIn(0, 0xFFFF))
    }

    fun recomputeGrid(
        width: Int,
        height: Int,
    ) {
        val bridge = sessions[activeSessionId]?.bridge ?: return
        bridge.recomputeGrid(width, height)
        syncGridDimensions(bridge)
        // bridge.recomputeGrid is a logging stub — the real reflow happens
        // here: rows/cols recomputed from the current surface size and the
        // native font cell metrics. Without it, surface re-attach after a
        // restart kept the bootstrap 24x80 grid while glyphs render at the
        // configured (larger) size: prompts got truncated ("/home/com.ter"
        // instead of the full path) and wrapping broke.
        recomputeGridFromFontMetrics()
    }

    /**
     * Hand the Android Surface to the renderer (ADR-0007). If the session bridge does not exist yet
     * (spawn in progress), the surface is kept pending and attached right after the session starts.
     */
    fun attachSurface(surface: android.view.Surface, width: Int, height: Int) {
        pendingSurface = surface
        pendingSurfaceWidth = width
        pendingSurfaceHeight = height
        val bridge = sessions[activeSessionId]?.bridge
        if (bridge != null) {
            bridge.attachSurface(surface, width, height)
        }
    }

    private fun attachPendingSurface(bridge: terminal.emulator.bridge.Bridge) {
        val surface = pendingSurface ?: return
        // The holder may have been destroyed while the bridge was spawning
        // (onSurfaceDestroyed clears the field, but a racing read can still
        // observe the stale value) — never attach a dead Surface.
        if (!surface.isValid) {
            pendingSurface = null
            return
        }
        bridge.attachSurface(surface, pendingSurfaceWidth, pendingSurfaceHeight)
        // the grid must match the font cell metrics against the
        // real surface. attachSurface makes the surface size authoritative;
        // syncGridDimensions pulls the native font cell metrics, then
        // recomputeGridFromFontMetrics resizes the grid so the renderer's
        // quads (surface/rows x surface/cols) match the glyph raster size
        // (font cell x density). Without this the grid stays at the default
        // 80x24 while glyphs are rasterized for the configured font size —
        // fonts look huge and rows overlap after every restart.
        syncGridDimensions(bridge)
        recomputeGridFromFontMetrics()
    }

    /**
     * Activate [newId] as the foreground session after its predecessor closed: final hung-thread
     * join, native ACTIVE_SESSION_ID sync switchSession), render-thread restart and focus re-send.
     *
     * Shared by handleSessionExit, closeDeadSession and closeSession so this lock-held sequence lives
     * in exactly one place instead of three drifting copies.
     *
     * Must be called with sessionLock held; the caller already removed the closing entry and set
     * activeSessionId = [newId].
     *
     * @param caller log prefix (e.g. "handleSessionExit")
     * @param markRunning set replacement.running = true first (closeSession needs it; the exit paths
     *   already run with the session running)
     * @param withRetry retry switchSession once on JNI failure (exit paths; a user close skips the
     *   retry to keep latency bounded)
     * @param syncGrid call syncGridDimensions on the replacement after the render start (closeSession
     *   only)
     */
    private fun activateReplacementSession(
        newId: Long,
        caller: String,
        markRunning: Boolean,
        withRetry: Boolean,
        syncGrid: Boolean,
    ) {
        val replacement =
            sessions[newId]
                ?: run {
                    LogUtil.w("Runtime", "$caller: new active session $newId already removed")
                    activeSessionId = 0L
                    updateState()
                    return
                }
        if (markRunning) {
            replacement.running = true
        }
        val bridge =
            replacement.bridge
                ?: run {
                    LogUtil.w("Runtime", "$caller: new active session $newId has no bridge")
                    activeSessionId = 0L
                    updateState()
                    return
                }
        // Final join of any hung thread: if it exited meanwhile, clear the
        // flag so a later close() destroys the native session (no leak).
        // Guard against joining ourselves: the exit paths run on a render
        // thread (poll.exit), and with background-session reaping
        // ANY session's render thread can end up here — including the
        // replacement's own, if it was previously recorded as hung. Joining
        // self always times out and would freeze every session operation
        // for the full timeout.
        val hung = replacement.hungRenderThread
        if (
            replacement.renderThreadPossiblyAlive &&
            hung != null &&
            hung !== Thread.currentThread() &&
            hung.isAlive
        ) {
            hung.interrupt()
            hung.join(THREAD_JOIN_TIMEOUT_MS)
        }
        if (
            replacement.renderThreadPossiblyAlive &&
            hung != null &&
            hung !== Thread.currentThread() &&
            !hung.isAlive
        ) {
            replacement.renderThreadPossiblyAlive = false
            replacement.hungRenderThread = null
        }
        // Sync the native ACTIVE_SESSION_ID so pollEvent/process_output
        // drive the replacement session. destroySession only clears the
        // native active id to 0; without an explicit switchSession the
        // replacement would never be polled and output stays frozen.
        // NOTE: this runs even when the old render thread is still alive
        // (it is exiting: running=false makes the loop end) — skipping
        // would leave native active=0 and freeze the replacement.
        var nativeSwitched =
            try {
                NativeBridge.switchSession(newId)
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "$caller: native switchSession failed", exception)
                if (withRetry) {
                    // One retry: a transient JNI failure leaves the native
                    // active id stale and the replacement degraded to the
                    // background sweep rate (2 chunks/frame).
                    try {
                        val retried = NativeBridge.switchSession(newId)
                        if (retried) {
                            LogUtil.w("Runtime", "$caller: native switchSession recovered on retry")
                        }
                        retried
                    } catch (retryException: Exception) {
                        LogUtil.e("Runtime", "$caller: native switchSession retry failed", retryException)
                        false
                    }
                } else {
                    false
                }
            }
        if (!nativeSwitched) {
            // Same guard on all three paths: the native session is already
            // gone (concurrent close destroyed it). Starting a render thread
            // would error-loop against a missing native session; the entry
            // is being removed by that close path anyway. Reset the intent
            // flags and refresh the UI state so nothing observes a
            // running-but-dead replacement in the meantime. closing=true is
            // defensive: if the native-side semantics ever change (new
            // destroy paths), the entry cannot become a frozen zombie that
            // the monitor skips forever.
            LogUtil.w(
                "Runtime",
                "$caller: native switchSession returned false for session $newId — skipping render start",
            )
            replacement.running = false
            replacement.closing = true
            updateState()
            return
        }
        // Restart unconditionally: a still-alive old thread is exiting;
        // startRenderThread interrupts+joins it and forces a fresh one.
        renderSupervisor.startRenderThread(replacement)
        if (syncGrid) {
            bridge.let { syncGridDimensions(it) }
        }
        // The replacement became active while the window is focused;
        // re-send focus-in so DECSET 1004 TUIs resume.
        if (lastWindowFocus && !replacement.closing) {
            bridge.focusEvent(true)
        }
        if (replacement.closing) {
            // A concurrent closeSession/closeDeadSession won the race for
            // the replacement; startRenderThread refused (closing flag) and
            // reset running=false. The entry will be removed by that close
            // path.
            LogUtil.d("Runtime", "$caller: replacement session $newId is closing — render start skipped")
        } else {
            LogUtil.d("Runtime", "$caller: restarted render for new active session $newId")
        }
    }

    private fun syncGridDimensions(bridge: Bridge) {
        val packed = bridge.getGridRowsColsPacked()
        val rows = (packed shr 32).toInt()
        val cols = packed.toInt()
        // Only overwrite cell metrics with valid values: the current bridge
        // returns 0f/0 (ADR-0007 stubs) and writing those would clobber the
        // real metrics computed from the surface.
        // Native metrics are in LOGICAL pixels (font pipeline units);
        // touch/anchor math in TerminalSurface works in physical pixels, so
        // scale by density here  — fixes long-press hit-testing
        // landing on wrong cells and the font-size mismatch reports).
        val density = context.resources.displayMetrics.density
        val rawCellWidth = bridge.getCellWidth()
        val rawCellHeight = bridge.getCellHeight()
        // Logical pixel dimensions (for grid computation): raw native values
        if (rawCellWidth > 0f) logicalCellWidth = rawCellWidth
        if (rawCellHeight > 0f) logicalCellHeight = rawCellHeight
        // Physical pixel dimensions (for rendering/touch): density-scaled
        val newCellWidth = rawCellWidth * density
        val newCellHeight = rawCellHeight * density
        if (newCellWidth > 0f) cellWidth = newCellWidth
        if (newCellHeight > 0f) cellHeight = newCellHeight
        // CAS with the size check INSIDE the lambda: the old code
        // read rows/cols outside the update, so a concurrent title CAS could
        // land between the check and the write. rows/cols are a snapshot
        // read from the bridge BEFORE the CAS: on a retry they may overwrite
        // a newer size with a slightly stale one — inherent to read-outside-
        // CAS, self-heals on the next sync, and strictly narrower than the
        // old lock-outside read-check-write.
        _state.update { previous ->
            if (rows > 0 && cols > 0 && (rows != previous.rows || cols != previous.cols)) {
                previous.copy(rows = rows, cols = cols)
            } else {
                previous
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4: State & surface lifecycle
    // ══════════════════════════════════════════════════════════════════════

    private fun updateState() {
        val currentTitle =
            sessions[activeSessionId]?.bridge?.getActiveSessionTitle() ?: _state.value.title
        // _state.update (CAS) instead of a read-modify-write assignment: the
        // render thread's title CAS could land between our read and write
        // and be overwritten by a stale title (self-heals next cycle, but
        // the CAS avoids the regression entirely). The map reads inside the
        // lambda go through the ConcurrentHashMap (weakly consistent, no
        // lock needed); callers that hold sessionLock get structural
        // serialization with session create/close as a bonus. currentTitle
        // itself is a snapshot read before the CAS: two concurrent
        // updateState calls can still write a stale title over a newer one
        // (same read-outside-CAS class as syncGridDimensions, self-heals).
        _state.update { previous ->
            previous.copy(
                isRunning = sessions.isNotEmpty(),
                title = currentTitle.ifEmpty { previous.title },
                activeSessionId = activeSessionId,
                sessionIds = sessions.keys.sorted(),
            )
        }
    }

    fun onSurfaceDestroyed() {
        // A pending surface (start/attachSurface before the bridge existed)
        // is stale the moment the holder is destroyed — attaching it later
        // would hand the new bridge a dead Surface and render black frames.
        pendingSurface = null
        setRenderPaused(true)
    }

    /**
     * Toggle the native renderer pause flag for every session. The flag lives on the single global
     * renderer (ffi.rs setRenderPaused), so pausing any session pauses the shared GPU pipeline.
     *
     * The flag must be cleared on the surface-restore path (TerminalSurface.surfaceCreated) — nothing
     * else does, and while it stays set, render_frame short-circuits with Ok(()) and the restored
     * session renders pure black frames with a healthy-looking render thread (no errors, no restart).
     */
    fun setRenderPaused(paused: Boolean) {
        for (entry in sessions.values) {
            entry.bridge?.setRenderPaused(paused)
        }
    }

    fun releaseAllGpuSurfaces() {
        // Surface destroyed: pause render threads cleanly instead of marking
        // them dead. Marking renderThreadExited on live threads makes the
        // monitor treat them as crashed and eventually close the sessions
        // after RENDER_MAX_RESTART_ATTEMPTS (~20s background).
        pauseRendering()
    }

    /**
     * Queue [action] on the same single-thread executor that stops render threads during
     * pauseRendering. Callers use this to release surfaces / views only AFTER the render thread's
     * join has confirmed it is no longer inside native render code — releasing an ANativeWindow while
     * the render thread still uses it is a use-after-free.
     */
    fun runAfterRenderThreadsStopped(action: () -> Unit) {
        surfaceTransitionExecutor.execute {
            synchronized(sessionLock) {
                action()
            }
        }
    }
}

/**
 * True when [file] starts with the ELF magic (0x7f 'E' 'L' 'F'). Used to exclude shebang scripts
 * from prefix-shell resolution: the linker-wrapper spawn path can only load real ELF binaries.
 * Top-level (not on the private companion) so installer/settings code can reuse it.
 */
internal fun isElf(file: java.io.File): Boolean = try {
    file.inputStream().use { input ->
        val magic = ByteArray(4)
        val read = input.read(magic)
        read == 4 &&
            magic[0] == 0x7f.toByte() &&
            magic[1] == 'E'.code.toByte() &&
            magic[2] == 'L'.code.toByte() &&
            magic[3] == 'F'.code.toByte()
    }
} catch (_: Exception) {
    false
}

//  fast-death recovery (warp WarpTerminalService.kt:906-915).
// Top-level (not on the private companion) so the unit tests can reach
// the pure decision/backoff logic.

/** A shell exiting within this window after spawn with no user input is treated as broken. */
internal const val FAST_DEATH_THRESHOLD_MS = 1500L

/** Bounded fast-death retries per session; exhausted → the exit is surfaced. */
internal const val MAX_FAST_DEATH_RETRIES = 3

/**
 * Fast-death decision (pure, unit-tested): the shell exited within [FAST_DEATH_THRESHOLD_MS] of
 * spawn, the user typed nothing, and the retry budget is not exhausted.
 */
internal fun shouldRetryFastDeath(
    aliveMs: Long,
    userTypedSinceSpawn: Boolean,
    fastDeathCount: Int,
): Boolean = aliveMs <= FAST_DEATH_THRESHOLD_MS &&
    !userTypedSinceSpawn &&
    fastDeathCount < MAX_FAST_DEATH_RETRIES

/**
 * Exponential backoff for retry [attempt] (1-based): 500ms << (attempt-1), capped at 5000ms (warp's
 * formula).
 */
internal fun fastDeathBackoffMs(attempt: Int): Long = minOf(500L shl (attempt - 1), 5000L)

/**
 * Wire payload sent to the native side for MCP run_command results (d1/d4: exit_code clamped to
 * 0..255 with -1 as the timeout sentinel; err_code: 0=ok, 1=timeout, 2=exception). Field names are
 * the cross-FFI contract — see Rust mcp/run_command parsing.
 */
@Serializable
internal data class RunCommandPayload(
    @SerialName("exit_code") val exitCode: Int,
    @SerialName("err_code") val errCode: Int,
    val stdout: String,
    val stderr: String,
)

internal fun runCommandPayload(
    exitCode: Int,
    errCode: Int,
    stdout: String,
    stderr: String,
): String {
    // Clamp exit_code to 0-255 per spec d4 (defense-in-depth; the caller
    // should already clamp, but this ensures the wire format is always valid).
    // POSIX exit codes wrap mod 256; preserve -1 (timeout sentinel).
    val clampedExit = if (exitCode == -1) -1 else exitCode and 0xFF
    return Json.encodeToString(
        RunCommandPayload(
            exitCode = clampedExit,
            errCode = errCode,
            stdout = stdout,
            stderr = stderr,
        ),
    )
}

// ── MCP run_command execution) ────────────────────────────

/** MCP run_command process timeout (seconds). */
private const val RUN_COMMAND_TIMEOUT_MS = 30_000L

/**
 * Grace period for draining stdout/stderr after the process exits or is killed. A grandchild that
 * inherited the pipe fds keeps the read open forever; bounding the drain keeps the caller from
 * hanging — previously the two `await()` calls were unbounded).
 */
private const val RUN_COMMAND_DRAIN_MS = 2_000L

/**
 * Result of [executeRunCommand]: captured streams, exit code and whether the process had to be
 * killed for exceeding the timeout.
 */
internal data class RunCommandResult(
    val stdout: String,
    val stderr: String,
    val exitCode: Int,
    val timedOut: Boolean,
)

/**
 * Execute `argv` as a child process with a bounded lifetime:
 * - `timeoutMs`: the process is force-killed when it does not exit in time (exitCode -1, timedOut
 *   true).
 * - `drainMs`: after the process exits (or is killed) the stdout/stderr readers get at most this
 *   long to reach EOF. A grandchild that inherited the pipe fds cannot hang the caller past this
 *   bound; the readers are cancelled and the streams come back empty.
 *
 * Pure JVM (no Android dependencies): unit-tested on the host.
 */
internal fun executeRunCommand(
    argv: List<String>,
    timeoutMs: Long = RUN_COMMAND_TIMEOUT_MS,
    drainMs: Long = RUN_COMMAND_DRAIN_MS,
    prefixDir: String? = null,
): RunCommandResult {
    // Android 15+ SELinux denies execve of app_data_file
    // binaries (execute_no_trans) for untrusted_app. termux-exec's
    // LD_PRELOAD hook wraps child execs for the shell, but run_command
    // spawns directly via ProcessBuilder — so a $PREFIX binary (echo,
    // coreutils applets, nix store paths) must be launched through the
    // system linker, exactly like PtyPair::spawn does on the native side.
    val wrapped = wrapTermuxExec(argv, prefixDir)
    val process =
        try {
            ProcessBuilder(wrapped).redirectErrorStream(false).start()
        } catch (e: java.io.IOException) {
            // d: process spawn failure (e.g. invalid binary) must
            // not crash the MCP server — return an error result instead.
            return RunCommandResult(
                stdout = "",
                stderr = "exec failed: ${e.message ?: e.toString()}\n",
                exitCode = 127, // 127 = "command not found" convention
                timedOut = false,
            )
        }
    // Readers run on daemon threads with a self-imposed deadline of
    // timeout+drain: they poll via ready() (never blocking on a read that
    // cannot be interrupted) so a grandchild inheriting the pipe fds can
    // at most delay the result until the deadline — it can never hang the
    // caller).
    val readBudgetMs = timeoutMs + drainMs
    val out = BoundedStreamRead(process.inputStream, readBudgetMs)
    val err = BoundedStreamRead(process.errorStream, readBudgetMs)
    try {
        val exited = process.waitFor(timeoutMs, TimeUnit.MILLISECONDS)
        if (!exited) {
            process.destroyForcibly()
        }
        // The join is bounded by the drain grace period — NOT by the full
        // read budget — so a grandchild holding the pipe open can delay
        // the result by at most drainMs. The reader threads keep polling
        // in the background and finish by their own deadline.
        val stdout = out.get(drainMs + 50)
        val stderr = err.get(50)
        val code = if (exited) process.exitValue() and 0xFF else -1
        return RunCommandResult(stdout, stderr, code, !exited)
    } finally {
        process.destroy()
        runCatching { process.inputStream.close() }
        runCatching { process.errorStream.close() }
    }
}

/**
 * Wrap a `$PREFIX` executable in the system linker when needed. Android 15+ SELinux denies
 * untrusted_app from exec'ing app_data_file binaries directly (`execute_no_trans`); the system
 * linker path is allowed because it only maps the file (`execute`). This mirrors the native
 * `PtyPair::spawn` SPAWN_LINKER logic.
 */
internal fun wrapTermuxExec(
    argv: List<String>,
    prefixDir: String?,
): List<String> {
    if (prefixDir.isNullOrBlank() || argv.isEmpty()) return argv
    val prefix = prefixDir.trimEnd('/')
    val executable = argv[0]
    if (!executable.startsWith("$prefix/")) return argv
    val linker =
        if (java.io.File("/system/bin/linker64").exists()) {
            "/system/bin/linker64"
        } else {
            "/system/bin/linker"
        }
    return listOf(linker, executable) + argv.drop(1)
}

/**
 * Reads [stream] on a daemon thread, appending into a shared buffer. The blocking read() returns as
 * soon as the write end of the pipe closes (process exit without descendants) — EOF is detected
 * naturally, unlike ready()-based polling (ready() returns false on EOF). When a grandchild holds
 * the pipe open, [get] joins with a bounded timeout and returns the partial output; the daemon
 * reader is then abandoned (it dies with the process), so the caller can never hang).
 */
private class BoundedStreamRead(
    private val stream: java.io.InputStream,
    private val budgetMs: Long,
) {
    private val sb = StringBuilder()

    private val thread: Thread =
        Thread(
            {
                val reader = java.io.BufferedReader(java.io.InputStreamReader(stream))
                val buf = CharArray(4096)
                val deadline = System.nanoTime() + budgetMs * 1_000_000L
                while (System.nanoTime() < deadline) {
                    val n = reader.read(buf)
                    if (n < 0) {
                        return@Thread
                    }
                    synchronized(sb) {
                        sb.append(buf, 0, n)
                    }
                }
            },
            "run-command-read",
        )
            .apply {
                isDaemon = true
                start()
            }

    /** Join the reader for at most [remainingMs]; returns what was read. */
    fun get(remainingMs: Long): String {
        thread.join(remainingMs)
        if (thread.isAlive) {
            // Best effort: a blocking read() may ignore close(), but the
            // daemon thread then exits with the process.
            runCatching { stream.close() }
        }
        return synchronized(sb) {
            sb.toString()
        }
    }
}

/**
 * Fast-death effective lifetime: prefer the native `alive_ms` measurement; when the respawn event
 * predates that field (<= 0) fall back to the Kotlin-side wall-clock since spawn. Pure so the
 * recovery decision is unit-testable.
 */
internal fun resolveAliveMs(aliveMs: Long, nowMs: Long, spawnedAtMs: Long): Long = if (aliveMs > 0) aliveMs else (nowMs - spawnedAtMs).coerceAtLeast(0)

/**
 * Next dead-render-thread restart backoff: double the previous delay up to [maxDelayMs] (the
 * exponential backoff in handleDeadRenderThread).
 */
internal fun nextRestartDelayMs(currentMs: Long, maxDelayMs: Long): Long = (currentMs * 2).coerceAtMost(maxDelayMs)

/**
 * Dead-render restart budget: attempt counter past [maxAttempts] means the session is closed
 * instead of restarted again.
 */
internal fun shouldCloseDeadRender(restartAttempts: Int, maxAttempts: Int): Boolean = restartAttempts > maxAttempts

/**
 * Fast-death respawn gate (re-checked under sessionLock inside tryFastDeathRecovery): a closing
 * session or an exhausted retry budget must NOT schedule another respawn, even when the earlier
 * shouldRetryFastDeath passed — the state may have changed while waiting.
 */
internal fun shouldScheduleFastDeathRetry(
    closing: Boolean,
    fastDeathCount: Int,
    maxRetries: Int,
): Boolean = !closing && fastDeathCount < maxRetries

/**
 * Initial synchronous render retry (switchSession): keep retrying while the first render result is
 * a failure (< 0) and the attempt counter is still below [maxAttempts]. Pure decision extracted
 * from the retry loop.
 */
internal fun initialRenderRetryNeeded(result: Int, attempts: Int, maxAttempts: Int): Boolean = result < 0 && attempts < maxAttempts

/**
 * switchSession Phase-3 concurrent-switch guard: while the first frame was rendering (outside
 * sessionLock), another switchSession may have published a different active session. Returns the id
 * of the session whose render thread must be stopped before publishing this switch, or null when
 * the active session did not change under us (or is this very target).
 */
internal fun concurrentRenderThreadToStop(
    activeSessionIdAfterRender: Long?,
    previousActiveId: Long?,
    targetId: Long,
    concurrentSessionId: Long?,
): Long? {
    if (activeSessionIdAfterRender == previousActiveId) return null
    val concurrentId = concurrentSessionId ?: return null
    if (concurrentId == targetId) return null
    return concurrentId
}

/**
 * switchSession Phase-1 failure restore: after a failed spawn the previous active session is
 * restarted unless it is the session that just failed.
 */
internal fun shouldRestorePreviousSession(previousId: Long?, failedTargetId: Long): Boolean = previousId != null && previousId != failedTargetId

/**
 * Pure grid-dimension computation behind recomputeGridFromFontMetrics: cols = floor(surfaceWidth /
 * cellWidth), rows = floor(surfaceHeight / cellHeight), each clamped to ≥ 1 so a tiny surface still
 * yields a usable grid. Returns (0, 0) for degenerate input (non-positive surface or cell metrics)
 * so callers can distinguish "invalid geometry" from a real one-cell grid and keep their current
 * dimensions instead.
 */
internal fun computeGridDimensions(
    surfaceWidth: Int,
    surfaceHeight: Int,
    cellWidth: Float,
    cellHeight: Float,
): Pair<Int, Int> {
    if (surfaceWidth <= 0 || surfaceHeight <= 0 || cellWidth <= 0f || cellHeight <= 0f) {
        return Pair(0, 0)
    }
    val cols = (surfaceWidth / cellWidth).toInt().coerceAtLeast(1)
    val rows = (surfaceHeight / cellHeight).toInt().coerceAtLeast(1)
    return Pair(rows, cols)
}
