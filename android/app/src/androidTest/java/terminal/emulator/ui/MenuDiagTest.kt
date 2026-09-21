package terminal.emulator.ui

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.waitForSession

/** 临时诊断：菜单不显示的根因定位（showSelectionMenu 静默早退 vs LaunchedEffect 未触发）。 */
@RunWith(JUnit4::class)
class MenuDiagTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun dumpMenuPreconditions() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            activity.terminalViewModel.selectAll(0)
        }
        composeTestRule.waitForIdle()
        Thread.sleep(1_000)
        val shot = device.takeScreenshot() ?: throw AssertionError("截图失败")
        var report = ""
        composeTestRule.activityRule.scenario.onActivity { activity: MainActivity ->
            val surface = findTerminalSurface(activity) as TerminalSurface
            val selection = activity.terminalViewModel.state.value.selection
            java.io.File(activity.filesDir, "menu_diag.png").outputStream().use { stream ->
                shot.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, stream)
            }
            val popupField = TerminalSurface::class.java.getDeclaredField("selectionMenuPopup")
            popupField.isAccessible = true
            val popup = popupField.get(surface) as android.widget.PopupWindow?
            report =
                "popupShown=${popup != null} popupShowing=${popup?.isShowing} " +
                "content=${popup?.contentView?.width}x${popup?.contentView?.height} " +
                "active=${selection.active} dismissed=${selection.menuDismissed}"
        }
        assertTrue("DIAG $report", false)
    }
}
