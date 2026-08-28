package terminal.emulator

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Direction
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.properties.Delegates

@RunWith(AndroidJUnit4::class)
class SettingsInstrumentedTest {
    private var device by Delegates.notNull<UiDevice>()
    private var initialized = false

    companion object {
        private const val TAG = "SettingsInstrumentedTest"
        private const val PACKAGE = "com.termux"
        private const val WAIT_TIMEOUT = 15_000L
    }

    @Before
    fun setUp() {
        try {
            device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            initialized = true
            // MainActivity.onCreate() requests POST_NOTIFICATIONS on first run
            // (Android 13+); the permission dialog overlays the activity and
            // blocks every UiAutomator lookup below. Grant up front so the
            // dialog never appears (see TestUtils.waitForSession).
            grantNotificationPermission()
            // Previous tests leave the app parked on the settings screen /
            // dialogs; am start merely brings that state to the foreground.
            // --activity-clear-task rebuilds the activity (state reset)
            // without force-stopping: force-stop would kill the
            // instrumentation process itself, since androidTest runs inside
            // the target app process.
            device.executeShellCommand("am start --activity-clear-task -n $PACKAGE/terminal.emulator.MainActivity")
            device.wait(Until.hasObject(By.pkg(PACKAGE).depth(0)), WAIT_TIMEOUT)
            Thread.sleep(5000)
        } catch (exception: Exception) {
            Log.e(TAG, "setUp failed", exception)
            throw exception
        }
    }

    @After
    fun tearDown() {
    }

    private fun openSettings() {
        assertTrue(
            "Drawer button (Open session drawer) must appear",
            device.wait(Until.hasObject(By.desc("Open session drawer")), WAIT_TIMEOUT),
        )
        device.findObject(By.desc("Open session drawer"))?.click()
        assertTrue(
            "Settings entry must appear in the session drawer",
            device.wait(Until.hasObject(By.text("Settings")), WAIT_TIMEOUT),
        )
        device.findObject(By.text("Settings"))?.click()
        assertTrue(
            "Settings screen must open (Appearance section visible)",
            device.wait(Until.hasObject(By.text("Appearance")), WAIT_TIMEOUT),
        )
    }

    private fun scrollTo(
        text: String,
        maxSwipes: Int = 25,
    ) {
        for (i in 0 until maxSwipes) {
            if (device.findObject(By.textContains(text)) != null) return
            val cx = device.displayWidth / 2
            device.swipe(cx, device.displayHeight * 3 / 4, cx, device.displayHeight / 4, 10)
            Thread.sleep(1200)
        }
    }

    /** Scroll until the Font Family row (with its Change action) is visible. */
    private fun scrollToChange() {
        scrollTo("Font Family")
        // The title enters the viewport at its bottom edge; the row below it
        // (Change action) may still be off-screen. One extra swipe fixes it.
        if (device.findObject(By.text("Change")) == null) {
            val cx = device.displayWidth / 2
            device.swipe(cx, device.displayHeight * 3 / 4, cx, device.displayHeight / 4, 10)
            Thread.sleep(1200)
        }
    }

    @Test
    fun settings_opens_and_shows_appearance() {
        openSettings()
        val found = device.wait(Until.hasObject(By.text("Appearance")), 5000)
        assertTrue("Settings should show Appearance section", found)
    }

    @Test
    fun settings_shows_font_family() {
        openSettings()
        val found = device.wait(Until.hasObject(By.text("Font Family")), 5000)
        assertTrue("Settings should show Font Family", found)
    }

    @Test
    fun settings_shows_theme_names_below_boxes() {
        openSettings()
        scrollTo("Dracula Plus")
        val found = device.wait(Until.hasObject(By.text("Dracula Plus")), 5000)
        assertTrue("Should see Dracula Plus theme name", found)
    }

    @Test
    fun settings_shows_bootstrap_with_install_buttons() {
        openSettings()
        scrollTo("Presets")
        // "Presets" is the header directly above the preset card; the
        // "Install" action sits at its bottom edge, so one extra swipe when
        // the preset card is taller than the viewport.
        if (device.findObject(By.text("Install")) == null) {
            val cx = device.displayWidth / 2
            device.swipe(cx, device.displayHeight * 3 / 4, cx, device.displayHeight / 4, 10)
            Thread.sleep(1200)
        }
        val hasBootstrap = device.findObject(By.textContains("Bootstrap")) != null
        assertTrue("Should see Bootstrap section", hasBootstrap)
        val hasTermuxDefault = device.findObject(By.text("Termux Default")) != null
        assertTrue("Should see Termux Default preset", hasTermuxDefault)
        val hasInstall = device.findObject(By.text("Install")) != null
        assertTrue("Should see Install button", hasInstall)
    }

    @Test
    fun settings_no_nerd_osc133_toggles() {
        openSettings()
        Thread.sleep(2000)
        val hasNerd = device.findObject(By.textContains("Nerd")) != null
        assertFalse("Should NOT see Nerd toggle", hasNerd)
        val hasOsc = device.findObject(By.textContains("OSC133")) != null
        assertFalse("Should NOT see OSC133 toggle", hasOsc)
    }
}
