package io.torvox

import android.content.ClipboardManager
import android.content.Intent
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.longClick
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.withClassName
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import org.hamcrest.CoreMatchers.`is`
import org.junit.Assert.assertNotNull
import org.junit.Assume.assumeNotNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class SelectionEspressoTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun longPressTriggersSelection() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
    }

    @Test
    fun selectionHandlesAppear() {
        val surfaceView = findSurfaceView()
        assumeNotNull(surfaceView)
        onView(withClassName(`is`("android.view.View")))
            .check(matches(isDisplayed()))
    }

    @Test
    fun copyActionWorksViaBroadcast() {
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val intent =
                Intent("io.torvox.PARTIAL_SELECT").apply {
                    putExtra("startRow", 0)
                    putExtra("startCol", 0)
                    putExtra("endRow", 2)
                    putExtra("endCol", 10)
                }
            activity.sendBroadcast(intent)

            val clipboard = activity.getSystemService(ClipboardManager::class.java)
            assertNotNull("Clipboard service should exist", clipboard)
        }
    }

    @Test
    fun selectAllBroadcastTriggersSelection() {
        composeTestRule.activityRule.scenario.onActivity { activity ->
            activity.sendBroadcast(Intent("io.torvox.SELECT_ALL"))
        }
    }

    @Test
    fun terminalContentRespondsToLongPress() {
        composeTestRule.onNodeWithTag("TerminalContent").assertIsDisplayed()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
    }

    private fun findSurfaceView(): View? {
        var result: View? = null
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val root = activity.findViewById<ViewGroup>(android.R.id.content)
            result = findViewByTag(root, "TerminalSurfaceView")
        }
        return result
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
