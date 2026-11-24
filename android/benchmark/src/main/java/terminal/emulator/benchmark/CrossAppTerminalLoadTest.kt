package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 跨应用负载与取数的纯逻辑判据。
 *
 * 跨应用对比的有效性建立在这几件事上：命令对两个应用完全相同、完成信号只认本轮的
 * 落盘文件、网格尺寸能核对、帧面统计按绝对时刻窗口取。故逐项钉死。
 */
class CrossAppTerminalLoadTest {
    private val paths = CrossAppLoadPaths("crossApp-1")

    @Test
    fun `grid parsing reads rows and columns`() {
        assertEquals(TerminalGrid(rows = 24, columns = 80), parseTerminalGrid("24 80\n"))
    }

    @Test
    fun `grid parsing rejects malformed or non-positive values`() {
        assertNull(parseTerminalGrid(null))
        assertNull(parseTerminalGrid(""))
        assertNull(parseTerminalGrid("24"))
        assertNull(parseTerminalGrid("24 80 3"))
        assertNull(parseTerminalGrid("0 80"))
        assertNull(parseTerminalGrid("rows columns"))
    }

    @Test
    fun `ready probe writes grid and ready marker to the same paths`() {
        val command = readyProbeCommand(paths)
        assertTrue(command, command.contains("stty size > ${paths.gridPath}"))
        assertTrue(command, command.contains("printf $CROSS_APP_READY_MARKER > ${paths.readyPath}"))
    }

    @Test
    fun `sustained output command carries the matrix shape and the done marker`() {
        val command = sustainedOutputCommand(paths, batchCount = 60, linesPerBatch = 1_000, pauseSeconds = "0.1")
        assertEquals(
            "for b in \$(seq 1 60); do seq 1 1000; sleep 0.1; done; printf DONE > ${paths.donePath}",
            command,
        )
    }

    @Test
    fun `wide character load uses the given completion instead of the logcat marker`() {
        val completion = "printf DONE > ${paths.donePath}"
        val command = WideCharacterLoad(lineCount = 2, canary = "__canary__", completion = completion).command()
        assertTrue(command, command.endsWith("; printf DONE > ${paths.donePath}"))
        assertTrue(command, !command.contains("log -t"))
    }

    @Test
    fun `layer window stats only count frames inside the window`() {
        val refreshPeriodNanos = 16_666_666L
        val presents = listOf(
            1_000_000_000L,
            1_016_666_666L,
            1_033_333_332L,
            2_000_000_000L,
        )
        val stats = layerWindowStats(
            presentTimesNanos = presents,
            windowStartNanos = 900_000_000L,
            windowEndNanos = 1_100_000_000L,
            refreshPeriodNanos = refreshPeriodNanos,
        )
        assertEquals(3, stats.frameCount)
        assertEquals(16, stats.p95IntervalMillis)
        assertTrue(
            "三帧在 33.3ms 内呈现，槽位比例应接近 1：${stats.presentationSlotRatio}",
            stats.presentationSlotRatio > 0.9,
        )
    }

    @Test
    fun `layer window stats report no ratio below two samples`() {
        val stats = layerWindowStats(
            presentTimesNanos = listOf(1_000_000_000L),
            windowStartNanos = 900_000_000L,
            windowEndNanos = 1_100_000_000L,
            refreshPeriodNanos = 16_666_666L,
        )
        assertEquals(1, stats.frameCount)
        assertEquals(0L, stats.p95IntervalMillis)
        assertEquals(0.0, stats.presentationSlotRatio, 0.0)
    }
}
