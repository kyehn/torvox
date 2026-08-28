package terminal.emulator.settings

import android.content.Context
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.test.core.app.ApplicationProvider
import app.cash.turbine.test
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.first
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
 * The DataStore itself is real (over a per-test temp file); only the Hilt-injected
 * [SettingsDataStoreProvider] is replaced by a MockK stand-in that exposes that store, so tests
 * never touch the app's real prefs.
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
        every { provider.screenWidthDp } returns 360f
        repository = SettingsRepository(provider)
    }

    @After
    fun tearDown() {
        prefsDir.deleteRecursively()
    }

    @Test
    fun `font size defaults to device-adaptive value then round-trips`() = runTest {
        repository.fontSize.test {
            // Screen stubbed at 360dp: raw formula 11.54sp lifts to the
            //  14sp floor (user-reported "default too small" fix;
            // floor raised from the  12sp).
            assertEquals(14f, awaitItem(), 0.01f)
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
    fun `bootstrap url round-trips`() = runTest {
        repository.bootstrapUrl.test {
            assertEquals("", awaitItem())
            repository.setBootstrapUrl("https://packages.termux.dev/apt/termux-main/bootstrap.zip")
            assertEquals("https://packages.termux.dev/apt/termux-main/bootstrap.zip", awaitItem())
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
    fun `first launch default font size adapts to screen width`() {
        // spec default-typography: 14sp floor / 24sp cap
        // (user-reported "default too small"; was 12/18 in ).
        assertEquals(14f, SettingsRepository.defaultFontSizeFor(0f), 0.01f)
        assertEquals(14f, SettingsRepository.defaultFontSizeFor(360f), 0.01f)
        assertEquals(14f, SettingsRepository.defaultFontSizeFor(412f), 0.01f)
        assertEquals(24f, SettingsRepository.defaultFontSizeFor(800f), 0.01f)
        assertEquals(19.23f, SettingsRepository.defaultFontSizeFor(600f), 0.01f)
    }

    @Test
    fun `first launch font size is persisted only while unset`() = runTest {
        repository.applyFirstLaunchDefaultFontSize(360f)
        // 360dp lands on the  14sp floor.
        assertEquals(14f, repository.fontSize.first(), 0.01f)
        // An explicit user pick wins over re-applying the default.
        repository.setFontSize(14f)
        repository.applyFirstLaunchDefaultFontSize(800f)
        assertEquals(14f, repository.fontSize.first(), 0.01f)
    }

    @Test
    fun `parse serialize round-trips`() {
        val original = mapOf("EDITOR" to "vim", "URL" to "https://x.test/a=b")
        assertEquals(original, parseEnvironmentVariables(serializeEnvironmentVariables(original)))
    }
}
