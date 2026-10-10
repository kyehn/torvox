package terminal.emulator.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.roundToInt

/**
 * 用户可选字号范围与精度。
 *
 * 断言的是**设备上的真实结果**（具体档位、能显示多少列），不是
 * `fontSizeMaxSp`/`fontSizeRangeSteps` 的公式复述——后者只能证明函数等于自身，
 * 无法发现「Kotlin 与原生各有一份上限常量且已漂移」这类真实缺陷。
 */
class FontSizeRangeTest {

    /**
     * Material `steps` 语义：总档数 = steps + 1（不含上界的中间档），
     * 故 `steps + 2` 个离散值、`steps + 1` 个等分区间。逐档列出实际取值。
     */
    private fun detents(spToPxScale: Float, screenWidthDp: Float): List<Float> {
        val steps = SettingsRepository.fontSizeRangeSteps(spToPxScale, screenWidthDp)
        val span = SettingsRepository.fontSizeMaxSp(spToPxScale, screenWidthDp) - SettingsRepository.FONT_SIZE_MIN_SP
        val interval = span / (steps + 1)
        return (0..steps + 1).map { SettingsRepository.FONT_SIZE_MIN_SP + interval * it }
    }

    @Test
    fun lowerBoundMatchesTermuxMinimum() {
        // Termux 下限是 4dip；本仓必须至少给到同样小的字号，否则用户无法再缩小。
        assertEquals(4f, SettingsRepository.FONT_SIZE_MIN_SP, 0f)
    }

    @Test
    fun detentsStartAtTermuxMinimumOnAStepGrid() {
        // 逐点照 Termux 的档位表（4dip 起、步长 2sp）核对，而不是断言「步长恒等于
        // FONT_SIZE_STEP_SP」——档数按 span/STEP 反推，那条断言构造本身恒真，检不出缺陷。
        // 360dp 手机、系数 2.625：上界 30sp（可用性那条更紧），共 14 档。
        val phone = detents(2.625f, 360f)
        assertEquals(4f, phone.first(), 0.001f)
        assertEquals(30f, phone.last(), 0.001f)
        assertEquals(14, phone.size)
        assertEquals(
            listOf(4f, 6f, 8f, 28f, 30f),
            listOf(phone[0], phone[1], phone[2], phone[phone.size - 2], phone.last()),
        )
        // 每档都落在 4dip 起、步长 2sp 的网格上：任一档偏离即为错位。
        listOf(0.75f, 1f, 1.5f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            detents(spToPxScale, 360f).forEach { sp ->
                val steps = (sp - SettingsRepository.FONT_SIZE_MIN_SP) / 2f
                assertEquals(
                    "spToPxScale=$spToPxScale 的档位 $sp 不在 4dip 起、步长 2sp 的网格上",
                    steps,
                    steps.roundToInt().toFloat(),
                    0.001f,
                )
            }
        }
    }

    @Test
    fun upperBoundNeverPassesTermuxPixelCeiling() {
        // 上界不得超过 Termux 的 256px：这是 DESIGN 要求「参考 Termux」的那条约束，
        // 在可用性上界更宽松的设备（大屏/高密度）上它就是更紧的那条。
        listOf(0.75f, 1f, 1.5f, 2f, 2.625f, 3f, 4f).forEach { spToPxScale ->
            listOf(360f, 411f, 600f, 900f, 2000f).forEach { widthDp ->
                val maxSp = SettingsRepository.fontSizeMaxSp(spToPxScale, widthDp)
                assertTrue(
                    "spToPxScale=$spToPxScale widthDp=$widthDp 时上界 ${maxSp * spToPxScale}px 越过 Termux 的 256px",
                    maxSp * spToPxScale <= SettingsRepository.FONT_SIZE_MAX_PX,
                )
                assertTrue(
                    "spToPxScale=$spToPxScale widthDp=$widthDp 的上界必须高于下限，否则调节条退化",
                    maxSp > SettingsRepository.FONT_SIZE_MIN_SP,
                )
            }
        }
    }

    @Test
    fun maxSizeStillShowsEnoughColumnsForACommand() {
        // 上界必须仍容纳得下 MIN_USABLE_COLUMNS 列——这是「范围过大」那条反馈的根因：
        // 此前上界只受 Termux 的 256px 约束，360dp 手机在系数 1.0 时上界 256sp，
        // 一屏只剩 3 列，调节条一路拖到的尽头并不可用。
        // 列数 = 屏宽 dp ÷ (字号 sp × 字形宽高比)（密度相约，与 defaultFontSizeFor 同口径）。
        listOf(320f, 360f, 411f, 600f, 900f, 2000f).forEach { widthDp ->
            listOf(0.75f, 1f, 2f, 2.625f, 4f).forEach { spToPxScale ->
                val maxSp = SettingsRepository.fontSizeMaxSp(spToPxScale, widthDp)
                val columns = widthDp / (maxSp * SettingsRepository.MONOSPACE_CHAR_ASPECT)
                // 容一个步长：上界按步长向下取整，最多损失一个步长的字号，
                // 对应列数只会略高于语义下限而不是低于一个步长的量。
                assertTrue(
                    "widthDp=$widthDp 系数=$spToPxScale 时上界 $maxSp 只剩 $columns 列，" +
                        "低于 ${SettingsRepository.MIN_USABLE_COLUMNS} 列一个步长以上",
                    columns >= SettingsRepository.MIN_USABLE_COLUMNS - SettingsRepository.FONT_SIZE_STEP_SP,
                )
            }
        }
    }

    @Test
    fun systemFontScalingShrinksTheCeiling() {
        // 系统「字体大小」> 1 时字形的实际像素尺寸随之上浮，若上限仍按仅显示密度
        // 计算，用户就能调到远超 Termux 像素上限的字号：fontScale=1.3
        // （系数 2.625→3.4125）时 Termux 那条上界须从 96sp 降到 74sp。
        // 屏宽取 1200dp：可用性上界 100sp 在两种系数下都更宽松，
        // 故收紧必须可见（窄屏幕上会是可用性那条先触顶，收紧被它掩盖）。
        val density = 2.625f
        val screenWidthDp = 1200f
        assertEquals(96f, SettingsRepository.fontSizeMaxSp(density, screenWidthDp), 0.001f)
        assertEquals(74f, SettingsRepository.fontSizeMaxSp(density * 1.3f, screenWidthDp), 0.001f)
    }

    @Test
    fun adaptiveDefaultStaysInsideSelectableRange() {
        // 自适应默认值只取决于屏宽，与 sp→px 系数无关；故对整个系数域取最紧上界
        // 一次判定，不逐个系数重复同一条断言。`fontSizeMaxSp` 随系数单调不增，
        // 最紧处在系数域上端 8.0。
        val tightestCeiling = SettingsRepository.fontSizeMaxSp(8f, 360f)
        listOf(0f, 320f, 360f, 411f, 600f, 900f, 2000f).forEach { widthDp ->
            val size = SettingsRepository.defaultFontSizeFor(widthDp)
            assertTrue(
                "widthDp=$widthDp 的默认值 $size 低于下限 ${SettingsRepository.FONT_SIZE_MIN_SP}",
                size >= SettingsRepository.FONT_SIZE_MIN_SP,
            )
            assertTrue(
                "widthDp=$widthDp 的默认值 $size 越出最紧可选上界 $tightestCeiling",
                size <= tightestCeiling,
            )
        }
    }

    @Test
    fun usableCeilingGrowsWithScreenWidth() {
        // 上界必须随屏宽单调不减：更宽的屏幕本就放得下更多列，若上界不随之放宽，
        // 大屏用户会在小字号上被无谓地卡住（调节条上界比屏幕允许的更小）。
        var previous = 0f
        listOf(320f, 360f, 411f, 480f, 600f, 720f, 900f, 1200f, 2000f).forEach { widthDp ->
            val ceiling = SettingsRepository.fontSizeMaxSp(1f, widthDp)
            assertTrue(
                "widthDp=$widthDp 的上界 $ceiling 小于更窄屏幕的上界 $previous",
                ceiling >= previous,
            )
            previous = ceiling
        }
    }
}
