package terminal.emulator.diag

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
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
            val text = bridge().getTerminalText()
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

    private fun cellCenterLuminance(
        row: Int,
        col: Int,
    ): Int {
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
        b.setCursorBlinkEnabled(false)
        Thread.sleep(250)

        fun assertCursorAtItsCell(stage: String): Pair<Int, Int> {
            val (row, col) = renderCursorRowCol()
            org.junit.Assert.assertTrue("$stage: render cursor hidden (-1)", row >= 0 && col >= 0)
            val lum = cellCenterLuminance(row, col)
            org.junit.Assert.assertTrue(
                "$stage: cursor cell ($row,$col) luminance $lum must be bright (>140)",
                lum > 140,
            )
            return row to col
        }

        var previous = assertCursorAtItsCell("T0-boot")
        bridge().writeToPty("abc".toByteArray())
        Thread.sleep(900)
        val t1 = assertCursorAtItsCell("T1-after-abc")
        previous = t1
        bridge().writeToPty("\n".toByteArray())
        Thread.sleep(900)
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
