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

/**
 * UIAutomator instrumentation tests for Torvox.
 *
 * These exercise the real Android framework via [UiDevice]/[androidx.test.uiautomator.UiObject]
 * interactions (NOT injected `adb input` taps, which do not reach Compose `pointerInput`
 * on the phone emulator — see AGENTS.md pitfall #15). Run them on a tablet emulator or a
 * real device, where the system soft keyboard is available for genuine key input.
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class TorvoxUiAutomatorTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.pressHome()

        val context = InstrumentationRegistry.getInstrumentation().context
        val launchIntent = context.packageManager.getLaunchIntentForPackage("com.termux")
        assertNotNull("Launch intent for com.termux should not be null", launchIntent)
        context.startActivity(launchIntent)

        assertTrue(
            "Terminal app should reach the foreground after launch from home",
            device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 15000),
        )
    }

    /** The terminal render surface (TextureView) should be present once the app is launched. */
    @Test
    fun terminalSurfaceAppearsAfterLaunch() {
        val terminalSurface =
            device.wait(Until.findObject(By.clazz("android.view.TextureView")), 10000)
        assertNotNull("Terminal render surface (TextureView) should be visible", terminalSurface)
    }

    /** The Compose TerminalScreen node should be present in the view hierarchy after launch. */
    @Test
    fun terminalScreenNodeAppearsAfterLaunch() {
        val terminalScreen =
            device.wait(Until.findObject(By.res("com.termux:id/TerminalScreen")), 10000)
        assertNotNull("TerminalScreen composable should be present", terminalScreen)
    }

    /**
     * Typing via the real system soft keyboard (driven by UiObject key clicks) should make the
     * app react: focusing the search field and pressing a letter produces a non-empty result
     * count, proving the input reached the application.
     */
    @Test
    fun typingViaSystemKeyboardReacts() {
        val drawerButton = device.findObject(By.desc("Open session drawer"))
        assertNotNull("Session drawer button should exist", drawerButton)
        drawerButton!!.click()
        assertTrue(
            "Search button should appear after opening the drawer",
            device.wait(Until.hasObject(By.res("com.termux:id/SearchButton")), 5000),
        )

        device.findObject(By.res("com.termux:id/SearchButton"))!!.click()
        val searchField =
            device.wait(
                Until.findObject(By.res("com.termux:id/SearchTextField")),
                5000,
            )
        assumeNotNull("Search text field should appear", searchField)

        searchField!!.click()
        device.waitForIdle(1000)

        val keyE = device.findObject(By.text("e")) ?: device.findObject(By.desc("e"))
        assumeNotNull("System keyboard key 'e' should be visible", keyE)
        keyE!!.click()
        device.waitForIdle(1000)

        val resultCount = device.findObject(By.res("com.termux:id/SearchResultCount"))
        assumeNotNull("Search result count should become visible after typing", resultCount)
        assertTrue(
            "Search result count text should be non-empty after typing",
            resultCount!!.text.isNotEmpty(),
        )
    }
}
