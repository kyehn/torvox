package io.torvox.ui

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@LargeTest
class TerminalUiAutomatorTest {
    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.pressHome()

        val context = InstrumentationRegistry.getInstrumentation().context
        val intent = context.packageManager.getLaunchIntentForPackage("com.termux")
        assertNotNull("Launch intent should not be null", intent)
        context.startActivity(intent)

        device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 15000)
    }

    @Test
    fun testTerminalSurfaceDisplayed() {
        val terminalView =
            device.wait(
                Until.findObject(By.clazz("android.view.TextureView")),
                10000,
            )
        assertNotNull("Terminal TextureView should be displayed", terminalView)
    }

    @Test
    fun testTerminalScreenContent() {
        val terminalScreen =
            device.wait(
                Until.findObject(By.res("com.termux:id/TerminalScreen")),
                10000,
            )
        assertNotNull("TerminalScreen composable should be present", terminalScreen)
    }

    @Test
    fun testOpenDrawer() {
        val displayWidth = device.displayWidth
        val displayHeight = device.displayHeight
        device.swipe(0, displayHeight / 2, displayWidth / 2, displayHeight / 2, 10)

        val drawerContent =
            device.wait(
                Until.findObject(By.res("com.termux:id/SessionDrawer")),
                5000,
            )
        assertNotNull("Session drawer should appear after swipe", drawerContent)
    }
}
