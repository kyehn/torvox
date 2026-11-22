package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 宿主负载的判据：解析设备侧 CPU 汇总行、算窗口内的抢占占比、标出抢占轮次。
 *
 * 样本取自本机 AVD 的 `/proc/stat` 实测输出（空闲与宿主被 8 个忙循环抢占两种状态）。
 */
class HostLoadTest {
    @Test
    fun parsesCpuSummaryRow() {
        val snapshot = parseCpuTime("cpu  32387 15944 57720 2132261 1119 0 1273 12169 0 0")

        assertEquals(32387L, snapshot.user)
        assertEquals(15944L, snapshot.nice)
        assertEquals(57720L, snapshot.system)
        assertEquals(2132261L, snapshot.idle)
        assertEquals(1119L, snapshot.iowait)
        assertEquals(0L, snapshot.irq)
        assertEquals(1273L, snapshot.softirq)
        assertEquals(12169L, snapshot.steal)
        assertEquals(2252873L, snapshot.total)
        assertEquals(119493L, snapshot.busy)
    }

    @Test
    fun takesSummaryRowInsteadOfPerCpuRows() {
        val output = """
            cpu  32387 15944 57720 2132261 1119 0 1273 12169 0 0
            cpu0 18020 7360 27368 1068519 732 0 159 5989 0 0
            cpu1 14367 8584 30351 1063741 387 0 1113 6180 0 0
        """.trimIndent()

        assertEquals(32387L, parseCpuTime(output).user)
    }

    @Test
    fun rejectsOutputWithoutCpuSummary() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseCpuTime("cpu0 18020 7360 27368 1068519 732 0 159 5989 0 0")
        }

        assertTrue(failure.message?.contains("cpu 汇总行") == true)
    }

    @Test
    fun computesStealRatioWhileGuestIdle() {
        val before = parseCpuTime("cpu  32472 15951 57815 2168505 1120 0 1273 12209 0 0")
        val after = parseCpuTime("cpu  32474 15952 57826 2169476 1125 0 1273 12274 0 0")

        val load = hostLoadBetween(before, after, windowMillis = 5_030)

        assertEquals(65.0 / 1055.0, load.stealRatio, 1e-9)
        assertEquals(79.0 / 1055.0, load.busyRatio, 1e-9)
        assertEquals(5_030L, load.windowMillis)
        assertFalse("模拟器空闲时的 steal 不拖慢任何东西，不判为抢占", load.contended)
    }

    @Test
    fun computesStealRatioOnIdleHost() {
        val before = parseCpuTime("cpu  32387 15944 57720 2132261 1119 0 1273 12169 0 0")
        val after = parseCpuTime("cpu  32389 15944 57727 2133244 1120 0 1273 12170 0 0")

        val load = hostLoadBetween(before, after, windowMillis = 5_030)

        assertEquals(1.0 / 994.0, load.stealRatio, 1e-9)
        assertEquals(10.0 / 994.0, load.busyRatio, 1e-9)
        assertFalse("空闲宿主的抢占占比应低于阈值", load.contended)
    }

    @Test
    fun reportsZeroLoadWhenCountersDoNotAdvance() {
        val snapshot = parseCpuTime("cpu  32387 15944 57720 2132261 1119 0 1273 12169 0 0")

        val load = hostLoadBetween(snapshot, snapshot, windowMillis = 12)

        assertEquals(0.0, load.stealRatio, 0.0)
        assertEquals(0.0, load.busyRatio, 0.0)
        assertEquals(12L, load.windowMillis)
    }

    @Test
    fun flagsContentionAboveThreshold() {
        // 实测：宿主安静时活跃窗口 1.9%–5.2%，宿主被占用时 38%–46%。
        assertTrue(HostLoad(stealRatio = 0.387, busyRatio = 0.92, windowMillis = 1_910).contended)
        assertFalse(HostLoad(stealRatio = 0.052, busyRatio = 0.84, windowMillis = 971).contended)
    }
}
