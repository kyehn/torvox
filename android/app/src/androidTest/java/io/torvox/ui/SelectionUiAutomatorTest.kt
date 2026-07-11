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
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class SelectionUiAutomatorTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 15000)
    }

    @Test
    fun tapOnTerminalSurface() {
        val terminal = device.findObject(By.pkg("com.termux"))
        assertNotNull("Terminal should exist", terminal)
        terminal!!.click()
    }

    @Test
    fun longPressAndContextMenuAppears() {
        val terminalSurface = device.findObject(By.desc("Terminal"))
        assertNotNull("TerminalSurface should exist", terminalSurface)
        terminalSurface!!.click()
    }
}
