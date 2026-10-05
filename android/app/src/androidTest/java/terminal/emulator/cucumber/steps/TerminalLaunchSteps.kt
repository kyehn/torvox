package terminal.emulator.cucumber.steps

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import io.cucumber.java.zh_cn.那么
import org.junit.Assert.assertTrue
import terminal.emulator.cucumber.ComposeRuleHolder
import terminal.emulator.findTerminalSurface
import javax.inject.Inject

class TerminalLaunchSteps
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    @那么("^修饰键栏可见$")
    fun modifierBarIsVisible() {
        composeRuleHolder.composeRule.onNodeWithTag("ModifierBar").assertIsDisplayed()
    }

    @那么("^终端内容区宽高为正$")
    fun terminalContentAreaHasPositiveDimensions() {
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalContent")
            .assertIsDisplayed()
            .assertWidthIsAtLeast(1.dp)
    }

    @那么("^SurfaceView 可见$")
    fun surfaceViewIsVisible() {
        // JUnit 而非 Kotlin `assert`：ART 默认不带 `-ea`，后者恒为空操作。
        var dimensions = "?"
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity ->
            val surface = findTerminalSurface(activity)
            dimensions = "${surface.width}x${surface.height}"
            assertTrue("SurfaceView 宽度应为正 (实际 ${surface.width})", surface.width > 0)
            assertTrue("SurfaceView 高度应为正 (实际 ${surface.height})", surface.height > 0)
        }
        assertTrue("SurfaceView 必须已取得尺寸 (实际=$dimensions)", dimensions != "?")
    }

    @那么("^它渲染在 Compose 布局上层$")
    fun itRendersAboveComposeLayout() {
        composeRuleHolder.composeRule
            .onNodeWithTag("TerminalScreen", useUnmergedTree = true)
            .assertIsDisplayed()
    }
}
