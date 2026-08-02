package terminal.emulator.runtime

import android.content.Context
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Handler
import android.os.Looper
import android.view.Surface
import androidx.compose.ui.graphics.Color
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
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.BridgeTheme
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.Shell
import terminal.emulator.bridge.TerminalConfig
import terminal.emulator.bridge.createBridge
import terminal.emulator.monitor.RenderWatchDog
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.BuiltInThemes
import java.util.concurrent.ConcurrentHashMap
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
 * Session state is encoded in two booleans:
 * - running=true, renderThreadExited=false  → alive
 * - running=true, renderThreadExited=true   → dead (needs cleanup)
 * - running=false, renderThreadExited=*     → stopped (stale entry; skip)
 * renderThreadExited is set by the render thread after loop exit;
 * always read under sessionLock alongside running.
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
    @Volatile var nextRestartDelayMs: Long = 200L,
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

    val renderSignaled =
        java.util.concurrent.atomic
            .AtomicBoolean(false)

    @Volatile var forceRenderRequested: Boolean = false

    @Volatile var lastRenderStart: Long = 0L

    @Volatile var lastRenderDone: Long = 0L
    var renderWatchDog: RenderWatchDog? = null

    @Volatile var lastSignalNanos: Long = System.nanoTime()

    fun notifyRender() {
        lastSignalNanos = System.nanoTime()
        renderSignaled.set(true)
        renderThreadRef?.let {
            java.util.concurrent.locks.LockSupport
                .unpark(it)
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

    init {
        // Warm up the bell ToneGenerator off the render thread: the first
        // bell would otherwise construct it (AudioManager connect) inline
        // in the render loop, stalling a frame. The lazy is thread-safe;
        // whoever touches it first wins, and failure is harmless (the lazy
        // simply retries on the next bell).
        Thread { bellToneGenerator }.apply {
            isDaemon = true
            name = "BellToneWarmUp"
            start()
        }
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow(RuntimeState())
    val state: StateFlow<RuntimeState> = _state.asStateFlow()

    private val sessions = ConcurrentHashMap<Long, SessionEntry>()

    // MCP dialog / file-pick requests from the embedded MCP server.
    // Wired by the UI layer (e.g. MainActivity) via setDialogRequestHandler;
    // responses are sent back through NativeBridge.dialogResult.
    @Volatile
    var dialogRequestHandler:
        ((sessionId: Long, requestId: Long, dialogType: String, title: String, message: String, options: List<String>) -> Unit)? =
        null

    @Volatile
    var pickFileRequestHandler:
        ((sessionId: Long, requestId: Long, startingPath: String, filter: String) -> Unit)? =
        null

    @Volatile var accentColor: Int = 0xFF2196F3.toInt()

    @Volatile var selectionBgColor: Int = 0xFF45475A.toInt()

    @Volatile var cellWidth: Float = 0f

    @Volatile var cellHeight: Float = 0f

    private val renderGeneration =
        java.util.concurrent.atomic
            .AtomicInteger(0)

    @Volatile private var activeSessionId: Long = 0L

    @Volatile private var starting = false
    private val sessionLock = Any()

    /**
     * Serialises surface lifecycle transitions (pause/resume). Both
     * [pauseRendering] and [resumeRendering] run on this single thread so
     * that surface-destroy → surface-available ordering is preserved;
     * running them on different threads can leave a fresh surface without
     * a render thread (async pause stopping a just-started resume).
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

    private fun handleSessionExit(entry: SessionEntry, exitCode: Int) {
        LogUtil.i("Runtime", "session ${entry.id} exited with code $exitCode")
        // Phase 1 (locked): capture the possibly-hung thread; the actual
        // join runs UNLOCKED below (up to THREAD_JOIN_TIMEOUT_MS) so a hung
        // GPU thread does not stall every session operation.
        val hungThreadToJoin: Thread?
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            entry.running = false
            hungThreadToJoin =
                if (entry.renderThreadPossiblyAlive) entry.hungRenderThread else null
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
                LogUtil.e("Runtime", "session ${entry.id} render thread possibly alive — skipping bridge close on exit")
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
                    activateReplacementSession(activeSessionId, "handleSessionExit", markRunning = false, withRetry = true, syncGrid = false)
                }
            }
            updateForegroundSessionCount(sessions.size)
            updateState()
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
        // LogUtil.e already mirrors to LogcatFileWriter — no duplicate write.
        LogUtil.e("Runtime", "session ${entry.id} exceeded max restart attempts, closing session")
        try {
            if (entry.renderThreadPossiblyAlive) {
                // The hung thread may still be inside native render code;
                // destroying the session under it is a use-after-free.
                // Leave the native session to be reclaimed at process death.
                LogUtil.e("Runtime", "session ${entry.id} render thread possibly alive — skipping bridge close")
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
                    activateReplacementSession(activeSessionId, "closeDeadSession", markRunning = false, withRetry = true, syncGrid = false)
                }
            }
        } // synchronized(sessionLock)
        updateState()
    }

    private fun startForegroundServiceIfNeeded() {
        if (!foregroundServiceRunning) {
            terminal.emulator.service.TerminalForegroundService
                .start(context)
            foregroundServiceRunning = true
            LogUtil.d("Runtime", "foreground service started")
        }
    }

    private fun stopForegroundService() {
        // NO flag gate (round-95): MainActivity.onCreate starts the service
        // directly via the static TerminalForegroundService.start() without
        // setting this flag, so a flag-gated stop would leak the service and
        // its PARTIAL_WAKE_LOCK when the Activity is destroyed before the
        // runtime's bootstrap completes. stopService on a non-running
        // service is a harmless no-op (returns false), so stop unconditionally
        // and reset the flag.
        val stopped = try {
            terminal.emulator.service.TerminalForegroundService
                .stop(context)
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
     * Update the foreground service session count, keeping the
     * [foregroundServiceRunning] flag in sync: the service stops itself
     * when the count reaches 0 (TerminalForegroundService.updateSessionCount
     * calls stop()), so the flag MUST be cleared here or a later
     * startForegroundServiceIfNeeded would skip restarting the service —
     * no foreground notification, no wake lock, and background sessions can
     * be killed.
     *
     * Exception-safe: called from inside sessionLock on the close paths;
     * a service exception must never escape the lock (it would skip
     * updateState and corrupt the session bookkeeping).
     */
    private fun updateForegroundSessionCount(count: Int) {
        if (count <= 0) {
            foregroundServiceRunning = false
        }
        try {
            terminal.emulator.service.TerminalForegroundService
                .updateSessionCount(context, count)
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
    )

    private val selectionState =
        java.util.concurrent.atomic.AtomicReference(
            SelectionStateSnapshot(0, 0, 0, 0, false, 0),
        )

    fun setScrollOffset(offset: Int) {
        val entry = sessions[activeSessionId] ?: return
        entry.scrollOffset = offset
        // The render thread already reads entry.scrollOffset and calls
        // bridge.setScrollOffset() under the surface lock, so calling it here
        // would be a redundant JNA round-trip + surface-lock acquisition on the
        // calling thread (often the UI thread during scroll). Just signal the
        // render thread to pick up the change.
        entry.notifyRender()
    }

    fun forceRender() {
        val entry = sessions[activeSessionId] ?: return
        entry.forceRenderRequested = true
        entry.notifyRender()
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 3: Session lifecycle
    // ══════════════════════════════════════════════════════════════════════

    private suspend fun buildConfig(
        rows: Int = 24,
        cols: Int = 80,
    ): TerminalConfig {
        val configReads =
            coroutineScope {
                val shellDeferred = async { settingsRepository.shell.first() }
                val scrollbackDeferred = async { settingsRepository.scrollbackLines.first() }
                val fontDeferred = async { computeFontSizeTenths() }
                val themeDeferred = async { resolveThemeName() }
                ConfigReads(
                    shellPath = shellDeferred.await(),
                    scrollbackLines = scrollbackDeferred.await(),
                    fontSizeTenths = fontDeferred.await(),
                    themeName = themeDeferred.await(),
                )
            }
        val resolvedTheme = BuiltInThemes.byName(configReads.themeName)
        val shell = resolveShell(configReads.shellPath)
        val bridgeTheme = makeBridgeTheme(resolvedTheme)
        accentColor = bridgeTheme.ansi5
        selectionBgColor = bridgeTheme.selectionBg
        val prefixDir = java.io.File(context.filesDir, "bootstrap/usr").absolutePath
        val homeDir = java.io.File(context.filesDir, "home").absolutePath
        val bashFile = java.io.File("$prefixDir/bin/bash")
        val bashComplete =
            bashFile.exists() &&
                java.io.File("$prefixDir/lib").isDirectory &&
                java.io.File("$prefixDir/etc").isDirectory
        val effectivePrefix = if (bashComplete) prefixDir else ""
        val effectiveShell = if (bashComplete) Shell.Custom("$prefixDir/bin/bash") else shell
        val effectiveHome =
            if (bashComplete) {
                homeDir
            } else {
                java.io
                    .File(context.filesDir, "home")
                    .apply {
                        if (!exists() && !mkdirs()) {
                            LogUtil.w("Runtime", "Failed to create home directory: $this")
                        }
                    }.absolutePath
            }
        val effectivePath: String =
            if (bashComplete) {
                "$prefixDir/bin:${System.getenv("PATH").orEmpty().ifEmpty { "/system/bin:/system/xbin" }}"
            } else {
                System.getenv("PATH").orEmpty().ifEmpty { "/system/bin:/system/xbin" }
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
        )
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2b: Render-thread supervision (extracted C6)
    // ══════════════════════════════════════════════════════════════════════

    /**
     * Owns render-thread lifecycle: monitor loop, dead-thread detection,
     * restart/backoff, and thread start/stop.
     *
     * Extracted from TerminalRuntime (kotlin-architecture-deepening C6) so
     * the supervision logic has one home and the orchestrator stays thin.
     * Inner class: accesses TerminalRuntime's session registry/locks without
     * threading them through constructors (pure code move, zero behavior
     * change).
     */
    inner class RenderSupervisor {
        internal fun startRenderMonitor() {
            synchronized(monitorLock) {
                if (renderMonitorJob?.isActive == true) return
                renderMonitorJob =
                    scope.launch {
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
            if (entry.restartAttempts > RENDER_MAX_RESTART_ATTEMPTS) {
                closeDeadSession(entry)
                return
            }
            val d =
                synchronized(sessionLock) {
                    val next = entry.nextRestartDelayMs
                    entry.nextRestartDelayMs = (entry.nextRestartDelayMs * 2).coerceAtMost(MAX_RESTART_DELAY_MS)
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
            // reconfigure the surface via setNativeWindow/updateNativeWindow.
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
                LogUtil.d("Runtime", "session ${entry.id} render thread restarted (attempt ${entry.restartAttempts})")
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
                    LogUtil.w("Runtime", "session ${entry.id} startRenderThread called from its own render thread — skipping join")
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
                Thread({
                    var diagCount = 0
                    var consecutiveErrors = 0
                    var lastScrollOffset = Int.MAX_VALUE
                    var lastSelection = SelectionStateSnapshot(0, 0, 0, 0, false, 0)
                    LogUtil.d("Runtime", "render thread started for session ${entry.id} generation=$generation")
                    // First iteration always processes ghostty output. Subsequent
                    // iterations skip output on blink/force-rendered frames to avoid
                    // the ~50ms per-frame ghostty tick when there's no new PTY data.
                    var shouldSkipOutput = false
                    while (entry.running && renderGeneration.get() == generation) {
                        try {
                            val bridge = entry.bridge ?: break
                            val selectionSnapshot = selectionState.get()
                            if (selectionSnapshot != lastSelection) {
                                bridge.setSelection(
                                    selectionSnapshot.startRow,
                                    selectionSnapshot.startCol,
                                    selectionSnapshot.endRow,
                                    selectionSnapshot.endCol,
                                    selectionSnapshot.hasSelection,
                                    selectionSnapshot.mode,
                                )
                                lastSelection = selectionSnapshot
                            }
                            val currentScrollOffset = entry.scrollOffset
                            if (currentScrollOffset != lastScrollOffset) {
                                bridge.setScrollOffset(currentScrollOffset)
                                lastScrollOffset = currentScrollOffset
                            }
                            entry.lastRenderStart = System.nanoTime()
                            // render() is a stub returning 0 (ADR-0007, no native
                            // render export): the count<0 transient-error branch
                            // below is currently unreachable. It is kept so the
                            // loop is correct the day a real render export lands.
                            val count = bridge.render(shouldSkipOutput)
                            if (count < 0) {
                                // Transient render error (surface not ready, snapshot unavailable, etc.)
                                // These resolve on their own; don't count them toward the fatal limit.
                                if (consecutiveErrors == 0) {
                                    LogUtil.w("Runtime", "session ${entry.id} transient render error code=$count")
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
                                val sleepMs = if (consecutiveErrors > 10) RENDER_ERROR_BACKOFF_MS else RENDER_ERROR_SLEEP_MS
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
                                                NativeBridge
                                                    .dialogResult(request.sessionId, request.requestId, "")
                                            }
                                            poll.pickFiles.forEach { request ->
                                                NativeBridge
                                                    .dialogResult(request.sessionId, request.requestId, "")
                                            }
                                            poll.clipboardReads.forEach { request ->
                                                try {
                                                    val text = clipboardAccess.clipboardText().orEmpty()
                                                    NativeBridge
                                                        .clipboardResult(request.sessionId, request.requestId, text)
                                                } catch (exception: Exception) {
                                                    // Class only: exception messages can embed clipboard text (round-108).
                                                    LogUtil.e("Runtime", "clipboard_read request dispatch failed: ${exception.javaClass.simpleName}")
                                                    NativeBridge
                                                        .clipboardResult(request.sessionId, request.requestId, "")
                                                }
                                            }
                                            poll.clipboardGets.forEach { request ->
                                                NativeBridge
                                                    .clipboardResult(request.sessionId, request.requestId, "")
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
                                            val exitedEntry = synchronized(sessionLock) { sessions[poll.sessionId] }
                                            if (exitedEntry != null) {
                                                LogUtil.i(
                                                    "Runtime",
                                                    "reaping background session ${poll.sessionId} (exit ${poll.exitCode})",
                                                )
                                                handleSessionExit(exitedEntry, poll.exitCode)
                                            }
                                        } else {
                                            // Full cleanup (bridge close, session removal,
                                            // state update) happens here; the render monitor
                                            // skips !running entries so it would never reap
                                            // an exited session.
                                            handleSessionExit(entry, poll.exitCode)
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
                                                val extra = synchronized(sessionLock) { sessions[exitInfo.sessionId] }
                                                if (extra != null) {
                                                    LogUtil.i(
                                                        "Runtime",
                                                        "reaping same-frame exited session ${exitInfo.sessionId} (exit ${exitInfo.exitCode})",
                                                    )
                                                    handleSessionExit(extra, exitInfo.exitCode)
                                                }
                                            }
                                        }
                                        break
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
                                if (!entry.forceRenderRequested) {
                                    val idleNanos = System.nanoTime() - entry.lastSignalNanos
                                    val timeoutNanos =
                                        if (idleNanos > RENDER_IDLE_THRESHOLD_NANOS) {
                                            RENDER_LATCH_IDLE_TIMEOUT_NANOS
                                        } else {
                                            RENDER_LATCH_TIMEOUT_NANOS
                                        }
                                    if (!entry.renderSignaled.get() && !entry.forceRenderRequested) {
                                        val timeoutMs = timeoutNanos / 1_000_000L
                                        bridge.waitOutput(timeoutMs)
                                        if (Thread.interrupted()) throw InterruptedException()
                                    }
                                    shouldSkipOutput = false
                                    entry.renderSignaled.set(false)
                                } else {
                                    entry.forceRenderRequested = false
                                    shouldSkipOutput = true
                                }
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
                                LogUtil.e("Runtime", "session ${entry.id} first render exception", exception)
                            } else if (consecutiveErrors % RENDER_ERROR_LOG_FREQUENCY == 0) {
                                LogUtil.e("Runtime", "session ${entry.id} render exception (x$consecutiveErrors)", exception)
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
                }, "Render-${entry.id}").apply {
                    isDaemon = true
                }
            entry.renderThreadRef = renderThread
            renderThread.start()
            entry.renderWatchDog =
                RenderWatchDog(
                    getStart = { entry.lastRenderStart },
                    getDone = { entry.lastRenderDone },
                    isRunning = { entry.running && !entry.renderThreadExited && activeSessionId == entry.id },
                    onHangDetected = {
                        LogUtil.e("Runtime", "session ${entry.id} render thread hung (>${RENDER_HANG_TIMEOUT_NANOS / 1_000_000L}s)")
                        LogcatFileWriter.write(
                            "Runtime",
                            "session ${entry.id} render hang detected — marking thread for restart",
                        )
                        // Mark the thread as dead. The render monitor (checkSessions)
                        // will detect this and restart the thread with exponential backoff.
                        // This avoids killing the entire process for a GPU hang.
                        entry.renderThreadExited = true
                    },
                    hangTimeoutNanos = RENDER_HANG_TIMEOUT_NANOS,
                ).also { it.start() }
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
                    LogUtil.e("Runtime", "session ${entry.id} render thread still alive after join — possibly hung")
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
     * Dispatches non-exit poll events (bell, notification, clipboard,
     * MCP dialogs/pick-file, toast, open-url, clipboard_get replies).
     *
     * Extracted from the render loop (kotlin-architecture-round2 K3) so the
     * loop body stays a tight poll → handle → wait cycle. Inner class:
     * accesses TerminalRuntime's handlers/context/clipboard without
     * threading them through constructors. Exit reaping stays in the loop
     * (it owns the break/cleanup control flow).
     */
    inner class EventDispatcher {
        /**
         * Handle all non-exit events in [poll]. Called from the render loop
         * after exit handling. Exceptions here must never skip the loop's
         * per-frame bookkeeping (caller wraps us in the outer try).
         */
        fun handle(poll: terminal.emulator.bridge.Bridge.PollResult) {
            if (poll.bel) {
                bellToneGenerator.startTone(BEL_TONE_TYPE, BEL_TONE_DURATION_MILLIS)
            }
            if (poll.notification != null) {
                val (title, body) = poll.notification
                val toastText = if (title.isNotEmpty()) "$title: $body" else body
                Handler(Looper.getMainLooper()).post {
                    android.widget.Toast
                        .makeText(context, toastText, android.widget.Toast.LENGTH_LONG)
                        .show()
                }
                terminal.emulator.ui
                    .TerminalNotificationHelper(context)
                    .showNotification(title, body)
            }
            if (poll.clipboard != null) {
                clipboardAccess.setClipboardText(poll.clipboard)
            }
            poll.dialogs.forEach { request ->
                try {
                    LogUtil.i("Runtime", "MCP dialog request session=${request.sessionId} type=${request.dialogType}")
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
                        // No handler (activity destroyed window):
                        // the event is already consumed, so reply
                        // with an empty result instead of leaving
                        // the native MCP tool call hanging.
                        LogUtil.w(
                            "Runtime",
                            "MCP dialog request dropped (no handler), replying empty",
                        )
                        NativeBridge
                            .dialogResult(request.sessionId, request.requestId, "")
                    }
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "dialog request dispatch failed", exception)
                    NativeBridge
                        .dialogResult(request.sessionId, request.requestId, "")
                }
            }
            poll.pickFiles.forEach { request ->
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
                        NativeBridge
                            .dialogResult(request.sessionId, request.requestId, "")
                    }
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "pick_file request dispatch failed", exception)
                    NativeBridge
                        .dialogResult(request.sessionId, request.requestId, "")
                }
            }
            poll.toastText?.let { text ->
                Handler(Looper.getMainLooper()).post {
                    android.widget.Toast
                        .makeText(context, text, android.widget.Toast.LENGTH_SHORT)
                        .show()
                }
            }
            poll.openUrl?.let { url ->
                try {
                    val intent =
                        android.content.Intent(
                            android.content.Intent.ACTION_VIEW,
                            android.net.Uri.parse(url),
                        )
                    intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
                    context.startActivity(intent)
                } catch (exception: Exception) {
                    // Never log the URL or the exception
                    // stack: both may carry token/query
                    // parameters and LogUtil writes the
                    // persistent log file unconditionally
                    // (round-103).
                    LogUtil.e("Runtime", "open_url failed: ${exception.javaClass.simpleName}")
                }
            }
            poll.clipboardReads.forEach { request ->
                try {
                    val text = clipboardAccess.clipboardText().orEmpty()
                    NativeBridge
                        .clipboardResult(request.sessionId, request.requestId, text)
                } catch (exception: Exception) {
                    // Class only: exception messages can embed clipboard text (round-108).
                    LogUtil.e("Runtime", "clipboard_read request dispatch failed: ${exception.javaClass.simpleName}")
                    NativeBridge
                        .clipboardResult(request.sessionId, request.requestId, "")
                }
            }
            poll.clipboardGets.forEach { request ->
                try {
                    val text = clipboardAccess.clipboardText().orEmpty()
                    NativeBridge
                        .clipboardResult(request.sessionId, request.requestId, text)
                } catch (exception: Exception) {
                    // Class only: exception messages can embed clipboard text (round-108).
                    LogUtil.e("Runtime", "clipboard_get request dispatch failed: ${exception.javaClass.simpleName}")
                    NativeBridge
                        .clipboardResult(request.sessionId, request.requestId, "")
                }
            }
        }
    }

    private companion object {
        private const val TENTHS_PER_UNIT = 10
        private const val FONT_SIZE_DISPLAY_RATIO = 0.6f
        private const val FONT_SIZE_MIN_PX = 300
        private const val FONT_SIZE_MAX_PX = 600
        private const val FONT_SIZE_HEIGHT_RATIO = 0.5f
        private const val FONT_SIZE_HEIGHT_MIN_PX = 250
        private const val FONT_SIZE_HEIGHT_MAX_PX = 500
        private const val RENDER_ERROR_LOG_FREQUENCY = 60
        private const val RENDER_MAX_CONSECUTIVE_ERRORS = 100
        private const val RENDER_MAX_TRANSIENT_ERRORS = 50 // ~2.5s of transient errors before thread exit
        private const val RENDER_ERROR_SLEEP_MS = 50L
        private const val RENDER_ERROR_BACKOFF_MS = 200L // Longer sleep after 10 consecutive transient errors
        private const val RENDER_LATCH_TIMEOUT_NANOS = 16_000_000L // 16ms for active (~60 FPS)
        private const val RENDER_LATCH_IDLE_TIMEOUT_NANOS = 500_000_000L // 500ms for idle (~2 FPS)
        private const val RENDER_IDLE_THRESHOLD_NANOS = 5_000_000_000L // 5s idle → switch to low-freq
        private const val RENDER_DIAGNOSTIC_FREQUENCY = 60
        private const val THREAD_JOIN_TIMEOUT_MS = 1000L
        private const val BEL_TONE_STREAM_TYPE = AudioManager.STREAM_NOTIFICATION
        private const val BEL_TONE_VOLUME = 50
        private const val BEL_TONE_TYPE = ToneGenerator.TONE_PROP_ACK
        private const val BEL_TONE_DURATION_MILLIS = 200
        private const val RENDER_HANG_TIMEOUT_NANOS = 10_000_000_000L // 10 seconds
        private const val RENDER_INITIAL_RETRY_MAX = 5
        private const val RENDER_INITIAL_RETRY_DELAY_MS = 150L

        // Render monitor — proactive death detection
        private const val RENDER_MONITOR_INTERVAL_MS = 500L
        private const val RENDER_MAX_RESTART_ATTEMPTS = 5
        private const val INITIAL_RESTART_DELAY_MS = 100L
        private const val MAX_RESTART_DELAY_MS = 1000L
        private const val GRACE_PERIOD_AFTER_RESTART_MS = 300L

        /** Cached ToneGenerator for bell — avoids per-event allocation. */
        private val bellToneGenerator by lazy {
            ToneGenerator(BEL_TONE_STREAM_TYPE, BEL_TONE_VOLUME)
        }
    }

    private data class ConfigReads(
        val shellPath: String,
        val scrollbackLines: Int,
        val fontSizeTenths: Int,
        val themeName: String,
    )

    internal suspend fun computeFontSizeTenths(): Int {
        val userFontSize = settingsRepository.fontSize.first()
        val density = context.resources.displayMetrics.density
        val cellHeightPixels = userFontSize * density
        return (cellHeightPixels * TENTHS_PER_UNIT.toFloat()).toInt()
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

    private fun makeBridgeTheme(resolvedTheme: terminal.emulator.ui.theme.TerminalTheme): BridgeTheme {
        fun colorToInt(color: androidx.compose.ui.graphics.Color): Int = ((color.alpha * 255).toInt() shl 24) or
            ((color.red * 255).toInt() shl 16) or
            ((color.green * 255).toInt() shl 8) or
            (color.blue * 255).toInt()
        val backgroundColor = colorToInt(resolvedTheme.background)
        val foregroundColor = colorToInt(resolvedTheme.foreground)
        val cursor = colorToInt(resolvedTheme.cursor)
        val ansiInts = resolvedTheme.ansi.map(::colorToInt)
        val resolvedSelectionBg = if (resolvedTheme.selectionBg == Color.Transparent) Color(0xFF45475A) else resolvedTheme.selectionBg
        return BridgeTheme(
            name = resolvedTheme.name,
            bg = backgroundColor,
            fg = foregroundColor,
            cursor = cursor,
            selectionBg = colorToInt(resolvedSelectionBg),
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
        // LogUtil.d already mirrors to LogcatFileWriter — no duplicate write.
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
        val displayW = context.resources.displayMetrics.widthPixels
        val displayH = context.resources.displayMetrics.heightPixels
        val density = context.resources.displayMetrics.density
        LogUtil.d(
            "Runtime",
            "displayMetrics: w=$displayW h=$displayH density=$density",
        )

        val bypassMinSurface = System.getProperty("test.minSurface") != null

        if (!bypassMinSurface && (width <= 0 || height <= 0)) {
            LogUtil.e("Runtime", "start() called with non-positive dimensions, waiting for surfaceChanged")
            starting = false
            return
        }

        val minWidth = (displayW * FONT_SIZE_DISPLAY_RATIO).toInt().coerceIn(FONT_SIZE_MIN_PX, FONT_SIZE_MAX_PX)
        val minHeight = (displayH * FONT_SIZE_HEIGHT_RATIO).toInt().coerceIn(FONT_SIZE_HEIGHT_MIN_PX, FONT_SIZE_HEIGHT_MAX_PX)
        if (!bypassMinSurface && (width < minWidth || height < minHeight)) {
            LogUtil.w(
                "Runtime",
                "start() called with small surface ${width}x$height (display=${displayW}x$displayH min=${minWidth}x$minHeight), waiting for correct surfaceChanged",
            )
            starting = false
            return
        }

        var windowPointer = 0L
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
            // Allow test override via system property (no DataStore dependency)
            val testUrl = System.getProperty("test.bootstrapUrl")
            val bootstrapUrl = if (testUrl != null) testUrl else settingsRepository.bootstrapUrl.first()
            if (bootstrapUrl.isNotEmpty()) {
                // Log only the origin (scheme://host), never the full URL:
                // private bootstrap URLs can carry token/query parameters,
                // and LogUtil writes the persistent log file unconditionally
                // (round-101).
                val origin =
                    runCatching {
                        val uri = android.net.Uri.parse(bootstrapUrl)
                        val scheme = uri.scheme ?: return@runCatching "<no-scheme>"
                        val host = uri.host
                        if (host.isNullOrBlank()) return@runCatching "<no-host>"
                        "$scheme://$host"
                    }.getOrNull() ?: "<unparsable>"
                LogUtil.d("Runtime", "Bootstrap URL set: $origin")
                val downloader = terminal.emulator.installer.BootstrapDownloader(context)
                val installer =
                    terminal.emulator.installer.BootstrapInstaller(
                        prefixDir = java.io.File(context.filesDir, "bootstrap/usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        stagingDir = java.io.File(context.filesDir, "bootstrap/usr-staging"),
                    )
                val secondStage =
                    terminal.emulator.installer.SecondStageRunner(
                        prefixDir = java.io.File(context.filesDir, "bootstrap/usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                    )
                val orchestrator = terminal.emulator.installer.BootstrapOrchestrator(downloader, installer, secondStage)
                when (orchestrator.getInstallStatus()) {
                    terminal.emulator.installer.BootstrapOrchestrator.Status.NOT_INSTALLED -> {
                        LogUtil.d("Runtime", "Bootstrap not installed, starting install...")
                        val result = orchestrator.ensureBootstrap(bootstrapUrl)
                        // Result message may embed the full bootstrap URL on
                        // failure (downloader exceptions include it); log only
                        // the outcome class — the persistent log file must not
                        // capture private tokens (round-102).
                        val outcome = result.fold({ it }, { "failed: ${it.javaClass.simpleName}" })
                        LogUtil.d("Runtime", "Bootstrap result: $outcome")
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
            LogUtil.d("Runtime", "bridge created: ${bridge.ping()} elapsed=${(System.nanoTime() - bridgeStartNs) / 1_000_000}ms")

            bridge.setSystemLocale(
                java.util.Locale
                    .getDefault()
                    .toLanguageTag(),
            )
            LogUtil.d("Runtime", "setSystemLocale: ${java.util.Locale.getDefault().toLanguageTag()}")

            val fontsDir = context.filesDir.resolve("fonts")
            fontsDir.apply {
                if (!exists() && !mkdirs()) {
                    LogUtil.w("Runtime", "Failed to create fonts directory: $this")
                }
            }
            bridge.setExtraFontPaths(listOf(fontsDir.absolutePath))

            // The bootstrap download/install above can take minutes. The
            // surface passed into start() may have been destroyed during
            // that window (rotation, split-screen); its ANativeWindow
            // pointer is dangling. Spawning on it would render into a dead
            // window (black screen / hang). Abort and let the next
            // surface-available event retry start() with a fresh surface.
            if (surface != null && !surface.isValid) {
                LogUtil.e("Runtime", "start(): surface became invalid during bootstrap, aborting (will retry on next surface)")
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

            bridge.setNativeWindow(windowPointer, width, height)
            LogUtil.d("Runtime", "setNativeWindow OK: width=$width height=$height")

            val spawnStartNs = System.nanoTime()
            val spawnResult = bridge.spawnTerminal(config.rows, config.cols, bridge.shellPath())
            val spawnElapsedMs = (System.nanoTime() - spawnStartNs) / 1_000_000
            LogUtil.d(
                "Runtime",
                "spawnTerminal: rows=${config.rows} cols=${config.cols} result=$spawnResult elapsed=${spawnElapsedMs}ms",
            )
            if (spawnResult <= 0L) {
                LogUtil.e("Runtime", "spawnTerminal returned $spawnResult — native session init failed, aborting start")
                // Deliberate duplicate with a stable grep anchor (FAILED prefix).
                LogcatFileWriter.write("Runtime", "FAILED to spawn terminal: native init returned $spawnResult")
                bridge.close()
                return
            }

            try {
                val initialFontFamily = settingsRepository.fontFamily.first()
                val effectiveFont = terminal.emulator.resolveEffectiveFontFamily(initialFontFamily)
                bridge.setFontFamily(effectiveFont)
                bridge.setTheme(config.theme)
                val cursorStyle = settingsRepository.cursorStyle.first()
                bridge.setCursorStyle(cursorStyle)
                val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
                bridge.setCursorBlinkEnabled(cursorBlinkEnabled)
                val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
                bridge.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
                LogUtil.d(
                    "Runtime",
                    "settings applied: fontFamily=$effectiveFont theme=${config.theme.name} cursorStyle=$cursorStyle cursorBlink=$cursorBlinkEnabled cursorSpeed=$cursorBlinkSpeedMs",
                )
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.e("Runtime", "Failed to apply initial settings (continuing with defaults)", exception)
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
            val startedEntry = entry!!
            // First render: problematic GPUs (Mali-G57 w/ missing SURFACE_VIEW_FORMATS)
            // can hang get_current_texture() indefinitely. The previous approach of
            // spawning a daemon thread to call bridge.render() caused mutex starvation —
            // the daemon would acquire the surface Mutex and hang, permanently blocking
            // the real render thread. Instead, signal the render loop to produce the
            // first frame via forceRenderRequested, which will be picked up by the
            // real render thread once it starts.
            // ADR-0007: attach the surface now that the bridge exists.
            attachPendingSurface(bridge)
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
                    LogUtil.w("Runtime", "start: session $finalSessionId closed/stopped during startup, skipping render start")
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
                // thread's title CAS (round-91) would otherwise be overwritten
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
                    LogUtil.e("Runtime", "Failed to start foreground service for session $finalSessionId", serviceException)
                }
                renderSupervisor.startRenderThread(startedEntry)
                renderSupervisor.startRenderMonitor()
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
            // Deliberate duplicate: the file copy includes the full stack
            // trace (LogUtil truncates it) and a stable FAILED grep anchor.
            LogcatFileWriter.write("Runtime", "FAILED to start terminal: ${exception.message}\n${exception.stackTraceToString()}")
            // Any failure after createBridge() (settings, setNativeWindow,
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
     * Creates a new terminal session. Serialized against concurrent calls
     * (double-tap on the new-session button) and against [start]: two
     * concurrent creations would each spawn a native session and start a
     * render thread, and two render threads consuming the single global
     * event queue misroute events (exit events dropped, sessions leaked).
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
            LogUtil.w("Runtime", "createSession: start() in progress (bootstrap), refusing concurrent creation")
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
                java.util.Locale
                    .getDefault()
                    .toLanguageTag(),
            )

            try {
                val initialFontFamily = settingsRepository.fontFamily.first()
                val effectiveFont = terminal.emulator.resolveEffectiveFontFamily(initialFontFamily)
                bridge.setFontFamily(effectiveFont)
                bridge.setTheme(config.theme)
                val cursorStyle = settingsRepository.cursorStyle.first()
                bridge.setCursorStyle(cursorStyle)
                val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
                bridge.setCursorBlinkEnabled(cursorBlinkEnabled)
                val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
                bridge.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.e("Runtime", "Failed to apply settings to new session (continuing with defaults)", exception)
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
                // Structural map change under the lock (round-93 invariant);
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
            // section (round-93, symmetric with start()): serializes with
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
                        LogUtil.e("Runtime", "Failed to start foreground service for session $nextId", serviceException)
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
            // Deliberate duplicate: the file copy includes the full stack
            // trace (LogUtil truncates it) and a stable FAILED grep anchor.
            LogcatFileWriter.write(
                "Runtime",
                "FAILED to create session $nextId: ${exception.message}\n${exception.stackTraceToString()}",
            )
            // If the failure happened before the entry was inserted (settings
            // application, spawnTerminal throwing), the bridge was never
            // rolled back above — close it to avoid leaking the native
            // session and PTY child.
            createdBridge?.let { leaked ->
                if (leaked !== sessions[nextId]?.bridge) {
                    try {
                        leaked.close()
                    } catch (closeException: Exception) {
                        LogUtil.e("Runtime", "Failed to close leaked bridge during createSession rollback", closeException)
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
                sessions[id] ?: run {
                    LogUtil.e("Runtime", "switchSession: session $id not found")
                    return
                }
            // Capture before any stopping happens; used to restore the
            // previous session if the switch/spawn fails.
            previousActiveId = activeSessionId
            if (id == activeSessionId) return
            // ADR-0007: hand the Surface to the renderer (attachWindow JNI
            // extracts the ANativeWindow inside Rust).
            target.bridge?.attachSurface(surface, width, height)
            val windowPointer = 0L

            if (!surface.isValid) {
                LogUtil.e("Runtime", "switchSession: surface is no longer valid, aborting")
                return
            }

            val current = sessions[activeSessionId]
            if (current != null) {
                // NOTE (round-92/93): stopRenderThread joins the OLD render
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
                        LogUtil.e("Runtime", "switchSession: session ${current.id} render thread hung — skipping surface release")
                    }
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "switchSession: error stopping current session", exception)
                }
            }

            try {
                target.bridge?.setNativeWindow(windowPointer, width, height)
                // Always update the GPU surface after setNativeWindow.
                // If releaseGpuSurface was called on this bridge during a
                // previous session switch, the wgpu surface is gone and must
                // be recreated from the current ANativeWindow via updateNativeWindow.
                target.bridge?.updateNativeWindow(windowPointer, width, height)
                target.running = true
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "switchSession: setNativeWindow failed for session $id", exception)
                // Spawn failure: the previous active session was stopped and
                // its GPU surface released above. Restore it so the terminal
                // is not left frozen (running=false, no render thread, and
                // the monitor skips !running entries forever).
                val previous = sessions[previousActiveId]
                if (previous != null && previous.id != id) {
                    LogUtil.w("Runtime", "switchSession: restoring previous session ${previous.id} after failure")
                    previous.running = true
                    previous.renderThreadExited = false
                    previous.restartAttempts = 0
                    try {
                        previous.bridge?.setNativeWindow(windowPointer, width, height)
                        previous.bridge?.updateNativeWindow(windowPointer, width, height)
                        renderSupervisor.startRenderThread(previous)
                        activeSessionId = previous.id
                    } catch (restoreException: Exception) {
                        LogUtil.e("Runtime", "switchSession: failed to restore previous session ${previous.id}", restoreException)
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
            // spawnTerminal/setNativeWindow (a transient race on shared
            // ANativeWindow). Retry the first synchronous render a few times
            // with a short delay so we present real content instead of
            // starting a render thread on a not-yet-ready surface (which would
            // block and trip the hang watchdog).
            var initialRender = target.bridge?.render() ?: 0
            var attempts = 1
            while (initialRender < 0 && attempts < RENDER_INITIAL_RETRY_MAX) {
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
                LogUtil.w("Runtime", "switchSession: session $id was closed during first frame, aborting switch")
                return
            }
            // A concurrent switchSession may have published a different
            // active session while we were rendering the first frame. Its
            // render thread is running; stop it before starting ours so only
            // one render thread ever consumes the shared event queue (two
            // consumers misroute bell/clipboard/notification events).
            if (activeSessionId != previousActiveId) {
                val concurrent = sessions[activeSessionId]
                if (concurrent != null && concurrent !== target) {
                    try {
                        renderSupervisor.stopRenderThread(concurrent)
                        LogUtil.w(
                            "Runtime",
                            "switchSession: stopped concurrent session ${concurrent.id} (active changed during first frame)",
                        )
                    } catch (exception: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "switchSession: failed to stop concurrent session ${concurrent.id}",
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
                    LogUtil.e("Runtime", "switchSession: native switchSession failed for session $id", exception)
                }
                target.bridge?.let { syncGridDimensions(it) }
                // Round-108 stopped applySettings from resizing background
                // sessions, so the newly active session must be aligned to the
                // current window size here: syncGridDimensions reads the
                // native grid (an ADR-0007 stub returning 0) and can't tell us
                // the real dims, so resize unconditionally with the latest UI
                // state (round-109).
                target.bridge?.resize(_state.value.rows.coerceAtLeast(1), _state.value.cols.coerceAtLeast(1))
                LogUtil.d("Runtime", "switched to session $id")
                // DECSET 1004 focus reporting is per-window and the new
                // active session never received a focus-in while backgrounded.
                // Re-send the last known window focus state so a focused
                // TUI (vim/fzf) resumes its FocusGained behaviour.
                if (lastWindowFocus) {
                    target.bridge?.focusEvent(true)
                }
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "switchSession: failed to start render thread for session $id", exception)
            }
        }
    }

    /**
     * Single entry point for the Activity teardown path (onDestroy): stop
     * the foreground service when no session is running, or refresh the
     * notification count when sessions survive. Centralizes the service
     * lifecycle here instead of MainActivity calling the static methods
     * directly (which bypassed the runtime's foregroundServiceRunning
     * flag and its exception protection).
     *
     * The count read and the stop decision both happen inside sessionLock
     * (round-94): a createSession landing between the snapshot and the stop
     * would otherwise leave a live session without its foreground service.
     *
     * Round-96 note: if onDestroy runs while runtime.start() is mid-bootstrap
     * (no session inserted yet), this stops the service and start() later
     * re-inserts a session and re-starts the service. That is the intended
     * background-session semantics (Termux-style: sessions outlive the
     * Activity), not a leak — the final state is one live session + one
     * foreground service. A leak would only occur if start() FAILED after
     * the service restart, which its rollback (updateForegroundSessionCount(0))
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
        val entry = synchronized(sessionLock) {
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
                    activateReplacementSession(newId, "closeSession", markRunning = true, withRetry = false, syncGrid = true)
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
        // SIGWINCH + shell reflow (round-110).
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
        // Resize only the active session: background sessions keep their own
        // grid dimensions; resizing every session with the active one's
        // size would fire spurious SIGWINCH + reflow on them (round-108).
        sessions[activeSessionId]?.bridge?.resize(currentRows, currentCols)
    }

    suspend fun applyFontSettings() {
        val fontSizeTenths = computeFontSizeTenths()
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
                entry.bridge?.setFontSizeInPlace(fontSizeTenths)
                entry.bridge?.let { syncGridDimensions(it) }
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
            val written = entry.bridge?.writeToPty(data) ?: false
            entry.notifyRender()
            return written
        }
        LogUtil.w("Runtime", "writeToPty: no active running session to receive write")
        return false
    }

    fun bridge(): Bridge? = sessions[activeSessionId]?.bridge

    @Volatile
    private var lastWindowFocus: Boolean = false

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
        // Round-99 note: the per-session join happens INSIDE sessionLock
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
                // Inside the lock (round-93): serializing here means the
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
    ) {
        LogUtil.d("Runtime", "setSelection: start=($startRow,$startCol) end=($endRow,$endCol) active=$hasSelection mode=$mode")
        selectionState.set(SelectionStateSnapshot(startRow, startCol, endRow, endCol, hasSelection, mode))
        val entry = sessions[activeSessionId]
        entry?.bridge?.setSelection(startRow, startCol, endRow, endCol, hasSelection, mode)
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
        selectionState.set(SelectionStateSnapshot(start.first, start.second, end.first, end.second, true, mode))
        entry.notifyRender()
        return bounds
    }

    /**
     * Resize the active session's terminal grid. Values are clamped to the
     * native u16 range (1..=65535) BEFORE the bridge call so the PTY/MCP
     * dimensions and the UI state agree (round-107).
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
        // this state back to the native value (round-115). Note the
        // upper bound is the u16 protocol limit, NOT a display-size sanity
        // limit: the UI paths (window insets, applySettings) supply real
        // grid sizes, so a 65535×65535 grid can only be requested by a
        // direct API caller; native would attempt the allocation.
        val clampedRows = rows.coerceIn(1, 0xFFFF)
        val clampedCols = cols.coerceIn(1, 0xFFFF)
        entry.bridge?.resize(clampedRows, clampedCols)
        // CAS (round-94): a plain copy would overwrite a title update the
        // render thread published between read and write.
        _state.update { it.copy(rows = clampedRows, cols = clampedCols) }
        entry.notifyRender()
    }

    fun recomputeGrid(
        width: Int,
        height: Int,
    ) {
        val bridge = sessions[activeSessionId]?.bridge ?: return
        bridge.recomputeGrid(width, height)
        syncGridDimensions(bridge)
    }

    /**
     * Hand the Android Surface to the renderer (ADR-0007). If the session
     * bridge does not exist yet (spawn in progress), the surface is kept
     * pending and attached right after the session starts.
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
        bridge.attachSurface(surface, pendingSurfaceWidth, pendingSurfaceHeight)
    }

    fun updateNativeWindow(
        windowPointer: Long,
        width: Int,
        height: Int,
    ) {
        val entry = sessions[activeSessionId] ?: return
        try {
            entry.bridge?.updateNativeWindow(windowPointer, width, height)
            entry.bridge?.let { syncGridDimensions(it) }
            // Route restarts through the same single-thread executor as
            // pause/resume so startRenderThread can never race a concurrent
            // pauseRendering() (which would leave two render loops or a
            // black screen).
            if (entry.renderThreadRef?.isAlive != true) {
                LogUtil.d("Runtime", "updateNativeWindow: render thread dead, restarting for session ${entry.id}")
                surfaceTransitionExecutor.execute {
                    // Re-check membership: the session may have been closed
                    // between the enqueue and the executor running (close
                    // happens on the IO thread). Starting a render thread
                    // for a removed session would render into a destroyed
                    // native session (use-after-free).
                    synchronized(sessionLock) {
                        if (sessions[entry.id] === entry && entry.running && entry.bridge != null) {
                            renderSupervisor.startRenderThread(entry)
                        } else {
                            LogUtil.w("Runtime", "updateNativeWindow: session ${entry.id} no longer active, skipping render restart")
                        }
                    }
                }
            } else {
                entry.forceRenderRequested = true
                java.util.concurrent.locks.LockSupport
                    .unpark(entry.renderThreadRef)
            }
        } catch (exception: Exception) {
            LogUtil.e("Runtime", "updateNativeWindow failed", exception)
        }
    }

    /**
     * Activate [newId] as the foreground session after its predecessor
     * closed: final hung-thread join, native ACTIVE_SESSION_ID sync
     * (switchSession), render-thread restart and focus re-send.
     *
     * Shared by handleSessionExit, closeDeadSession and closeSession
     * (kotlin-architecture-round3 R1) so this lock-held sequence lives in
     * exactly one place instead of three drifting copies.
     *
     * Must be called with sessionLock held; the caller already removed the
     * closing entry and set activeSessionId = [newId].
     *
     * @param caller log prefix (e.g. "handleSessionExit")
     * @param markRunning set replacement.running = true first (closeSession
     *   needs it; the exit paths already run with the session running)
     * @param withRetry retry switchSession once on JNI failure (exit paths;
     *   a user close skips the retry to keep latency bounded)
     * @param syncGrid call syncGridDimensions on the replacement after the
     *   render start (closeSession only)
     */
    private fun activateReplacementSession(
        newId: Long,
        caller: String,
        markRunning: Boolean,
        withRetry: Boolean,
        syncGrid: Boolean,
    ) {
        val replacement =
            sessions[newId] ?: run {
                LogUtil.w("Runtime", "$caller: new active session $newId already removed")
                activeSessionId = 0L
                updateState()
                return
            }
        if (markRunning) {
            replacement.running = true
        }
        val bridge =
            replacement.bridge ?: run {
                LogUtil.w("Runtime", "$caller: new active session $newId has no bridge")
                activeSessionId = 0L
                updateState()
                return
            }
        // Final join of any hung thread: if it exited meanwhile, clear the
        // flag so a later close() destroys the native session (no leak).
        // Guard against joining ourselves: the exit paths run on a render
        // thread (poll.exit), and with background-session reaping (round-61)
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
        val newCellWidth = bridge.getCellWidth()
        val newCellHeight = bridge.getCellHeight()
        if (newCellWidth > 0f) cellWidth = newCellWidth
        if (newCellHeight > 0f) cellHeight = newCellHeight
        // CAS with the size check INSIDE the lambda (round-94): the old code
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
        val currentTitle = sessions[activeSessionId]?.bridge?.getActiveSessionTitle() ?: _state.value.title
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
                rows = previous.rows,
                cols = previous.cols,
                activeSessionId = activeSessionId,
                sessionIds = sessions.keys.sorted(),
            )
        }
    }

    fun onSurfaceDestroyed() {
        for (entry in sessions.values) {
            entry.bridge?.setRenderPaused(true)
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
     * Queue [action] on the same single-thread executor that stops render
     * threads during pauseRendering. Callers use this to release surfaces /
     * views only AFTER the render thread's join has confirmed it is no
     * longer inside native render code — releasing an ANativeWindow while
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
