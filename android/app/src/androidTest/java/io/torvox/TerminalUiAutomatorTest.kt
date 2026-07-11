package io.torvox

import android.content.Intent
import android.view.KeyEvent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TerminalUiAutomatorTest {
    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.pressHome()

        val context = InstrumentationRegistry.getInstrumentation().context
        val intent = context.packageManager.getLaunchIntentForPackage("com.termux")
        intent?.addFlags(Intent.FLAG_ACTIVITY_CLEAR_TASK)
        context.startActivity(intent)

        device.wait(Until.hasObject(By.pkg("com.termux").depth(0)), 5000)
    }

    @Test
    fun appLaunches() {
        val termuxApp = device.wait(Until.findObject(By.pkg("com.termux")), 3000)
        assertNotNull("App should be running", termuxApp)
    }

    @Test
    fun keyboardInputWorks() {
        device.wait(Until.hasObject(By.pkg("com.termux")), 3000)
        device.findObject(By.pkg("com.termux"))?.let { terminal ->
            terminal.click()
            device.pressKeyCode(KeyEvent.KEYCODE_E)
            device.pressKeyCode(KeyEvent.KEYCODE_C)
            device.pressKeyCode(KeyEvent.KEYCODE_H)
            device.pressKeyCode(KeyEvent.KEYCODE_O)
            device.pressEnter()
        }
    }
}
