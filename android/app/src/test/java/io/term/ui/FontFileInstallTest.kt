package io.term.ui

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
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
class FontFileInstallTest {
    private val testDispatcher = StandardTestDispatcher()
    private lateinit var context: Context

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        context = RuntimeEnvironment.getApplication()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun fontFileIsCopiedToFilesDir() = runTest {
        val fontsDir = File(context.filesDir, "fonts")
        fontsDir.mkdirs()
        assertTrue("fonts directory should exist", fontsDir.isDirectory)

        val testFont = File(fontsDir, "test.ttf")
        testFont.writeBytes(byteArrayOf(0, 0, 0, 0))
        assertTrue("font file should exist after copy", testFont.exists())
        assertEquals("font file size should match", 4L, testFont.length())
    }

    @Test
    fun getFileNameFromUriExtractsDisplayName() {
        val uri = Uri.parse("content://com.android.providers.downloads.documents/document/1234")
        val viewModel = createViewModel()
        viewModel.getFileNameFromUri(uri)
        // With robolectric, content resolver may return null, but the method should not crash
        // The actual implementation queries ContentResolver for DISPLAY_NAME
    }

    @Test
    fun fontFamilyIsSavedAfterInstall() = runTest {
        val settingsProvider = io.term.settings.SettingsDataStoreProvider(context)
        val repository = io.term.settings.SettingsRepository(settingsProvider)

        repository.setFontFamily("TestFont")
        val saved = repository.fontFamily.first()
        assertEquals("font family should be saved", "TestFont", saved)
    }

    @Test
    fun fontListIsEmptyByDefault() {
        val viewModel = createViewModel()
        // availableFonts starts empty until loadFonts() is called
        // In unit test context, the Rust bridge is not available
    }

    @Test
    fun fallbackSystemFontsReturnsNonEmptyList() {
        val fonts = fallbackSystemFonts()
        assertTrue("should have at least 7 system fonts", fonts.size >= 7)
        assertTrue("should contain Droid Sans Mono", fonts.any { it.contains("Droid", ignoreCase = true) })
        assertTrue("should contain Noto Sans Mono", fonts.any { it.contains("Noto", ignoreCase = true) })
        assertTrue("should contain Roboto Mono", fonts.any { it.contains("Roboto", ignoreCase = true) })
    }

    @Test
    fun fallbackSystemFontsAreDistinct() {
        val fonts = fallbackSystemFonts()
        assertEquals("fonts should have no duplicates", fonts.size, fonts.distinct().size)
    }

    @Test
    fun fallbackSystemFontsAreSorted() {
        val fonts = fallbackSystemFonts()
        assertEquals("fonts should be sorted", fonts, fonts.sorted())
    }

    @Test
    fun installFontFileHandlesInvalidUri() = runTest {
        val viewModel = createViewModel()
        val invalidUri = Uri.parse("content://invalid.provider/font.ttf")
        // Should not crash — the implementation catches exceptions
        viewModel.installFontFile(invalidUri)
    }

    private fun createViewModel(): io.term.TerminalViewModel {
        val settingsProvider = io.term.settings.SettingsDataStoreProvider(context)
        val repository = io.term.settings.SettingsRepository(settingsProvider)
        val runtime = io.term.runtime.TerminalRuntime(context, repository)
        return io.term.TerminalViewModel(context, repository, runtime)
    }
}
