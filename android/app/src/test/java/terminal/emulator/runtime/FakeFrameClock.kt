package terminal.emulator.runtime

import android.view.Choreographer

/** 可手动步进的帧时钟：回调排队后由测试显式触发。 */
internal class FakeFrameClock : FrameClock {
    private val queued = mutableListOf<Choreographer.FrameCallback>()

    override fun post(callback: Choreographer.FrameCallback) {
        queued += callback
    }

    override fun remove(callback: Choreographer.FrameCallback) {
        queued -= callback
    }

    /** 走一个显示帧：执行此刻排队的回调。 */
    fun step(frameTimeNanos: Long = 0L) {
        val due = queued.toList()
        queued.clear()
        due.forEach { it.doFrame(frameTimeNanos) }
    }

    /** 队列里是否还有待执行的帧回调。 */
    fun hasQueued(): Boolean = queued.isNotEmpty()
}
