package terminal.emulator.benchmark

import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 输出类场景的帧面判据。
 *
 * 六项判据各自拦一种失败形态：没出帧、窗口数塌了、中途停摆、间隔变慢、槽位占比塌了、
 * 两条通道对不上。判据是「哪一项不达标即判红」，故逐项构造不达标的帧面并断言失败信息
 * 指到那一项。
 */
class OutputScenarioFramesTest {
    @Test
    fun acceptsFramesMeetingEveryCriterion() {
        assertOutputScenarioFrames(
            frames = frames(),
            minPresentedWindows = 8,
            maxPresentationGapMillis = 1_000L,
            maxLayerIntervalP95Millis = 250L,
            minLayerSlotRatio = 0.15,
        )
    }

    @Test
    fun rejectsScenarioWithoutPresentedFrames() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(presentedFrames = 0),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("没有呈现任何帧"))
    }

    @Test
    fun rejectsTooFewPresentedWindows() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(presentedWindows = 3),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("呈现窗口"))
    }

    @Test
    fun rejectsLongPresentationGap() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(presentationGapMillis = 1_500L),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("呈现最长停摆"))
    }

    @Test
    fun rejectsSlowLayerInterval() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(p95IntervalMillis = 300L),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("呈现间隔 p95"))
    }

    @Test
    fun rejectsLowSlotRatio() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(presentationSlotRatio = 0.05),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("刷新槽位"))
    }

    @Test
    fun rejectsEmptyTimelineWindow() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(windowFramesCount = 0),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("帧时间线在输出时间窗内没有帧"))
    }

    @Test
    fun rejectsUnattributedTimelineFrames() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(timelineContentFrames = 0),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("没有把终端内容层归属"))
    }

    @Test
    fun rejectsChannelsRecordingDifferentPresentEvents() {
        val failure = assertThrows(IllegalStateException::class.java) {
            assertOutputScenarioFrames(
                frames = frames(
                    attributedPresentTimesMillis = listOf(10.0, 20.0),
                    layerPresentTimesMillis = listOf(10.0, 90.0),
                ),
                minPresentedWindows = 8,
                maxPresentationGapMillis = 1_000L,
                maxLayerIntervalP95Millis = 250L,
                minLayerSlotRatio = 0.15,
            )
        }

        assertTrue(failure.message.orEmpty().contains("呈现时刻对不上"))
    }

    private fun frames(
        presentedFrames: Int = 900,
        presentedWindows: Int = 10,
        presentationGapMillis: Long = 500L,
        p95IntervalMillis: Long = 20L,
        presentationSlotRatio: Double = 0.9,
        windowFramesCount: Int = 30,
        timelineContentFrames: Int = 30,
        attributedPresentTimesMillis: List<Double> = listOf(1.0, 2.0),
        layerPresentTimesMillis: List<Double> = listOf(1.0, 2.0),
    ): OutputScenarioFrames = OutputScenarioFrames(
        presentedFrames = presentedFrames,
        presentedWindows = presentedWindows,
        presentationGapMillis = presentationGapMillis,
        layerStats = LayerFrameStats(
            refreshPeriodNanos = REFRESH_PERIOD_NANOS,
            contentFrameCount = 30,
            missedFrameCount = 0,
            medianIntervalMillis = 16L,
            p90IntervalMillis = 20L,
            p95IntervalMillis = p95IntervalMillis,
            maxIntervalMillis = 40L,
            presentationRatePerSecond = presentationSlotRatio * 1_000_000_000.0 / REFRESH_PERIOD_NANOS,
            presentTimesNanos = listOf(1_000_000_000L, 2_000_000_000L),
        ),
        timelineContentFrames = timelineContentFrames,
        attributedPresentTimesMillis = attributedPresentTimesMillis,
        layerPresentTimesMillis = layerPresentTimesMillis,
        windowFramesCount = windowFramesCount,
    )

    private companion object {
        /** 60Hz 模拟器的刷新周期（纳秒）：槽位比例由呈现频率除以刷新率折算。 */
        const val REFRESH_PERIOD_NANOS = 16_666_666L
    }
}
