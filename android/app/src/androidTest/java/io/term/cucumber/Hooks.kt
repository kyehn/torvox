package io.term.cucumber

import io.cucumber.java.Before
import io.cucumber.java.Scenario
import javax.inject.Inject

class Hooks
@Inject
constructor(
    private val composeRuleHolder: ComposeRuleHolder,
) {
    @Before
    fun setUp(scenario: Scenario) {
        composeRuleHolder.composeRule.waitForIdle()
    }
}
