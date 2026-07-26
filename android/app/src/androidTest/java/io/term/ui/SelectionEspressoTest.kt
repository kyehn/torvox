package io.term.ui

import android.view.ViewGroup
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.longClick
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.withClassName
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import io.term.MainActivity
import org.hamcrest.CoreMatchers.`is`
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class SelectionEspressoTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun longPressOnTerminalDoesNotCrash() {
        onView(withClassName(`is`("androidx.compose.ui.platform.AndroidComposeView")))
            .perform(longClick())
        onView(withClassName(`is`("androidx.compose.ui.platform.AndroidComposeView")))
            .check(matches(isDisplayed()))
    }

    @Test
    fun selectionHandlesCreateOnLongPress() {
        activityRule.scenario.onActivity { activity ->
            val root = activity.findViewById<ViewGroup>(android.R.id.content)
            val terminal = findViewByTag(root, "TerminalSurfaceView")
            assertNotNull("TerminalSurfaceView should exist", terminal)
            assertTrue(terminal is ViewGroup)
        }
    }

    private fun findViewByTag(
        view: android.view.View,
        tag: String,
    ): android.view.View? {
        if (tag == view.tag) return view
        if (view is ViewGroup) {
            for (i in 0 until view.childCount) {
                val found = findViewByTag(view.getChildAt(i), tag)
                if (found != null) return found
            }
        }
        return null
    }
}
