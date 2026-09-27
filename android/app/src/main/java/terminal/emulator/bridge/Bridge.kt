package terminal.emulator.bridge

import android.util.Log
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.runCatchingCancellable

/** Shell configuration for a terminal session. */
sealed interface Shell {
    /** Use the system default shell (/system/bin/sh). */
    data object SystemDefault : Shell

    /** Use a custom shell at the given path. */
    data class Custom(val path: String) : Shell
}

/**
 * ARGB → linear RGB floats (0..1 per channel) for the JNI cursor-color channel. The alpha byte is
 * intentionally dropped (the renderer treats the cursor as opaque).
 */
internal fun argbToRgbFloats(argb: Int): FloatArray = floatArrayOf(
    (argb shr 16 and 0xFF) / 255f,
    (argb shr 8 and 0xFF) / 255f,
    (argb and 0xFF) / 255f,
)

/**
 * Terminal theme expressed as ARGB ints for the native renderer. Matches
 * [terminal.emulator.ui.theme.TerminalTheme] conversion in makeBridgeTheme().
 */
data class BridgeTheme(
    val name: String,
    val background: Int,
    val foreground: Int,
    val cursor: Int,
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

/** Configuration passed to [createBridge]. */
data class TerminalConfig(
    val shell: Shell,
    val rows: Int,
    val cols: Int,
    val theme: BridgeTheme,
    val home: String,
    val workingDirectory: String,
    val prefix: String,
    val mkshrcPath: String,
    val scrollbackLines: Int,
    val fontSizeTenths: Int,
)

/** Create a new Bridge instance wrapping [NativeBridge] JNI exports. */
fun createBridge(config: TerminalConfig): Bridge = Bridge(config)

/**
 * Instance bridge wrapping [NativeBridge] static JNI exports.
 *
 * Each [Bridge] holds a session ID and manages session lifecycle so callers don't touch session IDs
 * directly. Every per-session JNI call goes through [onSession], which reports the one shared
 * "no result available" case instead of repeating the guard in each method.
 *
 * Bridge is a gateway to the native side by design; the function count is the JNI surface, not an
 * interface smell.
 */
// when-dispatch over the PollEvent sealed class — one branch per variant.
class Bridge(private val config: TerminalConfig) : TerminalQueryPort {
    /**
     * ADR-0007: native query path wired — all queries delegate to [NativeQueryPort], which maps 1:1
     * to the JNI query exports native/src/android/ffi.rs, "TerminalQueryPort" section). The stub only
     * backs the no-session window (sessionId == 0, before spawn).
     */
    private val queryPort: TerminalQueryPort = NativeQueryPort { sessionId }

    @Volatile private var sessionId: Long = 0L

    @Volatile private var lastSurfaceWidth: Int = 0

    @Volatile private var lastSurfaceHeight: Int = 0

    /**
     * Every-PTY-write hook: invoked with `SystemClock.elapsedRealtimeNanos()` on EVERY PTY write
     * path ([Bridge.writeToPty], [processKeyEvent], [encodeMouseEvent]) so hardware keys — which
     * bypass [terminal.emulator.runtime.TerminalRuntime.writeToPty] — are stamped for the
     * input→echo latency probe (emulator-performance-verification).
     *
     * Also the T3 render-wake seam: called on the same paths that would otherwise never notify the
     * render loop, so the SessionEntry wiring raises [terminal.emulator.runtime.SessionEntry.notifyRender]
     * here and a backspace after >5s idle no longer waits out the 500ms idle-latch tick for its echo.
     */
    @Volatile var onPtyWrite: ((Long) -> Unit)? = null

    /**
     * 统一转发一次 JNI 调用。
     *
     * 两种「拿不到结果」的状态共用 [onUnavailable]：会话尚未建立（id 为 0），
     * 以及检查之后、调用之前会话被销毁（native 对未知会话抛 RuntimeException，
     * 见 DESIGN 的 Activity 重建/进程回收场景）。两者都不是崩溃理由，但都记
     * 警告而非静默丢弃。
     */
    private inline fun <T> onSession(name: String, onUnavailable: T, call: (Long) -> T): T {
        val id = sessionId
        if (id == 0L) return onUnavailable
        return try {
            call(id)
        } catch (exception: RuntimeException) {
            LogUtil.w(TAG, "$name: 会话 $id 已销毁，返回缺省值（${exception.javaClass.simpleName}）")
            onUnavailable
        }
    }

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
                config.workingDirectory,
                config.prefix,
                config.mkshrcPath,
                config.scrollbackLines,
            )
        return sessionId
    }

    /**
     * 后台预热渲染器与字体库：PTY 已 spawn（shell 并行启动），attach 前把 wgpu 初始化与
     * 200+ 系统字体加载移到后台线程，不阻塞首帧链。
     */
    fun prefetchRenderStateAsync(scope: kotlinx.coroutines.CoroutineScope) {
        scope.launch(terminal.emulator.util.TerminalDispatchers.inputOutput) {
            try {
                NativeBridge.prefetchRenderState()
            } catch (exception: Exception) {
                android.util.Log.w("Bridge", "prefetchRenderState failed", exception)
            }
        }
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
        onSession("resize", Unit) { NativeBridge.resize(it, rows, cols) }
    }

    /**
     * Update the PTY winsize pixel fields (ws_xpixel/ws_ypixel) for this session, preserving
     * rows/cols. The Kotlin host calls this alongside each grid resize with the surface's pixel
     * dimensions so pixel-aware programs (`icat`, fullscreen TUIs) read real pixels from TIOCGWINSZ
     * ghostty-android pty_jni.c:84-87).
     */
    fun setPixelSize(widthPx: Int, heightPx: Int) {
        onSession("setPixelSize", Unit) { NativeBridge.setPixelSize(it, widthPx, heightPx) }
    }

    /**
     * Recompute the grid from pixel dimensions. The cell-size calculation lives in Rust: the renderer
     * derives cell metrics from the font pipeline, and [TerminalRuntime.syncGridDimensions] pulls the
     * real grid via [getGridRowsColsPacked] after a resize. This method only logs: the native side
     * resolves rows/cols from events).
     */
    fun recomputeGrid(width: Int, height: Int) {
        Log.d(TAG, "recomputeGrid($width,$height) — native resolves rows/cols from events")
    }

    fun getGridRowsColsPacked(): Long = onSession("getGridRowsColsPacked", 0L, NativeBridge::getGridRowsColsPacked)

    fun getCellWidth(): Float = onSession("getCellWidth", 0f, NativeBridge::getCellWidth)

    fun getCellHeight(): Float = onSession("getCellHeight", 0f, NativeBridge::getCellHeight)

    // ── Rendering ─────────────────────────────────────────────────────
    // ADR-0007 surface integration is implemented:
    // render/attachSurface/releaseGpuSurface/setRenderPaused map to the
    // wgpu renderer via JNI.

    /**
     * Render a frame. Returns >0 if output was available, 0 if idle, -1 on error.
     *
     * 「无会话」与「会话已销毁」都归为 idle（0）而非错误：两者都是「没有可渲染的会话」，
     * 且不会随首帧重试自愈（重试只对原生返回的 -1 有意义）。首帧重试依赖的 -1 由
     * `NativeBridge.render` 原样返回，不经 [onSession]。
     */
    fun render(): Int = onSession("render", RENDER_IDLE) {
        NativeBridge.render(it, lastSurfaceWidth, lastSurfaceHeight)
    }

    /**
     * Combined render + consumeNewOutput in a single JNI crossing (saves ~0.1-0.3ms per frame vs two
     * separate calls). Returns [RenderResult] with the render count, the new-output flag and the
     * viewport cursor row (-1 when hidden/off-viewport, drives the IME-follow pan).
     */
    fun renderWithNewOutput(): RenderResult =
        onSession("renderWithNewOutput", RenderResult(RENDER_IDLE, false, CURSOR_ROW_UNKNOWN)) {
            val packed = NativeBridge.renderWithNewOutput(it, lastSurfaceWidth, lastSurfaceHeight)
            val count = packed.toInt()
            // 仅屏蔽第 32 位：第 33..48 位承载光标行号，不得泄漏到输出标志
            // （空闲闭锁依赖该标志）。
            val newOutput = ((packed shr 32) and 0x1L) != 0L
            val cursorRow =
                ((packed shr 33) and CURSOR_ROW_HIDDEN_BITS.toLong()).toInt().let { raw ->
                    if (raw == CURSOR_ROW_HIDDEN_BITS) CURSOR_ROW_UNKNOWN else raw
                }
            RenderResult(count, newOutput, cursorRow)
        }

    data class RenderResult(val count: Int, val newOutput: Boolean, val cursorRow: Int)

    /**
     * Take and clear the native `new_output` flag for this session (P1-1 scroll-reset signal,
     * see docs/specification/REFERENCE.md). Called once per frame from the
     * render thread; returns true when PTY output was ingested since the last call. Unknown/destroyed
     * sessions report false.
     */
    fun consumeNewOutput(): Boolean = onSession("consumeNewOutput", false, NativeBridge::consumeNewOutput)

    /** Attach the Android Surface for GPU rendering (ADR-0007). */
    fun attachSurface(surface: Any, width: Int, height: Int) {
        lastSurfaceWidth = width
        lastSurfaceHeight = height
        onSession("attachWindow", Unit) { NativeBridge.attachWindow(it, surface, width, height) }
    }

    /**
     * Parks the calling thread for [timeoutMs] (or until [TerminalRuntime.notifyRender] unparks it,
     * whichever comes first).
     *
     * There is no native render JNI export yet (ADR-0007: surface integration pending), so this
     * park-based sleep both bounds the render-loop poll cadence (no 100% CPU busy-spin) and pairs
     * with [TerminalRuntime.SessionEntry.notifyRender] which calls LockSupport.unpark on the render
     * thread.
     *
     * The return value is advisory only — callers re-check the interrupt flag themselves after this
     * returns; `parkNanos` returns on interrupt without clearing the flag, so `Thread.interrupted()`
     * still sees it. Returns true if the wait was not interrupted.
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
        onSession("setRenderPaused", Unit) { NativeBridge.setRenderPaused(it, paused) }
    }

    // ── Events ────────────────────────────────────────────────────────
    data class PollResult(
        val clipboard: String? = null,
        val exit: Boolean = false,
        val exitCode: Int = 0,
        // native-measured child lifetime for the first exit.
        val exitAliveMs: Long = 0,
        val sessionId: Long = 0L,
        val clipboardReads: List<ClipboardRequest> = emptyList(),
        // Every exit event seen this frame, in order. The single-slot
        // exit/sessionId/exitCode fields above describe only the FIRST one;
        // extra exits in the same frame must be reaped from this list or
        // they would leak (native exit_reported is set at push and never
        // re-sent).
        val exits: List<ExitInfo> = emptyList(),
        // BEL 振铃到达（同帧 sticky；提示动作待定行为后另起一步）。
        val bell: Boolean = false,
    ) {
        /** Merge a later polled event into this result; later wins for scalar fields. */
        fun merge(later: PollResult): PollResult = PollResult(
            clipboard = later.clipboard ?: clipboard,
            exit = exit || later.exit,
            // exitCode belongs to the same (first) exit as sessionId.
            exitCode = if (later.exit && !exit) later.exitCode else exitCode,
            // alive_ms travels with its exit event.
            exitAliveMs = if (later.exit && !exit) later.exitAliveMs else exitAliveMs,
            // sessionId only serves exit attribution. The FIRST exit
            // seen in a frame wins: a later non-exit event must not
            // overwrite the exiting session's id (which would reap a
            // live session).
            sessionId = if (later.exit && !exit) later.sessionId else sessionId,
            // Request events accumulate: each one carries a distinct
            // request_id and must be dispatched exactly once.
            clipboardReads = clipboardReads + later.clipboardReads,
            exits = exits + later.exits,
            // Bell is sticky like exit: once raised in a frame it stays.
            bell = bell || later.bell,
        )
    }

    data class ExitInfo(
        val sessionId: Long,
        val exitCode: Int,
        // child lifetime (ms) measured natively — diagnostics payload,
        // not Kotlin event latency.
        val exitAliveMs: Long = 0,
    )

    data class ClipboardRequest(val sessionId: Long, val requestId: Long, val selection: String = "")

    fun pollAll(): PollResult {
        // Drain up to MAX_EVENTS_PER_POLL queued events per frame so a
        // backlog is consumed in a few frames instead of one event per
        // 16ms frame.
        // Results merge: a later event of the same kind wins (exit is
        // sticky — later events for a dead session are stale).
        var result = PollResult()
        // 有界排空：队列见空即 break；计数器具名（下划线形式需实验开关）。
        var pollAttempt = 0
        while (pollAttempt < MAX_EVENTS_PER_POLL) {
            pollAttempt += 1
            val json = NativeBridge.pollEvent() ?: break
            val parsed =
                try {
                    parseEvent(json)
                } catch (e: Exception) {
                    Log.w(TAG, "pollAll: bad JSON: ${e.message}")
                    continue
                }
            result = result.merge(parsed)
            // Do NOT break on exit: events queued after the Exit would
            // otherwise be stranded in the native queue — no other session
            // drains them. Exit is sticky in merge, so draining on is harmless.
        }
        return result
    }

    private fun parseEvent(json: String): PollResult = when (
        val event = pollEventJson.decodeFromString<PollEvent>(
            json,
        )
    ) {
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

        is PollEvent.Bell ->
            PollResult(bell = true, sessionId = event.sessionId)
    }

    // ── Theme / appearance ────────────────────────────────────────────
    // 端到端接线：setTheme 打包 54 字节（背景 3 + 前景 3 + ansi 48）交给原生调色板；
    // OSC 10/11/4 颜色处理位于终端引擎内部并经调色板 API 应用。光标颜色走独立的
    // setCursorColor 通道，以保持 54 字节布局稳定（ffi.rs 校验精确长度）。
    fun setTheme(theme: BridgeTheme) {
        Log.d(TAG, "setTheme: ${theme.name}")
        val data = ByteArray(THEME_PACKED_BYTES)
        fun packColor(offset: Int, argb: Int) {
            data[offset] = (argb shr 16 and 0xFF).toByte()
            data[offset + 1] = (argb shr 8 and 0xFF).toByte()
            data[offset + 2] = (argb and 0xFF).toByte()
        }
        packColor(0, theme.background)
        packColor(3, theme.foreground)
        val ansi =
            listOf(
                theme.ansi0,
                theme.ansi1,
                theme.ansi2,
                theme.ansi3,
                theme.ansi4,
                theme.ansi5,
                theme.ansi6,
                theme.ansi7,
                theme.ansi8,
                theme.ansi9,
                theme.ansi10,
                theme.ansi11,
                theme.ansi12,
                theme.ansi13,
                theme.ansi14,
                theme.ansi15,
            )
        ansi.forEachIndexed { index, color -> packColor(6 + index * 3, color) }
        val cursorRgb = argbToRgbFloats(theme.cursor)
        onSession("setTheme", Unit) {
            NativeBridge.setTheme(it, data)
            NativeBridge.setCursorColor(it, cursorRgb[0], cursorRgb[1], cursorRgb[2])
        }
    }

    fun setSystemLocale(locale: String) {
        Log.d(TAG, "setSystemLocale($locale)")
        onSession("setSystemLocale", Unit) { NativeBridge.setSystemLocale(it, locale) }
    }

    private var lastExtraFontPaths: List<String> = emptyList()

    fun setExtraFontPaths(paths: List<String>) {
        Log.d(TAG, "setExtraFontPaths($paths)")
        // The native call rebuilds the font pipeline (atlas realloc):
        // skip repeats, the drop-in dir content is picked up on rebuild.
        if (paths == lastExtraFontPaths) return
        lastExtraFontPaths = paths
        onSession("setExtraFontPaths", Unit) { NativeBridge.setExtraFontPaths(it, paths.toTypedArray()) }
    }

    // font.ttf 覆盖探测缓存：native 每次加载都会追加字体条目，
    // 同一文件只探测一次，内容变化才重新探测。
    private var lastDefaultFontProbeKey: String? = null
    private var lastDefaultFontProbeFamily: String? = null

    private fun probeDefaultFontFile(): String? {
        val file =
            terminal.emulator.termuxDefaultFontFile(config.home) ?: run {
                lastDefaultFontProbeKey = null
                lastDefaultFontProbeFamily = null
                return null
            }
        val key = "${file.absolutePath}:${file.lastModified()}:${file.length()}"
        if (key == lastDefaultFontProbeKey) return lastDefaultFontProbeFamily
        val family = loadFontFile(file.absolutePath)
        lastDefaultFontProbeKey = key
        lastDefaultFontProbeFamily = family
        return family
    }

    fun setFontFamily(family: String): Boolean {
        Log.d(TAG, "setFontFamily($family)")
        // DESIGN 字体选择节：font.ttf 存在即默认，不复制文件，直接应用覆盖存入设置。
        val override = probeDefaultFontFile()
        return onSession("setFontFamily", false) { NativeBridge.setFontFamily(it, override ?: family) }
    }

    fun setFontSize(sizeTenths: Int) {
        Log.d(TAG, "setFontSize($sizeTenths)")
        setFontSizeInPlace(sizeTenths)
    }

    fun setFontSizeInPlace(sizeTenths: Int) {
        Log.d(TAG, "setFontSizeInPlace($sizeTenths)")
        onSession("setFontSizeInPlace", Unit) { NativeBridge.setFontSizeInPlace(it, sizeTenths) }
    }

    fun setRasterScale(scale: Float) {
        onSession("setRasterScale", Unit) { NativeBridge.setRasterScale(it, scale) }
    }

    // Custom font loading probes the file in native code (fontdb), registers
    // it with the renderer and returns the family name; null on failure.
    fun loadFontFile(path: String): String? {
        Log.d(TAG, "loadFontFile($path)")
        return onSession("loadFontFile", null) { NativeBridge.loadFontFile(it, path) }
    }

    // ── Input ─────────────────────────────────────────────────────────
    fun feedTerminal(data: ByteArray): Boolean = onSession("feedTerminal", false) {
        NativeBridge.feedTerminal(it, data)
        true
    }

    fun writeToPty(data: ByteArray): Boolean = onSession("writeToPty", false) {
        // 原始字节端到端：此处解码为 Java String 会把非 UTF-8 序列
        // （粘贴的 GBK/ISO-8859-1、二进制协议）替换为 U+FFFD 并破坏子进程收到的内容。
        NativeBridge.feedPty(it, data)
        onPtyWrite?.invoke(android.os.SystemClock.elapsedRealtimeNanos())
        true
    }

    /**
     * Encode a mouse event via the Ghostty mouse encoder and write the resulting escape sequence to
     * the PTY. Returns true when a sequence was produced and written; false when mouse reporting is
     * disabled, encoding failed, or the session is gone (event dropped).
     */
    fun encodeMouseEvent(xPx: Float, yPx: Float, action: Int, button: Int, cellW: Float, cellH: Float): Boolean {
        val bytes =
            onSession("encodeMouseEvent", ByteArray(0)) {
                NativeBridge.encodeMouseEvent(it, xPx, yPx, action, button, cellW, cellH)
            }
        if (bytes.isEmpty()) return false
        return writeToPty(bytes)
    }

    /**
     * Whether the remote is on the alternate screen buffer (vim/less/htop). Lock-free; safe to call
     * on every touch-scroll event. When true, touch scroll gestures must be forwarded to the remote
     * as mouse-wheel escapes see [TerminalSurface] onScroll) rather than scrolling local scrollback.
     */
    fun isAltScreenActive(): Boolean = onSession("getAltScreenState", false, NativeBridge::getAltScreenState)

    /**
     * Whether the terminal is in application cursor mode (DECCKM, DEC private mode 1). Arrow keys
     * must then be encoded SS3 (`ESC OA`) instead of CSI (`ESC [ A`) — see docs/specification/REFERENCE.md.
     * Queried only for arrow-key key events.
     */
    fun isAppCursorMode(): Boolean =
        onSession("getMode", false) { NativeBridge.getMode(it, DEC_PRIVATE_MODE_APP_CURSOR, 0) }

    fun processKeyEvent(keyCode: Int, modifiers: Byte, action: Int, unicodeChar: Int, unshiftedChar: Int): Boolean {
        Log.d(TAG, "processKeyEvent($keyCode, $modifiers, $action)")
        // 仅 ACTION_DOWN 产生输出：onKeyDown 与 onKeyUp 都走这里，若在 UP 也写入
        // 会把每次击键写两遍（"llss"、双击 Enter、双击 Ctrl+C）。ACTION_UP 返回
        // false，交由平台默认实现（空操作）处理。
        if (action != android.view.KeyEvent.ACTION_DOWN) return false
        return onSession("processKeyEvent", false) { id ->
            val modifierBits = modifiers.toInt()
            val ctrlActive = modifierBits and 4 != 0
            val altActive = modifierBits and 2 != 0
            // DECCKM：终端处于应用光标模式（DEC 私有模式 1）时方向键须编码为 SS3
            // （`ESC OA`）而非 CSI（`ESC [ A`），见 docs/specification/REFERENCE.md。
            // 仅方向键查询，避免每次击键都做一次 mode_get 往返。
            val appCursorMode = keyCode in APP_CURSOR_KEY_CODES && isAppCursorMode()
            // 所有硬件按键都走输入法同一条编码路径。直接发送键名
            // （keyCodeToName："Up"、"Home"…）会把这些字面文本写进 PTY —— 原生
            // writeKey 不解析键名，vim/less 的方向键、Home/End、PageUp/Down、
            // Delete 都会失效。
            val encoded =
                terminal.emulator.ui.TerminalInputEncoder.encodeKeyEvent(
                    keyCode,
                    unicodeChar,
                    ctrlActive,
                    altActive,
                    appCursorMode,
                )
            if (encoded != null) {
                NativeBridge.feedPty(id, encoded)
                onPtyWrite?.invoke(android.os.SystemClock.elapsedRealtimeNanos())
                return@onSession true
            }
            // 编码器未覆盖的按键走原始可打印 unicode（补全平面安全）。按 Ctrl 时
            // 绝不走此路：原生 writeKey 会把单字节 ASCII 折叠为 c & 0x1F，把
            // Ctrl+9/Ctrl+0 变成 Ctrl+Y/Ctrl+P。Ctrl+可打印键要么在上面已编码，
            // 要么被有意丢弃（Ctrl+9/0 无传统映射）。
            if (!ctrlActive && unicodeChar > 0 && unicodeChar != 0x7F) {
                if (!Character.isValidCodePoint(unicodeChar)) return@onSession false
                NativeBridge.writeKey(id, String(Character.toChars(unicodeChar)), modifierBits, null)
                return@onSession true
            }
            // 部分输入法（Gboard 在 InputType.TYPE_NULL 下）发出的按键事件
            // unicodeChar == 0，尽管该键是可打印字母。回退到虚拟键盘的按键字符
            // 映射表推导字符，保证这类击键仍能到达 PTY。
            if (!ctrlActive && keyCode in android.view.KeyEvent.KEYCODE_A..android.view.KeyEvent.KEYCODE_Z) {
                val derived =
                    android.view.KeyCharacterMap.load(android.view.KeyCharacterMap.VIRTUAL_KEYBOARD)
                        .get(keyCode, 0)
                if (derived > 0) {
                    NativeBridge.writeKey(id, derived.toChar().toString(), modifierBits, null)
                    return@onSession true
                }
            }
            false
        }
    }

    fun focusEvent(focused: Boolean) {
        onSession("focusEvent", Unit) { NativeBridge.focusEvent(it, focused) }
    }

    // ── Terminal queries (delegated to TerminalQueryPort seam) ────────
    override fun getTitle(): String? = queryPort.getTitle()

    override fun getActiveSessionTitle(): String = queryPort.getActiveSessionTitle()

    // ── Selection ─────────────────────────────────────────────────────
    override fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean?) {
        queryPort.setSelection(startRow, startCol, endRow, endCol, hasSelection)
    }

    // ── Search / scrollback ────────────────────────────────────────────
    // 查询方法经 NativeQueryPort 转发到真实的原生 JNI 路径。native 对未知会话
    // 抛 IllegalArgumentException（如 bridge.close() 到会话表移除之间的窗口），
    // 在此转为缺省值返回，使 UI/触摸路径不崩。
    override fun clearSearchHighlights() = queryPort.clearSearchHighlights()

    override fun setSearchHighlights(data: ByteArray) = queryPort.setSearchHighlights(data)

    override fun scrollbackLine(row: Int): String? = runCatchingCancellable {
        queryPort.scrollbackLine(
            row,
        )
    }.getOrNull()

    override fun scrollbackLength(): Int = runCatchingCancellable { queryPort.scrollbackLength() }.getOrDefault(0)

    override fun cursorViewportPacked(): Long = runCatchingCancellable {
        queryPort.cursorViewportPacked()
    }.getOrDefault(
        -1L,
    )

    override fun isCellEmpty(row: Int, col: Int): Boolean = runCatchingCancellable {
        queryPort.isCellEmpty(
            row,
            col,
        )
    }.getOrDefault(true)

    override fun searchAllInScrollback(query: String, caseSensitive: Boolean): List<Triple<Int, Int, Int>>? =
        runCatchingCancellable { queryPort.searchAllInScrollback(query, caseSensitive) }
            .getOrNull()

    override fun setScrollOffset(offset: Int) = queryPort.setScrollOffset(offset)

    override fun setScrollYPx(offsetPx: Float) = queryPort.setScrollYPx(offsetPx)

    override fun getTerminalText(): String? = runCatchingCancellable { queryPort.getTerminalText() }.getOrNull()

    override fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int): String? =
        runCatchingCancellable {
            queryPort.selectionText(startRow, startCol, endRow, endCol)
        }
            .getOrNull()

    override fun hyperlinkAt(row: Int, col: Int): String? = runCatchingCancellable {
        queryPort.hyperlinkAt(
            row,
            col,
        )
    }.getOrNull()

    // 上游选择派生：native 侧已安装选区并回传界限；unknown session 异常
    // 与其余查询方法一致转为缺省值（UI/触摸路径不崩）。
    override fun selectWordAt(row: Int, col: Int): IntArray? = runCatchingCancellable {
        queryPort.selectWordAt(row, col)
    }.getOrNull()

    override fun selectLineAt(row: Int, col: Int): IntArray? = runCatchingCancellable {
        queryPort.selectLineAt(row, col)
    }.getOrNull()

    override fun selectAll(): IntArray? = runCatchingCancellable { queryPort.selectAll() }.getOrNull()

    override fun listFontFamilies(): List<String>? = runCatchingCancellable { queryPort.listFontFamilies() }.getOrNull()

    override fun getDefaultFontName(): String = runCatchingCancellable { queryPort.getDefaultFontName() }.getOrDefault(
        "",
    )

    override fun getFontInfo(): String? = runCatchingCancellable { queryPort.getFontInfo() }.getOrNull()

    companion object {
        private const val TAG = "Bridge"

        /** renderWithNewOutput packing: cursor row bits 33..48, this value = hidden/off-viewport. */
        const val CURSOR_ROW_HIDDEN_BITS = 0xFFFF

        /** render 缺省返回值：无可渲染的会话（未建立或已销毁），按 idle 处理。 */
        private const val RENDER_IDLE = 0

        /** Decoded cursor row when hidden/off-viewport (or no session). */
        const val CURSOR_ROW_UNKNOWN = -1

        /** Max events drained per pollAll() frame — bounds render-thread cost. */
        private const val MAX_EVENTS_PER_POLL = 32

        /** setTheme 打包长度：背景 3 + 前景 3 + 16 色 × 3 = 54 字节，ffi.rs 校验精确长度。 */
        private const val THEME_PACKED_BYTES = 54

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
