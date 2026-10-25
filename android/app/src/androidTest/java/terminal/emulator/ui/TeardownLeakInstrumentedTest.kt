package terminal.emulator.ui

import android.os.ParcelFileDescriptor
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

/**
 * 拆除路径的直接泄漏探头（守卫本身在 `TerminalSurface` 的
 * `onDetachedFromWindow`/三处弹窗创建点）。
 *
 * 修复前的实测形态：注释快照里一次全量套件挂 88 次
 * `WindowLeaked ... PopupDecorView`——`hideSelectionMenu("detach")` 默认
 * `restoreHandles = true`，在 `onDetachedFromWindow` 内重建手柄弹窗，挂到
 * 正在销毁的活动上；「重建弹窗窗口」的三处创建点当时只查
 * `isAttachedToWindow`，而框架在该回调内部仍报告 true（AttachInfo 后清）。
 * 守卫修复后若回归，此探头会在 recreate 循环里把排泄重新抓回屏幕。
 */
@RunWith(AndroidJUnit4::class)
class TeardownLeakInstrumentedTest : TerminalLogcatTest() {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    }

    @Test
    fun recreateWithSelectionMenuOpenDoesNotLeakPopupWindow() {
        composeTestRule.waitForSession()
        assertTrue("桥必须可用", composeTestRule.getBridge() != null)

        // 挂全屏选区让选区菜单先出现在屏幕上（修复形态下泄漏来自「菜单/手柄
        // 弹窗在拆桥时尚在重建」），随后立刻重建活动——detach 在菜单开着时发生。
        showSelectionOnTerminal()

        // 清空 logcat 标尺，让后面的读取只覆盖本轮 recreate 现场。
        shellCommand("logcat -c")

        composeTestRule.activityRule.scenario.recreate()
        device.waitForIdle(2000)
        composeTestRule.waitForSession()

        composeTestRule.activityRule.scenario.recreate()
        device.waitForIdle(2000)
        composeTestRule.waitForSession()

        composeTestRule.waitForIdle()

        // 应用在本轮现场没有通过系统编译器规避面（surface 已重建、会话可用）。
        composeTestRule
            .onAllNodesWithTag("TerminalScreen", useUnmergedTree = true)
            .fetchSemanticsNodes()
            .let { assertTrue("重建后终端界面必须仍在语义树中", it.isNotEmpty()) }

        val dump = shellCommand("logcat -d -v brief")
        val leakLines =
            dump
                .lineSequence()
                .filter { it.contains("WindowLeaked") || it.contains("has leaked window") }
                .filter { it.contains("com.termux") || it.contains("PopupDecorView") }
                .toList()
        assertFalse(
            "recreate 循环后不得再留 WindowLeaked（popup 拆桥守卫回归）\n" + leakLines,
            leakLines.isNotEmpty(),
        )
    }

    /** 挂全屏选区并以菜单「复制」项出现在 UiAutomator 树为出现信号。 */
    private fun showSelectionOnTerminal() {
        composeTestRule.activityRule.scenario.onActivity { activity ->
            val packed = checkNotNull(composeTestRule.getBridge()) { "桥接必须就绪" }.getGridRowsColsPacked()
            val rows = (packed ushr 32).toInt()
            val cols = (packed and 0xFFFFFFFFL).toInt()
            activity.terminalViewModel.startSelection(0, 0)
            activity.terminalViewModel.updateSelection(rows - 1, cols - 1)
            activity.terminalViewModel.endSelection()
        }
        UxTestUtils.pollUntilTrue(timeoutMs = 5000, intervalMs = 200) {
            device.hasObject(By.text("复制"))
        }
    }

    private fun shellCommand(command: String): String = InstrumentationRegistry
        .getInstrumentation()
        .uiAutomation
        .executeShellCommand(command)
        .use { ParcelFileDescriptor.AutoCloseInputStream(it).bufferedReader().readText() }
}
