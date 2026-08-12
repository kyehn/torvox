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

    @Test
    fun `shortcut binding round-trips via string keys`() = runTest {
        repository.setShortcutBinding("paste", "CTRL|SHIFT|54")
        repository.setShortcutBinding("new_session", "CTRL|SHIFT|36")
        // Read back via settings snapshot
        repository.settings.test {
            val state = awaitItem()
            assertEquals("CTRL|SHIFT|54", state.shortcutPaste)
            assertEquals("CTRL|SHIFT|36", state.shortcutNewSession)
            assertEquals("", state.shortcutCopy) // default
        }
    }

    @Test
    fun `clear shortcut binding reverts to default`() = runTest {
        repository.setShortcutBinding("paste", "CTRL|SHIFT|54")
        repository.clearShortcutBinding("paste")
        repository.settings.test {
            val state = awaitItem()
            assertEquals("", state.shortcutPaste)
        }
    }

    @Test
    fun `unknown action id in setShortcutBinding is ignored`() = runTest {
        repository.setShortcutBinding("nonexistent", "CTRL|54")
        repository.settings.test {
            val state = awaitItem()
            // All shortcut fields should remain at default
            assertEquals("", state.shortcutPaste)
        }
    }

    @Test
    fun `environment variables default to empty`() = runTest {
        repository.environmentVariables.test {
            assertEquals(emptyMap<String, String>(), awaitItem())
        }
    }

    @Test
    fun `environment variables round-trip via repository`() = runTest {
        repository.setEnvironmentVariables(
            mapOf("EDITOR" to "vim", "MY_FLAG" to "1"),
        )
        repository.environmentVariables.test {
            assertEquals(
                mapOf("EDITOR" to "vim", "MY_FLAG" to "1"),
                awaitItem(),
            )
        }
        repository.settings.test {
            val state = awaitItem()
            assertEquals(mapOf("EDITOR" to "vim", "MY_FLAG" to "1"), state.environmentVariables)
        }
    }

    @Test
    fun `environment variables empty map clears persisted value`() = runTest {
        repository.setEnvironmentVariables(mapOf("KEEP" to "1"))
        repository.setEnvironmentVariables(emptyMap())
        repository.environmentVariables.test {
            assertEquals(emptyMap<String, String>(), awaitItem())
        }
    }

    @Test
    fun `parse environment variables skips malformed lines and keeps values with equals`() {
        val parsed =
            parseEnvironmentVariables(
                "\n\nNO_EQUALS\n=emptykey\n  =blank\nURL=https://x.test/a=b\n  PADDED = value  \n",
            )
        // key trimmed, value untouched (leading space after '=' preserved)
        assertEquals(
            mapOf("URL" to "https://x.test/a=b", "PADDED" to " value  "),
            parsed,
        )
    }

    @Test
    fun `serialize environment variables sorts keys and joins with newline`() {
        assertEquals(
            "A=one\nB=two",
            serializeEnvironmentVariables(mapOf("A" to "one", "B" to "two")),
        )
        assertEquals("", serializeEnvironmentVariables(emptyMap()))
    }

    @Test
    fun `parse serialize round-trips`() {
        val original = mapOf("EDITOR" to "vim", "URL" to "https://x.test/a=b")
        assertEquals(original, parseEnvironmentVariables(serializeEnvironmentVariables(original)))
    }
}
