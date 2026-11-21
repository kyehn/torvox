package terminal.emulator.runtime

import android.content.Context
import android.util.Log
import androidx.annotation.MainThread
import androidx.test.core.app.ApplicationProvider
import io.mockk.mockk
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowLog
import terminal.emulator.TransientMessageChannel
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.settings.TermuxPropertiesRepository

/**
 * 正常退化路径的日志等级：必须是 debug（或 info），不得是 warn/error。
 *
 * 这三条路径都是产品的正常行为：会话刚关闭时输入法又提交了一次文本、Activity 重建
 * 时旧窗口的销毁回调晚于新视图到达、用户点了「新建会话（应急）」快捷方式。
 * 把它们记成 warn 等于把正常输入当成应用缺陷，`logcat` 里从此分不清「真问题」与
 * 「正常噪声」——而 `logcat-hygiene` 规范的第一条要求正是这条闸门。
 *
 * 判别力：断言的是**等级**而非「有没有日志」——同一条消息改成 warn 仍然有日志，
 * 只有钉住等级才能挡住「正常路径被升级成告警」的回归。
 */
@RunWith(RobolectricTestRunner::class)
class NormalPathLogSeverityTest {

    private val runtime: TerminalRuntime by lazy {
        TerminalRuntime(
            context = ApplicationProvider.getApplicationContext<Context>(),
            settingsRepository = mockk<SettingsRepository>(relaxed = true),
            termuxProperties = mockk<TermuxPropertiesRepository>(relaxed = true),
            transientMessages = TransientMessageChannel(),
        )
    }

    @Before
    fun setUp() {
        LogUtil.setLevel(LogLevel.DEBUG)
        ShadowLog.clear()
    }

    @After
    fun tearDown() {
        LogUtil.setLevel(LogLevel.DEFAULT)
    }

    @Test
    fun `input to a closed session is a debug line`() {
        assertFalse(
            "写入不存在的会话必须如实报告失败",
            runtime.writeToPty(42L, byteArrayOf('x'.code.toByte())),
        )
        val logs = ShadowLog.getLogsForTag(RUNTIME_LOG_TAG)
        assertTrue("未写入必须留有诊断", logs.any { it.type == Log.DEBUG && it.msg.contains("不可写") })
        assertTrue("会话关闭竞态不得记为警告或错误", logs.none { it.type >= Log.WARN })
    }

    @Test
    @MainThread
    fun `stale surface destroy is a debug line`() {
        runtime.onSurfaceClaimed("new-view")
        assertFalse("陈旧视图的销毁回调必须被忽略", runtime.onSurfaceDestroyed("old-view"))
        val logs = ShadowLog.getLogsForTag(RUNTIME_LOG_TAG)
        assertTrue(
            "陈旧视图的回调必须留有诊断",
            logs.any { it.type == Log.DEBUG && it.msg.contains("stale view ignored") },
        )
        assertTrue("Activity 重建期的正常时序不得记为警告或错误", logs.none { it.type >= Log.WARN })
    }

    @Test
    fun `failsafe request is an info line`() {
        runtime.requestFailsafeSession()
        val logs = ShadowLog.getLogsForTag(RUNTIME_LOG_TAG)
        assertTrue(
            "应急会话请求必须留有诊断",
            logs.any { it.type == Log.INFO && it.msg.contains("Failsafe session requested") },
        )
        assertTrue("规范明列的入口不得记为警告或错误", logs.none { it.type >= Log.WARN })
    }
}
