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
    fun rangeRespectsTermuxPixelCeiling() {
        // 换算后的 sp 上界不得越过 Termux 的像素上限，否则比 Termux 允许的还大。
        listOf(1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            val maxSp = SettingsRepository.fontSizeRangeMaxSp(spToPxScale)
            assertTrue(
                "spToPxScale=$spToPxScale 时上界 ${maxSp * spToPxScale}px 越过 Termux 的 ${SettingsRepository.FONT_SIZE_MAX_PX}px",
                maxSp * spToPxScale <= SettingsRepository.FONT_SIZE_MAX_PX,
            )
            assertTrue(
                "spToPxScale=$spToPxScale 时上界必须高于下限，否则调节条退化",
                maxSp > SettingsRepository.FONT_SIZE_MIN_SP,
            )
        }
    }

    @Test
    fun systemFontScalingShrinksTheCeiling() {
        // 系统「字体大小」> 1 时字形的实际像素尺寸随之上浮，若上限仍按仅显示密度
        // 计算，用户就能调到远超 Termux 像素上限的字号：实测 fontScale=1.3
        // （系数 2.625→3.412）时上界须从 96sp 降到 74sp。
        val density = 2.625f
        assertEquals(96f, SettingsRepository.fontSizeRangeMaxSp(density), 0.001f)
        assertEquals(
            74f,
            SettingsRepository.fontSizeRangeMaxSp(density * 1.3f),
            0.001f,
        )
        assertTrue(
            SettingsRepository.fontSizeRangeMaxSp(density * 1.3f) <
                SettingsRepository.fontSizeRangeMaxSp(density),
        )
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
        listOf(0.75f, 1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            val effective = SettingsRepository.effectiveFontSizeMaxSp(spToPxScale)
            assertTrue(
                "spToPxScale=$spToPxScale 的有效上界 $effective 越过原生钳位",
                effective <= SettingsRepository.NATIVE_FONT_SIZE_MAX_SP,
            )
            assertTrue(
                "spToPxScale=$spToPxScale 的有效上界 $effective 必须高于下限",
                effective > SettingsRepository.FONT_SIZE_MIN_SP,
            )
            assertEquals(
                "spToPxScale=$spToPxScale 的有效上界必须取小者",
                minOf(
                    SettingsRepository.fontSizeRangeMaxSp(spToPxScale),
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
        // 含 0.75（低于 1）：系数下界附近同样须整除。
        listOf(0.75f, 1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            val maxSp = SettingsRepository.effectiveFontSizeMaxSp(spToPxScale)
            val steps = SettingsRepository.effectiveFontSizeRangeSteps(spToPxScale)
            val span = maxSp - SettingsRepository.FONT_SIZE_MIN_SP
            assertEquals(
                "spToPxScale=$spToPxScale 的有效跨度未被步长整除",
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
