package terminal.emulator.runtime

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicBoolean

/**
 * 桌面通知轮询：把原生通知队列排空并交给投递器。
 *
 * 轮询节奏、幂等启动与单轮失败的处理都收在这里；调用方只提供四个事实：
 * 从哪里取事件、会话序号怎么标、投到哪里、应用是否在前台。前台判定按**投递时刻**
 * 读取，故应用在两次轮询之间切换前后台时，下一轮即走对通道。
 *
 * 通知走独立的原生队列（与事件队列分开），故需单独轮询；且与渲染生命周期无关——
 * 应用在后台时通知仍须及时出现（那时只有会话推进泵在跑，它把通知推进队列）。
 */
internal class DesktopNotificationPump(
    private val scope: CoroutineScope,
    private val pollEvent: () -> String?,
    private val sessionLabel: (Long) -> String,
    private val deliver: (SessionNotification, Boolean) -> Unit,
    private val foreground: () -> Boolean,
) {
    private val started = AtomicBoolean(false)

    /**
     * 幂等启动：`start()` 可能被多次调用（每次 Surface 重建都会走到），而轮询必须
     * 只有一个——两个协程同时消费同一条原生队列，通知会被随机分给其中一个投递器。
     */
    fun start() {
        if (!started.compareAndSet(false, true)) return
        scope.launch {
            while (isActive) {
                try {
                    drainDesktopNotifications(
                        pollEvent = pollEvent,
                        sessionLabel = sessionLabel,
                        deliver = { notification -> deliver(notification, foreground()) },
                    )
                } catch (exception: Exception) {
                    // 单次轮询失败不终止轮询：原生侧把 VT 处理的 panic 转成 Java 异常
                    // 抛出（见 `pollEvent` 的同类保护），异常逸出协程会走默认的未捕获
                    // 异常处理——直接带崩进程。渲染循环对同类失败也是记日志后继续。
                    LogUtil.error(TAG, "desktop notification polling failed", exception)
                }
                delay(POLL_INTERVAL_MS)
            }
        }
    }

    private companion object {
        const val TAG = "Runtime"

        /** 桌面通知轮询间隔（毫秒）：通知低频，100ms 足以让通知及时出现而不空转。 */
        const val POLL_INTERVAL_MS = 100L
    }
}
