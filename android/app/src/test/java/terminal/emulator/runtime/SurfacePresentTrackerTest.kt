package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 开孔缓冲区在位状态机：首帧门闸、世代失效与即时落回。
 *
 * 判别力：撤遮罩早一帧就是「切回前台闪一下」，故必须钉住延后的帧数；世代不符的
 * 上报若被采纳，新开孔还没有任何缓冲就被撤了遮罩，同样是闪烁。
 */
class SurfacePresentTrackerTest {
    /** 状态机与其外部影响的记录。 */
    private class Harness {
        val frameClock = FakeFrameClock()

        /** 每帧呈现的上报：元素为「此前无缓冲区」标记。 */
        val presented = mutableListOf<Boolean>()

        /** 「缓冲区在位」翻转记录。 */
        val bufferPresentChanges = mutableListOf<Boolean>()

        val tracker =
            SurfacePresentTracker(
                postToMain = { action -> action() },
                frameClock = frameClock,
                onFramePresented = { immediateCapture -> presented += immediateCapture },
                onBufferPresentChanged = { present -> bufferPresentChanges += present },
            )
    }

    @Test
    fun `present is published only after the deferral frames elapsed`() {
        val harness = Harness()
        harness.tracker.publish(true)

        assertFalse("排队中的首帧不得立刻算在位", harness.tracker.bufferPresent.value)
        harness.frameClock.step()
        assertFalse("门闸未走完不得撤遮罩", harness.tracker.bufferPresent.value)
        harness.frameClock.step()
        assertFalse("门闸未走完不得撤遮罩", harness.tracker.bufferPresent.value)
        harness.frameClock.step()

        assertTrue("两帧门闸走完后必须算在位", harness.tracker.bufferPresent.value)
        assertEquals("在位标志只在翻转时发布", listOf(true), harness.bufferPresentChanges)
    }

    @Test
    fun `a report from a stale epoch is discarded`() {
        val harness = Harness()
        val staleEpoch = harness.tracker.epoch
        harness.tracker.publish(false)

        harness.tracker.publish(true, staleEpoch)
        harness.frameClock.step()

        assertFalse("旧世代的上报不得让遮罩撤下", harness.tracker.bufferPresent.value)
        assertFalse("旧世代的上报不得排队帧回调", harness.frameClock.hasQueued())
    }

    @Test
    fun `a missing surface takes effect at once and voids the queued callback`() {
        val harness = Harness()
        harness.tracker.publish(true)

        harness.tracker.publish(false)

        assertFalse("surface 被拆必须立即落回无缓冲区", harness.tracker.bufferPresent.value)
        assertFalse("作废的帧回调必须从队列里撤掉", harness.frameClock.hasQueued())
        harness.frameClock.step()
        assertFalse("作废的帧回调不得再把标志置回在位", harness.tracker.bufferPresent.value)
    }

    @Test
    fun `only the first frame after a missing buffer wakes the cover capture`() {
        val harness = Harness()
        harness.tracker.publish(true)
        harness.frameClock.step()
        harness.frameClock.step()
        harness.frameClock.step()

        harness.tracker.publish(true)

        assertEquals(
            "此前无缓冲区时立即唤醒回读，之后的帧不再唤醒",
            listOf(true, false),
            harness.presented,
        )
        assertEquals("在位标志不得重复翻转", listOf(true), harness.bufferPresentChanges)
    }

    @Test
    fun `a new epoch restarts the deferral gate`() {
        val harness = Harness()
        harness.tracker.publish(true)
        harness.frameClock.step()
        harness.frameClock.step()
        harness.frameClock.step()
        assertTrue("首帧门闸走完后必须算在位", harness.tracker.bufferPresent.value)

        harness.tracker.publish(false)
        harness.tracker.publish(true)
        harness.frameClock.step()
        harness.frameClock.step()

        assertFalse("新一代的首帧必须重新走门闸", harness.tracker.bufferPresent.value)
        harness.frameClock.step()
        assertTrue("新一代门闸走完后必须算在位", harness.tracker.bufferPresent.value)
        assertEquals("在位标志只在翻转时发布", listOf(true, false, true), harness.bufferPresentChanges)
    }
}
