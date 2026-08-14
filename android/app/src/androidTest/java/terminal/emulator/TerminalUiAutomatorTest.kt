package terminal.emulator

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
        // 10s: cold start on slow emulators can exceed 3s.
        val termuxApp = device.wait(Until.findObject(By.pkg("com.termux")), 10_000)
        assertNotNull("App should be running", termuxApp)
    }

    @Test
    fun keyboardInputSmoke_noCrash() {
        // 10s: cold start on slow emulators can exceed 3s.
        device.wait(Until.hasObject(By.pkg("com.termux")), 10_000)
        val terminal = checkNotNull(device.findObject(By.pkg("com.termux"))) {
            "Terminal app must be running to send keystrokes"
        }
        terminal.click()
        device.pressKeyCode(KeyEvent.KEYCODE_E)
        device.pressKeyCode(KeyEvent.KEYCODE_C)
        device.pressKeyCode(KeyEvent.KEYCODE_H)
        device.pressKeyCode(KeyEvent.KEYCODE_O)
        device.pressEnter()
    }
}
