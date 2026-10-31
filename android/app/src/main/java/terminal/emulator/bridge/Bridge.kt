package terminal.emulator.bridge

import kotlinx.coroutines.launch
import terminal.emulator.input.KeyModifiers
import terminal.emulator.runtime.LogUtil
import terminal.emulator.util.runCatchingCancellable
import java.util.concurrent.atomic.AtomicReference

/** 终端会话的 shell 配置。 */
sealed interface Shell {
    /** 使用系统默认 shell（/system/bin/sh）。 */
    data object SystemDefault : Shell

    /** 使用指定路径的自定义 shell。 */
    data class Custom(val path: String) : Shell
}

/** 构造 [Bridge] 时传入的配置。 */
data class TerminalConfig(
    val shell: Shell,
    /** 终端主题标识；调色板真源在原生，桥接层与渲染层都不另存颜色副本。 */
    val themeName: String,
    val home: String,
    val workingDirectory: String,
    val prefix: String,
    val mkshrcPath: String,
    val fontSizeTenths: Int,
    val scrollbackLines: Int = 2000,
)

/**
 * 包裹 [NativeBridge] 静态 JNI 导出的实例桥接。每个 Bridge 持有会话 ID 并管理生命周期，
 * 使调用方不直接接触会话 ID；所有按会话的 JNI 调用都经 [onSession]，集中处理「拿不到结果」的情况。
 * 函数数量对应 JNI 表面而非接口异味。
 */
// 对 PollEvent 密封类做 when 分派，每个变体一个分支。
class Bridge(private val config: TerminalConfig) {
    /** 原生查询路径：所有查询委托给 [NativeQueryPort]，它与 ffi.rs 的 JNI 查询导出 1:1 对应。 */
    private val queryPort = NativeQueryPort { sessionId }

    @Volatile private var sessionId: Long = 0L

    @Volatile private var lastSurfaceWidth: Int = 0

    @Volatile private var lastSurfaceHeight: Int = 0

    /**
     * 每次 PTY 写入的钩子：在所有 PTY 写路径（[Bridge.writeToPty]、[processKeyEvent]、
     * [sendFallbackKey]、[encodeMouseEvent]）上调用，使绕过 TerminalRuntime.writeToPty
     * 的硬件按键也能唤醒渲染。
     * 同时是渲染唤醒接缝：空闲 >5s 后的退格不再需要等满 500ms 空闲闭锁才能看到回显。
     */
    @Volatile var onPtyWrite: (() -> Unit)? = null

    /**
     * 把 PTY 写入交给宿主排队的钩子；为 null 时直写。
     *
     * 原生 `feedPty` 在主端满时最多等 `WRITE_DRAIN_TIMEOUT`（5s），而击键与鼠标
     * 事件从 UI 线程直连——子进程不读 stdin 时每次按键冻住主线程。宿主把本钩子接到
     * 按帧合并的批缓冲上，写入即落到单线程发送器：UI 线程不阻塞，且与输入法写入
     * 共用同一条有序出口（此前两路各写，缓冲中待发的退格会被直写的击键越过）。
     *
     * [writeToPty] 这条**不走**本钩子——它本身就是批缓冲的 sink，排队会自调用。
     * 由 [terminal.emulator.ui.TerminalSurface] 在 attach/detach 之间设置。
     */
    @Volatile var ptyWriteQueue: ((Long, ByteArray) -> Unit)? = null

    /** 交给宿主队列（若有），否则直写。调用点无须再判，语义一致。 */
    private fun sendToPty(sessionId: Long, data: ByteArray) {
        val queue = ptyWriteQueue
        if (queue != null) queue(sessionId, data) else NativeBridge.feedPty(sessionId, data)
    }

    /**
     * 编码器未覆盖的键码走这条出口：编码在原生，写入与其余按键同一条队列。
     *
     * 原实现在原生侧直接同步写 PTY，主端满时最多等 5s——调用点在 UI 线程上，
     * 子进程不读 stdin 时每次这类击键冻住主线程，正是 [ptyWriteQueue] 要避免的事；
     * 它还绕过队列与 [onPtyWrite]，回显要多等一个空闲闭锁才出现。
     */
    private fun sendFallbackKey(sessionId: Long, text: String, metaPrefix: Boolean) {
        val encoded = NativeBridge.encodeKeyText(text, metaPrefix)
        sendToPty(sessionId, encoded)
        onPtyWrite?.invoke()
    }

    /**
     * 统一转发一次 JNI 调用。
     *
     * 两种「拿不到结果」的状态共用 [onUnavailable]：会话尚未建立（id 为 0），
     * 以及检查之后、调用之前会话被销毁（native 对未知会话抛
     * [IllegalStateException]，见 DESIGN 的 Activity 重建/进程回收场景）。
     * 两者都不是崩溃理由，但都记警告而非静默丢弃。
     *
     * 只捕获 [IllegalStateException] 这一种：其余异常（PTY 写入失败、主题不存在、
     * 尺寸越界、native panic）必须照实上抛。此前这里捕的是整个 `RuntimeException`，
     * 于是这些真实故障被一并归因为「会话已销毁」而无声消失——用户选了个损坏的
     * 主题名，界面毫无反应，日志还写着一条错误的归因。
     */
    private inline fun <T> onSession(name: String, onUnavailable: T, call: (Long) -> T): T {
        val capturedSessionId = sessionId
        if (capturedSessionId == 0L) return onUnavailable
        return try {
            call(capturedSessionId)
        } catch (sessionMissing: IllegalStateException) {
            LogUtil.warn(TAG, "$name: 会话 $capturedSessionId 已销毁，返回缺省值")
            onUnavailable
        }
    }

    /**
     * 查询委托的失败记警告后回缺省：与 [onSession] 同理，会话竞态不是崩溃理由，
     * 但真实异常必须可诊断——`getOrNull/getOrDefault` 静默吞掉后 VT 线程死亡
     * 类问题在日志里无迹可寻。取消信号由 [runCatchingCancellable] 重抛，不记日志。
     */
    private inline fun <T> onQuery(name: String, onUnavailable: T, call: () -> T): T = runCatchingCancellable(call)
        .onFailure {
            LogUtil.warn(TAG, "$name failed, returning default (${it.javaClass.simpleName})")
        }.getOrDefault(onUnavailable)

    fun ping(): String {
        if (!NativeBridge.isNativeLoaded()) throw RuntimeException("native library not loaded")
        val sessions = runCatchingCancellable { NativeBridge.listSessions() }.getOrDefault(LongArray(0))
        return "native library OK, sessions=${sessions.size}"
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
                config.scrollbackLines,
            )
        return sessionId
    }

    /**
     * 后台预热渲染器与字体库：PTY 已 spawn，故它不推迟 shell 的启动。
     *
     * 正常路径上初始化早在 spawn 前算网格时就已完成（`getCellSizePacked` → 原生
     * `render_state_mut`），这里通常是幂等空转；它真正保住的是「GPU 初始化失败不致命」：
     * 那次度量查询赶上 GPU 暂不可用会返回 0、由 Kotlin 退回默认网格，此处再试一次。
     *
     * 下方 `catch` 只接得住 JNI 层抛回的 `RuntimeException`；字体库无任何可用面时
     * `font_db::fatal` 走 `process::abort`，是进程级终止，此处接不到。
     */
    fun prefetchRenderStateAsync(scope: kotlinx.coroutines.CoroutineScope) {
        scope.launch(terminal.emulator.util.TerminalDispatchers.inputOutput) {
            try {
                NativeBridge.prefetchRenderState()
            } catch (exception: Exception) {
                LogUtil.warn("Bridge", "prefetchRenderState failed", exception)
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
                LogUtil.error(TAG, "close: destroySession failed", exception)
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

    /** 单元格宽高（字号单位）。宽高恒定同批消费，故一次查询一并取回。 */
    fun cellSize(): Pair<Float, Float> = unpackCellSize(NativeBridge.getCellSizePacked())

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
     * 返回渲染计数、输出标志、视口最后一个有内容的行（视口全空时为 -1）、
     * surface 失效标志（true = 缓存的原生窗口已被遗弃，宿主须换新的原生窗口才能恢复
     * 渲染）与备用屏激活标志。
     *
     * 备用屏随每帧上报而非另发阻塞查询：输入法弹出动画期间它会翻转（用户启动
     * helix 的同时键盘正收起），查询得到的缓存必然滞后于该帧的位移计算。
     */
    fun renderWithNewOutput(): RenderResult = onSession(
        "renderWithNewOutput",
        RenderResult(
            RENDER_IDLE,
            false,
            LAST_CONTENT_ROW_NONE,
            false,
            false,
        ),
    ) {
        decodeRenderResult(
            NativeBridge.renderWithNewOutput(it, lastSurfaceWidth, lastSurfaceHeight),
        )
    }

    data class RenderResult(
        val count: Int,
        val newOutput: Boolean,
        val lastContentRow: Int,
        val surfaceInvalidated: Boolean,
        /** 备用屏（helix/vim 等全屏 TUI）激活：输入法跟随位移须为 0，否则应用顶部被推出屏幕。 */
        val altScreenActive: Boolean,
    )

    /** 绑定 Android Surface 供 GPU 渲染。 */
    fun attachSurface(surface: Any, width: Int, height: Int) {
        lastSurfaceWidth = width
        lastSurfaceHeight = height
        onSession("attachWindow", Unit) { NativeBridge.attachWindow(it, surface, width, height) }
    }

    /**
     * 将调用线程挂起最多 [timeoutMs] 毫秒，或直到有新输出 / 宿主信号（先到为准），
     * 返回新的唤醒代数。
     *
     * 等待在原生条件变量上完成：PTY 读线程与 [wakeRender] 都是同一把通知，
     * 因此既保留「空闲时零唤醒」的省电语义，又让**没有输入**的后台输出立即上屏，
     * 不必等满空闲闭锁。调用方把返回值存回 `SessionEntry.wakeGeneration`，
     * 下次等待若期间已有通知则立即返回——不丢唤醒。
     */
    fun waitOutput(lastGeneration: Long, timeoutMs: Long): Long = onSession("waitOutput", lastGeneration) {
        NativeBridge.waitForOutput(it, lastGeneration, timeoutMs)
    }

    /**
     * 唤醒本会话的渲染循环：把原生唤醒代数 +1。
     *
     * 会话已销毁时静默忽略（[onSession] 契约）——渲染循环的存亡由渲染线程自身的
     * `running` 标志裁决，唤醒失败不构成错误。
     */
    fun wakeRender() {
        onSession("wakeRender", Unit) { NativeBridge.wakeRender(it) }
    }

    fun releaseGpuSurface() {
        LogUtil.debug(TAG, "releaseGpuSurface()")
        onSession("releaseGpuSurface", Unit) { NativeBridge.detachWindow(it) }
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
        val sessionId: Long = 0L,
        val clipboardReads: List<ClipboardRequest> = emptyList(),
        // 本帧见到的全部退出事件，按序。上方单槽字段只描述首个退出；
        // 同帧多余退出必须从该列表回收，否则会泄漏（原生 exit_reported 在推送时置位且不重发）。
        val exits: List<ExitInfo> = emptyList(),
    ) {
        /** 把后续轮询到的事件并入本结果，标量字段以后者为准。 */
        fun merge(later: PollResult): PollResult = PollResult(
            clipboard = later.clipboard ?: clipboard,
            exit = exit || later.exit,
            // exitCode 与 sessionId 属于同一次（首次）退出。
            exitCode = if (later.exit && !exit) later.exitCode else exitCode,
            // sessionId 只用于退出归属：本帧首个退出获胜，后续非退出事件不得覆盖
            // 正在退出会话的 id（否则会误回收仍存活的会话）。
            sessionId = if (later.exit && !exit) later.sessionId else sessionId,
            // 请求事件累加：每个都带不同的 request_id，必须且只能分发一次。
            clipboardReads = clipboardReads + later.clipboardReads,
            exits = exits + later.exits,
        )
    }

    data class ExitInfo(
        val sessionId: Long,
        /** 同 [PollResult.exitCode]：`null` 表示码未知，不可当 0 用。 */
        val exitCode: Int?,
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
                    LogUtil.warn(
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
        val event =
            pollEventJson.decodeFromString<PollEvent>(
                json,
            )
    ) {
        is PollEvent.Clipboard -> {
            PollResult(clipboard = event.text.ifEmpty { null }, sessionId = event.sessionId)
        }

        is PollEvent.Exit -> {
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
        }

        is PollEvent.ClipboardRead -> {
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
        }
    }

    // ── 主题 / 外观 ──
    // 端到端接线：只下发主题标识，终端默认色、ANSI 调色板、光标色与搜索高亮色都由
    // 原生按同一份构建期嵌入的 alacritty 主题解析，故两端不可能各自持一份颜色。
    fun setTheme(themeName: String) {
        LogUtil.debug(TAG, "setTheme: $themeName")
        onSession("setTheme", Unit) { NativeBridge.setTheme(it, themeName) }
    }

    /** 默认光标样式（`terminal-cursor-style`）；程序用 DECSCUSR 覆盖时以程序为准。 */
    fun setCursorStyle(nativeValue: Int) {
        onSession("setCursorStyle", Unit) { NativeBridge.setCursorStyle(it, nativeValue) }
    }

    fun setSystemLocale(locale: String) {
        LogUtil.debug(TAG, "setSystemLocale($locale)")
        onSession("setSystemLocale", Unit) { NativeBridge.setSystemLocale(it, locale) }
    }

    // 字体设置的应用会从 IO 协程与设置流程并发到达，去重状态用原子引用整体替换：
    // 普通可变字段的读-改-写会竞态，最坏结果是去重失效 → 原生重建字体管线
    // 并重新分配图集。
    private val lastExtraFontPaths = AtomicReference<List<String>>(emptyList())

    fun setExtraFontPaths(paths: List<String>) {
        LogUtil.debug(TAG, "setExtraFontPaths($paths)")
        // 原生调用会重建字体管线（重新分配图集），因此跳过重复调用，目录内容在重建时自行拾取。
        if (lastExtraFontPaths.getAndSet(paths) == paths) return
        onSession("setExtraFontPaths", Unit) { NativeBridge.setExtraFontPaths(it, paths.toTypedArray()) }
    }

    // font.ttf 覆盖探测缓存：native 每次加载都会追加字体条目，
    // 同一文件只探测一次，内容变化才重新探测。
    // 键与族名成对出现，用一个原子引用整体替换：拆成两个可变字段会出现
    // 「键已更新、族名还是旧的」这种撕裂状态。
    private val defaultFontProbe = AtomicReference<Pair<String?, String?>?>(null)

    private fun probeDefaultFontFile(): String? {
        val file =
            terminal.emulator.termuxDefaultFontFile(config.home) ?: run {
                defaultFontProbe.set(null)
                return null
            }
        val key = "${file.absolutePath}:${file.lastModified()}:${file.length()}"
        val cached = defaultFontProbe.get()
        if (cached != null && cached.first == key) return cached.second
        val family = loadFontFile(file.absolutePath)
        defaultFontProbe.set(key to family)
        return family
    }

    /**
     * 设置字族。
     *
     * 字族是**全局**的：原生侧只落在唯一的字体管线上，与会话无关
     * （`NativeBridge.setFontFamily` 的 session_id 未被使用）。故不经会话闸：
     * 会话建立之前也必须能下发——PTY 的初始 winsize 由字体度量算出，字族若在
     * spawn 之后才落定，attach 时的一次 resize 会给 shell 发 SIGWINCH，
     * shell 重排已打印的 prompt（首屏多出一行提示符）。
     *
     * null = 原生库未加载，无从验证（调用方不得清除设置）；
     * false = 原生明确未找到该字族。
     */
    fun setFontFamily(family: String): Boolean? {
        LogUtil.debug(TAG, "setFontFamily($family)")
        if (!NativeBridge.isNativeLoaded()) return null
        // DESIGN 字体选择节：font.ttf 存在即默认，不复制文件，直接应用覆盖存入设置。
        val override = probeDefaultFontFile()
        return NativeBridge.setFontFamily(0L, override ?: family)
    }

    /** 字号：同样是全局量（字体管线的唯一一份），会话建立之前也须能下发。 */
    fun setFontSizeInPlace(sizeTenths: Int) {
        LogUtil.debug(TAG, "setFontSizeInPlace($sizeTenths)")
        if (!NativeBridge.isNativeLoaded()) return
        NativeBridge.setFontSizeInPlace(0L, sizeTenths)
    }

    /** 光栅尺度（sp→px 系数）：全局量，与会话无关。 */
    fun setRasterScale(scale: Float) {
        if (!NativeBridge.isNativeLoaded()) return
        NativeBridge.setRasterScale(0L, scale)
    }

    /**
     * 自定义字体由原生 fontdb 探测文件、注册到渲染器并返回家族名，失败返回 null。
     *
     * 原生侧是全局操作（把文件装进唯一的字体库并应用），与会话无关：会话建立
     * 之前也必须可用——`font.ttf` 覆盖的探测发生在 spawn 之前。
     */
    fun loadFontFile(path: String): String? {
        LogUtil.debug(TAG, "loadFontFile($path)")
        if (!NativeBridge.isNativeLoaded()) return null
        return NativeBridge.loadFontFile(0L, path)
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
        onPtyWrite?.invoke()
        true
    }

    /**
     * 用 Ghostty 鼠标编码器编码鼠标事件并把转义序列写入 PTY。
     * 生成并写入序列时返回 true；关闭鼠标上报、编码失败或会话消失（事件丢弃）时返回 false。
     */
    fun encodeMouseEvent(
        pixelX: Float,
        pixelY: Float,
        action: Int,
        button: Int,
        modifiers: Int,
        cellWidth: Float,
        cellHeight: Float,
    ): Boolean {
        // 编码与写入在**同一次** `onSession` 里：两次 `onSession` 各读一遍 `sessionId`，
        // 两次之间会话被销毁并重建时，按旧会话坐标算出的转义序列会写进新会话的 PTY。
        // 与 `processKeyEvent` 同款写法。
        return onSession("encodeMouseEvent", false) { sessionId ->
            val bytes =
                NativeBridge.encodeMouseEvent(
                    sessionId,
                    pixelX,
                    pixelY,
                    action,
                    button,
                    modifiers,
                    cellWidth,
                    cellHeight,
                )
            if (bytes.isEmpty()) return@onSession false
            sendToPty(sessionId, bytes)
            onPtyWrite?.invoke()
            true
        }
    }

    /**
     * 把 `repeat` 档滚轮一次编码并写入 PTY（每档一个 press + release）。
     *
     * 触摸拖动一个事件可能滚过数行：逐档调用 [encodeMouseEvent] 会让 UI 线程按在
     * 每档一次 VT 应答上，PTY 也要被写满 2×repeat 次。合并后一次事件只有一次往返。
     * 返回是否真的写入了（上报关闭、编码失败或会话消失时为 false）。
     */
    fun encodeMouseWheel(
        pixelX: Float,
        pixelY: Float,
        button: Int,
        modifiers: Int,
        cellWidth: Float,
        cellHeight: Float,
        repeat: Int,
    ): Boolean {
        if (repeat <= 0) return false
        return onSession("encodeMouseWheel", false) { sessionId ->
            val bytes =
                NativeBridge.encodeMouseWheel(
                    sessionId,
                    pixelX,
                    pixelY,
                    button,
                    modifiers,
                    cellWidth,
                    cellHeight,
                    repeat,
                )
            if (bytes.isEmpty()) return@onSession false
            sendToPty(sessionId, bytes)
            onPtyWrite?.invoke()
            true
        }
    }

    /**
     * 远端是否启用鼠标上报（DECSET 1000/1002/1003）。
     *
     * 与 [isAltScreenActive] 同理读镜像而非查询通道：触摸拖动的每个事件都要判一次，
     * 查询会把 UI 线程按满一个查询超时，正是「滚动卡顿」的来源。
     */
    fun isMouseTrackingActive(): Boolean = onSession("mouseTrackingActive", false, NativeBridge::mouseTrackingActive)

    /**
     * 远端是否处于备用屏幕缓冲（vim/less/htop）。无锁，可在每次触摸滚动时安全调用。
     * 为真时触摸滚动必须以滚轮转义转发给远端（见 [terminal.emulator.ui.TerminalSurface] onScroll）而非滚动本地回滚。
     */
    fun isAltScreenActive(): Boolean = onSession("getAltScreenState", false, NativeBridge::getAltScreenState)

    fun processKeyEvent(keyCode: Int, modifiers: Int, action: Int, unicodeChar: Int, unshiftedChar: Int): Boolean {
        LogUtil.debug(TAG, "processKeyEvent($keyCode, $modifiers, $action)")
        // 仅 ACTION_DOWN 产生输出：onKeyDown 与 onKeyUp 都走这里，若在 UP 也写入
        // 会把每次击键写两遍（"llss"、双击 Enter、双击 Ctrl+C）。ACTION_UP 返回
        // false，交由平台默认实现（空操作）处理。
        if (action != android.view.KeyEvent.ACTION_DOWN) return false
        // 修饰位取上游 `key.Mods` 原始位（1=shift，2=ctrl，4=alt，8=super），
        // 由编码器直接消费，应用不再自行解读。
        val ctrlActive = modifiers and KeyModifiers.CTRL != 0
        // Alt/Super 在终端语义上是「加 ESC 前缀」，而本入口只处理编码器未覆盖的
        // 键码，故把这条规则折成一个布尔下发：掩码位值不必在两端各写一份。
        val metaPrefix = modifiers and (KeyModifiers.ALT or KeyModifiers.SUPER) != 0
        return onSession("processKeyEvent", false) { sessionId ->
            // 编码器按终端状态选择形态：DECCKM 下方向键走 SS3、modifyOtherKeys
            // 下 Ctrl+字母走 `CSI 27;mod;code~`、backarrow key mode 下退格发 BS
            // 而非 DEL。应用不再查询任何模式，也不再维护转义序列表。
            // 直接发送键名（keyCodeToName："Up"、"Home"…）会把这些字面文本写进
            // PTY —— 原生不解析键名，vim/less 的方向键、Home/End、PageUp/Down、
            // Delete 都会失效。
            val encoded =
                NativeBridge.encodeKeyEvent(
                    sessionId = sessionId,
                    keyCode = keyCode,
                    unicodeChar = unicodeChar,
                    unshiftedChar = unshiftedChar,
                    modifiers = modifiers,
                    action = action,
                )
            if (encoded.isNotEmpty()) {
                // 排队写：几乎全部按键走这一支，直写会让子进程不读 stdin 时冻住主线程。
                sendToPty(sessionId, encoded)
                onPtyWrite?.invoke()
                return@onSession true
            }
            // 编码器未覆盖的键码（上游没有对应的 Key，如 `KEYCODE_AT` 的 `@`
            // 实为 Shift+2）或该键不产生字节的（单独修饰键），回退到平台给出的
            // 码点直写。按 Ctrl 时绝不走此路：折叠在回退入口按 c & 0x1F 进行，
            // 会把 Ctrl+9/Ctrl+0 变成 Ctrl+Y/Ctrl+P。Ctrl+可打印键已由编码器
            // 编码或按上游规则丢弃。
            if (!ctrlActive && unicodeChar > 0 && unicodeChar != 0x7F) {
                if (!Character.isValidCodePoint(unicodeChar)) return@onSession false
                sendFallbackKey(sessionId, String(Character.toChars(unicodeChar)), metaPrefix)
                return@onSession true
            }
            // 部分输入法（Gboard 在 InputType.TYPE_NULL 下、组合输入中）发出的按键事件
            // unicodeChar == 0，尽管该键是可打印字符。回退到虚拟键盘的按键字符映射表
            // 推导字符，保证这类击键仍能到达 PTY：范围必须覆盖**整个**按键码域而非只有
            // A..Z，否则空格、数字与标点（输入法同样报告 unicodeChar=0）会被整键丢弃。
            if (!ctrlActive) {
                val derived = virtualKeyboardMap.get(keyCode, 0)
                if (derived > 0 && Character.isValidCodePoint(derived)) {
                    sendFallbackKey(sessionId, String(Character.toChars(derived)), metaPrefix)
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
    fun getTitle(): String? = onQuery("getTitle", null) { queryPort.getTitle() }

    /** 工作目录（OSC 7）；空串表示 shell 尚未上报。 */
    fun getPwd(): String = onQuery("getPwd", "") { queryPort.getPwd() }

    fun getActiveSessionTitle(): String = onQuery("getActiveSessionTitle", "") { queryPort.getActiveSessionTitle() }

    // ── 选区 ──

    /** 返回 false 表示写入被推迟（会话锁被 PTY 写入持有）：调用方保留旧快照并下一帧重试。 */
    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean?): Boolean =
        queryPort.setSelection(startRow, startCol, endRow, endCol, hasSelection)

    // ── 搜索 / 回滚 ──
    // 取值类查询经 NativeQueryPort 转发到真实的原生 JNI 路径，并经 onQuery 把
    // 会话竞态异常转为缺省值，使 UI/触摸路径不崩。下方五个写入类覆写不走 onQuery：
    // 它们返回 Unit，吞掉异常等于让调用方以为写入成功——错误必须出声。
    fun clearSearchHighlights() = queryPort.clearSearchHighlights()

    fun applySearchHighlights(scrollOffset: Int) = queryPort.applySearchHighlights(scrollOffset)

    fun scrollbackLine(row: Int): String? = onQuery("scrollbackLine", null) { queryPort.scrollbackLine(row) }

    fun scrollbackLength(): Int = onQuery("scrollbackLength", 0) { queryPort.scrollbackLength() }

    /**
     * 会话**已应用**的滚动偏移（0 = 视口贴底）。
     *
     * 与 [terminal.emulator.runtime.TerminalRuntime.activeSessionScrollOffset] 的「待消费命令槽」不同：只有 VT 侧
     * 接受了本次滚动后这个值才变化，故可作为「归位已完成」的稳定判据（仪器测试用）。
     */
    fun sessionScrollOffset(): Int = onSession("sessionScrollOffset", 0) {
        NativeBridge.getSessionScrollOffset(it)
    }

    /**
     * 最近一帧的回滚长度；首帧未发布时为 -1。**绝不同步询问 VT 线程**，
     * 故可在每一帧的滚动/拖柄热路径调用。
     */
    fun renderedScrollbackLength(): Int = onQuery("renderedScrollbackLength", -1) {
        queryPort.renderedScrollbackLength()
    }

    fun cursorViewportPacked(): Long = onQuery("cursorViewportPacked", -1L) {
        queryPort.cursorViewportPacked()
    }

    /** 最近一帧渲染取走的光标（同 [cursorViewportPacked] 的打包）；一次原子读，不等待 VT。 */
    fun renderedCursorPacked(): Long = onQuery("renderedCursorPacked", -1L) {
        queryPort.renderedCursorPacked()
    }

    fun searchSetNeedle(needle: String): Int = onQuery("searchSetNeedle", 0) { queryPort.searchSetNeedle(needle) }

    fun searchSelect(forward: Boolean): SearchSelection? = onQuery("searchSelect", null) {
        queryPort.searchSelect(forward)
    }

    fun setScrollOffset(offset: Int): Boolean = queryPort.setScrollOffset(offset)

    fun setScrollYPx(offsetPx: Float) = queryPort.setScrollYPx(offsetPx)

    fun getTerminalText(): String? = onQuery("getTerminalText", null) { queryPort.getTerminalText() }

    /** 可见视口文本（不含回滚）。判断「某行是否看得见」用它，别用全量文本。 */
    fun getVisibleText(): String = onQuery("getVisibleText", "") { queryPort.visibleText() }

    fun selectionText(startRow: Int, startCol: Int, endRow: Int, endCol: Int): String? =
        onQuery("selectionText", null) {
            queryPort.selectionText(startRow, startCol, endRow, endCol)
        }

    fun listFontFamilies(): List<String>? = onQuery("listFontFamilies", null) { queryPort.listFontFamilies() }

    fun hyperlinkAt(row: Int, col: Int): String? = onQuery("hyperlinkAt", null) {
        queryPort.hyperlinkAt(
            row,
            col,
        )
    }

    // 上游选择派生：native 侧已安装选区并回传界限；unknown session 异常
    // 与其余查询方法一致转为缺省值（UI/触摸路径不崩）。
    fun selectWordAt(row: Int, col: Int): IntArray? = onQuery("selectWordAt", null) {
        queryPort.selectWordAt(row, col)
    }

    fun selectAll(): IntArray? = onQuery("selectAll", null) { queryPort.selectAll() }

    fun wideCharTailCols(row: Int): IntArray = onQuery("wideCharTailCols", IntArray(0)) {
        queryPort.wideCharTailCols(row)
    }

    fun getDefaultFontName(): String = onQuery("getDefaultFontName", "") { queryPort.getDefaultFontName() }

    fun getFontInfo(): FontInfo? = onQuery("getFontInfo", null) { queryPort.getFontInfo()?.let(FontInfo::fromPacked) }

    companion object {
        private const val TAG = "Bridge"

        /**
         * 虚拟键盘的按键字符映射表：进程内取一次。
         *
         * `KeyCharacterMap.load` 每次都新建对象并走一次 `nativeLoad`，平台不缓存——
         * 原先在每次击键的回退路径上调，等于每次按键都重新加载整张映射表。
         */
        private val virtualKeyboardMap: android.view.KeyCharacterMap by lazy {
            android.view.KeyCharacterMap.load(android.view.KeyCharacterMap.VIRTUAL_KEYBOARD)
        }

        /** renderWithNewOutput 打包：内容下沿行占 bit 43..52，该值表示视口全空。 */
        const val LAST_CONTENT_ROW_NONE_BITS = 0x3FF

        /** renderWithNewOutput 打包：bit 53 = 原生 surface 已判死，须换新的原生窗口。 */
        const val SURFACE_INVALIDATED_BIT = 1L shl 53

        /** renderWithNewOutput 打包：bit 54 = 备用屏（helix/vim 等全屏 TUI）激活。 */
        const val ALT_SCREEN_ACTIVE_BIT = 1L shl 54

        /** getCellSizePacked 打包：低 32 位是单元格宽度的 `f32` 位。 */
        internal const val LOW_32_BITS = 0xFFFFFFFFL

        /** 视口全空（或无会话）时解码出的内容下沿行。 */
        const val LAST_CONTENT_ROW_NONE = -1

        /** render 缺省返回值：无可渲染的会话（未建立或已销毁），按 idle 处理。 */
        private const val RENDER_IDLE = 0

        /** pollAll() 每帧最多排空的事件数，限定渲染线程开销。 */
        private const val MAX_EVENTS_PER_POLL = 32
    }
}

/**
 * `renderWithNewOutput` 打包值 → [Bridge.RenderResult] 的纯解码。
 *
 * 顶层而非 [Bridge] 成员：解码只依赖位形，不依赖会话，故可脱离 JNI 直接以字面量
 * 测试。留在回调内联时，测试只能复述常量而测不到真实解码路径。
 */
internal fun decodeRenderResult(packed: Long): Bridge.RenderResult {
    val count = packed.toInt()
    // 仅屏蔽第 32 位：第 33..54 位承载内容下沿/失效位/备用屏位，
    // 不得泄漏到输出标志（空闲闭锁依赖该标志）。
    val newOutput = ((packed shr 32) and 0x1L) != 0L
    val lastContentRow =
        ((packed shr 43) and Bridge.LAST_CONTENT_ROW_NONE_BITS.toLong()).toInt().let { raw ->
            if (raw == Bridge.LAST_CONTENT_ROW_NONE_BITS) Bridge.LAST_CONTENT_ROW_NONE else raw
        }
    return Bridge.RenderResult(
        count,
        newOutput,
        lastContentRow,
        (packed and Bridge.SURFACE_INVALIDATED_BIT) != 0L,
        (packed and Bridge.ALT_SCREEN_ACTIVE_BIT) != 0L,
    )
}

/**
 * `getCellSizePacked` 打包值 → (宽, 高)。顶层而非 [Bridge] 成员：解码只依赖位形，
 * 不依赖会话，故可脱离 JNI 直接以字面量测试。
 */
internal fun unpackCellSize(packed: Long): Pair<Float, Float> {
    val width = Float.fromBits((packed and Bridge.LOW_32_BITS).toInt())
    return Pair(width, Float.fromBits((packed shr 32).toInt()))
}
