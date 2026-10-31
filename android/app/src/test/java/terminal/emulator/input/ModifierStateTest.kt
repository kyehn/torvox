package terminal.emulator.input

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * 粘滞修饰键的状态语义。
 *
 * 三条消费路径（工具条按键、硬件按键编码、输入法提交）都读这里的判定，故状态迁移
 * 由纯函数承载、单测直接钉住，而不是在各条路径上各写一遍 if。
 */
class ModifierStateTest {
    @Test
    fun `only locked once and held count as active`() {
        assertEquals(false, ModifierState.Off.isActive())
        assertEquals(true, ModifierState.Once.isActive())
        assertEquals(true, ModifierState.Held.isActive())
        assertEquals(true, ModifierState.Locked.isActive())
    }

    @Test
    fun `tapping toggles the one-shot state only`() {
        assertEquals(ModifierState.Once, ModifierState.Off.toggled())
        assertEquals(ModifierState.Off, ModifierState.Once.toggled())
        // 长按锁定的状态不被轻点关掉：轻点只管一次性那一档。
        assertEquals(ModifierState.Off, ModifierState.Locked.toggled())
        assertEquals(ModifierState.Off, ModifierState.Held.toggled())
    }

    // / 按住音量减期间每键都带 Ctrl：`Held` 不被一次性回收消费。
    @Test
    fun `pressing the virtual ctrl key holds it`() {
        assertEquals(ModifierState.Held, ModifierState.Off.afterVirtualCtrlKey(actionDown = true))
        assertEquals(ModifierState.Held, ModifierState.Once.afterVirtualCtrlKey(actionDown = true))
    }

    // / 用户手动锁定的 Ctrl 必须活过一次音量减的按住：置 `Held` 后抬起会把它关掉，
    // / 表现为「用了一次音量键，Ctrl 锁定就没了」。
    @Test
    fun `pressing the virtual ctrl key leaves a locked ctrl alone`() {
        assertNull(ModifierState.Locked.afterVirtualCtrlKey(actionDown = true))
    }

    @Test
    fun `releasing the virtual ctrl key clears only its own hold`() {
        assertEquals(ModifierState.Off, ModifierState.Held.afterVirtualCtrlKey(actionDown = false))
        // 按住期间用户自己切成的状态不碰：抬起属于音量键，不属于用户那次切换。
        assertNull(ModifierState.Once.afterVirtualCtrlKey(actionDown = false))
        assertNull(ModifierState.Locked.afterVirtualCtrlKey(actionDown = false))
        assertNull(ModifierState.Off.afterVirtualCtrlKey(actionDown = false))
    }
}
