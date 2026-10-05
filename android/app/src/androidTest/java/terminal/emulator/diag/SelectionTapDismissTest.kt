package terminal.emulator.diag

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatRule
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.firstColumnBeyondDrawerEdge
import terminal.emulator.getBridge
import terminal.emulator.terminalCellCenterOnScreen
import terminal.emulator.waitForTerminalScreen

/**
 * regression: a tap that dismisses an active selection must complete as a tap.
 *
 * Root cause (fixed): the surface's ACTION_DOWN handler dismissed the handle-overlay PopupWindow
 * while that window was the in-flight dispatch target of the very touch stream being forwarded
 * through it. The window vanished before UP arrived, the surface's GestureDetector kept a pending
 * DOWN, and 500ms later a phantom onLongPress fired at the tap position — spawning a fresh
 * paste-only selection with two stacked handles ("multiple pointers that never disappear").
 *
 * Deterministic check: long-press text → tap an empty area → wait past the long-press timeout
 * (600ms) → the selection must be inactive and NO new selection may have spawned at the tap cell.
 *
 * 手势坐标一律由单元格度量算出（[terminalCellCenterOnScreen]）：屏幕尺寸随设备而变，
 * 硬编码坐标在 CI 的 320×640 模拟器上越界，手势根本没进终端，断言却报「幽灵选择」。
 */
class SelectionTapDismissTest {
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
        UxTestUtils.pollUntilTrue(timeoutMs = 20_000, intervalMs = 200) {
            bridge().getTerminalText()?.isNotBlank() == true
        }
        Thread.sleep(1_000)
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private data class SelectionSnapshot(
        val active: Boolean,
        val startRow: Int,
        val startCol: Int,
        val endRow: Int,
        val endCol: Int,
    )

    /** Selection state read off the ViewModel's Compose state via the activity. */
    private fun selectionStateForTest(): SelectionSnapshot {
        var result = SelectionSnapshot(false, -1, -1, -1, -1)
        val rule = composeTestRule.activityRule as androidx.test.ext.junit.rules.ActivityScenarioRule<*>
        rule.scenario.onActivity { activity: android.app.Activity ->
            val sel = (activity as MainActivity).terminalViewModel.state.value.selection
            result =
                SelectionSnapshot(
                    sel.active,
                    sel.start?.row ?: -1,
                    sel.start?.col ?: -1,
                    sel.end?.row ?: -1,
                    sel.end?.col ?: -1,
                )
        }
        return result
    }

    @Test
    fun tapDismissesSelectionWithoutPhantomLongPress() {
        val activity = composeTestRule.activity
        // 标记直写固定视口行：长按点必须落在确定的文字上，既不依赖 shell 提示符落在
        // 哪一行，也不依赖屏幕尺寸（硬编码坐标在 CI 的 320×640 上直接越界）。
        // 列起点取「抽屉边缘区之后的第一列」：区内触摸被 Surface 直接丢弃
        // （`TerminalSurface.onTouchEvent` 对 `event.x < 32dp` 返回 false），
        // 长按会根本到不了手势检测器，而断言只会报「没建出选区」。
        val markerColumn = firstColumnBeyondDrawerEdge(activity, bridge())
        assertTrue("标记送显失败", bridge().feedTerminal(feedBytes(markerColumn)))
        val (pressX, pressY) =
            terminalCellCenterOnScreen(activity, bridge(), markerColumn + 2, MARKER_ROW)
        // UiDevice.swipe 把 DOWN/UP 发进同一主线程批处理，常被当点按吃掉；
        // 经 input flinger 按真实时长下发，保证长按定时器能触发。
        device.executeShellCommand(
            "input touchscreen swipe $pressX $pressY $pressX $pressY $LONG_PRESS_MILLIS",
        )
        Thread.sleep(800)
        val afterLongPress = selectionStateForTest()
        assertTrue(
            "long-press must create an active selection (got $afterLongPress, press=($pressX,$pressY))",
            afterLongPress.active,
        )

        // 点按离选区足够远的空白单元格。
        val (tapX, tapY) =
            terminalCellCenterOnScreen(activity, bridge(), markerColumn + 2, BLANK_ROW)
        device.click(tapX, tapY)

        // Wait past the 500ms long-press timeout: a phantom onLongPress would
        // spawn a NEW paste-only selection at the tap cell right about now.
        Thread.sleep(900)

        val afterTap = selectionStateForTest()
        assertFalse(
            "tap must dismiss the selection; phantom selection spawned: $afterTap",
            afterTap.active,
        )
    }

    companion object {
        /** 标记所在视口行（0 基）。 */
        private const val MARKER_ROW = 2

        /** 点按的空白视口行（0 基），与标记行相隔足够远。 */
        private const val BLANK_ROW = 6

        private const val LONG_PRESS_MILLIS = 1000

        /** 清屏后在标记行、从 [column] 列起直写词：长按必落文字，选区是词选择而非粘贴专用选择。 */
        private fun feedBytes(column: Int): ByteArray = "\u001B[2J\u001B[${MARKER_ROW + 1};${column + 1}Hphantomtarget"
            .toByteArray(Charsets.UTF_8)
    }
}
