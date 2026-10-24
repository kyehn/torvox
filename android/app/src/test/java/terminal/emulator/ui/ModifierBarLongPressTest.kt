package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 修饰键栏长按阈值：取系统 `ViewConfiguration.getLongPressTimeout()`，
 * 越界退回默认值（Termux ExtraKeysView 同款规则）。
 *
 * 用户把系统「长按时长」调长后，粘滞键锁定与方向键自动重复必须同步变慢；
 * 固定 400ms 会让无障碍设置在这里失效。
 */
class ModifierBarLongPressTest {

    @Test
    fun `system timeout inside the sane range is used as is`() {
        assertEquals(500L, modifierLongPressMillis(500L))
        assertEquals(200L, modifierLongPressMillis(200L))
        assertEquals(3000L, modifierLongPressMillis(3000L))
    }

    @Test
    fun `out of range timeout falls back to the default`() {
        assertEquals(400L, modifierLongPressMillis(199L))
        assertEquals(400L, modifierLongPressMillis(3001L))
        assertEquals(400L, modifierLongPressMillis(0L))
    }
}
