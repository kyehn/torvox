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
 */
class SelectionTapDismissTest {
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
            if (!bridge().getTerminalText().isNullOrBlank()) break
            Thread.sleep(200)
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
        // Long-press on prompt text (row 0): creates a word selection.
        device.swipe(300, 146, 300, 146, 54) // ~900ms hold
        Thread.sleep(800)
        val afterLongPress = selectionStateForTest()
        org.junit.Assert.assertTrue(
            "long-press must create an active selection (got $afterLongPress)",
            afterLongPress.active,
        )

        // Tap an empty area well away from the selection.
        device.click(540, 1200)

        // Wait past the 500ms long-press timeout: a phantom onLongPress would
        // spawn a NEW paste-only selection at the tap cell right about now.
        Thread.sleep(900)

        val afterTap = selectionStateForTest()
        org.junit.Assert.assertFalse(
            "tap must dismiss the selection; phantom selection spawned: $afterTap",
            afterTap.active,
        )
    }
}
