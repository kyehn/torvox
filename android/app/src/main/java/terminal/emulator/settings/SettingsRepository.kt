package terminal.emulator.settings

import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.flow.Flow
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
        val THEME_NAME = stringPreferencesKey("theme_name")
        val DAY_THEME_NAME = stringPreferencesKey("day_theme_name")
        val NIGHT_THEME_NAME = stringPreferencesKey("night_theme_name")
        val THEME_MODE = stringPreferencesKey("theme_mode")
        val SHELL = stringPreferencesKey("shell")
        val SCROLLBACK_LINES = intPreferencesKey("scrollback_lines")
        val APP_THEME_MODE = stringPreferencesKey("app_theme_mode")
        val TOUCH_BEHAVIOR = stringPreferencesKey("touch_behavior")
        val BOOTSTRAP_URL = stringPreferencesKey("bootstrap_url")
        val USE_NERD_FONT_GLYPHS = booleanPreferencesKey("use_nerd_font_glyphs")
        val USE_SEMANTIC_SELECTION = booleanPreferencesKey("use_semantic_selection")
        val KEYBOARD_MODE = stringPreferencesKey("keyboard_mode")
        val MCP_SERVER_ENABLED = booleanPreferencesKey("mcp_server_enabled")
        val BACKGROUND_IMAGE_PATH = stringPreferencesKey("bg_image_path")
        val BACKGROUND_BLUR_RADIUS = intPreferencesKey("bg_blur_radius")
        val BACKGROUND_ALPHA = floatPreferencesKey("bg_alpha")
        val CURSOR_BLINK = booleanPreferencesKey("cursor_blink")
        val CURSOR_STYLE = stringPreferencesKey("cursor_style")
        val CURSOR_SPEED = intPreferencesKey("cursor_speed")
        val BELL_MODE = intPreferencesKey("bell_mode")
    }

    companion object {
        const val DEFAULT_FONT_SIZE = 10f
        const val DEFAULT_SCROLLBACK_LINES = 50_000
        private const val DEFAULT_THEME = "Dracula Plus"
        const val DEFAULT_DAY_THEME_NAME = "Catppuccin Latte"
        const val DEFAULT_FOLLOW_SYSTEM = "follow_system"
        const val DEFAULT_THEME_MODE = "fixed"
        const val DEFAULT_TOUCH_BEHAVIOR = "right_click"
        const val DEFAULT_KEYBOARD_MODE = "secure"
        const val DEFAULT_SHELL = "/system/bin/sh"
        const val DEFAULT_BACKGROUND_BLUR_RADIUS = 0
        const val DEFAULT_BACKGROUND_ALPHA = 0.8f
        const val DEFAULT_CURSOR_SPEED_MS = 530
        const val DEFAULT_BELL_MODE = 0
    }

    val appThemeMode: Flow<String> = provider.dataStore.data.map { it[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM }
    val fontSize: Flow<Float> = provider.dataStore.data.map { it[Keys.FONT_SIZE] ?: DEFAULT_FONT_SIZE }
    val fontFamily: Flow<String> = provider.dataStore.data.map { it[Keys.FONT_FAMILY] ?: "" }
    val themeName: Flow<String> = provider.dataStore.data.map { it[Keys.THEME_NAME] ?: DEFAULT_THEME }
    val dayThemeName: Flow<String> = provider.dataStore.data.map { it[Keys.DAY_THEME_NAME] ?: DEFAULT_DAY_THEME_NAME }
    val nightThemeName: Flow<String> = provider.dataStore.data.map { it[Keys.NIGHT_THEME_NAME] ?: DEFAULT_THEME }
    val themeMode: Flow<String> = provider.dataStore.data.map { it[Keys.THEME_MODE] ?: DEFAULT_THEME_MODE }
    val shell: Flow<String> = provider.dataStore.data.map { it[Keys.SHELL] ?: DEFAULT_SHELL }
    val scrollbackLines: Flow<Int> = provider.dataStore.data.map { it[Keys.SCROLLBACK_LINES] ?: DEFAULT_SCROLLBACK_LINES }
    val touchBehavior: Flow<String> = provider.dataStore.data.map { it[Keys.TOUCH_BEHAVIOR] ?: DEFAULT_TOUCH_BEHAVIOR }
    val bootstrapUrl: Flow<String> = provider.dataStore.data.map { it[Keys.BOOTSTRAP_URL] ?: "" }
    val useNerdFontGlyphs: Flow<Boolean> = provider.dataStore.data.map { it[Keys.USE_NERD_FONT_GLYPHS] ?: false }
    val useSemanticSelection: Flow<Boolean> = provider.dataStore.data.map { it[Keys.USE_SEMANTIC_SELECTION] ?: false }
    val keyboardMode: Flow<String> = provider.dataStore.data.map { it[Keys.KEYBOARD_MODE] ?: DEFAULT_KEYBOARD_MODE }
    val mcpServerEnabled: Flow<Boolean> = provider.dataStore.data.map { it[Keys.MCP_SERVER_ENABLED] ?: false }
    val backgroundImagePath: Flow<String> = provider.dataStore.data.map { it[Keys.BACKGROUND_IMAGE_PATH] ?: "" }
    val backgroundBlurRadius: Flow<Int> =
        provider.dataStore.data.map {
            it[Keys.BACKGROUND_BLUR_RADIUS]
                ?: DEFAULT_BACKGROUND_BLUR_RADIUS
        }
    val backgroundAlpha: Flow<Float> = provider.dataStore.data.map { it[Keys.BACKGROUND_ALPHA] ?: DEFAULT_BACKGROUND_ALPHA }
    val cursorBlink: Flow<Boolean> = provider.dataStore.data.map { it[Keys.CURSOR_BLINK] ?: true }
    val cursorStyle: Flow<String> = provider.dataStore.data.map { it[Keys.CURSOR_STYLE] ?: "block" }
    val cursorSpeed: Flow<Int> = provider.dataStore.data.map { it[Keys.CURSOR_SPEED] ?: DEFAULT_CURSOR_SPEED_MS }
    val bellMode: Flow<Int> = provider.dataStore.data.map { it[Keys.BELL_MODE] ?: DEFAULT_BELL_MODE }

    /**
     * Single merged snapshot of every persisted setting, derived from one
     * DataStore read. UI subscribes to this one flow instead of 21 parallel
     * per-field pipelines (C7). Field defaults mirror the per-field flows
     * above; keep both in sync when adding a setting.
     */
    data class SettingsState(
        val appThemeMode: String = DEFAULT_FOLLOW_SYSTEM,
        val fontSize: Float = DEFAULT_FONT_SIZE,
        val fontFamily: String = "",
        val themeName: String = DEFAULT_THEME,
        val dayThemeName: String = DEFAULT_DAY_THEME_NAME,
        val nightThemeName: String = DEFAULT_THEME,
        val themeMode: String = DEFAULT_THEME_MODE,
        val shell: String = DEFAULT_SHELL,
        val scrollbackLines: Int = DEFAULT_SCROLLBACK_LINES,
        val touchBehavior: String = DEFAULT_TOUCH_BEHAVIOR,
        val bootstrapUrl: String = "",
        val useNerdFontGlyphs: Boolean = false,
        val useSemanticSelection: Boolean = false,
        val keyboardMode: String = DEFAULT_KEYBOARD_MODE,
        val mcpServerEnabled: Boolean = false,
        val backgroundImagePath: String = "",
        val backgroundBlurRadius: Int = DEFAULT_BACKGROUND_BLUR_RADIUS,
        val backgroundAlpha: Float = DEFAULT_BACKGROUND_ALPHA,
        val cursorBlink: Boolean = true,
        val cursorStyle: String = "block",
        val cursorSpeed: Int = DEFAULT_CURSOR_SPEED_MS,
        val bellMode: Int = DEFAULT_BELL_MODE,
    )

    val settings: Flow<SettingsState> = provider.dataStore.data.map { prefs ->
        SettingsState(
            appThemeMode = prefs[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM,
            fontSize = prefs[Keys.FONT_SIZE] ?: DEFAULT_FONT_SIZE,
            fontFamily = prefs[Keys.FONT_FAMILY] ?: "",
            themeName = prefs[Keys.THEME_NAME] ?: DEFAULT_THEME,
            dayThemeName = prefs[Keys.DAY_THEME_NAME] ?: DEFAULT_DAY_THEME_NAME,
            nightThemeName = prefs[Keys.NIGHT_THEME_NAME] ?: DEFAULT_THEME,
            themeMode = prefs[Keys.THEME_MODE] ?: DEFAULT_THEME_MODE,
            shell = prefs[Keys.SHELL] ?: DEFAULT_SHELL,
            scrollbackLines = prefs[Keys.SCROLLBACK_LINES] ?: DEFAULT_SCROLLBACK_LINES,
            touchBehavior = prefs[Keys.TOUCH_BEHAVIOR] ?: DEFAULT_TOUCH_BEHAVIOR,
            bootstrapUrl = prefs[Keys.BOOTSTRAP_URL] ?: "",
            useNerdFontGlyphs = prefs[Keys.USE_NERD_FONT_GLYPHS] ?: false,
            useSemanticSelection = prefs[Keys.USE_SEMANTIC_SELECTION] ?: false,
            keyboardMode = prefs[Keys.KEYBOARD_MODE] ?: DEFAULT_KEYBOARD_MODE,
            mcpServerEnabled = prefs[Keys.MCP_SERVER_ENABLED] ?: false,
            backgroundImagePath = prefs[Keys.BACKGROUND_IMAGE_PATH] ?: "",
            backgroundBlurRadius = prefs[Keys.BACKGROUND_BLUR_RADIUS] ?: DEFAULT_BACKGROUND_BLUR_RADIUS,
            backgroundAlpha = prefs[Keys.BACKGROUND_ALPHA] ?: DEFAULT_BACKGROUND_ALPHA,
            cursorBlink = prefs[Keys.CURSOR_BLINK] ?: true,
            cursorStyle = prefs[Keys.CURSOR_STYLE] ?: "block",
            cursorSpeed = prefs[Keys.CURSOR_SPEED] ?: DEFAULT_CURSOR_SPEED_MS,
            bellMode = prefs[Keys.BELL_MODE] ?: DEFAULT_BELL_MODE,
        )
    }

    suspend fun setFontSize(size: Float) = put(Keys.FONT_SIZE, size)

    suspend fun setFontFamily(family: String) = put(Keys.FONT_FAMILY, family)

    suspend fun setThemeName(name: String) = put(Keys.THEME_NAME, name)

    suspend fun setDayThemeName(name: String) = put(Keys.DAY_THEME_NAME, name)

    suspend fun setNightThemeName(name: String) = put(Keys.NIGHT_THEME_NAME, name)

    suspend fun setThemeMode(mode: String) = put(Keys.THEME_MODE, mode)

    suspend fun setAppThemeMode(mode: String) = put(Keys.APP_THEME_MODE, mode)

    suspend fun setShell(shell: String) = put(Keys.SHELL, shell)

    suspend fun setScrollbackLines(lines: Int) = put(Keys.SCROLLBACK_LINES, lines)

    suspend fun setTouchBehavior(behavior: String) = put(Keys.TOUCH_BEHAVIOR, behavior)

    suspend fun setBootstrapUrl(url: String) = put(Keys.BOOTSTRAP_URL, url)

    suspend fun setUseNerdFontGlyphs(enabled: Boolean) = put(Keys.USE_NERD_FONT_GLYPHS, enabled)

    suspend fun setUseSemanticSelection(enabled: Boolean) = put(Keys.USE_SEMANTIC_SELECTION, enabled)

    suspend fun setKeyboardMode(mode: String) = put(Keys.KEYBOARD_MODE, mode)

    suspend fun setMcpServerEnabled(enabled: Boolean) = put(Keys.MCP_SERVER_ENABLED, enabled)

    suspend fun setBackgroundImagePath(path: String) = put(Keys.BACKGROUND_IMAGE_PATH, path)

    suspend fun setBackgroundBlurRadius(radius: Int) = put(Keys.BACKGROUND_BLUR_RADIUS, radius)

    suspend fun setBackgroundAlpha(alpha: Float) = put(Keys.BACKGROUND_ALPHA, alpha)

    suspend fun setCursorBlink(enabled: Boolean) = put(Keys.CURSOR_BLINK, enabled)

    suspend fun setCursorStyle(style: String) = put(Keys.CURSOR_STYLE, style)

    suspend fun setCursorSpeed(speedMs: Int) = put(Keys.CURSOR_SPEED, speedMs)

    suspend fun setBellMode(id: Int) = put(Keys.BELL_MODE, id)

    private suspend fun <T> put(
        key: Preferences.Key<T>,
        value: T,
    ) {
        provider.dataStore.edit { it[key] = value }
    }
}
