package terminal.emulator.runtime

/**
 * 针对 [FrameTimingStats] 窗口的基线自适应降级检测器。
 *
 * 固定的绝对 WARN 阈值无法同时服务软件渲染模拟器（基线 ~555ms/帧）与真机（~17ms/帧）：
 * 在模拟器上永不触发，在真机上则漏掉渐进回归。本类改为学习每台设备自身的基线
 * （非降级窗口均值的 EMA），当某窗口均值攀升到基线的 [degradationFactor] 倍
 * 且持续高于绝对阈值 [attentionFloorNanos] 时标记为降级
 * ——使真实回归在任何硬件上都能在日志中显现，而无需假定设备类型。
 *
 * 基线更新规则：只有非降级窗口才推进 EMA，使持续回归持续告警
 * 而不是被吸收进基线（「温水煮青蛙」防护）。
 */
class FrameTimingTrend(
    private val degradationFactor: Double = DEFAULT_DEGRADATION_FACTOR,
    private val attentionFloorNanos: Long = DEFAULT_ATTENTION_FLOOR_NANOS,
    private val emaAlpha: Double = DEFAULT_EMA_ALPHA,
) {
    private var baselineNanos: Double? = null

    /**
     * 喂入一个已完成窗口的平均渲染时长；当其相对学习到的基线已降级
     * （且高于关注阈值）时返回 true。首个窗口仅用于初始化基线。
     */
    fun observe(windowAverageNanos: Long): Boolean {
        val baseline = baselineNanos
        if (baseline == null) {
            baselineNanos = windowAverageNanos.toDouble()
            return false
        }
        val degraded = windowAverageNanos >= baseline * degradationFactor
        if (!degraded) {
            baselineNanos = baseline * (1.0 - emaAlpha) + windowAverageNanos * emaAlpha
        }
        return degraded && windowAverageNanos >= attentionFloorNanos
    }

    /** 学习到的基线（ns），首个窗口之前为 null。供测试/调试。 */
    fun currentBaselineNanos(): Long? = baselineNanos?.toLong()

    companion object {
        /** 学习到的基线的 3 倍即视为值得记录的降级。 */
        const val DEFAULT_DEGRADATION_FACTOR = 3.0

        /**
         * 平均值低于 100ms 的窗口绝不算「降级」——低于 6 FPS 在真机上已是真问题，
         * 在模拟器上更是病态，故该阈值在两种情况下都不会损失信号。
         */
        const val DEFAULT_ATTENTION_FLOOR_NANOS = 100_000_000L

        /** EMA 平滑系数：0.25 权重给最新窗口，0.75 给历史。 */
        const val DEFAULT_EMA_ALPHA = 0.25
    }
}
