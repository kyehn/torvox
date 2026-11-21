package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * `--latency` 输出的解析判据。
 *
 * 样本取自模拟器上运行中的终端 SurfaceView（BLAST）图层实测输出（API 35 模拟器，
 * 持续输出时采集），断言用字面量而非重新计算的期望值，格式漂移时即判红。
 */
class SurfaceLayerFramesTest {
    @Test
    fun parsesLayerFrameOutput() {
        val stats = requireNotNull(parseLayerFrameStats(LAYER_FRAME_OUTPUT_SAMPLE))

        assertEquals(16_666_666L, stats.refreshPeriodNanos)
        assertEquals(6, stats.contentFrameCount)
        assertEquals(0, stats.missedFrameCount)
        assertEquals(16L, stats.medianIntervalMillis)
        assertEquals(17L, stats.p90IntervalMillis)
        assertEquals(17L, stats.maxIntervalMillis)
        assertEquals(60.97, stats.presentationRatePerSecond, 0.01)
        assertEquals(1.016, stats.presentationSlotRatio, 0.001)
    }

    @Test
    fun rejectsLayerWithoutContentFrames() {
        assertNull(parseLayerFrameStats("16666666\n0\t0\t0\n0\t0\t0\n"))
    }

    @Test
    fun rejectsOutputWithoutRefreshPeriod() {
        assertNull(parseLayerFrameStats("0\n8482814103026\t8482834374814\t8482814549390\n"))
    }

    private companion object {
        /** 终端内容层的实测帧记录：首行是 60Hz 的刷新周期，其后每行三列纳秒。 */
        val LAYER_FRAME_OUTPUT_SAMPLE =
            """
            16666666
            8482814103026	8482834374814	8482814549390
            8482834465529	8482851095309	8482837674519
            8482851178583	8482867993779	8482852381062
            8482867704784	8482883084229	8482869880401
            8482882926124	8482898789754	8482883404571
            8482899947827	8482916386562	8482901692082
            """.trimIndent()
    }
}
