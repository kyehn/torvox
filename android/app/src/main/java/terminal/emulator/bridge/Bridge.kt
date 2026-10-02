package terminal.emulator.bridge

import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.runCatchingCancellable

/** 终端会话的 shell 配置。 */
sealed interface Shell {
    /** 使用系统默认 shell（/system/bin/sh）。 */
    data object SystemDefault : Shell

    /** 使用指定路径的自定义 shell。 */
    data class Custom(val path: String) : Shell
}

/**
 * ARGB → 线性 RGB 浮点（每通道 0..1），供 JNI 光标颜色通道使用。
 * alpha 字节被刻意丢弃（渲染器将光标视为不透明）。
 */
internal fun argbToRgbFloats(argb: Int): FloatArray = floatArrayOf(
    (argb shr 16 and 0xFF) / 255f,
    (argb shr 8 and 0xFF) / 255f,
    (argb and 0xFF) / 255f,
)

/** 以 ARGB 整数表达的终端主题，供原生渲染器使用，对应 makeBridgeTheme() 的转换。 */
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

/** 传给 [createBridge] 的配置。 */
data class TerminalConfig(
    val shell: Shell,
    val rows: Int,
    val cols: Int,
    val theme: BridgeTheme,
    val home: String,
    val workingDirectory: String,
    val prefix: String,
    val mkshrcPath: String,
    val fontSizeTenths: Int,
)

/** 创建包裹 [NativeBridge] JNI 导出的 Bridge 实例。 */
fun createBridge(config: TerminalConfig): Bridge = Bridge(config)

/**
 * 包裹 [NativeBridge] 静态 JNI 导出的实例桥接。每个 Bridge 持有会话 ID 并管理生命周期，
 * 使调用方不直接接触会话 ID；所有按会话的 JNI 调用都经 [onSession]，集中处理「拿不到结果」的情况。
 * Bridge 按设计就是通往原生的网关，函数数量对应 JNI 表面而非接口异味。
 */
// 对 PollEvent 密封类做 when 分派，每个变体一个分支。
class Bridge(private val config: TerminalConfig) : TerminalQueryPort {
    /** 原生查询路径：所有查询委托给 [NativeQueryPort]，它与 ffi.rs 的 JNI 查询导出 1:1 对应。 */
    private val queryPort: TerminalQueryPort = NativeQueryPort { sessionId }

    @Volatile private var sessionId: Long = 0L

    @Volatile private var lastSurfaceWidth: Int = 0

    @Volatile private var lastSurfaceHeight: Int = 0

    /**
     * 每次 PTY 写入的钩子：在所有 PTY 写路径（[Bridge.writeToPty]、[processKeyEvent]、
     * [encodeMouseEvent]）上以 `SystemClock.elapsedRealtimeNanos()` 调用，使绕过
     * TerminalRuntime.writeToPty 的硬件按键也能为输入→回显延迟探针打点。
     * 同时是渲染唤醒接缝：空闲 >5s 后的退格不再需要等满 500ms 空闲闭锁才能看到回显。
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

    // ── 会话生命周期 ──

    /** 把配置的 [Shell] 解析为绝对可执行路径。 */
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
                LogUtil.w("Bridge", "prefetchRenderState failed", exception)
            }
        }
    }

    fun close() {
        if (sessionId != 0L) {
            try {
                NativeBridge.destroySession(sessionId)
            } catch (exception: Throwable) {
                // 清理路径：此处失败（未知会话的 RuntimeException、库部分加载的 UnsatisfiedLinkError）
                // 绝不能外逃——调用方只捕获 Exception，Error 会直达全局处理器并杀掉进程。
                // 无论成败都移除注册表项，原生容忍未知 ID。
                LogUtil.e(TAG, "close: destroySession failed", exception)
            }
            sessionId = 0L
        }
    }

    fun resize(rows: Int, cols: Int) {
        onSession("resize", Unit) { NativeBridge.resize(it, rows, cols) }
    }

    /**
     * 更新本会话 PTY winsize 的像素字段（ws_xpixel/ws_ypixel）而保持 rows/cols。
     * 每次网格 resize 时 Kotlin 侧同时以 Surface 像素尺寸调用，使感知像素的程序
     * （icat、全屏 TUI）能经 TIOCGWINSZ 读到真实像素。
     */
    fun setPixelSize(widthPx: Int, heightPx: Int) {
        onSession("setPixelSize", Unit) { NativeBridge.setPixelSize(it, widthPx, heightPx) }
    }

    fun getGridRowsColsPacked(): Long = onSession("getGridRowsColsPacked", 0L, NativeBridge::getGridRowsColsPacked)

    fun getCellWidth(): Float = onSession("getCellWidth", 0f, NativeBridge::getCellWidth)

    fun getCellHeight(): Float = onSession("getCellHeight", 0f, NativeBridge::getCellHeight)

    // ── 渲染 ──

    /**
     * 渲染一帧。>0 表示有输出，0 表示空闲，-1 表示出错。
     *
     * 「无会话」与「会话已销毁」都归为 idle（0）而非错误：两者都是「没有可渲染的会话」，
     * 且不会随首帧重试自愈（重试只对原生返回的 -1 有意义）。首帧重试依赖的 -1 由
     * `NativeBridge.render` 原样返回，不经 [onSession]。
     */
    fun render(): Int = onSession("render", RENDER_IDLE) {
        NativeBridge.render(it, lastSurfaceWidth, lastSurfaceHeight)
    }

    /**
     * 渲染与 new_output 读取合并为单次 JNI 穿越（比两次单独调用每帧省约 0.1-0.3ms）。
     * 返回渲染计数、输出标志、视口光标行（隐藏或在视口外时为 -1）、视口最后一个
     * 有内容的行（视口全空时为 -1）与 surface 失效标志（true = 缓存的原生窗口已被
     * 遗弃，宿主须换新的原生窗口才能恢复渲染）。
     */
    fun renderWithNewOutput(): RenderResult = onSession(
        "renderWithNewOutput",
        RenderResult(RENDER_IDLE, false, CURSOR_ROW_UNKNOWN, LAST_CONTENT_ROW_NONE, false),
    ) {
        val packed = NativeBridge.renderWithNewOutput(it, lastSurfaceWidth, lastSurfaceHeight)
        val count = packed.toInt()
        // 仅屏蔽第 32 位：第 33..53 位承载光标行/内容下沿/surface 失效位，
        // 不得泄漏到输出标志（空闲闭锁依赖该标志）。
        val newOutput = ((packed shr 32) and 0x1L) != 0L
        val cursorRow =
            ((packed shr 33) and CURSOR_ROW_HIDDEN_BITS.toLong()).toInt().let { raw ->
                if (raw == CURSOR_ROW_HIDDEN_BITS) CURSOR_ROW_UNKNOWN else raw
            }
        val lastContentRow =
            ((packed shr 43) and LAST_CONTENT_ROW_NONE_BITS.toLong()).toInt().let { raw ->
                if (raw == LAST_CONTENT_ROW_NONE_BITS) LAST_CONTENT_ROW_NONE else raw
            }
        RenderResult(
            count,
            newOutput,
            cursorRow,
            lastContentRow,
            (packed and SURFACE_INVALIDATED_BIT) != 0L,
        )
    }

    data class RenderResult(
        val count: Int,
        val newOutput: Boolean,
        val cursorRow: Int,
        val lastContentRow: Int,
        val surfaceInvalidated: Boolean,
    )

    /**
     * 读取并清除本会话原生的 `new_output` 标志（滚动复位信号）。
     * 渲染线程每帧调用一次；上次调用以来摄入过 PTY 输出则返回 true。会话未知/已销毁时返回 false。
     */
    fun consumeNewOutput(): Boolean = onSession("consumeNewOutput", false, NativeBridge::consumeNewOutput)

    /** 绑定 Android Surface 供 GPU 渲染。 */
    fun attachSurface(surface: Any, width: Int, height: Int) {
        lastSurfaceWidth = width
        lastSurfaceHeight = height
        onSession("attachWindow", Unit) { NativeBridge.attachWindow(it, surface, width, height) }
    }

    /**
     * 将调用线程挂起 [timeoutMs]，或直到 [terminal.emulator.runtime.SessionEntry.notifyRender] 唤醒（先到为准）。
     * 基于 park 的睡眠既限定渲染循环的轮询节奏（避免 100% CPU 空转），又与调用
     * LockSupport.unpark 的 SessionEntry.notifyRender 配对。
     * 返回值仅供参考：parkNanos 遇中断即返回且不清除中断标志，调用方须自行重查。
     * 未被中断则返回 true。
     */
    fun waitOutput(timeoutMs: Long): Boolean {
        if (timeoutMs <= 0L) return true
        java.util.concurrent.locks.LockSupport.parkNanos(timeoutMs * 1_000_000L)
        return !Thread.currentThread().isInterrupted
    }

    fun releaseGpuSurface() {
        LogUtil.d(TAG, "releaseGpuSurface()")
        onSession("releaseGpuSurface", Unit) { NativeBridge.detachWindow(it) }
    }

    fun setRenderPaused(paused: Boolean) {
        LogUtil.d(TAG, "setRenderPaused($paused)")
        onSession("setRenderPaused", Unit) { NativeBridge.setRenderPaused(it, paused) }
    }

    /**
     * 测试钩子：开启/关闭持续的 surface 级取纹理失败（等价于原生窗口的 BufferQueue
     * 被遗弃，实测形态是每帧都失败）。仅供仪器化用例验证 surface 失效自愈，
     * 无生产调用方。
     */
    fun setSurfaceLossInjectedForTest(injected: Boolean): Boolean =
        onSession("setSurfaceLossInjected", false) { NativeBridge.setSurfaceLossInjected(it, injected) }

    // ── 事件 ──
    data class PollResult(
        val clipboard: String? = null,
        val exit: Boolean = false,
        /** 退出码；`null` 为「原生未能取得」（`waitpid` 失败），不是退出码 0。 */
        val exitCode: Int? = null,
        // 首次退出时由原生测得的子进程存活时长。
        val exitAliveMs: Long = 0,
        val sessionId: Long = 0L,
        val clipboardReads: List<ClipboardRequest> = emptyList(),
        // 本帧见到的全部退出事件，按序。上方单槽字段只描述首个退出；
        // 同帧多余退出必须从该列表回收，否则会泄漏（原生 exit_reported 在推送时置位且不重发）。
        val exits: List<ExitInfo> = emptyList(),
        // BEL 振铃到达（同帧 sticky；提示动作待定行为后另起一步）。
        val bell: Boolean = false,
    ) {
        /** 把后续轮询到的事件并入本结果，标量字段以后者为准。 */
        fun merge(later: PollResult): PollResult = PollResult(
            clipboard = later.clipboard ?: clipboard,
            exit = exit || later.exit,
            // exitCode 与 sessionId 属于同一次（首次）退出。
            exitCode = if (later.exit && !exit) later.exitCode else exitCode,
            // alive_ms 随其退出事件一同传递。
            exitAliveMs = if (later.exit && !exit) later.exitAliveMs else exitAliveMs,
            // sessionId 只用于退出归属：本帧首个退出获胜，后续非退出事件不得覆盖
            // 正在退出会话的 id（否则会误回收仍存活的会话）。
            sessionId = if (later.exit && !exit) later.sessionId else sessionId,
            // 请求事件累加：每个都带不同的 request_id，必须且只能分发一次。
            clipboardReads = clipboardReads + later.clipboardReads,
            exits = exits + later.exits,
            // Bell 与 exit 同为 sticky：本帧一旦置起就保持。
            bell = bell || later.bell,
        )
    }

    data class ExitInfo(
        val sessionId: Long,
        /** 同 [PollResult.exitCode]：`null` 表示码未知，不可当 0 用。 */
        val exitCode: Int?,
        // 原生测得的子进程存活时长（毫秒），仅作诊断负载，不是 Kotlin 事件延迟。
        val exitAliveMs: Long = 0,
    )

    data class ClipboardRequest(val sessionId: Long, val requestId: Long, val selection: String = "")

    fun pollAll(): PollResult {
        // 每帧最多排空 MAX_EVENTS_PER_POLL 个事件，使积压在几帧内消化而非每帧只取一个。
        // 合并结果：同类以靠后的事件为准（exit 为 sticky，死会话的后续事件已陈旧）。
        var result = PollResult()
        // 有界排空：队列见空即 break；计数器具名（下划线形式需实验开关）。
        var pollAttempt = 0
        while (pollAttempt < MAX_EVENTS_PER_POLL) {
            pollAttempt += 1
            val json = NativeBridge.pollEvent() ?: break
            val parsed =
                try {
                    parseEvent(json)
                } catch (exception: Exception) {
                    // 只记异常类名与 JSON 长度，不记 `exception.message`（解析器会把出错的
                    // 片段带进 message，而事件 JSON 可能含剪贴板文本或 URL——见 PollEvent
                    // 的 `exceptionsWithDebugInfo = false` 脱敏约定）与 JSON 本身。
                    // 长度足以区分「空/截断」与「结构不符」，且不泄露内容。
                    LogUtil.w(
                        TAG,
                        "pollAll: undecodable event dropped (${exception.javaClass.simpleName}, ${json.length} chars)",
                    )
                    continue
                }
            result = result.merge(parsed)
            // 遇 exit 不 break：其后排队的事件否则会滞留在原生队列中——没有其他会话会排空它们。
            // exit 在 merge 中是 sticky，继续排空无害。
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

    // ── 主题 / 外观 ──
    // 端到端接线：setTheme 打包 54 字节（背景 3 + 前景 3 + ansi 48）交给原生调色板；
    // OSC 10/11/4 颜色处理位于终端引擎内部并经调色板 API 应用。光标颜色走独立的
    // setCursorColor 通道，以保持 54 字节布局稳定（ffi.rs 校验精确长度）。
    fun setTheme(theme: BridgeTheme) {
        LogUtil.d(TAG, "setTheme: ${theme.name}")
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
        LogUtil.d(TAG, "setSystemLocale($locale)")
        onSession("setSystemLocale", Unit) { NativeBridge.setSystemLocale(it, locale) }
    }

    private var lastExtraFontPaths: List<String> = emptyList()

    fun setExtraFontPaths(paths: List<String>) {
        LogUtil.d(TAG, "setExtraFontPaths($paths)")
        // 原生调用会重建字体管线（重新分配图集），因此跳过重复调用，目录内容在重建时自行拾取。
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
        LogUtil.d(TAG, "setFontFamily($family)")
        // DESIGN 字体选择节：font.ttf 存在即默认，不复制文件，直接应用覆盖存入设置。
        val override = probeDefaultFontFile()
        return onSession("setFontFamily", false) { NativeBridge.setFontFamily(it, override ?: family) }
    }

    fun setFontSizeInPlace(sizeTenths: Int) {
        LogUtil.d(TAG, "setFontSizeInPlace($sizeTenths)")
        onSession("setFontSizeInPlace", Unit) { NativeBridge.setFontSizeInPlace(it, sizeTenths) }
    }

    fun setRasterScale(scale: Float) {
        onSession("setRasterScale", Unit) { NativeBridge.setRasterScale(it, scale) }
    }

    // 自定义字体由原生 fontdb 探测文件、注册到渲染器并返回家族名，失败返回 null。
    fun loadFontFile(path: String): String? {
        LogUtil.d(TAG, "loadFontFile($path)")
        return onSession("loadFontFile", null) { NativeBridge.loadFontFile(it, path) }
    }

    // ── 输入 ──
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
     * 用 Ghostty 鼠标编码器编码鼠标事件并把转义序列写入 PTY。
     * 生成并写入序列时返回 true；关闭鼠标上报、编码失败或会话消失（事件丢弃）时返回 false。
     */
    fun encodeMouseEvent(
        xPx: Float,
        yPx: Float,
        action: Int,
        button: Int,
        cellWidth: Float,
        cellHeight: Float,
    ): Boolean {
        val bytes =
            onSession("encodeMouseEvent", ByteArray(0)) {
                NativeBridge.encodeMouseEvent(it, xPx, yPx, action, button, cellWidth, cellHeight)
            }
        if (bytes.isEmpty()) return false
        return writeToPty(bytes)
    }

    /**
     * 远端是否处于备用屏幕缓冲（vim/less/htop）。无锁，可在每次触摸滚动时安全调用。
     * 为真时触摸滚动必须以滚轮转义转发给远端（见 [terminal.emulator.ui.TerminalSurface] onScroll）而非滚动本地回滚。
     */
    fun isAltScreenActive(): Boolean = onSession("getAltScreenState", false, NativeBridge::getAltScreenState)

    /**
     * 终端是否处于应用光标模式（DECCKM，DEC 私有模式 1）。此时方向键须编码为 SS3（`ESC OA`）
     * 而非 CSI（`ESC [ A`），见 docs/specification/REFERENCE.md。仅在方向键事件时查询。
     */
    fun isAppCursorMode(): Boolean =
        onSession("getMode", false) { NativeBridge.getMode(it, DEC_PRIVATE_MODE_APP_CURSOR, 0) }

    fun processKeyEvent(keyCode: Int, modifiers: Byte, action: Int, unicodeChar: Int, unshiftedChar: Int): Boolean {
        LogUtil.d(TAG, "processKeyEvent($keyCode, $modifiers, $action)")
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
            // 部分输入法（Gboard 在 InputType.TYPE_NULL 下、组合输入中）发出的按键事件
            // unicodeChar == 0，尽管该键是可打印字符。回退到虚拟键盘的按键字符映射表
            // 推导字符，保证这类击键仍能到达 PTY：范围必须覆盖**整个**按键码域而非只有
            // A..Z，否则空格、数字与标点（输入法同样报告 unicodeChar=0）会被整键丢弃。
            if (!ctrlActive) {
                val derived =
                    android.view.KeyCharacterMap.load(android.view.KeyCharacterMap.VIRTUAL_KEYBOARD)
                        .get(keyCode, 0)
                if (derived > 0 && Character.isValidCodePoint(derived)) {
                    NativeBridge.writeKey(id, String(Character.toChars(derived)), modifierBits, null)
                    return@onSession true
                }
            }
            false
        }
    }

    fun focusEvent(focused: Boolean) {
        onSession("focusEvent", Unit) { NativeBridge.focusEvent(it, focused) }
    }

    // ── 终端查询（委托给 TerminalQueryPort 接缝） ──
    override fun getTitle(): String? = runCatchingCancellable { queryPort.getTitle() }.getOrNull()

    override fun getActiveSessionTitle(): String =
        runCatchingCancellable { queryPort.getActiveSessionTitle() }.getOrDefault("")

    // ── 选区 ──
    override fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean?) {
        queryPort.setSelection(startRow, startCol, endRow, endCol, hasSelection)
    }

    // ── 搜索 / 回滚 ──
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

    override fun listFontFamilies(): List<String>? = runCatchingCancellable { queryPort.listFontFamilies() }.getOrNull()

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

    override fun getDefaultFontName(): String = runCatchingCancellable { queryPort.getDefaultFontName() }.getOrDefault(
        "",
    )

    override fun getFontInfo(): String? = runCatchingCancellable { queryPort.getFontInfo() }.getOrNull()

    companion object {
        private const val TAG = "Bridge"

        /** renderWithNewOutput 打包：光标行占 bit 33..42，该值表示隐藏或在视口外。 */
        const val CURSOR_ROW_HIDDEN_BITS = 0x3FF

        /** renderWithNewOutput 打包：内容下沿行占 bit 43..52，该值表示视口全空。 */
        const val LAST_CONTENT_ROW_NONE_BITS = 0x3FF

        /** renderWithNewOutput 打包：bit 53 = 原生 surface 已判死，须换新的原生窗口。 */
        const val SURFACE_INVALIDATED_BIT = 1L shl 53

        /** 视口全空（或无会话）时解码出的内容下沿行。 */
        const val LAST_CONTENT_ROW_NONE = -1

        /** render 缺省返回值：无可渲染的会话（未建立或已销毁），按 idle 处理。 */
        private const val RENDER_IDLE = 0

        /** 隐藏/在视口外（或无会话）时解码出的光标行。 */
        const val CURSOR_ROW_UNKNOWN = -1

        /** pollAll() 每帧最多排空的事件数，限定渲染线程开销。 */
        private const val MAX_EVENTS_PER_POLL = 32

        /** setTheme 打包长度：背景 3 + 前景 3 + 16 色 × 3 = 54 字节，ffi.rs 校验精确长度。 */
        private const val THEME_PACKED_BYTES = 54

        /** DEC 私有模式 1 = 应用光标键（DECCKM）。 */
        private const val DEC_PRIVATE_MODE_APP_CURSOR = 1

        /** 编码结果依赖 DECCKM 的按键码。 */
        private val APP_CURSOR_KEY_CODES =
            setOf(
                android.view.KeyEvent.KEYCODE_DPAD_UP,
                android.view.KeyEvent.KEYCODE_DPAD_DOWN,
                android.view.KeyEvent.KEYCODE_DPAD_RIGHT,
                android.view.KeyEvent.KEYCODE_DPAD_LEFT,
            )
    }
}
