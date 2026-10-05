package terminal.emulator.cucumber

import io.cucumber.java.After
import io.cucumber.java.Before
import io.cucumber.java.Scenario
import terminal.emulator.MainActivity
import javax.inject.Inject

/**
 * 场景收尾还原持久化设置。
 *
 * `:app:connectedDebugAndroidTest` 的全部用例跑在**同一个** instrumentation
 * 进程里（无 orchestrator、无 `forkEvery`），主题名等 DataStore 设置跨场景、
 * 跨测试类存活。场景改了主题却不还原，其后所有按颜色与网格断言的用例都在
 * 陌生主题下运行——本地单跑该场景恒绿，整类连跑才判红（与字体族泄漏同族）。
 */
class Hooks
@Inject
constructor(private val composeRuleHolder: ComposeRuleHolder) {
    private var themeName = ""

    @Before
    fun setUp(scenario: Scenario) {
        composeRuleHolder.composeRule.waitForIdle()
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity: MainActivity ->
            themeName = activity.terminalViewModel.settings.value.themeName
        }
    }

    @After
    fun tearDown(scenario: Scenario) {
        composeRuleHolder.composeRule.activityRule.scenario.onActivity { activity: MainActivity ->
            if (activity.terminalViewModel.settings.value.themeName != themeName) {
                activity.terminalViewModel.setThemeName(themeName)
            }
        }
    }
}
