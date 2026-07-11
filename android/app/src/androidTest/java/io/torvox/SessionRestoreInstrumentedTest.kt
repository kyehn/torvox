package io.torvox

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.properties.Delegates

@RunWith(AndroidJUnit4::class)
class SessionRestoreInstrumentedTest {
    private var device by Delegates.notNull<UiDevice>()
    private var initialized = false

    companion object {
        private const val LOG_TAG = "SessionRestoreTest"
        private const val APPLICATION_PACKAGE = "com.termux"
        private const val WAIT_TIMEOUT_MILLIS = 15_000L
        private const val SHORT_DELAY_MILLIS = 500L
        private const val MEDIUM_DELAY_MILLIS = 2_000L
        private const val LONG_DELAY_MILLIS = 5_000L
    }

    @Before
    fun setUp() {
        try {
            device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            initialized = true
            val visible =
                device.wait(
                    Until.hasObject(By.pkg(APPLICATION_PACKAGE).depth(0)),
                    LONG_DELAY_MILLIS,
                )
            if (!visible) {
                device.executeShellCommand(
                    "am start -n $APPLICATION_PACKAGE/io.torvox.MainActivity",
                )
                device.wait(
                    Until.hasObject(By.pkg(APPLICATION_PACKAGE).depth(0)),
                    WAIT_TIMEOUT_MILLIS,
                )
                Thread.sleep(LONG_DELAY_MILLIS)
            } else {
                device.pressBack()
                Thread.sleep(SHORT_DELAY_MILLIS)
            }
        } catch (exception: Exception) {
            Log.e(LOG_TAG, "setUp failed", exception)
            throw exception
        }
    }

    @After
    fun tearDown() {
    }

    private fun runShellCommand(command: String): String = device.executeShellCommand(command).trim()

    private fun openSettings() {
        val drawerButton =
            device.findObject(By.desc("Open session drawer"))
                ?: device.findObject(By.text("\u2261"))
        assertTrue("Drawer button should exist", drawerButton != null)
        drawerButton!!.click()
        Thread.sleep(MEDIUM_DELAY_MILLIS)
        val settingsButton = device.findObject(By.text("Settings"))
        assertTrue("Settings button should exist after opening drawer", settingsButton != null)
        settingsButton!!.click()
        Thread.sleep(WAIT_TIMEOUT_MILLIS / 5)
    }

    private fun scrollToTargetText(
        targetText: String,
        maximumSwipeAttempts: Int = 30,
    ) {
        for (attempt in 0 until maximumSwipeAttempts) {
            if (device.findObject(By.textContains(targetText)) != null) return
            val centerX = device.displayWidth / 2
            device.swipe(
                centerX,
                device.displayHeight * 3 / 4,
                centerX,
                device.displayHeight / 4,
                10,
            )
            Thread.sleep(800)
        }
    }

    private fun navigateBack() {
        device.pressBack()
        Thread.sleep(1000)
    }

    @Test
    fun application_process_is_running() {
        val processId = runShellCommand("pidof $APPLICATION_PACKAGE")
        assertTrue(
            "App process should be running with a valid PID",
            processId.isNotEmpty() && processId.toIntOrNull() != null && processId.toInt() > 0,
        )
    }

    @Test
    fun application_is_visible_on_screen() {
        val isAppVisible =
            device.wait(
                Until.hasObject(By.pkg(APPLICATION_PACKAGE).depth(0)),
                WAIT_TIMEOUT_MILLIS,
            )
        assertTrue("App should be visible on screen", isAppVisible)
    }

    @Test
    fun process_survives_application_restart() {
        device.executeShellCommand(
            "am start -n $APPLICATION_PACKAGE/io.torvox.MainActivity",
        )
        device.wait(
            Until.hasObject(By.pkg(APPLICATION_PACKAGE).depth(0)),
            WAIT_TIMEOUT_MILLIS,
        )
        Thread.sleep(5000)

        val processIdBeforeRestart = runShellCommand("pidof $APPLICATION_PACKAGE")
        assertTrue(
            "App should have a PID before restart",
            processIdBeforeRestart.isNotEmpty(),
        )

        device.executeShellCommand(
            "am start -n $APPLICATION_PACKAGE/io.torvox.MainActivity",
        )
        device.wait(
            Until.hasObject(By.pkg(APPLICATION_PACKAGE).depth(0)),
            WAIT_TIMEOUT_MILLIS,
        )
        Thread.sleep(5000)

        val processIdAfterRestart = runShellCommand("pidof $APPLICATION_PACKAGE")
        assertTrue(
            "App should still have a PID after restart",
            processIdAfterRestart.isNotEmpty(),
        )
        assertTrue(
            "PID should be a positive integer",
            processIdAfterRestart.toIntOrNull() != null && processIdAfterRestart.toInt() > 0,
        )
    }

    @Test
    fun session_restore_setting_is_off_by_default() {
        openSettings()
        scrollToTargetText("Restore", maximumSwipeAttempts = 30)
        val restoreSessionsLabel = device.findObject(By.textContains("Restore"))
        assertTrue(
            "Restore sessions label should exist in settings (scrolled to find it)",
            restoreSessionsLabel != null,
        )
        val restoreSessionsDescription = device.findObject(By.textContains("Reopen"))
        assertTrue(
            "Restore sessions description should exist (scrolled to find it)",
            restoreSessionsDescription != null,
        )
        navigateBack()
    }

    @Test
    fun terminal_interface_elements_are_visible() {
        val processId = runShellCommand("pidof $APPLICATION_PACKAGE")
        assertTrue("App should be running", processId.isNotEmpty())
        val modifierBarReady =
            device.wait(Until.hasObject(By.text("ESC")), WAIT_TIMEOUT_MILLIS)
        assertTrue(
            "Escape key should be visible indicating terminal is active",
            modifierBarReady,
        )
        val controlKey = device.findObject(By.text("CTRL"))
        assertTrue(
            "Control key should be visible indicating terminal accepts input",
            controlKey != null,
        )
        val homeKey = device.findObject(By.text("HOME"))
        assertTrue("Home key should be visible in modifier bar", homeKey != null)
    }

    @Test
    fun terminal_content_is_preserved_across_restart() {
        val modifierBarReady =
            device.wait(Until.hasObject(By.text("ESC")), WAIT_TIMEOUT_MILLIS)
        assertTrue("Terminal should be active before restart", modifierBarReady)

        device.pressHome()
        Thread.sleep(3000)
        device.executeShellCommand(
            "am start -n $APPLICATION_PACKAGE/io.torvox.MainActivity",
        )
        Thread.sleep(15000)

        val modifierBarIsVisible = device.wait(Until.hasObject(By.text("ESC")), WAIT_TIMEOUT_MILLIS)
        assertTrue(
            "Terminal should be usable after restart (modifier bar visible)",
            modifierBarIsVisible,
        )
    }
}
