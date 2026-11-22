package terminal.emulator.benchmark

import android.os.SystemClock
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 场景预算的执行态：输出字节超限与 wall-clock 超时都终止场景并记「超限终止」。
 */
class ScenarioBudgetTest {
    private val scenario = Scenario(
        id = "S5",
        label = "持续输出",
        load = "连续字节流（逐行）",
        metrics = listOf("吞吐"),
        surfaces = listOf("呈现汇总"),
        verdict = "无丢帧、内存有界",
        wallClockLimitMillis = 30_000L,
        outputByteLimit = 1024L,
        samplingIntervalFloorMillis = 100L,
    )

    @Test
    fun accumulatesOutputBytes() {
        val budget = ScenarioBudget(scenario)

        budget.appendOutput(400)
        budget.appendOutput(600)

        assertEquals(1000L, budget.outputBytes)
    }

    @Test
    fun stopsScenarioWhenOutputExceedsCap() {
        val budget = ScenarioBudget(scenario)

        val failure = assertThrows(ScenarioLimitExceeded::class.java) {
            budget.appendOutput(1025)
        }

        assertTrue(failure.message?.contains("超限终止") == true)
    }

    @Test
    fun stopsScenarioWhenWallClockLimitPassed() {
        val budget = ScenarioBudget(
            scenario,
            startedAtElapsedMillis = SystemClock.elapsedRealtime() - 30_001L,
        )

        val failure = assertThrows(ScenarioLimitExceeded::class.java) {
            budget.checkDeadline()
        }

        assertTrue(failure.message?.contains("超限终止") == true)
    }

    @Test
    fun keepsScenarioWithinWallClockLimit() {
        val budget = ScenarioBudget(scenario)

        budget.checkDeadline()

        assertTrue(budget.elapsedMillis() <= scenario.wallClockLimitMillis)
    }
}
