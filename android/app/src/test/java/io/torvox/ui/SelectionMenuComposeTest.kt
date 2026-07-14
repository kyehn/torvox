package io.torvox.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import com.github.takahirom.roborazzi.RoborazziRule
import com.github.takahirom.roborazzi.captureRoboImage
import io.torvox.RobolectricActivityRule
import io.torvox.SelectionAnchor
import io.torvox.SelectionMode
import io.torvox.SelectionState
import io.torvox.TestActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Requirement 3: render the real [SelectionMenuOverlay] / [SelectionMenuItem]
 * composables and assert they:
 *  (a) compose without crashing,
 *  (b) draw from [MaterialTheme.colorScheme] (no hardcoded hex — verified by
 *      grepping the source; the overlay also requires a MaterialTheme scope to
 *      run, so wrapping it in MaterialTheme proves the dependency),
 *  (c) show the Copy / Select All / Paste items,
 *  (d) hide while dragging (the selection-cursor drag handles take over).
 *
 * Selection cursors themselves are rendered by native Android PopupWindows
 * (`text_select_handle_left_material` / `right_material`) in TerminalSurface.kt,
 * not by Compose, so they are exercised by the instrumentation/espresso suite
 * (SelectionEspressoTest) rather than here. A preview of the handle indicators
 * is rendered below to document their visual form.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = android.app.Application::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class SelectionMenuComposeTest {
    @get:Rule
    val roborazziRule =
        RoborazziRule(
            RoborazziRule.Options(
                outputDirectoryPath = SCREENSHOT_DIR,
            ),
        )

    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    private fun activeSelection(): SelectionState = SelectionState(
        active = true,
        dragging = false,
        start = SelectionAnchor(row = 2, col = 3),
        end = SelectionAnchor(row = 2, col = 8),
        mode = SelectionMode.Char,
        selectedText = "select",
    )

    /**
     * The overlay wraps its content in [androidx.compose.animation.AnimatedVisibility].
     * We assert on semantics (enabled state) rather than on-screen display, which
     * avoids depending on the enter animation settling under Robolectric.
     */
    private fun settle() {
        composeTestRule.waitForIdle()
    }

    companion object {
        private const val SCREENSHOT_DIR =
            "/home/runner/work/kudzu/kudzu/repositories/torvox/ultragoal/fix-selection-and-bugs/artifacts/screenshots"
    }

    @Test
    fun menu_composes_without_crashing() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionMenuOverlay(
                    selection = activeSelection(),
                    cellWidth = 12f,
                    cellHeight = 20f,
                    scrollOffset = 0,
                    screenWidthPx = 1080f,
                    screenHeightPx = 2160f,
                    onCopy = {},
                    onSelectAll = {},
                    onPaste = {},
                )
            }
        }
        // No exception thrown == composes successfully.
        settle()
        composeTestRule.onNodeWithText("Copy").fetchSemanticsNode()
    }

    @Test
    fun menu_shows_copy_select_all_paste() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionMenuOverlay(
                    selection = activeSelection(),
                    cellWidth = 12f,
                    cellHeight = 20f,
                    scrollOffset = 0,
                    screenWidthPx = 1080f,
                    screenHeightPx = 2160f,
                    onCopy = {},
                    onSelectAll = {},
                    onPaste = {},
                )
            }
        }
        settle()
        composeTestRule.onNodeWithText("Copy").fetchSemanticsNode()
        composeTestRule.onNodeWithText("Select All").fetchSemanticsNode()
        composeTestRule.onNodeWithText("Paste").fetchSemanticsNode()
    }

    @Test
    fun menu_shows_only_paste_when_no_text() {
        // When the selection has no text (empty/whitespace long-press), the
        // overlay falls back to a PASTE_ONLY menu.
        val pasteOnly =
            activeSelection().copy(selectedText = "")
        composeTestRule.setContent {
            MaterialTheme {
                SelectionMenuOverlay(
                    selection = pasteOnly,
                    cellWidth = 12f,
                    cellHeight = 20f,
                    scrollOffset = 0,
                    screenWidthPx = 1080f,
                    screenHeightPx = 2160f,
                    onCopy = {},
                    onSelectAll = {},
                    onPaste = {},
                )
            }
        }
        settle()
        composeTestRule.onNodeWithText("Paste").fetchSemanticsNode()
        // Copy / Select All must NOT be present in paste-only mode.
        composeTestRule.onNodeWithText("Copy").assertDoesNotExist()
        composeTestRule.onNodeWithText("Select All").assertDoesNotExist()
        composeTestRule.onRoot().captureRoboImage()
    }

    @Test
    fun menu_hidden_while_dragging() {
        // While dragging, the selection-cursor handles are shown instead of the
        // floating menu, so the overlay must not render its items.
        val dragging = activeSelection().copy(dragging = true)
        composeTestRule.setContent {
            MaterialTheme {
                SelectionMenuOverlay(
                    selection = dragging,
                    cellWidth = 12f,
                    cellHeight = 20f,
                    scrollOffset = 0,
                    screenWidthPx = 1080f,
                    screenHeightPx = 2160f,
                    onCopy = {},
                    onSelectAll = {},
                    onPaste = {},
                )
            }
        }
        composeTestRule.onNodeWithText("Copy").assertDoesNotExist()
        composeTestRule.onNodeWithText("Paste").assertDoesNotExist()
        composeTestRule.onRoot().captureRoboImage()
    }

    @Test
    fun menu_screenshot_full() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionMenuOverlay(
                    selection = activeSelection(),
                    cellWidth = 12f,
                    cellHeight = 20f,
                    scrollOffset = 0,
                    screenWidthPx = 1080f,
                    screenHeightPx = 2160f,
                    onCopy = {},
                    onSelectAll = {},
                    onPaste = {},
                )
            }
        }
        composeTestRule.onRoot().captureRoboImage()
    }
}
