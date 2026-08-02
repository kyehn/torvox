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
    external fun initSession(
        rows: Int,
        cols: Int,
        shell: String,
        home: String,
        user: String,
        path: String,
        workingDirectory: String,
        prefix: String,
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
     * Returns a JSON array of active session IDs.
     * Example: "[1, 2, 3]"
     */
    @JvmStatic
    external fun listSessions(): String?

    // ── Terminal I/O ──────────────────────────────────────────────────

    /** Resize the specified session. */
    @JvmStatic
    external fun resize(sessionId: Long, rows: Int, cols: Int)

    /** Write raw bytes to the PTY (binary-safe; no UTF-8 mangling). */
    @JvmStatic
    external fun feedPty(sessionId: Long, data: ByteArray)

    /**
     * Encode and submit a key event.
     * @param key Key name (e.g., "a", "Enter", "Escape", "Space")
     * @param mods Modifier bitmask (1=shift, 2=alt, 4=ctrl, 8=meta, 16=super)
     * @param text Optional composed text for IME input (null for non-IME keys)
     */
    @JvmStatic
    external fun writeKey(sessionId: Long, key: String, mods: Int, text: String?)

    /**
     * Forward an application-window focus change to a session so the child
     * receives DECSET 1004 focus reporting (`\x1b[I` / `\x1b[O`).
     */
    @JvmStatic
    external fun focusEvent(sessionId: Long, focused: Boolean): Boolean

    /**
     * Reply to an MCP `clipboard_get` request with the system clipboard text.
     * Like [dialogResult], a request must be answered exactly once: a second
     * reply for the same request id is a native no-op (round-101).
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
     * (ADR-0007). Returns 1 if output was presented, 0 if idle, -1 on error.
     */
    @JvmStatic
    external fun render(sessionId: Long, width: Int, height: Int): Int

    // ── MCP server ──────────────────────────────────────────────────────

    /** Enable or disable the MCP server. Starts/stops as needed. */
    @JvmStatic
    external fun setMcpEnabled(enabled: Boolean)

    // ── User input callbacks ────────────────────────────────────────────

    /** Called after showing a dialog or file picker to the user. */
    @JvmStatic
    external fun dialogResult(sessionId: Long, requestId: Long, result: String)

    // ── Logging ──────────────────────────────────────────────────────────

    /** Initialise native-side logging. Should be called once at startup. */
    @JvmStatic
    external fun initLogger()

    /** Set the log file path on the native side. */
    @JvmStatic
    external fun setLogFilePath(path: String)
}
