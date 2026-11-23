package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * 显示方向的解析判据。
 *
 * 旋转这一段要拿「系统已经转过去了」当证据：用户方向设置只说明要求转到哪，旋转动画
 * 期间设置值早已改掉而显示还没转。方向读错，旋转后的首帧就没有起点。
 */
class BackgroundForegroundBenchmarkTest {
    @Test
    fun parsesCurrentDisplayRotation() {
        assertEquals("ROTATION_90", parseDisplayRotation(DISPLAY_ROTATION_SAMPLE))
    }

    @Test
    fun reportsMissingDisplayRotation() {
        assertNull(parseDisplayRotation("  Display{#0 state=ON size=320x640 ROTATION_0}:\n    mRotation=0\n"))
    }

    private companion object {
        /** 窗口管理器的实测片段：`mCurrentRotation` 在 `DisplayRotation` 段内。 */
        val DISPLAY_ROTATION_SAMPLE =
            """
            DisplayRotation
              mRotation=0 mDeferredRotationPauseCount=0
                mCurrentRotation=ROTATION_90
                  mProposedRotation=0
            """.trimIndent()
    }
}
