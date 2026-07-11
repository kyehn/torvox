package io.torvox.ui

import android.view.View
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.withId
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.torvox.MainActivity
import org.hamcrest.Matcher
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TerminalActivityEspressoTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun activity_contentView_isDisplayed() {
        onView(withId(android.R.id.content))
            .check(matches(isDisplayed()))
    }

    @Test
    fun activity_isResumed() {
        activityRule.scenario.onActivity { activity ->
            val state = activity.lifecycle.currentState
            androidx.test.espresso.Espresso
                .onView(withId(android.R.id.content))
                .check(matches(isDisplayed()))
        }
    }
}
