package terminal.emulator

import android.util.Log
import android.view.KeyCharacterMap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowLog
import terminal.emulator.runtime.LogLevel
import terminal.emulator.runtime.LogUtil

/**
 * 死键后接无预组合形式的字母：丢弃重音是设计行为，日志只能是 debug。
 *
 * 死键（如 AZERTY 的 `´`）本身不产生字符，等下一次击键合成。`x` 没有带锐音符的
 * 预组合形式，于是重音按设计丢弃、裸字母照发——这是正常输入，逐次记 warn 等于
 * 把用户的键盘布局当异常。
 *
 * 判别力：断言的是**等级**而非「有没有日志」——同一条消息改成 warn 仍然有日志，
 * 只有钉住等级才能挡住「正常路径被升级成告警」的回归。
 *
 * 走字符级的入口而非伪造 [android.view.KeyEvent]：在测试进程里继承 Robolectric
 * 插桩过的框架类会与 mockk 的 inline agent 相撞，把测试 JVM 弄崩；字符级的
 * 合成逻辑本来就独立于平台事件。
 */
@RunWith(RobolectricTestRunner::class)
@OptIn(ExperimentalCoroutinesApi::class)
class DeadKeyLogSeverityTest {

    @Before
    fun setUp() {
        Dispatchers.setMain(StandardTestDispatcher())
        LogUtil.setLevel(LogLevel.DEBUG)
        ShadowLog.clear()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
        LogUtil.setLevel(LogLevel.DEFAULT)
    }

    @Test
    fun `dead key without a precomposed form is a debug line`() {
        val viewModel = createTestViewModel(createTestRuntime(sessionIds = emptyList()))

        // 死键本身不产生字符，等下一次击键合成。
        assertNull(
            "死键本身不产生输出",
            viewModel.resolveUnicodeChar(deadKeyChar, hasAltGr = false),
        )
        // `x` 没有带锐音符的预组合形式：重音按设计丢弃，裸字母照发。
        assertEquals(
            "裸字母照发",
            'x'.code,
            viewModel.resolveUnicodeChar('x'.code, hasAltGr = false),
        )

        val logs = ShadowLog.getLogsForTag("TerminalViewModel")
        assertTrue(
            "丢弃重音必须留有诊断",
            logs.any { it.type == Log.DEBUG && it.msg.contains("no combination") },
        )
        assertTrue("正常输入不得记为警告或错误", logs.none { it.type >= Log.WARN })
    }

    private companion object {
        /** 死键字符：组合符位（`COMBINING_ACCENT`）与组合锐音符（U+0301）之和。 */
        val deadKeyChar: Int = KeyCharacterMap.COMBINING_ACCENT or 0x0301
    }
}
