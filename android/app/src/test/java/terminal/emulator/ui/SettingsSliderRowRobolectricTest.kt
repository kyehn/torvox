package terminal.emulator.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.performSemanticsAction
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * [SettingsSliderRow] 的 JVM 回归测试。
 *
 * Material3 1.5 起滑块由 `SliderState` 承载刻度与区间，`rememberSliderState(value=…)` 只把 value 当作
 * 初值：外部值变化不会自动进入状态，拇指会停在旧值。这里锁死「外部值写回状态」与「区间/步数变化
 * 时重建状态」两条行为，防止再次退回只传初值的写法。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SettingsSliderRowRobolectricTest {

    @get:Rule val composeRule = createComposeRule()

    private val sliderMatcher = SemanticsMatcher.keyIsDefined(SemanticsProperties.ProgressBarRangeInfo)

    private val colors =
        SettingsColors(
            textColor = Color.White,
            secondaryText = Color.LightGray,
            accentColor = Color.Cyan,
            cardBackground = Color.Transparent,
        )

    /** 读取滑块当前的语义信息（值、区间、步数）。 */
    private fun sliderRangeInfo(): ProgressBarRangeInfo {
        val info =
            composeRule.onNode(sliderMatcher)
                .fetchSemanticsNode()
                .config.getOrNull(SemanticsProperties.ProgressBarRangeInfo)
        return requireNotNull(info) { "滑块节点缺少 ProgressBarRangeInfo 语义" }
    }

    /** 挂一个滑块行；外部值由 [valueHolder] 持有，便于测试中改写。 */
    private fun setSliderRow(
        valueHolder: MutableState<Float>,
        range: ClosedFloatingPointRange<Float>,
        steps: Int,
        enabled: Boolean = true,
        onValueChange: (Float) -> Unit = {},
    ) {
        composeRule.setContent {
            MaterialTheme {
                SettingsSliderRow(
                    title = "字号",
                    value = valueHolder.value,
                    valueRange = range,
                    steps = steps,
                    colors = colors,
                    enabled = enabled,
                    onValueChange = onValueChange,
                )
            }
        }
    }

    @Test
    fun `external value change moves the slider thumb`() {
        val value = mutableStateOf(12f)
        setSliderRow(value, range = 8f..24f, steps = 0)

        assertEquals(12f, sliderRangeInfo().current, 0f)

        value.value = 18f
        composeRule.waitForIdle()

        assertEquals(18f, sliderRangeInfo().current, 0f)
    }

    @Test
    fun `slider reports user changes through onValueChange`() {
        val value = mutableStateOf(12f)
        val changes = mutableListOf<Float>()
        setSliderRow(value, range = 8f..24f, steps = 0, onValueChange = { changes += it })

        composeRule.onNode(sliderMatcher)
            .performSemanticsAction(SemanticsActions.SetProgress) { action -> action(20f) }
        composeRule.waitForIdle()

        assertEquals(listOf(20f), changes)
        assertEquals(20f, sliderRangeInfo().current, 0f)
    }

    @Test
    fun `range and step change rebuild the slider state`() {
        val value = mutableStateOf(12f)
        val range = mutableStateOf(8f..24f)
        val steps = mutableStateOf(0)
        composeRule.setContent {
            MaterialTheme {
                SettingsSliderRow(
                    title = "字号",
                    value = value.value,
                    valueRange = range.value,
                    steps = steps.value,
                    colors = colors,
                    onValueChange = {},
                )
            }
        }
        assertEquals(8f..24f, sliderRangeInfo().range)
        assertEquals(0, sliderRangeInfo().steps)

        range.value = 8f..16f
        steps.value = 7
        composeRule.waitForIdle()

        val info = sliderRangeInfo()
        assertEquals(8f..16f, info.range)
        assertEquals(7, info.steps)
        assertEquals(12f, info.current, 0f)
    }

    @Test
    fun `disabled slider ignores value changes`() {
        val value = mutableStateOf(12f)
        val changes = mutableListOf<Float>()
        setSliderRow(value, range = 8f..24f, steps = 0, enabled = false, onValueChange = { changes += it })

        composeRule.onNode(sliderMatcher)
            .performSemanticsAction(SemanticsActions.SetProgress) { action -> action(20f) }
        composeRule.waitForIdle()

        assertTrue(changes.isEmpty())
        assertEquals(12f, sliderRangeInfo().current, 0f)
    }
}
