package terminal.emulator.ui

import androidx.annotation.MainThread

/**
 * Surface 挂载门闸：Surface 被销毁后，渲染 MUST 保持暂停，直到新窗口挂上原生。
 *
 * 平台回收 Surface 时会废弃它的 BufferQueue；此后渲染线程再取帧只会得到
 * `dequeueBuffer failed: No such device (-19)` 与 `ERROR_SURFACE_LOST_KHR`，
 * 交换链就此不可用（返回后永久黑屏）。实测形态：切后台后回前台，恢复渲染
 * 比新窗口挂载早 27ms，那一帧正落在已废弃的队列上。
 *
 * 故「Surface 销毁」那次暂停只能由**新窗口挂上**解除，恢复路径无权解除。
 * 独立于 [android.view.View] 是为了能在 JVM 单测里跑完整条时序——
 * 那条时序在设备上无法稳定构造。
 *
 * **仅限主线程调用**：账本 [RenderPauseLedger] 的持有集合是普通可变状态，非线程
 * 安全——后台线程归还会与主线程的领用交错，计数会算错（多归一次提前放开别人还在
 * 等的暂停，少归一次让渲染永远停在暂停态）。启动序列那条路径在 IO 调度器上挂载
 * 窗口，故那里 MUST 先切回主线程再解除。
 */
@MainThread
internal class SurfaceAttachGate(private val renderPauseLedger: RenderPauseLedger) {
    /** Surface 已销毁且新窗口尚未挂上；仅供诊断与断言使用。 */
    val isAwaitingAttach: Boolean
        get() = pauseToken != null

    /** 「Surface 销毁」那次渲染暂停的凭证；null 表示当前没有在等的暂停。 */
    private var pauseToken: Long? = null

    /**
     * Surface 被销毁：领一次渲染暂停，等新窗口挂上再解除。
     *
     * 重复销毁（销毁后新窗口迟迟不来又销毁一次）先领新的再归还旧的：
     * 反之会让持有者数经过 0，多出一对暂停/恢复。
     */
    fun onSurfaceDestroyed() {
        val previous = pauseToken
        val token = renderPauseLedger.acquire()
        pauseToken = token
        previous?.let { renderPauseLedger.release(it) }
    }

    /**
     * 新窗口已挂上原生：解除那次暂停。
     *
     * 幂等：没有在等的暂停时为空操作（同一窗口的尺寸重绑会多次走到这里）。
     */
    fun onSurfaceAttached() {
        val token = pauseToken ?: return
        pauseToken = null
        renderPauseLedger.release(token)
    }
}
