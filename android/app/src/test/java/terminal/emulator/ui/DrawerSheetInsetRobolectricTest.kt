package terminal.emulator.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * 抽屉面板的**底色**必须避开状态栏。
 *
 * Material3 把 `windowInsetsPadding` 施加在面板的内容而非 Surface（背景）上，
 * 面板底色会一路铺到屏幕顶端：状态栏区一半是浅色面板（白字时钟不可读）、
 * 一半是深色终端，而系统栏图标明暗只有一个全局开关，两侧无法同时清晰。
 *
 * 判据：面板 Surface 的顶边必须落在状态栏之下。修复前顶边是 0，浅色底色直接盖住时钟。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w360dp-h640dp-160dpi")
class DrawerSheetInsetRobolectricTest {
    @get:Rule val composeRule = createComposeRule()

    @Test
    fun `sheet background starts below the status bar`() {
        val statusBarTop = 24.dp
        composeRule.setContent {
            MaterialTheme {
                Box(modifier = Modifier.fillMaxSize().background(Color.Black)) {
                    DrawerSheet(statusBarTop = statusBarTop) {
                        androidx.compose.material3.Text("drawer content")
                    }
                }
            }
        }
        val sheetTop = composeRule.onNodeWithTag("DrawerSheet").fetchSemanticsNode().boundsInRoot.top
        assertTrue(
            "面板底色顶边 $sheetTop 必须落在状态栏 $statusBarTop 之下，否则浅色面板会盖住状态栏",
            sheetTop >= statusBarTop.value,
        )
        // 内容仍须可见（面板没有因为内缩而消失）。
        composeRule.onNodeWithText("drawer content").assertExists()
    }
}
