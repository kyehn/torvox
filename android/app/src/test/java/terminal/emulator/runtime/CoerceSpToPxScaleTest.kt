package terminal.emulator.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import terminal.emulator.settings.SettingsRepository

/**
 * sp→px 系数（[coerceSpToPxScale]）的换算与钳位。
 *
 * 该系数同时是字号的换算基准与推给原生 `setRasterScale` 的值，两者 MUST 同源：
 * 字形实际光栅尺度即 `sp × 系数`。钳位区间 MUST 与原生接受区间一致——区间外的值
 * 被原生拒收并记错误日志，会使字号上界与实际渲染脱节。
 *
 * 区间**不在本测试里再抄一份**：生产与原生导出用的是同一常量
 * （`NativeBridge.getRasterScaleRange` ← Rust `RASTER_SCALE_MIN/MAX`），此处若写死
 * `0.5f..8f`，改原生而忘改测试仍会全绿——那正是本用例一度沦为自验证的形态。
 * 本文件的职责因此收窄为「钳位逻辑本身」：给定任意外部区间，乘积必须落进它，
 * 且不得改动区间内的值。区间本身的真伪由 `NativeBridgeSmokeTest` 从原生读回断言。
 */
class CoerceSpToPxScaleTest {

    /** 与真机一致的一段区间（端点取自真机日志：density 2.625 × fontScale 1.3）。 */
    private val deviceRange = 0.5f..8.0f

    /**
     * 另一段刻意不同的区间。
     *
     * 它是本文件的核心判据：钳位点必须来自入参而非写死的端点——只要实现里还残留
     * 任何一份 Kotlin 自带的字面量，用这一段区间就会立刻算出别的值而判红。
     */
    private val reportedRange = 1.5f..6.0f

    @Test
    fun `scale is the product of density and the system font scaling`() {
        assertEquals(2.625f, coerceSpToPxScale(2.625f, 1.0f, deviceRange), 0.001f)
        assertEquals(3.4125f, coerceSpToPxScale(2.625f, 1.3f, deviceRange), 0.001f)
    }

    @Test
    fun `the system font scaling is what keeps the ceiling from being exceeded`() {
        // 缺陷根因：上界按仅密度（2.625）算出 96sp，但字形实际按
        // 密度 × 系统字体缩放（3.4125）光栅化，故 96sp 实际是 327.6px，
        // 越过 Termux 的 256px 上限。计入系统字体缩放后上界为 74sp，
        // 实际 252.5px 回到上限内。
        val densityOnly = coerceSpToPxScale(2.625f, 1.0f, deviceRange)
        val withFontScaling = coerceSpToPxScale(2.625f, 1.3f, deviceRange)
        assertEquals(2.625f, densityOnly, 0.001f)
        assertEquals(3.4125f, withFontScaling, 0.001f)

        val sliderMaxSpDensityOnly = SettingsRepository.fontSizeMaxSp(densityOnly)
        val sliderMaxSpWithFontScaling = SettingsRepository.fontSizeMaxSp(withFontScaling)
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
                coerceSpToPxScale(scale, 1.0f, deviceRange),
                0.001f,
            )
        }
    }

    @Test
    fun `the result is always inside whatever range the native side reports`() {
        // 无论如何组合密度与系统字体缩放，结果都不得落在原生会拒收的区间外。
        listOf(0.1f, 0.25f, 0.5f, 1f, 2.625f, 3f, 4f, 8f, 16f, 100f).forEach { density ->
            listOf(0.1f, 0.5f, 1f, 1.3f, 2f, 4f, 10f).forEach { fontScale ->
                val scale = coerceSpToPxScale(density, fontScale, reportedRange)
                assertTrue(
                    "density=$density fontScale=$fontScale 得到 $scale，超出原生区间 $reportedRange",
                    scale in reportedRange,
                )
            }
        }
    }

    @Test
    fun `oversized densities are clamped to the reported ceiling`() {
        // 高密度 + 大系统字体缩放会远超原生上界：须钳到上界而非原样送出
        // （原样送出即被原生拒收，字号上界与实际渲染脱节）。
        assertEquals(8f, coerceSpToPxScale(4f, 2f, deviceRange), 0.001f)
        assertEquals(8f, coerceSpToPxScale(100f, 100f, deviceRange), 0.001f)
        // 上界取自外部区间时钳位点随之改变——证明钳的是区间而非固定端点。
        assertEquals(6f, coerceSpToPxScale(100f, 100f, reportedRange), 0.001f)
    }

    @Test
    fun `tiny densities are clamped up to the reported floor`() {
        // 极低密度（如 0.1）同样越界，须钳到下界。
        assertEquals(0.5f, coerceSpToPxScale(0.1f, 1f, deviceRange), 0.001f)
        assertEquals(0.5f, coerceSpToPxScale(0.1f, 0.1f, deviceRange), 0.001f)
        assertEquals(1.5f, coerceSpToPxScale(0.1f, 0.1f, reportedRange), 0.001f)
    }
}
