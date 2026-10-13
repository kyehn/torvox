package terminal.emulator.ui

/**
 * Surface 归属记账：判定一次 `surfaceDestroyed` 是否还轮得到「暂停渲染」。
 *
 * 渲染输出只有一个消费者，但 Surface 回调可能来自**旧**视图：Activity 重建时旧窗口的
 * `surfaceDestroyed` 可能晚于新视图的 `surfaceCreated` 才到达。全局暂停是非计数布尔，
 * 而清除它的位置全在新视图的恢复路径上（`forceResumeRendering` 与 ON_RESUME）——
 * 旧视图那次迟到回调会把刚恢复的渲染再按停，此后无人再清：终端全黑，而渲染线程
 * 看起来完全健康（无错误、无重启、无自愈）。
 *
 * 与 [RenderPauseLedger] 同因同治，也同样独立于 [android.view.View]：这个记账要在
 * JVM 单测里跑完整条「旧视图销毁晚于新视图创建」的时序，而那条时序在设备上无法构造。
 */
internal class SurfaceOwnership {

    private var owner: Any? = null

    /** 当前归属的视图凭证；仅供诊断与断言使用。 */
    val currentOwner: Any?
        get() = owner

    /** [claim] 由 [release] 归还。凭证是视图实例本身（每个 `TerminalSurface` 一个）。 */
    fun claim(token: Any) {
        owner = token
    }

    /**
     * 归还 [claim] 领到的归属。
     *
     * 返回 `false` 表示这次回调来自**陈旧视图**（或根本没有归属），调用方 MUST NOT
     * 改动任何全局状态——那正是把新视图刚恢复的渲染再按停的那一步。
     */
    fun release(token: Any): Boolean {
        if (owner !== token) return false
        owner = null
        return true
    }
}
