package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `--frametimeline -all` 输出的解析判据。
 *
 * 样本取自模拟器上运行中的终端 SurfaceView（BLAST）图层实测输出（API 35 模拟器，
 * 持续输出时采集），断言用字面量而非重新计算的期望值，格式漂移时即判红。
 */
class FrameTimelineFramesTest {
    @Test
    fun parsesDisplayFramesAndLayerEntries() {
        val frames = parseFrameTimeline(FRAME_TIMELINE_OUTPUT_SAMPLE)

        assertEquals(2, frames.size)
        val firstFrame = frames[0]
        assertEquals(0, firstFrame.index)
        assertEquals(10.83, requireNotNull(firstFrame.presentTimeMillis), 0.001)
        assertEquals("Prediction Error", firstFrame.jankType)
        val layer = firstFrame.layers.single()
        assertEquals("SurfaceView[com.termux/terminal.emulator.MainActivity](BLAST)#11319", layer.name)
        assertEquals(15741, layer.ownerPid)
        assertEquals(11319, layer.layerId)
        assertTrue(layer.isBuffer)
        assertEquals("Presented", layer.presentState)
        assertEquals(10.83, requireNotNull(layer.presentTimeMillis), 0.001)
        assertEquals(30.92, requireNotNull(frames[1].presentTimeMillis), 0.001)
    }

    @Test
    fun selectsFramesWithinScenarioWindow() {
        val frames = parseFrameTimeline(FRAME_TIMELINE_OUTPUT_SAMPLE)

        assertEquals(listOf(0), frames.withinWindow(0.0, 20.0, 0.0).map { it.index })
        assertEquals(listOf(0, 1), frames.withinWindow(0.0, 40.0, 0.0).map { it.index })
        assertTrue(frames.withinWindow(100.0, 200.0, 0.0).isEmpty())
    }

    @Test
    fun shiftsWindowByBaseOffset() {
        val frames = parseFrameTimeline(FRAME_TIMELINE_OUTPUT_SAMPLE)

        assertTrue(frames.withinWindow(0.0, 20.0, 1000.0).isEmpty())
        assertEquals(listOf(1), frames.withinWindow(1020.0, 1040.0, 1000.0).map { it.index })
    }

    @Test
    fun attributesLayersByOwnerProcess() {
        val frames = parseFrameTimeline(FRAME_TIMELINE_OUTPUT_SAMPLE)

        assertEquals(2, frames.layersOwnedBy(15741).size)
        assertTrue(frames.layersOwnedBy(999).isEmpty())
    }

    @Test
    fun alignsTimelineBaseWithLatencyChannel() {
        // 实测：帧时间线的相对时刻 10.83 / 30.92 对应逐图层通道的绝对时刻
        // 17658699.05 / 17658719.14 毫秒。
        val offset = timelineBaseOffsetMillis(
            timelinePresentTimesMillis = listOf(10.83, 30.92),
            latencyPresentTimesMillis = listOf(17658699.05, 17658719.14),
        )

        assertEquals(17658688.22, requireNotNull(offset), 0.01)
    }

    @Test
    fun reportsNoBaseOffsetWithoutCommonEvents() {
        assertNull(timelineBaseOffsetMillis(listOf(10.83), listOf(20.0)))
        assertNull(timelineBaseOffsetMillis(emptyList(), listOf(20.0)))
    }

    @Test
    fun detectsPresentEventsMissingFromEitherChannel() {
        val latencyPresentTimesMillis = listOf(17658699.05, 17658719.14)

        assertTrue(samePresentEvents(listOf(10.83, 30.92), latencyPresentTimesMillis.map { it - 17658688.22 }))
        assertFalse(samePresentEvents(listOf(10.83), listOf(20.0)))
        assertFalse(samePresentEvents(emptyList(), listOf(20.0)))
    }

    private companion object {
        /** 终端持续输出时采集的两帧显示级帧时间线。 */
        val FRAME_TIMELINE_OUTPUT_SAMPLE =
            """
            Number of display frames : 64
            Display Frame 0 [*]
            Prediction State : Valid
            Jank Type : Prediction Error
            Present Metadata : Early Present
            Finish Metadata: On Time Finish
            Start Metadata: Late Start
            Vsync Period:  16.666666
            Present delta:   9.569087
            Present delta % refreshrate:   9.569087
            		    Start time		|    End time		|    Present time
            Expected	|	      4.74	|	     20.40	|	     20.40
            Actual  	|	      7.19	|	     10.76	|	     10.83
            ----------------------------------------------------------------------------------------

                Layer - TX - SurfaceView[com.termux/terminal.emulator.MainActivity](BLAST)#11319
                Token: -1
                Is Buffer?: 1
                Owner Pid : 15741
                Scheduled rendering rate: 0 fps
                Layer ID : 11319
                Present State : Presented
                Prediction State : None
                Jank Type : None
                Present Metadata : Unknown Present
                Finish Metadata: Unknown Finish
                Last latch time:   0.000000
                		    Start time		|    End time		|    Present time
                Actual  	|		N/A	|	      0.00	|	     10.83
                ----------------------------------------------------------------------------------------

            Display Frame 1 [*]
            Prediction State : Valid
            Jank Type : Prediction Error
            Present Metadata : Early Present
            Finish Metadata: On Time Finish
            Start Metadata: Unknown Start
            Vsync Period:  16.666666
            Present delta:   6.149020
            Present delta % refreshrate:   6.149020
            		    Start time		|    End time		|    Present time
            Expected	|	     21.40	|	     37.07	|	     37.07
            Actual  	|	     23.20	|	     30.00	|	     30.92
            ----------------------------------------------------------------------------------------

                Layer - TX - SurfaceView[com.termux/terminal.emulator.MainActivity](BLAST)#11319
                Token: -1
                Is Buffer?: 1
                Owner Pid : 15741
                Scheduled rendering rate: 0 fps
                Layer ID : 11319
                Present State : Presented
                Prediction State : None
                Jank Type : None
                Present Metadata : Unknown Present
                Finish Metadata: Unknown Finish
                Last latch time:   7.397025
                		    Start time		|    End time		|    Present time
                Actual  	|		N/A	|	     17.22	|	     30.92
                ----------------------------------------------------------------------------------------
            """.trimIndent()
    }
}
