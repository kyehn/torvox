package terminal.emulator.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.getBridge
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 工具栏配置驱动端到端（对标 sylirre TerminalUiTest.extraKeysConfigDrivesToolbar /
 * extraKeysTypeIntoShell）：保存含自定义键的布局 → 工具栏实时出现该键（经
 * SharedPreferences 监听，无需重建 Activity）→ 点击后序列到达 shell 回显。
 * finally 恢复原布局，避免污染后续测试。
 */
@RunWith(JUnit4::class)
class ToolbarConfigInstrumentedTest {
    companion object {
        private const val CUSTOM_ID = "e2e_custom_cfg"
        private const val CUSTOM_LABEL = "TBCFG"
        private const val CUSTOM_SEQUENCE = "tbzq"
        private const val APPEAR_TIMEOUT_MS = 10_000L
        private const val ECHO_TIMEOUT_MS = 15_000L
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", composeTestRule.getBridge())
    }

    private fun currentText(): String? {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    @Test
    fun customLayoutDrivesToolbarAndTypesIntoShell() {
        val preferences = ToolbarPreferences(composeTestRule.activity)
        val previous = preferences.getLayout()
        try {
            // prompt 门控：粘贴案证明冷启动 shell 未消费 stdin 前的输入会丢失。
            val promptSeen =
                UxTestUtils.pollUntilTrue(timeoutMs = 60_000, intervalMs = 200) {
                    val text = currentText()
                    text != null && (text.contains("$") || text.contains("#"))
                }
            assertNotNull("shell prompt 必须先就绪", promptSeen)

            InstrumentationRegistry.getInstrumentation().runOnMainSync {
                preferences.saveLayout(
                    listOf(
                        ToolbarItem.Custom(
                            label = CUSTOM_LABEL,
                            sequence = CUSTOM_SEQUENCE,
                            id = CUSTOM_ID,
                        ),
                    ),
                )
            }
            // 配置经监听实时生效：自定义键出现，默认 ESC 键消失。
            composeTestRule.waitUntil(timeoutMillis = APPEAR_TIMEOUT_MS) {
                try {
                    composeTestRule.onNodeWithTag("Key_$CUSTOM_ID").assertIsDisplayed()
                    true
                } catch (_: AssertionError) {
                    false
                }
            }
            composeTestRule.onNodeWithTag("Key_$CUSTOM_ID").assertIsDisplayed()

            // 点击自定义键：序列经 onKeyClick 直写 pty，shell 回显。
            composeTestRule.onNodeWithTag("Key_$CUSTOM_ID").performClick()
            val echoed =
                UxTestUtils.pollUntilTrue(timeoutMs = ECHO_TIMEOUT_MS, intervalMs = 100) {
                    currentText()?.replace("\n", "")?.contains(CUSTOM_SEQUENCE) == true
                }
            assertNotNull("自定义键序列必须到达 shell 回显: $CUSTOM_SEQUENCE", echoed)
        } finally {
            InstrumentationRegistry.getInstrumentation().runOnMainSync {
                preferences.saveLayout(previous)
            }
            // 恢复默认布局：ESC 键回到工具栏，后续测试不受污染。
            composeTestRule.waitUntil(timeoutMillis = APPEAR_TIMEOUT_MS) {
                try {
                    composeTestRule.onNodeWithTag("Key_ESC").assertIsDisplayed()
                    true
                } catch (_: AssertionError) {
                    false
                }
            }
        }
    }
}
