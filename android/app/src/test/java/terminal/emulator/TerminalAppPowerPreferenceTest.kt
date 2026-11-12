package terminal.emulator

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 适配器功耗偏好的判定。
 *
 * 判别力：省电模式开着时选独显会显著耗电（移动端 SoC 上集显通常也是唯一选项，
 * 桌面/高端设备上集显的呈现延迟明显更高），两个方向写反都只有在此才暴露。
 */
class TerminalAppPowerPreferenceTest {
    @Test
    fun `power save mode prefers the integrated gpu`() {
        assertEquals(
            TerminalApp.POWER_PREFERENCE_LOW_POWER,
            TerminalApp.powerPreferenceFor(isPowerSaveMode = true),
        )
    }

    @Test
    fun `normal mode prefers the discrete gpu`() {
        assertEquals(
            TerminalApp.POWER_PREFERENCE_HIGH_PERFORMANCE,
            TerminalApp.powerPreferenceFor(isPowerSaveMode = false),
        )
    }
}
