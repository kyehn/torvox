package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.settings.SettingsRepository

/**
 * sp→px 系数（[coerceSpToPxScale]）的换算与钳位。
 *
 * 该系数同时是字号的换算基准与推给原生 `setRasterScale` 的值，两者 MUST 同源：
 * 字形实际光栅尺度即 `sp × 系数`。钳位区间 MUST 与原生接受区间一致
 * （Rust `setRasterScale`：`if !(0.5..=8.0).contains(&scale)`，见
 * `native/src/android/ffi.rs`）——区间外的值被原生静默丢弃，会使字号上界与
 * 实际渲染脱节。
 */
class CoerceSpToPxScaleTest {

    /** 原生 `setRasterScale` 的接受区间，逐字对应 `ffi.rs` 中的 `0.5..=8.0`。 */
    private val nativeAcceptedRange = 0.5f..8.0f

    @Test
    fun `scale is the product of density and the system font scaling`() {
        assertEquals(2.625f, coerceSpToPxScale(density = 2.625f, fontScale = 1.0f), 0.001f)
        assertEquals(3.4125f, coerceSpToPxScale(density = 2.625f, fontScale = 1.3f), 0.001f)
    }

    @Test
    fun `the system font scaling is what keeps the ceiling from being exceeded`() {
        // 缺陷根因：上界按仅密度（2.625）算出 96sp，但字形实际按
        // 密度 × 系统字体缩放（3.4125）光栅化，故 96sp 实际是 327.6px，
        // 越过 Termux 的 256px 上限。计入系统字体缩放后上界为 74sp，
        // 实际 252.5px 回到上限内。
        val densityOnly = coerceSpToPxScale(density = 2.625f, fontScale = 1.0f)
        val withFontScaling = coerceSpToPxScale(density = 2.625f, fontScale = 1.3f)
        assertEquals(2.625f, densityOnly, 0.001f)
        assertEquals(3.4125f, withFontScaling, 0.001f)

        val sliderMaxSpDensityOnly = SettingsRepository.fontSizeRangeMaxSp(densityOnly)
        val sliderMaxSpWithFontScaling = SettingsRepository.fontSizeRangeMaxSp(withFontScaling)
        // 只按密度算出的上界在真实系数下越界。
        assertEquals(96f, sliderMaxSpDensityOnly, 0.001f)
        assertTrue(
            "按密度算出的上界 ${sliderMaxSpDensityOnly}sp 在真实系数下越界",
            sliderMaxSpDensityOnly * withFontScaling > SettingsRepository.FONT_SIZE_MAX_PX,
        )
        // 计入系统字体缩放后收紧，且实际像素回到 Termux 上限内。
        assertEquals(74f, sliderMaxSpWithFontScaling, 0.001f)
        assertTrue(
            "计入系统字体缩放后实际像素必须不超过 Termux 上限",
            sliderMaxSpWithFontScaling * withFontScaling <= SettingsRepository.FONT_SIZE_MAX_PX,
        )
    }

    @Test
    fun `values inside the native range pass through untouched`() {
        listOf(0.5f, 1f, 2.625f, 3.4125f, 8f).forEach { scale ->
            assertEquals(
                "系数 $scale 在原生接受区间内时不得被改动",
                scale,
                coerceSpToPxScale(density = scale, fontScale = 1.0f),
                0.001f,
            )
        }
    }

    @Test
    fun `the result is always inside the native accepted range`() {
        // 无论如何组合密度与系统字体缩放，结果都不得落在原生会静默丢弃的区间外。
        listOf(0.1f, 0.25f, 0.5f, 1f, 2.625f, 3f, 4f, 8f, 16f, 100f).forEach { density ->
            listOf(0.1f, 0.5f, 1f, 1.3f, 2f, 4f, 10f).forEach { fontScale ->
                val scale = coerceSpToPxScale(density, fontScale)
                assertTrue(
                    "density=$density fontScale=$fontScale 得到 $scale，超出原生区间 $nativeAcceptedRange",
                    scale in nativeAcceptedRange,
                )
            }
        }
    }

    @Test
    fun `oversized densities are clamped up to the native ceiling`() {
        // 高密度 + 大系统字体缩放会远超原生上界：须钳到上界而非原样送出
        // （原样送出即被原生静默丢弃，字号上界与实际渲染脱节）。
        assertEquals(8f, coerceSpToPxScale(density = 4f, fontScale = 2f), 0.001f)
        assertEquals(8f, coerceSpToPxScale(density = 100f, fontScale = 100f), 0.001f)
    }

    @Test
    fun `tiny densities are clamped up to the native floor`() {
        // 极低密度（如 0.1）同样越界，须钳到下界。
        assertEquals(0.5f, coerceSpToPxScale(density = 0.1f, fontScale = 1f), 0.001f)
        assertEquals(0.5f, coerceSpToPxScale(density = 0.1f, fontScale = 0.1f), 0.001f)
    }
}
