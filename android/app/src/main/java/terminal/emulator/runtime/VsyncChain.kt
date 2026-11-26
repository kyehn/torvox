package terminal.emulator.runtime

import android.view.Choreographer
import java.util.concurrent.atomic.AtomicBoolean

/**
 * 显示周期采样窗口：一秒内没有更小的帧间隔即重新起算（刷新率可能已变）。
 */
internal const val DISPLAY_PERIOD_WINDOW_NANOS: Long = 1_000_000_000L

/** 可采信的显示周期区间：1ms（1000Hz）到 50ms（20Hz）。 */
internal const val MIN_DISPLAY_PERIOD_NANOS: Long = 1_000_000L
internal const val MAX_DISPLAY_PERIOD_NANOS: Long = 50_000_000L

/** 空闲判定阈值：距上次信号超过 5s 且无滚动运动即转入空闲闭锁。 */
const val RENDER_IDLE_THRESHOLD_NANOS: Long = 5_000_000_000L

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
 * vsync 信号链：主线程上的自调度 Choreographer 帧回调。
 *
 * 链在无事可做时停掉：无会话、应用不可见、或全部会话连续空闲超过阈值且无待
 * 消费动画。停链后渲染线程落在 500ms 空闲闭锁上（那本就是空闲期的既定节拍），
 * 主线程不再按显示帧被唤醒。渲染态的延迟字段不靠本链逐帧消费：搜索高亮与选区
 * 各自经信号通道下发，下一帧即生效。
 *
 * 链同时测得显示周期（相邻两帧回调的时间戳差）：刷新率会被系统按省电与热节流
 * 调整，写死 16.7ms 会把显示降频误读成渲染故障，故周期由实测得出。
 */
internal class VsyncChain(
    private val postToMain: (() -> Unit) -> Unit,
    private val frameClock: FrameClock,
    private val wakeActiveSession: (frameTimeNanos: Long) -> Unit,
    private val hasSessions: () -> Boolean,
    private val anySessionActive: (nowNanos: Long) -> Boolean,
    private val appForeground: () -> Boolean,
) {
    /** 幂等保护：每进程至多一条自调度回调链（注册失败时重置以允许重试）。 */
    private val running = AtomicBoolean(false)

    /**
     * 实测的显示周期：由相邻两帧回调的时间戳差得出。
     *
     * 渲染线程读它做诊断，故用 volatile 发布。
     */
    @Volatile private var displayPeriodEstimate = DisplayPeriodEstimate(periodNanos = 0L, sampledAtNanos = 0L)

    /** 上一帧的 vsync 时间戳（只在主线程的帧回调里写）。 */
    private var lastFrameTimeNanos: Long = 0L

    /**
     * vsync 间隔的定窗统计：显示节拍的窗口统计必须来自实际帧回调时间戳。
     *
     * 复用 [FrameTimingStats]（渲染线程那份统计的另一个实例）：它只做定长环形写入与
     * 分位数计算，与写者无关。此处写者是主线程的帧回调，读者是渲染线程——但渲染
     * 线程读的不是这个对象，而是发布出去的报告快照。
     */
    private val vsyncIntervalStats = FrameTimingStats()

    /** 主线程发布的 vsync 间隔窗口报告（渲染线程读它做诊断）。 */
    @Volatile private var vsyncIntervalReport: FrameTimingReport? = null

    /**
     * 唯一的自调度帧回调：每个显示帧即一次 vsync 信号。
     *
     * 它只发信号（唤醒活动会话）——所有渲染均由渲染线程拥有，主线程绝不触碰
     * surface Mutex。
     */
    private val frameCallback =
        object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                val nowNanos = System.nanoTime()
                updateDisplayPeriod(frameTimeNanos)
                wakeActiveSession(frameTimeNanos)
                val hasSessions = hasSessions()
                val hasActiveSession = anySessionActive(nowNanos)
                if (shouldStopVsyncChain(hasSessions, appForeground(), hasActiveSession)) {
                    stop()
                    // 判定与停链之间可能刚好来了宿主侧信号：复核一次，活跃就重新拉起。
                    // 否则那一次信号会被吞掉，渲染只剩 17ms 闭锁这一路节拍，丢掉 vsync 对齐。
                    // 复核只认「停链原因就是会话空闲」：应用不可见或已无会话时复核必然
                    // 得到「仍活跃」，链于是每显示帧停一次又拉起一次，永不停住。
                    if (
                        shouldRestartVsyncChain(
                            hasSessions = hasSessions,
                            appForeground = appForeground(),
                            wasActive = hasActiveSession,
                            isActiveNow = anySessionActive(System.nanoTime()),
                        )
                    ) {
                        ensureRunning()
                    }
                    return
                }
                // 自调度：下一帧继续。
                frameClock.post(this)
            }
        }

    /**
     * 由帧回调时间戳测得显示周期。
     *
     * 帧回调只在主线程执行，故这里无需同步；读方（渲染线程）读的是发布后的快照。
     */
    private fun updateDisplayPeriod(frameTimeNanos: Long) {
        val previousFrameTimeNanos = lastFrameTimeNanos
        lastFrameTimeNanos = frameTimeNanos
        if (previousFrameTimeNanos == 0L) return
        val intervalNanos = frameTimeNanos - previousFrameTimeNanos
        displayPeriodEstimate =
            updateDisplayPeriodEstimate(
                estimate = displayPeriodEstimate,
                intervalNanos = intervalNanos,
                nowNanos = System.nanoTime(),
            )
        if (!isCredibleDisplayInterval(intervalNanos)) return
        vsyncIntervalStats.record(intervalNanos)
        vsyncIntervalStats.takeReport()?.let { report -> vsyncIntervalReport = report }
    }

    /**
     * 显示节拍诊断：实测周期与窗口统计（平均/p95/最大）。
     *
     * 报实测值而不是写死 16.7ms：刷新率会被系统按省电与热节流调整，写死的数字
     * 会把显示降频误读成渲染故障。
     */
    fun displayPeriodText(): String {
        val periodNanos = displayPeriodEstimate.periodNanos
        val periodText = if (periodNanos <= 0L) "未测得" else millisText(periodNanos)
        val report = vsyncIntervalReport ?: return periodText
        return "$periodText avg=${millisText(report.averageNanos)} " +
            "p95=${millisText(report.p95Nanos)} max=${millisText(report.maxNanos)}"
    }

    /** 纳秒→毫秒文本（一位小数）：帧时长分布的诊断口径统一用它。 */
    private fun millisText(nanos: Long): String {
        val tenthsOfMilli = (nanos + 50_000L) / 100_000L
        return "${tenthsOfMilli / 10}.${tenthsOfMilli % 10}ms"
    }

    /** 在主 Looper 上拉起 vsync 帧回调链。幂等：CAS 保护保证每进程至多一条链。 */
    fun ensureRunning() {
        if (!running.compareAndSet(false, true)) return
        postToMain {
            try {
                frameClock.post(frameCallback)
                LogUtil.debug("Runtime", "vsync frame callback chain started")
            } catch (exception: Exception) {
                // 允许后续会话启动时重试注册。
                running.set(false)
                LogUtil.error("Runtime", "vsync frame callback registration failed", exception)
            }
        }
    }

    /**
     * 停掉 vsync 帧回调链。
     *
     * 先同步放开运行标志（注册方才能重新拉起），再在主线程上摘掉待执行的帧回调：
     * Choreographer 的回调只能在创建它的线程上摘。摘除与重新拉起都经主线程排队，
     * 先后顺序因此保持——重新拉起不会被随后的摘除误伤。
     */
    fun stop() {
        running.set(false)
        postToMain {
            frameClock.remove(frameCallback)
        }
    }
}
