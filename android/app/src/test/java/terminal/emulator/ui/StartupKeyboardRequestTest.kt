package terminal.emulator.ui

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * `hide-soft-keyboard-on-startup = false` 的启动请求。
 *
 * 能验证的只有「视图从此愿意接收文本输入」这一步：真正弹不弹由窗口焦点与平台决定，
 * 单元测试里既没有窗口也没有输入法。若这一步不做，平台连问都不会问，键盘永远不出现。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class StartupKeyboardRequestTest {
    private fun surface() = TerminalSurface(ApplicationProvider.getApplicationContext<Context>())

    @Test
    fun `startup request makes the view accept text input`() {
        val surface = surface()
        assertFalse(surface.onCheckIsTextEditor())
        surface.requestKeyboardOnStart()
        assertTrue(surface.onCheckIsTextEditor())
    }

    /** 显式禁用软键盘优先于启动策略：用户关掉的键盘不该被一次启动请求重新打开。 */
    @Test
    fun `an explicitly disabled keyboard stays disabled`() {
        val surface = surface()
        surface.softKeyboardEnabled = false
        surface.requestKeyboardOnStart()
        assertFalse(surface.onCheckIsTextEditor())
    }
}
