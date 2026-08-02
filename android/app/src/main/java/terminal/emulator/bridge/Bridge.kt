package terminal.emulator.bridge

import android.util.Log

/**
 * Shell configuration for a terminal session.
 */
sealed interface Shell {
    /** Use the system default shell (/system/bin/sh). */
    data object SystemDefault : Shell

    /** Use a custom shell at the given path. */
    data class Custom(val path: String) : Shell
}

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
 * # ADR-0007 placeholder methods
 *
 * Surface integration is deferred (ADR-0007): rendering-related methods
 * (render/setNativeWindow/setSurfaceSize/setBackgroundParams/
 * setCursorBlink/setCursorBlinkSpeedMs/setBackgroundImage/setTheme/
 * setSystemLocale/setFontFamily/setFontSize/loadFontFile/
 * getCellWidth/getCellHeight/recomputeGrid/setSelection) are
 * no-op stubs that only log. They are deliberately grouped under the
 * "Rendering" and "Theme / appearance" sections so that implementing
 * ADR-0007 means filling in exactly those two blocks — every other method
 * in this class talks to the real native session.
 */
@Suppress("TooManyFunctions")
class Bridge(private val config: TerminalConfig) : TerminalQueryPort {
    /** ADR-0007: query path not wired; all queries delegate to the stub. */
    private val queryPort: TerminalQueryPort = StubQueryPort()

    @Volatile private var sessionId: Long = 0L

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

    /** Recompute grid from pixel dimensions (rows/cols derived by native side). */
    fun recomputeGrid(width: Int, height: Int) {
        // Currently native resize expects rows/cols, not pixels.
        // This is a placeholder — the actual cell-size calculation lives in Rust.
        Log.d(TAG, "recomputeGrid($width,$height) — native resolves rows/cols from events")
    }

    fun getGridRowsColsPacked(): Long = // Placeholder — real values arrive via pollEvent(). 0 means
        // "unknown": callers must not overwrite real dimensions with it
        // (a fixed 24x80 here would later shrink a running PTY through a
        // resize() triggered by the settings path).
        0L

    fun getCellWidth(): Float = 0f
    fun getCellHeight(): Float = 0f

    // ── Rendering ─────────────────────────────────────────────────────
    // ADR-0007: all methods in this section are placeholder stubs (log-only
    // or constant returns). Implementing surface integration replaces this
    // whole section; callers are already wired and unchanged.

    /** Render a frame. Returns >0 if output was available, 0 if idle, -1 on error. */
    fun render(shouldSkipOutput: Boolean = false): Int {
        // No per-frame log: this runs at 60fps on the render thread and
        // would spam logcat. Debug via logcat tag filtering if needed.
        // WARNING: no native JNI export for render() — the render loop
        // currently calls pollEvent() in a tight loop instead.
        return 0
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

    fun setNativeWindow(windowPointer: Long, width: Int, height: Int) {
        Log.d(TAG, "setNativeWindow($windowPointer, $width, $height)")
    }

    fun updateNativeWindow(windowPointer: Long, width: Int, height: Int) = setNativeWindow(windowPointer, width, height)

    fun releaseGpuSurface() {
        Log.d(TAG, "releaseGpuSurface()")
        if (sessionId != 0L) NativeBridge.detachWindow(sessionId)
    }

    fun setRenderPaused(paused: Boolean) {
        Log.d(TAG, "setRenderPaused($paused)")
    }

    /** Notify native renderer of new surface dimensions. */
    fun setSurfaceSize(width: Int, height: Int) {
        Log.d(TAG, "setSurfaceSize($width, $height)")
    }

    data class FontInfo(
        val cellWidth: Float,
        val cellHeight: Float,
        val descender: Float,
    )

    fun setBackgroundParams(radius: Int, alpha: Int) {
        Log.d(TAG, "setBackgroundParams(blur=$radius, alpha=$alpha)")
    }

    fun setCursorBlinkEnabled(enabled: Boolean) {
        Log.d(TAG, "setCursorBlink($enabled)")
    }

    fun setCursorBlinkSpeedMs(ms: Int) {
        Log.d(TAG, "setCursorBlinkSpeedMs(${ms}ms)")
    }

    fun resetCursorBlink() {
        Log.d(TAG, "resetCursorBlink()")
    }

    fun setBackgroundImage(rgbaData: ByteArray, width: Int, height: Int) {
        Log.d(TAG, "setBackgroundImage(${rgbaData.size}B, ${width}x$height)")
    }

    fun clearBackgroundImage() {
        Log.d(TAG, "clearBackgroundImage()")
    }

    // ── Events ────────────────────────────────────────────────────────
    data class PollResult(
        val bel: Boolean = false,
        val notification: Pair<String, String>? = null,
        val clipboard: String? = null,
        val exit: Boolean = false,
        val exitCode: Int = 0,
        val sessionId: Long = 0L,
        val dialogs: List<DialogRequest> = emptyList(),
        val pickFiles: List<PickFileRequest> = emptyList(),
        val toastText: String? = null,
        val openUrl: String? = null,
        val clipboardGets: List<ClipboardRequest> = emptyList(),
        val clipboardReads: List<ClipboardRequest> = emptyList(),
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
            toastText = later.toastText ?: toastText,
            openUrl = later.openUrl ?: openUrl,
            clipboardGets = clipboardGets + later.clipboardGets,
            clipboardReads = clipboardReads + later.clipboardReads,
            exits = exits + later.exits,
        )
    }

    data class ExitInfo(
        val sessionId: Long,
        val exitCode: Int,
    )

    data class ClipboardRequest(
        val sessionId: Long,
        val requestId: Long,
        val selection: String = "",
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

        is PollEvent.Clipboard ->
            PollResult(clipboard = event.text.ifEmpty { null }, sessionId = event.sessionId)

        is PollEvent.Exit ->
            PollResult(
                exit = true,
                exitCode = event.code,
                sessionId = event.sessionId,
                exits =
                listOf(
                    ExitInfo(
                        sessionId = event.sessionId,
                        exitCode = event.code,
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
    }

    // ── Theme / appearance ────────────────────────────────────────────
    // ADR-0007: placeholder stubs like the Rendering section — the native
    // side has no theme/font JNI exports yet; OSC 10/11/4 color handling
    // lives in the terminal engine and is applied via the palette API.
    fun setTheme(theme: BridgeTheme) {
        Log.d(TAG, "setTheme: ${theme.name}")
        // Rust GhosttyTerminal::set_theme is implemented (try_send); the
        // missing piece is a JNI export + external fun. Deferred with the
        // rest of ADR-0007 surface work (round-115).
    }

    fun setSystemLocale(locale: String) {
        Log.d(TAG, "setSystemLocale($locale)")
    }
    fun setFontFamily(family: String) {
        Log.d(TAG, "setFontFamily($family)")
    }
    fun setFontSize(sizeTenths: Int) {
        Log.d(TAG, "setFontSize($sizeTenths)")
    }
    fun setFontSizeInPlace(sizeTenths: Int) {
        Log.d(TAG, "setFontSizeInPlace($sizeTenths)")
    }
    fun setCursorStyle(style: String) {
        Log.d(TAG, "setCursorStyle($style)")
    }
    fun setExtraFontPaths(paths: List<String>) {
        Log.d(TAG, "setExtraFontPaths($paths)")
    }

    // ADR-0007 stub: font parsing lives in the native font pipeline, which is
    // not wired yet. Returning null makes the installer report the font as
    // unsupported — a deliberate honest failure (per project preference, do
    // not pretend success when the feature is unavailable).
    fun loadFontFile(path: String): String? {
        Log.d(TAG, "loadFontFile($path)")
        return null
    }

    // ── Input ─────────────────────────────────────────────────────────
    fun writeToPty(data: ByteArray): Boolean {
        if (sessionId == 0L) return false
        try {
            // Raw bytes end-to-end: decoding to a Java String here would
            // replace non-UTF-8 sequences (pasted GBK/ISO-8859-1, binary
            // protocols) with U+FFFD and corrupt what the child receives.
            NativeBridge.feedPty(sessionId, data)
        } catch (exception: RuntimeException) {
            // Race: session destroyed between the sessionId check and this
            // call (closeSession/stop on the IO thread). Input for a closed
            // session is dropped by design.
            Log.d(TAG, "writeToPty: session $sessionId already destroyed, dropping")
            return false
        }
        return true
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
            // Route ALL hardware keys through the same encoder the IME path
            // uses. Sending the key NAME (keyCodeToName: "Up", "Home", ...)
            // as literal bytes would write the text "Up" into the PTY — the
            // native writeKey does not parse key names, so vim/less arrows,
            // Home/End, PageUp/Down and Delete were all broken.
            val encoded =
                terminal.emulator.ui.TerminalInputEncoder
                    .encodeKeyEvent(keyCode, unicodeChar, ctrlActive, altActive)
            if (encoded != null) {
                NativeBridge.feedPty(sessionId, encoded)
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
            return false
        } catch (exception: RuntimeException) {
            // Race: session destroyed between the sessionId check and this
            // call. Keystrokes for a closed session are dropped by design.
            Log.d(TAG, "processKeyEvent: session $sessionId already destroyed, dropping")
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
    override fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean?, mode: Byte) {
        queryPort.setSelection(startRow, startCol, endRow, endCol, hasSelection, mode)
    }
    override fun expandAndSetSelection(row: Int, col: Int, mode: Byte): Pair<Pair<Int, Int>, Pair<Int, Int>>? = queryPort.expandAndSetSelection(row, col, mode)

    // ── Search / scrollback ────────────────────────────────────────────
    // ADR-0007: the native query path is not wired yet. The contract lives
    // in TerminalQueryPort: scrollbackLine/scrollbackLength/
    // searchAllInScrollback return "no data" (null/0/empty), isCellEmpty
    // returns true (long-press opens the paste popup only). When the
    // native path lands, swap StubQueryPort for the real port.
    override fun clearSearchHighlights() = queryPort.clearSearchHighlights()
    override fun setSearchHighlights(data: ByteArray) = queryPort.setSearchHighlights(data)
    override fun scrollbackLine(row: Int): String? = queryPort.scrollbackLine(row)
    override fun scrollbackLength(): Int = queryPort.scrollbackLength()
    override fun isCellEmpty(row: Int, col: Int): Boolean = queryPort.isCellEmpty(row, col)
    override fun searchAllInScrollback(query: String, caseSensitive: Boolean, fuzzyMatch: Boolean): List<Triple<Int, Int, Int>>? = queryPort.searchAllInScrollback(query, caseSensitive, fuzzyMatch)
    override fun setScrollOffset(offset: Int) = queryPort.setScrollOffset(offset)

    override fun getTerminalText(): String? = queryPort.getTerminalText()
    override fun listFontFamilies(): List<String>? = queryPort.listFontFamilies()
    override fun getDefaultFontName(): String = queryPort.getDefaultFontName()
    override fun getFontInfo(): FontInfo? = queryPort.getFontInfo()

    companion object {
        private const val TAG = "Bridge"

        /** Max events drained per pollAll() frame — bounds render-thread cost. */
        private const val MAX_EVENTS_PER_POLL = 32
    }
}
