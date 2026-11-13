package terminal.emulator.runtime

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.settings.ColorsProperties
import terminal.emulator.settings.TerminalCursorStyle
import java.io.File
import kotlin.math.round

// 运行期的纯计算：网格与视口尺寸、渲染节拍与失败退避、会话与选区判定。
// 这些函数无副作用、无平台状态，故可表驱动单测；把它们与 [TerminalRuntime] 的
// 编排逻辑分开，运行期门面只留「何时调用」与「谁来调用」。
/**
 * 用户滚动手势后的窗口期，期间新输出不得把视口拽回底部（见 [shouldResetScroll]）。
 * 默认 termux 对等路径传入零时间戳，故该保护保持关闭。
 */
const val RECENT_SCROLL_WINDOW_NANOS: Long = 100_000_000L

/**
 * 渲染循环节奏门控的运动新鲜度窗口。手势仍在移动时（运动事件时间戳落在窗口内），
 * 循环保持 17ms 活跃闭锁，即使空闲时钟已因先前 >5s 空闲而陈旧。
 *
 * 滚动卡顿的根因：两个节奏门都只看空闲时钟新鲜度。空闲超 5s 后 vsync 停止驱动且循环驻留
 * 500ms 空闲闭锁，事件间隙（模拟器运动事件稀疏、或主线程忙于手势与重组）内无唤醒源，
 * 循环驻留 ~500ms，滚动帧率跌至 ~2fps。手势一停止即在一帧内老化，空闲闭锁重新生效。
 */
const val SCROLL_MOTION_WINDOW_NANOS: Long = 250_000_000L

/**
 * 活跃期闭锁超时：约一个显示周期（60Hz 显示 ~16.7ms，120Hz 显示 ~8.3ms）。
 *
 * 仅在 vsync 链不健康时使用。waitOutput 是纯 park（PTY 到达无法提前唤醒），
 * 这个兜底节拍保证主线程阻塞、vsync 信号丢失时输入回显仍在下一节拍上屏。
 *
 * 它是**安全网**而不是帧预算：按显示周期倍数描述，故在高刷屏上它自然覆盖
 * 多个周期，不会反过来变成与显示帧同周期的第二路节拍。
 */
const val RENDER_LATCH_TIMEOUT_NANOS: Long = 17_000_000L

/**
 * vsync 链健康时的活跃闭锁超时：约两个显示周期（60Hz 显示 ~33.3ms）。
 *
 * 必须明显大于一个显示周期，否则闭锁与 vsync 两路节拍会在同一周期内各推一帧
 * （见 [activeLatchTimeoutNanos]）。
 */
const val RENDER_LATCH_VSYNC_SAFETY_TIMEOUT_NANOS: Long = 34_000_000L

/**
 * vsync 新鲜度阈值：超过它即认为 Choreographer 链已停摆，活跃闭锁退回
 * [RENDER_LATCH_TIMEOUT_NANOS]。取约六个 60Hz 周期（100ms），容忍主线程的一次卡顿，
 * 又能在链真正断掉时于 100ms 内恢复兜底节拍。
 */
const val RENDER_VSYNC_STALE_NANOS: Long = 100_000_000L

/**
 * 显示周期采样窗口：一秒内没有更小的帧间隔即重新起算（刷新率可能已变）。
 */
internal const val DISPLAY_PERIOD_WINDOW_NANOS: Long = 1_000_000_000L

/** 可采信的显示周期区间：1ms（1000Hz）到 50ms（20Hz）。 */
internal const val MIN_DISPLAY_PERIOD_NANOS: Long = 1_000_000L
internal const val MAX_DISPLAY_PERIOD_NANOS: Long = 50_000_000L

/**
 * 显示周期估计：周期长度与该估计的采样时刻。
 *
 * 周期长度 0 = 尚未测得（帧回调只到过一次，算不出间隔）。
 */
internal data class DisplayPeriodEstimate(val periodNanos: Long, val sampledAtNanos: Long)

/**
 * 帧间隔是否落在可采信的显示周期区间内。
 *
 * 区间外的一律不采信：主线程长时间被占用时相邻两帧可能相隔数百毫秒，
 * 那是丢帧而不是显示周期。
 */
internal fun isCredibleDisplayInterval(intervalNanos: Long): Boolean =
    intervalNanos in MIN_DISPLAY_PERIOD_NANOS..MAX_DISPLAY_PERIOD_NANOS

/**
 * 用相邻两帧回调的间隔更新显示周期估计。纯函数，便于表驱动测试。
 *
 * 帧间隔是显示周期的整数倍（丢帧只会让间隔变大），故取窗口内观测到的最小值；
 * 窗口（[DISPLAY_PERIOD_WINDOW_NANOS]）内没有更小的样本即重新起算——刷新率
 * 切换后旧的小间隔会随之退出窗口。
 *
 * 尚未测得时（周期长度为 0）首个样本即周期：此时「更短」永远为假，不特判就
 * 永远测不出周期，刷新率也就无从观察。
 *
 * 落在合理区间之外的间隔一律不采信：主线程长时间被占用时相邻两帧可能相隔
 * 数百毫秒，那是丢帧而不是显示周期。
 */
internal fun updateDisplayPeriodEstimate(
    estimate: DisplayPeriodEstimate,
    intervalNanos: Long,
    nowNanos: Long,
): DisplayPeriodEstimate {
    if (!isCredibleDisplayInterval(intervalNanos)) return estimate
    val unmeasured = estimate.periodNanos <= 0L
    val windowExpired = nowNanos - estimate.sampledAtNanos > DISPLAY_PERIOD_WINDOW_NANOS
    val shorter = intervalNanos < estimate.periodNanos
    if (!unmeasured && !windowExpired && !shorter) return estimate
    return DisplayPeriodEstimate(periodNanos = intervalNanos, sampledAtNanos = nowNanos)
}

/** 空闲闭锁：没有任何信号时每 500ms 醒来尝试一帧（约 2 FPS）。 */
const val RENDER_LATCH_IDLE_TIMEOUT_NANOS: Long = 500_000_000L

/** 空闲判定阈值：距上次信号超过 5s 且无滚动运动即转入空闲闭锁。 */
const val RENDER_IDLE_THRESHOLD_NANOS: Long = 5_000_000_000L

/**
 * 慢帧与慢循环的判定阈值（毫秒）：超过即记一行诊断。
 *
 * 放在文件顶层是为了让判定口径的用例能取到同一个值：另存一份会在阈值调整时
 * 与实现漂移，用例仍绿而实际判定已变。
 */
const val SLOW_FRAME_LOG_THRESHOLD_MS = 30.0

/**
 * 滚动复位决策（termux `onScreenUpdated` 对等语义），纯函数无副作用以便表驱动测试。
 *
 * termux 语义：除文本选区激活或自动滚动被显式关闭外，新输出都会把视口滚回底部。
 * termux 的 `isAutoScrollDisabled()` 是宿主应用的显式开关，不由用户滚动触发，
 * 故默认走分支 A（recentlyScrolled 保持 false）。
 *
 * @param scrollActive SCROLL 按钮显式锁定（用户想停留在回浏览位置）。
 * @param hasSelectionOrDrag 选区激活或手柄拖拽进行中。
 * @param newOutput 本帧消费到的原生 PTY 摄入标志（不是 render() 计数，后者含空闲重绘）。
 * @param recentlyScrolled UI 滚动手势后 [RECENT_SCROLL_WINDOW_NANOS] 内为真，
 *   使进行中的滚动不被落在手势中途的输出拽回。
 */
internal fun shouldResetScroll(
    scrollActive: Boolean,
    hasSelectionOrDrag: Boolean,
    newOutput: Boolean,
    recentlyScrolled: Boolean = false,
): Boolean = newOutput && !hasSelectionOrDrag && !scrollActive && !recentlyScrolled

/**
 * 渲染循环节奏门控：仅当空闲时钟已陈旧且无滚动运动时才选 500ms 空闲闭锁，
 * 否则保持 17ms 活跃闭锁使输入回显及时渲染。
 *
 * 输入写入经 [SessionEntry.notifyRender] 刷新空闲时钟（含 [Bridge.onPtyWrite] 接入的
 * 硬件按键与 IME 路径），故空闲 >5s 后的单次退格能把循环拉回活跃节奏，
 * shell 回显在下一个 17ms 闭锁节拍渲染，而无需等满 500ms。纯函数无副作用。
 *
 * @param idleNanos 距上次信号的时间（新鲜 = 刚有 notifyRender/新输出）。
 * @param hasScrollMotion 运动新鲜度门控：手势仍在移动时保持活跃。
 * @param idleThresholdNanos 新鲜度阈值（调用处传 `RENDER_IDLE_THRESHOLD_NANOS`）。
 */
internal fun shouldUseIdleLatch(idleNanos: Long, hasScrollMotion: Boolean, idleThresholdNanos: Long): Boolean =
    idleNanos > idleThresholdNanos && !hasScrollMotion

/**
 * 启动尺寸是否可用。
 *
 * 只拒绝非正尺寸：分屏、自由窗口与折叠半屏都是合法的小窗口。此前按整块显示尺寸
 * 推导像素下界（宽度按等宽字形宽高比折算、高度按行高折半），小窗口一律被判成
 * 「等正确的 surfaceChanged」——会话建不出来，界面停在空白。
 *
 * 网格退化的情形由网格计算自身兜住：行列至少一行一列（见 `computeGridDimensions`）。
 */
internal fun isStartSizeUsable(width: Int, height: Int): Boolean = width > 0 && height > 0

/**
 * vsync 帧回调链是否应当停掉。
 *
 * 无会话：没有可渲染的东西；应用不可见：渲染已暂停，链没有消费者；全部会话连续
 * 空闲超过阈值且无待消费动画：渲染线程落在 500ms 空闲闭锁上，链不再提供额外信息。
 *
 * 三者任一成立即停链——链常驻的代价是主线程按显示帧率被唤醒（120Hz 屏即每秒
 * 120 次），而空闲期它什么也不产出。
 */
internal fun shouldStopVsyncChain(hasSessions: Boolean, appForeground: Boolean, hasActiveSession: Boolean): Boolean =
    !hasSessions || !appForeground || !hasActiveSession

/**
 * 停链后的复核：是否因为「判定与停链之间刚好来了新信号」而重新拉起链。
 *
 * 复核只在停链原因就是「会话空闲」时才作数：停链原因若是「应用不可见」或「已无
 * 会话」，复核必然得到「会话仍活跃」——后台时宿主信号在 5 秒阈值内仍算新鲜，于是
 * 每显示帧停一次又拉起一次，链永不停住，主线程被逐帧唤醒，链启动日志也逐帧刷屏。
 *
 * @param hasSessions 判定时刻是否还有会话。
 * @param appForeground 复核时刻应用是否可见（切后台须立即生效，故复核时重读）。
 * @param wasActive 判定时刻是否有活跃会话。
 * @param isActiveNow 复核时刻是否有活跃会话。
 */
internal fun shouldRestartVsyncChain(
    hasSessions: Boolean,
    appForeground: Boolean,
    wasActive: Boolean,
    isActiveNow: Boolean,
): Boolean = hasSessions && appForeground && !wasActive && isActiveNow

/**
 * 活跃期闭锁超时：vsync 链健康时它只作安全网，不得成为第二路帧节拍。
 *
 * 两路节拍（Choreographer 每显示帧 + 17ms 闭锁）周期只差 0.3ms，相位漂移会让
 * 闭锁偶尔落在 vsync 前一瞬：同一显示周期推出两帧（实测成对呈现间隔 3–5ms），
 * 交换链随即排满，下一次 acquire 阻塞约 70ms（4 个 vsync）——这正是「滑动缓慢、
 * 卡顿、闪烁」的形态。健康时把超时放宽到 2 个 vsync，使每个显示帧最多一帧；
 * vsync 链停摆（[SessionEntry.lastVsyncNanos] 不再推进）时退回 17ms 兜底，
 * 保持输入回显与滚动不被主线程阻塞拖死。纯函数无副作用。
 *
 * @param vsyncFresh 最近的 vsync 是否落在 [RENDER_VSYNC_STALE_NANOS] 内。
 */
internal fun activeLatchTimeoutNanos(vsyncFresh: Boolean): Long =
    if (vsyncFresh) RENDER_LATCH_VSYNC_SAFETY_TIMEOUT_NANOS else RENDER_LATCH_TIMEOUT_NANOS

/**
 * 慢循环判定用的实际工作时长：整循环时长扣掉两段主动等待。
 *
 * 空闲门控驻留与渲染失败退避都是设计上主动等的等待，不是慢：把它们算进去会让
 * 「平台收走原生窗口 → 渲染失败 → 退避」这条正常退化路径在退避后被判成慢循环，
 * 记出 warn 级告警（`logcat-hygiene` 要求正常退化路径不得记 warn）。纯函数无副作用。
 *
 * @param totalNanos 整循环时长。
 * @param gateNanos 空闲门控驻留时长。
 * @param backoffNanos 渲染失败退避时长。
 */
internal fun slowLoopWorkNanos(totalNanos: Long, gateNanos: Long, backoffNanos: Long): Long =
    totalNanos - gateNanos - backoffNanos

/**
 * [file] 是否以 ELF 魔数（0x7f 'E' 'L' 'F'）开头。用于在 prefix shell 解析中排除 shebang 脚本：
 * linker-wrapper 的 spawn 路径只能加载真正的 ELF 二进制。
 */
internal fun isElf(file: File): Boolean = try {
    file.inputStream().use { input ->
        val magic = ByteArray(4)
        val read = input.read(magic)
        read == 4 &&
            magic[0] == 0x7f.toByte() &&
            magic[1] == 'E'.code.toByte() &&
            magic[2] == 'L'.code.toByte() &&
            magic[3] == 'F'.code.toByte()
    }
} catch (exception: Exception) {
    LogUtil.warn("Runtime", "isElf: cannot read ${file.absolutePath}", exception)
    false
}

/**
 * 系统解释器启动脚本判定：首行 shebang 指向系统路径（`/system/bin/`）的脚本可由应用进程直接执行， 无需经 linker
 * 桥接；指向应用私有目录的脚本不计入（其解释器本身尚不可用）。 与安装器/启动路径共享，供通用 bootstrap 启动器脚本使用，不针对特定发行版。
 */
internal fun isSystemShellScript(file: File): Boolean = try {
    file.inputStream().use { input ->
        val header = ByteArray(64)
        val read = input.read(header)
        if (read <= 0) {
            false
        } else {
            val firstLine = header.decodeToString(0, read).lineSequence().firstOrNull().orEmpty()
            firstLine.startsWith("#!/system/bin/")
        }
    }
} catch (exception: Exception) {
    LogUtil.warn("Runtime", "isSystemShellScript: cannot read ${file.absolutePath}", exception)
    false
}

// 死渲染线程重启的退避起点、上限与重启后的健康确认宽限期（见 RenderSupervisor）。
internal const val INITIAL_RESTART_DELAY_MS = 100L
internal const val MAX_RESTART_DELAY_MS = 1000L
internal const val GRACE_PERIOD_AFTER_RESTART_MS = 300L

/** 下次死渲染线程重启的退避延迟：把先前延迟翻倍，上限 [maxDelayMs]（handleDeadRenderThread 中的指数退避）。 */
internal fun nextRestartDelayMs(currentMs: Long, maxDelayMs: Long): Long = (currentMs * 2).coerceAtMost(maxDelayMs)

/** 死亡渲染重启预算：尝试计数超过 [maxAttempts] 即关闭会话而非再次重启。 */
internal fun shouldCloseDeadRender(restartAttempts: Int, maxAttempts: Int): Boolean = restartAttempts > maxAttempts

/**
 * 渲染线程是否已死：循环退出标志已置位，或线程引用在且不再存活。
 *
 * 监视器据此把条目分成「待重启」与「健康」两组，两组 MUST 互斥。此前「健康」只要求
 * `!renderThreadExited`，而线程刚死时该标志尚未置位——同一条目因此同时落入两组，
 * [TerminalRuntime.RenderSupervisor.decayRestartCounts] 随即把它的
 * [SessionEntry.restartAttempts] 清零，
 * 重启预算永不耗尽（每 500ms 重启一轮，`RENDER_MAX_RESTART_ATTEMPTS` 形同虚设，
 * 最终表现为渲染循环反复重建而会话永不关闭）。
 *
 * 线程引用缺失（null）**不算死**：条目插入映射到
 * [TerminalRuntime.RenderSupervisor.startRenderThread]
 * 真正把线程写入之间，`running` 已为真而引用仍为空。把这段正常的启动窗口判成死亡
 * 会让监视器抢先重启，与即将启动的线程争同一会话（两条线程消费唯一事件队列）。
 * 真正的孤儿条目由失败回滚按 bridge 身份移除。
 *
 * @param threadAlive 线程当前是否存活；null = 线程引用尚未写入
 */
internal fun isRenderThreadDead(renderThreadExited: Boolean, threadAlive: Boolean?): Boolean =
    renderThreadExited || threadAlive == false

/**
 * 初始同步渲染重试（switchSession）：只有硬失败（< 0：会话未注册、渲染态异常等）
 * 才重试。**`0` 不是失败**：它是「本帧没有可画的内容」，surface 尚未就绪、目标会话
 * 暂无新帧、缓存帧属于别的会话都会返回 0——切会话后重推请求已由原生渲染路径发出，
 * 该帧到达即呈现，重试只会空转。纯决策，从重试循环中抽出以便测试。
 */
internal fun initialRenderRetryNeeded(result: Int, attempts: Int, maxAttempts: Int): Boolean =
    result < 0 && attempts < maxAttempts

/**
 * switchSession 阶段 3 的并发切换守卫：首帧渲染期间（在 sessionLock 之外），
 * 另一个 switchSession 可能发布了不同的活动会话。返回在发布本次切换之前
 * 必须停止其渲染线程的会话 id；当活动会话未变（或正是本目标）时返回 null。
 */
internal fun concurrentRenderThreadToStop(
    activeSessionIdAfterRender: Long?,
    previousActiveId: Long?,
    targetId: Long,
    concurrentSessionId: Long?,
): Long? {
    if (activeSessionIdAfterRender == previousActiveId) return null
    val concurrentId = concurrentSessionId ?: return null
    if (concurrentId == targetId) return null
    return concurrentId
}

/** switchSession 阶段 1 的失败恢复：spawn 失败后重启前一个活动会话，除非它正是刚刚失败的那个会话。 */
internal fun shouldRestorePreviousSession(previousId: Long?, failedTargetId: Long): Boolean =
    previousId != null && previousId != failedTargetId

/**
 * 原生接受的光栅缩放（sp→px 系数）区间：原生为唯一真源（`NativeBridge.getRasterScaleRange`）。
 *
 * 此前此处另存一份 `0.5f..8f` 字面量，与 Rust `setRasterScale` 的守卫只由注释绑定——
 * 与字号上界那次同型的第二处重复：两端一旦漂移，`coerceSpToPxScale` 会把原生会拒收的
 * 系数当作合法值，而字号上界（`SettingsRepository.fontSizeMaxSp` 以系数为分母）
 * 随之用错系数算，重新出现「设置条范围与实际可设置范围不一致」。
 */
internal val rasterScaleRange: ClosedFloatingPointRange<Float> by lazy {
    // 可空签名与 `listFontFamilies` 同款：原生的失败值是 null，让它以 null 到达
    // 调用方并在此处被指名道姓地拒绝，好过在 `bounds.size` 上抛一个无来由的 NPE。
    // 不设兜底默认值：钳到一个猜测值只会让字号上界与原生实际接受区间悄悄脱节，
    // 正是本属性存在的理由所反对的。
    val bounds =
        requireNotNull(NativeBridge.getRasterScaleRange()) {
            "原生未返回光栅缩放区间（JNI 调用失败）"
        }
    require(bounds.size == 2 && bounds.all { it.isFinite() && it > 0f } && bounds[0] <= bounds[1]) {
        "原生未给出合法的光栅缩放区间: ${bounds.contentToString()}"
    }
    bounds[0]..bounds[1]
}

/**
 * 一条会话的完整渲染契约：主题 + 字体族 + 字号 + 光栅尺度。
 *
 * 两条下发路径（建会话后的 `applyRenderSettings`、配置变化后的 `applySettings`）
 * 共用本函数：漏掉 `setRasterScale` 的那一条既不编译失败也不影响字号本身，
 * 症状只是系统「字体大小」变化后 Kotlin 按新系数算、原生仍按旧值光栅——
 * 字形被拉伸，单元格度量与网格整体偏掉一个 fontScale 倍。
 */
internal fun Bridge.applyRenderContract(contract: RenderContract) {
    setTheme(contract.themeName, contract.themeOverrides)
    setCursorStyle(contract.cursorStyleNativeValue)
    applyFontContract(contract.fontFamily, contract.fontSizeTenths, contract.rasterScale)
}

/**
 * 字体契约（字族 + 字号 + 光栅尺度）：三项都是**全局**量，原生侧只落在唯一的
 * 字体管线上，与会话无关。
 *
 * 独立成函数是因为它有两个调用时机，且两处必须给同一组值：
 * 建会话前的 spawn（此时定下的字体度量决定 PTY 的初始 winsize）与建会话后的
 * 契约下发（主题等其余项此时才可下发）。
 */
internal fun Bridge.applyFontContract(fontFamily: String, fontSizeTenths: Int, rasterScale: Float) {
    setFontFamily(fontFamily)
    setFontSizeInPlace(fontSizeTenths)
    setRasterScale(rasterScale)
}

/** 一条会话的渲染设置快照（见 [applyRenderContract]）。 */
internal data class RenderContract(
    val fontSizeTenths: Int,
    val fontFamily: String,
    val themeName: String,
    val rasterScale: Float,
    /** `termux.properties` 的 `terminal-cursor-style`，取 `TerminalCursorStyle.nativeValue`。 */
    val cursorStyleNativeValue: Int = TerminalCursorStyle.BLOCK.nativeValue,
    /**
     * `colors.properties` 的调色板覆盖槽（19 个，未覆盖的槽为负值）。
     *
     * 随主题一起下发：覆盖是**全局配置**，不随主题切换消失，故每次换主题都带上它。
     * 默认值是「全部槽不覆盖」——原生按固定长度解包，槽数不对会被拒收。
     */
    val themeOverrides: IntArray = IntArray(ColorsProperties.PALETTE_OVERRIDE_SLOTS) { ColorsProperties.NO_OVERRIDE },
)

/**
 * 引导前的占位网格尺寸（xterm 的经典 24×80）。
 *
 * 单一来源：[RuntimeState] 的默认值、视图侧的初始镜像与 `buildConfig` 都取这里。
 * 三处各写一份时，改一处不会影响另两处——「首个会话的网格」与「视图在拿到真实
 * 尺寸前显示的网格」会分家，表现为启动瞬间的行列跳变。
 */
const val DEFAULT_GRID_ROWS = 24
const val DEFAULT_GRID_COLS = 80

/**
 * sp→设备像素的完整系数 = 显示密度 × 系统字体缩放，钳到原生接受的区间。
 *
 * 顶层纯函数以便测试：字号的每一次 sp↔px 换算都经此系数，钳位区间 MUST 与
 * 原生 `setRasterScale` 一致——区间外的值被原生拒收并记错误日志，会使字号上界
 * 与实际渲染脱节。区间由 [rasterScaleRange] 提供，故纯函数不直接碰 JNI。
 */
internal fun coerceSpToPxScale(density: Float, fontScale: Float, range: ClosedFloatingPointRange<Float>): Float =
    (density * fontScale).coerceIn(range.start, range.endInclusive)

/**
 * `default-working-directory` 的解析（termux 语义）。
 *
 * 空值 → 家目录；`~` 或 `~/x` → 家目录下；相对路径 → 相对家目录；绝对路径 → 原样。
 *
 * 解析结果必须落在真实存在且可读的目录上，否则回退家目录（termux 的
 * `getDefaultWorkingDirectoryInternalPropertyValueFromValue` 同样回退）：目录不存在或
 * 不可读时 PTY 会直接失败，会话根本起不来——配置里的一个笔误不该让应用「打不开终端」。
 *
 * 尾部的 `/` 一律去掉（`/` 自身除外）：shell 启动后经 OSC 7 回报的是规范化路径，若配置里
 * 写成 `~/src/`，会话抽屉会先显示 `src/`、第一条提示符后翻成 `src`——同一个会话在两帧之间
 * 换了名字，看起来像切换了目录。
 */
internal fun resolveWorkingDirectory(configured: String, homeDir: String): String {
    val trimmed = configured.trim()
    if (trimmed.isEmpty()) return homeDir
    val expanded =
        when {
            trimmed == "~" -> homeDir
            trimmed.startsWith("~/") -> homeDir + trimmed.removePrefix("~")
            else -> trimmed
        }
    val absolute = if (expanded.startsWith("/")) expanded else "$homeDir/$expanded"
    val normalized = absolute.trimEnd('/').ifEmpty { "/" }
    val directory = File(normalized)
    return if (directory.isDirectory && directory.canRead()) normalized else homeDir
}

/**
 * recomputeGridFromFontMetrics 背后的纯网格尺寸计算：cols = floor(surfaceWidth / cellWidth)，
 * rows = floor(surfaceHeight / cellHeight)，各自钳位到 ≥ 1，使极小 Surface 仍得到可用网格。
 * 退化输入（Surface 或单元格度量非正）返回 (0, 0)，
 * 使调用方能区分「几何无效」与真实的一行一列网格，并改为保持当前尺寸。
 */
internal fun computeGridDimensions(
    surfaceWidth: Int,
    surfaceHeight: Int,
    cellWidth: Float,
    cellHeight: Float,
): Pair<Int, Int> {
    if (surfaceWidth <= 0 || surfaceHeight <= 0 || cellWidth <= 0f || cellHeight <= 0f) {
        return Pair(0, 0)
    }
    val cols = (surfaceWidth / cellWidth).toInt().coerceAtLeast(1)
    val rows = (surfaceHeight / cellHeight).toInt().coerceAtLeast(1)
    return Pair(rows, cols)
}

/**
 * spawn PTY 之前能确定的最佳网格：用原生字体度量与已知的 surface 像素尺寸算出真实
 * 行列，度量或尺寸尚未就绪时退回 [DEFAULT_GRID_ROWS]×[DEFAULT_GRID_COLS]（旧行为）。
 *
 * 为什么必须在 spawn 之前算：PTY 的 winsize 决定 shell 怎么排版它的第一个 prompt。
 * 先按默认 24×80 建 PTY、attach 之后才 resize，会让 shell 收到 SIGWINCH 重排**已经
 * 打印过**的 prompt——真机日志里新会话首屏正是从中间截断、第二行才补齐。
 * 用真实网格建 PTY 后，attach 之后的首次 resize 变成零变更（见
 * [TerminalRuntime.recomputeGridFromFontMetrics] 的相等早退）。
 */
internal fun initialSpawnGrid(
    surfaceWidth: Int,
    surfaceHeight: Int,
    cellWidth: Float,
    cellHeight: Float,
    modifierBarHeightPx: Int,
    imeReserve: Int,
): Pair<Int, Int> {
    // 守卫必须看 spawn 时的原始像素尺寸，不能只看 computeGridDimensions 的 (0,0) 哨兵：
    // computeGridAvailableHeight 会把「高度 ≤ 键栏 + 输入法」钳到 1 px，于是 0 高度的
    // surface 会绕过哨兵、被 computeGridDimensions 钳成一行——一个 1 行的 PTY 比默认网格更糟
    // （布局尚未发生时的尺寸竞态正是这样）。
    if (surfaceWidth <= 0 || surfaceHeight <= 0 || cellWidth <= 0f || cellHeight <= 0f) {
        return Pair(DEFAULT_GRID_ROWS, DEFAULT_GRID_COLS)
    }
    return computeGridDimensions(
        surfaceWidth = surfaceWidth,
        surfaceHeight =
        computeGridAvailableHeight(
            surfaceHeight = surfaceHeight,
            modifierBarHeightPx = modifierBarHeightPx,
            imeReserve = imeReserve,
        ),
        cellWidth = cellWidth,
        cellHeight = cellHeight,
    )
}

/**
 * 网格的可用高度：Surface 高度扣掉修饰键栏覆盖层与输入法遮挡。
 *
 * 三条网格重算路径（运行期的 `recomputeGridFromFontMetrics`、
 * `TerminalSurface.ResizeManager.applyGridResize` 与 `recomputeRowsColsImmediate`）
 * MUST 共用本函数：它们此前各自展开这行算式，其中运行期那条还漏了 `coerceAtLeast`，
 * 于是「Surface 不高于键栏 + 输入法」这一几何下它返回 (0,0) 并**静默保持旧网格**
 * （输入法扣减被丢掉），另两条却收敛到一行——同一几何两种网格，正是
 * 「扣减量单一来源」要防的分叉。
 *
 * @param imeReserve [TerminalRuntime.imeGridReserve]：只在备用屏非零，主屏恒 0。
 */
internal fun computeGridAvailableHeight(surfaceHeight: Int, modifierBarHeightPx: Int, imeReserve: Int): Int =
    (surfaceHeight - modifierBarHeightPx - imeReserve).coerceAtLeast(1)

// surface 重建请求的最小间隔（纳秒）：一次重建含拆装视图 + 新 surface 交付 + 首帧，
// 短于此的重试只会连累在途的重建。
private const val SURFACE_RECREATE_MIN_INTERVAL_NANOS = 500_000_000L

// 单会话连续重建请求的快试上限：超过即转为慢重试（避免无限拆装视图，又不放弃自愈）。
internal const val SURFACE_RECREATE_MAX_ATTEMPTS = 5

// 慢重试间隔（纳秒）：快试用尽后每次请求的间隔。取 5s 是因为这段长尾只在平台迟迟
// 不交付新窗口时出现，一次重建本身要经历拆装视图 + 新 surface 交付 + 首帧。
internal const val SURFACE_RECREATE_SLOW_INTERVAL_NANOS = 5_000_000_000L

// 同一间隔的毫秒形式：只用于日志文案，避免两处常量各写一个数字。
internal const val SURFACE_RECREATE_SLOW_INTERVAL_MS = SURFACE_RECREATE_SLOW_INTERVAL_NANOS / 1_000_000

/**
 * 内容下沿像素：视口最后一个有内容的行的下沿（视口全空为 0）。
 *
 * 原生行顶为 `row * cellHeight`（浮点，见 `cell_builder.rs` 的 `quad_origin`），
 * 故此处必须先浮点乘后取整。先 `toInt` 再乘会每行丢掉小数并随行数累积
 * （N 行累积误差 = N × 小数部分），位移偏小、末行被键栏吞掉且随内容增多扩大。
 * 取最近整数：浮点乘自带表示误差（如 50×45.6 得 2279.9999），`round` 恰好回到
 * 本意像素边界；`ceil` 会把 2279.9999 进成 2280 后在某些对齐下多移 1px，
 * 整屏文字错开 1px 即产生数千边缘差异像素，违背稀疏会话「无变化」。
 */
internal fun computeContentBottomPx(contentRow: Int, cellHeightPx: Float): Int {
    if (contentRow < 0 || cellHeightPx <= 0f) return 0
    return round((contentRow + 1) * cellHeightPx).toInt()
}

/**
 * 切会话时是否需要对齐网格：目标会话原生网格与 UI 网格一致则跳过 `resize`
 *（无冗余 SIGWINCH）；查不到（0/异常）则返回 true 沿旧路无条件对齐。
 *
 * 网格查询以 lambda 传入：纯判定可单测，无需伪造 Bridge。
 */
internal fun shouldAlignGridOnSwitch(wantRows: Int, wantCols: Int, gridQuery: () -> Long): Boolean {
    val packed =
        try {
            gridQuery()
        } catch (exception: Exception) {
            // 查询失败与「查到 0」同解（都走无条件对齐），但必须留痕：
            // 静默吞掉会让「切会话后网格错位」无从追溯。
            LogUtil.error("Runtime", "grid query failed while switching session", exception)
            0L
        }
    return packed == 0L || (packed shr 32).toInt() != wantRows || packed.toInt() != wantCols
}

/**
 * 切会话的网格对齐执行：`[shouldAlignGridOnSwitch]` 为真才 `resize`。
 *
 * 独立成顶层函数是为了让判定逻辑可单测（`shouldAlignGridOnSwitch` 有同名单测）。
 */
internal fun alignGridOnSwitch(bridge: Bridge, rows: Int, cols: Int) {
    val wantRows = rows.coerceAtLeast(1)
    val wantCols = cols.coerceAtLeast(1)
    if (shouldAlignGridOnSwitch(wantRows, wantCols) { bridge.getGridRowsColsPacked() }) {
        bridge.resize(wantRows, wantCols)
    }
}

/**
 * 输入法弹出时终端 Surface 的上移像素：只移「键盘遮住且上方放不下」的内容高度。
 *
 * 网格自顶端锚定渲染，键盘遮住的是网格**末尾**行，故无条件按整块键盘高度平移会把
 * 稀疏会话（提示符在首行）整体推出屏幕上边界，终端区表现为全空（实测提示符由
 * y=134 落到 y=−686）。本函数返回的量等价于 Termux `adjustResize` 会砍掉的那些行高：
 * 放得下的内容每个像素都留在原处，放不下的才上移，且上移后末行恰好贴在键栏顶边。
 *
 * 备用屏（helix/vim 等全屏 TUI）恒占满视口，任何位移都会把应用顶部推出屏幕，
 * 且位移后的视觉行与触摸换算行错位；此时位移恒为 0（顶部保持可见，触摸对齐恢复）。
 * 主屏公式与行为不变。
 *
 * @param contentBottomPx 内容下沿像素（视口最后一个有内容的行的下沿，视口全空为 0）
 * @param surfaceHeightPx Surface 布局高度（容器高度，键栏覆盖其底部）
 * @param modifierBarHeightPx 键栏高度（网格已按同一口径预留）
 * @param imeBottomPx 键盘遮挡高度（已扣除被 `navigationBarsPadding` 消费的系统导航条）
 * @param isAltScreenActive 备用屏是否激活（全屏 TUI 应用）
 *
 * 上界取 `imeBottomPx`：位移超过键盘高度会在键盘上方留下一段终端背景空隙。
 * 网格已保证内容下沿不超过网格高度，故该上界在正常路径上恒不生效，
 * 只在字号变化瞬间（行数尚未随新行高重算）收敛位移。
 *
 * 位移把网格**顶部**若干行推出屏幕上边界（见 [terminal.emulator.ui.TerminalSurface] 的显出量处理）：被推出
 * 的行不在上滑行程里，只能由边界外的过度滑动按像素回收位移看回来。
 *
 * 显出量按「内容末行」与「光标所在行」中较低的一行计算：光标常落在最后一个有字行的
 * **下一行**（提示符整行占满、或命令行结尾换行后），只按内容末行算出的位移会把光标
 * 留在键盘后面——现象就是「prompt 占满整行时输入法光标不可见、没有剩余空间」。
 */
internal fun computeImeSurfaceShift(
    contentBottomPx: Int,
    surfaceHeightPx: Int,
    modifierBarHeightPx: Int,
    imeBottomPx: Int,
    cursorBottomPx: Int,
    isAltScreenActive: Boolean = false,
): Int {
    if (isAltScreenActive) return 0
    val visibleContentPx = surfaceHeightPx - modifierBarHeightPx - imeBottomPx
    // 光标行**低于**内容末行时取光标行；取不到光标（负值/0）时退化为只看内容末行。
    val revealBottomPx = maxOf(contentBottomPx, cursorBottomPx)
    return (revealBottomPx - visibleContentPx).coerceIn(0, imeBottomPx)
}

/**
 * surface 重建请求的纯决策（`maybeRequestSurfaceRecreate` 的判据）。
 *
 * 失效位在原生侧重建成功后回落为 0，故「判死后未回落」本身即是边沿。前
 * [SURFACE_RECREATE_MAX_ATTEMPTS] 次按 [SURFACE_RECREATE_MIN_INTERVAL_NANOS] 快试，
 * 用尽后改按 [SURFACE_RECREATE_SLOW_INTERVAL_NANOS] 慢重试：快试覆盖常态，慢重试
 * 兜住平台迟迟不交付新窗口的长尾，既不无限拆装视图，也不把渲染线程永久钉在死窗口上。
 *
 * @param attempts 已发出的请求次数
 * @param lastRequestNanos 上次请求时刻（纳秒，0 = 从未请求）
 * @param nowNanos 当前时刻（纳秒）
 */
internal fun decideSurfaceRecreate(attempts: Int, lastRequestNanos: Long, nowNanos: Long): SurfaceRecreateDecision {
    val fastPhase = attempts < SURFACE_RECREATE_MAX_ATTEMPTS
    val interval = if (fastPhase) SURFACE_RECREATE_MIN_INTERVAL_NANOS else SURFACE_RECREATE_SLOW_INTERVAL_NANOS
    val intervalElapsed = lastRequestNanos == 0L || nowNanos - lastRequestNanos >= interval
    return SurfaceRecreateDecision(request = intervalElapsed, exhausted = !fastPhase)
}

/**
 * [decideSurfaceRecreate] 的结论：[request] = 本帧请求换新的原生窗口，
 * [exhausted] = 已进入慢重试阶段（调用方据此只告警一次，请求本身照发）。
 */
internal data class SurfaceRecreateDecision(val request: Boolean = false, val exhausted: Boolean = false)

/**
 * 瞬时渲染错误是否按 warn 记：首次是正常退化，连续出现才是异常。
 *
 * 首次出现几乎都是平台收走原生窗口后的正常退化——切后台时系统回收 BufferQueue，
 * 渲染线程的探针帧取不到纹理，宿主随即换新窗口重挂载即恢复（见 `render-stability`
 * 规范「渲染 surface 失效后自愈」）。这类事件按 info 记，不占 W 闸门；
 * 连续出现说明新窗口迟迟不来，那才是异常，保留 warn。
 *
 * @param consecutiveErrors 含本次在内的连续错误数（1 = 首次）
 */
internal fun transientRenderErrorIsWarning(consecutiveErrors: Int): Boolean = consecutiveErrors > 1

/**
 * 请求换新的原生窗口是否按 warn 记：首次是正常退化，连续才是异常。
 *
 * 平台收走原生窗口后渲染帧取不到纹理，宿主随即请求换新窗口重挂载即恢复
 * （见 `logcat-hygiene` 规范「平台收走原生窗口后的瞬时渲染错误」）。首次请求
 * 按 info 记；连续请求说明新窗口迟迟不来，那才是异常，保留 warn。
 *
 * @param attempt 含本次在内的请求次数（1 = 首次）
 */
internal fun surfaceRecreateIsWarning(attempt: Int): Boolean = attempt > 1

/**
 * 渲染线程抛出的异常是否只是关闭路径的中断信号：不是渲染失败。
 *
 * 关闭路径（会话切换、运行期停止）中断渲染线程时，`runBlocking` 以取消异常收尾，
 * 其 cause 是 `InterruptedException`（栈顶落在 `BlockingCoroutine.joinBlocking`）。
 * 把它当渲染失败会记出 error 加完整栈——关闭流程的正常一步不是缺陷
 * （见 `logcat-hygiene`「应用自身的正常路径不得产生警告与错误」）。
 *
 * 判据取 cause 而非异常类型：取消异常也可能来自别处（协程作用域被取消），
 * 只有 cause 是 `InterruptedException` 时才说明是线程被中断。
 */
internal fun isRenderThreadInterruption(exception: Throwable): Boolean = exception is InterruptedException ||
    (exception is CancellationException && exception.cause is InterruptedException)

/** 渲染失败后的短退避（毫秒）：连续错误未达长退避阈值时用。 */
internal const val RENDER_ERROR_SLEEP_MS = 50L

/** 渲染失败后的长退避（毫秒）：连续错误超过阈值后用，避免持续失败时空转。 */
private const val RENDER_ERROR_BACKOFF_MS = 200L

/**
 * 连续渲染错误达到该次数后退避从短改长。
 *
 * 阈值原为内联字面量 10，其含义只记在长退避常量的注释里——改阈值而漏改注释，
 * 两处便各说各话。具名后阈值与含义同处一地。
 */
private const val RENDER_ERROR_BACKOFF_AFTER_ERRORS = 10

/**
 * 渲染失败后的退避时长（毫秒）。
 *
 * 连续错误数未超过 [RENDER_ERROR_BACKOFF_AFTER_ERRORS] 时用短退避，超过则用长退避：
 * 平台收走原生窗口这类正常退化只持续一两次，短退避让恢复更快；持续失败（新窗口
 * 迟迟不来）时长退避，避免渲染线程在无望的失败上空转。
 *
 * @param consecutiveErrors 到本次失败为止已连续发生的渲染错误数（调用点在计数加一后传入）
 */
internal fun renderErrorBackoffMillis(consecutiveErrors: Int): Long =
    if (consecutiveErrors > RENDER_ERROR_BACKOFF_AFTER_ERRORS) {
        RENDER_ERROR_BACKOFF_MS
    } else {
        RENDER_ERROR_SLEEP_MS
    }

/** 等待原生库加载完成的最长时间（毫秒）。 */
private const val NATIVE_LIBRARY_LOAD_TIMEOUT_MS = 3_000L

/** 等待原生库加载期间的轮询间隔（毫秒）。 */
private const val NATIVE_LIBRARY_LOAD_POLL_INTERVAL_MS = 50L

/**
 * 遮罩回读的安全取回：失败不致命，退回「没有遮罩」。
 *
 * 回读发生在渲染线程的**退出路径**上，所在的外层 try 只接 `InterruptedException`：
 * 这里的任何异常都会逃出渲染线程、掀掉整个进程（真机日志：切后台时遮罩帧回读
 * 撞上设备错误，进程 FATAL 退出并重启）。故异常在此就地兜住并如实上报——
 * 遮罩拿不到只是退回平底色（用户看到的是闪一下），而进程被掀掉是更坏的结果。
 *
 * `CancellationException` MUST 继续向上抛：那是协程取消信号，不是回读失败。
 *
 * @param read 取回遮罩帧；返回 null 表示没有可遮罩的内容。
 * @param onFailure 失败时的上报（异常照实交出，不静默吞掉）。
 */
internal inline fun <T> readCoverFrameSafely(read: () -> T?, onFailure: (Exception) -> Unit): T? = try {
    read()
} catch (exception: CancellationException) {
    throw exception
} catch (exception: Exception) {
    onFailure(exception)
    null
}

/**
 * 等待原生库加载完成，最多等 [NATIVE_LIBRARY_LOAD_TIMEOUT_MS] 毫秒。
 *
 * 原生库由 NativeInit 线程后台装载，首帧 surface 就绪时可能尚未完成；等待而非直接
 * 放弃，避免浪费一轮布局重建的启动延迟。等待以时间预算表达，不用「轮询次数」——
 * 轮询次数隐含「每次恰好间隔固定时长」的前提，负载高时实际等待会远超标称值。
 *
 * @param isLoaded 原生库是否已装载的判据
 * @return 超时前装载完成返回 true，超时返回 false
 */
internal suspend fun awaitNativeLibraryLoad(isLoaded: () -> Boolean = { NativeBridge.isNativeLoaded() }): Boolean =
    withTimeoutOrNull(NATIVE_LIBRARY_LOAD_TIMEOUT_MS) {
        while (!isLoaded()) {
            delay(NATIVE_LIBRARY_LOAD_POLL_INTERVAL_MS)
        }
        true
    } ?: false
