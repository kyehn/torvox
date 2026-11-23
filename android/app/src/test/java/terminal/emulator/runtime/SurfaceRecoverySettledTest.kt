package terminal.emulator.runtime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 恢复确认：连续稳定帧达到 [SURFACE_RECOVERY_HEALTHY_FRAMES] 才清零换视图预算。
 *
 * 不以「已出过任何帧」作恢复信号——2026-11-23 11-05-03 的每一代新设备都在首帧后
 * 13–28ms 再次丢失，按单帧成功清零会让 5 次快试与 500ms 间隔在 29 秒内反复失效。
 */
class SurfaceRecoverySettledTest {

    @Test
    fun `no healthy frame does not settle recovery`() {
        assertFalse(surfaceRecoverySettled(0))
    }

    @Test
    fun `stable frames below the threshold do not settle recovery`() {
        assertFalse(surfaceRecoverySettled(SURFACE_RECOVERY_HEALTHY_FRAMES - 1))
    }

    @Test
    fun `reaching the stable frame threshold settles recovery`() {
        assertTrue(surfaceRecoverySettled(SURFACE_RECOVERY_HEALTHY_FRAMES))
        assertTrue(surfaceRecoverySettled(SURFACE_RECOVERY_HEALTHY_FRAMES + 1))
    }
}
