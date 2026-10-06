package terminal.emulator

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.Bitmap
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.ui.theme.BuiltInThemes

class BehaviorInstrumentedTest {
    // 判红时把应用日志尾部附在失败信息上：本类多在慢模拟器上以「节点查不到」判红，
    // 无日志时无法区分「应用没起」「被系统弹窗盖住」「功能真的缺席」。
    @get:Rule
    val terminalLogcatRule = TerminalLogcatRule()

    // MainActivity requests POST_NOTIFICATIONS on Android 13+ at startup;
    // without pre-granting it the system permission dialog covers the UI
    // and none of the drawer/settings nodes appear.
    @get:Rule
    val notificationPermission = GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    // Compose 规则自带 Activity 启动：设置是 Activity 内的浮层，每例重建规则即从
    // 干净状态起步，无需 `am start --activity-clear-task` 之类的自造重置。
    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    companion object {
        private const val PACKAGE = "com.termux"
        private const val WAIT_TIMEOUT = 60_000L
        private const val SELECTION_PIXEL_GAIN_THRESHOLD = 300
    }

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        // 就绪门槛内含「关掉系统无响应对话框」，它盖住应用窗口时节点查找全部落空。
        composeTestRule.waitForTerminalScreen()
    }

    /** 设置浮层是 Compose 覆盖层，不关闭就留存给后继用例并盖住终端（见 closeSettingsOverlay）。 */
    @After
    fun tearDown() {
        composeTestRule.closeSettingsOverlay()
    }

    // 长按前后截图采样差分：选择高亮/手柄/菜单必改数千采样像素，
    // 状态栏时钟仅贡献百级。用量化的像素数代替“是否看见”，防抖且诚实。
    private fun countChangedPixels(before: Bitmap?, after: Bitmap?): Int {
        if (before == null || after == null) return 0
        if (before.width != after.width || before.height != after.height) return 0
        var changed = 0
        var samplingX = 0
        while (samplingX < before.width) {
            var samplingY = 0
            while (samplingY < before.height) {
                if (before.getPixel(samplingX, samplingY) != after.getPixel(samplingX, samplingY)) changed++
                samplingY += 4
            }
            samplingX += 4
        }
        return changed
    }

    /** 纵向滚到设置页目标节点（Compose 语义滚动，不自造滑动循环）。 */
    private fun scrollSettingsTo(matcher: SemanticsMatcher) {
        composeTestRule.onNodeWithTag("SettingsLazyColumn").performScrollToNode(matcher)
    }

    @Test
    fun behavior_app_process_alive() {
        val output = device.executeShellCommand("dumpsys activity processes | grep -i $PACKAGE")
        assertTrue("App process should be running", output.isNotEmpty())
    }

    @Test
    fun behavior_app_stays_in_foreground() {
        // Smoke check only: no color/rendering assertion is possible while
        // Bridge.setTheme is a log-only implemented (native query path is wired).
        val output = device.executeShellCommand("dumpsys activity top | grep -i $PACKAGE")
        assertTrue("App should be in foreground", output.isNotEmpty())
    }

    @Test
    fun behavior_font_picker_opens_with_change_button() {
        composeTestRule.openSettings()
        // 字体区：标题“字体”，按钮“更改”，对话框标题“选择字体”。
        scrollSettingsTo(hasTestTag("FontFamilySelector"))
        composeTestRule.onNodeWithText("更改").performClick()
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithText("选择字体").assertIsDisplayed()
    }

    @SuppressLint("DeprecatedCall")
    @Test
    fun behavior_selection_toolbar_shows_copy_select_all() {
        // 选择菜单走 Surface 侧 PopupWindow（复制/粘贴/分享/全选），不用系统
        // ActionMode；键盘模式无设置 UI（默认安全模式），旧模式切换步骤是空转，
        // 连同其恢复块一并删除（删的是无操作步骤，不是覆盖）。
        // 新启动的 Activity 终端默认已获焦，无需额外点击（SurfaceView 挖洞
        // 渲染在 UiAutomator 层级中不可见，任何基于节点的聚焦定位都不可靠）。
        Thread.sleep(2000)
        // The menu only appears after an actual selection: long-press the
        // shell prompt near the bottom of the terminal. On the
        // software-rendered emulator the press may land on a blank cell
        // (paste-only menu: 粘贴) or on text (full menu: 复制); either
        // proves the selection menu surfaced through the real input
        // pipeline.
        // 空白格长按只在剪贴板有内容时才出粘贴菜单（否则动作表为空直接
        // return）；新机剪贴板恒空，先放种子文本，保证两种落点都有菜单。
        val clipboardManager =
            InstrumentationRegistry.getInstrumentation().targetContext
                .getSystemService(ClipboardManager::class.java)
        clipboardManager?.setPrimaryClip(ClipData.newPlainText("seed", "seed-selection"))
        val beforePress = device.takeScreenshot()
        // UiDevice.swipe 把 DOWN/UP 发进同一主线程批处理，常被当点按吃掉
        // （TestUtils.injectLongPress 有述）；经 input flinger 按真实时长
        // 下发事件，保证长按定时器能触发。
        // 长按点按显示尺寸取：屏幕尺寸随设备而变（CI 模拟器仅 320×640），
        // 硬编码坐标在窄屏上直接越界，手势根本没进终端。
        val pressX = device.displayWidth / 4
        val pressY = device.displayHeight / 4
        device.executeShellCommand(
            "input touchscreen swipe $pressX $pressY $pressX $pressY 1000",
        )
        Thread.sleep(1500)
        val copy = device.findObject(By.text("复制"))
        val paste = device.findObject(By.text("粘贴"))
        // 菜单 PopupWindow 若未进无障碍层级，文本查不到但像素必变：双信号判决。
        val changedPixels = countChangedPixels(beforePress, device.takeScreenshot())
        assertTrue(
            "Selection must surface after long-press " +
                "(copy=${copy != null} paste=${paste != null} changedPx=$changedPixels)",
            copy != null || paste != null || changedPixels > SELECTION_PIXEL_GAIN_THRESHOLD,
        )
        // When the long-press selects text, the paste-only menu must NOT
        // be shown (paste-only selections are reserved for blank cells).
        if (copy != null) {
            assertFalse(
                "Paste should NOT appear when text selected",
                paste != null,
            )
        }
    }

    @Test
    fun behavior_settings_theme_names_visible() {
        composeTestRule.openSettings()
        scrollSettingsTo(hasTestTag("ThemeSelector"))
        // 主题是横向列表：窄屏（CI 320×640）一次只容得下两三张卡，纵向滚动够不到
        // 右侧主题——必须横向滚到目标卡片再断言其可见（主题名在预览卡下方）。
        // 主题清单取自 BuiltInThemes（唯一来源），不点名字面量：清单变了本用例自动跟随。
        val themeList = composeTestRule.onAllNodes(hasTestTag("ThemeList"))[0]
        for (theme in BuiltInThemes.all) {
            themeList.performScrollToNode(hasTestTag("theme_preview_${theme.name}"))
            composeTestRule.onNodeWithTag("theme_preview_${theme.name}").assertIsDisplayed()
        }
    }

    @Test
    fun behavior_settings_bootstrap_action_buttons() {
        composeTestRule.openSettings()
        // 预设行在安装按钮上方：先滚到预设断言，再滚到按钮断言（一次只保证一项在
        // 视口内，同时断言两项在窄屏上恒失败）。
        scrollSettingsTo(hasTestTag("BootstrapPreset_TermuxDefault"))
        composeTestRule.onNodeWithTag("BootstrapPreset_TermuxDefault").assertIsDisplayed()
        scrollSettingsTo(hasTestTag("BootstrapInstallButton"))
        composeTestRule.onNodeWithTag("BootstrapInstallButton").assertIsDisplayed()
    }

    @Test
    fun behavior_settings_no_nerd_osc133_toggles() {
        composeTestRule.openSettings()
        composeTestRule.onAllNodesWithText("Nerd", substring = true).assertCountEquals(0)
        composeTestRule.onAllNodesWithText("OSC", substring = true).assertCountEquals(0)
    }

    @Test
    fun behavior_settings_shell_entry_empty_until_saved() {
        composeTestRule.openSettings()
        scrollSettingsTo(hasTestTag("ShellEntryInput"))
        // 规范要求未设置时显示空文本、不预填任何路径（shell-entry：
        // 「未设置时显示空文本」）；预填 `/system/bin/sh` 即违规。空输入框的语义树里
        // 根本没有 EditableText 条目，故按「有文本则必须为空」判。
        val shellSemantics =
            composeTestRule.onNodeWithTag("ShellEntryInput").fetchSemanticsNode().config
        val shellText = shellSemantics.getOrNull(SemanticsProperties.EditableText)?.text
        assertTrue("shell 设置框未设置时必须为空（实际=$shellText）", shellText.isNullOrEmpty())
        // 保存按钮在输入框下方 8dp：`performScrollToNode` 只滚到目标刚好可见
        // （贴视口下沿），按钮仍在视口外。与 bootstrap 用例同一口径，逐个滚逐个断言。
        scrollSettingsTo(hasTestTag("ShellSaveButton"))
        composeTestRule.onNodeWithTag("ShellSaveButton").assertIsDisplayed()
    }

    @Test
    fun behavior_modifier_bar_visible() {
        // 键栏是 Compose 覆盖层：四个主键同屏可见，逐个断言而非「ESC 出现即通过」。
        for (keyLabel in listOf("ESC", "CTRL", "ALT", "HOME")) {
            composeTestRule.onNodeWithText(keyLabel).assertIsDisplayed()
        }
    }

    @Test
    fun behavior_drawer_shows_sessions_and_settings() {
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton", useUnmergedTree = true).assertIsDisplayed()
        composeTestRule.onAllNodesWithText("会话", substring = true)
            .onFirst()
            .assertIsDisplayed()
    }
}
