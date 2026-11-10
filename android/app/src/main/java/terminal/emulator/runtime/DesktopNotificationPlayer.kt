package terminal.emulator.runtime

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import terminal.emulator.MainActivity
import terminal.emulator.R
import terminal.emulator.TransientMessageChannel

/**
 * 一次桌面通知：归属会话、标题与正文。
 *
 * [sessionLabel] 是会话序号标签（如「会话 3」）；会话已关闭时为空串，通知照发但不带
 * 序号——程序的通知不该因为会话消失而被整条丢掉。
 */
data class SessionNotification(val sessionId: Long, val sessionLabel: String, val title: String, val body: String)

/**
 * OSC 9 / OSC 777 桌面通知的投递器。
 *
 * 应用在前台时用应用内提示通道呈现（用户在终端前，不需要离开当前界面）；在后台时用系统
 * 通知（应用内提示在后台不可见，用户根本看不到）。
 *
 * 两种动作以 lambda 注入：单测直接验证「前台走应用内提示、后台走系统通知」与文本口径，
 * 不必伪造系统服务。依赖系统服务的默认动作在伴身对象中实现。
 */
class DesktopNotificationPlayer internal constructor(
    private val inAppMessageAction: (String) -> Unit,
    private val systemNotificationAction: (SessionNotification, String) -> Unit,
) {
    constructor(context: Context, messages: TransientMessageChannel) : this(
        inAppMessageAction = { text -> messages.send(text) },
        systemNotificationAction = { notification, text -> showSystemNotification(context, notification, text) },
    )

    /**
     * 投递一条通知：[foreground] 为真走应用内提示通道，否则走系统通知。
     *
     * 文本口径见 [formatText]：会话序号 + 程序给的标题与正文。
     */
    fun dispatch(notification: SessionNotification, foreground: Boolean) {
        val text = formatText(notification)
        if (foreground) {
            inAppMessageAction(text)
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
            // 通知被用户关掉时 notify() 是**静默丢弃**，不抛异常：不先问一句，
            // 通知就凭空消失，日志里也看不出「没到」的原因。
            if (!manager.areNotificationsEnabled()) {
                val droppedCount = notificationsDisabledLog.record()
                if (droppedCount != null) {
                    LogUtil.warn(
                        TAG,
                        "notifications disabled by user; $droppedCount desktop notification(s) dropped",
                    )
                }
                return
            }
            createChannel(context, manager)
            // 点击通知后切到目标会话：请求码取桌面通知 id，同一会话的多次通知复用同一条
            // PendingIntent（FLAG_UPDATE_CURRENT 会更新携带的 extra）。
            val notificationId = desktopNotificationId(notification.sessionId)
            val intent =
                Intent(context, MainActivity::class.java).apply {
                    flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
                    putExtra(MainActivity.EXTRA_NOTIFICATION_SESSION_ID, notification.sessionId)
                }
            val pending =
                PendingIntent.getActivity(
                    context,
                    notificationId,
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
                manager.notify(notificationId, built)
            } catch (exception: Exception) {
                // 通知进不来不该带崩运行期（渠道被删、通知对象失效等）。
                LogUtil.warn(TAG, "notification post failed: ${exception.javaClass.simpleName}", exception)
            }
        }

        /** 通知被关闭时的诊断闸门：程序持续发通知时逐条记会刷屏。 */
        private val notificationsDisabledLog = CountedLogThrottle()

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

/**
 * 桌面通知的系统通知 id。
 *
 * 系统通知的 id 在整个应用内共享一张表，同 id 即同一条通知：桌面通知顶掉前台服务的
 * 常驻通知，用户就丢了「退出」与「唤醒锁」入口；常驻通知刷新（会话数变化）顶掉桌面
 * 通知，用户刚收到的提醒凭空消失。首个会话的 id 是 1，与常驻通知的 id 相同，故桌面
 * 通知的 id 落在独立号段里。
 *
 * 号段自 [DESKTOP_NOTIFICATION_ID_BASE] 起、容量 [DESKTOP_NOTIFICATION_ID_SPACE]：会话
 * id 由原生侧单调自增，实际取值远小于号段容量；取模使结果恒落在号段内，既不会与常驻
 * 通知的 id 相撞，也不会溢出成负数。
 *
 * 同一会话恒得同一 id：同一会话的后续通知仍顶替先前那条，而不是在通知栏里堆叠。
 */
internal fun desktopNotificationId(sessionId: Long): Int =
    DESKTOP_NOTIFICATION_ID_BASE + (sessionId % DESKTOP_NOTIFICATION_ID_SPACE).toInt()

/** 桌面通知 id 号段的起点：与前台服务的常驻通知 id（1）分区。 */
private const val DESKTOP_NOTIFICATION_ID_BASE = 1_000_000

/** 桌面通知 id 号段的容量。 */
private const val DESKTOP_NOTIFICATION_ID_SPACE = 1_000_000
