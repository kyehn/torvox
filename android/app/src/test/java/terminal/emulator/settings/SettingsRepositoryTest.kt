package terminal.emulator.settings

import android.content.Context
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.test.core.app.ApplicationProvider
import app.cash.turbine.test
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * DataStore-backed settings: round-trip writes and defaults.
 *
 * The DataStore itself is real (over a per-test temp file); only the
 * Hilt-injected [SettingsDataStoreProvider] is replaced by a MockK stand-in
 * that exposes that store, so tests never touch the app's real prefs.
 */
@RunWith(RobolectricTestRunner::class)
class SettingsRepositoryTest {
    private lateinit var prefsDir: File
    private lateinit var repository: SettingsRepository

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        prefsDir = File(context.cacheDir, "settings_test_${System.nanoTime()}")
        prefsDir.mkdirs()
        val dataStore: androidx.datastore.core.DataStore<Preferences> =
            PreferenceDataStoreFactory.create {
                File(prefsDir, "settings.preferences_pb")
            }
        val provider = mockk<SettingsDataStoreProvider>()
        every { provider.dataStore } returns dataStore
        repository = SettingsRepository(provider)
    }

    @After
    fun tearDown() {
        prefsDir.deleteRecursively()
    }

    @Test
    fun `font size defaults then round-trips`() = runTest {
        repository.fontSize.test {
            assertEquals(SettingsRepository.DEFAULT_FONT_SIZE, awaitItem())
            repository.setFontSize(22f)
            assertEquals(22f, awaitItem())
        }
    }

    @Test
    fun `string setting round-trips`() = runTest {
        repository.appThemeMode.test {
            assertEquals(SettingsRepository.DEFAULT_FOLLOW_SYSTEM, awaitItem())
            repository.setAppThemeMode("dark")
            assertEquals("dark", awaitItem())
        }
    }

    @Test
    fun `boolean setting round-trips`() = runTest {
        repository.mcpServerEnabled.test {
            assertEquals(false, awaitItem())
            repository.setMcpServerEnabled(true)
            assertEquals(true, awaitItem())
        }
    }

    @Test
    fun `int setting round-trips`() = runTest {
        repository.scrollbackLines.test {
            assertEquals(SettingsRepository.DEFAULT_SCROLLBACK_LINES, awaitItem())
            repository.setScrollbackLines(10_000)
            assertEquals(10_000, awaitItem())
        }
    }

    @Test
    fun `distinct keys do not clobber each other`() = runTest {
        repository.setFontSize(20f)
        repository.setThemeName("Solarized Dark")
        repository.fontSize.test {
            assertEquals(20f, awaitItem())
        }
        repository.themeName.test {
            assertEquals("Solarized Dark", awaitItem())
        }
    }
}
