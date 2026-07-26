package io.term

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
            device.executeShellCommand("am start -n $PACKAGE/io.term.MainActivity")
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
        val drawerBtn = device.findObject(By.desc("Open session drawer"))
        if (drawerBtn != null) {
            drawerBtn.click()
            Thread.sleep(1500)
        }
        val settingsBtn = device.findObject(By.text("Settings"))
        if (settingsBtn != null) {
            settingsBtn.click()
            Thread.sleep(3000)
        }
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
        val found = device.wait(Until.hasObject(By.text("Dracula Plus")), 5000)
        assertTrue("Should see Dracula Plus theme name", found)
    }

    @Test
    fun settings_shows_restore_sessions() {
        openSettings()
        val restoreReady = device.wait(Until.hasObject(By.textContains("Restore")), WAIT_TIMEOUT)
        if (!restoreReady) {
            scrollTo("Restore", maxSwipes = 40)
        }
        val found = device.findObject(By.textContains("Restore")) != null
        assertTrue("Should see Restore sessions", found)
    }

    @Test
    fun settings_shows_keyboard_mode() {
        openSettings()
        scrollTo("Keyboard Mode")
        val found = device.findObject(By.text("Keyboard Mode")) != null
        assertTrue("Should see Keyboard Mode", found)
    }

    @Test
    fun settings_shows_bootstrap_with_install_buttons() {
        openSettings()
        scrollTo("Bootstrap")
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
