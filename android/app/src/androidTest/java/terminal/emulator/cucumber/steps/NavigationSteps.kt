package terminal.emulator.cucumber.steps

import android.view.KeyEvent
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.openDrawer
import terminal.emulator.waitForSettingsScreen
import javax.inject.Inject

class NavigationSteps
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @When("^the back button is pressed$")
    fun backButtonIsPressed() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^the user opens the session drawer$")
    fun userOpensSessionDrawer() {
        composeRuleHolder.composeRule.openDrawer()
    }

    @When("^the user closes the drawer$")
    fun userClosesDrawer() {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
            activity.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
        }
        composeRuleHolder.composeRule.waitForIdle()
    }

    @When("^navigates to settings from the drawer$")
    fun navigatesToSettingsFromDrawer() {
        val rule = composeRuleHolder.composeRule
        // Prefer openDrawer then click, mirroring TestUtils.openSettings robustness for CI slowness.
        // Fast-path: SettingsButton is composed even when drawer is closed (see SearchSteps).
        // Compose assertion failures are AssertionError (an Error, not an
        // Exception) — use runCatching so the UiAutomator fallback runs.
        val directClickSucceeded =
            runCatching {
                rule.onNodeWithTag("SettingsButton", useUnmergedTree = true).performClick()
                rule.waitForIdle()
                // Quick probe: if SettingsScreen appears within 8s, we are done even without drawer.
                var probeDeadline = System.currentTimeMillis() + 8000
                while (System.currentTimeMillis() < probeDeadline) {
                    val visible =
                        runCatching {
                            rule
                                .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
                                .assertIsDisplayed()
                            true
                        }
                            .getOrDefault(false)
                    if (visible) return@runCatching true
                    Thread.sleep(200)
                }
                false
            }
                .getOrDefault(false)
        if (directClickSucceeded) {
            rule.waitForSettingsScreen(timeoutMs = 60_000)
            return
        }
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        // Try UiAutomator direct click before opening drawer (avoids animation).
        val uiDirect =
            runCatching {
                if (device.wait(Until.hasObject(By.text("Settings")), 5000)) {
                    device.findObject(By.text("Settings"))?.click()
                    rule.waitForIdle()
                    Thread.sleep(500)
                    runCatching {
                        rule
                            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
                            .assertIsDisplayed()
                    }
                        .isSuccess || device.hasObject(By.text("Font Family"))
                } else {
                    false
                }
            }
                .getOrDefault(false)
        if (uiDirect) {
            rule.waitForSettingsScreen(timeoutMs = 60_000)
            return
        }
        rule.openDrawer()
        if (device.wait(Until.hasObject(By.text("Settings")), 10_000)) {
            device.findObject(By.text("Settings"))?.click()
        } else if (device.wait(Until.hasObject(By.desc("Settings")), 10_000)) {
            device.findObject(By.desc("Settings"))?.click()
        }
        rule.waitForSettingsScreen(timeoutMs = 60_000)
    }

    @Then("^the drawer is displayed$")
    fun drawerIsDisplayed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SessionDrawer", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the terminal screen is fully visible$")
    fun terminalScreenIsFullyVisible() {
        composeRuleHolder.composeRule.waitUntil(timeoutMillis = 5000) {
            composeRuleHolder.composeRule
                .onAllNodes(hasTestTag("TerminalScreen"), useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Then("^the settings screen is displayed$")
    fun settingsScreenIsDisplayed() {
        composeRuleHolder.composeRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
