package terminal.emulator.runtime

import kotlinx.coroutines.CancellationException

// 渲染失败后的处置策略：死线程重启预算、surface 重建节奏、错误分级与退避。
// 判定与阈值都在此，运行期与渲染监管只留「何时调用」与「谁来调用」。

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
 * [RenderSupervisor.decayRestartCounts] 随即把它的
 * [SessionEntry.restartAttempts] 清零，
 * 重启预算永不耗尽（每 500ms 重启一轮，`RENDER_MAX_RESTART_ATTEMPTS` 形同虚设，
 * 最终表现为渲染循环反复重建而会话永不关闭）。
 *
 * 线程引用缺失（null）**不算死**：条目插入映射到
 * [RenderSupervisor.startRenderThread]
 * 真正把线程写入之间，`running` 已为真而引用仍为空。把这段正常的启动窗口判成死亡
 * 会让监视器抢先重启，与即将启动的线程争同一会话（两条线程消费唯一事件队列）。
 * 真正的孤儿条目由失败回滚按 bridge 身份移除。
 *
 * @param threadAlive 线程当前是否存活；null = 线程引用尚未写入
 */
internal fun isRenderThreadDead(renderThreadExited: Boolean, threadAlive: Boolean?): Boolean =
    renderThreadExited || threadAlive == false

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
 * 恢复确认所需的连续稳定帧数：新窗口连续出帧达到该数才清零换视图预算。
 *
 * 取 60（60fps 下约 1 秒）：2026-11-23 11-05-03 的每一代新设备都在首帧后
 * 13–28ms 再次丢失，按「出过一帧」清零会让 5 次快试与 500ms 间隔在 29 秒内
 * 反复失效（11:03:03 预算又从 `attempt 1/5` 起算）。稳定一秒才认定重建有效：
 * 短时软重启的重建熬不过它，真正恢复的窗口一秒内必然出满 60 帧
 * （渲染循环按 vsync 空转，空闲时同样计帧）。计数只算帧数不记时刻，不依赖挂钟。
 */
internal const val SURFACE_RECOVERY_HEALTHY_FRAMES = 60

/**
 * 恢复是否已确认：连续稳定帧数已达 [SURFACE_RECOVERY_HEALTHY_FRAMES]。
 *
 * 纯判定，供 [RenderSupervisor.handleSurfaceInvalidated] 与单测；
 * 不以「已出过任何帧」作恢复信号，理由见常量注释。
 */
internal fun surfaceRecoverySettled(stableFrames: Int): Boolean = stableFrames >= SURFACE_RECOVERY_HEALTHY_FRAMES

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
