package terminal.emulator.ui.theme

import android.os.Build
import androidx.compose.ui.graphics.Color
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/**
 * UserThemeStore (ghostty-android ThemeStore pattern): save/replace/delete
 * user themes with exact color round-trip through the packed Color.value
 * ULong; a corrupt stored blob self-heals to an empty list.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [Build.VERSION_CODES.TIRAMISU])
class UserThemeStoreTest {

    // Each test gets an isolated DataStore name AND instance (multiple
    // DataStore instances on the same file race; one store per test).
    private lateinit var store: UserThemeStore

    @Before
    fun freshStore() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        store = UserThemeStore(ctx, "user_themes_test_${System.nanoTime()}")
    }

    private fun theme(name: String) = TerminalTheme(
        name = name,
        background = Color(0xFF212121),
        foreground = Color(0xFFF8F8F2),
        cursor = Color(0xFFECEFF4),
        selectionBg = Color(0xFF44475A),
        ansi = List(16) { Color(0xFF000000 + it) },
    )

    @Test
    fun `save then read round-trips colors exactly`() = runBlocking {
        val t = theme("Mine")
        store.save(t)
        val loaded = store.userThemes.first()
        assertEquals(1, loaded.size)
        assertEquals("Mine", loaded[0].name)
        assertEquals(t.background.value, loaded[0].background.value)
        assertEquals(t.foreground.value, loaded[0].foreground.value)
        assertEquals(t.cursor.value, loaded[0].cursor.value)
        assertEquals(t.selectionBg.value, loaded[0].selectionBg.value)
        assertEquals(t.ansi.map { it.value }, loaded[0].ansi.map { it.value })
    }

    @Test
    fun `save with same name replaces entry`() = runBlocking {
        store.save(theme("A"))
        store.save(theme("A").copy(background = Color(0xFF101010)))
        store.save(theme("B"))
        val loaded = store.userThemes.first()
        assertEquals(2, loaded.size)
        assertEquals(Color(0xFF101010).value, loaded.first { it.name == "A" }.background.value)
    }

    @Test
    fun `delete removes entry`() = runBlocking {
        store.save(theme("A"))
        store.save(theme("B"))
        store.delete("A")
        val loaded = store.userThemes.first()
        assertEquals(1, loaded.size)
        assertEquals("B", loaded[0].name)
    }

    @Test
    fun `corrupt blob self-heals to empty`() = runBlocking {
        // Write a valid Preferences file whose "user_themes" value is invalid
        // JSON. DataStore itself can read the file (it is well-formed
        // protobuf); the JSON decode layer is what UserThemeStore self-heals.
        // A dedicated writer store on its own scope writes the blob, then the
        // scope is cancelled so its file lock is released before the store
        // under test opens the same file.
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val corruptName = "corrupt_${System.nanoTime()}"
        val corruptFile = ctx.preferencesDataStoreFile(corruptName)
        val writerScope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.IO)
        val writerStore =
            androidx.datastore.preferences.core.PreferenceDataStoreFactory.create(
                scope = writerScope,
                produceFile = { corruptFile },
            )
        writerStore.edit {
            it[androidx.datastore.preferences.core.stringPreferencesKey("user_themes")] = "{not json"
        }
        writerScope.cancel()

        val corruptStore = UserThemeStore(ctx, corruptName)
        assertTrue("corrupt blob must not crash", corruptStore.userThemes.first().isEmpty())
    }

    @Test
    fun `two instances on the same store name do not crash`() = runBlocking {
        // Regression: a fresh PreferenceDataStoreFactory per
        // UserThemeStore instance crashed with "multiple DataStores active
        // for the same file" when the ViewModel was recreated. The store
        // is now a process singleton per name; a second instance must be
        // able to read and write the same file.
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val name = "user_themes_dual_${System.nanoTime()}"
        val first = UserThemeStore(ctx, name)
        val second = UserThemeStore(ctx, name)

        first.save(theme("FromFirst"))
        // The second instance shares the singleton DataStore: read and
        // write both succeed without IllegalStateException.
        val seenBySecond = second.userThemes.first()
        assertEquals(1, seenBySecond.size)
        assertEquals("FromFirst", seenBySecond[0].name)
        second.save(theme("FromSecond"))
        val seenByFirst = first.userThemes.first()
        assertEquals(2, seenByFirst.size)
    }
}
