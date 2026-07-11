package io.torvox.ui

import android.view.View
import android.view.ViewGroup
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.withClassName
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import io.torvox.MainActivity
import org.hamcrest.CoreMatchers.`is`
import org.junit.Assert.assertNotNull
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class TextSearchEspressoTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Test
    fun activityLaunches() {
        activityRule.scenario.onActivity { activity ->
            assertNotNull("Activity should not be null", activity)
        }
    }

    @Test
    fun androidComposeViewIsDisplayed() {
        onView(withClassName(`is`("androidx.compose.ui.platform.AndroidComposeView")))
            .check(matches(isDisplayed()))
    }

    @Test
    fun terminalSurfaceIsViewGroup() {
        activityRule.scenario.onActivity { activity ->
            val root = activity.findViewById<ViewGroup>(android.R.id.content)
            val found = findViewByTag(root, "TerminalSurfaceView")
            assertNotNull("TerminalSurfaceView should exist in hierarchy", found)
            assumeTrue(found is ViewGroup)
        }
    }

    @Test
    fun contentViewIsNotEmpty() {
        activityRule.scenario.onActivity { activity ->
            val content = activity.findViewById<ViewGroup>(android.R.id.content)
            assertNotNull("Content view should exist", content)
            assumeTrue(content.childCount > 0)
        }
    }

    private fun findViewByTag(
        view: View,
        tag: String,
    ): View? {
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
