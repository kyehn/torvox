package terminal.emulator.ui

import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.core.view.WindowInsetsControllerCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.openDrawer
import terminal.emulator.waitForSession

/**
 * 系统栏图标明暗必须跟随**当前页面的底色**。
 *
 * 窗口是边到边的，系统栏自身透明、只决定图标明暗，故图标最终画在该页的背景像素上：
 * 终端页是深色终端背景（浅色图标），设置页是浅色面（深色图标）。同一个全局开关
 * 只有一个值，两个页面的底色相反，所以每页必须各自按自己的底色写一次。
 *
 * 旧实现只有终端页写，设置页沿用系统深浅色：深色系统 + 浅色设置页 → 浅色图标压在
 * 浅底上，时钟几乎不可见（预览截图 03-settings.png 的实测）。
 */
@RunWith(AndroidJUnit4::class)
class SystemBarIconInstrumentedTest : ComposeTerminalLogcatTest() {

    private fun lightStatusBars(): Boolean {
        val activity = composeTestRule.activity
        val controller = WindowInsetsControllerCompat(activity.window, activity.window.decorView)
        return controller.isAppearanceLightStatusBars
    }

    @Test
    fun systemBarIconsFollowEachPageBackground() {
        composeTestRule.waitForSession()
        // 终端页默认深色主题 → 浅色图标。
        assertEquals(
            "终端页是深色底，系统栏图标必须是浅色",
            false,
            lightStatusBars(),
        )

        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        // 节点存在即视为已挂载：点按到首帧之间有一段过渡，节点缺席是正常中间态。
        assertNotNull(
            "设置页未在 10s 内挂载",
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                settingsScreenShown()
            },
        )
        // 设置页是浅色面 → 深色图标。
        assertEquals(
            "设置页是浅色底，系统栏图标必须是深色",
            true,
            lightStatusBars(),
        )
    }

    /** 设置页是否已挂到组合树上；节点缺席是过渡期的正常中间态。 */
    private fun settingsScreenShown(): Boolean = try {
        composeTestRule.onNodeWithTag("SettingsScreen", useUnmergedTree = true).assertExists()
        true
    } catch (expected: AssertionError) {
        false
    }

    /**
     * 被遮住的终端页不得改写系统栏图标明暗。
     *
     * 设置页与终端页同处一棵组合树，各自都写同一个全局开关的话，谁最后重组谁赢。
     * 于是「打开设置页 → 终端页因为收到输出而重组」会把图标改回终端页那套，
     * 表现为切回终端瞬间图标闪一下。终端页在覆盖层可见时必须不写。
     */
    @Test
    fun theHiddenTerminalPageDoesNotOverwriteTheSystemBars() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        assertNotNull(
            "设置页未在 10s 内挂载",
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                settingsScreenShown()
            },
        )

        // 让终端页重组：输出一行字就会改终端状态。
        val bridge = composeTestRule.awaitBridge()
        val nonce = "recompose-${System.nanoTime()}"
        assertTrue(
            "输出未写入 PTY",
            bridge.writeToPty("echo $nonce\n".toByteArray()),
        )
        // 等终端真的收到输出（终端页随之重组）再断言。
        assertNotNull(
            "终端未在 10s 内收到输出",
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                bridge.getVisibleText().contains(nonce)
            },
        )
        composeTestRule.waitForIdle()

        assertEquals(
            "被遮住的终端页重组后不得改写系统栏图标明暗",
            true,
            lightStatusBars(),
        )
    }
}
