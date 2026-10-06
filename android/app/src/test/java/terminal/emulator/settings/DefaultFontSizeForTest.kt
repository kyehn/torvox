package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [SettingsRepository.defaultFontSizeFor] 的钳位行为。
 *
 * 覆盖的是**自适应默认值的取值区间** [SettingsRepository.ADAPTIVE_DEFAULT_MIN_SP]
 * ..[SettingsRepository.ADAPTIVE_DEFAULT_MAX_SP]，与用户可选范围
 * [SettingsRepository.FONT_SIZE_MIN_SP]..[SettingsRepository.FONT_SIZE_MAX_PX] 是两件事：
 * 前者决定「全新安装时算出的初始字号」，后者决定调节条能划到哪里。
 */
class DefaultFontSizeForTest {

    private fun expected(widthDp: Float): Float = (widthDp / 52f / 0.6f).coerceIn(
        SettingsRepository.ADAPTIVE_DEFAULT_MIN_SP,
        SettingsRepository.ADAPTIVE_DEFAULT_MAX_SP,
    )

    @Test
    fun smallPhoneNeverBelowFloor() {
        // 360dp：公式原值 11.5sp 低于下限 14sp，须被抬到下限。
        val size = SettingsRepository.defaultFontSizeFor(360f)
        assertTrue(
            "360dp 必须不小于下限，实际 $size",
            size >= SettingsRepository.ADAPTIVE_DEFAULT_MIN_SP,
        )
        assertEquals(SettingsRepository.ADAPTIVE_DEFAULT_MIN_SP, size, 0.001f)
    }

    @Test
    fun emulatorWidthHitsFloor() {
        // 411dp（1080px @420dpi）：公式原值 13.17sp 同样低于下限 14sp，落在下限上。
        assertEquals(expected(411f), SettingsRepository.defaultFontSizeFor(411f), 0.001f)
        assertEquals(SettingsRepository.ADAPTIVE_DEFAULT_MIN_SP, SettingsRepository.defaultFontSizeFor(411f), 0.001f)
    }

    @Test
    fun tabletClampsToMax() {
        // 900dp：公式原值 28.8sp 高于上限 24sp，须被压到上限。
        assertEquals(SettingsRepository.ADAPTIVE_DEFAULT_MAX_SP, SettingsRepository.defaultFontSizeFor(900f), 0.001f)
    }

    @Test
    fun degenerateWidthStillAtFloor() {
        assertEquals(SettingsRepository.ADAPTIVE_DEFAULT_MIN_SP, SettingsRepository.defaultFontSizeFor(0f), 0.001f)
        assertEquals(
            SettingsRepository.ADAPTIVE_DEFAULT_MIN_SP,
            SettingsRepository.defaultFontSizeFor(-100f),
            0.001f,
        )
    }
}
