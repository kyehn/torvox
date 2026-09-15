package terminal.emulator.ui

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * ToolbarPreferences persistence: the default layout mirrors termux-app
 * v0.119.0-beta.3 extra keys, save/get round-trips built-in and custom keys,
 * and a corrupted store falls back to the default layout instead of
 * crashing the modifier bar.
 */
@RunWith(RobolectricTestRunner::class)
class ToolbarPreferencesTest {

    private lateinit var preferences: ToolbarPreferences

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        context.getSharedPreferences("toolbar_prefs", Context.MODE_PRIVATE).edit().clear().commit()
        preferences = ToolbarPreferences(context)
    }

    private fun labelOf(item: ToolbarItem): String = when (item) {
        is ToolbarItem.Default -> item.key.defaultLabel
        is ToolbarItem.Custom -> item.label
    }

    @Test
    fun `default layout mirrors termux extra keys`() {
        val labels = preferences.defaultLayout().map(::labelOf)
        assertEquals(
            listOf(
                "ESC", "≡", "SCROLL", "HOME", "↑", "END", "PGUP",
                "TAB", "CTRL", "ALT", "←", "↓", "→", "PGDN",
            ),
            labels,
        )
    }

    @Test
    fun `saved layout round-trips built-in and custom keys`() {
        val layout =
            listOf(
                ToolbarItem.Default(ToolbarKey.ESC),
                ToolbarItem.Custom(label = "ls", sequence = "ls\n", id = "custom_1"),
                ToolbarItem.Default(ToolbarKey.ALT),
            )
        preferences.saveLayout(layout)
        val restored = preferences.getLayout()
        assertEquals(layout.size, restored.size)
        assertEquals("ESC", labelOf(restored[0]))
        assertEquals("ls", labelOf(restored[1]))
        assertEquals((restored[1] as ToolbarItem.Custom).sequence, "ls\n")
        assertEquals("ALT", labelOf(restored[2]))
    }

    @Test
    fun `corrupted store falls back to the default layout`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        context.getSharedPreferences("toolbar_prefs", Context.MODE_PRIVATE)
            .edit()
            .putString("layout", "{ not json")
            .commit()

        val restored = preferences.getLayout()
        assertEquals(preferences.defaultLayout().size, restored.size)
        assertTrue(restored.all { it is ToolbarItem.Default })
    }

    @Test
    fun `unknown key is skipped while known keys survive`() {
        // 旧版本残留未知键：只跳过该项，已存的已知键必须保留
        //（对等 ghostty ExtraKeysConfig.enabledKeysSkipUnknownIds）。
        val context = ApplicationProvider.getApplicationContext<Context>()
        context.getSharedPreferences("toolbar_prefs", Context.MODE_PRIVATE)
            .edit()
            .putString(
                "layout",
                """[{"key":"ESC"},{"key":"BOGUS_KEY"},{"key":"ALT"}]""",
            )
            .commit()

        val restored = preferences.getLayout()
        assertEquals(
            listOf("ESC", "ALT"),
            restored.map(::labelOf),
        )
    }

    @Test
    fun `all unknown keys fall back to the default layout`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        context.getSharedPreferences("toolbar_prefs", Context.MODE_PRIVATE)
            .edit()
            .putString("layout", """[{"key":"BOGUS_KEY"}]""")
            .commit()

        assertEquals(preferences.defaultLayout().size, preferences.getLayout().size)
    }
}
