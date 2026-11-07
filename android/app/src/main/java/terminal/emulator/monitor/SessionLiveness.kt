package terminal.emulator.monitor

/**
 * 会话挂起的层级。
 *
 * 层级决定恢复动作（见 [recoveryActionFor]）：渲染层可重启渲染线程，VT 层与输出
 * 投递阻塞只能关闭会话，生命周期层只记录。
 */
enum class HungLayer {
    /** 渲染帧在途且久未结束：卡在渲染线程内。 */
    RENDER,

    /** VT 线程正在执行命令批且久未返回。 */
    VT,

    /** PTY 输出已读出却投不进通道：归因下游未消费。 */
    OUTPUT_BACKLOG,

    /** 渲染线程已死或从未启动，而会话仍处于运行态。 */
    LIFECYCLE,
}

/** 挂起层级对应的恢复动作。 */
enum class LivenessRecovery {
    /** 重启渲染线程：渲染线程无状态，重建代价最低。 */
    RESTART_RENDER_THREAD,

    /** 关闭会话：VT 状态在原生侧持有，无法安全热重建。 */
    CLOSE_SESSION,

    /** 只记录，不自动处理。 */
    RECORD_ONLY,
}

/**
 * 一次挂起判定所需的输入快照。
 *
 * 三个忙标记都是「**此刻正在做什么**」而非「多久没做事」：空闲会话的 VT 线程阻塞
 * 在通道等待上、PTY 读取线程阻塞在 `read()` 上，两者都不推进任何计数——按「计数
 * 未推进」判定会把每个空闲超过阈值的 shell 判死。
 */
data class SessionLivenessInputs(
    /** 渲染帧在途时长（纳秒）；不在途时为 0。 */
    val renderFrameBusyNanos: Long,
    /** VT 线程已忙时长（纳秒）；0 = 空闲等待。 */
    val vtBusyNanos: Long,
    /** PTY 输出投递已阻塞时长（纳秒）；0 = 未在投递。 */
    val ptySendBusyNanos: Long,
    /** 渲染线程是否存活。 */
    val renderThreadAlive: Boolean,
)

/**
 * 挂起诊断结果。
 *
 * [hungLayer] 为 `null` 表示未挂起（含空闲会话）。[busyNanos] 是命中那一层的
 * 已忙时长，供日志说明「卡了多久」。
 */
data class SessionLivenessDiagnosis(val hungLayer: HungLayer?, val busyNanos: Long)

/**
 * 各层的挂起阈值（纳秒）。
 *
 * 阈值差异的理由：VT 批处理有 4ms 时长预算与 64 条上限，单条命令执行超过 15s 即
 * 异常；输出投递的 20s 上限覆盖通道满时的正常背压窗口。
 */
data class LivenessThresholds(
    val renderHangNanos: Long = 10_000_000_000L,
    val vtHangNanos: Long = 15_000_000_000L,
    val outputBacklogNanos: Long = 20_000_000_000L,
)

/**
 * 判定会话挂在哪一层；未挂起时返回层级为 `null` 的诊断。
 *
 * 判定顺序即优先级：命中即止。渲染层最先判——它是唯一可自愈的层，且渲染帧在途
 * 时其它层的读数多半只是它的下游后果。
 *
 * 纯函数：输入是判定瞬间的快照，不读时钟、不读全局状态，故五种情形都可由单测
 * 直接构造。
 */
fun diagnoseSessionLiveness(
    inputs: SessionLivenessInputs,
    thresholds: LivenessThresholds = LivenessThresholds(),
): SessionLivenessDiagnosis = when {
    inputs.renderFrameBusyNanos > thresholds.renderHangNanos ->
        SessionLivenessDiagnosis(HungLayer.RENDER, inputs.renderFrameBusyNanos)

    inputs.vtBusyNanos > thresholds.vtHangNanos ->
        SessionLivenessDiagnosis(HungLayer.VT, inputs.vtBusyNanos)

    inputs.ptySendBusyNanos > thresholds.outputBacklogNanos ->
        SessionLivenessDiagnosis(HungLayer.OUTPUT_BACKLOG, inputs.ptySendBusyNanos)

    !inputs.renderThreadAlive -> SessionLivenessDiagnosis(HungLayer.LIFECYCLE, 0L)

    else -> SessionLivenessDiagnosis(null, 0L)
}

/**
 * 挂起层级 → 恢复动作。
 *
 * VT 层与输出投递阻塞同归「关闭会话」：后者的归因本就是下游未消费，重启渲染线程
 * 对它无效。生命周期层只记录：Activity 重建、切后台期间不得误杀用户的 shell。
 */
fun recoveryActionFor(hungLayer: HungLayer?): LivenessRecovery = when (hungLayer) {
    HungLayer.RENDER -> LivenessRecovery.RESTART_RENDER_THREAD
    HungLayer.VT, HungLayer.OUTPUT_BACKLOG -> LivenessRecovery.CLOSE_SESSION
    HungLayer.LIFECYCLE, null -> LivenessRecovery.RECORD_ONLY
}

/**
 * 自动关闭的节流窗口（纳秒）。
 *
 * 同一会话在窗口内最多自动关闭一次：反复关闭又新建会让用户看到会话号乱跳。
 */
const val SESSION_CLOSE_THROTTLE_NANOS = 60_000_000_000L

/**
 * 现在能否自动关闭该会话。
 *
 * [lastCloseNanos] 为 0 表示从未自动关闭过——首个 0 值同时是「无历史」的哨兵与
 * 「距今远超窗口」的合法起点，两种读法都得出「可以关闭」，故无需另设布尔位。
 */
fun shouldCloseSession(lastCloseNanos: Long, nowNanos: Long): Boolean =
    lastCloseNanos <= 0L || nowNanos - lastCloseNanos >= SESSION_CLOSE_THROTTLE_NANOS
