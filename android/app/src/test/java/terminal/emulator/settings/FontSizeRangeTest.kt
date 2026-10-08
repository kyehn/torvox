package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 用户可选字号范围与精度的换算：取自 Termux
 * `TermuxAppSharedPreferences.getDefaultFontSizes`（下限 4dip、上限 256 像素、
 * 最小调整步长 2），换算到 sp 后每档必须恰好相差一个步长。
 */
class FontSizeRangeTest {

    @Test
    fun rangeCoversTermuxLowerBound() {
        // Termux 下限是 4dip；本仓必须至少给到同样小的字号，否则用户无法再缩小。
        assertEquals(4f, SettingsRepository.FONT_SIZE_MIN_SP, 0f)
    }

    @Test
    fun everyDetentIsExactlyOneStep() {
        // 档数必须让跨度恰好被步长整除，否则调节条会出现半档。
        listOf(1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { density ->
            val maxSp = SettingsRepository.fontSizeRangeMaxSp(density)
            val steps = SettingsRepository.fontSizeRangeSteps(density)
            val span = maxSp - SettingsRepository.FONT_SIZE_MIN_SP
            assertEquals(
                "density=$density 的跨度未被步长整除",
                SettingsRepository.FONT_SIZE_STEP_SP,
                span / (steps + 1),
                0.001f,
            )
        }
    }

    @Test
    fun rangeRespectsTermuxPixelCeiling() {
        // 换算后的 sp 上界不得越过 Termux 的像素上限，否则比 Termux 允许的还大。
        listOf(1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { density ->
            val maxSp = SettingsRepository.fontSizeRangeMaxSp(density)
            assertTrue(
                "density=$density 时上界 ${maxSp * density}px 越过 Termux 的 ${SettingsRepository.FONT_SIZE_MAX_PX}px",
                maxSp * density <= SettingsRepository.FONT_SIZE_MAX_PX,
            )
            assertTrue(
                "density=$density 时上界必须高于下限，否则调节条退化",
                maxSp > SettingsRepository.FONT_SIZE_MIN_SP,
            )
        }
    }

    @Test
    fun adaptiveDefaultStaysInsideSelectableRange() {
        // 自适应默认值必须落在用户可选范围内，否则调节条初始位置会越界。
        listOf(0f, 320f, 360f, 411f, 600f, 900f, 2000f).forEach { widthDp ->
            val size = SettingsRepository.defaultFontSizeFor(widthDp)
            assertTrue(
                "widthDp=$widthDp 的默认值 $size 越出可选范围",
                size >= SettingsRepository.FONT_SIZE_MIN_SP &&
                    size <= SettingsRepository.fontSizeRangeMaxSp(2.625f),
            )
        }
    }

    @Test
    fun effectiveMaxNeverExceedsNativeClamp() {
        // 有效上界是“可实际设置”的上界：不得超过原生 4.0..100.0 钳位，
        // 否则调节条位置与实际渲染脱节（低密度设备上滑块可拖到被静默丢弃的值）。
        listOf(0.75f, 1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { density ->
            val effective = SettingsRepository.effectiveFontSizeMaxSp(density)
            assertTrue(
                "density=$density 的有效上界 $effective 越过原生钳位",
                effective <= SettingsRepository.NATIVE_FONT_SIZE_MAX_SP,
            )
            assertTrue(
                "density=$density 的有效上界 $effective 必须高于下限",
                effective > SettingsRepository.FONT_SIZE_MIN_SP,
            )
            assertEquals(
                "density=$density 的有效上界必须取小者",
                minOf(
                    SettingsRepository.fontSizeRangeMaxSp(density),
                    SettingsRepository.NATIVE_FONT_SIZE_MAX_SP,
                ),
                effective,
                0.001f,
            )
        }
    }

    @Test
    fun effectiveDetentsStayExact() {
        // 有效区间替换调节条区间后，每档仍须恰好相差一个步长，否则出现半档。
        listOf(0.75f, 1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { density ->
            val maxSp = SettingsRepository.effectiveFontSizeMaxSp(density)
            val steps = SettingsRepository.effectiveFontSizeRangeSteps(density)
            val span = maxSp - SettingsRepository.FONT_SIZE_MIN_SP
            assertEquals(
                "density=$density 的有效跨度未被步长整除",
                SettingsRepository.FONT_SIZE_STEP_SP,
                span / (steps + 1),
                0.001f,
            )
        }
    }

    @Test
    fun adaptiveDefaultStaysInsideEffectiveRange() {
        // 自适应默认值必须落在有效区间内，否则首次渲染字号越界。
        listOf(0f, 320f, 360f, 411f, 600f, 900f, 2000f).forEach { widthDp ->
            val size = SettingsRepository.defaultFontSizeFor(widthDp)
            assertTrue(
                "widthDp=$widthDp 的默认值 $size 越出有效区间",
                size >= SettingsRepository.FONT_SIZE_MIN_SP &&
                    size <= SettingsRepository.effectiveFontSizeMaxSp(2.625f),
            )
        }
    }
}
