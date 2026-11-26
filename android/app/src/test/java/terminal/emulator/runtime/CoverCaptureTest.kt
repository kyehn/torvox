package terminal.emulator.runtime

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.Shell
import terminal.emulator.bridge.TerminalConfig
import terminal.emulator.ui.BackgroundRenderPause
import terminal.emulator.ui.RenderPauseLedger

/**
 * 遮罩回读与发布：强制回读补齐缓存、发布失败尺寸丢弃、切会话作废缓存。
 *
 * 判别力：回读是 GPU 同步等待，只在强制信号或新帧到达时执行；发布必须校验尺寸，
 * 否则错位位图会盖在开孔上。两者都靠结果断言，不靠调用计数。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class CoverCaptureTest {
    private val bridge =
        Bridge(
            TerminalConfig(
                shell = Shell.SystemDefault,
                home = "/tmp",
                workingDirectory = "/tmp",
                prefix = "",
                fontSizeTenths = 140,
            ),
        )

    private fun frame(width: Int = 2, height: Int = 2): Bridge.CoverFrame =
        Bridge.CoverFrame(width = width, height = height, pixels = ByteArray(width * height * 4) { 7 })

    @Test
    fun `forced capture publishes a frame when a publish is waiting`() = runTest {
        val journal = HoleStateJournal()
        var active = 1L to bridge
        val capture =
            CoverCapture(
                scope = backgroundScope,
                backgroundPause = BackgroundRenderPause(RenderPauseLedger { _ -> }),
                holeJournal = journal,
                bufferPresent = { true },
                activeSession = { active },
                readFrame = { frame() },
            )
        capture.start()
        capture.publishStored(refresh = true)
        advanceTimeBy(1)

        assertTrue("强制回读后必须发布一帧", capture.frame.value != null)
        assertTrue("回读结果必须进入缓存", capture.hasStored())
        assertEquals(true, journal.snapshot().last().coverPresent)

        active = 2L to bridge
        capture.onSessionSwitched()
        assertNull("切会话后旧遮罩不得留在开孔上", capture.frame.value)
        assertFalse("切会话后旧缓存必须作废", capture.hasStored())
    }

    @Test
    fun `periodic capture follows new frames and stops while paused`() = runTest {
        val pause = BackgroundRenderPause(RenderPauseLedger { _ -> })
        var reads = 0
        val capture =
            CoverCapture(
                scope = backgroundScope,
                backgroundPause = pause,
                holeJournal = HoleStateJournal(),
                bufferPresent = { true },
                activeSession = { 1L to bridge },
                readFrame = {
                    reads++
                    frame()
                },
            )
        capture.start()
        capture.onFramePresented(immediateCapture = true)
        advanceTimeBy(1)
        assertEquals(1, reads)

        capture.onFramePresented(immediateCapture = false)
        pause.onBackground()
        advanceTimeBy(COVER_CAPTURE_INTERVAL_MS)
        assertEquals("暂停期间不得回读 GPU", 1, reads)

        pause.onForeground()
        capture.onFramePresented(immediateCapture = false)
        advanceTimeBy(COVER_CAPTURE_INTERVAL_MS)
        assertEquals("恢复后的新帧必须回读", 2, reads)
    }

    @Test
    fun `a frame presented during a read is captured on its own signal`() = runTest {
        var reads = 0
        lateinit var capture: CoverCapture
        capture =
            CoverCapture(
                scope = backgroundScope,
                backgroundPause = BackgroundRenderPause(RenderPauseLedger { _ -> }),
                holeJournal = HoleStateJournal(),
                bufferPresent = { true },
                activeSession = { 1L to bridge },
                readFrame = {
                    reads++
                    if (reads == 1) capture.onFramePresented(immediateCapture = false)
                    frame()
                },
            )
        capture.start()
        capture.onFramePresented(immediateCapture = true)
        advanceTimeBy(1)
        assertEquals(1, reads)

        advanceTimeBy(COVER_CAPTURE_INTERVAL_MS)
        assertEquals("回读期间到达的新帧必须再回读一次", 2, reads)
    }

    @Test
    fun `invalid frame size is rejected without publishing`() = runTest {
        val capture =
            CoverCapture(
                scope = backgroundScope,
                backgroundPause = BackgroundRenderPause(RenderPauseLedger { _ -> }),
                holeJournal = HoleStateJournal(),
                bufferPresent = { false },
                activeSession = { null },
            )

        capture.publish(Bridge.CoverFrame(width = 2, height = 2, pixels = ByteArray(3)))

        assertNull("尺寸不符的遮罩不得发布", capture.frame.value)
    }
}
