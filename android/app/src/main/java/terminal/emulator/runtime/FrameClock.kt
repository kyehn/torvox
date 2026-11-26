package terminal.emulator.runtime

import android.view.Choreographer

/**
 * 显示帧时钟：把「下一个显示帧」这件事抽象出来。
 *
 * 生产实现是 [ChoreographerFrameClock]；测试用可手动步进的替身。抽出来是因为两条
 * 主线程节拍链（开孔在位门闸、vsync 信号链）都要按显示帧推进，而两者都不该在
 * 单测里真的等 vsync。
 */
internal interface FrameClock {
    fun post(callback: Choreographer.FrameCallback)

    fun remove(callback: Choreographer.FrameCallback)
}

/** 生产帧时钟：主线程的 Choreographer 显示帧回调。 */
internal object ChoreographerFrameClock : FrameClock {
    override fun post(callback: Choreographer.FrameCallback) {
        Choreographer.getInstance().postFrameCallback(callback)
    }

    override fun remove(callback: Choreographer.FrameCallback) {
        Choreographer.getInstance().removeFrameCallback(callback)
    }
}
