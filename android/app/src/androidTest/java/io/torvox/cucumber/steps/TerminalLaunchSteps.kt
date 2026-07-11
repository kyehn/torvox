package io.torvox.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.torvox.cucumber.ComposeRuleHolder
import io.torvox.findTerminalSurface
import io.torvox.waitForSession
import javax.inject.Inject

class TerminalLaunchSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Then("^the modifier bar is visible$")
    fun modifierBarIsVisible() {
        composeRuleHolder.composeRule
            .onNodeWithTag("ModifierBar")
            .assertIsDisplayed()
    }

    @Then("^the terminal content area has positive dimensions$")
    fun terminalContentAreaHasPositiveDimensions() {
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalContent")
            .assertIsDisplayed()
            .assertWidthIsAtLeast(1.dp)
    }

    @Then("^the SurfaceView is visible$")
    fun surfaceViewIsVisible() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            assert(surface != null) { "SurfaceView should exist" }
            assert(surface.width > 0) { "SurfaceView width should be positive" }
            assert(surface.height > 0) { "SurfaceView height should be positive" }
        }
    }

    @Then("^it renders above the Compose layout$")
    fun itRendersAboveComposeLayout() {
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
