package terminal.emulator

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.matcher.ViewMatchers.withId
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.atiurin.ultron.extensions.isDisplayed
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity

/**
 * Ultron-style on-device UI smoke test.
 *
 * Ultron (open-tool/ultron) wraps Espresso/Compose/UI Automator with a
 * stable DSL. torvox launches `MainActivity` via `ActivityScenarioRule`
 * so the activity runs in the app process (`com.termux`) and its Compose
 * semantics are visible to the test (the `runUltronUiTest { }` compose
 * environment starts a separate test-process activity and cannot see the
 * app's cross-process UI). We therefore verify the Compose nodes with the
 * standard `createAndroidComposeRule` (same pattern as `ModifierBarTest`)
 * and exercise Ultron's Espresso `isDisplayed()` extension on the root
 * content view to prove the Ultron integration compiles and runs on-device.
 */
@RunWith(AndroidJUnit4::class)
class UltronSmokeTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminalScreenIsDisplayed() {
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    @Test
    fun modifierBarKeysAreDisplayed() {
        composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_DRAWER").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Key_SCROLL").assertIsDisplayed()
    }

    @Test
    fun rootContentViewIsDisplayedViaUltron() {
        // Ultron Espresso extension: built-in retry + timeout.
        onView(withId(android.R.id.content)).isDisplayed()
    }
}
