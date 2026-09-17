package terminal.emulator.ui

import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * 修饰键布局持久化语义（对标 sylirre ExtraKeysConfigTest 模型层）：
 * 未设置回默认、未知键跳过、损坏回默认、保存往返。纯 JVM，无设备依赖。
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ToolbarPreferencesTest {

    private fun preferences(): ToolbarPreferences {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        context.getSharedPreferences("toolbar_prefs", android.content.Context.MODE_PRIVATE)
            .edit().clear().commit()
        return ToolbarPreferences(context)
    }

    @Test
    fun `unset layout returns default`() {
        assertEquals(preferences().defaultLayout(), preferences().getLayout())
    }

    @Test
    fun `unknown key is skipped without dropping layout`() {
        val prefs = preferences()
        prefs.saveLayout(
            listOf(
                ToolbarItem.Default(ToolbarKey.ESC),
                ToolbarItem.Custom(label = "OLD", sequence = "x", id = "custom_old"),
            ),
        )
        // 直接写入含未知键的 JSON：未知项跳过，已知项保留。
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val raw =
            """[{"key":"NO_SUCH_KEY"},{"key":"ESC","width":2}]"""
        context.getSharedPreferences("toolbar_prefs", android.content.Context.MODE_PRIVATE)
            .edit().putString("layout", raw).commit()
        val layout = prefs.getLayout()
        assertEquals(1, layout.size)
        val first = layout.first() as ToolbarItem.Default
        assertEquals(ToolbarKey.ESC, first.key)
        assertEquals(2, first.width)
    }

    @Test
    fun `corrupt json falls back to default`() {
        val prefs = preferences()
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        context.getSharedPreferences("toolbar_prefs", android.content.Context.MODE_PRIVATE)
            .edit().putString("layout", "{not-json").commit()
        assertEquals(prefs.defaultLayout(), prefs.getLayout())
    }

    @Test
    fun `save round trips custom layout`() {
        val prefs = preferences()
        val items =
            listOf(
                ToolbarItem.Default(ToolbarKey.ESC, width = 2),
                ToolbarItem.Custom(label = "GIT", sequence = "git status\n"),
            )
        prefs.saveLayout(items)
        val loaded = prefs.getLayout()
        assertEquals(2, loaded.size)
        assertEquals(items[0], loaded[0])
        val custom = loaded[1] as ToolbarItem.Custom
        assertEquals("GIT", custom.label)
        assertEquals("git status\n", custom.sequence)
    }

    @Test
    fun `default layout matches termux fourteen keys`() {
        val labels = preferences().defaultLayout().map {
            (it as ToolbarItem.Default).key
        }
        assertEquals(14, labels.size)
        assertTrue(labels.contains(ToolbarKey.CTRL))
        assertTrue(labels.contains(ToolbarKey.ALT))
    }
}
