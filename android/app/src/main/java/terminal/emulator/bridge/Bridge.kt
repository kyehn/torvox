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
    val ansi0: Int, val ansi1: Int, val ansi2: Int, val ansi3: Int,
    val ansi4: Int, val ansi5: Int, val ansi6: Int, val ansi7: Int,
    val ansi8: Int, val ansi9: Int, val ansi10: Int, val ansi11: Int,
    val ansi12: Int, val ansi13: Int, val ansi14: Int, val ansi15: Int,
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
 */
class Bridge(private val config: TerminalConfig) {
    @Volatile private var sessionId: Long = 0L

    fun ping(): String {
        if (!NativeBridge.isNativeLoaded()) throw RuntimeException("native library not loaded")
        return "native library OK, sessions=${NativeBridge.getSessionCount()}"
    }

    // ── Session lifecycle ─────────────────────────────────────────────
    fun spawnTerminal(rows: Int, cols: Int): Long {
        sessionId = NativeBridge.initSession(rows, cols)
        return sessionId
    }

    fun close() {
        if (sessionId != 0L) {
            NativeBridge.destroySession(sessionId)
            sessionId = 0L
        }
    }

    fun resize(rows: Int, cols: Int) {
        if (sessionId != 0L) NativeBridge.resize(sessionId, rows, cols)
    }

    /** Recompute grid from pixel dimensions (rows/cols derived by native side). */
    fun recomputeGrid(width: Int, height: Int) {
        // Currently native resize expects rows/cols, not pixels.
        // This is a placeholder — the actual cell-size calculation lives in Rust.
        Log.d(TAG, "recomputeGrid($width,$height) — native resolves rows/cols from events")
    }

    fun getGridRowsColsPacked(): Long =
        // Placeholder; real values come via pollEvent(). Packed: (rows.toLong() shl 32) or cols.toLong()
        (24L shl 32) or 80L

    fun getCellWidth(): Float = 0f
    fun getCellHeight(): Float = 0f

    // ── Grid queries (used by tests and ViewModel) ─────────────────────
    fun getGridRows(): Int = (getGridRowsColsPacked() shr 32).toInt()
    fun getGridCols(): Int = getGridRowsColsPacked().toInt()
    /** Save a frame capture for test verification. */
    fun saveTestFrame(dataDir: String) { Log.d(TAG, "saveTestFrame($dataDir)") }

    // ── Rendering ─────────────────────────────────────────────────────
    /** Render a frame. Returns >0 if output was available, 0 if idle, -1 on error. */
    fun render(shouldSkipOutput: Boolean = false): Int {
        Log.d(TAG, "render(skip=$shouldSkipOutput)")
        // WARNING: no native JNI export for render() — the render loop
        // currently calls pollEvent() in a tight loop instead.
        return 0
    }

    fun waitOutput(timeoutMs: Long): Boolean = false

    fun setNativeWindow(windowPointer: Long, width: Int, height: Int) {
        Log.d(TAG, "setNativeWindow($windowPointer, $width, $height)")
    }

    fun updateNativeWindow(windowPointer: Long, width: Int, height: Int) =
        setNativeWindow(windowPointer, width, height)

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

    fun getDefaultFontName(): String = "monospace"

    data class FontInfo(
        val cellWidth: Float,
        val cellHeight: Float,
        val descender: Float,
    )

    fun getFontInfo(): FontInfo? = null

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
        val dialog: DialogRequest? = null,
        val pickFile: PickFileRequest? = null,
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
        val json = NativeBridge.pollEvent() ?: return PollResult()
        return try {
            val obj = org.json.JSONObject(json)
            val eventType = obj.optString("event", "")
            PollResult(
                bel = eventType == "bell",
                notification = if (eventType == "notification") {
                    Pair(obj.optString("title", ""), obj.optString("body", ""))
                } else null,
                clipboard = if (eventType == "clipboard") obj.optString("text", "").ifEmpty { null } else null,
                exit = eventType == "exit",
                exitCode = if (eventType == "exit") obj.optInt("code", 0) else 0,
                dialog =
                    if (eventType == "show_dialog") {
                        DialogRequest(
                            sessionId = obj.optLong("session_id", 0L),
                            requestId = obj.optLong("request_id", 0L),
                            dialogType = obj.optString("dialog_type", ""),
                            title = obj.optString("title", ""),
                            message = obj.optString("message", ""),
                            options = obj.optJSONArray("options")?.let { arr ->
                                (0 until arr.length()).map { arr.optString(it) }
                            } ?: emptyList(),
                        )
                    } else null,
                pickFile =
                    if (eventType == "pick_file") {
                        PickFileRequest(
                            sessionId = obj.optLong("session_id", 0L),
                            requestId = obj.optLong("request_id", 0L),
                            startingPath = obj.optString("starting_path", ""),
                            filter = obj.optString("filter", ""),
                        )
                    } else null,
            )
        } catch (e: Exception) {
            Log.w(TAG, "pollAll: bad JSON: ${e.message}")
            PollResult()
        }
    }

    // ── Theme / appearance ────────────────────────────────────────────
    fun setTheme(theme: BridgeTheme) {
        Log.d(TAG, "setTheme: ${theme.name}")
        // TODO: add native JNI setTheme when Rust side implements it
    }

    fun setSystemLocale(locale: String) { Log.d(TAG, "setSystemLocale($locale)") }
    fun setFontFamily(family: String) { Log.d(TAG, "setFontFamily($family)") }
    fun setFontSize(sizeTenths: Int) { Log.d(TAG, "setFontSize($sizeTenths)") }
    fun setFontSizeInPlace(sizeTenths: Int) { Log.d(TAG, "setFontSizeInPlace($sizeTenths)") }
    fun setCursorStyle(style: String) { Log.d(TAG, "setCursorStyle($style)") }
    fun setExtraFontPaths(paths: List<String>) { Log.d(TAG, "setExtraFontPaths($paths)") }
    fun loadFontFile(path: String): String? { Log.d(TAG, "loadFontFile($path)"); return null }

    // ── Input ─────────────────────────────────────────────────────────
    fun writeToPty(data: ByteArray): Boolean {
        if (sessionId == 0L) return false
        val text = try {
            data.decodeToString()
        } catch (e: Exception) {
            Log.e(TAG, "writeToPty: invalid UTF-8", e)
            return false
        }
        NativeBridge.feedPty(sessionId, text)
        return true
    }

    fun processKeyEvent(keyCode: Int, modifiers: Byte, action: Int, unicodeChar: Int, unshiftedChar: Int): Boolean {
        Log.d(TAG, "processKeyEvent($keyCode, $modifiers, $action, unicode=$unicodeChar)")
        if (sessionId == 0L) return false
        // For soft keyboard: if unicodeChar is a printable character, send it directly
        if (unicodeChar > 0 && unicodeChar != 0x7F) {
            val ch = unicodeChar.toChar().toString()
            NativeBridge.writeKey(sessionId, ch, modifiers.toInt(), null)
            return true
        }
        // For hardware keyboard: map key code to key name
        val keyName = keyCodeToName(keyCode) ?: return false
        NativeBridge.writeKey(sessionId, keyName, modifiers.toInt(), null)
        return true
    }

    fun focusEvent(focused: Boolean) { Log.d(TAG, "focusEvent($focused)") }

    // ── Terminal queries ──────────────────────────────────────────────
    fun cwd(): String = ""
    fun getTitle(): String? = null
    fun getActiveSessionTitle(): String = getTitle() ?: ""
    // ── Selection ─────────────────────────────────────────────────────
    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean? = null, mode: Byte = 0) {
        Log.d(TAG, "setSelection: ($startRow,$startCol)-($endRow,$endCol)")
    }
    fun setSelectionEndpoint(handleSide: Byte, anchorRow: Int, anchorCol: Int, hasSelection: Boolean) {
        Log.d(TAG, "setSelectionEndpoint($handleSide, ($anchorRow,$anchorCol))")
    }
    fun expandAndSetSelection(row: Int, col: Int, mode: Byte = 0): Pair<Pair<Int, Int>, Pair<Int, Int>>? = null

    // ── Search ─────────────────────────────────────────────────────────
    fun clearSearchHighlights() { Log.d(TAG, "clearSearchHighlights()") }
    fun setSearchHighlights(data: ByteArray) { Log.d(TAG, "setSearchHighlights: ${data.size}B") }
    fun scrollbackLine(row: Int): String? = null
    fun scrollbackLength(): Int = 0
    fun isCellEmpty(row: Int, col: Int): Boolean = true
    fun searchAllInScrollback(query: String, caseSensitive: Boolean, fuzzyMatch: Boolean): List<Triple<Int, Int, Int>>? = null
    fun setScrollOffset(offset: Int) { Log.d(TAG, "setScrollOffset($offset)") }

    /** Return full terminal text content. */
    fun getTerminalText(): String? {
        Log.d(TAG, "getTerminalText() — no native export yet")
        return null
    }

    /** List available font families from native side. */
    fun listFontFamilies(): List<String>? {
        Log.d(TAG, "listFontFamilies() — no native export yet")
        return null
    }

    companion object {
        private const val TAG = "Bridge"
    }
}

/** Minimal key-code → name mapping for [Bridge.processKeyEvent]. */
private fun keyCodeToName(code: Int): String? = when (code) {
    android.view.KeyEvent.KEYCODE_ENTER -> "Enter"
    android.view.KeyEvent.KEYCODE_TAB -> "Tab"
    android.view.KeyEvent.KEYCODE_SPACE -> "Space"
    android.view.KeyEvent.KEYCODE_DEL -> "Backspace"
    android.view.KeyEvent.KEYCODE_FORWARD_DEL -> "Delete"
    android.view.KeyEvent.KEYCODE_ESCAPE -> "Escape"
    android.view.KeyEvent.KEYCODE_DPAD_UP -> "Up"
    android.view.KeyEvent.KEYCODE_DPAD_DOWN -> "Down"
    android.view.KeyEvent.KEYCODE_DPAD_LEFT -> "Left"
    android.view.KeyEvent.KEYCODE_DPAD_RIGHT -> "Right"
    android.view.KeyEvent.KEYCODE_PAGE_UP -> "PageUp"
    android.view.KeyEvent.KEYCODE_PAGE_DOWN -> "PageDown"
    android.view.KeyEvent.KEYCODE_MOVE_HOME -> "Home"
    android.view.KeyEvent.KEYCODE_MOVE_END -> "End"
    android.view.KeyEvent.KEYCODE_INSERT -> "Insert"
    android.view.KeyEvent.KEYCODE_BREAK -> "Pause"
    android.view.KeyEvent.KEYCODE_NUMPAD_ENTER -> "Enter"
    android.view.KeyEvent.KEYCODE_SHIFT_LEFT, android.view.KeyEvent.KEYCODE_SHIFT_RIGHT -> null // modifiers
    android.view.KeyEvent.KEYCODE_ALT_LEFT, android.view.KeyEvent.KEYCODE_ALT_RIGHT -> null
    android.view.KeyEvent.KEYCODE_CTRL_LEFT, android.view.KeyEvent.KEYCODE_CTRL_RIGHT -> null
    android.view.KeyEvent.KEYCODE_META_LEFT, android.view.KeyEvent.KEYCODE_META_RIGHT -> null
    in 7..16 -> ('0' + code - 7).toString()  // KEYCODE_0=7..KEYCODE_9=16
    in 29..54 -> ('a' + code - 29).toString()  // KEYCODE_A=29..KEYCODE_Z=54
    else -> null
}
