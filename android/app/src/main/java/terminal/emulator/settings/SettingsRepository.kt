package terminal.emulator.settings

import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class SettingsRepository
@Inject
constructor(
    private val provider: SettingsDataStoreProvider,
) {
    private object Keys {
        val FONT_SIZE = floatPreferencesKey("font_size")
        val FONT_FAMILY = stringPreferencesKey("font_family")
        val BOLD_FONT_FAMILY = stringPreferencesKey("bold_font_family")
        val ITALIC_FONT_FAMILY = stringPreferencesKey("italic_font_family")
        val THEME_NAME = stringPreferencesKey("theme_name")
        val DAY_THEME_NAME = stringPreferencesKey("day_theme_name")
        val NIGHT_THEME_NAME = stringPreferencesKey("night_theme_name")
        val THEME_MODE = stringPreferencesKey("theme_mode")
        val SHELL = stringPreferencesKey("shell")
        val SCROLLBACK_LINES = intPreferencesKey("scrollback_lines")
        val APP_THEME_MODE = stringPreferencesKey("app_theme_mode")
        val BOOTSTRAP_URL = stringPreferencesKey("bootstrap_url")
        val USE_NERD_FONT_GLYPHS = booleanPreferencesKey("use_nerd_font_glyphs")
        val KEYBOARD_MODE = stringPreferencesKey("keyboard_mode")
        val SHIZUKU_ENABLED = booleanPreferencesKey("shizuku_enabled")
    }

    companion object {
        const val DEFAULT_FONT_SIZE = 14f
        const val DEFAULT_SCROLLBACK_LINES = 10_000
        private const val DEFAULT_THEME = "Dracula Plus"
        const val DEFAULT_DAY_THEME_NAME = "Catppuccin Latte"
        const val DEFAULT_FOLLOW_SYSTEM = "follow_system"
        const val DEFAULT_THEME_MODE = "fixed"
        const val DEFAULT_KEYBOARD_MODE = "secure"
        const val DEFAULT_SHELL = "/system/bin/sh"

        /**
         * Device-adaptive first-launch font size (sp): a fresh install gets a size that shows roughly
         * [DEFAULT_FONT_COLUMNS_TARGET] visible columns (a monospace glyph is ~0.6em wide: sp = widthDp
         * / (0.6 * target)), clamped to [MIN_FONT_SP, MAX_FONT_SP]. (spec default-typography): the
         * floor is 14sp so small phones can never land below readable. Calibrated against real termux
         * 0.118.3 on the same emulator (1080x2400@420dpi, ): termux glyph band 27px / char pitch
         * ~21.2px / ~51 cols vs ours 27px / 21.8px / ~49 cols — within the C_ref ±10% tolerance, no
         * further change needed.
         */
        fun defaultFontSizeFor(screenWidthDp: Float): Float = (screenWidthDp / DEFAULT_FONT_COLUMNS_TARGET / MONOSPACE_CHAR_ASPECT).coerceIn(
            MIN_FONT_SP,
            MAX_FONT_SP,
        )

        private const val DEFAULT_FONT_COLUMNS_TARGET = 52f
        private const val MONOSPACE_CHAR_ASPECT = 0.6f

        /** termux default_font_size parity: never launch below 14sp. */
        const val MIN_FONT_SP = 14f
        private const val MAX_FONT_SP = 24f
    }

    val appThemeMode: Flow<String> =
        provider.dataStore.data.map { it[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM }
    private val deviceDefaultFontSize: Float
        get() = defaultFontSizeFor(provider.screenWidthDp)

    val fontSize: Flow<Float> =
        provider.dataStore.data.map { it[Keys.FONT_SIZE] ?: deviceDefaultFontSize }

    /** True once the user has explicitly picked a font size; false on a fresh install. */
    val fontSizeExplicitlySet: Flow<Boolean> =
        provider.dataStore.data.map { it[Keys.FONT_SIZE] != null }
    val fontFamily: Flow<String> = provider.dataStore.data.map { it[Keys.FONT_FAMILY] ?: "" }
    val boldFontFamily: Flow<String> = provider.dataStore.data.map { it[Keys.BOLD_FONT_FAMILY] ?: "" }
    val italicFontFamily: Flow<String> =
        provider.dataStore.data.map { it[Keys.ITALIC_FONT_FAMILY] ?: "" }
    val themeName: Flow<String> = provider.dataStore.data.map { it[Keys.THEME_NAME] ?: DEFAULT_THEME }
    val dayThemeName: Flow<String> =
        provider.dataStore.data.map { it[Keys.DAY_THEME_NAME] ?: DEFAULT_DAY_THEME_NAME }
    val nightThemeName: Flow<String> =
        provider.dataStore.data.map { it[Keys.NIGHT_THEME_NAME] ?: DEFAULT_THEME }
    val themeMode: Flow<String> =
        provider.dataStore.data.map { it[Keys.THEME_MODE] ?: DEFAULT_THEME_MODE }
    val shell: Flow<String> = provider.dataStore.data.map { it[Keys.SHELL] ?: DEFAULT_SHELL }
    val scrollbackLines: Flow<Int> =
        provider.dataStore.data.map { it[Keys.SCROLLBACK_LINES] ?: DEFAULT_SCROLLBACK_LINES }
    val bootstrapUrl: Flow<String> = provider.dataStore.data.map { it[Keys.BOOTSTRAP_URL] ?: "" }
    val useNerdFontGlyphs: Flow<Boolean> =
        provider.dataStore.data.map { it[Keys.USE_NERD_FONT_GLYPHS] ?: false }
    val keyboardMode: Flow<String> =
        provider.dataStore.data.map { it[Keys.KEYBOARD_MODE] ?: DEFAULT_KEYBOARD_MODE }
    val shizukuEnabled: Flow<Boolean> =
        provider.dataStore.data.map { it[Keys.SHIZUKU_ENABLED] ?: false }

    /**
     * Single merged snapshot of every persisted setting, derived from one DataStore read. UI
     * subscribes to this one flow instead of 16 parallel per-field pipelines (C7). Field defaults
     * mirror the per-field flows above; keep both in sync when adding a setting.
     */
    data class SettingsState(
        val appThemeMode: String = DEFAULT_FOLLOW_SYSTEM,
        val fontSize: Float = DEFAULT_FONT_SIZE,
        val fontFamily: String = "",
        val boldFontFamily: String = "",
        val italicFontFamily: String = "",
        val themeName: String = DEFAULT_THEME,
        val dayThemeName: String = DEFAULT_DAY_THEME_NAME,
        val nightThemeName: String = DEFAULT_THEME,
        val themeMode: String = DEFAULT_THEME_MODE,
        val shell: String = DEFAULT_SHELL,
        val scrollbackLines: Int = DEFAULT_SCROLLBACK_LINES,
        val bootstrapUrl: String = "",
        val useNerdFontGlyphs: Boolean = false,
        val keyboardMode: String = DEFAULT_KEYBOARD_MODE,
        val shizukuEnabled: Boolean = false,
    )

    val settings: Flow<SettingsState> =
        provider.dataStore.data.map { prefs ->
            SettingsState(
                appThemeMode = prefs[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM,
                fontSize = prefs[Keys.FONT_SIZE] ?: deviceDefaultFontSize,
                fontFamily = prefs[Keys.FONT_FAMILY] ?: "",
                boldFontFamily = prefs[Keys.BOLD_FONT_FAMILY] ?: "",
                italicFontFamily = prefs[Keys.ITALIC_FONT_FAMILY] ?: "",
                themeName = prefs[Keys.THEME_NAME] ?: DEFAULT_THEME,
                dayThemeName = prefs[Keys.DAY_THEME_NAME] ?: DEFAULT_DAY_THEME_NAME,
                nightThemeName = prefs[Keys.NIGHT_THEME_NAME] ?: DEFAULT_THEME,
                themeMode = prefs[Keys.THEME_MODE] ?: DEFAULT_THEME_MODE,
                shell = prefs[Keys.SHELL] ?: DEFAULT_SHELL,
                scrollbackLines = prefs[Keys.SCROLLBACK_LINES] ?: DEFAULT_SCROLLBACK_LINES,
                bootstrapUrl = prefs[Keys.BOOTSTRAP_URL] ?: "",
                useNerdFontGlyphs = prefs[Keys.USE_NERD_FONT_GLYPHS] ?: false,
                keyboardMode = prefs[Keys.KEYBOARD_MODE] ?: DEFAULT_KEYBOARD_MODE,
                shizukuEnabled = prefs[Keys.SHIZUKU_ENABLED] ?: false,
            )
        }

    suspend fun setFontSize(size: Float) = put(Keys.FONT_SIZE, size)

    /**
     * Persist the device-adaptive default font size on first launch so a fresh install renders a
     * legible grid before the user touches the font-size slider. No-op once the user has explicitly
     * picked a size.
     */
    suspend fun applyFirstLaunchDefaultFontSize(screenWidthDp: Float) {
        // Skip the write transaction entirely once the user has picked a size.
        if (fontSizeExplicitlySet.first()) return
        provider.dataStore.edit { prefs ->
            if (prefs[Keys.FONT_SIZE] == null) {
                prefs[Keys.FONT_SIZE] = defaultFontSizeFor(screenWidthDp.coerceAtLeast(0f))
            }
        }
    }

    suspend fun setFontFamily(family: String) = put(Keys.FONT_FAMILY, family)

    suspend fun setBoldFontFamily(family: String) = put(Keys.BOLD_FONT_FAMILY, family)

    suspend fun setItalicFontFamily(family: String) = put(Keys.ITALIC_FONT_FAMILY, family)

    suspend fun setThemeName(name: String) = put(Keys.THEME_NAME, name)

    suspend fun setDayThemeName(name: String) = put(Keys.DAY_THEME_NAME, name)

    suspend fun setNightThemeName(name: String) = put(Keys.NIGHT_THEME_NAME, name)

    suspend fun setThemeMode(mode: String) = put(Keys.THEME_MODE, mode)

    suspend fun setAppThemeMode(mode: String) = put(Keys.APP_THEME_MODE, mode)

    suspend fun setShell(shell: String) = put(Keys.SHELL, shell)

    suspend fun setScrollbackLines(lines: Int) = put(Keys.SCROLLBACK_LINES, lines)

    suspend fun setBootstrapUrl(url: String) = put(Keys.BOOTSTRAP_URL, url)

    suspend fun setUseNerdFontGlyphs(enabled: Boolean) = put(Keys.USE_NERD_FONT_GLYPHS, enabled)

    suspend fun setKeyboardMode(mode: String) = put(Keys.KEYBOARD_MODE, mode)

    suspend fun setShizukuEnabled(enabled: Boolean) = put(Keys.SHIZUKU_ENABLED, enabled)

    private suspend fun <T> put(
        key: Preferences.Key<T>,
        value: T,
    ) {
        provider.dataStore.edit { it[key] = value }
    }
}
