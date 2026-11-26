package terminal.emulator.runtime

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 桌面通知轮询：幂等启动、按节拍排空、前台判定取投递时刻。
 *
 * 判别力在取用次数：启动两次只允许一条循环消费队列；前台标志在两次投递之间翻转时，
 * 第二条必须走新值——两处都只有计数/结果能证明，输出相同不足以证明没有重复消费。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class DesktopNotificationPumpTest {
    private fun notificationJson(body: String): String =
        """{"event":"desktop_notification","session_id":1,"title":"","body":"$body"}"""

    @Test
    fun `start is idempotent and delivery reads foreground at dispatch time`() = runTest {
        val queue = ArrayDeque(listOf(notificationJson("first"), null, notificationJson("second"), null))
        var pollCount = 0
        var foreground = true
        val delivered = mutableListOf<Pair<String, Boolean>>()
        val pump =
            DesktopNotificationPump(
                scope = backgroundScope,
                pollEvent = {
                    pollCount += 1
                    queue.removeFirstOrNull()
                },
                sessionLabel = { "会话 $it" },
                deliver = { notification, isForeground -> delivered += notification.body to isForeground },
                foreground = { foreground },
            )

        pump.start()
        pump.start()
        advanceTimeBy(1)
        assertEquals("启动两次只能有一条轮询循环", 2, pollCount)
        assertEquals(listOf("first" to true), delivered)

        foreground = false
        advanceTimeBy(100)
        assertEquals(listOf("first" to true, "second" to false), delivered)
    }

    @Test
    fun `single poll failure does not stop the polling loop`() = runTest {
        val queue = ArrayDeque(listOf(notificationJson("after failure"), null))
        var failNextPoll = true
        val delivered = mutableListOf<String>()
        val pump =
            DesktopNotificationPump(
                scope = backgroundScope,
                pollEvent = {
                    if (failNextPoll) {
                        failNextPoll = false
                        throw IllegalStateException("native poll failed")
                    }
                    queue.removeFirstOrNull()
                },
                sessionLabel = { "" },
                deliver = { notification, _ -> delivered += notification.body },
                foreground = { true },
            )

        pump.start()
        advanceTimeBy(1)
        assertEquals("失败的一轮不得投递", emptyList<String>(), delivered)

        advanceTimeBy(100)
        assertEquals(listOf("after failure"), delivered)
    }
}
