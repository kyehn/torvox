package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test
import terminal.emulator.settings.SettingsRepository

/**
 * Pure theme-name selection behind `TerminalRuntime.resolveThemeName`:
 * fixed modes take their key directly, follow-system resolves day/night
 * from the app mode or the system flag.
 */
class SelectThemeNameTest {

    private fun stored(
        themeMode: String = "follow",
        appThemeMode: String = "follow",
    ) = SettingsRepository.SettingsState(
        fontSize = 14f,
        themeName = "fixed-theme",
        dayThemeName = "day-theme",
        nightThemeName = "night-theme",
        themeMode = themeMode,
        appThemeMode = appThemeMode,
    )

    @Test
    fun `fixed mode returns the fixed name`() {
        assertEquals("fixed-theme", selectThemeName(stored(themeMode = "fixed"), systemDark = true))
    }

    @Test
    fun `day mode returns the day name`() {
        assertEquals("day-theme", selectThemeName(stored(themeMode = "day"), systemDark = true))
    }

    @Test
    fun `night mode returns the night name`() {
        assertEquals("night-theme", selectThemeName(stored(themeMode = "night"), systemDark = false))
    }

    @Test
    fun `follow mode tracks the system flag`() {
        assertEquals("night-theme", selectThemeName(stored(), systemDark = true))
        assertEquals("day-theme", selectThemeName(stored(), systemDark = false))
    }

    @Test
    fun `explicit app mode overrides the system flag`() {
        assertEquals("day-theme", selectThemeName(stored(appThemeMode = "day"), systemDark = true))
        assertEquals("night-theme", selectThemeName(stored(appThemeMode = "night"), systemDark = false))
    }
}
