
package terminal.emulator.cucumber

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import io.cucumber.junit.WithJunitRule
import org.junit.Rule
import terminal.emulator.MainActivity

@WithJunitRule
class ComposeRuleHolder {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()
}
