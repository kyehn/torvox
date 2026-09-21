package terminal.emulator.diag

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.getBridge
import terminal.emulator.grantNotificationPermission

/**
 * acceptance: the cursor block visible in the screenshot must sit at the render-source cursor cell,
 * and moving the cursor must not leave stale blocks at previous positions.
 *
 * Root causes fixed:
 * - empty-cell Block origin pushed one row down (pass.rs Y math)
 * - band redraws over LoadOp::Load never erased old blocks (clear_instances)
 *
 * Evidence: sample center pixel of cell rect for render-source cursor — must be bright (block on);
 * previous cursor cell must be dark (no stale block).
 */
class CursorPixelAcceptanceTest {
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
        val deadline = System.currentTimeMillis() + 20_000
        while (System.currentTimeMillis() < deadline) {
            // 桥在会话孵化完成前为 null：容忍空桥继续轮询，而非首轮即抛。
            val ready = runCatching { bridge() }.getOrNull()
            val text = ready?.getTerminalText()
            if (!text.isNullOrBlank()) break
            Thread.sleep(200)
        }
        Thread.sleep(1_500)
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private fun renderCursorRowCol(): Pair<Int, Int> {
        val packed = bridge().cursorViewportPacked()
        val row = if (packed >= 0) (packed shr 32).toInt() else -1
        val col = if (packed >= 0) (packed and 0xffffffffL).toInt() else -1
        return row to col
    }

    private fun cellCenterLuminance(row: Int, col: Int): Int {
        val shot = device.takeScreenshot() ?: return -1
        val b = bridge()
        val density =
            InstrumentationRegistry.getInstrumentation().targetContext.resources.displayMetrics.density
        val cw = b.getCellWidth() * density
        val ch = b.getCellHeight() * density
        val surfaceTop = 122
        val cx = ((col + 0.5) * cw).toInt()
        val cy = (surfaceTop + ((row + 0.55) * ch)).toInt()
        if (cx < 0 || cx >= shot.width || cy < 0 || cy >= shot.height) return -1
        val p = shot.getPixel(cx, cy)
        return ((p shr 16 and 0xFF) * 299 + (p shr 8 and 0xFF) * 587 + (p and 0xFF) * 114) / 1000
    }

    @Test
    fun cursorBlockMatchesRenderCursorCell() {
        val b = bridge()
        Thread.sleep(250)

        fun cursorBrightAtItsCell(): Pair<Int, Int>? {
            val (row, col) = renderCursorRowCol()
            if (row < 0 || col < 0) return null
            if (cellCenterLuminance(row, col) <= 140) return null
            return row to col
        }

        fun assertCursorAtItsCell(stage: String): Pair<Int, Int> {
            // 首帧 CellData 推送滞后约一包：T0 即判隐藏是时序误报，轮询至光标格变亮。
            val ready =
                UxTestUtils.pollUntilTrue(timeoutMs = 12_000, intervalMs = 500) {
                    cursorBrightAtItsCell() != null
                }
            assertNotNull("$stage: 光标格必须变亮", ready)
            return cursorBrightAtItsCell()
                ?: throw AssertionError("$stage: render cursor hidden (-1)")
        }

        var previous = assertCursorAtItsCell("T0-boot")
        bridge().writeToPty("abc".toByteArray())
        // 冷机首帧慢：固定睡眠不可靠，轮询至光标格变亮（超时仍按原断言失败）。
        val t1ready =
            UxTestUtils.pollUntilTrue(timeoutMs = 12_000, intervalMs = 500) {
                val (row, col) = renderCursorRowCol()
                row >= 0 && col >= 0 && cellCenterLuminance(row, col) > 140
            }
        assertNotNull("T1-after-abc: 光标格必须变亮", t1ready)
        val t1 = assertCursorAtItsCell("T1-after-abc")
        previous = t1
        bridge().writeToPty("\n".toByteArray())
        val t3ready =
            UxTestUtils.pollUntilTrue(timeoutMs = 12_000, intervalMs = 500) {
                val (row, col) = renderCursorRowCol()
                row >= 0 && col >= 0 && cellCenterLuminance(row, col) > 140
            }
        assertNotNull("T3-after-enter: 光标格必须变亮", t3ready)
        val t3 = assertCursorAtItsCell("T3-after-enter")
        if (t3.first != previous.first) {
            val staleLum = cellCenterLuminance(previous.first, previous.second)
            org.junit.Assert.assertTrue(
                "T3: stale block at (${previous.first},${previous.second}) lum=$staleLum must be <140",
                staleLum < 140,
            )
        }
    }
}
