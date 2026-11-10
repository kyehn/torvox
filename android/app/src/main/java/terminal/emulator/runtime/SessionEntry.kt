package terminal.emulator.runtime

import terminal.emulator.bridge.Bridge
import terminal.emulator.monitor.FrameMarks
import terminal.emulator.monitor.RenderWatchDog
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

/**
 * 会话状态由两个布尔量编码：running 且未退出 = 存活；running 且已退出 = 死亡（待清理）；
 * running 为假 = 已停止（陈旧条目，跳过）。renderThreadExited 由渲染线程在循环退出后置位，
 * 须与 running 一同在 sessionLock 下读取。
 */
internal data class SessionEntry(
    val sessionId: Long,
    // 构造时即绑定、此后不再重新赋值：非空类型让「会话已建而 bridge 未就绪」这一
    // 假想状态在类型上不可表达，故不存在恒假的 null 检查。
    val bridge: Bridge,
    // 无锁读取（notifyRender/pokeVsync 在锁外解引用唤醒），与相邻字段同为易变。
    @Volatile var renderThreadRef: Thread?,
    @Volatile var running: Boolean,
    @Volatile var renderThreadExited: Boolean = false,
    @Volatile var restartAttempts: Int = 0,
    // 与 decayRestartCounts/confirmRestartGrace 的重置值同源（INITIAL_RESTART_DELAY_MS），
    // 故新会话从与「健康期后」完全相同的延迟起步。
    @Volatile var nextRestartDelayMs: Long = INITIAL_RESTART_DELAY_MS,
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
    // 手指滚动手势**不**写这里：那是一次性意图，抬手即失效；写这里会清掉用户仍然
    // 点亮的 SCROLL 按钮锁（按钮仍在渲染为锁定，新输出却已经把视口拽到底）。
    @Volatile var scrollActive: Boolean = false,
    // 手指滚动进行中：同样抑制新输出引起的滚动复位，但抬手即失效，与按钮锁互不干扰。
    @Volatile var scrollGestureActive: Boolean = false,
    // 最近一次 UI 线程滚动手势的时间戳（System.nanoTime，在 setScrollOffset 中写入），
    // 渲染线程据此计算 recentlyScrolled 保护；0L 表示从未滚动过。
    @Volatile var lastScrollNanos: Long = 0L,
    // 最近一次 Choreographer 显示帧的时间戳（System.nanoTime）。渲染循环据此判断
    // vsync 链是否健康：健康时活跃闭锁只作安全网，绝不能同时充当第二路帧节拍
    // （两路节拍相邻会在一帧内推出两帧，交换链排满后 acquire 阻塞 ~70ms）。
    @Volatile var lastVsyncNanos: Long = 0L,
    // shell 已退出且 [Process completed (code X)] 提示已送入终端（见 feedProcessCompletedPrompt）。
    // 会话保持可见与运行，直到用户按 Enter。
    @Volatile var waitingForProcessCompleted: Boolean = false,
    // 显示 [Process completed] 提示时捕获的退出码；Enter 确认关闭时复用。
    /**
     * 退出会话缓存的退出码；`null` 表示原生未能取得（`waitpid` 失败），**不是** 0。
     *
     * 用户在 `[Process completed]` 提示上回车确认关闭时，渲染循环要把缓存的退出码
     * 再交给 `handleSessionExit` 一次；可空类型让「码未知」在这一跳里也不会退化成 0。
     */
    @Volatile var processExitCode: Int? = null,
    // 用户在提示上按 Enter 时由 writeToPty 置位。渲染循环侦测到后重新派发 handleSessionExit，
    // 使 bridge 关闭留在渲染线程上（避免对存活渲染循环的 UAF）。
    @Volatile var processCompletedConfirmed: Boolean = false,
    // 上次因「会话挂起」被自动关闭的时刻（System.nanoTime）；0 = 从未自动关闭过。
    // 自动关闭按 60s 节流：反复关闭又新建会让用户看到会话号乱跳。
    @Volatile var lastAutoCloseNanos: Long = 0L,
    // 本会话是否已向 Surface 呈现过至少一帧。
    //
    // 按**会话**记，不按进程记：进程级的首帧标志在首个会话出帧后恒为真，无法反映
    // 后建会话是否出过帧——「新建会话一片空白」正是这样被掩盖的。
    @Volatile var firstFramePresented: Boolean = false,
    // 本会话开始渲染的时刻（System.nanoTime）；0 = 尚未开始计时。
    //
    // 只在该会话**第一次**获得渲染线程时落定：渲染线程重启不重置它（重启不改变
    // 「这个会话出过帧没有」这一事实），切后台暂停也不重置（暂停期间不出帧是正常的）。
    @Volatile var firstFrameWaitStartNanos: Long = 0L,
    // 宿主侧信号到达时的钩子：帧回调链在空闲时会停掉，靠它把链重新拉起。
    // 单测构造的条目不需要链，默认空实现。
    val onSessionActive: () -> Unit = {},
) {
    // renderSignaled 取代了每帧一个 CountDownLatch：后者有丢唤醒竞态——bridge.render() 后循环发布
    // 新的 latch 并等待，但渲染期间生产者对旧 latch 的 countDown() 会让新 latch 始终未置位，
    // 线程只能等满超时。锁/条件变量下的合并标志同时避免了竞态与每帧分配。
    //
    // 保留的可接受窗口：落在 get() 检查与 waitOutput 后 set(false) 之间的 notifyRender() 会被并入标志，
    // 并可能被该 set(false) 一并清除，代价是一个空闲超时（最长 ~500ms）——绝不会丢帧
    // （下次信号或周期帧节拍会重渲染）。信号合并天然允许此情形，逐信号队列在此属过度设计。

    val renderSignaled = AtomicBoolean(false)

    @Volatile var forceRenderRequested: Boolean = false

    // vsync 对齐：由主线程上的 Choreographer 帧回调在每个显示帧置起。
    // 该回调只发信号——它绝不触碰 surface Mutex，也绝不直接调用 bridge.render()
    // （有互斥量饿死的先例：非渲染线程的渲染调用卡在 surface Mutex 上，
    // 永久阻塞了真正的渲染线程）。由渲染循环的唤醒门控消费。
    @Volatile var vsyncRequested: Boolean = false

    /**
     * 一帧的起止时刻（纳秒）：渲染线程是唯一写者，看门狗是读者。
     * 两个独立 `@Volatile` 会被跨帧混读，伪造出 `start > done`。
     * 整条记录单次发布，读者拿到的必定是同一帧的起止。
     */
    @Volatile var frameMarks: FrameMarks = FrameMarks()

    @Volatile var renderWatchDog: RenderWatchDog? = null

    @Volatile var lastSignalNanos: Long = System.nanoTime()

    /**
     * 渲染线程上次醒来时原生唤醒源的代数。
     *
     * 只由渲染线程读写；与 [Bridge.waitOutput] 的返回值配套，使「查完信号再进场」
     * 与通知竞争时不丢唤醒（见 `wakeGeneration` 的代数语义）。
     */
    @Volatile var wakeGeneration: Long = 0L

    fun notifyRender() {
        lastSignalNanos = System.nanoTime()
        onSessionActive()
        renderSignaled.set(true)
        // 唤醒原生条件变量而非 unpark：渲染线程在原生侧等待（PTY 输出与宿主信号共用
        // 同一把通知），unpark 对它没有任何作用。
        bridge.wakeRender()
    }

    /**
     * vsync 对齐的唤醒：置起渲染线程唤醒，但既不触碰空闲时钟，也不设置 [renderSignaled]。
     * 此前每个 vsync 都经 [notifyRender] 刷新 [lastSignalNanos]，
     * 使该时钟永远新鲜，空闲闭锁因而永不生效
     *（空闲终端上自我维持的 60-166fps 循环）。
     */
    fun pokeVsync() {
        bridge.wakeRender()
    }

    /**
     * 让渲染线程立刻离开等待去复查 `running`/会话状态。
     *
     * 原生条件变量等待不可被 `Thread.interrupt()` 打断，故拆卸路径必须先通知，
     * 再中断（中断仍负责其余可中断阻塞），否则 join 会白等满超时。
     *
     * 目标线程 MUST 由调用方显式给出：拆卸路径会先把 [renderThreadRef] 置空
     * （新线程随即接管这个引用），按引用取线程在那一刻拿到的恒是空——中断信号
     * 从未送达，join 只能白等满超时。
     */
    fun wakeRenderThread(renderThread: Thread) {
        bridge.wakeRender()
        renderThread.interrupt()
    }

    /**
     * 标记渲染线程已退出——但只在它仍是本会话注册的渲染线程时。
     *
     * 旧线程退出时若无条件置位，会覆盖新线程的存活状态：换线程的路径先把
     * [renderThreadRef] 换成新线程（或置空）再 join 旧线程，旧线程随后的退出写入
     * 把新线程误标为已死，监视器随即把健康的渲染线程杀掉重启——GPU 挂起后的恢复
     * 路径恰好就走这条路。
     */
    fun markRenderThreadExited() {
        if (renderThreadRef === Thread.currentThread()) {
            renderThreadExited = true
        }
    }

    @Volatile var scrollOffset: Int = 0

    /**
     * 本会话渲染线程最后见到的视口最后一个有内容的行（0 起，
     * [Bridge.LAST_CONTENT_ROW_NONE] = 视口全空）。输入法跟随位移按它裁剪平移量：
     * 稀疏会话不下移（内容原位），内容占满网格时按整块键盘高度上移。
     * 与活动会话的 [TerminalRuntime.lastContentRowFlow] 对应，会话切换时随之重新初始化。
     */
    @Volatile var lastContentRow: Int = Bridge.LAST_CONTENT_ROW_NONE

    /**
     * 本会话渲染线程最后见到的备用屏激活状态（helix/vim 等全屏 TUI）。
     * 备用屏恒占满视口，输入法跟随位移必须为 0，否则应用顶部被推出屏幕。
     * 与活动会话的 [TerminalRuntime.altScreenActiveFlow] 对应，会话切换时随之重新初始化。
     */
    @Volatile var altScreenActive: Boolean = false

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
     * 旧计数永远写不回去。
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
