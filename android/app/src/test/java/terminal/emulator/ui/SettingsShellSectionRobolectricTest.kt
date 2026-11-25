package terminal.emulator.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Shell 启动入口的保存口径：命令行按规范原样保存，空命令行表示「走默认入口探测链」，
 * 保存时不得检查命令文本。判别力：保存按钮若要求命令行为空，空命令行入口根本存不下来，
 * 用户便无法保存「用默认入口」这一条目。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class SettingsShellSectionRobolectricTest {
    @get:Rule val composeRule = createComposeRule()

    private val savedEntries = mutableListOf<Pair<String, String>>()

    private fun showSection() {
        composeRule.setContent {
            MaterialTheme {
                TerminalConfigSection(
                    activeShellName = "",
                    shellEntries = emptyList(),
                    onShellEntrySaved = { name, command -> savedEntries += name to command },
                    onShellEntryRemoved = {},
                    onShellEntryActivated = {},
                    textColor = Color.Unspecified,
                    accentColor = Color.Unspecified,
                    cardBackground = Color.Unspecified,
                    sectionTitleColor = Color.Unspecified,
                )
            }
        }
    }

    @Test
    fun `entry with empty command is saved as is`() {
        showSection()
        composeRule.onNodeWithTag("ShellEntryName").performTextInput("默认入口")
        composeRule.onNodeWithTag("ShellEntrySaveButton").performClick()
        composeRule.waitForIdle()
        assertEquals(listOf("默认入口" to ""), savedEntries)
    }

    @Test
    fun `entry without name is not saved`() {
        showSection()
        composeRule.onNodeWithTag("ShellEntryInput").performTextInput("/system/bin/sh")
        composeRule.onNodeWithTag("ShellEntrySaveButton").performClick()
        composeRule.waitForIdle()
        assertEquals(emptyList<Pair<String, String>>(), savedEntries)
    }
}
