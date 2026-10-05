package terminal.emulator.diag

import android.graphics.Bitmap
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatRule
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.bridge.Bridge
import terminal.emulator.placeTextAtRow
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalScreen

/**
 * 斜体像素验收：SGR 3 斜体文本必须产生与正体不同的字形像素，
 * 而非回退为正体（回归“斜体文本无法显示/与正体无异”类缺失）。
 *
 * 差分法：同一标记写两次——先正体、再斜体，逐像素比较裁剪区（裁掉状态栏与
 * 修饰键栏，排除时钟跳动与按键噪声）。标记写进**运行时正在呈现的会话**，
 * 呈现与截图同源。隔离会话 + 原生直绘不可行：运行时会话任意一次输出即覆盖
 * 隔离帧，且此后隔离会话在渲染空闲分支恒被判为“会话不一致”而永不重绘，
 * 标记永久丢失（实测差分恒为 0）。落格判据是渲染光标落在标记末列
 * （[placeTextAtRow]），无需沉降等待与自差分重拍。
 */
class SgrItalicPixelAcceptanceTest {
    @get:Rule val terminalLogcatRule = TerminalLogcatRule()

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalScreen()
    }

    /** 呈现是异步的（运行时循环/VSync 节拍）：轮询截图直到与 [plain] 出现差异。 */
    private fun awaitChangedFrom(plain: Bitmap): Bitmap? {
        var latest = plain
        val changed =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 250) {
                val shot = device.takeScreenshot() ?: return@pollUntilTrue false
                latest = shot
                countDifferingPixels(plain, shot) > PIXEL_GAIN_THRESHOLD
            }
        return if (changed != null) latest else null
    }

    /**
     * 裁剪区差分：两截图逐像素比较（步长 3 采样），只统计中带区域——
     * 裁掉状态栏（时钟跳动）与修饰键栏（按键噪声）。口径收归 [UxTestUtils.countDiffInBand]。
     */
    private fun countDifferingPixels(first: Bitmap, second: Bitmap): Int =
        UxTestUtils.countDiffInBand(
            first,
            second,
            first.height / 6,
            first.height * 4 / 5,
            SAMPLE_STEP_PX,
            PIXEL_DELTA_THRESHOLD,
        )

    @Test
    fun sgrItalicTextProducesDistinctGlyphPixels() {
        val bridge: Bridge = composeTestRule.awaitBridge()
        // 正体基线：写入并泵送呈现后截图。落格（光标到位）不等于已上屏，
        // 直写不置脏，必须泵一次 shell 输出触发推送，否则截到旧帧。
        placeTextAtRow(bridge, MARKER_ROW, MARKER_TEXT)
        val plainShot = device.takeScreenshot() ?: throw AssertionError("截图失败")
        // 斜体帧：同列同标记加 \e[3m，同样泵送后等变化出现。
        placeTextAtRow(bridge, MARKER_ROW, "\u001B[3m$MARKER_TEXT")
        val italicShot = awaitChangedFrom(plainShot)
        val italicDiff = if (italicShot == null) 0 else countDifferingPixels(plainShot, italicShot)
        UxTestUtils.metric("sgr_italic_glyph_diff", italicDiff)
        assertTrue(
            "斜体字形像素必须与正体不同 (差分=$italicDiff)",
            italicDiff > PIXEL_GAIN_THRESHOLD,
        )
    }

    companion object {
        private const val MARKER_TEXT = "STYLE_MARK_X"

        /** 标记所在视口行：取中带，避开顶部状态栏与底部修饰键栏/提示符。 */
        private const val MARKER_ROW = 12

        /** 逐像素通道和差分阈值：低于视为同一字形。 */
        private const val PIXEL_DELTA_THRESHOLD = 40

        /** 差分抽样步长：只影响统计速度。 */
        private const val SAMPLE_STEP_PX = 3

        /** 斜体与正体差分下限：低于视为斜体未生效。 */
        private const val PIXEL_GAIN_THRESHOLD = 20
    }
}
