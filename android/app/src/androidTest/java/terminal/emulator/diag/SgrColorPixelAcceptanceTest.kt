package terminal.emulator.diag

import android.graphics.Bitmap
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.Bridge
import terminal.emulator.countBluishPixels
import terminal.emulator.countGreenishPixels
import terminal.emulator.countReddishPixels
import terminal.emulator.getBridge
import terminal.emulator.placeTextAtRow
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 颜色像素验收：SGR 彩色文本必须在屏幕上产生对应色相的主导像素，
 * 而非只显示背景（回归“颜色文本只显示背景”类缺失）。
 *
 * 标记写进**运行时正在呈现的会话**：渲染循环与截图同源，无需任何跨会话
 * 呈现技巧。隔离会话 + 原生直绘不可行——`render_inner` 的空闲分支按
 * `last_frame` 所属会话决定是否重绘，运行时会话一旦产出新输出即覆盖隔离帧，
 * 且此后隔离会话在空闲分支恒被判为“会话不一致”而永不重绘，标记永久丢失
 * （实测红/蓝像素恒为 0）。字节只过 Ghostty 解析器（直写 VT，不经 shell 行编辑）。
 */
class SgrColorPixelAcceptanceTest : TerminalLogcatTest() {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.waitForTerminalPixels()
    }

    private fun awaitBridge(): Bridge {
        // 会话孵化中桥为 null：确定性轮询就绪，而非单次读取碰运气。
        var ready: Bridge? = null
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                ready = composeTestRule.getBridge()
                ready != null
            }
        assertNotNull("运行时桥必须就绪", seen)
        return requireNotNull(ready) { "运行时桥必须就绪" }
    }

    @Test
    fun sgrRedTextProducesRedPixels() {
        val bridge = awaitBridge()
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val beforeRed = countReddishPixels(before)
        val beforeGreen = countGreenishPixels(before)
        val beforeBlue = countBluishPixels(before)
        // 三色各占一行：同行的三段在小网格（CI 模拟器仅 25 列）上必折行，
        // 折行后行内位置随网格变化。行号取视口上部，任何后续输出滚动视口都
        // 不会把它们推出可见区。
        val markers = listOf(
            Triple(2, "EEE_RED", "\u001B[31m"),
            Triple(4, "EEE_GREEN", "\u001B[32m"),
            Triple(6, "EEE_BLUE", "\u001B[34m"),
        )
        for ((row, text, sgr) in markers) {
            placeTextAtRow(bridge, row, "$sgr$text\u001B[0m")
        }
        // 呈现是异步的（运行时循环节拍）：落格不等于已上屏，盲等固定时长在慢机上
        // 不可靠；改为轮询截图直到三色增益出现，超时大声失败。运行时循环持续呈现
        // 同一网格，每轮截图都是有效采样，不存在覆盖竞态。
        var red = 0
        var green = 0
        var blue = 0
        val gained =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 500) {
                val shot = device.takeScreenshot() ?: return@pollUntilTrue false
                red = countReddishPixels(shot)
                green = countGreenishPixels(shot)
                blue = countBluishPixels(shot)
                red > beforeRed + PIXEL_GAIN_THRESHOLD &&
                    green > beforeGreen + PIXEL_GAIN_THRESHOLD &&
                    blue > beforeBlue + PIXEL_GAIN_THRESHOLD
            }
        assertNotNull("SGR 三色必须呈现 (红=$red 绿=$green 蓝=$blue 前=$beforeRed/$beforeGreen/$beforeBlue)", gained)
        assertTrue(
            "SGR 红色文本必须产生红色像素 (前=$beforeRed 现=$red)",
            red > beforeRed + PIXEL_GAIN_THRESHOLD,
        )
        assertTrue(
            "SGR 绿色文本必须产生绿色像素 (前=$beforeGreen 现=$green)",
            green > beforeGreen + PIXEL_GAIN_THRESHOLD,
        )
        assertTrue(
            "SGR 蓝色文本必须产生蓝色像素 (前=$beforeBlue 现=$blue)",
            blue > beforeBlue + PIXEL_GAIN_THRESHOLD,
        )
    }

    companion object {
        private const val PIXEL_GAIN_THRESHOLD = 20
    }
}
