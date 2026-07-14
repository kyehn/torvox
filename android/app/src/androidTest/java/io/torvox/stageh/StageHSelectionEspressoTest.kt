package io.torvox.stageh

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.lifecycle.Lifecycle
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.withClassName
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.torvox.MainActivity
import org.hamcrest.CoreMatchers.`is`
import org.hamcrest.Matchers.endsWith
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Stage H — Espresso-driven verification of the default (session 1) terminal surface,
 * crash-freedom, and the session drawer / Settings entry point.
 *
 * Runs on the single default session only (no New Session / session switching — that path
 * triggers a known wgpu GPU-surface hang that crashes the app and is fixed separately).
 */
@RunWith(AndroidJUnit4::class)
class StageHSelectionEspressoTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun terminalSurfacePresent_andNoCrash() {
        composeTestRule.waitForIdle()
        // Terminal content composable is present.
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        // The real GPU terminal surface (TerminalSurface : TextureView) is present and visible.
        onView(withClassName(endsWith("TerminalSurface")))
            .check(matches(isDisplayed()))
        // The activity is alive and resumed (no crash).
        assertTrue(
            "MainActivity must be at least RESUMED",
            composeTestRule.activity.lifecycle.currentState
                .isAtLeast(Lifecycle.State.RESUMED),
        )
    }

    @Test
    fun sessionDrawerOpens_andSettingsButtonPresent() {
        composeTestRule.waitForIdle()
        // Open the session drawer via the hamburger button (contentDescription
        // "Open session drawer").
        composeTestRule
            .onNodeWithContentDescription("Open session drawer")
            .performClick()
        composeTestRule.waitForIdle()
        // The Settings button is present inside the drawer.
        composeTestRule
            .onNodeWithContentDescription("Settings")
            .assertIsDisplayed()
    }
}
