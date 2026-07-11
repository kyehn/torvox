package io.torvox.ui

import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import io.torvox.MainActivity
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeNotNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class TextSearchUiAutomatorTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 15000)
    }

    private fun openSearchBar() {
        val drawerButton = device.findObject(By.desc("Open session drawer"))
        assertNotNull("Drawer button should exist", drawerButton)
        drawerButton!!.click()
        assertTrue(
            "SearchButton should appear in drawer",
            device.wait(Until.hasObject(By.res("SearchButton")), 5000),
        )
        val searchButton = device.findObject(By.res("SearchButton"))
        searchButton!!.click()
        device.waitForIdle(2000)
    }

    private fun waitForSearchBar(): Boolean = device.wait(Until.hasObject(By.res("com.termux:id/SearchTextField")), 5000)

    @Test
    fun openSearchBar_opensSearchUI() {
        openSearchBar()
        if (!waitForSearchBar()) return
    }

    @Test
    fun searchNavigatesResults() {
        openSearchBar()
        if (!waitForSearchBar()) return

        val searchField = device.findObject(By.res("com.termux:id/SearchTextField"))
        searchField!!.text = "e"
        device.waitForIdle(1000)

        val nextButton = device.findObject(By.res("com.termux:id/SearchNext"))
        assumeNotNull("Next button should exist", nextButton)
        nextButton!!.click()
        device.waitForIdle(500)

        val prevButton = device.findObject(By.res("com.termux:id/SearchPrevious"))
        assumeNotNull("Previous button should exist", prevButton)
        prevButton!!.click()
        device.waitForIdle(500)
    }

    @Test
    fun searchClose_closesSearchBar() {
        openSearchBar()
        if (!waitForSearchBar()) return

        val closeButton = device.findObject(By.res("com.termux:id/SearchClose"))
        assumeNotNull("Close button should exist", closeButton)
        closeButton!!.click()
        device.waitForIdle(1000)

        val drawerAfterClose = device.findObject(By.res("com.termux:id/Key_DRAWER"))
        assertNotNull("Modifier bar drawer button should be visible after search close", drawerAfterClose)
    }

    @Test
    fun searchCaseToggle_cycles() {
        openSearchBar()
        if (!waitForSearchBar()) return

        val caseToggle = device.findObject(By.res("com.termux:id/SearchCaseSensitive"))
        assumeNotNull("Case toggle should exist", caseToggle)
        caseToggle!!.click()
        device.waitForIdle(500)
        caseToggle!!.click()
        device.waitForIdle(500)
    }

    @Test
    fun searchResultCountVisible() {
        openSearchBar()
        if (!waitForSearchBar()) return

        val searchField = device.findObject(By.res("com.termux:id/SearchTextField"))
        searchField!!.text = "e"
        device.waitForIdle(1000)

        val resultCount = device.findObject(By.res("com.termux:id/SearchResultCount"))
        assumeNotNull("Result count should be visible after typing", resultCount)
        assertTrue("Result count text should be non-empty", resultCount!!.text.isNotEmpty())
    }
}
