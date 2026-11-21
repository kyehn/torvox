package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 环境固定的判据：显示模式、首选模式与固定性能模式的解析。
 *
 * 断言用设备实测输出作字面量，不拿被测实现算出的值反过来当期望。
 */
class EnvironmentPinningTest {
    @Test
    fun parsesDisplayModes() {
        val modes = parseDisplayModes(DISPLAY_OUTPUT_SAMPLE)

        assertEquals(1, modes.size)
        assertEquals(1, modes[0].id)
        assertEquals(320, modes[0].width)
        assertEquals(640, modes[0].height)
        assertEquals(60.000004, modes[0].refreshRateHz, 0.0)
    }

    @Test
    fun parsesActiveDisplayMode() {
        val mode = parseActiveDisplayMode(DISPLAY_OUTPUT_SAMPLE)

        assertEquals(1, mode.id)
        assertEquals(60.000004, mode.refreshRateHz, 0.0)
    }

    @Test
    fun rejectsDisplayDumpWithoutDisplayDevice() {
        val failure = runCatching { parseDisplayModes("mActiveRenderFrameRate=60.000004\n") }

        assertTrue(failure.exceptionOrNull()?.message.orEmpty().contains("DisplayDeviceInfo"))
    }

    @Test
    fun parsesPreferredDisplayMode() {
        val mode = parsePreferredDisplayMode("User preferred display mode: 320 640 60.000004\n")

        assertEquals(320, mode?.width)
        assertEquals(640, mode?.height)
        assertEquals(60.000004, mode?.refreshRateHz ?: 0.0, 0.0)
    }

    @Test
    fun treatsUnsetPreferredDisplayModeAsAbsent() {
        assertNull(parsePreferredDisplayMode("User preferred display mode: null\n"))
    }

    @Test
    fun readsFixedPerformanceModeWhenDeviceReportsIt() {
        val state = parseFixedPerformanceMode("  mFixedPerformanceModeEnabled=true\n")

        assertEquals(FixedPerformanceState.ENABLED, state)
    }

    @Test
    fun reportsFixedPerformanceModeAsDisabledWhenDeviceSaysSo() {
        val state = parseFixedPerformanceMode("  mFixedPerformanceModeEnabled=false\n")

        assertEquals(FixedPerformanceState.DISABLED, state)
    }

    @Test
    fun reportsFixedPerformanceModeAsUnverifiableWhenDeviceHasNoSuchField() {
        val state = parseFixedPerformanceMode("  mWakefulness=Awake\n  mStayOn=true\n")

        assertEquals(FixedPerformanceState.UNVERIFIABLE, state)
    }

    private companion object {
        /** 真实 `dumpsys display` 的显示设备行（裁剪到显示模式与活动模式）。 */
        val DISPLAY_OUTPUT_SAMPLE =
            """
            DisplayDeviceInfo{"Built-in Screen": uniqueId="local:4619827259835644672", 320 x 640, modeId 1, renderFrameRate 60.000004, defaultModeId 1, userPreferredModeId -1, supportedModes [{id=1, width=320, height=640, fps=60.000004, vsync=60.000004, synthetic=false, alternativeRefreshRates=[], supportedHdrTypes=[]}], colorMode 0}
              mActiveRenderFrameRate=60.000004
            """.trimIndent()
    }
}
