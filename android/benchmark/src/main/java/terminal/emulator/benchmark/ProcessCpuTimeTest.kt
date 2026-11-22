package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 进程 CPU 时间的判据：解析 `/proc/<pid>/stat` 的计时字段并换算成毫秒。
 *
 * 样本取自本机 AVD 上 `com.android.settings` 冷启动后的实测输出。
 */
class ProcessCpuTimeTest {
    @Test
    fun parsesProcessTimingFields() {
        val stat = "715 (ndroid.settings) R 355 355 0 0 -1 4194624 12683 0 13 0 20 22 0 0 10 -10 25 0 " +
            "1157926 16864022528 46733 18446744073709551615 1 1 0 0 0 0 4612 1 1073775864 0 0 0 17 0 0 0 0 0 0"

        val cpuTime = parseProcessCpuTime(stat, uptimeMillis = 11_580_010)

        assertEquals(200L, cpuTime.userMillis)
        assertEquals(220L, cpuTime.systemMillis)
        assertEquals(420L, cpuTime.totalMillis)
        assertEquals(750L, cpuTime.ageMillis)
    }

    @Test
    fun parsesProcessNameContainingSpaces() {
        val stat = "42 (my app) S 1 1 0 0 -1 0 1 0 0 0 30 15 0 0 0 0 1 0 500 0 0 0 0"

        val cpuTime = parseProcessCpuTime(stat, uptimeMillis = 20_000)

        assertEquals(300L, cpuTime.userMillis)
        assertEquals(150L, cpuTime.systemMillis)
        assertEquals(450L, cpuTime.totalMillis)
        assertEquals(15_000L, cpuTime.ageMillis)
    }

    @Test
    fun rejectsStatLineWithoutFields() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseProcessCpuTime("42 (my app) S 1 1", uptimeMillis = 20_000)
        }

        assertTrue(failure.message?.contains("字段不足") == true)
    }

    @Test
    fun rejectsStatLineWithoutProcessName() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseProcessCpuTime("42 my app S 1 1", uptimeMillis = 20_000)
        }

        assertTrue(failure.message?.contains("格式不对") == true)
    }
}
