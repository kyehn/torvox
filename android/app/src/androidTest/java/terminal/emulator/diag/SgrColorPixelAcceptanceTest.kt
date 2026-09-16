package terminal.emulator.diag

import android.graphics.Bitmap
import android.graphics.Color
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.Bridge
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

    private fun awaitBridge(): Bridge {
        // 会话孵化中桥为 null：确定性轮询就绪，而非单次读取碰运气。
        var ready: Bridge? = null
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                ready = composeTestRule.getBridge()
                ready != null
            }
        assertNotNull("运行时桥必须就绪", seen)
        return ready!!
    }

    private fun isReddish(pixel: Int): Boolean {
        val red = Color.red(pixel)
        val green = Color.green(pixel)
        val blue = Color.blue(pixel)
        return red > 110 && red - blue > 50 && red - green > 30
    }

    private fun isGreenish(pixel: Int): Boolean {
        val red = Color.red(pixel)
        val green = Color.green(pixel)
        val blue = Color.blue(pixel)
        return green > 110 && green - red > 50 && green - blue > 30
    }

    private fun isBluish(pixel: Int): Boolean {
        val red = Color.red(pixel)
        val green = Color.green(pixel)
        val blue = Color.blue(pixel)
        return blue > 110 && blue - red > 50 && blue - green > 30
    }

    private fun countPixels(shot: Bitmap, predicate: (Int) -> Boolean): Int {
        var count = 0
        for (y in 0 until shot.height step 3) {
            for (x in 0 until shot.width step 3) {
                if (predicate(shot.getPixel(x, y))) count++
            }
        }
        return count
    }

    private fun isLightish(pixel: Int): Boolean {
        // Dracula 前景近白：有字形即有亮像素（与红色与否无关）。
        return Color.red(pixel) > 150 && Color.green(pixel) > 150 && Color.blue(pixel) > 150
    }

    private fun countLightPixels(shot: Bitmap): Int {
        var count = 0
        for (y in 0 until shot.height step 3) {
            for (x in 0 until shot.width step 3) {
                if (isLightish(shot.getPixel(x, y))) count++
            }
        }
        return count
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
        val bridge = awaitBridge()
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val beforeCount = countRedPixels(before)
        // 三色标记：若全无色=系统性丢色；若唯红无=红色特异。
        // 真 ESC 字节经 echo 原样送显：不依赖 shell printf 解释转义，
        // Ghostty 收到即解析（单生产者有序，无直写 VT 的提示行竞态）。
        val markers = listOf("RED_LINE" to 31, "GREEN_LINE" to 32, "BLUE_LINE" to 34)
        for ((marker, code) in markers) {
            // echo 不解释参数：真 ESC 原字节直达 PTY，Ghostty 必须解析出颜色。
            val fed = bridge.writeToPty("echo '\u001B[${code}m$marker\u001B[0m'\n".toByteArray())
            assertTrue("颜色块命令必须送达 shell: $marker", fed)
            val gridded =
                UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                    bridge.getTerminalText()?.contains(marker) == true
                }
            assertNotNull("颜色块必须落格, 实际尾部: ${bridge.getTerminalText()?.takeLast(200)}", gridded)
        }
        // 取证：滚动位置（视口内/回滚区）。
        val fullText = bridge.getTerminalText().orEmpty()
        val lines = fullText.split("\n")
        val markerRows = lines.mapIndexedNotNull { index, line -> if (line.contains("LINE")) index else null }
        for ((index, line) in lines.withIndex().take(50)) {
            val rowText = line.replace("\u001B", "<ESC>")
            android.util.Log.i("SgrGrid", "row$index=[$rowText]")
        }
        android.util.Log.i(
            "SgrDiag",
            "scrollback=${bridge.scrollbackLength()} rows=${lines.size} " +
                "markerRows=$markerRows",
        )
        Thread.sleep(1_200)
        // 软件渲染滞后网格：多次采样等红色呈现，同时记亮像素数以切分
        // “面空白”（亮≈0）与“有字无色”（亮≫0 但红≈0）两个故障域。
        var after = device.takeScreenshot() ?: throw AssertionError("截图失败")
        var afterCount = countRedPixels(after)
        var lightCount = countLightPixels(after)
        var greenCount = countPixels(after, ::isGreenish)
        var blueCount = countPixels(after, ::isBluish)
        val deadline = android.os.SystemClock.uptimeMillis() + 20_000
        while (afterCount <= beforeCount + 20 && android.os.SystemClock.uptimeMillis() < deadline) {
            android.util.Log.i("SgrDiag", "red=$afterCount green=$greenCount blue=$blueCount light=$lightCount")
            Thread.sleep(2_500)
            after = device.takeScreenshot() ?: throw AssertionError("截图失败")
            afterCount = countRedPixels(after)
            lightCount = countLightPixels(after)
            greenCount = countPixels(after, ::isGreenish)
            blueCount = countPixels(after, ::isBluish)
        }
        val finalSummary =
            "final red=$afterCount green=$greenCount blue=$blueCount light=$lightCount (before=$beforeCount)"
        android.util.Log.i("SgrDiag", finalSummary)
        // 临时诊断：强制全量重绘（resize 使脏带失效），看红色是否出现。
        // 出现=脏带失效 bug；仍无=着色器 fg 通路 bug。诊断完即删。
        bridge.resize(25, 80)
        Thread.sleep(3_000)
        val redrawn = device.takeScreenshot() ?: throw AssertionError("截图失败")
        android.util.Log.i(
            "SgrDiag",
            "after-resize red=" + countRedPixels(redrawn) + " light=" + countLightPixels(redrawn),
        )
        assertTrue(
            "SGR 红色文本必须产生红色像素 (前=$beforeCount 后=$afterCount 亮=$lightCount)",
            afterCount > beforeCount + 20,
        )
    }
}
