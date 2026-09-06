package terminal.emulator.bridge

import android.util.Log

/**
 * JNI bridge to the native Rust terminal engine (`native.so`).
 *
 * All functions are direct `external fun` JNI exports — no JNA, no wire encoding.
 *
 * Session lifecycle:
 * 1. Call [initSession] to create a new terminal session (returns session ID)
 * 2. Call [feedPty] / [writeKey] to send input
 * 3. Call [pollEvent] every frame (~16ms) to drain events (title, bell, clipboard, exit)
 * 4. Call [destroySession] when done
 *
 * Surface rendering:
 * - [attachWindow] passes Android Surface pointer to the native render thread
 * - [detachWindow] detaches when surface is destroyed
 * - [resize] updates terminal dimensions
 */
object NativeBridge {
    private const val TAG = "NativeBridge"
    private var nativeLoaded = false

    init {
        try {
            System.loadLibrary("native")
            nativeLoaded = true
            Log.i(TAG, "Native library loaded: native")
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "Failed to load native library: ${e.message}")
        }
    }

    fun isNativeLoaded(): Boolean = nativeLoaded

    // ── Session lifecycle ─────────────────────────────────────────────

    /** Create a new terminal session. Returns session ID (0 on failure). */
    @JvmStatic
    @Suppress("LongParameterList") // JNI signature mirrors native init_session - parameters cannot be grouped
    external fun initSession(
        rows: Int,
        cols: Int,
        shell: String,
        home: String,
        user: String,
        path: String,
        workingDirectory: String,
        prefix: String,
        scrollbackLines: Int,
        env: Array<String>?,
    ): Long

    /** Destroy a session by ID. Returns true on success. */
    @JvmStatic
    external fun destroySession(sessionId: Long): Boolean

    /** Switch the active session. Returns true if the session exists. */
    @JvmStatic
    external fun switchSession(sessionId: Long): Boolean

    /** Returns the number of active sessions. */
    @JvmStatic
    external fun getSessionCount(): Int

    /**
     * Returns the current scrollback row count for a session (0 when the
     * session is unknown). Feed for the memory gauge emitted with the
     * frame-timing window: an unbounded scrollback would show up as a
     * monotonically growing row count.
     */
    @JvmStatic
    external fun getScrollbackRows(sessionId: Long): Int

    /**
     * Returns a JSON array of active session IDs.
     * Example: "[1, 2, 3]"
     */
    @JvmStatic
    external fun listSessions(): String?

    // ── Terminal I/O ──────────────────────────────────────────────────

    /** Resize the specified session. */
    @JvmStatic
    external fun resize(sessionId: Long, rows: Int, cols: Int)

    /**
     * Update the PTY winsize pixel fields (ws_xpixel/ws_ypixel) for the
     * specified session, preserving rows/cols. pixel-aware programs
     * `icat`, fullscreen TUIs) read the pixel size from TIOCGWINSZ.
     */
    @JvmStatic
    external fun setPixelSize(sessionId: Long, widthPx: Int, heightPx: Int)

    /** Write raw bytes to the PTY (binary-safe; no UTF-8 mangling). */
    @JvmStatic
    external fun feedPty(sessionId: Long, data: ByteArray)

    /**
     * Feed bytes directly to the VT parser (not the PTY). Test-only path to
     * inject escape sequences (OSC 8 links, DECSET) that must be parsed by
     * the terminal rather than echoed by the shell.
     */
    @JvmStatic
    external fun feedTerminal(sessionId: Long, data: ByteArray)

    /**
     * Encode and submit a key event.
     * @param key Key name (e.g., "a", "Enter", "Escape", "Space")
     * @param mods Modifier bitmask (1=shift, 2=alt, 4=ctrl, 8=meta, 16=super)
     * @param text Optional composed text for IME input (null for non-IME keys)
     */
    @JvmStatic
    external fun writeKey(sessionId: Long, key: String, mods: Int, text: String?)

    /**
     * Encode a mouse event into terminal escape sequences using the Ghostty
     * mouse encoder (SGR/X10/UTF-8 per the application's DECSET selection).
     * Position is in surface pixels; cellW/cellH are the live cell dims.
     * Returns an empty array when mouse reporting is off or encoding fails
     * the event is dropped — zelland renderer/mod.rs pattern).
     */
    @JvmStatic
    external fun encodeMouseEvent(
        sessionId: Long,
        xPx: Float,
        yPx: Float,
        action: Int,
        button: Int,
        cellW: Float,
        cellH: Float,
    ): ByteArray

    /**
     * Whether the remote is on the alternate screen buffer (vim/less/htop).
     * Lock-free mirror maintained by the Rust VT thread; safe to call on every
     * touch-scroll event. Backs [Bridge.isAltScreenActive] so touch-scroll on
     * the alternate screen forwards to the remote as wheel escapes instead of
     * scrolling local scrollback (Haven research: altScreen wheel consumption).
     */
    @JvmStatic
    external fun getAltScreenState(sessionId: Long): Boolean

    /**
     * Query a terminal mode (ghostty `mode_get`); `kind` 0 = DEC private
     * modes, non-zero = ANSI modes. Backs the DECCKM (application cursor
     * keys, DEC private mode 1) lookup used to switch arrow keys between
     * SS3 (`ESC OA`) and CSI (`ESC [ A`) — research-haven.md:141,
     * research-zed-port.md:252.
     */
    @JvmStatic
    external fun getMode(sessionId: Long, modeNum: Int, kind: Int): Boolean

    /**
     * Drain the OSC 133 last-command-output buffer (termlib
     * getLastCommandOutput equivalent); null when empty. Reading clears it.
     */
    @JvmStatic
    external fun getLastCommandOutput(sessionId: Long): String?

    /**
     * Forward an application-window focus change to a session so the child
     * receives DECSET 1004 focus reporting (`\x1b[I` / `\x1b[O`).
     */
    @JvmStatic
    external fun focusEvent(sessionId: Long, focused: Boolean): Boolean

    /**
     * Reply to an MCP `clipboard_get` request with the system clipboard text.
     * Like [dialogResult], a request must be answered exactly once: a second
     * reply for the same request id is a native no-op.
     */
    @JvmStatic
    external fun clipboardResult(sessionId: Long, requestId: Long, text: String)

    // ── Events ────────────────────────────────────────────────────────

    /**
     * Poll the event queue. Returns a JSON-encoded event or null.
     * Call every frame (~16ms) in a coroutine.
     *
     * Event JSON format (serde internal tag, snake_case):
     *   {"event":"bell","session_id":1}
     *   {"event":"clipboard","session_id":1,"text":"copied text"}
     *   {"event":"exit","session_id":1,"code":0}
     *   {"event":"show_dialog","session_id":1,"request_id":2,"dialog_type":"input","title":"","message":"","options":[]}
     *   {"event":"pick_file","session_id":1,"request_id":3,"starting_path":"","filter":""}
     */
    @JvmStatic
    external fun pollEvent(): String?

    /**
     * Take and clear the per-session `new_output` flag (P1-1 scroll-reset
     * signal). Raised by the native PTY ingest path; read-and-cleared by the
     * render thread once per frame as a BYPASS read alongside [pollEvent] —
     * deliberately not a queued event variant so sustained output (tail -f)
     * cannot starve bell/dialog/exit events. See
     * docs/reference/dual-flag-protocol.md.
     */
    @JvmStatic
    external fun consumeNewOutput(sessionId: Long): Boolean

    // ── Surface ───────────────────────────────────────────────────────

    /**
     * Attach an Android Surface for GPU rendering.
     * The native side creates a wgpu surface from the ANativeWindow pointer.
     */
    @JvmStatic
    external fun attachWindow(sessionId: Long, surface: Any, width: Int, height: Int)

    /** Detach the current surface. */
    @JvmStatic
    external fun detachWindow(sessionId: Long)

    /**
     * Render one frame for the session from the CellData fast path
     * ADR-0007). Returns 1 if output was presented, 0 if idle, -1 on error.
     */
    @JvmStatic
    external fun render(sessionId: Long, width: Int, height: Int): Int

    /**
     * Combined render + consumeNewOutput in a single JNI crossing.
     *
     * Returns a packed `Long`:
     *   - bits 0..31  = render count (same as [render])
     *   - bit  32     = new_output flag (1 = PTY output ingested, 0 = idle)
     *
     * Usage:
     * ```kotlin
     * val packed = NativeBridge.renderWithNewOutput(sessionId, width, height)
     * val count = packed.toInt()
     * val newOutput = (packed shr 32) != 0L
     * ```
     */
    @JvmStatic
    external fun renderWithNewOutput(sessionId: Long, width: Int, height: Int): Long

    // ── MCP server ──────────────────────────────────────────────────────

    /** Enable or disable the MCP server. Starts/stops as needed. */
    @JvmStatic
    external fun setMcpEnabled(enabled: Boolean)

    external fun setMcpSocketPath(path: String)

    // ── User input callbacks ────────────────────────────────────────────

    /** Called after showing a dialog or file picker to the user. */
    @JvmStatic
    external fun dialogResult(sessionId: Long, requestId: Long, result: String)

    /**
     * Answer an MCP `run_command` request (same request/response routing as [dialogResult]). `result` is the
     * JSON payload `{"exit_code":N,"stdout":...,"stderr":...}` produced by
     * [terminal.emulator.runtime.TerminalRuntime.dispatchRunCommandRequest].
     * Like [dialogResult], must be answered exactly once.
     */
    external fun runCommandResult(sessionId: Long, requestId: Long, result: String)

    // ── Screenshot ─────────────────────────────────────────────────────

    /**
     * Reply to an MCP `screenshot` request. Kotlin captures RGBA pixels
     * via [captureFrame] and sends them back through this export.
     *
     * Like [dialogResult], must be answered exactly once.
     */
    external fun screenshotResult(
        sessionId: Long,
        requestId: Long,
        width: Int,
        height: Int,
        pixels: ByteArray,
    )

    /**
     * Capture the current terminal frame as RGBA pixels via GPU readback.
     * Must be called from the render thread (which owns the wgpu context).
     *
     * @param sessionId the active session to capture
     * @return byte array with [width:u32 LE][height:u32 LE][RGBA pixels], or null on failure
     */
    @JvmStatic
    external fun captureFrame(sessionId: Long): ByteArray?

    // ── Logging ──────────────────────────────────────────────────────────

    /** Initialise native-side logging. Should be called once at startup. */
    @JvmStatic
    external fun initLogger()

    // ── TerminalQueryPort (native query exports) ─────────────────────────

    /** Terminal title (OSC 0/2) for a session, or null when unknown. */
    @JvmStatic
    external fun getTitle(sessionId: Long): String?

    /** Number of scrollback rows for a session. */
    @JvmStatic
    external fun scrollbackLength(sessionId: Long): Int

    /** Trimmed text of one row, or null for an empty row. Absolute row. */
    @JvmStatic
    external fun scrollbackLine(sessionId: Long, row: Int): String?

    /** Cursor viewport position packed `(y << 32) | x`, or -1 when hidden. */
    @JvmStatic
    external fun getCursorViewportPacked(sessionId: Long): Long

    /** Visible + scrollback text joined by newlines. */
    @JvmStatic
    external fun getTerminalText(sessionId: Long): String?

    /**
     * Extract selection text with Ghostty's native formatter: soft-wrapped
     * lines are joined without '\n' and trailing whitespace is trimmed —
     * the same wrap-aware semantics as termux-app's
     * TerminalBuffer.getSelectedText (joinBackLines). Coordinates are grid
     * rows/cols (absolute: row 0 = top of scrollback). Returns "" on error.
     */
    @JvmStatic
    external fun selectionText(
        sessionId: Long,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        rectangle: Boolean,
    ): String?

    /** OSC 8 hyperlink URI at a grid cell (row 0 = top of scrollback), or null. */
    @JvmStatic
    external fun hyperlinkAt(sessionId: Long, row: Int, col: Int): String?

    /**
     * Search the whole scrollback. Returns a JSON array of
     * `{"row":int,"start_col":int,"end_col":int}` (byte-offset columns),
     * or `[]` on timeout. Debounce from the UI thread.
     */
    @JvmStatic
    external fun searchAllInScrollback(
        sessionId: Long,
        query: String,
        caseSensitive: Boolean,
        fuzzyMatch: Boolean,
    ): String?

    /** True when the cell at (row, col) has no printable codepoint. */
    @JvmStatic
    external fun isCellEmpty(sessionId: Long, row: Int, col: Int): Boolean

    /** Monospace font families known to the pipeline. */
    @JvmStatic
    external fun listFontFamilies(): Array<String>?

    /** Default font family name. */
    @JvmStatic
    external fun getDefaultFontName(): String?

    /** Structured font info as JSON (see [FontInfoDto]); null before the
     *  renderer is initialized. */
    @JvmStatic
    external fun getFontInfo(): String?

    /** Clear renderer search highlights. */
    @JvmStatic
    external fun clearSearchHighlights(sessionId: Long)

    /** Set renderer search highlight ranges (byte-packed, see TerminalSurface). */
    @JvmStatic
    external fun setSearchHighlights(sessionId: Long, data: ByteArray)

    /**
     * Set active text selection (visible-grid rows/cols).
     * mode: 0=Char 1=Word 2=Line 3=Semantic 4=Block (see SelectionMode).
     * selectionBgArgb: theme selection background color, ARGB packed.
     */
    @JvmStatic
    external fun setSelection(
        sessionId: Long,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean,
        mode: Byte,
        selectionBgArgb: Int,
    )
    external fun setTheme(sessionId: Long, data: ByteArray)

    external fun setBackgroundImage(
        sessionId: Long,
        data: ByteArray,
        width: Int,
        height: Int,
    )

    external fun clearBackgroundImage(sessionId: Long)

    external fun setBackgroundParams(sessionId: Long, blurRadius: Int, alphaTenths: Int)

    /**
     * Set the bell-flash overlay phase for the next rendered frame.
     * [phase] decays 1.0 → 0.0 over ~300-500ms after a bell; the native
     * renderer composites a full-screen quad whose alpha scales with it.
     */
    external fun setFlashState(sessionId: Long, phase: Float)

    external fun setCursorBlink(sessionId: Long, enabled: Boolean, speedMs: Int)

    external fun resetCursorBlink(sessionId: Long)

    external fun setRenderPaused(sessionId: Long, paused: Boolean)

    external fun setCursorStyle(sessionId: Long, style: String)

    /** App-level cursor color override in linear RGB (0..1 per channel);
     *  0xFFFFFFFF sentinel clears the override (follow the terminal). */
    external fun setCursorColor(sessionId: Long, r: Float, g: Float, b: Float)

    external fun setFontFamily(sessionId: Long, family: String): Boolean

    /** Slot: 0=bold, 1=italic, 2=bold-italic (ghostty-android 4-slot). */
    external fun setFontFamilyForStyle(sessionId: Long, family: String, slot: Int): Boolean

    external fun setFontSizeInPlace(sessionId: Long, sizeTenths: Int)

    /** Set glyph rasterization scale (device pixel density) for crisp text. */
    external fun setRasterScale(sessionId: Long, scale: Float)

    external fun loadFontFile(sessionId: Long, path: String): String?

    external fun setSystemLocale(sessionId: Long, locale: String)

    external fun setExtraFontPaths(sessionId: Long, paths: Array<String>)

    external fun getCellWidth(sessionId: Long): Float

    external fun getCellHeight(sessionId: Long): Float

    external fun getGridRowsColsPacked(sessionId: Long): Long

    external fun setScrollOffset(sessionId: Long, offset: Int)

    external fun setScrollYPx(sessionId: Long, offsetPx: Float)
}
