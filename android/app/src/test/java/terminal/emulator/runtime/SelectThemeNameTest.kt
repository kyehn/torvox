package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Test
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.AppThemeMode
import terminal.emulator.ui.theme.TerminalThemeMode
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName

/**
 * `TerminalRuntime.resolveThemeName` 的真实选择路径：把设置快照逐字段喂给
 * [resolveTerminalThemeName] 与 [resolveAppDarkMode]，断日夜两组的选取，以及
 * 「跟随系统」时由软件主题（或系统标志）决定的二级裁决。
 */
class SelectThemeNameTest {

    private fun stored(
        themeMode: String = TerminalThemeMode.DAY_NIGHT,
        appThemeMode: String = AppThemeMode.FOLLOW_SYSTEM,
    ) = SettingsRepository.SettingsState(
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
        assertEquals("fixed-theme", selectThemeName(stored(themeMode = TerminalThemeMode.FIXED), systemDark = true))
        assertEquals("fixed-theme", selectThemeName(stored(themeMode = TerminalThemeMode.FIXED), systemDark = false))
    }

    @Test
    fun `day night mode tracks the software theme`() {
        assertEquals("night-theme", selectThemeName(stored(), systemDark = true))
        assertEquals("day-theme", selectThemeName(stored(), systemDark = false))
    }

    @Test
    fun `explicit app mode overrides the system flag`() {
        assertEquals("day-theme", selectThemeName(stored(appThemeMode = AppThemeMode.DAY), systemDark = true))
        assertEquals("night-theme", selectThemeName(stored(appThemeMode = AppThemeMode.NIGHT), systemDark = false))
    }

    @Test
    fun `legacy terminal modes normalize to day night`() {
        // 旧版四态落盘值：day/night/follow_system 在 UI 上都等价于开关开启。
        for (legacy in listOf("day", "night", "follow_system")) {
            assertEquals(
                "legacy mode $legacy must keep the day/night switch on",
                TerminalThemeMode.DAY_NIGHT,
                TerminalThemeMode.normalize(legacy),
            )
            assertEquals("night-theme", selectThemeName(stored(themeMode = legacy), systemDark = true))
        }
        assertEquals(TerminalThemeMode.FIXED, TerminalThemeMode.normalize(TerminalThemeMode.FIXED))
    }
}
