package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 用户可选字号范围与精度，对标 Termux
 * `TermuxAppSharedPreferences.getDefaultFontSizes`（下限 4dip、上限 256 **像素**、
 * 最小调整步长 2）。
 *
 * 断言的是**与 Termux 的对照结果**（具体档位、像素上界与舍入误差），不是
 * `fontSizeMaxSp`/`fontSizeRangeSteps` 的公式复述——后者只能证明函数等于自身，
 * 无法发现「Kotlin 与原生各有一份上限常量且已漂移」这类真实缺陷。
 */
class FontSizeRangeTest {

    /**
     * Material `steps` 语义：总档数 = steps + 1（不含上界的中间档），
     * 故 `steps + 2` 个离散值、`steps + 1` 个等分区间。逐档列出实际取值，
     * 与 Termux 的「4dip 起、步长 2、256px 止」逐点对照。
     */
    private fun detents(spToPxScale: Float): List<Float> {
        val steps = SettingsRepository.fontSizeRangeSteps(spToPxScale)
        val span = SettingsRepository.fontSizeMaxSp(spToPxScale) - SettingsRepository.FONT_SIZE_MIN_SP
        val interval = span / (steps + 1)
        return (0..steps + 1).map { SettingsRepository.FONT_SIZE_MIN_SP + interval * it }
    }

    @Test
    fun lowerBoundMatchesTermuxMinimum() {
        // Termux 下限是 4dip；本仓必须至少给到同样小的字号，否则用户无法再缩小。
        assertEquals(4f, SettingsRepository.FONT_SIZE_MIN_SP, 0f)
        listOf(1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            assertEquals(
                "spToPxScale=$spToPxScale 的调节条下端必须等于 Termux 的 4dip",
                4f,
                detents(spToPxScale).first(),
                0.001f,
            )
        }
    }

    @Test
    fun detentsAreTermuxStepGrid() {
        // 每档恰好一个 Termux 步长（2sp），首尾分别等于下限与换算后的像素上限档。
        listOf(1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            val values = detents(spToPxScale)
            values.zipWithNext().forEachIndexed { index, (lower, upper) ->
                assertEquals(
                    "spToPxScale=$spToPxScale 第 $index 档步长必须是 ${SettingsRepository.FONT_SIZE_STEP_SP}sp",
                    SettingsRepository.FONT_SIZE_STEP_SP,
                    upper - lower,
                    0.001f,
                )
            }
            assertEquals(
                "spToPxScale=$spToPxScale 的上端必须等于 fontSizeMaxSp",
                SettingsRepository.fontSizeMaxSp(spToPxScale),
                values.last(),
                0.001f,
            )
        }
    }

    @Test
    fun upperBoundStaysUnderTermuxPixelCeilingWithinOneStep() {
        // 上界不得超过 Termux 的 256px；同时因按 sp 步长取整，损失必须小于一个步长
        // （否则「参考 Termux」的误差大于 Termux 自身的调整粒度）。
        listOf(0.75f, 1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            val maxSp = SettingsRepository.fontSizeMaxSp(spToPxScale)
            val maxPx = maxSp * spToPxScale
            assertTrue(
                "spToPxScale=$spToPxScale 时上界 ${maxPx}px 越过 Termux 的 256px",
                maxPx <= SettingsRepository.FONT_SIZE_MAX_PX,
            )
            val lostPx = SettingsRepository.FONT_SIZE_MAX_PX - maxPx
            assertTrue(
                "spToPxScale=$spToPxScale 时上界损失 ${lostPx}px 超过一个步长",
                lostPx < SettingsRepository.FONT_SIZE_STEP_SP * spToPxScale,
            )
            assertTrue(
                "spToPxScale=$spToPxScale 的上界必须高于下限，否则调节条退化",
                maxSp > SettingsRepository.FONT_SIZE_MIN_SP,
            )
        }
    }

    @Test
    fun lowDensityIsNotTruncatedBelowTermuxRange() {
        // 低密度设备上 Termux 的 256px 对应 256/系数 sp。此前 Kotlin 侧另有一份与
        // 原生守卫重复的 100sp 常量，使调节条上界在系数 ≤2.56 的设备上被截到 100sp
        // （系数 1.0 时只有 100px，是 Termux 允许值的四分之一），而原生对超限值
        // 静默丢弃——用户看到的正是「设置条范围与实际可设置范围不一致」。
        // 下列是具体上界（256px 换算后按步长向下取整）。
        assertEquals(256f, SettingsRepository.fontSizeMaxSp(1f), 0.001f)
        assertEquals(128f, SettingsRepository.fontSizeMaxSp(2f), 0.001f)
        assertEquals(340f, SettingsRepository.fontSizeMaxSp(0.75f), 0.001f)
    }

    @Test
    fun systemFontScalingShrinksTheCeiling() {
        // 系统「字体大小」> 1 时字形的实际像素尺寸随之上浮，若上限仍按仅显示密度
        // 计算，用户就能调到远超 Termux 像素上限的字号：实测 fontScale=1.3
        // （系数 2.625→3.4125）时上界须从 96sp 降到 74sp。
        val density = 2.625f
        assertEquals(96f, SettingsRepository.fontSizeMaxSp(density), 0.001f)
        assertEquals(74f, SettingsRepository.fontSizeMaxSp(density * 1.3f), 0.001f)
    }

    @Test
    fun adaptiveDefaultStaysInsideSelectableRange() {
        // 自适应默认值必须落在调节条两端之内，否则初始位置越界。
        listOf(0f, 320f, 360f, 411f, 600f, 900f, 2000f).forEach { widthDp ->
            listOf(1f, 2f, 2.625f, 3f).forEach { spToPxScale ->
                val size = SettingsRepository.defaultFontSizeFor(widthDp)
                assertTrue(
                    "widthDp=$widthDp scale=$spToPxScale 的默认值 $size 越出可选范围",
                    size >= SettingsRepository.FONT_SIZE_MIN_SP &&
                        size <= SettingsRepository.fontSizeMaxSp(spToPxScale),
                )
            }
        }
    }
}