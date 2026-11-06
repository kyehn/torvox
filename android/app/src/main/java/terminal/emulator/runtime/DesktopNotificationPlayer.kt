package terminal.emulator.runtime

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import terminal.emulator.MainActivity
import terminal.emulator.R

/**
 * 一次桌面通知：归属会话、标题与正文。
 *
 * [sessionLabel] 是会话序号标签（如「会话 3」）；会话已关闭时为空串，通知照发但不带
 * 序号——程序的通知不该因为会话消失而被整条丢掉。
 */
data class SessionNotification(val sessionId: Long, val sessionLabel: String, val title: String, val body: String)

/**
 * OSC 9 / OSC 777 桌面通知的投递器（DESIGN：处理 OSC 9 等通知，输出会话序号和通知）。
 *
 * 应用在前台时用 Toast（用户在终端前，不需要离开当前界面）；在后台时用系统通知
 * （Toast 在后台不可见，用户根本看不到）。
 *
 * 两种动作以 lambda 注入：单测直接验证「前台走 Toast、后台走系统通知」与文本口径，
 * 不必伪造系统服务。依赖系统服务的默认动作在伴身对象中实现。
 */
class DesktopNotificationPlayer internal constructor(
    private val toastAction: (SessionNotification, String) -> Unit,
    private val systemNotificationAction: (SessionNotification, String) -> Unit,
) {
    constructor(context: Context) : this(
        toastAction = { notification, text -> showToast(context, text) },
        systemNotificationAction = { notification, text -> showSystemNotification(context, notification, text) },
    )

    /**
     * 投递一条通知：[foreground] 为真走 Toast，否则走系统通知。
     *
     * 文本口径见 [formatText]：会话序号 + 程序给的标题与正文。
     */
    fun dispatch(notification: SessionNotification, foreground: Boolean) {
        val text = formatText(notification)
        if (foreground) {
            toastAction(notification, text)
        } else {
            systemNotificationAction(notification, text)
        }
    }

    /**
     * 通知文本：会话序号 + 程序给的标题与正文。
     *
     * 标题与正文都为空时退回会话标签——空通知在系统栏里是一行空白，用户无从判断
     * 发生了什么。
     */
    fun formatText(notification: SessionNotification): String {
        val content =
            listOf(notification.title, notification.body)
                .filter { it.isNotEmpty() }
                .joinToString(" ")
        if (notification.sessionLabel.isEmpty()) return content
        if (content.isEmpty()) return notification.sessionLabel
        return "${notification.sessionLabel}: $content"
    }

    private companion object {
        const val TAG = "DesktopNotification"

        /** 桌面通知渠道 id：与前台服务的常驻通知渠道分开（重要性不同，见下）。 */
        const val CHANNEL_ID = "desktop_notification"

        /**
         * Toast 在主线程展示：Toast 的构造与展示都要求有 Looper。
         *
         * 惰性求值：单测只注入动作、不碰系统服务，而类初始化时构造 Handler 需要
         * 真实的 Looper（非 Robolectric 的 JVM 单测里拿不到）。
         */
        val mainHandler: Handler by lazy { Handler(Looper.getMainLooper()) }

        /**
         * 前台展示：Toast。
         *
         * 投递到主线程：本类的动作由运行期的 IO 协程调用，而 Toast 的构造需要
         * Looper（无 Looper 的线程上构造会抛 IllegalStateException）。
         */
        fun showToast(context: Context, text: String) {
            mainHandler.post {
                try {
                    Toast.makeText(context, text, Toast.LENGTH_LONG).show()
                } catch (exception: Exception) {
                    // Toast 依赖窗口与上下文：进程被杀后重建的短暂窗口里可能失败。
                    // 通知丢失不得带崩运行期。
                    LogUtil.warn(TAG, "toast failed: ${exception.javaClass.simpleName}", exception)
                }
            }
        }

        /**
         * 后台展示：系统通知。
         *
         * 重要性取 [NotificationManager.IMPORTANCE_DEFAULT]：程序的通知要出声，
         * 与常驻会话通知（IMPORTANCE_LOW，只表示「还活着」）语义不同，故另立渠道。
         */
        fun showSystemNotification(context: Context, notification: SessionNotification, text: String) {
            val manager = context.getSystemService(NotificationManager::class.java)
            if (manager == null) {
                LogUtil.warn(TAG, "notification manager unavailable; notification dropped")
                return
            }
            createChannel(context, manager)
            // 点击通知后切到目标会话：请求码取会话 id，同一会话的多次通知复用同一条
            // PendingIntent（FLAG_UPDATE_CURRENT 会更新携带的 extra）。
            val intent =
                Intent(context, MainActivity::class.java).apply {
                    flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
                    putExtra(MainActivity.EXTRA_NOTIFICATION_SESSION_ID, notification.sessionId)
                }
            val pending =
                PendingIntent.getActivity(
                    context,
                    notification.sessionId.toInt(),
                    intent,
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                )
            val built =
                Notification
                    .Builder(context, CHANNEL_ID)
                    .setContentTitle(notification.title.ifEmpty { notification.sessionLabel })
                    .setContentText(notification.body.ifEmpty { text })
                    .setSubText(notification.sessionLabel)
                    .setSmallIcon(R.drawable.ic_notification)
                    .setContentIntent(pending)
                    .setAutoCancel(true)
                    .setCategory(Notification.CATEGORY_EVENT)
                    .build()
            try {
                manager.notify(notification.sessionId.toInt(), built)
            } catch (exception: Exception) {
                // POST_NOTIFICATIONS 未授权时 notify 抛 SecurityException：通知进不来
                // 不该带崩运行期。
                LogUtil.warn(TAG, "notification post failed: ${exception.javaClass.simpleName}", exception)
            }
        }

        /** 建渠道：渠道一经创建其重要性不可改，故每次投递前确保它存在。 */
        fun createChannel(context: Context, manager: NotificationManager) {
            val channel =
                NotificationChannel(
                    CHANNEL_ID,
                    context.getString(R.string.notification_desktop_channel_name),
                    NotificationManager.IMPORTANCE_DEFAULT,
                ).apply {
                    description = context.getString(R.string.notification_desktop_channel_desc)
                }
            manager.createNotificationChannel(channel)
        }
    }
}
