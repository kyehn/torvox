package terminal.emulator.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import terminal.emulator.R
import terminal.emulator.bridge.CjkState
import terminal.emulator.bridge.FontInfo

/**
 * 设置页的 CJK 备用字体提示必须说真话。
 *
 * 三条判据各自对应一类曾经出现过的事实错误：
 * 1. 同一事实只能发一次（状态行与缺失警告曾由两个发射点各发一份）。
 * 2. 「按设计跳过」不是缺失，不得报「未检测到」。
 * 3. 原生尚未上报时既不是缺失也不是跳过，不得断言任何一种结论。
 *
 * 期望文案一律从字符串资源读取：文案是产品决定，不该让文案改动把测试带红。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class CjkFallbackStatusRobolectricTest {
    @get:Rule val composeRule = createComposeRule()

    private fun showStatus(fontInfo: FontInfo) {
        composeRule.setContent {
            MaterialTheme {
                CjkFallbackStatusLine(fontInfo = fontInfo, secondaryText = Color.Gray)
            }
        }
    }

    private fun info(state: CjkState, families: List<String> = emptyList()): FontInfo = FontInfo(
        activeName = "Droid Sans Mono",
        activeMonospaced = true,
        cjkState = state,
        cjkFamilies = families,
        otherFamilies = emptyList(),
        cellWidthPx = 7f,
        cellHeightPx = 14f,
        fontSizeSp = 14f,
    )

    private fun string(resourceId: Int): String = androidx.test.core.app.ApplicationProvider
        .getApplicationContext<android.content.Context>()
        .getString(resourceId)

    private fun string(resourceId: Int, vararg args: Any): String = androidx.test.core.app.ApplicationProvider
        .getApplicationContext<android.content.Context>()
        .getString(resourceId, *args)

    @Test
    fun `missing fallback reports the gap exactly once`() {
        showStatus(info(CjkState.NONE))
        val warning = string(R.string.cjk_fallback_missing_warning)
        composeRule.onNodeWithText(warning).assertExists()
        // 同一句警告不得出现两次。
        composeRule.onAllNodesWithText(warning).assertCountEquals(1)
    }

    @Test
    fun `skipped by design reports the reason instead of a gap`() {
        showStatus(info(CjkState.SKIPPED_NON_CJK_LOCALE))
        composeRule
            .onNodeWithTag("CjkFallbackStatusLine")
            .assertTextEquals(string(R.string.font_info_cjk_skipped_non_cjk_locale))
        composeRule
            .onAllNodesWithText(string(R.string.cjk_fallback_missing_warning))
            .assertCountEquals(0)
    }

    @Test
    fun `not reported yet states no conclusion`() {
        showStatus(info(CjkState.NOT_REPORTED))
        composeRule
            .onNodeWithTag("CjkFallbackStatusLine")
            .assertTextEquals(string(R.string.font_info_cjk_not_reported))
        composeRule
            .onAllNodesWithText(string(R.string.cjk_fallback_missing_warning))
            .assertCountEquals(0)
    }

    @Test
    fun `real fallback lists the families`() {
        showStatus(info(CjkState.FALLBACK, families = listOf("Noto Sans CJK SC")))
        composeRule
            .onNodeWithTag("CjkFallbackStatusLine")
            .assertTextEquals(string(R.string.font_info_cjk_fallback, "Noto Sans CJK SC"))
    }
}
