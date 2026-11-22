package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 场景矩阵的契约：12 类场景齐全，每类都有负载、指标、采集面、判据与资源上限。
 *
 * 期望值写死在这里：矩阵是编排的单一事实源，少一个场景或某一项被清空都会让跨应用
 * 对比缺一块，而缺口在现场跑完之前看不出来。
 */
class ScenarioMatrixTest {
    @Test
    fun coversEveryScenarioCategory() {
        assertEquals(
            listOf(
                "S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8", "S9", "S10", "S11", "S12",
            ),
            SCENARIO_MATRIX.map { it.id },
        )
        assertEquals(
            listOf(
                "冷启动", "温启动", "热启动", "稳态空载", "持续输出", "突发输出",
                "长滚动", "宽字符行", "交互延迟", "输入洪水", "后台前台切换", "资源极端",
            ),
            SCENARIO_MATRIX.map { it.label },
        )
    }

    @Test
    fun everyScenarioDeclaresLoadMetricsSurfacesAndVerdict() {
        SCENARIO_MATRIX.forEach { scenario ->
            assertTrue("${scenario.id} 缺负载", scenario.load.isNotBlank())
            assertTrue("${scenario.id} 缺指标", scenario.metrics.isNotEmpty())
            assertTrue("${scenario.id} 缺采集面", scenario.surfaces.isNotEmpty())
            assertTrue("${scenario.id} 缺判据", scenario.verdict.isNotBlank())
            scenario.metrics.forEach { metric ->
                assertTrue("${scenario.id} 的指标名是空的", metric.isNotBlank())
            }
            scenario.surfaces.forEach { surface ->
                assertTrue("${scenario.id} 的采集面是空的", surface.isNotBlank())
            }
        }
    }

    @Test
    fun everyScenarioHasResourceCaps() {
        SCENARIO_MATRIX.forEach { scenario ->
            assertTrue(
                "${scenario.id} 的 wall-clock 上限 ${scenario.wallClockLimitMillis}ms 不在 1–60 秒内",
                scenario.wallClockLimitMillis in 1L..60_000L,
            )
            assertEquals(
                "${scenario.id} 的输出字节上限不是 8 MB",
                8L * 1024 * 1024,
                scenario.outputByteLimit,
            )
            assertTrue(
                "${scenario.id} 的采样间隔下限 ${scenario.samplingIntervalFloorMillis}ms 低于 100ms",
                scenario.samplingIntervalFloorMillis >= 100L,
            )
        }
    }

    @Test
    fun scenarioIdsAreUnique() {
        assertEquals(SCENARIO_MATRIX.size, SCENARIO_MATRIX.map { it.id }.toSet().size)
    }
}
