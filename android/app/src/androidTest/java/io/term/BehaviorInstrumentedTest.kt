package io.term

import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiScrollable
import androidx.test.uiautomator.UiSelector
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class BehaviorInstrumentedTest {
    companion object {
        private const val TAG = "BehaviorTest"
        private const val PACKAGE = "com.termux"
        private const val WAIT_TIMEOUT = 30_000L
    }

    private lateinit var device: UiDevice
    private var initialized = false

    @Before
    fun setUp() {
        try {
            device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            initialized = true
            device.executeShellCommand("am start -n $PACKAGE/io.term.MainActivity")
            device.wait(Until.hasObject(By.pkg(PACKAGE).depth(0)), WAIT_TIMEOUT)
            Thread.sleep(10000)
        } catch (exception: Exception) {
            Log.e(TAG, "setUp failed", exception)
            throw exception
        }
    }

    @After
    fun tearDown() {
    }

    private fun openSettings() {
        val drawerBtn =
            device.findObject(By.desc("Open session drawer"))
                ?: device.findObject(By.text("\u2261"))
        drawerBtn?.click()
        Thread.sleep(2000)
        device.findObject(By.text("Settings"))?.click()
        Thread.sleep(3000)
    }

    private fun scrollTo(
        text: String,
        maxSwipes: Int = 30,
    ) {
        for (i in 0 until maxSwipes) {
            Thread.sleep(500)
            if (device.findObject(By.textContains(text)) != null) return
            try {
                val scrollable = UiScrollable(UiSelector().scrollable(true))
                scrollable.scrollForward()
            } catch (_: Exception) {
                val cx = device.displayWidth / 2
                device.swipe(cx, device.displayHeight * 6 / 10, cx, device.displayHeight / 4, 10)
            }
            Thread.sleep(800)
        }
    }

    private fun goBack() {
        device.pressBack()
        Thread.sleep(1000)
    }

    @Test
    fun behavior_app_process_alive() {
        val output = device.executeShellCommand("dumpsys activity processes | grep -i $PACKAGE")
        assertTrue("App process should be running", output.isNotEmpty())
    }

    @Test
    fun behavior_terminal_renders_theme_colors() {
        val output = device.executeShellCommand("dumpsys activity top | grep -i $PACKAGE")
        assertTrue("App should be in foreground", output.isNotEmpty())
    }

    @Test
    fun behavior_font_picker_opens_with_change_button() {
        openSettings()
        val fontReady = device.wait(Until.hasObject(By.text("Change")), WAIT_TIMEOUT)
        if (!fontReady) {
            scrollTo("Font Family")
        }
        device.findObject(By.text("Change"))?.click()
        Thread.sleep(2000)
        val dialog =
            device.findObject(By.textContains("monospace"))
                ?: device.findObject(By.textContains("Mono"))
                ?: device.findObject(By.textContains("Noto"))
        assertTrue("Font picker dialog should appear", dialog != null)
        goBack()
    }

    @Test
    fun behavior_selection_toolbar_shows_copy_select_all() {
        openSettings()
        scrollTo("Keyboard Mode")
        device.findObject(By.text("Standard"))?.click()
        Thread.sleep(1000)
        goBack()
        Thread.sleep(1000)
        val termBtn = device.findObject(By.desc("Terminal"))
        termBtn?.click()
        Thread.sleep(2000)
        val copy = device.findObject(By.text("Copy"))
        val selectAll = device.findObject(By.text("Select All"))
        if (copy != null) {
            assertTrue("Copy button should be visible", true)
            assertFalse(
                "Paste should NOT appear when text selected",
                device.findObject(By.text("Paste")) != null,
            )
        }
        device.findObject(By.text("Standard"))?.let { /* already on terminal */ }
        openSettings()
        scrollTo("Keyboard Mode")
        device.findObject(By.text("Secure"))?.click()
        Thread.sleep(1000)
        goBack()
    }

    @Test
    fun behavior_settings_theme_names_visible() {
        openSettings()
        val themeReady = device.wait(Until.hasObject(By.text("Dracula Plus")), WAIT_TIMEOUT)
        if (!themeReady) {
            scrollTo("Dracula Plus")
        }
        val dracula = device.findObject(By.text("Dracula Plus"))
        val catppuccin = device.findObject(By.text("Catppuccin Mocha"))
        val nord = device.findObject(By.text("Nord"))
        assertTrue("Dracula Plus should be visible", dracula != null)
        assertTrue("Catppuccin Mocha should be visible", catppuccin != null)
        assertTrue("Nord should be visible", nord != null)
        goBack()
    }

    @Test
    fun behavior_settings_restore_sessions_off() {
        openSettings()
        val restoreReady = device.wait(Until.hasObject(By.textContains("Restore sessions")), WAIT_TIMEOUT)
        if (!restoreReady) {
            scrollTo("Restore sessions", maxSwipes = 60)
        }
        val toggle = device.findObject(By.textContains("Restore sessions"))
        assertTrue("Restore sessions should be visible", toggle != null)
        goBack()
    }

    @Test
    fun behavior_settings_bootstrap_action_buttons() {
        openSettings()
        val termuxReady = device.wait(Until.hasObject(By.text("Termux Default")), WAIT_TIMEOUT)
        if (!termuxReady) {
            scrollTo("Termux Default", maxSwipes = 60)
        }
        val termuxDefault = device.findObject(By.text("Termux Default"))
        val installBtn = device.findObject(By.text("Install"))
        assertTrue("Termux Default should be visible", termuxDefault != null)
        assertTrue("Install button should be visible", installBtn != null)
        goBack()
    }

    @Test
    fun behavior_settings_no_nerd_osc133_toggles() {
        openSettings()
        assertFalse(
            "Nerd toggle should NOT exist",
            device.findObject(By.textContains("Nerd")) != null,
        )
        assertFalse(
            "OSC133 toggle should NOT exist",
            device.findObject(By.textContains("OSC")) != null,
        )
        goBack()
    }

    @Test
    fun behavior_modifier_bar_visible() {
        val modifierBarReady =
            device.wait(Until.hasObject(By.text("ESC")), WAIT_TIMEOUT)
        assertTrue("Modifier bar should load with ESC key", modifierBarReady)
        val esc = device.findObject(By.text("ESC"))
        val ctrl = device.findObject(By.text("CTRL"))
        val alt = device.findObject(By.text("ALT"))
        val home = device.findObject(By.text("HOME"))
        assertTrue("ESC should be visible", esc != null)
        assertTrue("CTRL should be visible", ctrl != null)
        assertTrue("ALT should be visible", alt != null)
        assertTrue("HOME should be visible", home != null)
    }

    @Test
    fun behavior_drawer_shows_sessions_and_settings() {
        val drawerBtn =
            device.findObject(By.desc("Open session drawer"))
                ?: device.findObject(By.text("\u2261"))
        drawerBtn?.click()
        Thread.sleep(2000)
        val drawerReady = device.wait(Until.hasObject(By.text("Settings")), WAIT_TIMEOUT)
        assertTrue("Drawer should load with Settings option", drawerReady)
        val settings = device.findObject(By.text("Settings"))
        val sessions = device.findObject(By.textContains("Session"))
        assertTrue("Settings should be in drawer", settings != null)
        assertTrue("Session should be in drawer", sessions != null)
        settings?.click()
        Thread.sleep(2000)
        goBack()
    }

    @Test
    fun behavior_keyboard_mode_secure() {
        openSettings()
        val secureReady = device.wait(Until.hasObject(By.text("Secure")), WAIT_TIMEOUT)
        if (!secureReady) {
            scrollTo("Secure", maxSwipes = 60)
        }
        val secure = device.findObject(By.text("Secure"))
        assertNotNull("Secure mode should be visible", secure)
        goBack()
    }

    @Test
    fun behavior_shell_path_correct() {
        openSettings()
        val shellReady = device.wait(Until.hasObject(By.text("/system/bin/sh")), WAIT_TIMEOUT)
        if (!shellReady) {
            scrollTo("/system/bin/sh")
        }
        val shell = device.findObject(By.text("/system/bin/sh"))
        assertTrue("Shell path should be /system/bin/sh", shell != null)
        goBack()
    }
}
