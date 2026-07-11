package io.torvox.settings

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
        val SESSION_RESTORE = booleanPreferencesKey("session_restore")
        val KEYBOARD_MODE = stringPreferencesKey("keyboard_mode")
        val USB_SERIAL_ENABLED = booleanPreferencesKey("usb_serial_enabled")
        val MCP_SERVER_ENABLED = booleanPreferencesKey("mcp_server_enabled")
        val VOLUME_KEY_MAP = booleanPreferencesKey("volume_key_map")
        val BACKGROUND_IMAGE_PATH = stringPreferencesKey("bg_image_path")
        val BACKGROUND_BLUR_RADIUS = intPreferencesKey("bg_blur_radius")
        val BACKGROUND_ALPHA = floatPreferencesKey("bg_alpha")
        val CURSOR_BLINK = booleanPreferencesKey("cursor_blink")
        val CURSOR_STYLE = stringPreferencesKey("cursor_style")
        val CURSOR_SPEED = intPreferencesKey("cursor_speed")
    }

    companion object {
        const val DEFAULT_FONT_SIZE = 18f
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
    val sessionRestore: Flow<Boolean> = provider.dataStore.data.map { it[Keys.SESSION_RESTORE] ?: false }
    val keyboardMode: Flow<String> = provider.dataStore.data.map { it[Keys.KEYBOARD_MODE] ?: DEFAULT_KEYBOARD_MODE }
    val usbSerialEnabled: Flow<Boolean> = provider.dataStore.data.map { it[Keys.USB_SERIAL_ENABLED] ?: false }
    val mcpServerEnabled: Flow<Boolean> = provider.dataStore.data.map { it[Keys.MCP_SERVER_ENABLED] ?: false }
    val volumeKeyMap: Flow<Boolean> = provider.dataStore.data.map { it[Keys.VOLUME_KEY_MAP] ?: false }
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

    suspend fun setFontSize(size: Float) {
        provider.dataStore.edit { it[Keys.FONT_SIZE] = size }
    }

    suspend fun setFontFamily(family: String) {
        provider.dataStore.edit { it[Keys.FONT_FAMILY] = family }
    }

    suspend fun setThemeName(name: String) {
        provider.dataStore.edit { it[Keys.THEME_NAME] = name }
    }

    suspend fun setDayThemeName(name: String) {
        provider.dataStore.edit { it[Keys.DAY_THEME_NAME] = name }
    }

    suspend fun setNightThemeName(name: String) {
        provider.dataStore.edit { it[Keys.NIGHT_THEME_NAME] = name }
    }

    suspend fun setThemeMode(mode: String) {
        provider.dataStore.edit { it[Keys.THEME_MODE] = mode }
    }

    suspend fun setAppThemeMode(mode: String) {
        provider.dataStore.edit { it[Keys.APP_THEME_MODE] = mode }
    }

    suspend fun setShell(shell: String) {
        provider.dataStore.edit { it[Keys.SHELL] = shell }
    }

    suspend fun setScrollbackLines(lines: Int) {
        provider.dataStore.edit { it[Keys.SCROLLBACK_LINES] = lines }
    }

    suspend fun setTouchBehavior(behavior: String) {
        provider.dataStore.edit { it[Keys.TOUCH_BEHAVIOR] = behavior }
    }

    suspend fun setBootstrapUrl(url: String) {
        provider.dataStore.edit { it[Keys.BOOTSTRAP_URL] = url }
    }

    suspend fun setUseNerdFontGlyphs(enabled: Boolean) {
        provider.dataStore.edit { it[Keys.USE_NERD_FONT_GLYPHS] = enabled }
    }

    suspend fun setUseSemanticSelection(enabled: Boolean) {
        provider.dataStore.edit { it[Keys.USE_SEMANTIC_SELECTION] = enabled }
    }

    suspend fun setSessionRestore(enabled: Boolean) {
        provider.dataStore.edit { it[Keys.SESSION_RESTORE] = enabled }
    }

    suspend fun setKeyboardMode(mode: String) {
        provider.dataStore.edit { it[Keys.KEYBOARD_MODE] = mode }
    }

    suspend fun setUsbSerialEnabled(enabled: Boolean) {
        provider.dataStore.edit { it[Keys.USB_SERIAL_ENABLED] = enabled }
    }

    suspend fun setMcpServerEnabled(enabled: Boolean) {
        provider.dataStore.edit { it[Keys.MCP_SERVER_ENABLED] = enabled }
    }

    suspend fun setVolumeKeyMap(enabled: Boolean) {
        provider.dataStore.edit { it[Keys.VOLUME_KEY_MAP] = enabled }
    }

    suspend fun setBackgroundImagePath(path: String) {
        provider.dataStore.edit { it[Keys.BACKGROUND_IMAGE_PATH] = path }
    }

    suspend fun setBackgroundBlurRadius(radius: Int) {
        provider.dataStore.edit { it[Keys.BACKGROUND_BLUR_RADIUS] = radius }
    }

    suspend fun setBackgroundAlpha(alpha: Float) {
        provider.dataStore.edit { it[Keys.BACKGROUND_ALPHA] = alpha }
    }

    suspend fun setCursorBlink(enabled: Boolean) {
        provider.dataStore.edit { it[Keys.CURSOR_BLINK] = enabled }
    }

    suspend fun setCursorStyle(style: String) {
        provider.dataStore.edit { it[Keys.CURSOR_STYLE] = style }
    }

    suspend fun setCursorSpeed(speedMs: Int) {
        provider.dataStore.edit { it[Keys.CURSOR_SPEED] = speedMs }
    }
}
