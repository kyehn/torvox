package terminal.emulator.ui

import androidx.annotation.MainThread

/**
 * 「切后台」那次渲染暂停的持有者。
 *
 * 切后台时领一次暂停：应用不可见期间渲染线程继续跑，只会在已被系统回收的
 * BufferQueue 上取帧（`ERROR_SURFACE_LOST_KHR`，返回后永久黑屏）。
 *
 * **只有「回到前台」这一转场能解除它**，其余路径无权代还。渲染路径上的
 * 「恢复」事件（Surface 重新交付、尺寸防抖到期）可能在应用仍处于后台时到达：
 * 平台收走原生窗口后宿主会换一个新 SurfaceView，新窗口交付时的那次恢复若把
 * 后台暂停一并还掉，渲染线程就重新在后台对着正在消亡的队列取帧——
 * 这正是「切回前台后终端永久黑屏」的形态。
 *
 * 与 [SurfaceAttachGate] 同构：凭证由本类私有持有并自行归还，别处拿不到也
 * 还不掉，故不可能出现「归还了别人的持有」。
 */
internal class BackgroundRenderPause(private val renderPauseLedger: RenderPauseLedger) {
    /**
     * 「切后台」那次渲染暂停的凭证；null 表示当前没有在等的暂停。
     *
     * 写入方是主线程（生命周期转场），读取方 [isPaused] 跑在渲染监视的 IO
     * 作用域：没有 volatile 时后台线程可能永远看不到切后台状态，进而在后台
     * 拉起渲染线程。
     */
    @Volatile
    private var pauseToken: Long? = null

    /** 当前是否持有「切后台」那次暂停；可从任意线程读取。 */
    val isPaused: Boolean
        get() = pauseToken != null

    /**
     * 应用切到后台：领一次渲染暂停。
     *
     * 重复切入后台（未回到前台又切一次）先领新的再归还旧的：反之会让持有者数
     * 经过 0，多出一对暂停/恢复。
     */
    @MainThread
    fun onBackground() {
        val previous = pauseToken
        val token = renderPauseLedger.acquire()
        pauseToken = token
        previous?.let { renderPauseLedger.release(it) }
    }

    /**
     * 应用回到前台：归还切后台那次暂停。
     *
     * 幂等：没有在等的暂停时为空操作（回到前台后又有别的路径触发同一转场）。
     *
     * 先归还再清凭证：归还到清凭证之间，后台线程读到的仍是「已暂停」（保守），
     * 反过来会让它以为已恢复而在后台拉起渲染线程。
     */
    @MainThread
    fun onForeground() {
        val token = pauseToken ?: return
        renderPauseLedger.release(token)
        pauseToken = null
    }
}
