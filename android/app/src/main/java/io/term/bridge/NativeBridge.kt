package io.term.bridge

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
    external fun initSession(rows: Int, cols: Int): Long

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

    /** Write raw text to the PTY (keyboard input). */
    @JvmStatic
    external fun feedPty(sessionId: Long, data: String)

    /**
     * Encode and submit a key event.
     * @param key Key name (e.g., "a", "Enter", "Escape", "Space")
     * @param mods Modifier bitmask (1=shift, 2=alt, 4=ctrl, 8=meta, 16=super)
     * @param text Optional composed text for IME input (null for non-IME keys)
     */
    @JvmStatic
    external fun writeKey(sessionId: Long, key: String, mods: Int, text: String?)

    // ── Events ────────────────────────────────────────────────────────

    /**
     * Poll the event queue. Returns a JSON-encoded event or null.
     * Call every frame (~16ms) in a coroutine.
     *
     * Event JSON format:
     *   {"Title":{"session_id":1,"title":"new title"}}
     *   {"Bell":{"session_id":1}}
     *   {"Clipboard":{"session_id":1,"text":"copied text"}}
     *   {"Exit":{"session_id":1,"code":0}}
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

    // ── MCP server ──────────────────────────────────────────────────────

    /** Enable or disable the MCP server. Starts/stops as needed. */
    @JvmStatic
    external fun setMcpEnabled(enabled: Boolean)

    // ── User input callbacks ────────────────────────────────────────────

    /** Called after showing a dialog or file picker to the user. */
    @JvmStatic
    external fun dialogResult(sessionId: Long, requestId: Long, result: String)

    // ── Session persistence ─────────────────────────────────────────────

    /** Set the persistence save path for a session (empty = disable). */
    @JvmStatic
    external fun setSessionSavePath(sessionId: Long, path: String)
}
