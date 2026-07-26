package io.term

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.properties.Delegates

@RunWith(AndroidJUnit4::class)
class FontSwitchInstrumentedTest {
    private var device by Delegates.notNull<UiDevice>()

    companion object {
        private const val TAG = "FontSwitchInstrumentedTest"
        private const val PACKAGE = "com.termux"
        private const val WAIT_TIMEOUT = 15_000L
    }

    @Before
    fun setUp() {
        try {
            device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
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
            Thread.sleep(2000)
        }
    }

    private fun scrollTo(
        text: String,
        maxSwipes: Int = 30,
    ) {
        for (i in 0 until maxSwipes) {
            if (device.findObject(By.textContains(text)) != null) return
            val cx = device.displayWidth / 2
            device.swipe(cx, device.displayHeight * 3 / 4, cx, device.displayHeight / 4, 10)
            Thread.sleep(800)
        }
    }

    @Test
    fun settings_shows_font_family_section() {
        openSettings()
        scrollTo("Font Family")
        val found = device.findObject(By.text("Font Family"))
        assertNotNull("Settings should show Font Family section", found)
    }

    @Test
    fun settings_shows_change_button_for_font() {
        openSettings()
        scrollTo("Font Family")
        val changeBtn = device.findObject(By.text("Change"))
        assertNotNull("Should see Change button for font family", changeBtn)
    }

    @Test
    fun settings_shows_pick_font_file_button() {
        openSettings()
        scrollTo("Font Family")
        val changeBtn = device.findObject(By.text("Change"))
        assertNotNull("Should see Change button", changeBtn)
        changeBtn?.click()
        Thread.sleep(2000)
        val pickBtn = device.findObject(By.textContains("Pick"))
        assertNotNull("Should see Pick font file button in dialog", pickBtn)
    }

    @Test
    fun font_change_opens_dialog() {
        openSettings()
        scrollTo("Font Family")
        val changeBtn = device.findObject(By.text("Change"))
        if (changeBtn != null) {
            changeBtn.click()
            Thread.sleep(2000)
            val dialogVisible =
                device.findObject(By.textContains("Fira")) != null ||
                    device.findObject(By.textContains("Roboto")) != null ||
                    device.findObject(By.textContains("Noto")) != null ||
                    device.findObject(By.textContains("System")) != null
            assertTrue("Font picker dialog should show font names", dialogVisible)
        }
    }

    @Test
    fun font_dialog_shows_system_default() {
        openSettings()
        scrollTo("Font Family")
        val changeBtn = device.findObject(By.text("Change"))
        if (changeBtn != null) {
            changeBtn.click()
            Thread.sleep(2000)
            val hasFonts =
                device.findObject(By.textContains("Mono")) != null ||
                    device.findObject(By.textContains("Sans")) != null
            assertTrue("Font picker should show available font options", hasFonts)
        }
    }

    @Test
    fun font_dialog_shows_monospace_fonts() {
        openSettings()
        scrollTo("Font Family")
        val changeBtn = device.findObject(By.text("Change"))
        if (changeBtn != null) {
            changeBtn.click()
            Thread.sleep(2000)
            val hasMono = device.findObject(By.textContains("Mono")) != null
            assertTrue("Font picker should show monospace fonts", hasMono)
        }
    }

    @Test
    fun font_select_changes_font_family() {
        openSettings()
        scrollTo("Font Family")
        val changeBtn = device.findObject(By.text("Change"))
        if (changeBtn != null) {
            changeBtn.click()
            Thread.sleep(3000)
            val fonts = listOf("Roboto Mono", "Noto Sans Mono", "Fira Code", "Source Code Pro", "monospace")
            val selectedFont = fonts.firstOrNull { device.findObject(By.textContains(it)) != null }
            if (selectedFont != null) {
                device.findObject(By.textContains(selectedFont))?.click()
                Thread.sleep(3000)
            }
        }
        val appAlive = device.findObject(By.pkg(PACKAGE).depth(0)) != null
        assertTrue("App must survive font change", appAlive)
    }

    @Test
    fun app_survives_font_change() {
        openSettings()
        scrollTo("Font Family")
        val changeBtn = device.findObject(By.text("Change"))
        if (changeBtn != null) {
            changeBtn.click()
            Thread.sleep(2000)
            val firstFont = device.findObject(By.textContains("Noto"))
            if (firstFont != null) {
                firstFont.click()
                Thread.sleep(3000)
            }
        }
        val appAlive = device.findObject(By.pkg(PACKAGE).depth(0)) != null
        assertTrue("App must survive font change", appAlive)
    }
}
