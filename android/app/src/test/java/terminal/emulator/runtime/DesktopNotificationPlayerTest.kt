package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 桌面通知投递器：前后台分派与文本口径。
 *
 * 动作以 lambda 注入，不需要 Robolectric：断言只关心「前台走应用内提示、后台走系统
 * 通知」与「文本里带会话序号和程序给的正文」。
 */
class DesktopNotificationPlayerTest {
    /** 收集两种动作的调用记录。 */
    private class Recorder {
        val inAppMessages = mutableListOf<String>()
        val systemNotifications = mutableListOf<String>()

        fun player() = DesktopNotificationPlayer(
            inAppMessageAction = { text -> inAppMessages.add(text) },
            systemNotificationAction = { _, text -> systemNotifications.add(text) },
        )
    }

    private fun notification(
        sessionId: Long = 7L,
        sessionLabel: String = "会话 2",
        title: String = "",
        body: String = "构建完成",
    ) = SessionNotification(
        sessionId = sessionId,
        sessionLabel = sessionLabel,
        title = title,
        body = body,
    )

    @Test
    fun `foreground notification goes to the in-app message only`() {
        val recorder = Recorder()
        recorder.player().dispatch(notification(), foreground = true)
        assertEquals(listOf("会话 2: 构建完成"), recorder.inAppMessages)
        assertEquals(emptyList<String>(), recorder.systemNotifications)
    }

    @Test
    fun `background notification goes to system notification only`() {
        val recorder = Recorder()
        recorder.player().dispatch(notification(), foreground = false)
        assertEquals(emptyList<String>(), recorder.inAppMessages)
        assertEquals(listOf("会话 2: 构建完成"), recorder.systemNotifications)
    }

    /** OSC 777 带标题：标题与正文都在文本里，顺序为标题在前。 */
    @Test
    fun `title and body are both carried`() {
        val recorder = Recorder()
        recorder.player().dispatch(notification(title = "构建", body = "完成"), foreground = true)
        assertEquals(listOf("会话 2: 构建 完成"), recorder.inAppMessages)
    }

    /** 会话已关闭（序号未知）：通知照发，只是不带会话序号。 */
    @Test
    fun `unknown session drops the label prefix`() {
        val recorder = Recorder()
        recorder.player().dispatch(notification(sessionLabel = ""), foreground = true)
        assertEquals(listOf("构建完成"), recorder.inAppMessages)
    }

    /** 程序既没给标题也没给正文：退回会话标签，不是一条空通知。 */
    @Test
    fun `empty notification falls back to the session label`() {
        val recorder = Recorder()
        recorder.player().dispatch(notification(body = ""), foreground = true)
        assertEquals(listOf("会话 2"), recorder.inAppMessages)
    }
}
