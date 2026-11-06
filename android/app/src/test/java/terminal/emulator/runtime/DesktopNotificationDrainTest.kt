package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 桌面通知排空：每轮有上限、队列为空即返回、解码失败只丢该条。
 *
 * 判别力：排空不设上限时，通知的产出快于消费就会让循环永不结束，轮询再也回不到
 * 等待点，投递速率随之失控——故本组用例钉住「一轮最多取用固定条数」。
 *
 * 用例喂的是原生侧真实 JSON 形状（`native/src/event.rs`）：解码走真解码器，
 * 「解码失败」由畸形 JSON 触发，不靠替身模拟。
 */
class DesktopNotificationDrainTest {
    /** 构造一条原生侧形状的通知 JSON。 */
    private fun notificationJson(body: String, sessionId: Long = 1): String =
        """{"event":"desktop_notification","session_id":$sessionId,"title":"","body":"$body"}"""

    /** 会话标签：按会话 id 给出序号标签。 */
    private fun label(sessionId: Long) = "会话 $sessionId"

    /** 洪泛时排空有界：达到上限即停止，其余留在原生队列里。 */
    @Test
    fun `drain stops at the limit and leaves the rest queued`() {
        val queue = (1..100).map { notificationJson("note$it") }.toMutableList()
        var pollCount = 0
        val deliveredBodies = mutableListOf<String>()
        val delivered =
            drainDesktopNotifications(
                pollEvent = {
                    pollCount += 1
                    queue.removeFirstOrNull()
                },
                sessionLabel = ::label,
                deliver = { deliveredBodies.add(it.body) },
                maxNotifications = 4,
            )
        assertEquals(4, delivered)
        assertEquals(4, pollCount)
        assertEquals(listOf("note1", "note2", "note3", "note4"), deliveredBodies)
        assertEquals(96, queue.size)
    }

    /** 队列为空即返回：不投递、不空转。 */
    @Test
    fun `empty queue returns without delivering`() {
        var pollCount = 0
        val delivered =
            drainDesktopNotifications(
                pollEvent = {
                    pollCount += 1
                    null
                },
                sessionLabel = ::label,
                deliver = { throw AssertionError("空队列时不得投递") },
            )
        assertEquals(0, delivered)
        assertEquals(1, pollCount)
    }

    /** 解码失败只丢这一条：坏数据不得带崩排空，也不得带走后面的通知。 */
    @Test
    fun `decode failure drops only that entry`() {
        val queue = mutableListOf("not json", notificationJson("good"))
        val deliveredBodies = mutableListOf<String>()
        val delivered =
            drainDesktopNotifications(
                pollEvent = { queue.removeFirstOrNull() },
                sessionLabel = ::label,
                deliver = { deliveredBodies.add(it.body) },
            )
        assertEquals(1, delivered)
        assertEquals(listOf("good"), deliveredBodies)
        assertTrue(queue.isEmpty())
    }

    /** 超出上限的通知由下一轮继续排空：分轮不得丢通知。 */
    @Test
    fun `remaining notifications are drained in the next round`() {
        val queue =
            mutableListOf(
                notificationJson("first"),
                notificationJson("second"),
                notificationJson("third"),
            )
        val deliveredBodies = mutableListOf<String>()
        val firstRound =
            drainDesktopNotifications(
                pollEvent = { queue.removeFirstOrNull() },
                sessionLabel = ::label,
                deliver = { deliveredBodies.add(it.body) },
                maxNotifications = 2,
            )
        val secondRound =
            drainDesktopNotifications(
                pollEvent = { queue.removeFirstOrNull() },
                sessionLabel = ::label,
                deliver = { deliveredBodies.add(it.body) },
                maxNotifications = 2,
            )
        assertEquals(2, firstRound)
        assertEquals(1, secondRound)
        assertEquals(listOf("first", "second", "third"), deliveredBodies)
        assertTrue(queue.isEmpty())
    }

    /** 非通知事件被忽略：通知队列里混进别的变体时不得投递出空通知。 */
    @Test
    fun `non notification events are ignored`() {
        val queue = mutableListOf("""{"event":"bell","session_id":1}""")
        val delivered =
            drainDesktopNotifications(
                pollEvent = { queue.removeFirstOrNull() },
                sessionLabel = ::label,
                deliver = { throw AssertionError("非通知事件不得投递") },
            )
        assertEquals(0, delivered)
    }

    /** 投递内容带会话序号标签：会话序号取会话表的升序下标。 */
    @Test
    fun `delivered notification carries the session label`() {
        val queue = mutableListOf(notificationJson("构建完成", sessionId = 7))
        var delivered: SessionNotification? = null
        val count =
            drainDesktopNotifications(
                pollEvent = { queue.removeFirstOrNull() },
                sessionLabel = ::label,
                deliver = { delivered = it },
            )
        assertEquals(1, count)
        assertEquals(
            SessionNotification(sessionId = 7, sessionLabel = "会话 7", title = "", body = "构建完成"),
            delivered,
        )
    }
}
