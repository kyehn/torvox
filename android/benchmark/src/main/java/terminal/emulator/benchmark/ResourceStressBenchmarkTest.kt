package terminal.emulator.benchmark

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 压力复原的判据：屏幕状态要按当前状态复原。
 *
 * 电源键是切换语义：施加压力时按过一次，屏幕状态与原先相反。复原时若不看当前状态，
 * 就会把设备留在被改过的状态里（原先亮屏被留在灭屏，或原先灭屏被留在亮屏）。
 */
class ResourceStressBenchmarkTest {
    @Test
    fun restoresScreenWhenStressFlippedIt() {
        // 原先亮屏，施加压力后灭屏：需要再按一次复原。
        assertTrue(needsScreenStateRestore(screenWasAwake = true, screenIsAwakeNow = false))
        // 原先灭屏，施加压力后亮屏：同样需要再按一次复原。
        assertTrue(needsScreenStateRestore(screenWasAwake = false, screenIsAwakeNow = true))
    }

    @Test
    fun keepsScreenWhenStateMatchesBaseline() {
        assertFalse(needsScreenStateRestore(screenWasAwake = true, screenIsAwakeNow = true))
        assertFalse(needsScreenStateRestore(screenWasAwake = false, screenIsAwakeNow = false))
    }
}
