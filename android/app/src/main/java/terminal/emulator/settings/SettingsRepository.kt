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
        val MCP_SERVER_ENABLED = booleanPreferencesKey("mcp_server_enabled")
        val ENVIRONMENT_VARIABLES = stringPreferencesKey("environment_variables")
        val BACKGROUND_IMAGE_PATH = stringPreferencesKey("bg_image_path")
        val BACKGROUND_BLUR_RADIUS = intPreferencesKey("bg_blur_radius")
        val BACKGROUND_ALPHA = floatPreferencesKey("bg_alpha")
        val CURSOR_BLINK = booleanPreferencesKey("cursor_blink")
        val CURSOR_STYLE = stringPreferencesKey("cursor_style")
        val CURSOR_SPEED = intPreferencesKey("cursor_speed")
        val BELL_MODE = intPreferencesKey("bell_mode")
        val SHORTCUT_PASTE = stringPreferencesKey("shortcut_paste")
        val SHORTCUT_NEW_SESSION = stringPreferencesKey("shortcut_new_session")
        val SHORTCUT_CLOSE_SESSION = stringPreferencesKey("shortcut_close_session")
        val SHORTCUT_COPY = stringPreferencesKey("shortcut_copy")
        val SHORTCUT_TOGGLE_SCROLL = stringPreferencesKey("shortcut_toggle_scroll")
    }

    companion object {
        const val DEFAULT_FONT_SIZE = 10f
        const val DEFAULT_SCROLLBACK_LINES = 10_000
        private const val DEFAULT_THEME = "Dracula Plus"
        const val DEFAULT_DAY_THEME_NAME = "Catppuccin Latte"
        const val DEFAULT_FOLLOW_SYSTEM = "follow_system"
        const val DEFAULT_THEME_MODE = "fixed"
        const val DEFAULT_KEYBOARD_MODE = "secure"
        const val DEFAULT_SHELL = "/system/bin/sh"
        const val DEFAULT_BACKGROUND_BLUR_RADIUS = 0
        const val DEFAULT_BACKGROUND_ALPHA = 0.8f
        const val DEFAULT_CURSOR_SPEED_MS = 530
        const val DEFAULT_BELL_MODE = 0

        /**
         * Device-adaptive first-launch font size (sp): a fresh install gets a
         * size derived from screen width (~64 columns on a 360dp portrait
         * phone, ~100 on an 800dp landscape tablet) instead of a fixed
         * absolute value, clamped to a readable 8..16sp band.
         */
        fun defaultFontSizeFor(screenWidthDp: Float): Float = (screenWidthDp / 32f).coerceIn(8f, 16f)
    }

    val appThemeMode: Flow<String> = provider.dataStore.data.map { it[Keys.APP_THEME_MODE] ?: DEFAULT_FOLLOW_SYSTEM }
    private val deviceDefaultFontSize: Float
        get() = defaultFontSizeFor(provider.screenWidthDp)

    val fontSize: Flow<Float> = provider.dataStore.data.map { it[Keys.FONT_SIZE] ?: deviceDefaultFontSize }

    /** True once the user has explicitly picked a font size; false on a fresh install. */
    val fontSizeExplicitlySet: Flow<Boolean> = provider.dataStore.data.map { it[Keys.FONT_SIZE] != null }
    val fontFamily: Flow<String> = provider.dataStore.data.map { it[Keys.FONT_FAMILY] ?: "" }
    val boldFontFamily: Flow<String> = provider.dataStore.data.map { it[Keys.BOLD_FONT_FAMILY] ?: "" }
    val italicFontFamily: Flow<String> = provider.dataStore.data.map { it[Keys.ITALIC_FONT_FAMILY] ?: "" }
    val themeName: Flow<String> = provider.dataStore.data.map { it[Keys.THEME_NAME] ?: DEFAULT_THEME }
    val dayThemeName: Flow<String> = provider.dataStore.data.map { it[Keys.DAY_THEME_NAME] ?: DEFAULT_DAY_THEME_NAME }
    val nightThemeName: Flow<String> = provider.dataStore.data.map { it[Keys.NIGHT_THEME_NAME] ?: DEFAULT_THEME }
    val themeMode: Flow<String> = provider.dataStore.data.map { it[Keys.THEME_MODE] ?: DEFAULT_THEME_MODE }
    val shell: Flow<String> = provider.dataStore.data.map { it[Keys.SHELL] ?: DEFAULT_SHELL }
    val scrollbackLines: Flow<Int> = provider.dataStore.data.map { it[Keys.SCROLLBACK_LINES] ?: DEFAULT_SCROLLBACK_LINES }
    val bootstrapUrl: Flow<String> = provider.dataStore.data.map { it[Keys.BOOTSTRAP_URL] ?: "" }
    val useNerdFontGlyphs: Flow<Boolean> = provider.dataStore.data.map { it[Keys.USE_NERD_FONT_GLYPHS] ?: false }
    val keyboardMode: Flow<String> = provider.dataStore.data.map { it[Keys.KEYBOARD_MODE] ?: DEFAULT_KEYBOARD_MODE }
    val mcpServerEnabled: Flow<Boolean> = provider.dataStore.data.map { it[Keys.MCP_SERVER_ENABLED] ?: false }

    /** User-defined environment overrides (KEY=VALUE), one per line. */
    val environmentVariables: Flow<Map<String, String>> =
        provider.dataStore.data.map {
            parseEnvironmentVariables(it[Keys.ENVIRONMENT_VARIABLES].orEmpty())
        }
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
        val mcpServerEnabled: Boolean = false,
        val environmentVariables: Map<String, String> = emptyMap(),
        val backgroundImagePath: String = "",
        val backgroundBlurRadius: Int = DEFAULT_BACKGROUND_BLUR_RADIUS,
        val backgroundAlpha: Float = DEFAULT_BACKGROUND_ALPHA,
        val cursorBlink: Boolean = true,
        val cursorStyle: String = "block",
        val cursorSpeed: Int = DEFAULT_CURSOR_SPEED_MS,
        val bellMode: Int = DEFAULT_BELL_MODE,
        val shortcutPaste: String = "",
        val shortcutNewSession: String = "",
        val shortcutCloseSession: String = "",
        val shortcutCopy: String = "",
        val shortcutToggleScroll: String = "",
    )

    val settings: Flow<SettingsState> = provider.dataStore.data.map { prefs ->
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
            mcpServerEnabled = prefs[Keys.MCP_SERVER_ENABLED] ?: false,
            environmentVariables = parseEnvironmentVariables(prefs[Keys.ENVIRONMENT_VARIABLES].orEmpty()),
            backgroundImagePath = prefs[Keys.BACKGROUND_IMAGE_PATH] ?: "",
            backgroundBlurRadius = prefs[Keys.BACKGROUND_BLUR_RADIUS] ?: DEFAULT_BACKGROUND_BLUR_RADIUS,
            backgroundAlpha = prefs[Keys.BACKGROUND_ALPHA] ?: DEFAULT_BACKGROUND_ALPHA,
            cursorBlink = prefs[Keys.CURSOR_BLINK] ?: true,
            cursorStyle = prefs[Keys.CURSOR_STYLE] ?: "block",
            cursorSpeed = prefs[Keys.CURSOR_SPEED] ?: DEFAULT_CURSOR_SPEED_MS,
            bellMode = prefs[Keys.BELL_MODE] ?: DEFAULT_BELL_MODE,
            shortcutPaste = prefs[Keys.SHORTCUT_PASTE] ?: "",
            shortcutNewSession = prefs[Keys.SHORTCUT_NEW_SESSION] ?: "",
            shortcutCloseSession = prefs[Keys.SHORTCUT_CLOSE_SESSION] ?: "",
            shortcutCopy = prefs[Keys.SHORTCUT_COPY] ?: "",
            shortcutToggleScroll = prefs[Keys.SHORTCUT_TOGGLE_SCROLL] ?: "",
        )
    }

    suspend fun setFontSize(size: Float) = put(Keys.FONT_SIZE, size)

    /**
     * Persist the device-adaptive default font size on first launch so a
     * fresh install renders a legible grid before the user touches the
     * font-size slider. No-op once the user has explicitly picked a size.
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

    suspend fun setMcpServerEnabled(enabled: Boolean) = put(Keys.MCP_SERVER_ENABLED, enabled)

    /** Persist user-defined environment overrides ("KEY=VALUE" lines). */
    suspend fun setEnvironmentVariables(vars: Map<String, String>) = put(Keys.ENVIRONMENT_VARIABLES, serializeEnvironmentVariables(vars))

    suspend fun setBackgroundImagePath(path: String) = put(Keys.BACKGROUND_IMAGE_PATH, path)

    suspend fun setBackgroundBlurRadius(radius: Int) = put(Keys.BACKGROUND_BLUR_RADIUS, radius)

    suspend fun setBackgroundAlpha(alpha: Float) = put(Keys.BACKGROUND_ALPHA, alpha)

    suspend fun setCursorBlink(enabled: Boolean) = put(Keys.CURSOR_BLINK, enabled)

    suspend fun setCursorStyle(style: String) = put(Keys.CURSOR_STYLE, style)

    suspend fun setCursorSpeed(speedMs: Int) = put(Keys.CURSOR_SPEED, speedMs)

    suspend fun setBellMode(id: Int) = put(Keys.BELL_MODE, id)

    /** Persist a serialized shortcut binding ("CTRL|SHIFT|54") for the given action id. */
    suspend fun setShortcutBinding(actionId: String, serialized: String) {
        val key = when (actionId) {
            terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_PASTE -> Keys.SHORTCUT_PASTE
            terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_NEW_SESSION -> Keys.SHORTCUT_NEW_SESSION
            terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_CLOSE_SESSION -> Keys.SHORTCUT_CLOSE_SESSION
            terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_COPY -> Keys.SHORTCUT_COPY
            terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_TOGGLE_SCROLL -> Keys.SHORTCUT_TOGGLE_SCROLL
            else -> return
        }
        put(key, serialized)
    }

    /** Clear a persisted shortcut binding (revert to default). */
    suspend fun clearShortcutBinding(actionId: String) = setShortcutBinding(actionId, "")

    private suspend fun <T> put(
        key: Preferences.Key<T>,
        value: T,
    ) {
        provider.dataStore.edit { it[key] = value }
    }
}

/**
 * Parse the persisted "KEY=VALUE" (one per line) environment overrides.
 * Lines without '=' or with a blank key are skipped; the first '=' splits
 * key from value so values may contain '=' and keep surrounding
 * whitespace. Mirrors the native `parse_env_entries` (shell_env.rs)
 * contract exactly (key trimmed, value untouched).
 */
internal fun parseEnvironmentVariables(raw: String): Map<String, String> {
    val result = linkedMapOf<String, String>()
    for (line in raw.lineSequence()) {
        val equals = line.indexOf('=')
        if (equals <= 0) continue
        val key = line.substring(0, equals).trim()
        if (key.isEmpty()) continue
        result[key] = line.substring(equals + 1)
    }
    return result
}

/** Serialize environment overrides to "KEY=VALUE" lines (sorted for stability). */
internal fun serializeEnvironmentVariables(vars: Map<String, String>): String = vars.entries
    .sortedBy { it.key }
    .joinToString(separator = "\n") { "${it.key}=${it.value}" }
