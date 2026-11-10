package terminal.emulator

import app.cash.turbine.test
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * [TransientMessageChannel] 的投递语义。
 *
 * 判别力：通道没有界面订阅者时消息必须直接丢弃——排队补发会让用户在一段无关的操作之后
 * 收到一条过时反馈（比不反馈更糟），而排队本身还要占住平台提示窗口那条限额预算。
 */
class TransientMessageChannelTest {

    @Test
    fun `a sent message reaches the collector`() = runTest {
        val channel = TransientMessageChannel()

        channel.flow.test {
            channel.send("字体已应用")
            assertEquals("字体已应用", awaitItem())
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `a message sent without a subscriber is dropped`() = runTest {
        val channel = TransientMessageChannel()
        channel.send("无人接收")

        channel.flow.test {
            channel.send("有人接收")
            assertEquals("有人接收", awaitItem())
            expectNoEvents()
            cancelAndIgnoreRemainingEvents()
        }
    }
}
