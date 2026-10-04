package terminal.emulator.runtime

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.os.SystemClock
import android.view.Surface
import androidx.compose.ui.graphics.toArgb
import androidx.core.net.toUri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import terminal.emulator.BuildConfig
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.BridgeTheme
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.Shell
import terminal.emulator.bridge.TerminalConfig
import terminal.emulator.bridge.createBridge
import terminal.emulator.monitor.FrameMarks
import terminal.emulator.monitor.RenderWatchDog
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.util.runCatchingCancellable
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicReference
import javax.inject.Inject
import javax.inject.Singleton

data class RuntimeState(
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val rows: Int = 24,
    val cols: Int = 80,
    val activeSessionId: Long = 0L,
    val sessionIds: List<Long> = emptyList(),
    val scrollResetEpoch: Long = 0L,
)

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
 * 会话状态由两个布尔量编码：running 且未退出 = 存活；running 且已退出 = 死亡（待清理）；
 * running 为假 = 已停止（陈旧条目，跳过）。renderThreadExited 由渲染线程在循环退出后置位，
 * 须与 running 一同在 sessionLock 下读取。
 */
internal data class SessionEntry(
    val id: Long,
    // 不变式：条目存活期间 bridge 从不为 null（createSession 中以非 null 创建且不再重新赋值），
    // 各处 `entry.bridge == null` 检查纯属防御，恒为假。
    var bridge: Bridge?,
    // 无锁读取（notifyRender/pokeVsync 在锁外解引用唤醒），与相邻字段同为易变。
    @Volatile var renderThreadRef: Thread?,
    @Volatile var running: Boolean,
    @Volatile var renderThreadExited: Boolean = false,
    @Volatile var restartAttempts: Int = 0,
    // 初始值同 TerminalRuntime.INITIAL_RESTART_DELAY_MS（100L）；decayRestartCounts/confirmRestartGrace
    // 会在健康期后重置为该常量，故新会话须从相同延迟起步。
    @Volatile var nextRestartDelayMs: Long = 100L,
    // 已调度死亡渲染线程的重启且仍在等待退避延迟。防止渲染监视器在退避延迟（最长 1000ms）
    // 未到期期间每个 500ms 节拍重复派发同一死亡条目——否则 restartAttempts 被重复计数而提前关闭会话。
    @Volatile var restartScheduled: Boolean = false,
    // 渲染线程未能在 join 超时内退出，可能仍在对本会话的 bridge/surface 执行原生渲染代码。
    // 置位期间 closeSession/stop/closeDeadSession 绝不可调用 releaseGpuSurface() 或 bridge.close()，
    // 在复活的线程下销毁原生会话即 use-after-free。仅在后续 join 确认线程退出后清除；
    // 泄漏的会话随进程消亡回收。
    @Volatile var renderThreadPossiblyAlive: Boolean = false,
    // 未能在 join 超时内退出的渲染线程（GPU 挂起）。保留以便后续退出路径再 join 一次：
    // 若已退出则可安全关闭会话，仍挂起则必须跳过关闭。
    @Volatile var hungRenderThread: Thread? = null,
    // closeSession 开始时在 sessionLock 下置位。startRenderThread 检查它并拒绝启动新线程，
    // 封闭「关闭与重启」的 TOCTOU 窗口：否则并发的 resumeRendering/switchSession 可能在关闭决定之后、
    // bridge.close() 之前启动新渲染线程，留下轮询已销毁会话的孤儿线程（全局事件队列双重消费、原生 UAF 风险）。
    @Volatile var closing: Boolean = false,
    // 为真时（SCROLL 按钮激活），新输出不应自动复位滚动——用户有意停留在回浏览位置。
    @Volatile var scrollActive: Boolean = false,
    // 最近一次 UI 线程滚动手势的时间戳（System.nanoTime，在 setScrollOffset 中写入），
    // 渲染线程据此计算 recentlyScrolled 保护；0L 表示从未滚动过。
    @Volatile var lastScrollNanos: Long = 0L,
    // 输入→回显延迟探针：输入打点落在 writeToPty 与 Bridge.onPtyWrite（硬件按键绕过 writeToPty），
    // 回显配对发生在渲染循环消费原生 new_output 标志时。
    val latencyProbe: LatencyProbe = LatencyProbe(),
    // shell 已退出且 [Process completed (code X)] 提示已送入终端（见 feedProcessCompletedPrompt）。
    // 会话保持可见与运行，直到用户按 Enter。
    @Volatile var waitingForProcessCompleted: Boolean = false,
    // 显示 [Process completed] 提示时捕获的退出码；Enter 确认关闭时复用。
    /**
     * 退出会话缓存的退出码；`null` 表示原生未能取得（`waitpid` 失败），**不是** 0。
     *
     * 用户在 `[Process completed]` 提示上回车确认关闭时，渲染循环要把缓存的退出码
     * 再交给 [handleSessionExit] 一次；可空类型让「码未知」在这一跳里也不会退化成 0。
     */
    @Volatile var processExitCode: Int? = null,
    // 用户在提示上按 Enter 时由 writeToPty 置位。渲染循环侦测到后重新派发 handleSessionExit，
    // 使 bridge 关闭留在渲染线程上（避免对存活渲染循环的 UAF）。
    @Volatile var processCompletedConfirmed: Boolean = false,
) {
    // renderSignaled 取代了每帧一个 CountDownLatch：后者有丢唤醒竞态——bridge.render() 后循环发布
    // 新的 latch 并等待，但渲染期间生产者对旧 latch 的 countDown() 会让新 latch 始终未置位，
    // 线程只能等满超时。锁/条件变量下的合并标志同时避免了竞态与每帧分配。
    //
    // 保留的可接受窗口：落在 get() 检查与 waitOutput 后 set(false) 之间的 notifyRender() 会被并入标志，
    // 并可能被该 set(false) 一并清除，代价是一个空闲超时（最长 ~500ms）——绝不会丢帧
    // （下次信号或周期帧节拍会重渲染）。信号合并天然允许此情形，逐信号队列在此属过度设计。

    val renderSignaled = java.util.concurrent.atomic.AtomicBoolean(false)

    @Volatile var forceRenderRequested: Boolean = false

    // vsync 对齐：由主线程上的 Choreographer 帧回调在每个显示帧置起。
    // 该回调只发信号——它绝不触碰 surface Mutex，也绝不直接调用 bridge.render()
    // （有互斥量饿死的先例：非渲染线程的渲染调用卡在 surface Mutex 上，
    // 永久阻塞了真正的渲染线程）。由渲染循环的唤醒门控消费。
    @Volatile var vsyncRequested: Boolean = false

    /**
     * 一帧的起止时刻（纳秒）：渲染线程是唯一写者，看门狗是读者。
     * 两个独立 `@Volatile` 会被跨帧混读，伪造出 `start > done`（R25-T2）。
     * 整条记录单次发布，读者拿到的必定是同一帧的起止。
     */
    @Volatile var frameMarks: FrameMarks = FrameMarks()

    @Volatile var renderWatchDog: RenderWatchDog? = null

    @Volatile var lastSignalNanos: Long = System.nanoTime()

    fun notifyRender() {
        lastSignalNanos = System.nanoTime()
        renderSignaled.set(true)
        renderThreadRef?.let {
            java.util.concurrent.locks.LockSupport.unpark(it)
        }
    }

    /**
     * vsync 对齐的唤醒：置起渲染线程唤醒，但既不触碰空闲时钟，也不设置 [renderSignaled]。
     * 此前每个 vsync 都经 [notifyRender] 刷新 [lastSignalNanos]，
     * 使该时钟永远新鲜，空闲闭锁因而永不生效
     *（空闲终端上自我维持的 60-166fps 循环）。
     */
    fun pokeVsync() {
        renderThreadRef?.let {
            java.util.concurrent.locks.LockSupport.unpark(it)
        }
    }

    @Volatile var scrollOffset: Int = 0

    /**
     * 本会话渲染线程最后见到的视口光标行（0 起，Bridge.CURSOR_ROW_UNKNOWN = 隐藏/在视口外）。
     * 与活动会话的 cursorRowFlow 对应，使会话切换无需 JNI 查询即可重新初始化它。
     */
    @Volatile var cursorRow: Int = Bridge.CURSOR_ROW_UNKNOWN

    /**
     * 本会话渲染线程最后见到的视口最后一个有内容的行（0 起，
     * [Bridge.LAST_CONTENT_ROW_NONE] = 视口全空）。输入法跟随位移按它裁剪平移量：
     * 稀疏会话不下移（内容原位），内容占满网格时按整块键盘高度上移。
     * 与活动会话的 [lastContentRowFlow] 对应，会话切换时随之重新初始化。
     */
    @Volatile var lastContentRow: Int = Bridge.LAST_CONTENT_ROW_NONE

    /**
     * 渲染线程最后见到的原生 surface 失效标志（缓存的原生窗口对应被遗弃的
     * BufferQueue，reconfigure 不可复活）。失效时由 `maybeRequestSurfaceRecreate`
     * 按间隔请求宿主换新的原生窗口（私有成员，dokka 无法解析链接，故不用方括号）。
     */
    @Volatile var surfaceInvalidated: Boolean = false

    /**
     * 换视图预算：已请求次数、上次请求时刻（纳秒）与「上限已告警」标志。
     *
     * 三项是一条逻辑记录，由渲染线程与 `surfaceTransitionExecutor` 线程各自成组读写。
     * `AtomicReference` 装整条不可变记录：读到的必定是某一次完整写入；
     * 写回一律 CAS——期间若有复位落地（整条替换），CAS 失败并按新值重判，
     * 旧计数永远写不回去（R25-T1）。
     */
    val surfaceRecreateBudget = AtomicReference(SurfaceRecreateBudget())

    /** 不可变的换视图预算快照；替换整条记录即为原子复位。 */
    data class SurfaceRecreateBudget(
        val attempts: Int = 0,
        val lastRequestNanos: Long = 0L,
        val exhaustedLogged: Boolean = false,
    )

    /**
     * 逐像素滚动余量（px，正值 = 内容下移），由渲染线程与 [scrollOffset] 一同取用。
     * 手势结束/会话切换时重置为 0，避免陈旧偏移泄漏到下一次手势。
     */
    @Volatile var scrollRemainderPx: Float = 0f

    /**
     * 滚动手势是否正在移动：最近的滚动运动事件（逐像素 `setScrollRemainderPx` 或整行 `setScrollOffset`）
     * 在 [SCROLL_MOTION_WINDOW_NANOS] 内刷新过 [lastScrollNanos]。
     * 使渲染循环在手势期间保持活跃节奏，不被 ~500ms 空闲闭锁驻留。
     */
    fun hasScrollMotion(): Boolean = System.nanoTime() - lastScrollNanos < SCROLL_MOTION_WINDOW_NANOS
}

// ═══════════════════════════════════════════════════════════════════════════
// 一、字段与注入依赖
// ═══════════════════════════════════════════════════════════════════════════

@Singleton
class TerminalRuntime
@Inject
constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepository: SettingsRepository,
) {
    /**
     * 由 TerminalSurface 移交的 Surface；待会话的 Bridge 就绪后才绑定（attach 只能在 spawn 之后）。
     *
     * 三元组必须放在**同一个** `@Volatile` 持有者里：三个独立字段只保证各自可见，
     * 读者仍可能拿到新 Surface 配旧尺寸（或反之）——而 `attachSurface` 的尺寸必须与
     * 该 Surface 同源（见 `ffi.rs` 的 BufferQueue 几何说明），否则交换链配置与队列几何
     * 各说各话，模拟器上直接拒绝出队。
     */
    @Volatile private var pendingSurface: PendingSurface? = null

    /** 一次移交的 Surface 与其尺寸；不可变，读一次即自洽。 */
    private class PendingSurface(val surface: android.view.Surface, val width: Int, val height: Int)

    /** 渲染线程生命周期监管。 */
    val renderSupervisor = RenderSupervisor()

    private val clipboardAccess = ClipboardAccess(context, tag = "Runtime")

    private val eventDispatcher = EventDispatcher()

    private val scope = CoroutineScope(SupervisorJob() + terminal.emulator.util.TerminalDispatchers.inputOutput)

    private val _state = MutableStateFlow(RuntimeState())
    val state: StateFlow<RuntimeState> = _state.asStateFlow()

    /**
     * 活动会话的视口光标行（0 起，[Bridge.CURSOR_ROW_UNKNOWN] = 隐藏/在视口外）。
     * 仅在变化时由渲染线程发布；键盘打开时输入法跟随滚动订阅它。
     * 与 [state] 分开，使键盘关闭时光标移动不触发 state 订阅者重组。
     */
    private val cursorRowFlowInternal = MutableStateFlow(Bridge.CURSOR_ROW_UNKNOWN)
    val cursorRowFlow: StateFlow<Int> = cursorRowFlowInternal.asStateFlow()

    /**
     * 活动会话的视口最后一个有内容的行（0 起，[Bridge.LAST_CONTENT_ROW_NONE] = 视口全空）。
     * 仅在变化时由渲染线程发布；输入法跟随位移订阅它以裁剪平移量。
     * 与 [state] 分开，使光标/内容变化不触发 state 订阅者重组。
     */
    private val lastContentRowFlowInternal = MutableStateFlow(Bridge.LAST_CONTENT_ROW_NONE)
    val lastContentRowFlow: StateFlow<Int> = lastContentRowFlowInternal.asStateFlow()

    /**
     * 自愈请求信号：原生 surface 判死并判定需要换新原生窗口时递增（见
     * `maybeRequestSurfaceRecreate`），由持有 `SurfaceView` 的界面读取并换掉整个视图。
     *
     * 原生 surface 判死（其原生窗口的 BufferQueue 被遗弃，实测此后每帧
     * `begin_frame failed`、终端永久黑屏）后，新视图的 `surfaceCreated` 会带来**新的**
     * 原生窗口——同一窗口反复 detach/attach 是唤不活被遗弃的 BufferQueue 的。
     *
     * 用**快照状态**而非帧轮询或回调：帧轮询（`withFrameNanos` 循环）会让 Compose 永远
     * 处于「有挂起帧回调」状态，仪器化用例的 idling 判定随之超时（`ComposeNotIdleException`）；
     * 回调则要多注册/注销一份生命周期。快照状态由运行期持有而非组合捕获，换视图导致的
     * 组合重建也不会让它指向已销毁的状态。
     *
     * 注意消费侧必须真的被重算：Compose 仪器化用例里组合只在 `waitForIdle`/`advanceTimeBy`
     * 期间前进，写入方（如本类）无法替测试推进时钟。
     */
    private val surfaceRecreateSignalState = androidx.compose.runtime.mutableIntStateOf(0)
    val surfaceRecreateSignal: androidx.compose.runtime.IntState get() = surfaceRecreateSignalState

    private val sessions = ConcurrentHashMap<Long, SessionEntry>()

    // ── vsync 帧回调链 ──
    // UI 工作投递到主 Looper（Choreographer.getInstance() 需要 Looper，
    // 而 TerminalRuntime 本身可能由 DI 在任意线程构建）。
    private val mainHandler = android.os.Handler(android.os.Looper.getMainLooper())

    /** 幂等保护：每进程仅一条自调度回调链（CAS 保护；注册失败时重置以允许重试）。 */
    private val vsyncChainStarted = java.util.concurrent.atomic.AtomicBoolean(false)

    /**
     * 唯一的自调度 Choreographer 帧回调：每个显示帧即一次 vsync 信号。
     * 它只置起 [SessionEntry.vsyncRequested] 并经 [SessionEntry.notifyRender] 唤醒渲染线程
     * ——所有渲染均由渲染线程拥有（主线程绝不触碰 surface Mutex）。
     *
     * 无论本帧是否推出画面，链都会在帧末无条件重新投递自身：RenderState 的延迟字段
     * （search_highlights / selection / pending_flash_phase）依赖逐帧消费，链断会使其静默冻结。
     */
    private val vsyncFrameCallback =
        object : android.view.Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                // 无锁读取：sessions 是 ConcurrentHashMap，activeSessionId 是 @Volatile。
                val entry = sessions[activeSessionId]
                // 空闲会话保持驻留：唤醒不得刷新空闲时钟（pokeVsync），且时钟超过阈值即停止，
                // 于是 500ms 空闲闭锁生效，空闲终端不再以显示帧率空耗 CPU/GPU。
                // 链本身保持存活（开销很小）。
                if (
                    entry != null &&
                    (
                        System.nanoTime() - entry.lastSignalNanos <= RENDER_IDLE_THRESHOLD_NANOS ||
                            entry.hasScrollMotion()
                        )
                ) {
                    entry.vsyncRequested = true
                    entry.pokeVsync()
                }
                // 自调度：无论本帧是否产出绘制，都以显示刷新率保持链存活。
                android.view.Choreographer.getInstance().postFrameCallback(this)
            }
        }

    /** 在主 Looper 上启动 vsync 帧回调链。幂等：CAS 保护保证每进程仅一条链。 */
    private fun ensureVsyncChainStarted() {
        if (!vsyncChainStarted.compareAndSet(false, true)) return
        mainHandler.post {
            try {
                android.view.Choreographer.getInstance().postFrameCallback(vsyncFrameCallback)
                LogUtil.d("Runtime", "vsync frame callback chain started")
            } catch (exception: Exception) {
                // 允许后续会话启动时重试注册。
                vsyncChainStarted.set(false)
                LogUtil.e("Runtime", "vsync frame callback registration failed", exception)
            }
        }
    }

    @Volatile var accentColor: Int = 0xFF2196F3.toInt()

    @Volatile var cellWidth: Float = 0f

    @Volatile var cellHeight: Float = 0f

    /**
     * ModifierBar 覆盖层高度（物理像素）。该预留量的唯一所有者：网格计算
     * （recomputeGridFromFontMetrics 与 TerminalSurface.applyGridResize）与输入法跟随滚动
     * 都减去同一值，故行数与滚动永远不会对工具栏遮盖哪些行产生分歧。
     */
    internal val modifierBarHeightPx: Int
        get() = (MODIFIER_BAR_HEIGHT_DP * context.resources.displayMetrics.density + 0.5f).toInt()

    // 最近一次推给原生的字号（十分之一单位）。缩放手势以其为锚点，使预览/确定从实际渲染尺寸
    // 而非原始设置值出发（后者在字号从未显式设置时可能不同）。
    @Volatile internal var appliedFontSizeTenths: Int = 0

    // 逻辑像素单元格尺寸（用于网格行列计算），是未经密度缩放的原生原始值。
    @Volatile var logicalCellWidth: Float = 0f

    @Volatile var logicalCellHeight: Float = 0f

    private val renderGeneration = java.util.concurrent.atomic.AtomicInteger(0)

    @Volatile private var activeSessionId: Long = 0L

    @Volatile private var starting = false
    private val sessionLock = Any()

    /**
     * 应急会话请求（兼容 termux 的应用快捷方式 extra `com.termux.app.failsafe_session`）：
     * 下个会话以系统 shell 启动且不做 prefix 引导，避免引导损坏导致终端不可用。
     * 由 buildConfig 消费一次；已有会话时重复点击为空操作（见 start() 的 `sessions.isNotEmpty()` 守卫）。
     */
    @Volatile
    var failsafeRequested: Boolean = false
        private set

    fun requestFailsafeSession() {
        failsafeRequested = true
        LogUtil.w(
            "Runtime",
            "Failsafe session requested — next start uses /system/bin/sh without prefix",
        )
    }

    /**
     * 串行化 Surface 生命周期转换（暂停/恢复）。[pauseRendering] 与 [resumeRendering] 都跑在这唯一线程上，
     * 以保持 surface-destroy → surface-available 的顺序；分处不同线程可能让新 Surface 没有渲染线程
     * （异步暂停停掉了刚启动的恢复）。
     */
    private val surfaceTransitionExecutor: java.util.concurrent.ExecutorService =
        java.util.concurrent.Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "SurfaceTransition").apply { isDaemon = true }
        }

    @Volatile private var foregroundServiceRunning = false

    @Volatile private var renderMonitorJob: Job? = null

    // 串行化 startRenderMonitor 的「检查后赋值」：start()（IO 协程）与
    // resumeRendering（surfaceTransitionExecutor 线程）都可能到达，
    // 未同步的一对操作会启动两个监视循环
    // （stopRenderMonitor 只取消被引用的那个，另一个会空转到作用域取消为止）。
    private val monitorLock = Any()

    // ══════════════════════════════════════════════════════════════════════
    // 二、渲染线程生命周期
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 应答 OSC 52 剪贴板读取请求：读取系统剪贴板并经
     * [NativeBridge.clipboardResult] 回复。空文本是合法结果；
     * 只有读取失败才回 null——回空串会被远端当成「用户清空了剪贴板」。
     */
    private fun dispatchClipboardRequests(requests: List<terminal.emulator.bridge.Bridge.ClipboardRequest>) {
        requests.forEach { request ->
            clipboardAccess.clipboardText().fold(
                onSuccess = { text ->
                    NativeBridge.clipboardResult(request.sessionId, request.requestId, text)
                },
                onFailure = {
                    // 不作答（而非空串）：远端请求悬空好过粘出空白。
                    // 失败详情已由 ClipboardAccess 记日志。
                    NativeBridge.clipboardResult(request.sessionId, request.requestId, null)
                },
            )
        }
    }

    /**
     * 把 [Process completed (code X)] - press Enter 提示直接送入 VT 解析器
     * （子进程已消失，PTY 不再承载写入，必须带内更新画面）。
     */
    private fun feedProcessCompletedPrompt(entry: SessionEntry, exitCode: Int?) {
        // 码未知时如实写「exit code unknown」，不留空也不谎报 0。
        val rendered = exitCode?.toString() ?: PROCESS_EXIT_CODE_UNKNOWN_TEXT
        val text = PROCESS_COMPLETED_PROMPT_PREFIX + rendered + PROCESS_COMPLETED_PROMPT_SUFFIX
        try {
            entry.bridge?.feedTerminal(text.encodeToByteArray())
        } catch (exception: Exception) {
            LogUtil.w("Runtime", "feedTerminal failed for [Process completed] prompt", exception)
        }
        entry.notifyRender()
    }

    /**
     * 前台会话的 shell 退出后保持可见并显示 [Process completed] 提示而非立即关闭（termux 对等语义）。
     * 条目以 running=true 留在会话表中，直到用户按 Enter 才关闭。
     * 提示已显示时返回 true（调用方应提前返回且不关闭会话）。
     */
    private fun maybeShowProcessCompletedPrompt(entry: SessionEntry, exitCode: Int?): Boolean {
        if (entry.id != activeSessionId || entry.waitingForProcessCompleted) return false
        // 正常退出（exit 0）直接关闭会话；非零退出与「码未知」都保留现场待回车确认关闭。
        if (exitCode == 0) return false
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return false
            entry.waitingForProcessCompleted = true
            entry.processExitCode = exitCode
        }
        LogUtil.i(
            "Runtime",
            "session ${entry.id} exited with code $exitCode; showing [Process completed] prompt",
        )
        feedProcessCompletedPrompt(entry, exitCode)
        return true
    }

    /**
     * 处理会话退出。
     *
     * [exitCode] 可为 `null`：原生侧 `waitpid` 失败、子进程确已退出但码无从取得。
     * 这类死亡**不能**当作退出码 0 处理——按 0 走会立即静默关闭会话，用户既看不到
     * `[Process completed]` 提示，也无从得知 shell 是怎么没的。码未知一律按「非正常
     * 退出」对待：保留现场、显示退出码未知的提示，等用户回车确认。
     */
    private fun handleSessionExit(
        entry: SessionEntry,
        exitCode: Int?,
        // 原生测得的子进程存活时长（毫秒）。本函数不消费它：
        // `[Process completed]` 提示按 Termux 只含退出码。整条 alive_ms 链路
        // （原生测量 → 事件序列化 → 本形参）当前无人消费，已登记在台账待清理。
        aliveMs: Long,
    ) {
        LogUtil.d("Runtime", "session ${entry.id} alive ${aliveMs}ms before exit")
        // 启动入口失败不得回退：shell 退出即走 [Process completed] 提示，
        // 输出保留显示，由用户确认关闭。
        // 本函数既用于 shell 首次退出（poll.exit 分支），也用于用户在 [Process completed]
        // 提示上按 Enter 后的确认关闭（渲染循环重新派发）；确认关闭时跳过提示路径。
        val confirmedClose = entry.processCompletedConfirmed
        if (!confirmedClose && maybeShowProcessCompletedPrompt(entry, exitCode)) return
        LogUtil.i("Runtime", "session ${entry.id} exited with code $exitCode")
        // 阶段 1（加锁）：捕获可能挂起的线程；实际 join 在下方不持锁执行
        // （最长 THREAD_JOIN_TIMEOUT_MS），使挂起的 GPU 线程不阻塞所有会话操作。
        val hungThreadToJoin: Thread?
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            entry.running = false
            hungThreadToJoin = if (entry.renderThreadPossiblyAlive) entry.hungRenderThread else null
        }

        // 阶段 2（不持锁）：最后机会——此前挂起的线程可能已恢复并退出（GPU 已解阻）。
        // 以新的超时 join 它；仅当它仍然存活时才跳过 bridge 关闭。
        // 防止 join 自身：本函数在渲染线程上运行（poll.exit），
        // 而并发的 join 超时路径可能把本线程记为挂起。join 自身必然超时，
        // 徒然泄漏原生会话——调用方本就即将退出循环。
        val hungExited =
            if (hungThreadToJoin == null || hungThreadToJoin === Thread.currentThread()) {
                true
            } else if (hungThreadToJoin.isAlive) {
                hungThreadToJoin.interrupt()
                hungThreadToJoin.join(THREAD_JOIN_TIMEOUT_MS)
                !hungThreadToJoin.isAlive
            } else {
                true
            }

        // 锁内决定关闭资格，实际的 bridge.close() 放到锁外：Session::drop 会 kill 子进程
        // 并 join reader/wait 线程（可达 ~100ms+），持锁会阻塞所有会话操作（切换/创建/关闭）。
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            // 看门狗线程是每会话一个；在此停止（正常退出路径），
            // 否则每个退出会话都会泄漏一个轮询线程。
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            // 仅当已 join 的线程仍是记录的挂起线程时才清标志（期间无其他路径替换它）。
            if (hungExited && entry.hungRenderThread === hungThreadToJoin) {
                entry.renderThreadPossiblyAlive = false
                entry.hungRenderThread = null
            }
        }
        // 渲染线程仍卡在原生代码里时由同一条守卫跳过关闭（在其下销毁即 UAF）。
        entry.closeBridgeUnlessRenderThreadAlive("shell exit")
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            sessions.remove(entry.id)
            if (entry.id == activeSessionId) {
                // 前台会话刚刚消失；接替会话没有渲染线程（只有活动会话渲染），
                // 其输出/事件永远不会被轮询，终端将看似卡死。此刻立即激活它。
                activeSessionId = sessions.keys.sorted().lastOrNull() ?: 0L
                if (activeSessionId != 0L) {
                    activateReplacementSession(
                        activeSessionId,
                        "handleSessionExit",
                        markRunning = false,
                        withRetry = true,
                        syncGrid = false,
                    )
                }
            }
            updateForegroundSessionCount(sessions.size)
            updateState()
        }
    }

    private fun closeDeadSession(entry: SessionEntry) {
        // 锁内置 closing 标志，与 closeSession 对称：handleSessionExit/closeDeadSession 的
        // 接替分支会为新活动会话调用 startRenderThread，缺少该标志时竞争的
        // closeDeadSession(entry) 可能在接替渲染线程启动期间关闭其 bridge，
        // 留下双重消费全局事件队列的孤儿线程。
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            entry.closing = true
            entry.running = false
        }
        // LogUtil.e 已写入 logcat，不重复输出。
        LogUtil.e("Runtime", "session ${entry.id} exceeded max restart attempts, closing session")
        entry.closeBridgeUnlessRenderThreadAlive("restart limit exceeded")
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            sessions.remove(entry.id)
            // 保持前台服务计数同步，与 handleSessionExit 和 closeSession 对称：
            // 经此路径关闭最后一个会话时必须停掉前台服务并清除其陈旧通知。
            updateForegroundSessionCount(sessions.size)
            if (entry.id == activeSessionId) {
                // 同 handleSessionExit：接替会话需要渲染线程，
                // 否则其输出/事件永远不会被轮询，终端将看似卡死。
                val remaining = sessions.keys.sorted()
                activeSessionId = remaining.lastOrNull() ?: 0L
                if (activeSessionId != 0L) {
                    activateReplacementSession(
                        activeSessionId,
                        "closeDeadSession",
                        markRunning = false,
                        withRetry = true,
                        syncGrid = false,
                    )
                }
            }
        } // synchronized(sessionLock)
        updateState()
    }

    private fun startForegroundServiceIfNeeded() {
        if (!foregroundServiceRunning) {
            terminal.emulator.service.TerminalForegroundService.start(context)
            foregroundServiceRunning = true
            LogUtil.d("Runtime", "foreground service started")
        }
    }

    private fun stopForegroundService() {
        // 不以标志位为闸门：MainActivity.onCreate 经静态 TerminalForegroundService.start()
        // 直接启动服务而不设置该标志，若以标志位控制停止，当 Activity 在运行期引导完成前
        // 销毁时会泄漏服务及其 PARTIAL_WAKE_LOCK。对未运行的服务调 stopService
        // 是无害空操作（返回 false），故无条件停止并重置标志。
        val stopped =
            try {
                terminal.emulator.service.TerminalForegroundService.stop(context)
            } catch (serviceException: Exception) {
                if (serviceException is kotlinx.coroutines.CancellationException) {
                    throw serviceException
                }
                // stopService 的 binder 失败（罕见）不得让标志残留为真：陈旧标志会使
                // 下次 startForegroundServiceIfNeeded 跳过启动，存活会话因此没有通知也没有唤醒锁。
                // 无论如何都重置标志——服务要么已停止，要么即将被系统杀死。
                LogUtil.e("Runtime", "Failed to stop foreground service", serviceException)
                false
            }
        foregroundServiceRunning = false
        if (stopped) {
            LogUtil.d("Runtime", "foreground service stopped")
        }
    }

    /**
     * 更新前台服务的会话数，并保持 [foregroundServiceRunning] 标志同步：计数降到 0 时服务自行停止，
     * 故此处必须清除标志，否则后续 startForegroundServiceIfNeeded 会跳过重启服务
     * ——无前台通知、无唤醒锁，后台会话可能被杀。
     *
     * 异常安全：在关闭路径上于 sessionLock 内调用，服务异常绝不能逃出锁
     * （否则会跳过 updateState 并破坏会话簿记）。
     */
    private fun updateForegroundSessionCount(count: Int) {
        if (count <= 0) {
            foregroundServiceRunning = false
        }
        try {
            terminal.emulator.service.TerminalForegroundService.updateSessionCount(context, count)
        } catch (exception: Exception) {
            if (exception is kotlinx.coroutines.CancellationException) {
                throw exception
            }
            LogUtil.e("Runtime", "updateSessionCount failed", exception)
        }
    }

    private data class SelectionStateSnapshot(
        val startRow: Int,
        val startCol: Int,
        val endRow: Int,
        val endCol: Int,
        val hasSelection: Boolean,
        // 选区手柄拖拽进行中（UI 线程，经 setSelectionDragging 置位）。渲染线程将其并入
        // hasSelectionOrDrag 参与滚动复位决策；不会转发给原生 setSelection
        // ——拖拽只影响滚动语义，绝不影响绘制出的选区范围。
        val dragging: Boolean = false,
    )

    private val selectionState =
        java.util.concurrent.atomic.AtomicReference(
            SelectionStateSnapshot(0, 0, 0, 0, false),
        )

    /** 活动会话的滚动偏移：会话切换时由 Surface 读取以重同步其本地选区计算偏移。 */
    fun activeSessionScrollOffset(): Int {
        synchronized(sessionLock) {
            return sessions[activeSessionId]?.scrollOffset ?: 0
        }
    }

    fun setScrollOffset(offset: Int) {
        val entry = sessions[activeSessionId] ?: return
        entry.scrollOffset = offset
        // 记录手势时间，使渲染线程的 recentlyScrolled 保护能在用户滚动后的
        // RECENT_SCROLL_WINDOW_NANOS 内抑制新输出引起的滚动复位。
        entry.lastScrollNanos = System.nanoTime()
        // 渲染线程已读取 entry.scrollOffset 并在 surface 锁下调 bridge.setScrollOffset()，
        // 在此调用只会给调用线程（滚动时通常是 UI 线程）多一次 JNI 往返与 surface 锁获取。
        // 只信号渲染线程去取该变更即可。
        entry.notifyRender()
    }

    /**
     * 从手势层同步逐像素滚动余量，交由渲染线程转发给原生
     * （同 [setScrollOffset]：只存储 + 通知，由渲染线程在 surface 锁下完成 JNI 穿越）。
     */
    fun setScrollRemainderPx(px: Float) {
        val entry = sessions[activeSessionId] ?: return
        entry.scrollRemainderPx = px
        // 此处也盖上滚动运动时钟（不只 setScrollOffset 中盖）：逐像素的亚行运动是
        // 拖拽期间频率最高的事件，必须保持循环的活跃节奏。
        entry.lastScrollNanos = System.nanoTime()
        entry.notifyRender()
    }

    /** 从 TerminalViewModel 同步滚动激活状态，使渲染线程知道新输出时是否自动复位滚动。 */
    fun setScrollActive(active: Boolean) {
        val entry = sessions[activeSessionId] ?: return
        entry.scrollActive = active
    }

    /**
     * 在 UI 线程标记选区手柄拖拽的开始/结束，经既有 selectionState 通道并入
     * SelectionStateSnapshot.dragging。渲染线程在滚动复位决策中视拖拽等同选区激活
     * （termux skipScrolling 对等语义）。endSelection/clearSelection 经 setSelection
     * 以 dragging=false 覆写快照，故无需专门的清除路径。
     */
    fun setSelectionDragging(dragging: Boolean) {
        val current = selectionState.get()
        if (current.dragging == dragging) return
        selectionState.set(current.copy(dragging = dragging))
        sessions[activeSessionId]?.notifyRender()
    }

    fun forceRender() {
        val entry = sessions[activeSessionId] ?: return
        entry.forceRenderRequested = true
        entry.notifyRender()
    }

    /**
     * 原生 surface 判死后按间隔请求宿主换新的原生窗口（[surfaceRecreateSignal] 递增）。
     *
     * 由渲染线程在每帧读到失效位时调用。失效位在重建成功后由原生回落为 0，
     * 故请求天然边沿触发；间隔限流与次数上限由 [decideSurfaceRecreate] 裁决，
     * 避免重建无望时反复拆装视图（抖动与耗电）。
     */
    private fun maybeRequestSurfaceRecreate(entry: SessionEntry) {
        val now = System.nanoTime()
        val budgetRef = entry.surfaceRecreateBudget
        while (true) {
            // 每次重读：CAS 若因并发复位失败，重判必须用最新预算，
            // 否则会把复位后的新预算又推回耗尽（R25-T1）。
            val budget = budgetRef.get()
            val decision = decideSurfaceRecreate(budget.attempts, budget.lastRequestNanos, now)
            if (decision.exhausted) {
                if (!budget.exhaustedLogged &&
                    budgetRef.compareAndSet(budget, budget.copy(exhaustedLogged = true))
                ) {
                    LogUtil.e(
                        TAG,
                        "surface invalidated and $SURFACE_RECREATE_MAX_ATTEMPTS recreate attempts exhausted; " +
                            "terminal will stay blank until the surface is recreated by the platform",
                    )
                }
                return
            }
            if (!decision.request) return
            if (budgetRef.compareAndSet(
                    budget,
                    budget.copy(attempts = budget.attempts + 1, lastRequestNanos = now),
                )
            ) {
                // 快照状态在主线程写：组合的重算调度与主线程一致，跨线程写只会让换视图延迟到
                // 下一次读，且无法保证与界面重建同帧完成。
                mainHandler.post {
                    surfaceRecreateSignalState.intValue += 1
                    // 走 LogUtil 而非裸 android.util.Log：仪器化失败时 TerminalLogcatRule
                    // 只按终端相关标签过滤，独立标签的锚点会被整条丢掉。
                    LogUtil.w(TAG, "surface recreate signal -> ${surfaceRecreateSignalState.intValue}")
                }
                LogUtil.w(
                    TAG,
                    // 报刚才写下的那次计数，而不是重新读引用——并发复位会让日志
                    // 印出一个从未请求过的次数。
                    "surface invalidated (attempt ${budget.attempts + 1}/$SURFACE_RECREATE_MAX_ATTEMPTS): " +
                        "requesting a fresh Android surface",
                )
                return
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 三、会话生命周期
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 写入带 termux 对等提示符的 mksh rc 文件（自愈式）。
     *
     * 根因：无 rc 文件时交互式 mksh 回退到 AOSP `/system/etc/mkshrc` 的
     * 38 列宽提示符 `:/data/.../home $ `。任何长于剩余 ~10 列的命令都会触发 mksh 的
     * 横向滚动重绘（`\r` + 滚动窗口 + `<` 标记 + 退格串），表现为用户报告的乱码回显
     * （"cho …" 碎片与右缘杂散 `<`）。真 Termux 用简短的 `PS1='$ '` 彻底避开。
     *
     * 文件位于应用数据目录而非 `$HOME`：该路径作为 [TerminalConfig.mkshrcPath] 交给原生
     * 并注入为 `$ENV`，交互式 mksh 正是经此读取。
     *
     * mksh 在设置 `$ENV` 时读它，否则交互式读 `~/.mkshrc`；我们先 source 系统 rc
     * （保留其 PATH/别名设置）再覆盖 PS1。引导中的 bash 从不读此文件。
     * 文件不含内容标记时覆写，使陈旧安装自愈。
     */
    private fun ensureMkshPromptRc() {
        val mkshRcFile = java.io.File(context.applicationInfo.dataDir, MKSHRC_FILENAME)
        // 新内容独有前缀：含 OSC 7 发射的旧 rc 不匹配，覆盖自愈。
        val contentMarker = "# terminal: termux-parity prompt (see"
        if (mkshRcFile.isFile) {
            try {
                if (mkshRcFile.readText().contains(contentMarker)) return
            } catch (_: Exception) {
                // 不可读——下方覆写
            }
        }
        try {
            mkshRcFile.parentFile?.mkdirs()
            mkshRcFile.writeText(
                "# terminal: termux-parity prompt (see TerminalRuntime.ensureMkshPromptRc)\n" +
                    ". /system/etc/mkshrc\n" +
                    "PS1='\$ '\n",
            )
        } catch (exception: Exception) {
            LogUtil.w("Runtime", "Failed to write $MKSHRC_FILENAME: $exception")
        }
    }

    /** Absolute path of the mksh rc file written by [ensureMkshPromptRc]. */
    private val mkshrcPath: String
        get() = java.io.File(context.applicationInfo.dataDir, MKSHRC_FILENAME).absolutePath

    private suspend fun buildConfig(rows: Int = DEFAULT_GRID_ROWS, cols: Int = DEFAULT_GRID_COLS): TerminalConfig {
        val configReads = coroutineScope {
            val shellDeferred = async { settingsRepository.shell.first() }
            val fontDeferred = async { computeFontSizeTenths() }
            val themeDeferred = async { resolveThemeName() }
            ConfigReads(
                shellPath = shellDeferred.await(),
                fontSizeTenths = fontDeferred.await(),
                themeName = themeDeferred.await(),
            )
        }
        val resolvedTheme = BuiltInThemes.byName(configReads.themeName)
        val shell = resolveShell(configReads.shellPath)
        val bridgeTheme = makeBridgeTheme(resolvedTheme)
        accentColor = bridgeTheme.ansi5
        val prefixDir = java.io.File(context.filesDir, "usr").absolutePath
        val homeDir =
            java.io
                .File(context.filesDir, "home")
                .apply {
                    if (!exists() && !mkdirs()) {
                        LogUtil.w("Runtime", "Failed to create home directory: $this")
                    }
                }
                .absolutePath
        // Failsafe（termux 应用快捷方式 “New session (Failsafe)”）：完全绕开引导程序——
        // 系统 shell、系统 PATH、无 PREFIX——避免引导程序损坏后终端彻底不可用
        // （对应 termux-app TermuxSession.java:95-113 的 isFailsafe 路径）。
        if (failsafeRequested) {
            failsafeRequested = false
            ensureMkshPromptRc()
            return TerminalConfig(
                shell = Shell.SystemDefault,
                rows = rows,
                cols = cols,
                fontSizeTenths = configReads.fontSizeTenths,
                theme = bridgeTheme,
                home = homeDir,
                workingDirectory = homeDir,
                prefix = "",
                mkshrcPath = mkshrcPath,
            )
        }
        // 默认入口依次探测 bash 与 login（DESIGN :188 文件存在即启动，不检查权限）。
        val prefixShell = findPrefixShell(prefixDir)
        LogUtil.d(
            "Runtime",
            "prefixShell=$prefixShell prefixDir=$prefixDir",
        )
        val effectivePrefix = if (prefixShell != null) prefixDir else ""
        val effectiveShell = resolveEffectiveShell(prefixDir, prefixShell, shell)
        ensureMkshPromptRc()
        // 无启动目录设置（DESIGN :126）：工作目录恒为家目录。
        return TerminalConfig(
            shell = effectiveShell,
            rows = rows,
            cols = cols,
            fontSizeTenths = configReads.fontSizeTenths,
            theme = bridgeTheme,
            home = homeDir,
            workingDirectory = homeDir,
            prefix = effectivePrefix,
            mkshrcPath = mkshrcPath,
        )
    }

    // ══════════════════════════════════════════════════════════════════════
    // 三之二、渲染线程监管
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 拥有渲染线程生命周期：监视循环、死线程检测、重启/退避与线程启停。
     *
     * 内部类：直接访问 TerminalRuntime 的会话注册表与锁，无需经构造函数层层传递
     * （纯代码搬迁，行为零变化）。
     */
    inner class RenderSupervisor {
        internal fun startRenderMonitor() {
            synchronized(monitorLock) {
                if (renderMonitorJob?.isActive == true) return
                renderMonitorJob = scope.launch {
                    while (isActive) {
                        delay(RENDER_MONITOR_INTERVAL_MS)
                        checkSessions()
                    }
                }
            }
        }

        internal fun stopRenderMonitor() {
            synchronized(monitorLock) {
                renderMonitorJob?.cancel()
                renderMonitorJob = null
            }
        }

        internal fun checkSessions() {
            val deadSessions = mutableListOf<SessionEntry>()
            val healthySessions = mutableListOf<SessionEntry>()
            synchronized(sessionLock) {
                scanSessionsForDeath(deadSessions, healthySessions)
            }
            decayRestartCounts(healthySessions)
            for (entry in deadSessions) {
                scope.launch {
                    handleDeadRenderThread(entry)
                }
            }
        }

        internal fun scanSessionsForDeath(
            deadSessions: MutableList<SessionEntry>,
            healthySessions: MutableList<SessionEntry>,
        ) {
            for (entry in sessions.values) {
                if (!entry.running) continue
                if (entry.bridge == null) continue
                if (entry.renderThreadExited) {
                    deadSessions.add(entry)
                } else {
                    val thread = entry.renderThreadRef
                    if (thread != null && !thread.isAlive) {
                        deadSessions.add(entry)
                    }
                    if (entry.restartAttempts > 0) {
                        healthySessions.add(entry)
                    }
                }
            }
        }

        internal fun decayRestartCounts(healthySessions: MutableList<SessionEntry>) {
            for (entry in healthySessions) {
                synchronized(sessionLock) {
                    if (!entry.running || entry.renderThreadExited) continue
                    if (entry.restartAttempts > 0) {
                        entry.restartAttempts = 0
                        entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                    }
                }
            }
        }

        internal suspend fun handleDeadRenderThread(entry: SessionEntry) {
            LogUtil.w(
                "Runtime",
                "session ${entry.id} render thread exited, restart attempt ${entry.restartAttempts + 1}",
            )

            // 阶段 1（加锁）：仅做快速状态检查。阶段 2（不持锁）：
            // stopDeadRenderThreadResources 会 join 旧线程（最长 1s），
            // 跨 join 持有 sessionLock 会阻塞所有会话操作。
            synchronized(sessionLock) {
                if (!entry.running) return
                if (!entry.renderThreadExited) {
                    val thread = entry.renderThreadRef
                    if (thread != null && thread.isAlive) return
                    entry.renderThreadExited = true
                }
                if (entry.bridge == null) {
                    entry.running = false
                    entry.renderThreadExited = false
                    return
                }
            }

            // 阶段 2（不持锁）：下方 join 在渲染线程卡死于原生 GPU 代码时
            // 最长阻塞 THREAD_JOIN_TIMEOUT_MS——这正是若持有 sessionLock
            // 就会拖住所有会话操作的情形，故刻意置于锁外。
            stopDeadRenderThreadResources(entry)

            synchronized(sessionLock) {
                // 上方 join 后 running 仍为 true（它是会话意图标志，只有 pause/stop 置假）。
                // 在此重查，使在阶段 1 与此刻之间运行的并发 pauseRendering()
                // 能取消重启，而非在已销毁的 Surface 上启动渲染线程。
                if (!entry.running) return
                if (entry.bridge == null || !sessions.containsKey(entry.id)) return
                if (entry.restartScheduled) return
                entry.restartScheduled = true
                entry.restartAttempts++
            }
            // closeDeadSession 在锁外运行：它会关闭 bridge（Session::drop 会 kill
            // 并 join 线程，~100ms+）并可能启动接替渲染线程（join 最长
            // THREAD_JOIN_TIMEOUT_MS），两者绝不可在持有 sessionLock 时运行。
            // 尝试计数单调递增且 closeDeadSession 会重查 sessions.containsKey，
            // 故并发竞争调用无害（第二次调用立即返回）。
            if (shouldCloseDeadRender(entry.restartAttempts, RENDER_MAX_RESTART_ATTEMPTS)) {
                closeDeadSession(entry)
                return
            }
            val restartDelayMillis =
                synchronized(sessionLock) {
                    val next = entry.nextRestartDelayMs
                    entry.nextRestartDelayMs =
                        nextRestartDelayMs(entry.nextRestartDelayMs, MAX_RESTART_DELAY_MS)
                    next
                }

            delay(restartDelayMillis)
            restartRenderThreadAfterDelay(entry)
            delay(GRACE_PERIOD_AFTER_RESTART_MS)
            confirmRestartGrace(entry)
        }

        internal fun stopDeadRenderThreadResources(entry: SessionEntry) {
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            // 注意：此处绝不要把 entry.running 置假。running 是会话意图标志
            // （pause/stop 置假，start/resume 置真）。死线程重启路径依赖 running 保持为真，
            // 使延迟重启（restartRenderThreadAfterDelay）仍能执行；
            // 也使并发的 pauseRendering()（将 running 置假）能正确取消待执行的重启。
            entry.renderThreadRef?.let { renderThread ->
                renderThread.interrupt()
                renderThread.join(THREAD_JOIN_TIMEOUT_MS)
                if (renderThread.isAlive) {
                    LogUtil.w("Runtime", "Render thread did not exit within timeout, continuing anyway")
                    entry.renderThreadPossiblyAlive = true
                    entry.hungRenderThread = renderThread
                    return
                }
            }
            entry.renderThreadRef = null
            entry.renderSignaled.set(false)
            // renderThreadRef 的 join 成功，并不能证明先前 join 超时时记录的挂起线程也已死亡
            // ——它可能仍在原生渲染代码中。仅在未记录挂起线程时才清标志，
            // 或先给挂起线程最后一次 join；无条件清除会让关闭路径在存活线程下
            // 销毁原生会话（use-after-free）。
            val hung = entry.hungRenderThread
            if (entry.renderThreadPossiblyAlive && hung != null && hung.isAlive) {
                hung.interrupt()
                hung.join(THREAD_JOIN_TIMEOUT_MS)
            }
            if (entry.renderThreadPossiblyAlive && hung != null && !hung.isAlive) {
                entry.renderThreadPossiblyAlive = false
                entry.hungRenderThread = null
            }
            // 此处跳过 releaseGpuSurface——新渲染线程会经 attachSurface 重新配置 Surface。
        }

        internal suspend fun restartRenderThreadAfterDelay(entry: SessionEntry) {
            synchronized(sessionLock) {
                // 先消费调度标记：无论本次重启执行还是被取消（暂停/关闭），
                // 后续监视器派发都必须能调度一次全新的重启。
                entry.restartScheduled = false
                if (!sessions.containsKey(entry.id)) return
                // 仅当会话仍意图运行时才重启。已暂停的会话（Surface 已销毁）
                // 不应获得渲染线程——恢复路径会在 Surface 回来时启动它。
                if (!entry.running) return
                // 其他线程可能已启动（例如恢复与延迟重启竞争）；不要启动第二个。
                if (entry.renderThreadRef?.isAlive == true) return
                if (entry.bridge == null) return
                entry.renderThreadExited = false
                startRenderThread(entry)
                LogUtil.d(
                    "Runtime",
                    "session ${entry.id} render thread restarted (attempt ${entry.restartAttempts})",
                )
            }
        }

        internal suspend fun confirmRestartGrace(entry: SessionEntry) {
            synchronized(sessionLock) {
                if (!sessions.containsKey(entry.id)) return
                if (!entry.running) return
                if (entry.renderThreadExited) return
                val thread = entry.renderThreadRef
                if (thread != null && thread.isAlive) {
                    entry.restartAttempts = 0
                    entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                    LogUtil.d("Runtime", "session ${entry.id} render thread healthy after restart")
                }
            }
        }

        internal fun startRenderThread(entry: SessionEntry) {
            // 拒绝为正在关闭的会话启动线程：closeSession 在启动不持锁的
            // 停止/关闭序列之前会在 sessionLock 下置 entry.closing，
            // 且本函数的每个调用方都持有 sessionLock
            // ——因此正在关闭的条目绝不可能获得新渲染线程（孤儿线程 TOCTOU，见 closing 字段说明）。
            // 同时重置运行意图标志：调用方（resumeRendering/switchSession/closeSession 阶段 3）
            // 在调用前已将其置真，若保留为真会使 closeSession 的锁内重查
            // 把该条目误判为运行中而跳过 bridge.close（泄漏原生会话与子进程）。
            if (entry.closing) {
                entry.running = false
                LogUtil.d("Runtime", "session ${entry.id} closing — refusing to start render thread")
                return
            }
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            entry.renderThreadExited = false
            entry.running = false
            entry.renderSignaled.set(false)
            val oldThread = entry.renderThreadRef
            entry.renderThreadRef = null
            oldThread?.let { t ->
                if (t === Thread.currentThread()) {
                    // join 自身必然超时并错误地把当前线程标记为挂起；跳过
                    // （调用方就是渲染线程本身，如 handleSessionExit 的接替）。
                    LogUtil.w(
                        "Runtime",
                        "session ${entry.id} startRenderThread called from its own render thread — skipping join",
                    )
                } else {
                    t.interrupt()
                    t.join(THREAD_JOIN_TIMEOUT_MS)
                    if (t.isAlive) {
                        LogUtil.w(
                            "Runtime",
                            "session ${entry.id} previous render thread still alive after join — forcing new thread anyway",
                        )
                        // 旧线程可能仍在原生渲染代码中。记录之，
                        // 使关闭路径跳过 releaseGpuSurface/close，
                        // 并保留引用以便退出时做最后一次 join。
                        entry.renderThreadPossiblyAlive = true
                        entry.hungRenderThread = t
                    } else {
                        // 仅当已 join 的线程就是记录的挂起线程（或未记录）时才清标志。
                        // 早先 GPU 停滞留下的另一个挂起线程可能仍在原生代码中存活；
                        // 在此清除会让关闭路径在其下销毁会话（use-after-free）。
                        if (entry.hungRenderThread == null || entry.hungRenderThread === t) {
                            entry.renderThreadPossiblyAlive = false
                            entry.hungRenderThread = null
                        }
                    }
                }
            }
            val generation = renderGeneration.incrementAndGet()
            entry.running = true
            val renderThread =
                Thread(
                    {
                        try {
                            runBlocking {
                                // 显示优先级的渲染线程：输入法打开或 Surface 频繁变动时，
                                // 帧管线会与 UI 线程争抢 CPU。THREAD_PRIORITY_DISPLAY 让帧生产
                                // 优先于 UI 线程时效性较低的工作，在真机与 SwiftShader 模拟器上
                                // 都可降低帧时间抖动。
                                Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY)
                                var diagCount = 0
                                var consecutiveErrors = 0
                                var lastScrollOffset = Int.MAX_VALUE
                                var lastScrollRemainderPx = Float.NaN
                                var lastSelection = SelectionStateSnapshot(0, 0, 0, 0, false)
                                // 每线程帧时长统计：渲染线程重启时重置（新生涯，不带陈旧历史）。
                                val frameTiming = FrameTimingStats()
                                // 整循环周期统计（渲染 + pollAll + 事件派发 + waitOutput）：
                                // 上方 frameTiming 只覆盖 bridge.render()；
                                // 循环窗口的倒数才是真实帧率，且能区分「原生渲染慢」
                                // 与「循环中其他环节慢」。
                                val loopTiming = FrameTimingStats()
                                // 基线自适应降级检测器：学习设备自身的帧时间基线，
                                // 对回归至其约 3 倍以上的窗口告警——同一套机制
                                // 既适用于软件模拟器（~555ms/帧）也适用于真机（~17ms），
                                // 无需固定阈值。
                                val frameTimingTrend = FrameTimingTrend()
                                LogUtil.d(
                                    "Runtime",
                                    "render thread started for session ${entry.id} generation=$generation",
                                )
                                while (entry.running && renderGeneration.get() == generation) {
                                    // 用户在 [Process completed] 提示上按了 Enter
                                    // ——writeToPty 只发信号；关闭路径在此渲染线程上执行，
                                    // 避免在存活的渲染循环下销毁 bridge。
                                    if (entry.processCompletedConfirmed) {
                                        handleSessionExit(entry, entry.processExitCode, 0L)
                                        break
                                    }
                                    try {
                                        val loopFrameStart = System.nanoTime()
                                        val bridge = entry.bridge ?: break
                                        // ── vsync 唤醒门控 ──
                                        // 循环仅在有唤醒源触发时才渲染：
                                        //   ① vsyncRequested — Choreographer 帧回调
                                        //     （显示帧信号；该回调只发信号，绝不触碰 surface
                                        //     Mutex，也绝不自行调用 render），
                                        //   ② forceRenderRequested — 即时反馈旁路
                                        //     （forceRender()，语义不变），
                                        //   ③ renderSignaled — 任意 notifyRender() 生产者
                                        //     （滚动/选区/PTY 相邻的 UI 信号）。
                                        // 否则驻留在输出闭锁上。闭锁超时（活跃 16ms / 空闲 500ms）
                                        // 同时充当安全网节奏：超时返回仍会落下一次渲染尝试，
                                        // 使主线程阻塞或 vsync 信号丢失时延迟字段的消费不会停摆。
                                        // 关于 PTY 输出：waitOutput 是纯 park——PTY 到达无法提前唤醒它；
                                        // 新输出在下一次 vsync 尝试（~16.7ms）或本超时回退时
                                        // 被取走，此时 receive_cell_data 消费待处理数据。
                                        // 唤醒后 render() 无条件调用——空闲门控决策完全位于原生门控内部。
                                        if (
                                            !entry.vsyncRequested &&
                                            !entry.forceRenderRequested &&
                                            !entry.renderSignaled.get()
                                        ) {
                                            val timeoutNanos: Long =
                                                if (
                                                    shouldUseIdleLatch(
                                                        idleNanos =
                                                        System.nanoTime() -
                                                            entry.lastSignalNanos,
                                                        hasScrollMotion =
                                                        entry.hasScrollMotion(),
                                                        idleThresholdNanos =
                                                        RENDER_IDLE_THRESHOLD_NANOS,
                                                    )
                                                ) {
                                                    RENDER_LATCH_IDLE_TIMEOUT_NANOS
                                                } else {
                                                    RENDER_LATCH_TIMEOUT_NANOS
                                                }
                                            bridge.waitOutput(timeoutNanos / 1_000_000L)
                                            if (Thread.interrupted()) throw InterruptedException()
                                        }
                                        // 渲染前消费唤醒标志。三者都无条件清除：
                                        // 落在门控检查与本次清除之间的信号会并入下方的渲染；
                                        // 落在渲染期间的信号则等下一次 vsync/超时节拍
                                        // ——最多多一帧延迟，绝不会丢唤醒。
                                        // 在此（而非在驻留分支内）清除 renderSignaled 修掉了忙等泄漏：
                                        // 当 vsyncRequested 赢得门控检查时，renderSignaled 会永远为真，
                                        // 之后每次迭代都跳过驻留，
                                        // 线程在两次渲染之间全速空转空耗 CPU。
                                        entry.vsyncRequested = false
                                        entry.forceRenderRequested = false
                                        entry.renderSignaled.set(false)
                                        val selectionSnapshot = selectionState.get()
                                        if (selectionSnapshot != lastSelection) {
                                            bridge.setSelection(
                                                selectionSnapshot.startRow,
                                                selectionSnapshot.startCol,
                                                selectionSnapshot.endRow,
                                                selectionSnapshot.endCol,
                                                selectionSnapshot.hasSelection,
                                            )
                                            lastSelection = selectionSnapshot
                                        }
                                        val currentScrollOffset = entry.scrollOffset
                                        if (currentScrollOffset != lastScrollOffset) {
                                            bridge.setScrollOffset(currentScrollOffset)
                                            lastScrollOffset = currentScrollOffset
                                        }
                                        val currentRemainderPx = entry.scrollRemainderPx
                                        if (currentRemainderPx != lastScrollRemainderPx) {
                                            bridge.setScrollYPx(currentRemainderPx)
                                            lastScrollRemainderPx = currentRemainderPx
                                        }
                                        entry.frameMarks = entry.frameMarks.copy(startNanos = System.nanoTime())
                                        // 渲染与 new_output 消费合并为单次 JNI 穿越（每帧省 ~0.1-0.3ms）。
                                        // 解构超过 3 项被 detekt 禁止，故取对象再逐字段读。
                                        val renderResult = bridge.renderWithNewOutput()
                                        val count = renderResult.count
                                        val newOutput = renderResult.newOutput
                                        val cursorRow = renderResult.cursorRow
                                        if (cursorRow != entry.cursorRow) {
                                            entry.cursorRow = cursorRow
                                            if (entry.id == activeSessionId) {
                                                cursorRowFlowInternal.value = cursorRow
                                            }
                                        }
                                        val lastContentRow = renderResult.lastContentRow
                                        if (lastContentRow != entry.lastContentRow) {
                                            entry.lastContentRow = lastContentRow
                                            if (entry.id == activeSessionId) {
                                                lastContentRowFlowInternal.value = lastContentRow
                                            }
                                        }
                                        if (renderResult.surfaceInvalidated) {
                                            entry.surfaceInvalidated = true
                                            maybeRequestSurfaceRecreate(entry)
                                        } else if (entry.surfaceInvalidated) {
                                            // 恢复即清零预算：下一次判死仍从满额开始。
                                            entry.surfaceInvalidated = false
                                            entry.surfaceRecreateBudget.set(SessionEntry.SurfaceRecreateBudget())
                                        }
                                        val frameMs = (System.nanoTime() - entry.frameMarks.startNanos) / 1_000_000.0
                                        if (frameMs > SLOW_FRAME_LOG_THRESHOLD_MS) {
                                            LogUtil.w(
                                                "Runtime",
                                                "SLOW_FRAME session=${entry.id} render=$frameMs count=$count newOutput=$newOutput scrollOffset=$currentScrollOffset",
                                            )
                                        }
                                        if (newOutput) {
                                            // 延迟探针的回显配对：本帧消费了 PTY 输出；
                                            // 若有待配对的输入打点，就落下一个输入→回显样本。
                                            entry.latencyProbe
                                                .onEchoFrame(
                                                    SystemClock.elapsedRealtimeNanos(),
                                                )
                                                ?.let { latencyNanos ->
                                                    if (BuildConfig.DEBUG) {
                                                        LogUtil.d(
                                                            "Runtime",
                                                            "latency session=${entry.id} echo=${latencyNanos / 1_000_000.0}ms",
                                                        )
                                                    }
                                                    // 周期性 p50/p95 汇总写入 logcat
                                                    // （LATENCY_REPORT 标记便于 grep，
                                                    // 供离线采集分位数）。
                                                    val sampleCount = entry.latencyProbe.sampleCount
                                                    if (sampleCount % LATENCY_REPORT_EVERY == 0) {
                                                        LogUtil.i(
                                                            "Runtime",
                                                            "LATENCY_REPORT session=${entry.id} ${entry.latencyProbe.report()}",
                                                        )
                                                    }
                                                }
                                        }
                                        if (newOutput) {
                                            // 只有真实的 PTY 摄入才刷新空闲时钟：count 同样统计静态网格的
                                            // 空闲重绘，会使 lastSignalNanos 永远处于空闲阈值内，
                                            // 500ms 空闲闭锁因而永不生效。持续输出流
                                            // （tail -f、ping、gradle）总是携带 newOutput，故保持活跃。
                                            entry.lastSignalNanos = System.nanoTime()
                                            // 滚动语义（termux onScreenUpdated 对等）：消费原生 new_output
                                            // 标志（PTY 摄入的旁路标志，而非同样统计空闲重绘的
                                            // render() 计数），且仅在无选区/拖拽、SCROLL 锁关闭、
                                            // 且 RECENT_SCROLL_WINDOW_NANOS 内无滚动手势时才把视口复位到底部。
                                            // 被跳过的复位就此丢弃（「跳过 = 放弃」）：标志已是读后即清，
                                            // 下次输出到达时会再次复位（termux：持续输出总是胜出）。
                                            if (
                                                entry.scrollOffset != 0 &&
                                                shouldResetScroll(
                                                    scrollActive = entry.scrollActive,
                                                    hasSelectionOrDrag =
                                                    selectionSnapshot.hasSelection ||
                                                        selectionSnapshot.dragging,
                                                    newOutput = newOutput,
                                                    recentlyScrolled =
                                                    System.nanoTime() - entry.lastScrollNanos <
                                                        RECENT_SCROLL_WINDOW_NANOS,
                                                )
                                            ) {
                                                // 渲染线程上的单点复位写入；lastScrollOffset 保持不动，
                                                // 使既有的差量推送在下一帧把偏移 0 转发给原生。
                                                entry.scrollOffset = 0
                                                // 搜索高亮按 Compose 镜像换算绘制：此处同步递增复位计数，
                                                // 使订阅方重读真实偏移，否则高亮按旧偏移错位。
                                                _state.update { current ->
                                                    current.copy(
                                                        scrollResetEpoch = current.scrollResetEpoch + 1,
                                                    )
                                                }
                                            }
                                        }
                                        if (count < 0) {
                                            // 瞬时渲染错误（Surface 未就绪、快照不可用等），
                                            // 会自行恢复，不计入致命上限。
                                            // 绝不在此 break：本线程同时驱动 pollAll 泵送，
                                            // 退出会冻结原生输出处理且不保证重启
                                            // （旋转/切应用的 Surface 中断会让终端永久卡死）。
                                            // 正常退出仍走上方 entry.running / generation 条件。
                                            if (consecutiveErrors == 0 ||
                                                consecutiveErrors % RENDER_MAX_TRANSIENT_ERRORS == 0
                                            ) {
                                                LogUtil.w(
                                                    "Runtime",
                                                    "session ${entry.id} transient render error code=$count (consecutive=$consecutiveErrors, surviving)",
                                                )
                                            }
                                            consecutiveErrors++
                                            // 帧已正常返回（只是渲染失败），渲染线程没有挂起。
                                            // 必须刷新完成时刻：`RenderWatchDog` 以
                                            // 「开始 > 完成」判定挂起，若此处不更新，
                                            // 持续失败（Surface 迟迟不就绪）会让完成时刻
                                            // 冻结在最后一帧成功处，10s 后看门狗把仍在循环的
                                            // 线程判为挂死，反复重启直至 `closeDeadSession`
                                            // 关掉用户的 shell。
                                            entry.frameMarks = entry.frameMarks.copy(doneNanos = System.nanoTime())
                                            // 自适应退避：前 10 次 50ms，之后 200ms
                                            val sleepMs =
                                                if (consecutiveErrors > 10) {
                                                    RENDER_ERROR_BACKOFF_MS
                                                } else {
                                                    RENDER_ERROR_SLEEP_MS
                                                }
                                            delay(sleepMs)
                                        } else {
                                            if (consecutiveErrors > 0) {
                                                LogUtil.i(
                                                    "Runtime",
                                                    "session ${entry.id} recovered after $consecutiveErrors errors",
                                                )
                                            }
                                            consecutiveErrors = 0
                                        }
                                        // 输出泵与渲染解耦：无论渲染成功与否都必须排空原生输出，
                                        // 否则一次持续渲染失败会阻塞读取线程并冻结全部会话。
                                        try {
                                            val poll = bridge.pollAll()
                                            // 退出优先在独立分支中处理：该事件已从原生队列消费且无法重放，
                                            // 故下方剪贴板处理中的异常绝不能跳过清理。
                                            if (poll.exit) {
                                                // 先回复空值，再做任何清理：这些剪贴板读取请求
                                                // 已从原生队列消费且永不会再被派发，
                                                // 不予回复会挂起请求方。
                                                // 先于 handleSessionExit 回复（后者可能关闭
                                                // bridge，~100ms+）也把延迟降到最低。
                                                // 每次回复单独保护：此处的 JNI 失败绝不能
                                                dispatchClipboardRequests(poll.clipboardReads)
                                                if (poll.sessionId != 0L && poll.sessionId != entry.id) {
                                                    // 后台（非活动）会话的 shell 已退出。
                                                    // 其渲染线程已停止，不会再有其他方回收它
                                                    // ——原生清扫只经本队列上报一次。在此关闭它
                                                    // （handleSessionExit 对非活动会话是安全的：
                                                    // 接替分支以 entry.id == activeSessionId 为闸门）。
                                                    val exitedEntry =
                                                        synchronized(sessionLock) { sessions[poll.sessionId] }
                                                    if (exitedEntry != null) {
                                                        LogUtil.i(
                                                            "Runtime",
                                                            "reaping background session ${poll.sessionId} (exit ${poll.exitCode})",
                                                        )
                                                        handleSessionExit(
                                                            exitedEntry,
                                                            poll.exitCode,
                                                            poll.exitAliveMs,
                                                        )
                                                    }
                                                } else {
                                                    // 完整清理（关闭 bridge、移除会话、更新状态）在此进行；
                                                    // 渲染监视器跳过 !running 的条目，因而绝不会回收已退出会话。
                                                    handleSessionExit(entry, poll.exitCode, poll.exitAliveMs)
                                                }
                                                // 两个分支共用：回收同一帧内退出的其他会话
                                                // （首个已在上方处理）。它们的原生 exit_reported
                                                // 标志已置位且不重发。
                                                // 仅排除 poll.sessionId——列表中其他每个 id
                                                // （含后台分支下的 entry.id）都必须回收，
                                                // 否则 Kotlin 条目、原生会话与僵尸子进程将永久泄漏。
                                                // handleSessionExit 幂等（内部重查 containsKey）。
                                                poll.exits.forEach { exitInfo ->
                                                    if (exitInfo.sessionId != poll.sessionId) {
                                                        val extra =
                                                            synchronized(
                                                                sessionLock,
                                                            ) { sessions[exitInfo.sessionId] }
                                                        if (extra != null) {
                                                            LogUtil.i(
                                                                "Runtime",
                                                                "reaping same-frame exited session ${exitInfo.sessionId} (exit ${exitInfo.exitCode})",
                                                            )
                                                            handleSessionExit(
                                                                extra,
                                                                exitInfo.exitCode,
                                                                exitInfo.exitAliveMs,
                                                            )
                                                        }
                                                    }
                                                }
                                                if (!entry.waitingForProcessCompleted) {
                                                    break
                                                }
                                                // 正在显示 [Process completed] 提示
                                                // ——保持会话可见（running 保持为真）
                                                // 直到用户按 Enter。
                                                // 原生 exit_reported 已置位，故不会再有退出事件到达。
                                            }
                                            eventDispatcher.handle(poll)
                                        } catch (exception: Exception) {
                                            LogUtil.e(
                                                "Runtime",
                                                "pollAll failed for session ${entry.id}; deferred events dropped",
                                                exception,
                                            )
                                        }
                                        if (count >= 0) {
                                            diagCount++
                                            if (diagCount == 1) {
                                                LogUtil.d("Runtime", "session ${entry.id} first render OK")
                                            }
                                            if (diagCount % RENDER_DIAGNOSTIC_FREQUENCY == 0) {
                                                val title =
                                                    try {
                                                        bridge.getActiveSessionTitle()
                                                    } catch (exception: Exception) {
                                                        LogUtil.e("Runtime", "title query failed", exception)
                                                        ""
                                                    }
                                                if (title.isNotEmpty() && title != _state.value.title) {
                                                    // CAS 更新：收集器与 IO 会话函数也会写 _state，
                                                    // 此处非原子的读-改-写会覆盖它们的会话列表。
                                                    _state.update { current -> current.copy(title = title) }
                                                }
                                            }
                                            entry.frameMarks = entry.frameMarks.copy(doneNanos = System.nanoTime())
                                            frameTiming.record(entry.frameMarks.doneNanos - entry.frameMarks.startNanos)
                                            frameTiming.takeReport()?.let { report ->
                                                // 与计时窗口一同输出的内存计量：回滚行数跨窗口单调增长
                                                // 即表示历史无界。
                                                val scrollbackRows =
                                                    try {
                                                        NativeBridge.getScrollbackRows(entry.id)
                                                    } catch (exception: Exception) {
                                                        LogUtil.w("Runtime", "scrollback query failed", exception)
                                                        -1
                                                    }
                                                val summary =
                                                    "session ${entry.id} frame timing window " +
                                                        "(${report.frameCount} frames): " +
                                                        "avg=${report.averageNanos / 1_000_000L}ms " +
                                                        "p95=${report.p95Nanos / 1_000_000L}ms " +
                                                        "max=${report.maxNanos / 1_000_000L}ms " +
                                                        "scrollback=$scrollbackRows rows"
                                                val trendDegraded = frameTimingTrend.observe(report.averageNanos)
                                                val baselineNanos = frameTimingTrend.currentBaselineNanos()
                                                val baselineMs = baselineNanos?.div(1_000_000L)
                                                when {
                                                    // 绝对病态：超出任何设备预期的停滞
                                                    // （模拟器基线 ~555ms/帧；真机 ~17ms）。
                                                    report.p95Nanos >= FRAME_TIME_WARN_P95_NANOS ||
                                                        report.maxNanos >= FRAME_TIME_WARN_MAX_NANOS ->
                                                        LogUtil.w(
                                                            "Runtime",
                                                            "$summary — severe stall(s), investigate render cost",
                                                        )

                                                    // 相对基线的回归（约为设备自身学习到的基线的 3 倍，
                                                    // 且平均至少 100ms）：捕捉绝对阈值无法覆盖的
                                                    // 渐进式、设备特定的降级。
                                                    trendDegraded ->
                                                        LogUtil.w(
                                                            "Runtime",
                                                            "$summary — degraded vs baseline (${baselineMs}ms), " +
                                                                "investigate render cost",
                                                        )

                                                    // 正常窗口：用 Info 而非 Debug，使该计量在 release 构建中仍保留
                                                    // ——LogUtil.d 受 BuildConfig.DEBUG 门控，
                                                    // 在 release APK 上会隐藏每个窗口，
                                                    // 使渐进问题不可见。
                                                    // 每 60 个渲染帧一行（真机 ~1s，模拟器 ~33s），
                                                    // 是安静但始终存在的信号。
                                                    else -> LogUtil.i("Runtime", summary)
                                                }
                                            }
                                            // 尾部队列等待已移除——驻留现在发生在上方的循环顶部唤醒门控
                                            // （同一闭锁、同一活跃/空闲超时）。落到此处即直接返回门控。
                                        }
                                        // 整循环周期：平均值的倒数才是真实帧率
                                        // （含 waitOutput + pollAll + 事件派发）。
                                        // 若渲染均值 ~16ms 而此处 ~50ms，
                                        // 说明时间花在循环的其他环节，而非原生渲染路径。
                                        loopTiming.record(System.nanoTime() - loopFrameStart)
                                        loopTiming.takeReport()?.let { loopReport ->
                                            val loopAvgMs = loopReport.averageNanos / 1_000_000L
                                            val fps = if (loopAvgMs > 0) 1_000L / loopAvgMs else 0L
                                            LogUtil.i(
                                                "Runtime",
                                                "session ${entry.id} loop timing window " +
                                                    "(${loopReport.frameCount} frames): " +
                                                    "avg=${loopAvgMs}ms p95=${loopReport.p95Nanos / 1_000_000L}ms " +
                                                    "max=${loopReport.maxNanos / 1_000_000L}ms ≈${fps}fps",
                                            )
                                        }
                                    } catch (exception: InterruptedException) {
                                        // 渲染线程在关闭期间（会话切换/运行期停止）被中断。
                                        // 这是预期信号而非渲染失败——干净地退出循环。
                                        Thread.currentThread().interrupt()
                                        break
                                    } catch (exception: Exception) {
                                        consecutiveErrors++
                                        if (consecutiveErrors == 1) {
                                            LogUtil.e(
                                                "Runtime",
                                                "session ${entry.id} first render exception",
                                                exception,
                                            )
                                        } else if (consecutiveErrors % RENDER_ERROR_LOG_FREQUENCY == 0) {
                                            LogUtil.e(
                                                "Runtime",
                                                "session ${entry.id} render exception (x$consecutiveErrors)",
                                                exception,
                                            )
                                        }
                                        // 异常帧同样是「渲染线程没挂」的证据：
                                        // 不刷新完成时刻，连续异常会让 `start > done`
                                        // 一直成立，`RenderWatchDog` 在 10s 后把仍在循环的
                                        // 线程判为挂死。当前上限恰好（约 5s）小于超时，
                                        // 但那只是两个常量的巧合，任一改动即成误杀。
                                        entry.frameMarks = entry.frameMarks.copy(doneNanos = System.nanoTime())
                                        if (consecutiveErrors > RENDER_MAX_CONSECUTIVE_ERRORS) {
                                            LogUtil.e(
                                                "Runtime",
                                                "session ${entry.id} too many render exceptions ($consecutiveErrors), stopping render thread",
                                                exception,
                                            )
                                            break
                                        }
                                        delay(RENDER_ERROR_SLEEP_MS)
                                    }
                                }
                                entry.renderThreadExited = true
                                LogUtil.d("Runtime", "render thread stopped for session ${entry.id}")
                            }
                        } catch (expected: InterruptedException) {
                            // runBlocking 体外的中断（join 前 interrupt 已送达
                            // 但循环尚未进入 try 区）：join 是协作式关闭，
                            // 中断即退出信号而非崩溃，instrumentation 不应报红。
                            Thread.currentThread().interrupt()
                            entry.renderThreadExited = true
                            LogUtil.d("Runtime", "render thread interrupted for session ${entry.id}")
                        }
                    },
                    "Render-${entry.id}",
                )
                    .apply {
                        isDaemon = true
                    }
            entry.renderThreadRef = renderThread
            renderThread.start()
            // 以显示 vsync 驱动新渲染循环的唤醒门控
            // （每进程一条自调度的 Choreographer 链）。
            ensureVsyncChainStarted()
            entry.renderWatchDog =
                RenderWatchDog(
                    getMarks = { entry.frameMarks },
                    isRunning = {
                        entry.running && !entry.renderThreadExited && activeSessionId == entry.id
                    },
                    onHangDetected = {
                        LogUtil.e(
                            "Runtime",
                            "session ${entry.id} render thread hung (>${RENDER_HANG_TIMEOUT_NANOS / 1_000_000L}s) — marking thread for restart",
                        )
                        // 把线程标记为死亡。渲染监视器（checkSessions）会侦测到并
                        // 以指数退避重启线程，避免因 GPU 挂起而杀掉整个进程。
                        entry.renderThreadExited = true
                    },
                    hangTimeoutNanos = RENDER_HANG_TIMEOUT_NANOS,
                )
                    .also { it.start() }
        }

        internal fun stopRenderThread(entry: SessionEntry): Boolean {
            entry.renderWatchDog?.stop()
            entry.renderWatchDog = null
            entry.running = false
            val thread = entry.renderThreadRef
            entry.renderThreadRef = null
            entry.renderSignaled.set(false)
            thread?.let { t ->
                t.interrupt()
                t.join(THREAD_JOIN_TIMEOUT_MS)
                if (t.isAlive) {
                    LogUtil.e(
                        "Runtime",
                        "session ${entry.id} render thread still alive after join — possibly hung",
                    )
                    entry.renderThreadPossiblyAlive = true
                    entry.hungRenderThread = t
                    return false
                }
            }
            // 仅当已 join 的线程就是记录的挂起线程（或未记录）时才清除存活标志；
            // 另一个挂起线程可能仍在原生代码中存活，在此清除会让关闭路径在其下
            // 销毁原生会话。
            if (entry.hungRenderThread == null || entry.hungRenderThread === thread) {
                entry.renderThreadPossiblyAlive = false
                entry.hungRenderThread = null
            }
            return true
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 四之二、事件派发
    // ══════════════════════════════════════════════════════════════════════

    /**
     * 派发非退出的轮询事件（剪贴板）。
     *
     * 内部类：直接访问 TerminalRuntime 的处理器/上下文/剪贴板，无需经构造函数传递。
     * 退出回收留在循环中（它拥有 break/清理控制流）。
     */
    inner class EventDispatcher {
        /**
         * 处理 [poll] 中所有非退出事件，由渲染循环在退出处理之后调用。
         * 此处的异常绝不能跳过循环的逐帧簿记（调用方把我们包在外层 try 中）。
         */
        fun handle(poll: terminal.emulator.bridge.Bridge.PollResult) {
            if (poll.clipboard != null) {
                clipboardAccess.setClipboardText(poll.clipboard)
            }
            dispatchClipboardRequests(poll.clipboardReads)
        }
    }

    private companion object {
        private const val TAG = "Runtime"

        /** mksh 交互 shell 经 `$ENV` 加载的启动文件名（DESIGN Shell 节）。 */
        const val MKSHRC_FILENAME = ".mkshrc"

        const val DEFAULT_GRID_ROWS = 24
        const val DEFAULT_GRID_COLS = 80
        private const val TENTHS_PER_UNIT = 10

        /** 网格与像素尺寸上界：PTY winsize 字段为 u16，原生对超限值抛 IllegalArgumentException。 */
        private const val U16_MAX = 0xFFFF

        /** 缩放预览的字号上下界（十分之一单位），对齐原生 setFontSizeInPlace 的钳位（4.0..100.0）。 */
        private const val MIN_FONT_SIZE_TENTHS = 40
        private const val MAX_FONT_SIZE_TENTHS = 1000

        /** 按字体度量重算网格时为 ModifierBar 预留的覆盖层高度，
         *  等于两行按钮（BUTTON_HEIGHT_DP 36 × 2，零间距），
         *  使网格预留与输入法跟随滚动对工具栏遮盖哪些行保持一致。 */
        private const val MODIFIER_BAR_HEIGHT_DP = 72f
        private const val FONT_SIZE_DISPLAY_RATIO = 0.6f
        private const val FONT_SIZE_MIN_PX = 300
        private const val FONT_SIZE_MAX_PX = 600
        private const val FONT_SIZE_HEIGHT_RATIO = 0.5f
        private const val FONT_SIZE_HEIGHT_MIN_PX = 250
        private const val FONT_SIZE_HEIGHT_MAX_PX = 500
        private const val RENDER_ERROR_LOG_FREQUENCY = 60

        // 前台会话的 shell 退出时送入终端的 [Process completed] 提示（保持可见直到按 Enter）。

        /** 退出码未知时写入提示的文本：如实说明，不用 0 冒充。 */
        private const val PROCESS_EXIT_CODE_UNKNOWN_TEXT = "exit code unknown"

        private const val PROCESS_COMPLETED_PROMPT_PREFIX = "\r\n[Process completed (code "
        private const val PROCESS_COMPLETED_PROMPT_SUFFIX = ") - press Enter]"

        private const val RENDER_MAX_CONSECUTIVE_ERRORS = 100
        private const val RENDER_MAX_TRANSIENT_ERRORS =
            50 // 约 2.5s 瞬时错误后退出线程
        private const val RENDER_ERROR_SLEEP_MS = 50L
        private const val RENDER_ERROR_BACKOFF_MS =
            200L // 连续 10 次瞬时错误后延长睡眠

        // logcat 中 LATENCY_REPORT 汇总的输出节奏（样本数）。
        private const val LATENCY_REPORT_EVERY = 50

        // 17ms active latch = 单个 vsync 周期（60Hz 显示 ~16.7ms）：
        // waitOutput 是纯 park（PTY 到达无法提前唤醒，见 gate 注释），
        // 8ms 的旧值让 active 态在每次 vsync 之外再多落一次兜底渲染，
        // 实测渲染循环以 ~166fps 空转冲刷（多数帧被显示端丢弃，Immediate
        // 下还叠加撕裂）。17ms 兜底把 active 节拍上界收敛到显示刷新率：
        // 渲染要么由 Choreographer vsync 唤醒（~16.7ms 一次），要么由
        // 本超时兜底，二者不会在同一周期重复触发。最坏输入→回显量化
        // 延迟约 17ms，不可感知。
        private const val RENDER_LATCH_TIMEOUT_NANOS = 17_000_000L

        // 慢帧诊断：超过此值的帧输出一行 SLOW_FRAME（渲染阶段墙钟时间）供离线拆解。
        private const val SLOW_FRAME_LOG_THRESHOLD_MS = 30.0
        private const val RENDER_LATCH_IDLE_TIMEOUT_NANOS = 500_000_000L // 500ms for idle (~2 FPS)
        private const val RENDER_IDLE_THRESHOLD_NANOS = 5_000_000_000L // 5s idle → switch to low-freq
        private const val RENDER_DIAGNOSTIC_FREQUENCY = 60
        private const val THREAD_JOIN_TIMEOUT_MS = 1000L
        private const val RENDER_HANG_TIMEOUT_NANOS = 10_000_000_000L // 10 seconds

        // 帧时间诊断（FrameTimingStats + FrameTimingTrend）：下方的绝对阈值覆盖超出任何设备
        // 预期的停滞（实测模拟器空闲窗口平均个位数毫秒；真机目标 ~17ms）。
        // 渐进式/设备特定的回归由基线自适应的 FrameTimingTrend 捕捉
        // （约为学习到的基线的 3 倍，平均 ≥100ms），
        // 使真实降级在任何硬件上都能在日志中显现。
        private const val FRAME_TIME_WARN_P95_NANOS = 1_000_000_000L // 1s p95
        private const val FRAME_TIME_WARN_MAX_NANOS = 2_000_000_000L // 2s 单帧
        private const val RENDER_INITIAL_RETRY_MAX = 5
        private const val RENDER_INITIAL_RETRY_DELAY_MS = 150L

        // 渲染监视器 —— 主动死亡检测
        private const val RENDER_MONITOR_INTERVAL_MS = 500L
        private const val RENDER_MAX_RESTART_ATTEMPTS = 5
        private const val INITIAL_RESTART_DELAY_MS = 100L
        private const val MAX_RESTART_DELAY_MS = 1000L
        private const val GRACE_PERIOD_AFTER_RESTART_MS = 300L
    }

    /** 自定义入口原样透传；默认入口走前缀探测结果，探测失败即系统 shell。 */
    private fun resolveEffectiveShell(prefixDir: String, prefixShell: String?, shell: Shell): Shell {
        if (shell is Shell.Custom) return shell
        val prefixShellChecked = prefixShell ?: return shell
        return Shell.Custom("$prefixDir/$prefixShellChecked")
    }

    private data class ConfigReads(val shellPath: String, val fontSizeTenths: Int, val themeName: String)

    /**
     * 默认入口探测：依次尝试 bash 与 login，文件存在即用（DESIGN :188），不检查权限。
     */
    private fun findPrefixShell(prefixDir: String): String? = listOf("bin/bash", "bin/login").firstOrNull { candidate ->
        java.io.File("$prefixDir/$candidate").isFile
    }

    internal suspend fun computeFontSizeTenths(): Int {
        val userFontSize = settingsRepository.fontSize.first()
        if (settingsRepository.fontSizeExplicitlySet.first()) {
            // fontSize 以 sp 为单位（SettingsRepository 默认 10f），fontSizeTenths 是同一值的
            // 十分之一 sp 形式（原生字体管线直接消费 sp，光栅缩放会施加密度）。
            // 此处再乘密度会双重缩放字号（10sp → 225 tenths = 22.5sp），
            // 使设置滑块与渲染尺寸不一致。
            return (userFontSize * TENTHS_PER_UNIT.toFloat()).toInt()
        }
        // 全新安装：按屏幕宽度推导合理默认值
        // （唯一来源：SettingsRepository.defaultFontSizeFor），
        // 使手机（~360dp）和平板（~600dp）显示相同的列数。
        val widthDp = context.resources.configuration.screenWidthDp.toFloat()
        return (SettingsRepository.defaultFontSizeFor(widthDp) * TENTHS_PER_UNIT.toFloat()).toInt()
    }

    /**
     * 缩放预览：把新字体度量推给原生字体管线并刷新 Kotlin 单元格度量，但不 resize 网格。
     * 渲染器在下一帧按新单元格尺寸绘制（cell_builder 每帧读取字体度量），
     * 而 ghostty 保持其行列数，直到手势经 [appliedFontSizeSp]/setFontSize 终结时才执行
     * 完整应用（含网格重排）。调用方需限频——即使在软件 GPU 模拟器上也足够每秒跑数次。
     *
     * 手势期间不得 resize 网格：每次 preview 都重排会高频发 SIGWINCH +
     * ghostty 重排 + native 清图集重光栅，触摸格点与渲染格点持续处于
     * 中间态（布局混乱/撕裂）。网格只在手势结束 finalize 时重算一次。
     */
    fun setFontSizePreview(sizeSp: Float) {
        val tenths = (sizeSp * TENTHS_PER_UNIT.toFloat()).toInt()
        if (tenths < MIN_FONT_SIZE_TENTHS || tenths > MAX_FONT_SIZE_TENTHS) return
        // 同值跳过：手势 preview 高频推送同一字号时不走 JNI，
        // 与 native 侧跳过配合，缩放期间不抖动。
        if (tenths == appliedFontSizeTenths) return
        val entry = sessions[activeSessionId] ?: return
        val bridge = entry.bridge ?: return
        bridge.setFontSizeInPlace(tenths)
        // 只同步触摸/渲染度量，不重算网格不 resize：触摸映射跟上新字形，
        // 网格行列保持到 finalize，避免手势期间中间态错位。
        syncCellMetricsOnly(bridge)
        appliedFontSizeTenths = tenths
    }

    /**
     * 只同步单元格度量（触摸/渲染用），不碰网格行列、不 resize。
     * 手势 preview 路径专用；finalize/设置路径仍走全量同步 + 重算。
     */
    private fun syncCellMetricsOnly(bridge: Bridge) {
        val density = context.resources.displayMetrics.density
        val rawCellWidth = bridge.getCellWidth()
        val rawCellHeight = bridge.getCellHeight()
        if (rawCellWidth > 0f) logicalCellWidth = rawCellWidth
        if (rawCellHeight > 0f) logicalCellHeight = rawCellHeight
        val newCellWidth = rawCellWidth * density
        val newCellHeight = rawCellHeight * density
        if (newCellWidth > 0f) cellWidth = newCellWidth
        if (newCellHeight > 0f) cellHeight = newCellHeight
    }

    /**
     * 原生管线当前渲染的字号（sp），即经 setFontSizeInPlace 最后一次推送的值；
     * 首次应用前回落为按设备自适应的缺省值。
     *
     * 回落必须走 [SettingsRepository.defaultFontSizeFor] 这唯一一处缺省策略：曾硬编码
     * 14sp，于是宽屏设备（真实缺省可达 24sp）的「字体实际大小」恒报 14sp，
     * 与设置页与渲染实际都矛盾。
     */
    fun appliedFontSizeSp(): Float {
        val tenths = appliedFontSizeTenths
        if (tenths > 0) return tenths / TENTHS_PER_UNIT.toFloat()
        val metrics = context.resources.displayMetrics
        return SettingsRepository.defaultFontSizeFor(metrics.widthPixels / metrics.density)
    }

    internal suspend fun resolveThemeName(): String {
        // 单次快照：按字段流逐个 first() 会把同一个 preferences_pb 读五遍并解析五次，
        // 而一次读取已含全部字段（SettingsRepository.settings 是唯一合并快照）。
        var stored = settingsRepository.settings.first()
        // 清除是写操作（会 await dataStore.edit），故清除成功后必须重读：
        // 否则返回的是刚被清除的未知名，`buildConfig` 的 `BuiltInThemes.byName` 在
        // 未知名上抛异常，本次建会话直接失败，用户看不到终端。
        if (clearUnknownThemeNames(stored.dayThemeName, stored.nightThemeName, stored.themeName)) {
            stored = settingsRepository.settings.first()
        }
        val systemDark =
            (
                context.resources.configuration.uiMode and
                    android.content.res.Configuration.UI_MODE_NIGHT_MASK
                ) ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES
        val effectiveDark =
            when (stored.appThemeMode) {
                "day" -> false
                "night" -> true
                else -> systemDark
            }
        return when (stored.themeMode) {
            "day" -> stored.dayThemeName
            "night" -> stored.nightThemeName
            "fixed" -> stored.themeName
            else -> if (effectiveDark) stored.nightThemeName else stored.dayThemeName
        }
    }

    /**
     * 存有无法解析主题名的键即设置数据错误：记日志并清除该键，
     * 下次读取回落默认值（DESIGN:16 设置数据错误 → 清除设置数据、:24 不做 Fallback）。
     *
     * 返回是否真的清除过：调用方据此决定要不要重读快照，否则会把刚清除的未知名
     * 当作解析结果返回。
     */
    private suspend fun clearUnknownThemeNames(vararg names: String): Boolean {
        val unknown = names.filter { BuiltInThemes.byNameOrNull(it) == null }.toSet()
        if (unknown.isEmpty()) return false
        LogUtil.e("Runtime", "Unknown terminal theme, clearing setting: $unknown")
        settingsRepository.clearUnknownThemeNames(unknown)
        return true
    }

    /** 空即默认入口（DESIGN :122 未设置时为空），其余原样透传，不特殊处理。 */
    private fun resolveShell(shellPath: String): Shell = if (shellPath.isEmpty()) {
        Shell.SystemDefault
    } else {
        Shell.Custom(shellPath)
    }

    private fun makeBridgeTheme(resolvedTheme: terminal.emulator.ui.theme.TerminalTheme): BridgeTheme {
        val backgroundColor = resolvedTheme.background.toArgb()
        val foregroundColor = resolvedTheme.foreground.toArgb()
        val cursor = resolvedTheme.cursor.toArgb()
        val ansiInts = resolvedTheme.ansi.map { it.toArgb() }
        return BridgeTheme(
            name = resolvedTheme.name,
            background = backgroundColor,
            foreground = foregroundColor,
            cursor = cursor,
            ansi0 = ansiInts[0],
            ansi1 = ansiInts[1],
            ansi2 = ansiInts[2],
            ansi3 = ansiInts[3],
            ansi4 = ansiInts[4],
            ansi5 = ansiInts[5],
            ansi6 = ansiInts[6],
            ansi7 = ansiInts[7],
            ansi8 = ansiInts[8],
            ansi9 = ansiInts[9],
            ansi10 = ansiInts[10],
            ansi11 = ansiInts[11],
            ansi12 = ansiInts[12],
            ansi13 = ansiInts[13],
            ansi14 = ansiInts[14],
            ansi15 = ansiInts[15],
        )
    }

    suspend fun start(surface: Surface?, width: Int, height: Int) {
        synchronized(sessionLock) {
            if (sessions.isNotEmpty() || starting) return
            starting = true
        }
        // LogUtil.d 已写入 logcat，不重复输出。
        LogUtil.d("Runtime", "start() called: surface=$surface width=$width height=$height")
        // 记住 startRuntime 传入的 Surface，待会话 bridge 就绪后由渲染器绑定。
        if (surface != null) {
            pendingSurface = PendingSurface(surface, width, height)
        }
        if (!NativeBridge.isNativeLoaded()) {
            // NativeInit 线程后台加载 libnative：首帧 surface 就绪时可能尚未完成。
            // 等待至多 3 秒而非直接放弃，避免浪费一轮布局重建的启动延迟。
            var waited = 0
            while (!NativeBridge.isNativeLoaded() && waited < 60) {
                kotlinx.coroutines.delay(50)
                waited++
            }
        }
        if (!NativeBridge.isNativeLoaded()) {
            // 与 createSession 的守卫对称。没有它，bridge.ping() 抛 RuntimeException，
            // 回滚中的 destroySession 抛 UnsatisfiedLinkError（是 Error，catch(Exception) 捕获不到），
            // 原生库缺失或 ABI 不匹配时应用会崩溃。
            LogUtil.e("Runtime", "start: native library not loaded, aborting start")
            synchronized(sessionLock) {
                starting = false
            }
            return
        }
        val displayW = context.resources.displayMetrics.widthPixels
        val displayH = context.resources.displayMetrics.heightPixels
        val density = context.resources.displayMetrics.density
        LogUtil.d(
            "Runtime",
            "displayMetrics: w=$displayW h=$displayH density=$density",
        )

        val bypassMinSurface = System.getProperty("test.minSurface") != null

        if (!bypassMinSurface && (width <= 0 || height <= 0)) {
            LogUtil.e(
                "Runtime",
                "start() called with non-positive dimensions, waiting for surfaceChanged",
            )
            starting = false
            return
        }

        val minWidth =
            (displayW * FONT_SIZE_DISPLAY_RATIO).toInt().coerceIn(FONT_SIZE_MIN_PX, FONT_SIZE_MAX_PX)
        val minHeight =
            (displayH * FONT_SIZE_HEIGHT_RATIO)
                .toInt()
                .coerceIn(FONT_SIZE_HEIGHT_MIN_PX, FONT_SIZE_HEIGHT_MAX_PX)
        if (!bypassMinSurface && (width < minWidth || height < minHeight)) {
            LogUtil.w(
                "Runtime",
                "start() called with small surface ${width}x$height (display=${displayW}x$displayH min=${minWidth}x$minHeight), waiting for correct surfaceChanged",
            )
            starting = false
            return
        }

        // 提到外层以便失败路径能关闭它；start() 在 createBridge() 之前提前返回时保持为 null。
        var startedBridge: terminal.emulator.bridge.Bridge? = null
        if (surface != null) {
            // 原生侧经 attachWindow(JNI) 接收 Surface，Kotlin 绝不跨桥传递裸 ANativeWindow 指针；
            // 指针为 0 时不得中止启动，否则终端根本无法启动。
            LogUtil.d("Runtime", "surface present — render integration pending (ADR-0007)")
        } else {
            LogUtil.d("Runtime", "no surface — using GPU offscreen rendering path")
        }

        try {
            // 引导严格选择性启用：全新应用运行系统 shell 且不下载任何内容，
            // 除非用户在设置中显式配置了引导 URL。首启自动下载 Termux 引导
            // （~150 MB）既侵入又无上限。测试可用系统属性覆盖（不依赖 DataStore）。
            val testUrl = System.getProperty("test.bootstrapUrl")
            val bootstrapUrl = if (testUrl != null) testUrl else settingsRepository.bootstrapUrl.first()
            if (bootstrapUrl.isNotEmpty()) {
                // 仅记录来源（scheme://host），不记录完整 URL：
                // 私有引导程序地址可能携带 token 与查询参数，logcat 无差别记录。

                val origin =
                    runCatchingCancellable {
                        val uri = bootstrapUrl.toUri()
                        val scheme = uri.scheme ?: return@runCatchingCancellable "<no-scheme>"
                        val host = uri.host
                        if (host.isNullOrBlank()) return@runCatchingCancellable "<no-host>"
                        "$scheme://$host"
                    }
                        .getOrNull() ?: "<unparsable>"
                LogUtil.d("Runtime", "Bootstrap URL set: $origin")
                val downloader = terminal.emulator.installer.BootstrapDownloader(context)
                val installer =
                    terminal.emulator.installer.BootstrapInstaller(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        stagingDir = java.io.File(context.filesDir, "usr-staging"),
                    )
                val secondStage =
                    terminal.emulator.installer.SecondStageRunner(
                        prefixDir = java.io.File(context.filesDir, "usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                    )
                val installOrchestrator =
                    terminal.emulator.installer.BootstrapOrchestrator(downloader, installer, secondStage)
                when (installOrchestrator.getInstallStatus()) {
                    terminal.emulator.installer.BootstrapOrchestrator.Status.NOT_INSTALLED -> {
                        // 绝不自动下载：Termux 引导（~150 MB）必须从设置显式安装，
                        // 应用不得自行下载或安装。仅记录状态使缺失安装仍可诊断；
                        // 设置中的引导按钮是唯一入口。
                        LogUtil.d("Runtime", "Bootstrap not installed — install manually from Settings")
                    }

                    terminal.emulator.installer.BootstrapOrchestrator.Status.INSTALLED -> {
                        LogUtil.d("Runtime", "Bootstrap already installed")
                    }

                    else -> {}
                }
            }
            val configStartNs = System.nanoTime()
            val config = buildConfig()
            LogUtil.d(
                "Runtime",
                "buildConfig: fontSizeTenths=${config.fontSizeTenths} rows=${config.rows} cols=${config.cols} theme=${config.theme.name} elapsed=${(System.nanoTime() - configStartNs) / 1_000_000}ms",
            )
            val bridgeStartNs = System.nanoTime()
            val bridge = createBridge(config)
            startedBridge = bridge
            LogUtil.d(
                "Runtime",
                "bridge created: ${bridge.ping()} elapsed=${(System.nanoTime() - bridgeStartNs) / 1_000_000}ms",
            )

            bridge.setSystemLocale(
                java.util.Locale.getDefault().toLanguageTag(),
            )
            LogUtil.d("Runtime", "setSystemLocale: ${java.util.Locale.getDefault().toLanguageTag()}")

            val fontDropDir = terminal.emulator.termuxFontDir(context)
            fontDropDir.apply {
                if (!exists() && !mkdirs()) {
                    LogUtil.w("Runtime", "Failed to create font drop-in directory: $this")
                }
            }
            // 注意：此处刻意不调用 bridge.setExtraFontPaths
            // ——Bridge 在 sessionId == 0（spawnTerminal 之前）会跳过它，
            // 那会静默丢弃额外的字体路径（用户字体永远不加载，回退找到 0 个）。
            // 它在下方 spawnTerminal 之后被再次调用。

            // 上方的引导下载/安装可能耗时数分钟。传入 start() 的 Surface
            // 可能已在此期间被销毁（旋转、分屏）；其 ANativeWindow 指针已悬空。
            // 在其上 spawn 会渲染到已死的窗口（黑屏/挂起）。中止，
            // 让下一次 surface-available 事件用新的 Surface 重试 start()。
            if (surface != null && !surface.isValid) {
                LogUtil.e(
                    "Runtime",
                    "start(): surface became invalid during bootstrap, aborting (will retry on next surface)",
                )
                // bridge（原生引擎）已在上方创建；必须关闭它，
                // 否则每次无效 Surface 重试都会泄漏一个。
                // （流分析保证此处 startedBridge 非 null：
                // 赋值发生在同一 try 块内的更早处。）
                try {
                    startedBridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e("Runtime", "Failed to close bridge on invalid surface", closeException)
                }
                startedBridge = null
                starting = false
                return
            }

            val spawnStartNs = System.nanoTime()
            val spawnResult = bridge.spawnTerminal(config.rows, config.cols, bridge.shellPath())
            val spawnElapsedMs = (System.nanoTime() - spawnStartNs) / 1_000_000
            LogUtil.d(
                "Runtime",
                "spawnTerminal: rows=${config.rows} cols=${config.cols} result=$spawnResult elapsed=${spawnElapsedMs}ms",
            )
            if (spawnResult <= 0L) {
                LogUtil.e(
                    "Runtime",
                    "spawnTerminal returned $spawnResult — native session init failed, aborting start",
                )
                bridge.close()
                return
            }
            // 渲染预热与 shell 启动并行：wgpu 初始化 + 字体库加载移出 attach→首帧链。
            bridge.prefetchRenderStateAsync(scope)

            // sessionId 此时已非零——用户字体目录（home/.termux/font）此刻才真正到达原生字体库。
            // 在 spawnTerminal 之前调用则是静默空操作。
            bridge.setExtraFontPaths(listOf(fontDropDir.absolutePath))
            // 同样的 sessionId 门控也适用于系统区域设置：上方的 spawn 前 setSystemLocale
            // 被 Bridge.setSystemLocale 的 sessionId == 0 守卫丢弃，使原生管线停留在
            // locale ""（无 CJK locale 增强）。在此重新应用，使 CJK 回退顺序与系统区域设置一致。
            bridge.setSystemLocale(
                java.util.Locale.getDefault().toLanguageTag(),
            )

            try {
                val initialFontFamily = settingsRepository.fontFamily.first()
                val effectiveFont = terminal.emulator.resolveEffectiveFontFamily(initialFontFamily)
                bridge.setFontFamily(effectiveFont)
                // 原生渲染器以硬编码的 14.0px 字体启动；不设此项则用户的字号设置永远到不了
                // GPU 路径——字形始终很小，表现为「设置无效/重启后更糟」。
                bridge.setFontSizeInPlace(config.fontSizeTenths)
                // 按设备密度光栅化字形，使高密度屏上文字清晰
                // （swash 位图按 raster_scale 缩放，着色器按该尺度采样图集）。
                val density = context.resources.displayMetrics.density
                // raster_scale 必须覆盖完整的 sp→px 映射：字号以 sp 存储，
                // 而 sp 同时随显示密度与用户系统字体缩放而缩放。仅按 density 光栅化
                // 会在 fontScale > 1 时（如「字体大小」无障碍设置）光栅不足，
                // 着色器随后放大图集位图——即「文字模糊」问题的来源。
                bridge.setRasterScale(
                    (density * context.resources.configuration.fontScale).coerceIn(0.5f, 4f),
                )
                // 从新字体度量刷新 cellWidth/cellHeight 并重算网格，
                // 使首个渲染帧与配置的字号一致。不做此步，渲染器会在 spawn 时的旧网格上
                // 绘制新尺寸的单元格——日志中「字号设置与实际不符」的闪烁
                // （cell_builder 会在 ~60-160ms 内用新单元格度量配旧网格记录，
                // 直到下一次 insets/surface 事件）。
                syncGridDimensions(bridge)
                recomputeGridFromFontMetrics()
                appliedFontSizeTenths = config.fontSizeTenths
                bridge.setTheme(config.theme)
                LogUtil.d(
                    "Runtime",
                    "settings applied: fontFamily=$effectiveFont fontSizeTenths=${config.fontSizeTenths} theme=${config.theme.name}",
                )
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.e(
                    "Runtime",
                    "Failed to apply initial settings (continuing with defaults)",
                    exception,
                )
            }

            // 原生 spawn 结果是权威会话 ID（当 createSession 与这条较慢的引导路径
            // 并发运行时，原生与 Kotlin 两侧的序列可能漂移）。在锁内以该 ID 插入。
            var finalSessionId = 0L
            var entry: SessionEntry? = null
            var abandonedReason: String? = null
            synchronized(sessionLock) {
                if (sessions.isNotEmpty()) {
                    // 上方引导下载运行期间已有会话被创建（或 start() 重入）。
                    // 插入第二个活动条目会在唯一的全局事件队列上启动第二个渲染线程，
                    // 并覆盖现有会话的 UI 状态——改为销毁刚 spawn 的原生会话，
                    // 保持现有会话活动。
                    LogUtil.w("Runtime", "start: sessions already exist, aborting own insertion")
                    starting = false
                    abandonedReason = "duplicate"
                } else {
                    finalSessionId = spawnResult
                    entry =
                        SessionEntry(
                            id = finalSessionId,
                            bridge = bridge,
                            renderThreadRef = null,
                            running = true,
                        )
                    sessions[finalSessionId] = entry
                    activeSessionId = finalSessionId
                    bridge.onPtyWrite = { nanos ->
                        entry.latencyProbe.onInputWritten(nanos)
                        // 输入写入唤醒：每次 PTY 写入（经 processKeyEvent/writeKey 的硬件按键、
                        // IME sendKeyEvent 退格、鼠标）都必须离开空闲驻留。
                        // 空闲 >5s 后循环驻留在 500ms 闭锁上且 vsync 泵已停
                        // ——没有此唤醒，退格的 shell 回显要等满整个 500ms 空闲闭锁节拍
                        // （即输入→回显延迟）。
                        entry.notifyRender()
                    }
                }
            }
            if (abandonedReason != null) {
                // 在锁外关闭 bridge（Session::drop 会 join PTY 读取线程数百毫秒；
                // 跨它持有 sessionLock 会冻结所有会话操作）。与
                // createSessionInner 的 abandonedByStart/abandonedByStop 回滚一致。
                try {
                    bridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e("Runtime", "start: failed to close $abandonedReason bridge", closeException)
                }
                return
            }
            // entry 在上方每个分支中都已赋值；只是跨提前返回时智能转换看不到可空性。
            val startedEntry = requireNotNull(entry) { "start: session entry must exist after spawn" }
            // 首帧渲染：有问题的 GPU（缺少 SURFACE_VIEW_FORMATS 的 Mali-G57 等）
            // 可能让 get_current_texture() 无限挂起。此前派生守护线程调用 bridge.render()
            // 的做法会导致互斥量饿死——守护线程会获取 surface Mutex 并挂起，
            // 永久阻塞真正的渲染线程。改为经 forceRenderRequested 信号渲染循环产出首帧，
            // 由真正的渲染线程启动后取走。
            attachPendingSurface(bridge)
            // 带网格尺寸的首次 resize 必须在 spawn 之后立即发出
            // ——attachPendingSurface → recomputeGridFromFontMetrics 会做这件事；
            // 在 spawn 序列中锚定网格尺寸以便验证。
            LogUtil.d(
                "Runtime",
                "first resize after spawn: grid=${_state.value.rows}x${_state.value.cols}",
            )
            startedEntry.forceRenderRequested = true
            // 在同一个 sessionLock 临界区内启动渲染线程、发布 UI 状态并启动前台服务与监视器：
            // 最后一个写入者对并发关闭路径胜出。先检查后发布会留下一个窗口，
            // 使关闭已完成而 start() 仍发布幽灵 RuntimeState / 复活服务。
            synchronized(sessionLock) {
                val stillActive = sessions[finalSessionId] === startedEntry
                if (!stillActive) {
                    // 插入之后有并发关闭落地：条目已在清理中（closeSession 已移除条目）。
                    // 此刻发布 RuntimeState 或启动前台服务会在拆除之下复活幽灵 UI 状态与通知。
                    LogUtil.w(
                        "Runtime",
                        "start: session $finalSessionId closed/stopped during startup, skipping render start",
                    )
                    // 防御性：若这是并发关闭而非完全停止，则为存活会话保持监视器运行。
                    // 当前不可达（start() 只向空映射插入，且 starting=true 阻止并发创建，
                    // 因此不可能存在其他会话），但若该不变式日后变化，遵守它成本很低。
                    if (sessions.isNotEmpty()) {
                        renderSupervisor.startRenderMonitor()
                    }
                    return
                }

                // 在启动线程之前发布状态：否则渲染线程对 title 的 CAS
                // 会在其首帧被这次直接赋值覆盖。
                _state.value =
                    RuntimeState(
                        isRunning = true,
                        rows = config.rows,
                        cols = config.cols,
                        activeSessionId = finalSessionId,
                        sessionIds = listOf(finalSessionId),
                    )
                LogUtil.d(
                    "Runtime",
                    "session $finalSessionId config: rows=${config.rows} cols=${config.cols} fontSizeTenths=${config.fontSizeTenths}",
                )
                LogUtil.d("Runtime", "session $finalSessionId started")
                try {
                    startForegroundServiceIfNeeded()
                    updateForegroundSessionCount(sessions.size)
                } catch (serviceException: Exception) {
                    if (serviceException is kotlinx.coroutines.CancellationException) {
                        // 实践中不可达（该加锁块内无挂起点；服务调用是同步 IPC），
                        // 但按项目约定重抛。若它真的触发，条目保持已插入状态，
                        // 清理由调用方的作用域拆除负责。
                        throw serviceException
                    }
                    // 与 createSession 同样的守卫：服务启动失败
                    // 绝不能回滚已创建的会话。
                    LogUtil.e(
                        "Runtime",
                        "Failed to start foreground service for session $finalSessionId",
                        serviceException,
                    )
                }
                renderSupervisor.startRenderThread(startedEntry)
                renderSupervisor.startRenderMonitor()
            }
            // 在会话注册之后才重算网格
            // ——先前（插入之前）的尝试是静默空操作，
            // 于是 24x80 的启动网格留存，尽管原生字体是 47px，
            // 留下巨大的垂直空隙（92px 行高对 36px 字形）。
            try {
                startedEntry.bridge?.let { syncGridDimensions(it) }
                // 网格保持初始 24x80——字体度量不决定网格尺寸；
                // 内容溢出时终端自行滚动。
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "initial grid recompute failed", exception)
            }
        } catch (exception: Exception) {
            // 取消与失败统一先回滚已 spawn 的 bridge，再按类型处理：
            // 取消恰是最易发生的路径（Activity 销毁），跳过回滚即永久泄漏
            // 原生会话及其 PTY 子进程（既不在 sessions 里，也无 UI 入口）。
            try {
                startedBridge?.close()
            } catch (closeException: Exception) {
                LogUtil.e("Runtime", "Failed to close bridge during start rollback", closeException)
            }
            if (exception is kotlinx.coroutines.CancellationException) {
                // 重抛取消：吞没它会破坏结构化并发（与 createSessionInner 同一约定）。
                // finally 块仍会重置 starting。
                throw exception
            }
            LogUtil.e("Runtime", "Failed to start terminal", exception)
            // 完整堆栈经 LogUtil 抵达 logcat，并带稳定的 FAILED grep 锚点。
            // createBridge() 之后的任何失败（设置、attachSurface、spawnTerminal 抛异常而非返回 0）
            // 否则会永久泄漏原生会话及其 PTY 子进程。
            // 若失败发生在条目插入之后（例如 startRenderThread 在锁内抛异常），
            // 映射中仍持有 renderThreadRef 为 null 的条目：checkSessions 的存活逻辑
            // 永不将其标记为死亡（线程引用为 null），幽灵标签页将永久存在。
            // 在锁内移除它——bridge 已在上方关闭，且关闭是幂等的。
            // 同时恢复 UI 状态：失败前发布的 RuntimeState 否则会保持 isRunning=true
            // 却配上已死的 id。
            synchronized(sessionLock) {
                startedBridge?.let { bridgeToRemove ->
                    sessions.entries.removeIf { it.value.bridge === bridgeToRemove }
                }
                if (sessions.isEmpty()) {
                    activeSessionId = 0L
                    _state.update { RuntimeState() }
                    // 服务可能在失败前已在锁内启动；把计数归零，
                    // 使通知与唤醒锁不活得比空会话映射更久
                    // （updateForegroundSessionCount 异常安全，且在 0 时清除运行标志）。
                    updateForegroundSessionCount(0)
                }
            }
        } finally {
            starting = false
        }
    }

    // 架构说明：每个会话当前各自创建 bridge 与独立 GPU Surface
    // （surface.rs 按 ANativeWindow 拥有各自的 wgpu 管线）。
    // 跨会话共享单条预初始化 GPU 管线是可能的未来优化（可缩短会话创建时间），
    // 但尚未实现——不要假定存在共享管线。
    private val createSessionMutex = kotlinx.coroutines.sync.Mutex()

    /**
     * 新建终端会话。与并发调用（新会话按钮连点）以及 [start] 串行化：
     * 两次并发创建会各自 spawn 原生会话并启动渲染线程，
     * 而两个渲染线程消费唯一的全局事件队列会导致事件错投（退出事件被丢弃、会话泄漏）。
     */
    suspend fun createSession(surface: Surface, width: Int, height: Int): Long = createSessionMutex.withLock {
        createSessionInner(surface, width, height)
    }

    private suspend fun createSessionInner(surface: Surface, width: Int, height: Int): Long {
        if (starting) {
            LogUtil.w(
                "Runtime",
                "createSession: start() in progress (bootstrap), refusing concurrent creation",
            )
            return -1L
        }
        if (width <= 0 || height <= 0) {
            LogUtil.e("Runtime", "createSession: invalid dimensions ${width}x$height")
            return -1L
        }
        if (!surface.isValid) {
            LogUtil.e("Runtime", "createSession: surface is not valid")
            return -1L
        }
        if (!NativeBridge.isNativeLoaded()) {
            // UnsatisfiedLinkError 是 Error 而非 Exception——当原生库缺失/损坏时
            // （如 x86 模拟器或 32 位设备上的 ABI 不匹配）它会逃出下方的 catch 并使应用崩溃。
            // 软失败并复用既有的回滚路径。
            LogUtil.e("Runtime", "createSession: native library not loaded, refusing to spawn")
            return -1L
        }
        var nextId = 0L
        var createdBridge: terminal.emulator.bridge.Bridge? = null
        // 回滚路径可见的条目引用：并发 closeSession 可能把它标记为
        // renderThreadPossiblyAlive，此时绝不可 bridge.close()。
        var hangGuardedEntry: SessionEntry? = null
        try {
            val configStartNs = System.nanoTime()
            val config = buildConfig()
            val bridgeStartNs = System.nanoTime()
            val bridge = createBridge(config).also { createdBridge = it }
            LogUtil.d(
                "Runtime",
                "createSession bridgeElapsed=${(System.nanoTime() - bridgeStartNs) / 1_000_000}ms",
            )
            bridge.setSystemLocale(
                java.util.Locale.getDefault().toLanguageTag(),
            )

            // 先 spawn（在锁外），使原生会话 ID 成为权威映射键。
            // 当 start()（较慢的引导路径，同样在其锁外 spawn）与 createSession 并发时，
            // Kotlin 的 max+1 序列与原生序列可能漂移；
            // 以原生 ID 路由 switchSession/handleSessionExit 可保持同步。
            val spawnStartNs = System.nanoTime()
            val spawnResult = bridge.spawnTerminal(config.rows, config.cols, bridge.shellPath())
            val spawnElapsedMs = (System.nanoTime() - spawnStartNs) / 1_000_000
            LogUtil.d(
                "Runtime",
                "createSession spawnTerminal result=$spawnResult elapsed=${spawnElapsedMs}ms",
            )
            if (spawnResult <= 0L) {
                // 无回退：启动入口失败不尝试其他 shell；已有会话原样保留显示，失败经 logcat 输出。
                throw RuntimeException("native spawn failed (result=$spawnResult)")
            }
            bridge.prefetchRenderStateAsync(scope)
            nextId = spawnResult

            // spawn 之后才应用渲染设置：Bridge 的每项设置在 sessionId == 0 时都是空操作，
            // spawnTerminal 之前调用会静默丢弃（用户主题被丢、渲染器停在默认调色板）。
            // 原生渲染状态是进程级单例，而 start() 会在 surface 过小/无效时提前返回
            // （见上方 bypassMinSurface 分支），此时 createSession 是首个建会话的入口；
            // 不在此补齐整组设置，本进程余下所有会话都会用硬编码的默认字号、
            // 默认光栅尺度渲染，且用户字体目录永不注册。
            bridge.setTheme(config.theme)
            bridge.setSystemLocale(
                java.util.Locale.getDefault().toLanguageTag(),
            )
            bridge.setExtraFontPaths(listOf(terminal.emulator.termuxFontDir(context).absolutePath))
            try {
                val effectiveFont =
                    terminal.emulator.resolveEffectiveFontFamily(settingsRepository.fontFamily.first())
                bridge.setFontFamily(effectiveFont)
                bridge.setFontSizeInPlace(config.fontSizeTenths)
                bridge.setRasterScale(
                    (
                        context.resources.displayMetrics.density *
                            context.resources.configuration.fontScale
                        ).coerceIn(0.5f, 4f),
                )
                appliedFontSizeTenths = config.fontSizeTenths
                LogUtil.d(
                    "Runtime",
                    "createSession settings applied: fontFamily=$effectiveFont fontSizeTenths=${config.fontSizeTenths}",
                )
            } catch (exception: Exception) {
                if (exception is kotlinx.coroutines.CancellationException) throw exception
                LogUtil.e(
                    "Runtime",
                    "Failed to apply settings to new session (continuing with defaults)",
                    exception,
                )
            }

            val entry: SessionEntry
            val abandonedByStart: Boolean
            synchronized(sessionLock) {
                if (starting) {
                    // start()（引导慢路径）可能在我们先前检查之后启动，
                    // 并在我们挂起于 spawn 调用期间插入了自己的会话。
                    // 插入第二个条目会让两个会话中有一个没有渲染线程（僵尸 shell 进程），
                    // 并使 UI 会话列表失步。拒绝插入；调用方
                    // （onSurfaceTextureAvailable 回退）会在引导完成后经 start() 重试。
                    LogUtil.w("Runtime", "createSession: start() began during spawn, abandoning insertion")
                    abandonedByStart = true
                } else {
                    entry =
                        SessionEntry(
                            id = nextId,
                            bridge = bridge,
                            renderThreadRef = null,
                            running = false,
                        )
                    sessions[nextId] = entry
                    abandonedByStart = false
                    hangGuardedEntry = entry
                    bridge.onPtyWrite = { nanos ->
                        entry.latencyProbe.onInputWritten(nanos)
                        // 输入写入唤醒：每次 PTY 写入都把渲染循环推离空闲闭锁，
                        // 使输入回显按 17ms 活跃节奏渲染（而非 500ms 空闲闭锁节拍）。
                        entry.notifyRender()
                    }
                }
            }
            if (abandonedByStart) {
                // 在锁外关闭 bridge（Session::drop 会 join PTY 读取线程），
                // 使刚 spawn 的原生会话及其 shell 子进程不被泄漏。
                try {
                    bridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e("Runtime", "createSession: failed to close abandoned bridge", closeException)
                }
                return -1L
            }

            try {
                // 在锁外：switchSessionInternal 会执行同步的首帧渲染，
                // 在挂起的 GPU 上可能阻塞；跨它持有 sessionLock
                // 会冻结所有会话操作。needsSpawn=false：该会话已在上方 spawn
                // ——在此再 spawn 会创建第二个原生会话，其 ID 与映射键分歧
                // （输入/输出分裂 + 泄漏的 shell 进程）。
                switchSessionInternal(
                    nextId,
                    surface,
                    width,
                    height,
                )
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "Failed to switch to new session $nextId, rolling back", exception)
                // 结构性的映射变更在锁内完成（不变式）；
                // 下方的 bridge 关闭留在锁外（Session::drop 会 join 线程）。
                synchronized(sessionLock) {
                    sessions.remove(nextId)
                }
                // 关闭原生 bridge，避免 Rust 侧会话及其 PTY 子进程泄漏；
                // 渲染线程仍活着时由同一条守卫跳过（在其下销毁即 use-after-free）。
                val rollbackEntry = hangGuardedEntry
                if (rollbackEntry != null) {
                    rollbackEntry.closeBridgeUnlessRenderThreadAlive("switch failed rollback")
                } else {
                    try {
                        bridge.close()
                    } catch (closeException: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "Failed to close bridge during session $nextId rollback",
                            closeException,
                        )
                    }
                }
                throw exception
            }

            updateState()
            // 在引导路径之外创建的会话（如关闭全部会话后的「+」按钮）
            // 必须让前台服务重新运行：关闭路径在计数为 0 时会停掉它，
            // 没有它就没有前台通知也没有 PARTIAL_WAKE_LOCK——后台会话会被杀。
            // 服务启动与存活重查在同一个 sessionLock 段内完成，与 start() 对称：
            // 与并发关闭路径串行化，使关闭既不会落在服务启动与重查之间
            // （在拆除之下复活通知），也不会在段中途清空映射（幽灵 id）。
            // 单独保护：此处的 ForegroundServiceStartNotAllowedException
            // （API 31+ 后台启动）或 ROM 的 SecurityException
            // 绝不能经下方通用 catch 泄漏已插入的会话
            // （那会跳过 bridge 关闭并对一个存活会话返回 -1）。
            val stillPresent: Boolean
            synchronized(sessionLock) {
                stillPresent = sessions.containsKey(nextId)
                if (stillPresent) {
                    try {
                        startForegroundServiceIfNeeded()
                        updateForegroundSessionCount(sessions.size)
                    } catch (serviceException: Exception) {
                        if (serviceException is kotlinx.coroutines.CancellationException) {
                            // 实践中不可达（该加锁段内无挂起点），按约定重抛；
                            // 任何清理由作用域拆除负责。
                            throw serviceException
                        }
                        LogUtil.e(
                            "Runtime",
                            "Failed to start foreground service for session $nextId",
                            serviceException,
                        )
                    }
                }
            }
            if (!stillPresent) {
                LogUtil.w(
                    "Runtime",
                    "createSession: session $nextId removed concurrently (exit/close), rolling back",
                )
                // 注意：此 close 可能与移除该条目的路径在锁外的 close 竞争
                // （handleSessionExit / closeSession）。构造上安全：
                // Bridge.close() 凭其 sessionId!=0 守卫幂等，
                // 原生 destroySession 凭注册表移除幂等。
                val concurrentRemovalEntry = hangGuardedEntry
                if (concurrentRemovalEntry != null) {
                    concurrentRemovalEntry.closeBridgeUnlessRenderThreadAlive(
                        "concurrent removal rollback",
                    )
                } else {
                    bridge.close()
                }
                return -1L
            }
            LogUtil.d("Runtime", "session $nextId created and activated")
            return nextId
        } catch (exception: Exception) {
            // 取消与失败统一先回滚未入库的 bridge，再按类型处理（与 start 同形）。
            // 已入库的会话由并发关闭路径负责；未入库的桥接没有渲染线程，
            // 只有条目已被并发移除且其渲染线程挂起时才跳过（UAF 守卫）。
            val orphanEntry = hangGuardedEntry?.takeIf { sessions[nextId] == null }
            if (orphanEntry != null) {
                orphanEntry.closeBridgeUnlessRenderThreadAlive("createSession failed")
            } else if (createdBridge != null && createdBridge !== sessions[nextId]?.bridge) {
                try {
                    createdBridge.close()
                } catch (closeException: Exception) {
                    LogUtil.e(
                        "Runtime",
                        "Failed to close leaked bridge during createSession rollback",
                        closeException,
                    )
                }
            }
            if (exception is kotlinx.coroutines.CancellationException) {
                // 重抛取消：吞没它会破坏结构化并发。
                throw exception
            }
            LogUtil.e("Runtime", "Failed to create session $nextId", exception)
            // 完整堆栈经 LogUtil 抵达 logcat，并带稳定的 FAILED grep 锚点。
            return -1L
        }
    }

    suspend fun switchSession(id: Long, surface: Surface, width: Int, height: Int) {
        switchSessionInternal(id, surface, width, height)
        updateState()
    }

    private suspend fun switchSessionInternal(id: Long, surface: Surface, width: Int, height: Int) {
        // 阶段 1（加锁）：校验、停止上一个渲染线程、（重）配置目标 bridge。
        // 阶段 2（不持锁）：同步的首帧渲染重试——bridge.render() 在挂起的 GPU 上
        // 可能永久阻塞，跨它持有 sessionLock 会冻结所有会话操作
        // （关闭/切换/停止、渲染监视器）。
        // 阶段 3（加锁）：启动新渲染线程并发布新的活动会话。
        val target: SessionEntry
        val previousActiveId: Long
        synchronized(sessionLock) {
            target =
                sessions[id]
                    ?: run {
                        LogUtil.e("Runtime", "switchSession: session $id not found")
                        return
                    }
            // 在任何停止动作之前捕获；用于切换/spawn 失败时恢复前一个会话。
            previousActiveId = activeSessionId
            if (id == activeSessionId) return
            // 把 Surface 交给渲染器（attachWindow JNI 在 Rust 内部提取 ANativeWindow）。
            // 这是惰性的：它只存储引用——wgpu surface 在首个渲染帧才创建，
            // 而那发生在旧会话线程停止、其 surface 在下方释放之后。
            // 因此释放顺序为：attach 存储 → 停止旧线程 → 释放旧 surface →
            // 首帧时创建新 surface；同一 ANativeWindow 绝不会被两个存活的 wgpu surface 持有。
            target.bridge?.attachSurface(surface, width, height)

            if (!surface.isValid) {
                LogUtil.e("Runtime", "switchSession: surface is no longer valid, aborting")
                return
            }

            val current = sessions[activeSessionId]
            if (current != null) {
                // stopRenderThread 在持有 sessionLock 时 join 旧渲染线程
                // （最长 THREAD_JOIN_TIMEOUT_MS = 1s）。switchSession 运行在
                // IO 调度器协程上（TerminalViewModel），因此不会 ANR UI 线程
                // ——但它会拖住所有经 sessionLock 协调的操作
                // （关闭/切换/创建/stopForegroundServiceIfIdle，包括 MainActivity.onDestroy）。
                // 这是已接受的取舍：join 只在旧线程卡死于原生代码时才阻塞
                // （GPU 挂起——渲染本已降级），而无锁停止会让 closeSession
                // 与该条目的拆除竞争（bridge 上的 use-after-free）。
                try {
                    val stopped = renderSupervisor.stopRenderThread(current)
                    if (stopped) {
                        // 在新 bridge 于同一 ANativeWindow 上创建自己的 surface 之前，
                        // 释放旧 bridge 的 GPU surface。这可避免两个 wgpu surface
                        // 共享同一 ANativeWindow 时 Vulkan 驱动报
                        // VK_ERROR_NATIVE_WINDOW_IN_USE_KHR。
                        // 渲染线程挂起时（join 超时）跳过——线程恢复后释放即是 use-after-free。
                        current.bridge?.releaseGpuSurface()
                    } else {
                        LogUtil.e(
                            "Runtime",
                            "switchSession: session ${current.id} render thread hung — skipping surface release",
                        )
                    }
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "switchSession: error stopping current session", exception)
                }
            }

            // Surface 已在上方经 attachSurface 交给渲染器；此处无需其他操作。
            target.running = true
        }

        // 阶段 2（不持锁）：在事件驱动的渲染线程启动前，同步渲染新会话的首帧，
        // 使重配后的交换链立即显示真实内容而非一闪而过的空白/清屏帧。
        // 上方的交换链重配会丢弃上一个会话的后缓冲，
        // 而渲染线程的首帧要等操作系统线程调度后才呈现——那段空隙正是空白闪烁。
        // 在此呈现即可消除它。渲染线程紧接着接管
        // （它会先重渲染一次，随后按 RENDER_LATCH_IDLE_TIMEOUT_NANOS 节奏闭锁，
        // 事件驱动的模型得以保留）。
        //
        // 注意：刻意置于 sessionLock 之外——bridge.render() 在 GPU 故障时
        // （Mali-G57 的 get_current_texture）可能无限挂起。
        try {
            // GPU surface 在 spawnTerminal/attachSurface 之后可能尚未完全配置好
            // （共享 ANativeWindow 上的一瞬竞争）。以短暂延迟重试几次首次同步渲染，
            // 以便呈现真实内容，而不是在尚未就绪的 Surface 上启动渲染线程
            // （那会阻塞并触发挂起看门狗）。
            var initialRender = target.bridge?.render() ?: 0
            var attempts = 1
            while (initialRenderRetryNeeded(initialRender, attempts, RENDER_INITIAL_RETRY_MAX)) {
                delay(RENDER_INITIAL_RETRY_DELAY_MS)
                initialRender = target.bridge?.render() ?: 0
                attempts++
            }
            LogUtil.d(
                "Runtime",
                "switchSession: initial render for session $id result=$initialRender (attempts=$attempts)",
            )
            target.forceRenderRequested = true
            target.notifyRender()
        } catch (exception: Exception) {
            if (exception is kotlinx.coroutines.CancellationException) throw exception
            LogUtil.e(
                "Runtime",
                "switchSession: initial render failed for session $id",
                exception,
            )
        }

        // 阶段 3（加锁）：发布切换。
        synchronized(sessionLock) {
            // 重新校验：首帧渲染期间会话可能已关闭（用户关闭，或被监视器回收）。
            if (sessions[id] !== target) {
                LogUtil.w(
                    "Runtime",
                    "switchSession: session $id was closed during first frame, aborting switch",
                )
                return
            }
            // 我们渲染首帧期间，并发的 switchSession 可能发布了另一个活动会话。
            // 它的渲染线程正在运行；在启动我们的之前先停掉它，
            // 使任何时刻只有一个渲染线程消费共享事件队列
            // （两个消费者会错投剪贴板事件）。
            val concurrentToStop =
                concurrentRenderThreadToStop(
                    activeSessionIdAfterRender = activeSessionId,
                    previousActiveId = previousActiveId,
                    targetId = id,
                    concurrentSessionId = sessions[activeSessionId]?.id,
                )
            if (concurrentToStop != null) {
                val concurrent = sessions[concurrentToStop]
                if (concurrent != null) {
                    try {
                        renderSupervisor.stopRenderThread(concurrent)
                        LogUtil.w(
                            "Runtime",
                            "switchSession: stopped concurrent session $concurrentToStop (active changed during first frame)",
                        )
                    } catch (exception: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "switchSession: failed to stop concurrent session $concurrentToStop",
                            exception,
                        )
                    }
                }
            }
            // 只有启动渲染线程需要「失败即恢复前一个会话」：此刻目标会话的线程尚未
            // 起来，恢复不会撞上在跑的线程。其后的步骤（发布活动 id、native switchSession、
            // 网格对齐、focus 转发）失败时目标线程已经在消费全局事件队列，
            // 此时再拉起前一个会话会同时存在两个消费者——剪贴板事件会被错投。
            try {
                renderSupervisor.startRenderThread(target)
            } catch (exception: Exception) {
                LogUtil.e(
                    "Runtime",
                    "switchSession: failed to start render thread for session $id",
                    exception,
                )
                // 前一个活动会话已停止且其 GPU surface 已释放，恢复它，
                // 使终端不会被留在冻结状态（无渲染线程，且监视器会永远跳过
                // !running 的条目）。
                val previous = sessions[previousActiveId]
                if (previous != null && shouldRestorePreviousSession(previous.id, id)) {
                    LogUtil.w(
                        "Runtime",
                        "switchSession: restoring previous session ${previous.id} after failure",
                    )
                    previous.running = true
                    previous.renderThreadExited = false
                    previous.restartAttempts = 0
                    try {
                        renderSupervisor.startRenderThread(previous)
                        activeSessionId = previous.id
                    } catch (restoreException: Exception) {
                        LogUtil.e(
                            "Runtime",
                            "switchSession: failed to restore previous session ${previous.id}",
                            restoreException,
                        )
                    }
                }
                return@synchronized sessionLock
            }
            try {
                activeSessionId = id
                // 重新初始化光标/内容下沿滚动源：新会话的渲染线程从此刻起在变化时重新发布。
                cursorRowFlowInternal.value = target.cursorRow
                lastContentRowFlowInternal.value = target.lastContentRow
                // 清除上一个会话残留的逐像素滚动余量：原生视口偏移是全局的，
                // 故新会话必须从对齐状态开始
                // （其渲染线程也会在首帧转发零余量）。
                setScrollRemainderPx(0f)
                // 同步原生 ACTIVE_SESSION_ID，使 pollEvent/process_output 作用于新会话。
                // 不做此步，除首个会话外的所有会话会共用同一个原生活动会话，
                // 多会话的输出/退出检测将静默失效。
                try {
                    NativeBridge.switchSession(id)
                } catch (exception: Exception) {
                    LogUtil.e(
                        "Runtime",
                        "switchSession: native switchSession failed for session $id",
                        exception,
                    )
                }
                target.bridge?.let { syncGridDimensions(it) }
                // 尺寸一致跳过 resize：冗余 SIGWINCH 会清 mksh 提示符；
                // 查不到原生网格则无条件对齐（fail-open）。
                alignGridOnSwitch(target.bridge, _state.value.rows, _state.value.cols)
                LogUtil.d("Runtime", "switched to session $id")
                // DECSET 1004 焦点上报是按窗口的，新活动会话在后台期间
                // 从未收到 focus-in。重新发送最后已知的窗口焦点状态，
                // 使获得焦点的 TUI（vim/fzf）恢复其 FocusGained 行为。
                if (lastWindowFocus) {
                    target.bridge?.focusEvent(true)
                }
            } catch (exception: Exception) {
                // 目标会话的渲染线程此刻已在运行，故此处不做任何恢复：
                // 拉起前一个会话会产生第二个事件队列消费者。目标会话继续以
                // 刚发布的状态运行，网格对齐失败由上面的错误日志暴露。
                LogUtil.e(
                    "Runtime",
                    "switchSession: post-start step failed for session $id",
                    exception,
                )
            }
        }
    }

    /**
     * Activity 拆除路径（onDestroy）的唯一入口：无会话运行时停止前台服务，会话存活时刷新通知计数。
     * 把服务生命周期集中在此，而不是由 MainActivity 直接调用静态方法
     * （那会绕过运行期的 foregroundServiceRunning 标志及其异常保护）。
     *
     * 计数读取与停止决定都在 sessionLock 内完成：否则落在快照与停止之间的
     * createSession 会让一个存活会话失去其前台服务。
     *
     * 注意：若 onDestroy 在 runtime.start() 引导中途运行（尚未插入会话），本方法停止服务，
     * 而 start() 稍后重新插入会话并重启服务。这正是预期的后台会话语义
     * （termux 式：会话比 Activity 活得更久），不是泄漏
     * ——最终状态是一个存活会话 + 一个前台服务。只有当 start() 在服务重启之后
     * 失败才会泄漏，而其回滚（updateForegroundSessionCount(0)）已处理该情形。
     */
    fun stopForegroundServiceIfIdle() {
        synchronized(sessionLock) {
            if (sessions.isEmpty()) {
                stopForegroundService()
            } else {
                updateForegroundSessionCount(sessions.size)
            }
        }
    }

    fun closeSession(id: Long) {
        // 阶段 1（加锁）：立即捕获关闭资格并清除运行意图标志，与 handleSessionExit 对称
        // ——延迟重启（restartRenderThreadAfterDelay）在锁内检查 running，
        // 必须看到 false 才能取消。若稍后在不持锁状态下读 running，
        // 可能撞上 startRenderThread 瞬时的 running=false→true 窗口，
        // 把关闭误判为安全，而全新渲染线程正在启动
        // （孤儿线程、全局事件队列双重消费）。
        var wasRunning: Boolean = false
        val entry =
            synchronized(sessionLock) {
                val sessionEntry = sessions[id] ?: return
                wasRunning = sessionEntry.running
                sessionEntry.running = false
                sessionEntry.closing = true
                sessionEntry
            }
        LogUtil.d("Runtime", "closeSession($id)")

        // 停止任何会话的渲染线程（活动或非活动）。
        // running 与 renderThreadPossiblyAlive 是 @Volatile；条目引用在锁外仍然有效
        // （只有在我们最后一个引用消失时对象才会被丢弃）。
        var renderThreadStopped = true
        if (wasRunning) {
            renderThreadStopped = renderSupervisor.stopRenderThread(entry)
        } else if (entry.renderThreadPossiblyAlive) {
            // 先前的 join 在 running 仍为 true 时超时（GPU 挂起）；
            // 该标志在把 running 置假的暂停路径中依然存活。
            // 线程可能仍在原生渲染代码中，故这不是可安全关闭的状态。
            renderThreadStopped = false
        }
        // 关闭前在锁内重查（纯状态读取，不 join——此处 join 会持有 sessionLock
        // 最长 THREAD_JOIN_TIMEOUT_MS 并阻塞所有会话操作）。
        // 任何并发的 startRenderThread 都会被 entry.closing 拒绝（并重置 running=false），
        // 故此处存活的只可能是早于本次关闭的线程；
        // 改为把该会话标记为不可关闭（原生会话待进程消亡时回收）——安全且不阻塞。
        synchronized(sessionLock) {
            if (entry.renderThreadRef?.isAlive == true) {
                renderThreadStopped = false
            }
        }
        if (renderThreadStopped && !entry.renderThreadPossiblyAlive) {
            // 仅在确认渲染线程已死时才释放 GPU surface。
            // stopRenderThread 的 join 可能在当前线程上成功，
            // 而更早挂起（来自先前 join 超时、记录在 hungRenderThread 中）的线程
            // 仍在原生代码中存活——renderThreadPossiblyAlive 覆盖该情形。
            // 在此释放/关闭会在该线程恢复时构成 use-after-free。
            entry.bridge?.releaseGpuSurface()
            // 同样的道理适用于 close()：destroySession 会拆除挂起线程
            // 可能仍在触碰的原生会话/PTY/wgpu 上下文。
            // 原生会话待进程消亡时回收；在此销毁会在该线程恢复的瞬间构成 use-after-free。
            entry.bridge?.close()
        } else {
            LogUtil.e(
                "Runtime",
                "session $id render thread hung — skipping surface release AND bridge close to avoid use-after-free",
            )
        }

        // 阶段 3（加锁）：移除会话，若它是活动的则切换到接替会话。
        // 接替会话的挂起线程 join 与渲染重启留在锁内，与 handleSessionExit 对称
        // （它们仅在关闭活动会话的路径上运行）。
        synchronized(sessionLock) {
            if (!sessions.containsKey(id)) return
            sessions.remove(id)
            updateForegroundSessionCount(sessions.size)

            // 若关闭的是活动会话，切换到另一个
            if (id == activeSessionId) {
                val remaining = sessions.keys.sorted()
                if (remaining.isNotEmpty()) {
                    val newId = remaining.last()
                    activeSessionId = newId
                    activateReplacementSession(
                        newId,
                        "closeSession",
                        markRunning = true,
                        withRetry = false,
                        syncGrid = true,
                    )
                } else {
                    activeSessionId = 0L
                }
            }
            updateState()
        }
    }

    suspend fun applySettings() {
        val config = buildConfig()
        val fontFamily = settingsRepository.fontFamily.first()
        val effectiveFontFamily = terminal.emulator.resolveEffectiveFontFamily(fontFamily)
        // 只换字体/字号/主题，不 resize 各会话网格：buildConfig() 的默认 24x80 会
        // 缩小存活的 PTY（vim/htop 收到多余的 SIGWINCH 并重排）。
        sessions.values.forEach { entry ->
            entry.bridge?.setFontSizeInPlace(config.fontSizeTenths)
            entry.bridge?.setFontFamily(effectiveFontFamily)
            entry.bridge?.setTheme(config.theme)
            entry.notifyRender()
        }
        // 字体度量已变——但网格尺寸保持不变。
        // 内容超出可视区域时终端自行滚动。
    }

    /**
     * 按当前原生字体度量重算活动会话的网格：rows = (surface - ModifierBar) / cell_height，
     * cols = surface / cell_width。Surface 尺寸或度量尚未知时为空操作
     * （attach 之前就应用了字体）。每次字号变更与初始字体应用后调用。
     */
    private fun recomputeGridFromFontMetrics() {
        val barHeightPx = modifierBarHeightPx
        // Surface 尺寸：在首次 attachSurface 落地之前，pendingSurface
        // （由 startRuntime 设置）是权威来源。一次取出三元组：分开两次读可能跨过
        // 一次并发移交，把 1080x2400 与 1080x2340 拼成一个从未存在过的矩形，
        // 由此算出的网格会以 SIGWINCH 推给 PTY，却与任何 Surface 都不匹配。
        val pending = pendingSurface
        val surfaceW = pending?.width ?: 0
        val surfaceH = pending?.height ?: 0
        val currentRows = _state.value.rows.coerceAtLeast(1)
        val currentCols = _state.value.cols.coerceAtLeast(1)
        if (surfaceW <= 0 || surfaceH <= 0 || cellWidth <= 0f || cellHeight <= 0f) return
        // 由物理 surface / 物理单元格度量计算网格尺寸。
        // 两者都是物理像素（已按密度缩放）。ModifierBar 覆盖 Surface 底部，
        // 故计算 rows 之前先减去其高度。
        val (newRows, newCols) =
            computeGridDimensions(
                surfaceWidth = surfaceW,
                surfaceHeight = surfaceH - barHeightPx,
                cellWidth = cellWidth,
                cellHeight = cellHeight,
            )
        if (newRows == 0 || newCols == 0) {
            // 退化几何（可用高度 ≤ 0，例如 Surface 不高于 ModifierBar）：
            // 保持当前网格，而不是把它塌缩为一行。
            return
        }
        if (newRows == currentRows && newCols == currentCols) return
        LogUtil.d(
            "Runtime",
            "recomputeGridFromFontMetrics: ${currentRows}x$currentCols -> ${newRows}x$newCols " +
                "(cell ${cellWidth}x$cellHeight, surface ${surfaceW}x$surfaceH)",
        )
        // 只 resize 活动会话：后台会话保留各自的网格尺寸；
        // 用活动会话的尺寸 resize 所有会话会对它们触发多余的 SIGWINCH 与重排。
        sessions[activeSessionId]?.bridge?.resize(newRows, newCols)
        _state.update { it.copy(rows = newRows, cols = newCols) }
    }

    /**
     * 应用字体设置，返回 native 应用结果：true 全会话成功，false 任一失败，
     * null 无会话可验证（调用方此时不得清除设置）。
     */
    suspend fun applyFontSettings(): Boolean? {
        val fontSizeTenths = computeFontSizeTenths()
        appliedFontSizeTenths = fontSizeTenths
        val fontFamily = settingsRepository.fontFamily.first()
        val effectiveFontFamily = terminal.emulator.resolveEffectiveFontFamily(fontFamily)
        LogUtil.d(
            "Runtime",
            "applyFontSettings: fontFamily='$fontFamily' effective='$effectiveFontFamily' fontSizeTenths=$fontSizeTenths sessions=${sessions.size}",
        )
        var applied: Boolean? = null
        sessions.values.forEach { entry ->
            try {
                val familyResult = entry.bridge?.setFontFamily(effectiveFontFamily)
                LogUtil.d("Runtime", "setFontFamily result: $familyResult")
                if (familyResult != null) {
                    applied = (applied ?: true) && familyResult
                }
                entry.bridge?.setFontSizeInPlace(fontSizeTenths)
                entry.bridge?.let { syncGridDimensions(it) }
                // 网格必须随字体变化。syncGridDimensions 只读取既有的原生网格
                // （重启后仍是默认的 80x24）；若不按 Surface 与新字体度量重算 rows/cols，
                // 渲染器会按 surface/80 x surface/24（13.5x92）布置网格，
                // 而字形却按 41.2x81.4 的单元格光栅化——字体显得巨大且行重叠。
                // 重算并 resize 活动会话。
                recomputeGridFromFontMetrics()
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "applyFontSettings failed for session", exception)
                applied = false
            }
        }
        return applied
    }

    /**
     * 输入的目标会话 id：调用方在用户事件发生时取一次，随字节传到这里。
     *
     * 输入字节可能在别的线程刷写（输入法批缓冲的帧回调与 PtyWriter 线程），
     * 那时再解析 `sessions[activeSessionId]` 会把粘贴尾部写进用户刚切换到的新会话。
     */
    val inputTargetSessionId: Long get() = activeSessionId

    fun writeToPty(sessionId: Long, data: ByteArray): Boolean {
        val entry = sessions[sessionId]
        // 受理判据是「会话是否还活着」而不是 running（渲染意图标志）：Surface 销毁会经
        // pauseRendering 把 running 置假，而 PTY 子进程此刻仍然活着。按 running 拒绝等于
        // 在每次 surface 销毁→重建的空窗期静默吞掉击键，而恢复本身是异步的
        // （surfaceTransitionExecutor）——切回前台立刻打字就会丢字。
        if (entry != null && !entry.closing) {
            // shell 已退出且正在显示 [Process completed]
            // ——唯一被接受的输入是 Enter，它确认提示并让渲染循环执行关闭路径。
            if (entry.waitingForProcessCompleted) {
                if (data.any { it == '\r'.code.toByte() || it == '\n'.code.toByte() }) {
                    synchronized(sessionLock) {
                        entry.processCompletedConfirmed = true
                    }
                    entry.renderSignaled.set(true)
                    entry.notifyRender()
                }
                return true
            }
            val written = entry.bridge?.writeToPty(data) ?: false
            if (written) {
                // 延迟探针的输入打点（用 elapsed-realtime 时钟：它能跨深度睡眠存活，
                // 而 nanoTime 的单调基准不能）。
                entry.latencyProbe.onInputWritten(SystemClock.elapsedRealtimeNanos())
            }
            entry.notifyRender()
            return written
        }
        LogUtil.w("Runtime", "writeToPty: 会话 $sessionId 不可写（不存在或正在关闭）")
        return false
    }

    /** 字节直接送入 VT 解析器（注入转义序列的测试路径）。 */
    fun feedTerminal(data: ByteArray): Boolean {
        val entry = sessions[activeSessionId] ?: return false
        return entry.bridge?.feedTerminal(data) ?: false
    }

    fun bridge(): Bridge? = sessions[activeSessionId]?.bridge

    /**
     * 活动会话的输入→回显延迟汇总。样本数 N<30 时为 `NOT MEASURED`
     * ——调用方必须原样呈现，而不得编造数字。
     */
    fun latencyReport(): String = sessions[activeSessionId]?.latencyProbe?.report() ?: "latency NOT MEASURED n=0"

    @Volatile private var lastWindowFocus: Boolean = false

    fun focusChange(focused: Boolean) {
        lastWindowFocus = focused
        // 焦点上报（DECSET 1004）按窗口生效：只有活动会话会收到。
        // 向每个会话广播会让单次窗口焦点变化执行 N 次同步 JNI RPC。
        val entry = sessions[activeSessionId] ?: return
        val bridge = entry.bridge ?: return
        // 不在主线程做：原生 focus_event 先经 VT 线程做 1004 模式查询（最长 50ms）
        // 再写 PTY，两者都在会话锁内。此前在 UI 线程同步调用，VT 线程一旦卡住
        // （大输出、GC）每次窗口焦点变化都会把主线程堵满 50ms——正是掉帧的形状。
        // 焦点顺序由单线程执行器保持，与键盘事件进入 PTY 的顺序一致（xterm 语义：
        // 焦点上报与按键的先后须与用户操作顺序相符）。
        scope.launch(terminal.emulator.util.TerminalDispatchers.inputOutput) {
            bridge.focusEvent(focused)
        }
    }

    fun pauseRendering() {
        // stopRenderThread 会 join 渲染线程（每会话最长 THREAD_JOIN_TIMEOUT_MS）；
        // 在主线程上（Surface 销毁）且有 3 个以上会话时可能超出 5s 的 ANR 阈值，
        // 故在主线程之外执行。surfaceDestroyed 立即返回，渲染只是停止。
        //
        // 暂停与恢复都走同一个单线程执行器，以保持 surface-destroy → surface-available
        // 的顺序：否则异步暂停可能停掉同步恢复刚启动的渲染线程
        // （固定尺寸设备旋转）。
        //
        // 注意：每会话的 join 发生在 sessionLock 内，故有多个会话时主线程经
        // sessionLock 的操作（onDestroy → stopForegroundServiceIfIdle）会按总和停顿。
        // 该代价仅在渲染线程确实挂起时出现（join 立即返回则无停顿）；
        // 与 switchSession 锁内 join 是同一取舍，故不在此另立策略。
        surfaceTransitionExecutor.execute {
            synchronized(sessionLock) {
                sessions.values.forEach { entry ->
                    if (entry.running) {
                        renderSupervisor.stopRenderThread(entry)
                        entry.running = false
                        LogUtil.d("Runtime", "pauseRendering: session ${entry.id} stopped")
                    }
                }
            }
        }
    }

    fun resumeRendering() {
        surfaceTransitionExecutor.execute {
            synchronized(sessionLock) {
                // 恢复即重置换视图预算：ON_RESUME 之后平台会重新交付 Surface，
                // 这正是一次全新的机会。此前预算只在「渲染成功」时清零，而切后台
                // 往返期间预算会在过渡抖动里被 5 次请求迅速耗尽（实测 2.5s 内耗尽，
                // 随后每次都停在 exhausted），真正的换视图要等到 ~20s 后的
                // surfaceCreated 才发生——于是终端长时间空白。
                //
                // 保留原有「恢复渲染时清零」也无妨，两处语义一致。
                sessions.values.forEach { entry ->
                    entry.surfaceInvalidated = false
                    entry.surfaceRecreateBudget.set(SessionEntry.SurfaceRecreateBudget())
                }
                // 只有活动会话渲染（见 switchSessionInternal）；
                // 为每个会话启动线程会创建单一全局原生事件队列的多个消费者，
                // 退出事件可能因此被错误的会话线程处理（关闭掉无辜的会话）。
                val activeEntry = sessions[activeSessionId]
                if (activeEntry != null && !activeEntry.running && activeEntry.bridge != null) {
                    try {
                        activeEntry.running = true
                        renderSupervisor.startRenderThread(activeEntry)
                        LogUtil.d("Runtime", "resumeRendering: session ${activeEntry.id} restarted")
                    } catch (exception: Exception) {
                        LogUtil.e("Runtime", "resumeRendering failed for session ${activeEntry.id}", exception)
                    }
                }
                // 在锁内：在此串行化意味着监视器先于任何并发关闭路径的
                // stopRenderMonitor 启动并被其取消
                // ——绝不会在拆除完成后监视器又被复活。
                renderSupervisor.startRenderMonitor()
            }
        }
    }

    fun setSelection(startRow: Int, startCol: Int, endRow: Int, endCol: Int, hasSelection: Boolean) {
        LogUtil.d(
            "Runtime",
            "setSelection: start=($startRow,$startCol) end=($endRow,$endCol) active=$hasSelection",
        )
        // 全量快照覆写：dragging 在此被重置为 false
        // ——每条提交路径（endSelection/clearSelection/syncSelectionToNative）
        // 都汇入本调用，故已结束的拖动总能清除 dragging 保护。
        selectionState.set(
            SelectionStateSnapshot(startRow, startCol, endRow, endCol, hasSelection),
        )
        val entry = sessions[activeSessionId]
        entry
            ?.bridge
            ?.setSelection(startRow, startCol, endRow, endCol, hasSelection)
        entry?.notifyRender()
    }

    /**
     * 调整活动会话的终端网格尺寸。值在 bridge 调用之前钳位到原生 u16 范围（1..=65535），
     * 使 PTY 网格尺寸与 UI 状态保持一致。
     */
    fun resize(rows: Int, cols: Int) {
        val entry = sessions[activeSessionId] ?: return
        // 在 bridge 调用之前钳位，使原生（PTY 网格尺寸）与 UI 状态永不分歧：
        // 0 在原生上是合法尺寸但会破坏别处的网格计算，
        // 而原生 resize 拒绝任何 >u16 的值（否则 Kotlin 状态会携带一个
        // PTY 静默拒绝的值）。此钳位是唯一的端到端守卫。
        // 当原生网格命令被丢弃（ResizeOutcome::Dropped）时，
        // 本状态持有「请求的」尺寸而原生保留旧的缓存尺寸；
        // 该分歧会在下次 resize 事件自愈，且一旦 getGridRowsColsPacked 真实可用，
        // syncGridDimensions 会把本状态改回原生值。
        // 注意上界是 u16 协议限制，而非显示尺寸的合理性限制：
        // UI 路径（window insets、applySettings）提供的是真实网格尺寸，
        // 故 65535×65535 的网格只可能由直接 API 调用者请求；原生会尝试分配。
        val clampedRows = rows.coerceIn(1, U16_MAX)
        val clampedCols = cols.coerceIn(1, U16_MAX)
        entry.bridge?.resize(clampedRows, clampedCols)
        // CAS：普通 copy 会覆盖渲染线程在读与写之间发布的 title 更新。
        _state.update { it.copy(rows = clampedRows, cols = clampedCols) }
        entry.notifyRender()
    }

    /**
     * 更新活动会话 PTY winsize 的像素字段（ws_xpixel/ws_ypixel）而保持 rows/cols。
     * 每次网格 resize 时以终端 Surface 的像素尺寸一同调用，
     * 使感知像素的程序（icat、全屏 TUI）经 TIOCGWINSZ 读到真实像素而非 0。
     * 值按原生 u16 范围钳位，同 [resize]；两者都为 0 是合法的（清空字段）
     * 且 ioctl 会成功，故无需下界守卫。
     */
    fun setPixelSize(widthPx: Int, heightPx: Int) {
        val entry = sessions[activeSessionId] ?: return
        entry.bridge?.setPixelSize(widthPx.coerceIn(0, U16_MAX), heightPx.coerceIn(0, U16_MAX))
    }

    /**
     * 按当前 Surface 尺寸与原生字体单元格度量重算 rows/cols，并把权威网格同步到会话状态。
     *
     * 不做此步，重启后重新绑定 Surface 会保留引导期的 24x80 网格，
     * 而字形却按配置的（更大）尺寸渲染：提示符被截断
     * （"/home/com.ter" 而非完整路径）且换行失效。
     */
    fun recomputeGrid() {
        val bridge = sessions[activeSessionId]?.bridge ?: return
        syncGridDimensions(bridge)
        recomputeGridFromFontMetrics()
    }

    /**
     * 把 Android Surface 交给渲染器。若会话 bridge 尚不存在（spawn 进行中），
     * 则保留为待绑定，会话启动后立即绑定。
     */
    fun attachSurface(surface: android.view.Surface, width: Int, height: Int) {
        pendingSurface = PendingSurface(surface, width, height)
        val bridge = sessions[activeSessionId]?.bridge
        if (bridge != null) {
            bridge.attachSurface(surface, width, height)
        }
    }

    private fun attachPendingSurface(bridge: terminal.emulator.bridge.Bridge) {
        // 一次取出三元组：读两次之间可能有并发移交，尺寸必须与该 Surface 同源。
        val pending = pendingSurface ?: return
        // bridge spawn 期间持有者可能已被销毁
        // （onSurfaceDestroyed 会清空该字段，但竞争的读取仍可能
        // 观察到陈旧值）——绝不绑定已死的 Surface。
        if (!pending.surface.isValid) {
            pendingSurface = null
            return
        }
        bridge.attachSurface(pending.surface, pending.width, pending.height)
        // 网格必须以真实 Surface 为准匹配字体单元格度量。
        // attachSurface 使 Surface 尺寸成为权威；syncGridDimensions 取来原生字体单元格度量，
        // 随后 recomputeGridFromFontMetrics resize 网格，使渲染器的
        // 四边形（surface/rows x surface/cols）匹配字形光栅尺寸
        // （字体单元格 x 密度）。不做此步，网格会停留在默认的 80x24，
        // 而字形却按配置字号光栅化——每次重启后字体显得巨大且行重叠。
        syncGridDimensions(bridge)
        recomputeGridFromFontMetrics()
    }

    /**
     * 在前一个会话关闭后把 [newId] 激活为前台会话：挂起线程的最终 join、
     * 原生 ACTIVE_SESSION_ID 同步（switchSession）、渲染线程重启与焦点重发。
     *
     * 由 handleSessionExit、closeDeadSession 与 closeSession 共用，
     * 使这段持锁序列只存在于一处而非三份漂移的副本。
     *
     * 必须在持有 sessionLock 时调用；调用方已移除正在关闭的条目并设置
     * activeSessionId = [newId]。
     *
     * @param caller 日志前缀（如 "handleSessionExit"）
     * @param markRunning 先把 replacement.running 置真（closeSession 需要；退出路径本就以运行中状态执行）
     * @param withRetry JNI 失败时重试 switchSession 一次（退出路径；用户关闭跳过重试以限制延迟）
     * @param syncGrid 在渲染启动后对接替会话调用 syncGridDimensions（仅 closeSession）
     */
    private fun activateReplacementSession(
        newId: Long,
        caller: String,
        markRunning: Boolean,
        withRetry: Boolean,
        syncGrid: Boolean,
    ) {
        val replacement =
            sessions[newId]
                ?: run {
                    LogUtil.w("Runtime", "$caller: new active session $newId already removed")
                    activeSessionId = 0L
                    updateState()
                    return
                }
        if (markRunning) {
            replacement.running = true
        }
        val bridge =
            replacement.bridge
                ?: run {
                    LogUtil.w("Runtime", "$caller: new active session $newId has no bridge")
                    activeSessionId = 0L
                    updateState()
                    return
                }
        // 挂起线程的最终 join：若它在此期间已退出，就清标志，
        // 使后续 close() 能销毁原生会话（不泄漏）。
        // 防止 join 自身：退出路径在渲染线程上运行（poll.exit），
        // 而在回收后台会话的情况下任何会话的渲染线程都可能到达此处
        // ——包括接替会话自己的（若它先前被记为挂起）。
        // join 自身必然超时，会使所有会话操作冻结满整个超时时间。
        val hung = replacement.hungRenderThread
        if (
            replacement.renderThreadPossiblyAlive &&
            hung != null &&
            hung !== Thread.currentThread() &&
            hung.isAlive
        ) {
            hung.interrupt()
            hung.join(THREAD_JOIN_TIMEOUT_MS)
        }
        if (
            replacement.renderThreadPossiblyAlive &&
            hung != null &&
            hung !== Thread.currentThread() &&
            !hung.isAlive
        ) {
            replacement.renderThreadPossiblyAlive = false
            replacement.hungRenderThread = null
        }
        // 同步原生 ACTIVE_SESSION_ID，使 pollEvent/process_output 驱动接替会话。
        // destroySession 只把原生活动 id 清为 0；没有显式的 switchSession，
        // 接替会话将永远不被轮询，输出保持冻结。
        // 注意：即使旧渲染线程仍存活也照常执行
        // （它正在退出：running=false 会使循环结束）——跳过会让原生 active=0
        // 并冻结接替会话。
        var nativeSwitched =
            try {
                NativeBridge.switchSession(newId)
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "$caller: native switchSession failed", exception)
                if (withRetry) {
                    // 重试一次：瞬时 JNI 失败会让原生活动 id 陈旧，
                    // 接替会话降级到后台清扫速率（2 块/帧）。
                    try {
                        val retried = NativeBridge.switchSession(newId)
                        if (retried) {
                            LogUtil.w("Runtime", "$caller: native switchSession recovered on retry")
                        }
                        retried
                    } catch (retryException: Exception) {
                        LogUtil.e("Runtime", "$caller: native switchSession retry failed", retryException)
                        false
                    }
                } else {
                    false
                }
            }
        if (!nativeSwitched) {
            // 三条路径共用的守卫：原生会话已消失（被并发关闭销毁）。
            // 启动渲染线程会对着缺失的原生会话陷入错误循环；
            // 且该条目本就会被那条关闭路径移除。
            // 重置意图标志并刷新 UI 状态，使期间没有任何东西观察到
            // 一个「运行中但已死」的接替会话。closing=true 是防御性的：
            // 若原生侧语义日后变化（新增销毁路径），
            // 该条目不会变成被监视器永远跳过的僵死僵尸。
            LogUtil.w(
                "Runtime",
                "$caller: native switchSession returned false for session $newId — skipping render start",
            )
            replacement.running = false
            replacement.closing = true
            updateState()
            return
        }
        // 无条件重启：仍存活的旧线程正在退出；
        // startRenderThread 会 interrupt+join 它并强制换上一个新线程。
        renderSupervisor.startRenderThread(replacement)
        if (syncGrid) {
            bridge.let { syncGridDimensions(it) }
        }
        // 接替会话在窗口获得焦点时成为活动会话；
        // 重发 focus-in 使 DECSET 1004 的 TUI 恢复。
        if (lastWindowFocus && !replacement.closing) {
            bridge.focusEvent(true)
        }
        if (replacement.closing) {
            // 并发的 closeSession/closeDeadSession 赢得了接替会话的竞争；
            // startRenderThread 已拒绝（closing 标志）并把 running 重置为 false。
            // 该条目将由那条关闭路径移除。
            LogUtil.d("Runtime", "$caller: replacement session $newId is closing — render start skipped")
        } else {
            LogUtil.d("Runtime", "$caller: restarted render for new active session $newId")
        }
    }

    private fun syncGridDimensions(bridge: Bridge) {
        val packed = bridge.getGridRowsColsPacked()
        val rows = (packed shr 32).toInt()
        val cols = packed.toInt()
        // 只用有效值覆写单元格度量：当前 bridge 返回 0f/0（桩），
        // 写入这些值会覆盖按 Surface 计算出的真实度量。
        // 原生度量以逻辑像素（字体管线单位）给出；
        // 而 TerminalSurface 中的触摸/锚点计算以物理像素进行，
        // 故在此按密度缩放——这修掉了长按命中测试落到错误单元格
        // 以及字号不匹配的反馈。
        val density = context.resources.displayMetrics.density
        val rawCellWidth = bridge.getCellWidth()
        val rawCellHeight = bridge.getCellHeight()
        // 逻辑像素尺寸（用于网格计算）：原生原始值
        if (rawCellWidth > 0f) logicalCellWidth = rawCellWidth
        if (rawCellHeight > 0f) logicalCellHeight = rawCellHeight
        // 物理像素尺寸（用于渲染/触摸）：已按密度缩放
        val newCellWidth = rawCellWidth * density
        val newCellHeight = rawCellHeight * density
        val hadCellMetrics = cellWidth > 0f && cellHeight > 0f
        if (newCellWidth > 0f) cellWidth = newCellWidth
        if (newCellHeight > 0f) cellHeight = newCellHeight
        // 单元格度量首次可用时必须重算网格：启动序列里 [attachPendingSurface] 的这次
        // 同步可能早于 native 字体度量就绪，[recomputeGridFromFontMetrics] 会因
        // cellWidth/cellHeight == 0 提前返回，而此后没有任何路径重试——网格会永久停在
        // spawn 默认 24×80（屏幕下半部空白），直到旋转等外部尺寸事件。
        if (!hadCellMetrics && cellWidth > 0f && cellHeight > 0f) {
            recomputeGridFromFontMetrics()
        }
        // CAS，尺寸检查置于 lambda 内部：旧代码在 update 之外读 rows/cols，
        // 并发的 title CAS 可能落在检查与写入之间。
        // rows/cols 是 CAS 之前从 bridge 读到的快照：重试时它们可能用略陈旧的尺寸
        // 覆盖更新的尺寸——这是「CAS 外读取」的固有特性，下次同步会自愈，
        // 且严格窄于旧的在锁外读-检查-写。
        _state.update { previous ->
            if (rows > 0 && cols > 0 && (rows != previous.rows || cols != previous.cols)) {
                previous.copy(rows = rows, cols = cols)
            } else {
                previous
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // 四、状态与 Surface 生命周期
    // ══════════════════════════════════════════════════════════════════════

    private fun updateState() {
        val currentTitle =
            sessions[activeSessionId]?.bridge?.getActiveSessionTitle() ?: _state.value.title
        // 用 _state.update（CAS）而非读-改-写赋值：渲染线程的 title CAS
        // 可能落在我们的读与写之间，被陈旧 title 覆盖
        // （下个周期会自愈，但 CAS 完全避免该回归）。
        // lambda 内的映射读取走 ConcurrentHashMap（弱一致，无需加锁）；
        // 持有 sessionLock 的调用方额外获得与会话创建/关闭的结构性串行化。
        // currentTitle 本身是 CAS 之前的快照读取：两个并发的 updateState 调用
        // 仍可能用陈旧 title 覆盖更新的 title
        // （与 syncGridDimensions 同属「CAS 外读取」一类，会自愈）。
        _state.update { previous ->
            previous.copy(
                isRunning = sessions.isNotEmpty(),
                title = currentTitle.ifEmpty { previous.title },
                activeSessionId = activeSessionId,
                sessionIds = sessions.keys.sorted(),
            )
        }
    }

    fun onSurfaceDestroyed() {
        // 待绑定的 Surface（bridge 存在之前的 start/attachSurface）
        // 在持有者销毁的瞬间即已陈旧——稍后绑定会把已死的 Surface 交给新 bridge
        // 并渲染出黑帧。整条记录（含尺寸）一并清空，否则重算网格会沿用已销毁尺寸。
        pendingSurface = null
        setRenderPaused(true)
    }

    /**
     * 切换所有会话的原生渲染器暂停标志。该标志位于单一的全局渲染器上
     * （ffi.rs setRenderPaused），故暂停任一会话都会暂停共享的 GPU 管线。
     *
     * 该标志必须在 Surface 恢复路径（TerminalSurface.surfaceCreated）上清除
     * ——没有别处会做这件事：只要它保持置位，render_frame 就以 Ok() 短路，
     * 恢复后的会话会渲染出纯黑帧，而渲染线程看起来完全健康（无错误、无重启）。
     */
    fun setRenderPaused(paused: Boolean) {
        for (entry in sessions.values) {
            entry.bridge?.setRenderPaused(paused)
        }
    }

    fun releaseAllGpuSurfaces() {
        // Surface 销毁：干净地暂停渲染线程，而不是把它们标记为死亡。
        // 在存活线程上标记 renderThreadExited 会让监视器视其为已崩溃，
        // 并在 RENDER_MAX_RESTART_ATTEMPTS 之后（约 7s：6 轮监视间隔与退避之和）关闭这些会话。
        pauseRendering()
    }

    /**
     * 把 [action] 排入 pauseRendering 期间停止渲染线程所用的同一个单线程执行器。
     * 调用方借此只在渲染线程的 join 确认其已不在原生渲染代码中之后
     * 才释放 Surface/视图——在渲染线程仍在使用时释放 ANativeWindow 即 use-after-free。
     */
    fun runAfterRenderThreadsStopped(action: () -> Unit) {
        surfaceTransitionExecutor.execute {
            synchronized(sessionLock) {
                action()
            }
        }
    }
}

/**
 * [file] 是否以 ELF 魔数（0x7f 'E' 'L' 'F'）开头。用于在 prefix shell 解析中排除 shebang 脚本：
 * linker-wrapper 的 spawn 路径只能加载真正的 ELF 二进制。
 */
internal fun isElf(file: java.io.File): Boolean = try {
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
    LogUtil.w("Runtime", "isElf: cannot read ${file.absolutePath}", exception)
    false
}

/**
 * 系统解释器启动脚本判定：首行 shebang 指向系统路径（`/system/bin/`）的脚本可由应用进程直接执行， 无需经 linker
 * 桥接；指向应用私有目录的脚本不计入（其解释器本身尚不可用）。 与安装器/启动路径共享，供通用 bootstrap 启动器脚本使用，不针对特定发行版。
 */
internal fun isSystemShellScript(file: java.io.File): Boolean = try {
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
    LogUtil.w("Runtime", "isSystemShellScript: cannot read ${file.absolutePath}", exception)
    false
}

/** 下次死渲染线程重启的退避延迟：把先前延迟翻倍，上限 [maxDelayMs]（handleDeadRenderThread 中的指数退避）。 */
internal fun nextRestartDelayMs(currentMs: Long, maxDelayMs: Long): Long = (currentMs * 2).coerceAtMost(maxDelayMs)

/** 死亡渲染重启预算：尝试计数超过 [maxAttempts] 即关闭会话而非再次重启。 */
internal fun shouldCloseDeadRender(restartAttempts: Int, maxAttempts: Int): Boolean = restartAttempts > maxAttempts

/**
 * 初始同步渲染重试（switchSession）：只要首帧渲染结果为失败（< 0）
 * 且尝试计数仍低于 [maxAttempts] 就继续重试。纯决策，从重试循环中抽出以便测试。
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

// surface 重建请求的最小间隔（纳秒）：一次重建含拆装视图 + 新 surface 交付 + 首帧，
// 短于此的重试只会连累在途的重建。
private const val SURFACE_RECREATE_MIN_INTERVAL_NANOS = 500_000_000L

// 单会话连续重建请求上限：超过即停止并告警（重建无望时避免无限拆装视图）。
private const val SURFACE_RECREATE_MAX_ATTEMPTS = 5

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
    return kotlin.math.round((contentRow + 1) * cellHeightPx).toInt()
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
            0L
        }
    return packed == 0L || (packed shr 32).toInt() != wantRows || packed.toInt() != wantCols
}

/**
 * 切会话的网格对齐执行：`[shouldAlignGridOnSwitch]` 为真才 `resize`。
 *
 * 单独成顶层函数只因 `TerminalRuntime` 已顶满 `LargeClass` 阈值（基线恰 1600，
 * 类内多 1 个 token 行即挂）：调用点保持一行，判定逻辑可单测。
 */
internal fun alignGridOnSwitch(bridge: Bridge?, rows: Int, cols: Int) {
    val wantRows = rows.coerceAtLeast(1)
    val wantCols = cols.coerceAtLeast(1)
    if (shouldAlignGridOnSwitch(wantRows, wantCols) { bridge?.getGridRowsColsPacked() ?: 0L }) {
        bridge?.resize(wantRows, wantCols)
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
 * @param contentBottomPx 内容下沿像素（视口最后一个有内容的行的下沿，视口全空为 0）
 * @param surfaceHeightPx Surface 布局高度（容器高度，键栏覆盖其底部）
 * @param modifierBarHeightPx 键栏高度（网格已按同一口径预留）
 * @param imeBottomPx 键盘遮挡高度（已扣除被 `navigationBarsPadding` 消费的系统导航条）
 *
 * 上界取 `imeBottomPx`：位移超过键盘高度会在键盘上方留下一段终端背景空隙。
 * 网格已保证内容下沿不超过网格高度，故该上界在正常路径上恒不生效，
 * 只在字号变化瞬间（行数尚未随新行高重算）收敛位移。
 */
internal fun computeImeSurfaceShift(
    contentBottomPx: Int,
    surfaceHeightPx: Int,
    modifierBarHeightPx: Int,
    imeBottomPx: Int,
): Int {
    val visibleContentPx = surfaceHeightPx - modifierBarHeightPx - imeBottomPx
    return (contentBottomPx - visibleContentPx).coerceIn(0, imeBottomPx)
}

/**
 * surface 重建请求的纯决策（[maybeRequestSurfaceRecreate] 的判据）。
 *
 * 失效位在原生侧重建成功后回落为 0，故「判死后未回落」本身即是边沿；间隔限流
 * 覆盖重建无望的情形（窗口始终换不到），次数上限兜住无限拆装视图的抖动与耗电。
 *
 * @param attempts 已发出的请求次数
 * @param lastRequestNanos 上次请求时刻（纳秒，0 = 从未请求）
 * @param nowNanos 当前时刻（纳秒）
 */
internal fun decideSurfaceRecreate(attempts: Int, lastRequestNanos: Long, nowNanos: Long): SurfaceRecreateDecision {
    if (attempts >= SURFACE_RECREATE_MAX_ATTEMPTS) {
        return SurfaceRecreateDecision(exhausted = true)
    }
    val intervalElapsed = lastRequestNanos == 0L || nowNanos - lastRequestNanos >= SURFACE_RECREATE_MIN_INTERVAL_NANOS
    return SurfaceRecreateDecision(request = intervalElapsed)
}

/**
 * [decideSurfaceRecreate] 的结论：[request] = 本帧请求换新的原生窗口，
 * [exhausted] = 次数上限已用尽（调用方据此只告警一次）。
 */
internal data class SurfaceRecreateDecision(val request: Boolean = false, val exhausted: Boolean = false)
