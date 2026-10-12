package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.ThemeMode
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName

/**
 * `TerminalRuntime.resolveThemeName` 的真实选择路径：把设置快照逐字段喂给
 * [resolveTerminalThemeName] 与 [resolveAppDarkMode]，断日夜两组的选取，以及
 * 「跟随系统」时由软件主题（或系统标志）决定的二级裁决。
 */
class SelectThemeNameTest {

    private fun stored(themeMode: String = ThemeMode.FOLLOW_SYSTEM, appThemeMode: String = ThemeMode.FOLLOW_SYSTEM) =
        SettingsRepository.SettingsState(
            fontSize = 14f,
            themeName = "fixed-theme",
            dayThemeName = "day-theme",
            nightThemeName = "night-theme",
            themeMode = themeMode,
            appThemeMode = appThemeMode,
        )

    /** 与 [TerminalRuntime.resolveThemeName] 同形的取值。 */
    private fun selectThemeName(stored: SettingsRepository.SettingsState, systemDark: Boolean): String =
        resolveTerminalThemeName(
            mode = stored.themeMode,
            fixedName = stored.themeName,
            dayName = stored.dayThemeName,
            nightName = stored.nightThemeName,
            isDark = resolveAppDarkMode(stored.appThemeMode, systemDark),
        )

    @Test
    fun `fixed mode returns the fixed name`() {
        assertEquals("fixed-theme", selectThemeName(stored(themeMode = ThemeMode.FIXED), systemDark = true))
    }

    @Test
    fun `day mode returns the day name`() {
        assertEquals("day-theme", selectThemeName(stored(themeMode = ThemeMode.DAY), systemDark = true))
    }

    @Test
    fun `night mode returns the night name`() {
        assertEquals("night-theme", selectThemeName(stored(themeMode = ThemeMode.NIGHT), systemDark = false))
    }

    @Test
    fun `follow mode tracks the system flag`() {
        assertEquals("night-theme", selectThemeName(stored(), systemDark = true))
        assertEquals("day-theme", selectThemeName(stored(), systemDark = false))
    }

    @Test
    fun `explicit app mode overrides the system flag`() {
        assertEquals("day-theme", selectThemeName(stored(appThemeMode = ThemeMode.DAY), systemDark = true))
        assertEquals("night-theme", selectThemeName(stored(appThemeMode = ThemeMode.NIGHT), systemDark = false))
    }
}
