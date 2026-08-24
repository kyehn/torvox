// TODO(kotlin-2.4.0-false-positive)
@file:Suppress("UNNECESSARY_NOT_NULL_ASSERTION")

package terminal.emulator.ui

import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import java.io.File

@RunWith(AndroidJUnit4::class)
@LargeTest
class TextSearchUiAutomatorTest {
    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // without pre-granting it the system permission dialog covers the UI
    // and the drawer/search nodes never appear (cold-start runs).
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 15000)
    }

    private fun openSearchBar() {
        if (!device.wait(Until.hasObject(By.desc("Open session drawer")), 30000)) {
            val dumpFile = File(InstrumentationRegistry.getInstrumentation().targetContext.filesDir, "cold_start_dump.xml")
            device.dumpWindowHierarchy(dumpFile)
            throw AssertionError("Drawer button should appear (cold start, session spawn). UI dump:\n${dumpFile.readText()}")
        }
        val drawerButton = device.findObject(By.desc("Open session drawer"))
        requireNotNull(drawerButton).click()
        assertTrue(
            "SearchButton should appear in drawer",
            device.wait(Until.hasObject(By.res("SearchButton")), 5000),
        )
        // Let the drawer open animation settle: clicking a moving target
        // taps the wrong coordinates (compose-semantic clicks are not
        // affected, UiAutomator coordinate clicks are).
        device.waitForIdle(1500)
        val searchButton = device.findObject(By.res("SearchButton"))
        requireNotNull(searchButton).click()
        device.waitForIdle(2000)
        if (!device.wait(Until.hasObject(By.res("SearchTextField")), 1500)) {
            val dumpFile = File(InstrumentationRegistry.getInstrumentation().targetContext.filesDir, "search_open_dump.xml")
            device.dumpWindowHierarchy(dumpFile)
            throw AssertionError("Search bar did not open after SearchButton click. UI dump:\n${dumpFile.readText()}")
        }
    }

    private fun waitForSearchBar(): Boolean = device.wait(Until.hasObject(By.res("SearchTextField")), 5000)

    @Test
    fun openSearchBar_opensSearchUI() {
        openSearchBar()
        assertTrue("Search bar must open after clicking search button", waitForSearchBar())
    }

    @Test
    fun searchNavigatesResults() {
        openSearchBar()
        assertTrue("Search bar must open", waitForSearchBar())

        val searchField = device.findObject(By.res("SearchTextField"))
        assertNotNull("Search field should exist", searchField)
        requireNotNull(searchField).text = "e"
        device.waitForIdle(1000)

        val nextButton = device.findObject(By.res("SearchNext"))
        assertNotNull("Next button should exist", nextButton)
        requireNotNull(nextButton).click()
        device.waitForIdle(500)

        val resultCount = device.findObject(By.res("SearchResultCount"))
        assertNotNull("Result count should be visible after navigating", resultCount)
        assertTrue("Result count text should be non-empty", requireNotNull(resultCount).text.isNotEmpty())

        val prevButton = device.findObject(By.res("SearchPrevious"))
        assertNotNull("Previous button should exist", prevButton)
        requireNotNull(prevButton).click()
        device.waitForIdle(500)
    }

    @Test
    fun searchClose_closesSearchBar() {
        openSearchBar()
        assertTrue("Search bar must open", waitForSearchBar())

        val closeButton = device.findObject(By.res("SearchClose"))
        assertNotNull("Close button should exist", closeButton)
        requireNotNull(closeButton).click()
        device.waitForIdle(1000)

        val drawerAfterClose = device.findObject(By.res("Key_DRAWER"))
        assertNotNull("Modifier bar drawer button should be visible after search close", drawerAfterClose)
        assertTrue(
            "Search bar must be gone after close",
            !device.wait(Until.hasObject(By.res("SearchTextField")), 1000),
        )
    }

    @Test
    fun searchCaseToggle_cycles() {
        openSearchBar()
        assertTrue("Search bar must open", waitForSearchBar())

        val caseToggle = device.findObject(By.res("SearchCaseSensitive"))
        assertNotNull("Case toggle should exist", caseToggle)
        requireNotNull(caseToggle).click()
        device.waitForIdle(500)
        requireNotNull(caseToggle).click()
        device.waitForIdle(500)
        val caseToggleAfter = device.findObject(By.res("SearchCaseSensitive"))
        assertNotNull("Case toggle must remain present after cycling", caseToggleAfter)
    }

    @Test
    fun searchResultCountVisible() {
        openSearchBar()
        assertTrue("Search bar must open", waitForSearchBar())

        val searchField = device.findObject(By.res("SearchTextField"))
        assertNotNull("Search field should exist", searchField)
        // Focus the field first: UiAutomator setText on an unfocused
        // compose TextField does not always dispatch the text change.
        searchField.click()
        device.waitForIdle(500)
        requireNotNull(searchField).text = "e"
        Thread.sleep(2200) // search debounce (150 ms) + scrollback scan
        device.waitForIdle(500)

        val resultCount = device.findObject(By.res("SearchResultCount"))
        assertNotNull("Result count should be visible after typing", resultCount)
        assertTrue("Result count text should be non-empty", requireNotNull(resultCount).text.isNotEmpty())
    }
}
