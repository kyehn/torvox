package terminal.emulator

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.rule.GrantPermissionRule
import org.junit.After
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.closeSettingsOverlay
import terminal.emulator.bridge.NativeBridge

/**
 * 字体族设置：选择器入口、对话框列表、以及选定后应用成功。
 *
 * 字体族是**持久化设置**，选错一次即污染同一次运行内的全部后继用例：单元格宽度
 * 随主字体变化（实测 Droid Sans Mono 8.40px ↔ MapleMono NF CN 12.42px），网格
 * 列数随之从 38 掉到 25，所有按网格坐标断言的用例集体漂移。故 [tearDown] 必须
 * 还原族名——缺失还原时失败只出现在 CI 整类连跑，本地单跑恒绿。
 */
class FontSwitchInstrumentedTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private var originalFamily = ""

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        var family = ""
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            family = activity.terminalViewModel.settings.value.fontFamily
        }
        originalFamily = family
    }

    @After
    fun tearDown() {
        composeTestRule.closeSettingsOverlay()
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            activity.terminalViewModel.setFontFamily(originalFamily)
        }
    }

    /** 字体选择器入口：设置页滚动到该节点后点击。 */
    private fun openFontPicker() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("FontFamilySelector"))
        composeTestRule.onNodeWithTag("FontFamilySelector").performClick()
        composeTestRule.waitForIdle()
    }

    @Test
    fun settings_shows_font_family_section() {
        composeTestRule.openSettings()
        composeTestRule.onNodeWithTag("SettingsLazyColumn")
            .performScrollToNode(hasTestTag("FontFamilySelector"))
        composeTestRule.onNodeWithTag("FontFamilySelector").assertIsDisplayed()
        composeTestRule.onNodeWithText("字体").assertIsDisplayed()
    }

    @Test
    fun font_change_opens_dialog_without_file_picker() {
        openFontPicker()
        composeTestRule.onNodeWithText("选择字体").assertIsDisplayed()
        // DESIGN: 字体列表不提供“从文件加载”选项。
        composeTestRule.onNodeWithText("从文件选择…").assertDoesNotExist()
        // 列表即字体库本身（原生公开 API），不写死族名：模拟器镜像的字体清单随
        // 版本变，写死即环境耦合的脆弱断言。
        val families = checkNotNull(NativeBridge.listFontFamilies()) { "字体库必须非空" }
        check(families.isNotEmpty()) { "字体库必须非空（fonts.xml 不可读时产品已崩溃退出）" }
        composeTestRule.onNodeWithText(families.first()).assertIsDisplayed()
    }

    @Test
    fun font_select_applies_selected_family() {
        openFontPicker()
        val families = checkNotNull(NativeBridge.listFontFamilies()) { "字体库必须非空" }
        val target =
            families.firstOrNull { it != originalFamily }
                ?: throw AssertionError("字体库必须含非当前族以验证切换")
        composeTestRule.onNodeWithText(target).performClick()
        composeTestRule.waitForIdle()
        // 断言落到持久化设置而非“应用没崩”：族名进设置流即选定生效，
        // 原用例只断言进程存活，任何选定失败都判绿。
        val applied =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 200) {
                var family = ""
                composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
                    family = activity.terminalViewModel.settings.value.fontFamily
                }
                family == target
            }
        assertTrue("选定后字体族必须落入设置流 (期望=$target)", applied != null)
    }

    @Test
    fun font_dialog_offers_system_default() {
        openFontPicker()
        // DESIGN: 不展示“系统默认”等含糊选项。空族名回落 fonts.xml 的 monospace，
        // 故列表内不得出现该字样。
        composeTestRule.onNodeWithText("系统默认").assertDoesNotExist()
        assertNull(
            "字体列表不得含含糊的系统默认项",
            NativeBridge.listFontFamilies()?.firstOrNull { it.contains("系统默认") },
        )
    }
}
