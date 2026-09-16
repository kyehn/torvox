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
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.getBridge
import terminal.emulator.grantNotificationPermission

/**
 * 颜色像素验收：SGR 红色文本必须在屏幕上产生红色主导像素，
 * 而非只显示背景（回归“颜色文本只显示背景”类缺失）。
 *
 * 隔离会话 + 直写 VT（feedTerminal）：静止门等启动风暴过后，
 * shell 空闲无竞争。绝不经 shell 键入转义——shell 行编辑器把 ESC
 * 当元键前缀吃掉，命令不成形、无输出，网格断言只能匹配到自己的
 * 回显（空断言）。差分法：喂色块前后截图计数比较，不依赖光标行列定位，
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

    private fun countRedPixels(shot: Bitmap): Int = countPixels(shot, ::isReddish)

    /**
     * 直绘验收：隔离会话 + 直接 render()，与运行时共享 surface。
     *
     * 渲染暂停与直接呈现互斥（render_frame_with_plan 在 paused 时直接
     * 返回 Ok 且不呈现），因此本测试全程不暂停：运行时线程约每 500ms
     * 重绘其自有会话，可能覆盖本会话帧；每次迭代先呈现再立即截图，
     * 取多轮最大红色计数判决——只要管线能呈现红色，必有一帧命中。
     * 字节只过 Ghostty 解析器（直写 VT，不经 shell 行编辑器）。
     */
    @Test
    fun sgrRedTextProducesRedPixels() {
        awaitBridge() // 运行时就绪（surface 已挂载）即可；本测试不用其会话。
        // 固定 24x80（与 VtCorrectness 同口径）：getGridRowsColsPacked 的行数
        // 含回滚、列数是内容区折算值，不可直接用作新会话视口几何。
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = context.filesDir.resolve("sgr-test-home").apply { mkdirs() }.absolutePath
        val sessionId = NativeBridge.initSession(24, 80, "/system/bin/sh", home, home, "", 2000)
        assertTrue("隔离会话创建失败", sessionId != 0L)
        try {
            val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
            val beforeRed = countRedPixels(before)
            // 隔离会话自有 VT（shell 空闲无输出）：直写字节只过 Ghostty 解析器。
            val markers = listOf("RED_LINE" to 31, "GREEN_LINE" to 32, "BLUE_LINE" to 34)
            for ((marker, code) in markers) {
                NativeBridge.feedTerminal(
                    sessionId,
                    "\u001B[${code}m$marker\u001B[0m\r\n".toByteArray(Charsets.UTF_8),
                )
                val gridded =
                    UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                        NativeBridge.getTerminalText(sessionId)?.replace("\n", "")?.contains(marker) == true
                    }
                assertNotNull(
                    "颜色块必须落格, 实际尾部: ${NativeBridge.getTerminalText(sessionId)?.takeLast(200)}",
                    gridded,
                )
            }
            // 运行时线程会重绘其自有会话帧：每轮先呈现本会话再立即截图，
            // 取最大红色计数——呈现成功即有一轮命中红色。
            var maxRed = 0
            var maxGreen = 0
            var maxBlue = 0
            repeat(SamplingRoundCount) {
                val rendered = NativeBridge.render(sessionId, 0, 0)
                val shot = device.takeScreenshot() ?: throw AssertionError("截图失败")
                maxRed = maxOf(maxRed, countRedPixels(shot))
                maxGreen = maxOf(maxGreen, countPixels(shot, ::isGreenish))
                maxBlue = maxOf(maxBlue, countPixels(shot, ::isBluish))
                android.util.Log.i("SgrDiag", "render rc=$rendered red=$maxRed green=$maxGreen blue=$maxBlue")
                Thread.sleep(SamplingIntervalMillis)
            }
            android.util.Log.i(
                "SgrDiag",
                "maxRed=$maxRed maxGreen=$maxGreen maxBlue=$maxBlue (beforeRed=$beforeRed)",
            )
            assertTrue(
                "SGR 红色文本必须产生红色像素 (前=$beforeRed 最大红=$maxRed 绿=$maxGreen 蓝=$maxBlue)",
                maxRed > beforeRed + PixelGainThreshold,
            )
        } finally {
            runCatching { NativeBridge.destroySession(sessionId) }
        }
    }

    companion object {
        private const val SamplingRoundCount = 10
        private const val SamplingIntervalMillis = 400L
        private const val PixelGainThreshold = 20
    }
}
