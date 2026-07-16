package io.torvox.ui

import android.content.Context
import android.graphics.Bitmap
import android.util.Log
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.test.platform.app.InstrumentationRegistry
import io.torvox.MainActivity
import io.torvox.analyzeNonBlackRatio
import io.torvox.decodeRgbaToBitmap
import io.torvox.getBridge
import io.torvox.waitForSession
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.Rule
import org.junit.Test
import java.io.File

class ScreenshotVerificationTest {
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun captureGpuFrame(dataDir: String): File {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.saveTestFrame(dataDir)
        val frameFile = File(dataDir, "test_frame.rgba")
        assertTrue("test_frame.rgba should exist", frameFile.exists())
        assertTrue(
            "test_frame.rgba size should be > 1000 bytes, got ${frameFile.length()}",
            frameFile.length() > 1000,
        )
        return frameFile
    }

    @Test
    fun verify_01_terminal_renders_text() {
        composeTestRule.waitForSession()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        val frameFile = captureGpuFrame(dataDir)
        val bitmap = decodeRgbaToBitmap(frameFile)
        val ratio = analyzeNonBlackRatio(bitmap)
        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        val pngFile = File(screenshotDir, "01-terminal-raw-frame.png")
        pngFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()
        assertTrue(
            "Non-black pixel ratio $ratio is <= 0.05 — terminal likely not rendering",
            ratio > 0.05,
        )
    }

    @Test
    fun verify_02_modifier_bar_exists() {
        composeTestRule.onNodeWithTag("Key_DRAWER").assertExists()
        composeTestRule.onNodeWithTag("Key_ESC").assertExists()
        composeTestRule.onNodeWithTag("Key_TAB").assertExists()
        composeTestRule.onNodeWithTag("Key_CTRL").assertExists()
        composeTestRule.onNodeWithTag("Key_ALT").assertExists()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "02-modifier-bar.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_03_drawer_opens() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        composeTestRule.onNodeWithTag("SettingsButton").assertExists()
        composeTestRule.onNodeWithTag("SearchButton").assertExists()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "03-drawer-open.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_04_settings_font_info() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(300)
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        composeTestRule.onNodeWithTag("FontFamilySelector").assertIsDisplayed()
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "04-settings-font-info.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_05_settings_bootstrap() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(300)
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        composeTestRule.onNodeWithTag("SettingsScreen").assertIsDisplayed()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "05-settings-bootstrap-section.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_06_terminal_ui_elements() {
        composeTestRule.onNodeWithTag("TerminalScreen").assertExists()
        composeTestRule.onNodeWithTag("Key_ESC").assertExists()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "06-terminal-text-content.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_07_drawer_sessions_list() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "07-drawer-sessions.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_08_keyboard_appears() {
        composeTestRule.onNodeWithTag("TerminalScreen").performClick()
        Thread.sleep(3000)
        composeTestRule.waitForIdle()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "08-keyboard-modifier-bar.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_09_search_then_back() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(300)
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(300)
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("test")
        composeTestRule.waitForIdle()
        Thread.sleep(300)
        composeTestRule.onNodeWithTag("SearchClose").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(1000)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "09-search-then-back.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_11_session_creation_timing() {
        val startNanos = System.nanoTime()
        composeTestRule.waitForSession()
        val elapsedMs = (System.nanoTime() - startNanos) / 1_000_000
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        dir.resolve("11-timing.txt").writeText("session_creation_ms=$elapsedMs")
        assertTrue("Session creation took ${elapsedMs}ms, expected < 15000ms", elapsedMs < 15_000)
    }

    @Test
    fun verify_12_modifier_keys_functional() {
        composeTestRule.onNodeWithTag("Key_CTRL").assertExists()
        composeTestRule.onNodeWithTag("Key_ALT").assertExists()
        for (i in 1..10) composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        for (i in 1..10) composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val bitmap =
                Bitmap.createBitmap(
                    activity.window.decorView.width,
                    activity.window.decorView.height,
                    Bitmap.Config.ARGB_8888,
                )
            val canvas = android.graphics.Canvas(bitmap)
            activity.window.decorView.draw(canvas)
            dir.resolve("12-modifier-bar.png").outputStream().use {
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)
            }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_13_search_highlights_gpu() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'x'\n".toByteArray())
        Thread.sleep(2000)
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchButton").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("SearchTextField").performTextInput("x")
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        val frameFile = captureGpuFrame(dataDir)
        val bitmap = decodeRgbaToBitmap(frameFile)
        val ratio = analyzeNonBlackRatio(bitmap)
        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        val pngFile = File(screenshotDir, "13-search-gpu.png")
        pngFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()
        assertTrue("Non-black ratio $ratio <= 0.05", ratio > 0.05)
    }

    @Test
    fun verify_14_selection_gpu() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo 'hello world'\n".toByteArray())
        Thread.sleep(2000)
        bridge.setSelection(0u, 0u, 0u, 5u, active = true)
        bridge.render()
        Thread.sleep(500)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        val frameFile = captureGpuFrame(dataDir)
        val bitmap = decodeRgbaToBitmap(frameFile)
        val ratio = analyzeNonBlackRatio(bitmap)
        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        val pngFile = File(screenshotDir, "14-selection-gpu.png")
        pngFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()
        assertTrue("Non-black ratio $ratio <= 0.05", ratio > 0.05)
    }

    @Test
    fun verify_15_keyboard_jelly_burst() {
        composeTestRule.waitForSession()
        composeTestRule.onNodeWithTag("TerminalScreen").performClick()
        Thread.sleep(1500)
        composeTestRule.waitForIdle()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        for (i in 0 until 5) {
            val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
            bridge.saveTestFrame(dataDir)
            val frameFile = File(dataDir, "test_frame.rgba")
            assertTrue("Frame $i: test_frame.rgba should exist", frameFile.exists())
            assertTrue(
                "Frame $i: file size too small: ${frameFile.length()}",
                frameFile.length() > 1000,
            )
            val bitmap = decodeRgbaToBitmap(frameFile)
            val ratio = analyzeNonBlackRatio(bitmap)
            val pngFile = File(screenshotDir, "15-burst-frame-$i.png")
            pngFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
            assertTrue("Frame $i: non-black ratio $ratio <= 0.05", ratio > 0.05)
            if (i < 4) Thread.sleep(100)
        }
    }

    @Test
    fun verify_17_font_switch_before_after() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        val beforeFile = captureGpuFrame(dataDir)
        val beforeBitmap = decodeRgbaToBitmap(beforeFile)
        val beforeRatio = analyzeNonBlackRatio(beforeBitmap)
        val beforePng = File(screenshotDir, "17-font-before.png")
        beforePng.outputStream().use { beforeBitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        beforeBitmap.recycle()
        bridge.setFontFamily("Noto Sans Mono")
        bridge.render()
        Thread.sleep(2000)
        bridge.writeToPty("echo 'after font change'\n".toByteArray())
        Thread.sleep(2000)
        val afterFile = captureGpuFrame(dataDir)
        val afterBitmap = decodeRgbaToBitmap(afterFile)
        val afterRatio = analyzeNonBlackRatio(afterBitmap)
        val afterPng = File(screenshotDir, "17-font-after.png")
        afterPng.outputStream().use { afterBitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        afterBitmap.recycle()
        assertTrue("Before: non-black ratio $beforeRatio <= 0.05", beforeRatio > 0.05)
        assertTrue("After: non-black ratio $afterRatio <= 0.05", afterRatio > 0.05)
    }

    @Test
    fun verify_18_cjk_rendering() {
        composeTestRule.waitForSession()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        bridge.writeToPty("echo '你好世界test_cjk_123'\n".toByteArray())
        Thread.sleep(3000)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dataDir = context.filesDir.absolutePath
        val frameFile = captureGpuFrame(dataDir)
        val bitmap = decodeRgbaToBitmap(frameFile)
        val ratio = analyzeNonBlackRatio(bitmap)
        val screenshotDir = File(context.filesDir, "screenshots")
        screenshotDir.mkdirs()
        val pngFile = File(screenshotDir, "18-cjk.png")
        pngFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()
        assertTrue("Non-black ratio $ratio <= 0.05", ratio > 0.05)
    }

    @Test
    fun verify_19_theme_names_visibility() {
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(300)
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        composeTestRule
            .onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("ThemeSelector"))
        composeTestRule.onNodeWithTag("ThemeSelector").assertIsDisplayed()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "19-theme-names.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    @Test
    fun verify_20_default_font_name() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("Bridge is null")
        val name = bridge.getDefaultFontName()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        dir.resolve("20-font-name.txt").writeText("default_font=$name")
        assertTrue("Default font name is empty", name.isNotEmpty())
    }

    @Test
    fun verify_21_restore_sessions_off_by_default() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val prefs = context.getSharedPreferences(context.packageName + "_preferences", Context.MODE_PRIVATE)
        val restoreEnabled = prefs.getBoolean("restore_sessions", false)
        val dir = File(context.filesDir, "screenshots")
        dir.mkdirs()
        dir.resolve("21-restore-sessions.txt").writeText("restore_sessions_enabled=$restoreEnabled")
        assertTrue("Restore sessions should be OFF by default, was enabled", !restoreEnabled)
    }

    @Test
    fun verify_22_full_ui_elements() {
        composeTestRule.onNodeWithTag("TerminalScreen").assertExists()
        composeTestRule.onNodeWithTag("Key_DRAWER").assertExists()
        composeTestRule.onNodeWithTag("Key_ESC").assertExists()
        composeTestRule.onNodeWithTag("Key_TAB").assertExists()
        composeTestRule.onNodeWithTag("Key_CTRL").assertExists()
        composeTestRule.onNodeWithTag("Key_ALT").assertExists()
        composeTestRule.waitForIdle()
        Thread.sleep(500)
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val dir = File(context.filesDir, "screenshots")
            dir.mkdirs()
            val rootView = activity.window.decorView
            val bitmap = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            rootView.draw(canvas)
            val file = File(dir, "22-full-ui.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }

    companion object {
        @JvmStatic @BeforeClass
        fun setupClass() {
            System.setProperty("torvox.test.minSurface", "true")
            try {
                Runtime.getRuntime().exec(arrayOf("sh", "-c", "wm size reset"))
            } catch (e: Exception) {
                // best-effort, non-asserting: resetting the emulator display size is a
                // pre-test environment normalization and must not mask real failures.
                Log.e("ScreenshotVerificationTest", "wm size reset failed (best-effort)", e)
            }
        }
    }
}
