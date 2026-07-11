package io.torvox.cucumber

import androidx.compose.ui.test.junit4.createAndroidComposeRule
import io.cucumber.junit.WithJunitRule
import io.torvox.MainActivity
import org.junit.Rule

@WithJunitRule
class ComposeRuleHolder {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()
}
