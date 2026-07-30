
package terminal.emulator.cucumber

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import io.cucumber.junit.WithJunitRule
import terminal.emulator.MainActivity
import org.junit.Rule

@WithJunitRule
class ComposeRuleHolder {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()
}
