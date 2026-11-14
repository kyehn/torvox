package terminal.emulator.ui

import android.view.View

/**
 * 弹窗窗口的存活追踪：判定一个 `PopupWindow` 的内容视图是否还挂在窗口上。
 *
 * 两个平台事实合起来才够用：
 *
 * - `PopupWindow.isShowing` 只说明「我们调用过 show」——平台直接收走窗口（父视图
 *   窗口令牌变化/销毁）时它仍为 true，只看它就会把菜单永久留在一个已消失的窗口上；
 * - `contentView.isAttachedToWindow` 在「已 `showAtLocation`、首帧遍历尚未完成」这段
 *   窗口里仍为 false——同一帧内的第二次 show 会据此判死窗口、拆掉重建。
 *
 * 被丢弃的那个窗口留下一个从未在 InputManager 注册的输入通道，GC 时打出
 * `InputManager-JNI: Input channel object 'PopupWindow:...' was disposed without
 * first being removed with the input manager!`；拆窗重建本身也造成菜单与手柄闪一下。
 *
 * 故本类把两种事实都记下：内容视图挂上/摘下（平台事实）与「刚 show、还没挂上」
 * （我们自己知道的那段窗口）。**每个窗口一个实例**：旧窗口的迟到回调因此不会
 * 误改新窗口的状态。
 */
internal class PopupWindowLiveness {
    /** 内容视图已挂上窗口（平台事实）。 */
    private var attached = false

    /** 已 show、首帧遍历未完成：这段窗口里窗口仍然活着。 */
    private var pending = false

    /** 窗口已 show：首帧遍历完成前内容视图还没挂上，这段时间窗口同样可用。 */
    fun markShown() {
        pending = true
    }

    /** 内容视图挂上窗口：平台事实落地，「还没挂上」那段窗口结束。 */
    fun markAttached() {
        attached = true
        pending = false
    }

    /** 内容视图离开窗口：平台把窗口收走了，此后不可用。 */
    fun markDetached() {
        attached = false
        pending = false
    }

    /** 窗口是否仍可用（已 show，且尚未被平台收走）。 */
    val alive: Boolean
        get() = attached || pending
}

/**
 * 把内容视图的挂载/摘载事实接进 [liveness]。
 *
 * 挂载与摘载都在平台摘掉窗口时发生：摘载即窗口已不在，不可再用。
 */
internal fun View.bindPopupWindowLiveness(liveness: PopupWindowLiveness) {
    addOnAttachStateChangeListener(
        object : View.OnAttachStateChangeListener {
            override fun onViewAttachedToWindow(view: View) {
                liveness.markAttached()
            }

            override fun onViewDetachedFromWindow(view: View) {
                liveness.markDetached()
            }
        },
    )
}
