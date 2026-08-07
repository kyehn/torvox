package terminal.emulator.ui.theme

import android.content.Context
import androidx.compose.ui.graphics.Color
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * User-created terminal themes, persisted in DataStore as JSON.
 *
 * Reference: ghostty-android ThemeStore.java — presets are immutable, user
 * themes are stored (there as SharedPreferences JSON/CSV) and merged into the
 * pick list; a corrupt entry is skipped rather than losing every theme.
 * torvox uses DataStore + kotlinx-serialization (the project's existing
 * serialization stack) with the same "skip corrupt entry" self-heal.
 *
 * Color values are stored as the packed Compose [Color.value] ULong (color
 * space + ARGB), which round-trips exactly through
 * `Color(ulongValue)`; [UserThemeDto.toTerminalTheme] converts.
 */
@Serializable
data class UserThemeDto(
    val name: String,
    val backgroundValue: ULong,
    val foregroundValue: ULong,
    val cursorValue: ULong,
    val selectionBgValue: ULong,
    val ansiValue: List<ULong>,
) {
    fun toTerminalTheme(): TerminalTheme =
        TerminalTheme(
            name = name,
            background = Color(backgroundValue),
            foreground = Color(foregroundValue),
            cursor = Color(cursorValue),
            selectionBg = Color(selectionBgValue),
            ansi = ansiValue.map { Color(it) },
        )

    companion object {
        fun fromTerminalTheme(theme: TerminalTheme): UserThemeDto =
            UserThemeDto(
                name = theme.name,
                backgroundValue = theme.background.value,
                foregroundValue = theme.foreground.value,
                cursorValue = theme.cursor.value,
                selectionBgValue = theme.selectionBg.value,
                ansiValue = theme.ansi.map { it.value },
            )
    }
}

/** DataStore-backed store of user-created themes (ghostty-android ThemeStore pattern). */
class UserThemeStore(
    private val context: Context,
    private val storeName: String = "user_themes",
) {
    private val key = stringPreferencesKey("user_themes")
    private val json = Json { ignoreUnknownKeys = true }
    // Runtime-parameterized DataStore (the preferencesDataStore delegate is
    // per-property; a factory lets tests isolate per-test store names).
    private val dataStore: androidx.datastore.core.DataStore<androidx.datastore.preferences.core.Preferences> by lazy {
        PreferenceDataStoreFactory.create(
            produceFile = { context.preferencesDataStoreFile(storeName) },
        )
    }

    val userThemes: Flow<List<TerminalTheme>> =
        dataStore.data.map { prefs ->
            val raw = prefs[key] ?: return@map emptyList()
            try {
                json.decodeFromString<List<UserThemeDto>>(raw).map { it.toTerminalTheme() }
            } catch (e: Exception) {
                // Self-heal: a corrupt blob is treated as no user themes
                // (ghostty-android ThemeStore.userThemes() JSONException path).
                emptyList()
            }
        }

    suspend fun save(theme: TerminalTheme) {
        dataStore.edit { prefs ->
            val current =
                try {
                    prefs[key]?.let { json.decodeFromString<List<UserThemeDto>>(it) } ?: emptyList()
                } catch (e: Exception) {
                    emptyList()
                }
            val withoutSameName = current.filter { it.name != theme.name }
            val updated = withoutSameName + UserThemeDto.fromTerminalTheme(theme)
            prefs[key] = json.encodeToString(updated)
        }
    }

    suspend fun delete(name: String) {
        dataStore.edit { prefs ->
            val current =
                try {
                    prefs[key]?.let { json.decodeFromString<List<UserThemeDto>>(it) } ?: emptyList()
                } catch (e: Exception) {
                    emptyList()
                }
            prefs[key] = json.encodeToString(current.filter { it.name != name })
        }
    }
}
