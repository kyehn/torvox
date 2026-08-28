package terminal.emulator.bridge

import android.util.Log
import terminal.emulator.runtime.LogUtil

/**
 * Shell configuration for a terminal session.
 */
sealed interface Shell {
    /** Use the system default shell (/system/bin/sh). */
    data object SystemDefault : Shell

    /** Use a custom shell at the given path. */
    data class Custom(val path: String) : Shell
}

/** ARGB → linear RGB floats (0..1 per channel) for the JNI cursor-color
 *  channel. The alpha byte is intentionally dropped (the renderer treats
 *  the cursor as opaque). */
internal fun argbToRgbFloats(argb: Int): FloatArray = floatArrayOf(
    (argb shr 16 and 0xFF) / 255f,
    (argb shr 8 and 0xFF) / 255f,
    (argb and 0xFF) / 255f,
)

/**
 * Terminal theme expressed as ARGB ints for the native renderer.
 * Matches [terminal.emulator.ui.theme.TerminalTheme] conversion in makeBridgeTheme().
 */
data class BridgeTheme(
    val name: String,
    val bg: Int,
    val fg: Int,
    val cursor: Int,
    val selectionBg: Int,
    val ansi0: Int,
    val ansi1: Int,
    val ansi2: Int,
    val ansi3: Int,
    val ansi4: Int,
    val ansi5: Int,
    val ansi6: Int,
    val ansi7: Int,
    val ansi8: Int,
    val ansi9: Int,
    val ansi10: Int,
    val ansi11: Int,
    val ansi12: Int,
    val ansi13: Int,
    val ansi14: Int,
    val ansi15: Int,
)

/**
 * Configuration passed to [createBridge].
 */
data class TerminalConfig(
    val shell: Shell,
    val rows: Int,
    val cols: Int,
    val theme: BridgeTheme,
    val home: String,
    val user: String,
    val path: String,
    val workingDirectory: String,
    val prefix: String,
    val scrollbackLines: Int,
    val font_size_tenths: Int,
    /** User-defined environment overrides ("KEY=VALUE" semantics). */
    val env: Map<String, String> = emptyMap(),
)

/** Create a new Bridge instance wrapping [NativeBridge] JNI exports. */
fun createBridge(config: TerminalConfig): Bridge = Bridge(config)

/**
 * Instance bridge wrapping [NativeBridge] static JNI exports.
 *
 * Each [Bridge] holds a session ID and manages session lifecycle so callers
 * don't touch session IDs directly. Methods without a native counterpart
 * log a warning and return a safe default.
 *
 * Bridge is a gateway to the native side by design; the function count
 * is the JNI surface, not an interface smell.
 *
 * # ADR-0007 surface integration (implemented)
 *
 * The rendering path is live: `render`/`attachSurface`/`releaseGpuSurface`
 * map to the wgpu renderer via JNI, and the background-image, cursor-blink
 * and render-pause settings are wired end-to-end. The
 * remaining log-only helpers (`recomputeGrid`, `getCellWidth/Height`,
 * `loadFontFile`, `setSystemLocale`,...) are either superseded by other
 * channels (attachSurface carries the size; events carry grid dims) or are
 * query-path stubs backed by [NativeQueryPort].
 */
// when-dispatch over the PollEvent sealed class — one branch per variant.
@Suppress("TooManyFunctions", "LongMethod") // parseEvent is a straight
class Bridge(private val config: TerminalConfig) : TerminalQueryPort {
    /**
     * ADR-0007: native query path wired — all queries delegate to
     * [NativeQueryPort], which maps 1:1 to the JNI query exports
     * native/src/android/ffi.rs, "TerminalQueryPort" section). The stub
     * only backs the no-session window (sessionId == 0, before spawn).
     */
    private val queryPort: TerminalQueryPort = NativeQueryPort { sessionId }

    @Volatile private var sessionId: Long = 0L

    @Volatile private var lastSurfaceWidth: Int = 0

    @Volatile private var lastSurfaceHeight: Int = 0

    /**
     * Input→echo latency hook (emulator-performance-verification): invoked
     * with `SystemClock.elapsedRealtimeNanos()` on EVERY PTY write path
     * (Bridge.writeToPty, processKeyEvent, encodeMouseEvent) so hardware
     * keys — which bypass TerminalRuntime.writeToPty — are stamped too.
     * Wired by SessionEntry to its [LatencyProbe].
     */
    @Volatile var onPtyWrite: ((Long) -> Unit)? = null

    fun ping(): String {
        if (!NativeBridge.isNativeLoaded()) throw RuntimeException("native library not loaded")
        return "native library OK, sessions=${NativeBridge.getSessionCount()}"
    }

    // ── Session lifecycle ─────────────────────────────────────────────

    /** Resolve the configured [Shell] to an absolute executable path. */
    fun shellPath(): String = when (val shell = config.shell) {
        is Shell.SystemDefault -> "/system/bin/sh"
        is Shell.Custom -> shell.path
    }

    fun spawnTerminal(rows: Int, cols: Int, shell: String): Long {
        // Reference (zed-android-port util/env.rs EnvOp; Termux
        // ExtraKeys/env passthrough): serialize user environment overrides
        // as a "KEY=VALUE" array. null (empty) is fine — the native side
        // skips it entirely.
        val envArray: Array<String>? =
            if (config.env.isEmpty()) {
                null
            } else {
                config.env.entries.sortedBy { it.key }.map { "${it.key}=${it.value}" }.toTypedArray()
            }
        sessionId =
            NativeBridge.initSession(
                rows,
                cols,
                shell,
                config.home,
                config.user,
                config.path,
                config.workingDirectory,
                config.prefix,
                config.scrollbackLines,
                envArray,
            )
        return sessionId
    }

    fun close() {
        if (sessionId != 0L) {
            try {
                NativeBridge.destroySession(sessionId)
            } catch (exception: Throwable) {
                // Cleanup path: a failure here (RuntimeException from the
                // native side for an unknown session, UnsatisfiedLinkError for
                // a partially loaded library) must never escape — callers
                // catch(Exception) only, and an Error would reach the global
                // handler and kill the process. The registry entry is removed
                // regardless; native tolerates unknown IDs.
                Log.e(TAG, "close: destroySession failed", exception)
            }
            sessionId = 0L
        }
    }

    fun resize(rows: Int, cols: Int) {
        if (sessionId == 0L) return
        try {
            NativeBridge.resize(sessionId, rows, cols)
        } catch (exception: RuntimeException) {
            // Race: the session was destroyed on the IO thread between the
            // sessionId check and this call (closeSession/stop/exit). The
            // native side throws RuntimeException for unknown sessions;
            // dropping the resize is correct — the session is gone.
            Log.d(TAG, "resize: session $sessionId already destroyed, dropping")
        }
    }

    /**
     * Update the PTY winsize pixel fields (ws_xpixel/ws_ypixel) for this
     * session, preserving rows/cols. The Kotlin host calls this alongside
     * each grid resize with the surface's pixel dimensions so pixel-aware
     * programs (`icat`, fullscreen TUIs) read real pixels from TIOCGWINSZ
     * ghostty-android pty_jni.c:84-87).
     */
    fun setPixelSize(widthPx: Int, heightPx: Int) {
        if (sessionId == 0L) return
        try {
            NativeBridge.setPixelSize(sessionId, widthPx, heightPx)
        } catch (exception: RuntimeException) {
            // Same destruction race as resize: dropping is correct.
            Log.d(TAG, "setPixelSize: session $sessionId already destroyed, dropping")
        }
    }

    /**
     * Recompute the grid from pixel dimensions. The cell-size calculation
     * lives in Rust: the renderer derives cell metrics from the font
     * pipeline, and [TerminalRuntime.syncGridDimensions] pulls the real
     * grid via [getGridRowsColsPacked] after a resize. This method only
     * logs: the native side resolves rows/cols from events).
     */
    fun recomputeGrid(width: Int, height: Int) {
        Log.d(TAG, "recomputeGrid($width,$height) — native resolves rows/cols from events")
    }

    fun getGridRowsColsPacked(): Long {
        if (sessionId == 0L) return 0L
        return try {
            NativeBridge.getGridRowsColsPacked(sessionId)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "getGridRowsColsPacked failed: ${exception.javaClass.simpleName}")
            0L
        }
    }

    fun getCellWidth(): Float {
        if (sessionId == 0L) return 0f
        return try {
            NativeBridge.getCellWidth(sessionId)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "getCellWidth failed: ${exception.javaClass.simpleName}")
            0f
        }
    }

    fun getCellHeight(): Float {
        if (sessionId == 0L) return 0f
        return try {
            NativeBridge.getCellHeight(sessionId)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "getCellHeight failed: ${exception.javaClass.simpleName}")
            0f
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────
    // ADR-0007 surface integration is implemented:
    // render/attachSurface/releaseGpuSurface/setRenderPaused map to the
    // wgpu renderer via JNI.

    /** Render a frame. Returns >0 if output was available, 0 if idle, -1 on error. */
    fun render(): Int {
        if (sessionId == 0L) return 0
        return try {
            NativeBridge.render(sessionId, lastSurfaceWidth, lastSurfaceHeight)
        } catch (exception: RuntimeException) {
            // Class only: exception messages can embed session data.
            LogUtil.e("Bridge", "render failed: ${exception.javaClass.simpleName}")
            -1
        }
    }

    /**
     * Combined render + consumeNewOutput in a single JNI crossing (saves ~0.1-0.3ms
     * per frame vs two separate calls). Returns [RenderResult] with the render
     * count and the new-output flag.
     */
    fun renderWithNewOutput(): RenderResult {
        if (sessionId == 0L) return RenderResult(0, false)
        return try {
            val packed = NativeBridge.renderWithNewOutput(sessionId, lastSurfaceWidth, lastSurfaceHeight)
            val count = packed.toInt()
            val newOutput = (packed shr 32) != 0L
            RenderResult(count, newOutput)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "renderWithNewOutput failed: ${exception.javaClass.simpleName}")
            RenderResult(-1, false)
        }
    }

    data class RenderResult(val count: Int, val newOutput: Boolean)

    /**
     * Take and clear the native `new_output` flag for this session (P1-1
     * scroll-reset signal, dual-flag protocol — see
     * docs/reference/dual-flag-protocol.md). Called once per frame from the
     * render thread; returns true when PTY output was ingested since the
     * last call. Unknown/destroyed sessions report false.
     */
    fun consumeNewOutput(): Boolean {
        if (sessionId == 0L) return false
        return try {
            NativeBridge.consumeNewOutput(sessionId)
        } catch (exception: RuntimeException) {
            // Class only: exception messages can embed session data.
            LogUtil.e("Bridge", "consumeNewOutput failed: ${exception.javaClass.simpleName}")
            false
        }
    }

    /** Attach the Android Surface for GPU rendering (ADR-0007). */
    fun attachSurface(surface: Any, width: Int, height: Int) {
        if (sessionId == 0L) return
        lastSurfaceWidth = width
        lastSurfaceHeight = height
        try {
            NativeBridge.attachWindow(sessionId, surface, width, height)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "attachWindow failed: ${exception.javaClass.simpleName}")
        }
    }

    /**
     * Parks the calling thread for [timeoutMs] (or until
     * [TerminalRuntime.notifyRender] unparks it, whichever comes first).
     *
     * There is no native render JNI export yet (ADR-0007: surface
     * integration pending), so this park-based sleep both bounds the
     * render-loop poll cadence (no 100% CPU busy-spin) and pairs with
     * [TerminalRuntime.SessionEntry.notifyRender] which calls
     * LockSupport.unpark on the render thread.
     *
     * The return value is advisory only — callers re-check the interrupt
     * flag themselves after this returns; `parkNanos` returns on interrupt
     * without clearing the flag, so `Thread.interrupted()` still sees it.
     * Returns true if the wait was not interrupted.
     */
    fun waitOutput(timeoutMs: Long): Boolean {
        if (timeoutMs <= 0L) return true
        java.util.concurrent.locks.LockSupport.parkNanos(timeoutMs * 1_000_000L)
        return !Thread.currentThread().isInterrupted
    }

    fun releaseGpuSurface() {
        Log.d(TAG, "releaseGpuSurface()")
        if (sessionId != 0L) NativeBridge.detachWindow(sessionId)
    }

    fun setRenderPaused(paused: Boolean) {
        Log.d(TAG, "setRenderPaused($paused)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setRenderPaused(sessionId, paused)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setRenderPaused failed: ${exception.javaClass.simpleName}")
        }
    }

    fun setBackgroundParams(radius: Int, alpha: Int) {
        Log.d(TAG, "setBackgroundParams(blur=$radius, alpha=$alpha)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setBackgroundParams(sessionId, radius, alpha)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setBackgroundParams failed: ${exception.javaClass.simpleName}")
        }
    }

    /**
     * Queue the bell-flash overlay phase (0..1) for the next rendered frame.
     * The native renderer composites a full-screen quad whose alpha scales
     * with the phase; Kotlin animates it down after a bell (BellMode.SCREEN_FLASH).
     */
    fun setFlashState(phase: Float) {
        if (sessionId == 0L) return
        try {
            NativeBridge.setFlashState(sessionId, phase.coerceIn(0f, 1f))
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setFlashState failed: ${exception.javaClass.simpleName}")
        }
    }

    @Volatile private var cursorBlinkEnabled = true

    @Volatile private var cursorBlinkSpeedMs = 600

    fun setCursorBlinkEnabled(enabled: Boolean) {
        cursorBlinkEnabled = enabled
        Log.d(TAG, "setCursorBlink($enabled, speed=$cursorBlinkSpeedMs)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setCursorBlink(sessionId, enabled, cursorBlinkSpeedMs)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setCursorBlinkEnabled failed: ${exception.javaClass.simpleName}")
        }
    }

    fun setCursorBlinkSpeedMs(ms: Int) {
        cursorBlinkSpeedMs = ms.coerceIn(100, 1000)
        Log.d(TAG, "setCursorBlinkSpeedMs(${cursorBlinkSpeedMs}ms)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setCursorBlink(sessionId, cursorBlinkEnabled, cursorBlinkSpeedMs)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setCursorBlinkSpeedMs failed: ${exception.javaClass.simpleName}")
        }
    }

    fun resetCursorBlink() {
        Log.d(TAG, "resetCursorBlink()")
        if (sessionId == 0L) return
        try {
            NativeBridge.resetCursorBlink(sessionId)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "resetCursorBlink failed: ${exception.javaClass.simpleName}")
        }
    }

    fun setBackgroundImage(rgbaData: ByteArray, width: Int, height: Int) {
        if (sessionId == 0L) return
        try {
            NativeBridge.setBackgroundImage(sessionId, rgbaData, width, height)
        } catch (exception: RuntimeException) {
            // Class only: exception messages can embed session data.
            LogUtil.e("Bridge", "setBackgroundImage failed: ${exception.javaClass.simpleName}")
        }
    }

    fun clearBackgroundImage() {
        if (sessionId == 0L) return
        try {
            NativeBridge.clearBackgroundImage(sessionId)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "clearBackgroundImage failed: ${exception.javaClass.simpleName}")
        }
    }

    // ── Events ────────────────────────────────────────────────────────
    data class PollResult(
        val bel: Boolean = false,
        val notification: Pair<String, String>? = null,
        val clipboard: String? = null,
        val exit: Boolean = false,
        val exitCode: Int = 0,
        // native-measured child lifetime for the first exit.
        val exitAliveMs: Long = 0,
        val sessionId: Long = 0L,
        val dialogs: List<DialogRequest> = emptyList(),
        val pickFiles: List<PickFileRequest> = emptyList(),
        val dialogCancels: List<Pair<Long, Long>> = emptyList(),
        val toastText: String? = null,
        val openUrl: String? = null,
        val clipboardGets: List<ClipboardRequest> = emptyList(),
        val clipboardReads: List<ClipboardRequest> = emptyList(),
        val runCommands: List<RunCommandRequest> = emptyList(),
        val screenshots: List<ScreenshotRequest> = emptyList(),
        val progress: Pair<Int, Int>? = null,
        // Every exit event seen this frame, in order. The single-slot
        // exit/sessionId/exitCode fields above describe only the FIRST one;
        // extra exits in the same frame must be reaped from this list or
        // they would leak (native exit_reported is set at push and never
        // re-sent).
        val exits: List<ExitInfo> = emptyList(),
    ) {
        /** Merge a later polled event into this result; later wins for scalar fields. */
        fun merge(later: PollResult): PollResult = PollResult(
            bel = bel || later.bel,
            notification = later.notification ?: notification,
            clipboard = later.clipboard ?: clipboard,
            exit = exit || later.exit,
            // exitCode belongs to the same (first) exit as sessionId.
            exitCode = if (later.exit && !exit) later.exitCode else exitCode,
            // alive_ms travels with its exit event.
            exitAliveMs = if (later.exit && !exit) later.exitAliveMs else exitAliveMs,
            // sessionId only serves exit attribution. The FIRST exit
            // seen in a frame wins: a later non-exit event (e.g. a
            // dialog for another session) must not overwrite the
            // exiting session's id (which would reap a live session).
            sessionId = if (later.exit && !exit) later.sessionId else sessionId,
            // Request events accumulate: each one carries a distinct
            // request_id and must be dispatched exactly once (a single
            // slot would silently drop concurrent MCP requests).
            dialogs = dialogs + later.dialogs,
            pickFiles = pickFiles + later.pickFiles,
            dialogCancels = dialogCancels + later.dialogCancels,
            toastText = later.toastText ?: toastText,
            openUrl = later.openUrl ?: openUrl,
            clipboardGets = clipboardGets + later.clipboardGets,
            clipboardReads = clipboardReads + later.clipboardReads,
            runCommands = runCommands + later.runCommands,
            screenshots = screenshots + later.screenshots,
            progress = later.progress ?: progress,
            exits = exits + later.exits,
        )
    }

    data class ExitInfo(
        val sessionId: Long,
        val exitCode: Int,
        // child lifetime (ms) measured natively — the
        // fast-death decision uses this, not Kotlin event latency.
        val exitAliveMs: Long = 0,
    )

    data class ClipboardRequest(
        val sessionId: Long,
        val requestId: Long,
        val selection: String = "",
    )

    /** MCP `run_command` request dispatched from a poll event. */
    data class RunCommandRequest(
        val sessionId: Long,
        val requestId: Long,
        val command: String = "",
    )

    /** MCP `screenshot` request dispatched from a poll event. */
    data class ScreenshotRequest(
        val sessionId: Long,
        val requestId: Long,
    )

    data class DialogRequest(
        val sessionId: Long,
        val requestId: Long,
        val dialogType: String,
        val title: String,
        val message: String,
        val options: List<String>,
    )

    data class PickFileRequest(
        val sessionId: Long,
        val requestId: Long,
        val startingPath: String,
        val filter: String,
    )

    fun pollAll(): PollResult {
        // Drain up to MAX_EVENTS_PER_POLL queued events per frame so a
        // backlog (e.g. an agent firing many OSC 9 / dialog requests) is
        // consumed in a few frames instead of one event per 16ms frame.
        // Results merge: a later event of the same kind wins (exit is
        // sticky — later events for a dead session are stale).
        var result = PollResult()
        // Plain for loop: `break` is required when the queue drains.
        for (unused in 0 until MAX_EVENTS_PER_POLL) {
            val json = NativeBridge.pollEvent() ?: break
            val parsed =
                try {
                    parseEvent(json)
                } catch (e: Exception) {
                    Log.w(TAG, "pollAll: bad JSON: ${e.message}")
                    continue
                }
            result = result.merge(parsed)
            // Do NOT break on exit: events queued after the Exit (e.g. MCP
            // dialog/pick_file requests dispatched before the exit was
            // detected) would otherwise be stranded in the native queue —
            // no other session drains them and the MCP call would hang.
            // Exit is sticky in merge, so draining on is harmless.
        }
        return result
    }

    private fun parseEvent(json: String): PollResult = when (val event = pollEventJson.decodeFromString<PollEvent>(json)) {
        is PollEvent.Bell ->
            PollResult(bel = true, sessionId = event.sessionId)

        is PollEvent.Notification ->
            PollResult(
                notification = event.title to event.body,
                sessionId = event.sessionId,
            )

        is PollEvent.Progress ->
            PollResult(
                progress = event.state to event.value,
                sessionId = event.sessionId,
            )

        is PollEvent.Clipboard ->
            PollResult(clipboard = event.text.ifEmpty { null }, sessionId = event.sessionId)

        is PollEvent.Exit ->
            PollResult(
                exit = true,
                exitCode = event.code,
                exitAliveMs = event.aliveMs,
                sessionId = event.sessionId,
                exits =
                listOf(
                    ExitInfo(
                        sessionId = event.sessionId,
                        exitCode = event.code,
                        exitAliveMs = event.aliveMs,
                    ),
                ),
            )

        is PollEvent.ShowDialog ->
            PollResult(
                dialogs =
                listOf(
                    DialogRequest(
                        sessionId = event.sessionId,
                        requestId = event.requestId,
                        dialogType = event.dialogType,
                        title = event.title,
                        message = event.message,
                        options = event.options,
                    ),
                ),
            )

        is PollEvent.PickFile ->
            PollResult(
                pickFiles =
                listOf(
                    PickFileRequest(
                        sessionId = event.sessionId,
                        requestId = event.requestId,
                        startingPath = event.startingPath,
                        filter = event.filter,
                    ),
                ),
            )

        is PollEvent.DialogCancel ->
            PollResult(
                dialogCancels =
                listOf(
                    event.sessionId to event.requestId,
                ),
            )

        is PollEvent.GetClipboard ->
            PollResult(
                clipboardGets =
                listOf(
                    ClipboardRequest(
                        sessionId = event.sessionId,
                        requestId = event.requestId,
                    ),
                ),
            )

        is PollEvent.ClipboardRead ->
            PollResult(
                clipboardReads =
                listOf(
                    ClipboardRequest(
                        sessionId = event.sessionId,
                        requestId = event.requestId,
                        selection = event.selection,
                    ),
                ),
            )

        is PollEvent.Toast -> PollResult(toastText = event.text)

        is PollEvent.OpenUrl -> PollResult(openUrl = event.url)

        is PollEvent.RunCommand ->
            PollResult(
                runCommands =
                listOf(
                    RunCommandRequest(
                        sessionId = event.sessionId,
                        requestId = event.requestId,
                        command = event.command,
                    ),
                ),
            )

        is PollEvent.Screenshot ->
            PollResult(
                screenshots =
                listOf(
                    ScreenshotRequest(
                        sessionId = event.sessionId,
                        requestId = event.requestId,
                    ),
                ),
            )
    }

    // ── Theme / appearance ────────────────────────────────────────────
    // Wired end-to-end: setTheme packs 54 bytes
    // (bg3 fg3 ansi48) for the native palette; OSC 10/11/4 color handling
    // lives in the terminal engine and is applied via the palette API.
    // The cursor color rides a separate [setCursorColor] channel so the
    // 54-byte layout stays stable (ffi.rs validates the exact length).
    fun setTheme(theme: BridgeTheme) {
        Log.d(TAG, "setTheme: ${theme.name}")
        if (sessionId == 0L) return
        val data = ByteArray(54)
        fun packColor(dst: Int, argb: Int) {
            data[dst] = (argb shr 16 and 0xFF).toByte()
            data[dst + 1] = (argb shr 8 and 0xFF).toByte()
            data[dst + 2] = (argb and 0xFF).toByte()
        }
        packColor(0, theme.bg)
        packColor(3, theme.fg)
        val ansi = listOf(
            theme.ansi0, theme.ansi1, theme.ansi2, theme.ansi3,
            theme.ansi4, theme.ansi5, theme.ansi6, theme.ansi7,
            theme.ansi8, theme.ansi9, theme.ansi10, theme.ansi11,
            theme.ansi12, theme.ansi13, theme.ansi14, theme.ansi15,
        )
        ansi.forEachIndexed { i, c -> packColor(6 + i * 3, c) }
        try {
            NativeBridge.setTheme(sessionId, data)
            setCursorColor(theme.cursor)
        } catch (exception: RuntimeException) {
            // Race: session destroyed between the check and the call.
            Log.d(TAG, "setTheme: session $sessionId already destroyed, dropping")
        }
    }

    /** App-level cursor color override, applied on top of the theme's own
     *  cursor color. */
    fun setCursorColor(argb: Int) {
        if (sessionId == 0L) return
        try {
            val rgb = argbToRgbFloats(argb)
            NativeBridge.setCursorColor(sessionId, rgb[0], rgb[1], rgb[2])
        } catch (exception: RuntimeException) {
            Log.d(TAG, "setCursorColor: session $sessionId already destroyed, dropping")
        }
    }

    fun setSystemLocale(locale: String) {
        Log.d(TAG, "setSystemLocale($locale)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setSystemLocale(sessionId, locale)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setSystemLocale failed: ${exception.javaClass.simpleName}")
        }
    }

    fun setExtraFontPaths(paths: List<String>) {
        Log.d(TAG, "setExtraFontPaths($paths)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setExtraFontPaths(sessionId, paths.toTypedArray())
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setExtraFontPaths failed: ${exception.javaClass.simpleName}")
        }
    }
    fun setFontFamily(family: String) {
        Log.d(TAG, "setFontFamily($family)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setFontFamily(sessionId, family)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setFontFamily failed: ${exception.javaClass.simpleName}")
        }
    }

    /**
     * Set the independent family for a style slot — 0=bold, 1=italic,
     * 2=bold-italic (ghostty-android TerminalFontStore 4-slot design).
     * Empty family clears the slot.
     */
    fun setFontFamilyForStyle(family: String, slot: Int) {
        Log.d(TAG, "setFontFamilyForStyle(slot=$slot, $family)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setFontFamilyForStyle(sessionId, family, slot)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setFontFamilyForStyle failed: ${exception.javaClass.simpleName}")
        }
    }

    fun setFontSize(sizeTenths: Int) {
        Log.d(TAG, "setFontSize($sizeTenths)")
        setFontSizeInPlace(sizeTenths)
    }

    fun setFontSizeInPlace(sizeTenths: Int) {
        Log.d(TAG, "setFontSizeInPlace($sizeTenths)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setFontSizeInPlace(sessionId, sizeTenths)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setFontSizeInPlace failed: ${exception.javaClass.simpleName}")
        }
    }

    fun setRasterScale(scale: Float) {
        if (sessionId == 0L) return
        try {
            NativeBridge.setRasterScale(sessionId, scale)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setRasterScale failed: ${exception.javaClass.simpleName}")
        }
    }

    fun setCursorStyle(style: String) {
        Log.d(TAG, "setCursorStyle($style)")
        if (sessionId == 0L) return
        try {
            NativeBridge.setCursorStyle(sessionId, style)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "setCursorStyle failed: ${exception.javaClass.simpleName}")
        }
    }

    // Custom font loading probes the file in native code (fontdb), registers
    // it with the renderer and returns the family name; null on failure.
    fun loadFontFile(path: String): String? {
        Log.d(TAG, "loadFontFile($path)")
        if (sessionId == 0L) return null
        return try {
            NativeBridge.loadFontFile(sessionId, path)
        } catch (exception: RuntimeException) {
            LogUtil.e("Bridge", "loadFontFile failed: ${exception.javaClass.simpleName}")
            null
        }
    }

    // ── Input ─────────────────────────────────────────────────────────
    // Command-safety reference (sushi-ssh CommandSafety.kt:1-220,
    // https://github.com/hlan-net/sushi-ssh-client): three-level shell
    // command classifier — SAFE (read-only, auto-exec) / CONFIRM (write,
    // ask user) / BLOCKED (never: shutdown, rm -rf /, fork-bomb, `curl |
    // bash` via shell-interpreter pipe/chain detection). torvox has no
    // MCP run_command tool today; if one is added (e.g. an agent reading
    // the terminal writes commands), classify first with a port of this
    // classifier plus a visible CONFIRM surface, never execute a
    // BLOCKED pattern. Same idea also protects any future "tap to run a
    // suggested command" affordance.
    fun feedTerminal(data: ByteArray): Boolean {
        if (sessionId == 0L) return false
        try {
            NativeBridge.feedTerminal(sessionId, data)
        } catch (exception: RuntimeException) {
            Log.d(TAG, "feedTerminal: session $sessionId destroyed, dropping")
            return false
        }
        return true
    }

    fun writeToPty(data: ByteArray): Boolean {
        if (sessionId == 0L) return false
        try {
            // Raw bytes end-to-end: decoding to a Java String here would
            // replace non-UTF-8 sequences (pasted GBK/ISO-8859-1, binary
            // protocols) with U+FFFD and corrupt what the child receives.
            NativeBridge.feedPty(sessionId, data)
            onPtyWrite?.invoke(android.os.SystemClock.elapsedRealtimeNanos())
        } catch (exception: RuntimeException) {
            // Race: session destroyed between the sessionId check and this
            // call (closeSession/stop on the IO thread). Input for a closed
            // session is dropped by design.
            Log.d(TAG, "writeToPty: session $sessionId already destroyed, dropping")
            return false
        }
        return true
    }

    /**
     * Encode a mouse event via the Ghostty mouse encoder and write the
     * resulting escape sequence to the PTY. Returns true when a sequence
     * was produced and written; false when mouse reporting is disabled,
     * encoding failed, or the session is gone (event dropped).
     */
    fun encodeMouseEvent(xPx: Float, yPx: Float, action: Int, button: Int, cellW: Float, cellH: Float): Boolean {
        if (sessionId == 0L) return false
        val bytes =
            try {
                NativeBridge.encodeMouseEvent(sessionId, xPx, yPx, action, button, cellW, cellH)
            } catch (exception: RuntimeException) {
                Log.d(TAG, "encodeMouseEvent: session $sessionId destroyed, dropping")
                return false
            }
        if (bytes.isEmpty()) return false
        return writeToPty(bytes)
    }

    /**
     * Whether the remote is on the alternate screen buffer (vim/less/htop).
     * Lock-free; safe to call on every touch-scroll event. When true, touch
     * scroll gestures must be forwarded to the remote as mouse-wheel escapes
     * see [TerminalSurface] onScroll) rather than scrolling local scrollback.
     */
    fun isAltScreenActive(): Boolean {
        if (sessionId == 0L) return false
        return try {
            NativeBridge.getAltScreenState(sessionId)
        } catch (exception: RuntimeException) {
            Log.d(TAG, "isAltScreenActive: session $sessionId destroyed, returning false")
            false
        }
    }

    /**
     * Whether the terminal is in application cursor mode (DECCKM, DEC
     * private mode 1). Arrow keys must then be encoded SS3 (`ESC OA`)
     * instead of CSI (`ESC [ A`) — research-haven.md:141,
     * research-zed-port.md:252. Queried only for arrow-key key events.
     */
    fun isAppCursorMode(): Boolean {
        if (sessionId == 0L) return false
        return try {
            NativeBridge.getMode(sessionId, DEC_PRIVATE_MODE_APP_CURSOR, 0)
        } catch (exception: RuntimeException) {
            Log.d(TAG, "isAppCursorMode: session $sessionId destroyed, returning false")
            false
        }
    }

    fun processKeyEvent(keyCode: Int, modifiers: Byte, action: Int, unicodeChar: Int, unshiftedChar: Int): Boolean {
        Log.d(TAG, "processKeyEvent($keyCode, $modifiers, $action)")
        if (sessionId == 0L) return false
        // Only ACTION_DOWN produces output: both onKeyDown and onKeyUp route
        // here, and writing on UP would double every keystroke ("llss",
        // double Enter, double Ctrl+C). ACTION_UP returns false so the
        // platform default (no-op) handles it.
        if (action != android.view.KeyEvent.ACTION_DOWN) return false
        try {
            val modifierBits = modifiers.toInt()
            val ctrlActive = modifierBits and 4 != 0
            val altActive = modifierBits and 2 != 0
            // DECCKM: when the terminal is in application cursor mode
            // (DEC private mode 1) arrow keys must be encoded SS3 (`ESC OA`)
            // instead of CSI (`ESC [ A`) — research-haven.md:141,
            // research-zed-port.md:252. Only queried for arrow keys to avoid
            // a mode_get round-trip on every keystroke.
            val appCursorMode =
                keyCode in APP_CURSOR_KEY_CODES && isAppCursorMode()
            // Route ALL hardware keys through the same encoder the IME path
            // uses. Sending the key NAME (keyCodeToName: "Up", "Home",...)
            // as literal bytes would write the text "Up" into the PTY — the
            // native writeKey does not parse key names, so vim/less arrows,
            // Home/End, PageUp/Down and Delete were all broken.
            val encoded =
                terminal.emulator.ui.TerminalInputEncoder
                    .encodeKeyEvent(keyCode, unicodeChar, ctrlActive, altActive, appCursorMode)
            if (encoded != null) {
                NativeBridge.feedPty(sessionId, encoded)
                onPtyWrite?.invoke(android.os.SystemClock.elapsedRealtimeNanos())
                return true
            }
            // Fallback for keys the encoder does not handle: raw printable
            // unicode (supplementary-plane safe). Never while Ctrl is held:
            // the native writeKey folds single-byte ASCII with c & 0x1F,
            // which would turn Ctrl+9/Ctrl+0 into Ctrl+Y/Ctrl+P. Ctrl+printable
            // keys are either encoded above or intentionally dropped
            // (Ctrl+9/0 have no traditional mapping).
            if (!ctrlActive && unicodeChar > 0 && unicodeChar != 0x7F) {
                val ch =
                    if (Character.isValidCodePoint(unicodeChar)) {
                        String(Character.toChars(unicodeChar))
                    } else {
                        return false
                    }
                NativeBridge.writeKey(sessionId, ch, modifierBits, null)
                return true
            }
            // some IMEs (Gboard under InputType.TYPE_NULL) emit
            // key events with unicodeChar == 0 even though the key is a
            // printable letter. Derive the character from the virtual
            // keyboard's key character map as a fallback so those key
            // presses still reach the PTY.
            if (!ctrlActive && keyCode in android.view.KeyEvent.KEYCODE_A..android.view.KeyEvent.KEYCODE_Z) {
                val derived =
                    android.view.KeyCharacterMap
                        .load(android.view.KeyCharacterMap.VIRTUAL_KEYBOARD)
                        .get(keyCode, 0)
                if (derived > 0) {
                    NativeBridge.writeKey(sessionId, derived.toChar().toString(), modifierBits, null)
                    return true
                }
            }
            return false
        } catch (exception: RuntimeException) {
            // Either the session was destroyed between the check and the
            // call, or the native write failed. Log the actual message so
            // the two are distinguishable (payload itself is never logged).
            Log.d(TAG, "processKeyEvent: session $sessionId write failed: ${exception.message}")
            return false
        }
    }

    fun focusEvent(focused: Boolean) {
        if (sessionId == 0L) return
        try {
            NativeBridge.focusEvent(sessionId, focused)
        } catch (exception: RuntimeException) {
            // Session closed between the id check and the native call.
            Log.d(TAG, "focusEvent: session gone: ${exception.message}")
        }
    }

    // ── Terminal queries (delegated to TerminalQueryPort seam) ────────
    override fun getTitle(): String? = queryPort.getTitle()
    override fun getActiveSessionTitle(): String = queryPort.getActiveSessionTitle()

    // ── Selection ─────────────────────────────────────────────────────
    override fun setSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean?,
        mode: Byte,
        selectionBgArgb: Int,
    ) {
        queryPort.setSelection(startRow, startCol, endRow, endCol, hasSelection, mode, selectionBgArgb)
    }
    override fun expandAndSetSelection(row: Int, col: Int, mode: Byte): Pair<Pair<Int, Int>, Pair<Int, Int>>? = queryPort.expandAndSetSelection(row, col, mode)

    // ── Search / scrollback ────────────────────────────────────────────
    // Query methods delegate to the real native JNI path via
    // NativeQueryPort. Native exports throw IllegalArgumentException for
    // unknown sessions (e.g. the window between bridge.close() and
    // session-map removal); catch it here so UI/触摸 paths never crash.
    override fun clearSearchHighlights() = queryPort.clearSearchHighlights()
    override fun setSearchHighlights(data: ByteArray) = queryPort.setSearchHighlights(data)
    override fun scrollbackLine(row: Int): String? = runCatching { queryPort.scrollbackLine(row) }.getOrNull()
    override fun scrollbackLength(): Int = runCatching { queryPort.scrollbackLength() }.getOrDefault(0)
    override fun cursorViewportPacked(): Long = runCatching { queryPort.cursorViewportPacked() }.getOrDefault(-1L)
    override fun isCellEmpty(row: Int, col: Int): Boolean = runCatching { queryPort.isCellEmpty(row, col) }.getOrDefault(true)
    override fun searchAllInScrollback(query: String, caseSensitive: Boolean, fuzzyMatch: Boolean): List<Triple<Int, Int, Int>>? = runCatching { queryPort.searchAllInScrollback(query, caseSensitive, fuzzyMatch) }.getOrNull()
    override fun setScrollOffset(offset: Int) = queryPort.setScrollOffset(offset)

    override fun getTerminalText(): String? = runCatching { queryPort.getTerminalText() }.getOrNull()
    override fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int, rectangle: Boolean): String? = runCatching { queryPort.selectionText(startRow, startCol, endRow, endCol, rectangle) }.getOrNull()
    override fun hyperlinkAt(row: Int, col: Int): String? = runCatching { queryPort.hyperlinkAt(row, col) }.getOrNull()
    override fun listFontFamilies(): List<String>? = runCatching { queryPort.listFontFamilies() }.getOrNull()
    override fun getDefaultFontName(): String = runCatching { queryPort.getDefaultFontName() }.getOrDefault("monospace")
    override fun getFontInfo(): String? = runCatching { queryPort.getFontInfo() }.getOrNull()

    companion object {
        private const val TAG = "Bridge"

        /** Max events drained per pollAll() frame — bounds render-thread cost. */
        private const val MAX_EVENTS_PER_POLL = 32

        /** DEC private mode 1 = application cursor keys (DECCKM). */
        private const val DEC_PRIVATE_MODE_APP_CURSOR = 1

        /** Key codes whose encoding depends on DECCKM. */
        private val APP_CURSOR_KEY_CODES =
            setOf(
                android.view.KeyEvent.KEYCODE_DPAD_UP,
                android.view.KeyEvent.KEYCODE_DPAD_DOWN,
                android.view.KeyEvent.KEYCODE_DPAD_RIGHT,
                android.view.KeyEvent.KEYCODE_DPAD_LEFT,
            )
    }
}
