package terminal.emulator.diag

import android.graphics.Bitmap
import android.graphics.Color
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.grantNotificationPermission

/**
 * 颜色像素验收：SGR 红色文本必须在屏幕上产生红色主导像素，
 * 而非只显示背景（回归“颜色文本只显示背景”类缺失）。
 *
 * 差分法：喂红色块前后截图计数比较，不依赖光标行列定位，
 * 避开共享 shell 会话滚动带来的行号漂移。
 */
class SgrColorPixelAcceptanceTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        grantNotificationPermission()
        composeTestRule.waitUntil(timeoutMillis = 60_000) {
            try {
                composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
                true
            } catch (_: AssertionError) {
                false
            } catch (_: Exception) {
                false
            }
        }
        Thread.sleep(1_500)
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private fun isReddish(pixel: Int): Boolean {
        val red = Color.red(pixel)
        val green = Color.green(pixel)
        val blue = Color.blue(pixel)
        return red > 110 && red - blue > 50 && red - green > 30
    }

    private fun countRedPixels(shot: Bitmap): Int {
        var count = 0
        // 步进 3 像素采样：红色文本块远大于此粒度，速度与稳定性兼顾。
        for (y in 0 until shot.height step 3) {
            for (x in 0 until shot.width step 3) {
                if (isReddish(shot.getPixel(x, y))) count++
            }
        }
        return count
    }

    @Test
    fun sgrRedTextProducesRedPixels() {
        val bridge = bridge()
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val beforeCount = countRedPixels(before)
        // 三行红色块：像素数远超噪声，且 \u001b[0m 后恢复默认。
        bridge.feedTerminal("\u001b[31mRED_LINE_ONE\u001b[0m\n".toByteArray())
        bridge.feedTerminal("\u001b[31mRED_LINE_TWO\u001b[0m\n".toByteArray())
        bridge.feedTerminal("\u001b[31mRED_LINE_THREE\u001b[0m\n".toByteArray())
        Thread.sleep(1_200)
        val after = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val afterCount = countRedPixels(after)
        assertTrue(
            "SGR 红色文本必须产生红色像素 (前=$beforeCount 后=$afterCount)",
            afterCount > beforeCount + 20,
        )
    }
}
