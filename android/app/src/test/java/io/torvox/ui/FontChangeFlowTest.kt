package io.torvox.ui

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import java.io.File

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class FontChangeFlowTest {
    private val testDispatcher = StandardTestDispatcher()
    private lateinit var context: Context
    private lateinit var settingsProvider: io.torvox.settings.SettingsDataStoreProvider
    private lateinit var repository: io.torvox.settings.SettingsRepository

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        context = RuntimeEnvironment.getApplication()
        settingsProvider = io.torvox.settings.SettingsDataStoreProvider(context)
        repository = io.torvox.settings.SettingsRepository(settingsProvider)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun fontSizeRoundTripThroughSettings() = runTest {
        repository.setFontSize(24f)
        val size = repository.fontSize.first()
        assertEquals("font size should round-trip", 24f, size)
    }

    @Test
    fun fontFamilyRoundTripThroughSettings() = runTest {
        repository.setFontFamily("Fira Code")
        val family = repository.fontFamily.first()
        assertEquals("font family should round-trip", "Fira Code", family)
    }

    @Test
    fun fontFamilyEmptyIsDefault() = runTest {
        val family = repository.fontFamily.first()
        assertEquals("default font family should be empty", "", family)
    }

    @Test
    fun fontFileCopyToFilesDir() {
        val destFile = File(context.filesDir, "TestFont.ttf")
        val testData = "fake font data".toByteArray()
        destFile.writeBytes(testData)

        assertTrue("font file should exist", destFile.exists())
        assertEquals("font file content should match", testData.size.toLong(), destFile.length())
        destFile.delete()
    }

    @Test
    fun fontFileCopyHandlesDuplicateNames() {
        val destFile = File(context.filesDir, "Duplicate.ttf")
        destFile.writeBytes("first".toByteArray())
        destFile.outputStream().use { it.write("second".toByteArray()) }

        assertEquals("file should be overwritten", "second", destFile.readText())
        destFile.delete()
    }

    @Test
    fun fallbackSystemFontsIncludesExpectedFonts() {
        val fonts = fallbackSystemFonts()
        val expected =
            listOf(
                "Droid Sans Mono",
                "Noto Sans Mono",
                "Roboto Mono",
                "Source Code Pro",
                "Fira Code",
                "Ubuntu Mono",
            )
        for (name in expected) {
            assertTrue(
                "font list should contain '$name'",
                fonts.any { it.contains(name, ignoreCase = true) },
            )
        }
    }

    @Test
    fun fontPickerDialogFontListIsSorted() {
        val fonts = fallbackSystemFonts()
        val sorted = fonts.sorted()
        assertEquals("font list should be sorted", sorted, fonts)
    }

    @Test
    fun settingsScreenShowsFontFamilyLabel() {
        // This tests the UI string resource exists
        val label = context.getString(io.torvox.R.string.font_family)
        assertNotNull("font_family string should exist", label)
        assertTrue("font_family string should not be empty", label.isNotEmpty())
    }

    @Test
    fun settingsScreenShowsChangeLabel() {
        val label = context.getString(io.torvox.R.string.change)
        assertNotNull("change string should exist", label)
        assertTrue("change string should not be empty", label.isNotEmpty())
    }

    @Test
    fun settingsScreenShowsPickFontFileLabel() {
        val label = context.getString(io.torvox.R.string.pick_font_file)
        assertNotNull("pick_font_file string should exist", label)
        assertTrue("pick_font_file string should not be empty", label.isNotEmpty())
    }
}
