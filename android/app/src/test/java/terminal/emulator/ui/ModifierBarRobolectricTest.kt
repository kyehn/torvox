package terminal.emulator.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import terminal.emulator.input.ModifierState
import terminal.emulator.input.next

/**
 * JVM (Robolectric) Compose tests for [ModifierBar] — proof that pure-UI semantics and Roborazzi
 * screenshots run WITHOUT an emulator. ModifierBar has no JNI dependency, so it is the reference
 * case for "backend/frontend separation in tests" (see docs/standards/TESTING.md §Instrumented
 * 方法论).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class ModifierBarRobolectricTest {

    @get:Rule val composeRule = createComposeRule()

    private fun setModifierBar() {
        composeRule.setContent {
            MaterialTheme {
                var ctrl by remember { mutableStateOf(ModifierState.Off) }
                var alt by remember { mutableStateOf(ModifierState.Off) }
                ModifierBar(
                    onKeyClick = {},
                    ctrlState = ctrl,
                    altState = alt,
                    onToggleCtrl = { ctrl = ctrl.next() },
                    onToggleAlt = { alt = alt.next() },
                )
            }
        }
    }

    @Test
    fun `ctrl key visible and toggles selection state`() {
        setModifierBar()
        composeRule.onNodeWithTag("Key_CTRL").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_CTRL").assertIsNotSelected()
        composeRule.onNodeWithTag("Key_CTRL").performClick()
        composeRule.onNodeWithTag("Key_CTRL").assertIsSelected()
    }

    @Test
    fun `arrow keys and common keys are rendered`() {
        setModifierBar()
        composeRule.onNodeWithTag("Key_\u2191").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_HOME").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
        composeRule.onNodeWithTag("Key_TAB").assertIsDisplayed()
    }

    @Test
    fun `plain extra key fires on touch down without waiting for up`() {
        // spec modifier-bar-interaction "按下即发": DOWN alone must
        // deliver the key (termux semantics), not wait for a dwell window
        // or the lift.
        var clicks = 0
        composeRule.setContent {
            MaterialTheme {
                ModifierBar(onKeyClick = { clicks++ })
            }
        }
        composeRule.onNodeWithTag("Key_ESC").performTouchInput {
            down(center)
        }
        composeRule.waitForIdle()
        org.junit.Assert.assertEquals(
            "key must fire on ACTION_DOWN before any UP arrives",
            1,
            clicks,
        )
        // Release so the injected stream ends cleanly.
        composeRule.onNodeWithTag("Key_ESC").performTouchInput { up() }
    }

    @Test
    fun `modifier bar golden screenshot`() {
        setModifierBar()
        composeRule.onRoot().captureRoboImage()
    }
}
