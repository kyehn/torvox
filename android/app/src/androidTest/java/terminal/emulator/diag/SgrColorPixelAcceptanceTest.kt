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
 * 共享前台会话 + 直写 VT（feedTerminal）：静止门等启动风暴过后，
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
     * 同步直绘验收：隔离会话 + 直接 render()，与运行时零竞争。
     *
     * 前序共享会话方案被证伪（标记要么被行编辑器吞、要么落格无法区分
     * 解析与回显）：隔离会话自有 VT（shell 空闲无输出），feedTerminal
     * 字节只过 Ghostty 解析器；render(sessionId) 把该会话 CellData
     * 同步画到已挂载的真 surface（surface 是全局单例，与会话无关；
     * render_cell_data 不检查 render_paused，故暂停运行时绘制后仍可直绘，
     * 暂停仅用于消除运行时线程的帧竞争，finally 必恢复）。
     * render() 返回值即判决：1=已呈现，0=空闲，-1=原生渲染失败。
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
            // 暂停运行时绘制，独占 surface；直接 render() 不受暂停影响。
            NativeBridge.setRenderPaused(sessionId, true)
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
                // 软件渲染滞后网格：每次截图前同步重绘本会话，覆盖运行时残帧；
                // 绿/蓝计数用于鉴别“系统性丢色”与“单色特异”。
                var after = device.takeScreenshot() ?: throw AssertionError("截图失败")
                var redCount = countRedPixels(after)
                var greenCount = countPixels(after, ::isGreenish)
                var blueCount = countPixels(after, ::isBluish)
                val deadline = android.os.SystemClock.uptimeMillis() + 20_000
                while (redCount <= beforeRed + 20 && android.os.SystemClock.uptimeMillis() < deadline) {
                    val rendered = NativeBridge.render(sessionId, 0, 0)
                    android.util.Log.i("SgrDiag", "render rc=$rendered")
                    Thread.sleep(1_500)
                    after = device.takeScreenshot() ?: throw AssertionError("截图失败")
                    redCount = countRedPixels(after)
                    greenCount = countPixels(after, ::isGreenish)
                    blueCount = countPixels(after, ::isBluish)
                }
                android.util.Log.i(
                    "SgrDiag",
                    "red=$redCount green=$greenCount blue=$blueCount (beforeRed=$beforeRed)",
                )
                assertTrue(
                    "SGR 红色文本必须产生红色像素 (前=$beforeRed 后=$redCount 绿=$greenCount 蓝=$blueCount)",
                    redCount > beforeRed + 20,
                )
            } finally {
                runCatching { NativeBridge.setRenderPaused(sessionId, false) }
            }
        } finally {
            runCatching { NativeBridge.destroySession(sessionId) }
        }
    }
}
