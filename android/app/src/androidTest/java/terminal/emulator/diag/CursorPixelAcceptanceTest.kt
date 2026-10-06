package terminal.emulator.diag

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatRule
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.bridge.Bridge
import terminal.emulator.cleanUpTerminalState
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.pixelLuminance
import terminal.emulator.terminalCellSizePx
import terminal.emulator.terminalGridColumns
import terminal.emulator.waitForTerminalScreen

/**
 * 光标块验收：截图里的块光标必须落在渲染源报告的光标格上，
 * 光标移走后原格必须无残留块。
 *
 * 光标经 VT 直写定位（CUP）而非经 shell 回显：块光标在**空白格**上与「格内
 * 有文字」在亮度上不可区分。原先用「写入 abc 再回车」造位移，回车前一格正好
 * 压着 shell 回显的文字，测到 `lum=240` 便判「残留块」——判红与擦除逻辑无关，
 * 且随前序用例留下的行内容漂移（同一用例单跑恒绿、整类连跑偶红）。
 * 清屏后把光标移到**空行**再下移一行，原格必为纯背景，亮度不降即唯一地
 * 只能是残留块。
 */
class CursorPixelAcceptanceTest {
    @get:Rule val terminalLogcatRule = TerminalLogcatRule()

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForTerminalScreen()
        composeTestRule.awaitBridge()
    }

    /** 共用会话跨全部用例留存：不收尾即把选区、滚动偏移与回滚留给后继用例。 */
    @After
    fun resetSession() = composeTestRule.cleanUpTerminalState()

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private fun renderCursorRowCol(): Pair<Int, Int> {
        val packed = bridge().cursorViewportPacked()
        val row = if (packed >= 0) (packed shr 32).toInt() else -1
        val col = if (packed >= 0) (packed and 0xffffffffL).toInt() else -1
        return row to col
    }

    /**
     * 光标格与**同行空白格**的亮度差。
     *
     * 块光标是实心块，与所在主题的背景必然反差明显；反过来「绝对亮度够亮」
     * 只能对深色主题成立——浅色主题里块光标恰是深色的，判据随即失效
     * （实测读数 0）。取差值即与主题无关。
     */
    private fun cursorContrast(shot: android.graphics.Bitmap, row: Int, col: Int): Int {
        val activity = composeTestRule.activity
        val (cellWidth, cellHeight) = terminalCellSizePx(activity, bridge())
        val location = IntArray(2)
        findTerminalSurface(activity).getLocationOnScreen(location)
        val rowY = (location[1] + (row + 0.55) * cellHeight).toInt()
        val cursorX = (location[0] + (col + 0.5) * cellWidth).toInt()
        // 参照格取同一行最右侧：该行只有光标格有内容，其余皆背景。
        val backgroundX = (location[0] + (terminalGridColumns(activity, bridge()) - 1.5) * cellWidth).toInt()
        val cursor = pixelLuminance(shot, cursorX, rowY)
        val background = pixelLuminance(shot, backgroundX, rowY)
        if (cursor < 0 || background < 0) return 0
        return kotlin.math.abs(cursor - background)
    }

    /** 清屏后把光标放到 [row] 行首列，并轮询至渲染源确认落点。 */
    private fun moveCursorToEmptyRow(row: Int) {
        val bridge = bridge()
        assertTrue("清屏送显失败", bridge.feedTerminal("\u001B[2J\u001B[H".toByteArray(Charsets.UTF_8)))
        assertTrue("光标定位送显失败", bridge.feedTerminal("\u001B[${row + 1};1H".toByteArray(Charsets.UTF_8)))
        assertNotNull(
            "渲染源光标必须落在 ($row,0), 实际 ${renderCursorRowCol()}",
            UxTestUtils.pollUntilTrue(timeoutMs = 12_000, intervalMs = 200) {
                renderCursorRowCol() == row to 0
            },
        )
    }

    private fun awaitCursorContrast(row: Int, col: Int, stage: String) {
        assertNotNull(
            "$stage: 光标格 ($row,$col) 必须与背景反差, 实测反差=" +
                device.takeScreenshot()?.let { cursorContrast(it, row, col) },
            UxTestUtils.pollUntilTrue(timeoutMs = 12_000, intervalMs = 300) {
                val shot = device.takeScreenshot() ?: return@pollUntilTrue false
                cursorContrast(shot, row, col) > CURSOR_CONTRAST_MINIMUM
            },
        )
    }

    @Test
    fun cursorBlockMatchesRenderCursorCell() {
        val previousRow = 5
        moveCursorToEmptyRow(previousRow)
        awaitCursorContrast(previousRow, 0, "T0-定位")

        val currentRow = previousRow + 2
        moveCursorToEmptyRow(currentRow)
        awaitCursorContrast(currentRow, 0, "T1-移动")

        // 块光标是不闪烁的实心块，移走后原格必须与背景同色。
        assertNotNull(
            "T2: 旧光标格 ($previousRow,0) 必须无残留块",
            UxTestUtils.pollUntilTrue(timeoutMs = 12_000, intervalMs = 300) {
                val shot = device.takeScreenshot() ?: return@pollUntilTrue false
                cursorContrast(shot, previousRow, 0) <= CURSOR_CONTRAST_MINIMUM
            },
        )
    }

    companion object {
        /** 块光标与背景的最小亮度差（低于此即认为该格只剩背景）。 */
        private const val CURSOR_CONTRAST_MINIMUM = 60
    }
}
