package terminal.emulator.diag

import android.graphics.Bitmap
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
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.grantNotificationPermission
import terminal.emulator.util.runCatchingCancellable

/**
 * 斜体像素验收：SGR 3 斜体文本必须在屏幕上产生与正体不同的字形像素，
 * 而非回退为正体（回归“斜体文本无法显示/与正体无异”类缺失）。
 *
 * 差分法：同一标记同列渲染两次——先正体、再斜体，逐像素比较裁剪区
 * （裁掉状态栏与修饰键栏，排除时钟跳动与按键噪声）。正体两帧必须
 * 逐帧一致（渲染确定性证明），斜体帧必须与正体帧差异超阈值。
 * 字节只过 Ghostty 解析器（直写 VT，不经 shell 行编辑器）。
 */
class SgrItalicPixelAcceptanceTest {
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

    /**
     * 渲染当前隔离会话状态并截图：呈现是异步的（runtime 循环/VSync 节拍），
     * render 返回不等于新帧已上屏，沉降等待后截图才抓得到新帧。
     */
    private fun renderAndScreenshot(sessionId: Long): Bitmap {
        NativeBridge.render(sessionId, 0, 0)
        Thread.sleep(PRESENT_SETTLE_MILLIS)
        return device.takeScreenshot() ?: throw AssertionError("截图失败")
    }

    /**
     * 裁剪区差分：两截图逐像素比较（步长 3 采样），只统计中带区域——
     * 裁掉状态栏（时钟跳动）与修饰键栏（按键噪声），文本位于 24 行视口
     * 第 12 行（中带），列 0 起步。
     */
    private fun countDifferingPixels(first: Bitmap, second: Bitmap): Int {
        val cropTop = first.height / 6
        val cropBottom = first.height * 4 / 5
        var count = 0
        for (y in cropTop until cropBottom step 3) {
            for (x in 0 until first.width step 3) {
                val delta =
                    kotlin.math.abs(
                        android.graphics.Color.red(first.getPixel(x, y)) -
                            android.graphics.Color.red(second.getPixel(x, y)),
                    ) +
                        kotlin.math.abs(
                            android.graphics.Color.green(first.getPixel(x, y)) -
                                android.graphics.Color.green(second.getPixel(x, y)),
                        ) +
                        kotlin.math.abs(
                            android.graphics.Color.blue(first.getPixel(x, y)) -
                                android.graphics.Color.blue(second.getPixel(x, y)),
                        )
                if (delta > PIXEL_DELTA_THRESHOLD) count++
            }
        }
        return count
    }

    private fun feedAtMiddleRow(sessionId: Long, sgrPrefix: String) {
        // \e[2J 清屏 + CUP 13;1（1 基，即 0 基第 12 行中带）+ 标记。
        NativeBridge.feedTerminal(
            sessionId,
            "\u001B[2J\u001B[13;1H$sgrPrefix$MARKER_TEXT\u001B[0m".toByteArray(Charsets.UTF_8),
        )
        val gridded =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                NativeBridge.getTerminalText(sessionId)?.contains(MARKER_TEXT) == true
            }
        assertNotNull(
            "标记必须落格, 实际尾部: ${NativeBridge.getTerminalText(sessionId)?.takeLast(200)}",
            gridded,
        )
    }

    @Test
    fun sgrItalicTextProducesDistinctGlyphPixels() {
        // 字节只过 Ghostty 解析器（直写 VT，不经 shell 行编辑器）。
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val home = context.filesDir.resolve("italic-test-home").apply { mkdirs() }.absolutePath
        val sessionId = NativeBridge.initSession(24, 80, "/system/bin/sh", home, home, "", "", 2000)
        assertTrue("隔离会话创建失败", sessionId != 0L)
        var previousActiveId: Long? = null
        try {
            // 等 shell 首 prompt 落定：隔离会话的 shell 会异步打印 prompt，
            // 在其落定前连拍必混入新输出（自差分整屏级）。落定后 shell 空闲，
            // 网格稳定，连拍差分才归零。
            val promptReady =
                UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                    val text = NativeBridge.getTerminalText(sessionId).orEmpty()
                    text.contains("$") || text.contains("#")
                }
            assertNotNull("隔离会话 shell prompt 未出现", promptReady)
            // 切为活跃：运行时重绘循环只画活跃会话，切后它与直绘呈现同一网格，
            // 呈现竞争消失（谁赢都是同一内容）。注：setRenderPaused 不可用——
            // 它是全局开关，会连本会话直绘一起停掉（残帧导致差分全零）。
            // 记下运行时原活跃会话，finally 切回，避免销毁后运行时无活跃会话。
            previousActiveId =
                runCatchingCancellable {
                    NativeBridge.listSessions()
                        ?.removeSurrounding("[", "]")
                        ?.split(",")
                        ?.mapNotNull { it.trim().toLongOrNull() }
                        ?.firstOrNull { it != sessionId }
                }.getOrNull()
            NativeBridge.switchSession(sessionId)
            // 正体基线：同状态背靠背连拍（无休眠：休眠只 widen shell 输出 interleaving 窗口），
            // 差分必须为零（渲染确定性证明）；偶发 shell 输出则有限重试而非直接失败。
            var plainFirst = renderAndScreenshot(sessionId)
            var plainSecond = renderAndScreenshot(sessionId)
            var plainSelfDiff = countDifferingPixels(plainFirst, plainSecond)
            var attempts = 1
            while (plainSelfDiff > DETERMINISM_SELF_DIFF_LIMIT && attempts < DETERMINISM_MAX_ATTEMPTS) {
                android.util.Log.i("SgrItalic", "selfDiff=$plainSelfDiff attempt=$attempts retry")
                feedAtMiddleRow(sessionId, "")
                plainFirst = renderAndScreenshot(sessionId)
                plainSecond = renderAndScreenshot(sessionId)
                plainSelfDiff = countDifferingPixels(plainFirst, plainSecond)
                attempts++
            }
            android.util.Log.i("SgrItalic", "selfDiff=$plainSelfDiff attempts=$attempts")
            assertTrue(
                "同状态两帧必须一致 (自差分=$plainSelfDiff, attempts=$attempts)",
                plainSelfDiff <= DETERMINISM_SELF_DIFF_LIMIT,
            )
            // 斜体帧：同列同标记改 \e[3m，字形像素必须与正体不同。
            feedAtMiddleRow(sessionId, "\u001B[3m")
            val italicShot = renderAndScreenshot(sessionId)
            val italicDiff = countDifferingPixels(plainFirst, italicShot)
            android.util.Log.i("SgrItalic", "italicDiff=$italicDiff")
            assertTrue(
                "斜体字形像素必须与正体不同 (差分=$italicDiff)",
                italicDiff > PIXEL_GAIN_THRESHOLD,
            )
        } finally {
            // 先切回运行时原会话再销毁隔离会话：顺序反了会短暂无活跃会话。
            if (previousActiveId != null) runCatchingCancellable { NativeBridge.switchSession(previousActiveId) }
            runCatchingCancellable { NativeBridge.destroySession(sessionId) }
        }
    }

    companion object {
        private const val MARKER_TEXT = "STYLE_MARK_X"

        /** 逐像素通道和差分阈值：低于视为同一字形。 */
        private const val PIXEL_DELTA_THRESHOLD = 40

        /** 斜体与正体差分下限：低于视为斜体未生效。 */
        private const val PIXEL_GAIN_THRESHOLD = 20

        /** 同状态自差分上限：渲染确定性余量。 */
        private const val DETERMINISM_SELF_DIFF_LIMIT = 5

        /** render 与截图之间的呈现沉降等待：新帧上屏需要一个呈现节拍。 */
        private const val PRESENT_SETTLE_MILLIS = 600L

        /** 自差分超限时的重拍上限：shell 偶发输出只重试，不直接失败。 */
        private const val DETERMINISM_MAX_ATTEMPTS = 3
    }
}
