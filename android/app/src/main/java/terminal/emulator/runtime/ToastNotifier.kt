package terminal.emulator.runtime

import android.content.Context
import android.widget.Toast

/**
 * 单例 Toast 投递：同一时刻只留一条，新的到来先取消旧的。
 *
 * 平台对同一 UID 同时只允许一个 Toast 窗口。应用在短时间内连续弹两条（连续切换
 * 字体、切换输入法开关）时，第二条会因旧窗口尚未摘除而抛 `BadTokenException`，
 * 平台同时打出 W 级告警——应用自身触发的告警必须消除，不能靠忽略。
 *
 * 取消与展示都经主线程的消息队列：取消在前、展示在后，平台先摘旧窗口再挂新窗口。
 *
 * 只可在主线程调用：Toast 的构造与展示都要求有 Looper。
 */
object ToastNotifier {
    /** 仍在显示的那条 Toast：平台要等它被摘掉才肯接纳下一条。 */
    private var showing: Toast? = null

    /**
     * 展示一条 Toast，取代仍在显示的那条。
     */
    fun show(context: Context, text: CharSequence, duration: Int = Toast.LENGTH_SHORT) {
        showing?.cancel()
        // 用应用上下文建 Toast：Toast 会持有建它的上下文，沿用 Activity 上下文会
        // 让这条静态引用把 Activity 钉住。应用主题与界面主题同源，外观一致。
        val toast = Toast.makeText(context.applicationContext, text, duration)
        showing = toast
        toast.show()
    }
}
