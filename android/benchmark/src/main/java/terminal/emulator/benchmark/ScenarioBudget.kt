package terminal.emulator.benchmark

import android.os.SystemClock

/**
 * 场景的资源预算：wall-clock 上限与输出字节上限的执行态。
 *
 * 上限的作用是「极端场景快速死亡不浪费资源」：超时或输出超限即终止该场景并记
 * 「超限终止」，不继续等待、不继续追加输出。预算取自场景矩阵，不在执行期临时决定。
 *
 * 构造时记下起始时刻，之后按单调时钟算已耗用的墙钟——墙上时钟被校准时不会把预算
 * 判定带偏。
 */
internal class ScenarioBudget(
    /** 该预算对应的场景。 */
    val scenario: Scenario,
    /** 场景开始时刻（单调时钟毫秒）。 */
    private val startedAtElapsedMillis: Long = SystemClock.elapsedRealtime(),
) {
    /** 已写入的输出字节数。 */
    var outputBytes: Long = 0L
        private set

    /** 场景已耗用的墙钟毫秒。 */
    fun elapsedMillis(): Long = SystemClock.elapsedRealtime() - startedAtElapsedMillis

    /**
     * 追加输出字节；超上限即终止场景。
     *
     * 字节数由负载的生产者记账：场景喂给终端的字节一多，终端侧的解析与渲染负担就
     * 按量增长，超限即终止才不会把设备耗死在失败场景上。
     */
    fun appendOutput(bytes: Int) {
        require(bytes >= 0) { "输出字节数不能为负：$bytes" }
        outputBytes += bytes
        if (outputBytes > scenario.outputByteLimit) {
            throw ScenarioLimitExceeded(
                "场景 ${scenario.id} 输出 $outputBytes 字节，超过上限 " +
                    "${scenario.outputByteLimit} 字节：超限终止",
            )
        }
    }

    /** 场景剩余 wall-clock 毫秒；非负。 */
    fun remainingMillis(): Long = (scenario.wallClockLimitMillis - elapsedMillis()).coerceAtLeast(0L)

    /** 场景 wall-clock 是否已超限。 */
    fun isScenarioLimitExceeded(): Boolean = elapsedMillis() > scenario.wallClockLimitMillis

    /** 在预算内睡眠；超限时抛出 ScenarioLimitExceeded。 */
    fun sleepWithinBudget(millis: Long) {
        if (millis <= 0) return
        val available = remainingMillis()
        if (available <= 0) {
            checkDeadline()
            return
        }
        val toSleep = minOf(millis, available)
        SystemClock.sleep(toSleep)
        checkDeadline()
    }
    fun checkDeadline() {
        val elapsed = elapsedMillis()
        if (elapsed > scenario.wallClockLimitMillis) {
            throw ScenarioLimitExceeded(
                "场景 ${scenario.id} 已耗用 ${elapsed}ms，超过上限 " +
                    "${scenario.wallClockLimitMillis}ms：超限终止",
            )
        }
    }
}

/** 场景超出资源上限：终止该场景并记「超限终止」。 */
internal class ScenarioLimitExceeded(message: String) : IllegalStateException(message)
