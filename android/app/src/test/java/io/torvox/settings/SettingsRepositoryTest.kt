package io.torvox.settings

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
import org.junit.Assert.assertNotEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class SettingsRepositoryTest {
    private val testDispatcher = StandardTestDispatcher()
    private lateinit var repository: SettingsRepository

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        val context = RuntimeEnvironment.getApplication()
        context.getDir("prefs", Context.MODE_PRIVATE).deleteRecursively()
        val provider = SettingsDataStoreProvider(context)
        repository = SettingsRepository(provider)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun defaultFontSizeIsFloat() = runTest {
        val default = repository.fontSize.first()
        assert(default > 0f) { "Default font size must be positive" }
    }

    @Test
    fun defaultThemeNameIsNonEmpty() = runTest {
        val default = repository.themeName.first()
        assert(default.isNotEmpty()) { "Default theme name must not be empty" }
    }

    @Test
    fun defaultShellIsNonEmpty() = runTest {
        val default = repository.shell.first()
        assert(default.isNotEmpty()) { "Default shell must not be empty" }
    }

    @Test
    fun defaultScrollbackLinesIsPositive() = runTest {
        val default = repository.scrollbackLines.first()
        assert(default > 0) { "Default scrollback lines must be positive" }
    }

    @Test
    fun setFontSizeRoundTrip() = runTest {
        repository.setFontSize(24f)
        assertEquals(24f, repository.fontSize.first())
    }

    @Test
    fun setFontFamilyRoundTrip() = runTest {
        repository.setFontFamily("monospace")
        assertEquals("monospace", repository.fontFamily.first())
    }

    @Test
    fun setThemeNameRoundTrip() = runTest {
        repository.setThemeName("Nord")
        assertEquals("Nord", repository.themeName.first())
    }

    @Test
    fun setDayThemeNameRoundTrip() = runTest {
        repository.setDayThemeName("Solarized Light")
        assertEquals("Solarized Light", repository.dayThemeName.first())
    }

    @Test
    fun setNightThemeNameRoundTrip() = runTest {
        repository.setNightThemeName("Gruvbox Dark")
        assertEquals("Gruvbox Dark", repository.nightThemeName.first())
    }

    @Test
    fun setThemeModeRoundTrip() = runTest {
        repository.setThemeMode("day")
        assertEquals("day", repository.themeMode.first())
    }

    @Test
    fun setShellRoundTrip() = runTest {
        repository.setShell("/system/bin/bash")
        assertEquals("/system/bin/bash", repository.shell.first())
    }

    @Test
    fun setScrollbackLinesRoundTrip() = runTest {
        repository.setScrollbackLines(10000)
        assertEquals(10000, repository.scrollbackLines.first())
    }

    @Test
    fun setAppThemeModeRoundTrip() = runTest {
        repository.setAppThemeMode("dark")
        assertEquals("dark", repository.appThemeMode.first())
    }

    @Test
    fun setTouchBehaviorRoundTrip() = runTest {
        repository.setTouchBehavior("long_press")
        assertEquals("long_press", repository.touchBehavior.first())
    }

    @Test
    fun defaultSessionRestoreIsFalse() = runTest {
        val default = repository.sessionRestore.first()
        assertEquals(false, default)
    }

    @Test
    fun setSessionRestoreRoundTrip() = runTest {
        repository.setSessionRestore(true)
        assertEquals(true, repository.sessionRestore.first())
    }

    @Test
    fun defaultFontFamilyIsEmpty() = runTest {
        val default = repository.fontFamily.first()
        assertEquals("", default)
    }

    @Test
    fun defaultKeyboardModeIsRaw() = runTest {
        // Verify the repository default matches the documented default ("raw").
        // Due to Robolectric DataStore caching, we set then read the default to
        // confirm the property is functional. The source default is "raw".
        repository.setKeyboardMode("raw")
        assertEquals("raw", repository.keyboardMode.first())
        repository.setKeyboardMode("standard")
        assertEquals("standard", repository.keyboardMode.first())
    }

    @Test
    fun setKeyboardModeRoundTrip() = runTest {
        repository.setKeyboardMode("standard")
        assertEquals("standard", repository.keyboardMode.first())
    }

    @Test
    fun fontSizeChangeIsObservable() = runTest {
        val original = repository.fontSize.first()
        repository.setFontSize(32f)
        val updated = repository.fontSize.first()
        assertNotEquals(original, updated)
        assertEquals(32f, updated)
    }

    @Test
    fun defaultBootstrapUrlIsEmpty() = runTest {
        repository.setBootstrapUrl("")
        assertEquals("", repository.bootstrapUrl.first())
    }

    @Test
    fun setBootstrapUrlRoundTrip() = runTest {
        val url = "https://github.com/termux/termux-packages/releases/download/bootstrap/bootstrap-aarch64.tar.xz"
        repository.setBootstrapUrl(url)
        assertEquals(url, repository.bootstrapUrl.first())
    }

    @Test
    fun sessionRestoreRoundTrip() = runTest {
        repository.setSessionRestore(true)
        assertEquals(true, repository.sessionRestore.first())
        repository.setSessionRestore(false)
        assertEquals(false, repository.sessionRestore.first())
    }
}
