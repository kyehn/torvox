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

            }
        }
    }

    @Test
    fun maxSizeStillShowsEnoughColumnsForACommand() {
        // 上界必须仍容纳得下 MIN_USABLE_COLUMNS 列——这是「范围过大」那条反馈的根因：
        // 此前上界只受 Termux 的 256px 约束，360dp 手机在系数 1.0 时上界 256sp，
        // 一屏只剩 3 列，调节条一路拖到的尽头并不可用。
        //
        // 判据取**具体设备的字面列数**而不是「列数 >= 列数下限」这个与被验公式同款的
        // 构造式——后者恒真，检不出任何缺陷。做法：按上界与该屏宽手算出应显示的列数
        // （等宽字形宽高比 0.6 是字体的外部事实，与实现共用但此处当作已知常量），
        // 与实现给出的上界逐台对照。
        data class Device(val widthDp: Float, val spToPxScale: Float, val expectedColumns: Int)
        listOf(
            // 360dp / 系数 1.0：上界 floor(360/(0.6*20)/2)*2 = 30sp → 360/(30*0.6) = 20 列
            Device(360f, 1f, 20),
            // 411dp / 系数 2.625：Termux 那条 floor(256/2.625/2)*2 = 96sp，列数那条 34sp，
            // 取紧者 34sp → 411/(34*0.6) = 20.1 列
            Device(411f, 2.625f, 20),
            // 800dp 平板 / 系数 2.0：列数那条 floor(800/12/2)*2 = 66sp → 800/(66*0.6) = 20.2 列
            Device(800f, 2f, 20),
            // 1200dp 大屏 / 系数 1.5：列数那条 100sp，Termux 那条 170sp →
            // 100sp → 1200/(100*0.6) = 20 列
            Device(1200f, 1.5f, 20),
        ).forEach { device ->
            val maxSp = SettingsRepository.fontSizeMaxSp(device.spToPxScale, device.widthDp)
            val columns = device.widthDp / (maxSp * SettingsRepository.MONOSPACE_CHAR_ASPECT)
            assertEquals(
                "${device.widthDp}dp / 系数 ${device.spToPxScale} 时上界 $maxSp 应给出 " +
                    "${device.expectedColumns} 列（等宽字形宽高比 ${SettingsRepository.MONOSPACE_CHAR_ASPECT}）",
                device.expectedColumns.toFloat(),
                columns,
                0.55f,
            )
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
    fun adaptiveDefaultIsAlwaysReachableOnTheSlider() {
        // 窄屏上「至少 MIN_USABLE_COLUMNS 列」可能低于 ADAPTIVE_DEFAULT_MIN_SP
        // （那个下限的存在正是为了让小屏初始字号仍可读）。若上界不跟着抬，
        // 全新安装得到的默认字号就落在调节条之外——同一类「范围与实际可设范围
        // 不一致」，只是换到了另一端。
        listOf(0f, 120f, 150f, 200f, 320f, 360f, 480f, 900f, 2000f).forEach { widthDp ->
            listOf(0.75f, 1f, 2.625f, 4f).forEach { spToPxScale ->
                val maxSp = SettingsRepository.fontSizeMaxSp(spToPxScale, widthDp)
                val defaultSp = SettingsRepository.defaultFontSizeFor(widthDp)
                assertTrue(
                    "widthDp=$widthDp 系数=$spToPxScale 时上界 $maxSp 低于默认值 $defaultSp",
                    maxSp >= defaultSp,
                )
            }
        }
    }

    @Test
    fun usableCeilingTracksTheScreenWidth() {
        // 逐宽度断**具体上界值**，不是「单调不减」：单调性对恒值同样成立，
        // 列数约束被整条删掉后（上界退化为 Termux 的 256px 恒值）它仍全绿，
        // 检不出这条约束是否还在。字面值表才是外部锚点。
        // 取系数 1.0 使 Termux 那条恒为 256sp（比列数那条宽松），于是测的就是列数约束。
        listOf(
            // 320dp → floor(320/12/2)*2 = 26sp
            320f to 26f,
            // 360dp → floor(360/12/2)*2 = 30sp
            360f to 30f,
            // 411dp → floor(411/12/2)*2 = 34sp
            411f to 34f,
            // 480dp → floor(480/12/2)*2 = 40sp
            480f to 40f,
            // 600dp → floor(600/12/2)*2 = 50sp
            600f to 50f,
            // 900dp → floor(900/12/2)*2 = 74sp
            900f to 74f,
            // 1200dp → floor(1200/12/2)*2 = 100sp
            1200f to 100f,
            // 2000dp → floor(2000/12/2)*2 = 166sp
            2000f to 166f,
        ).forEach { (widthDp, expectedCeiling) ->
            assertEquals(
                "widthDp=$widthDp 的上界（系数 1.0，等宽字形宽高比 " +
                    "${SettingsRepository.MONOSPACE_CHAR_ASPECT}，至少 " +
                    "${SettingsRepository.MIN_USABLE_COLUMNS} 列）",
                expectedCeiling,
                SettingsRepository.fontSizeMaxSp(1f, widthDp),
                0.001f,
            )
        }
    }
}
