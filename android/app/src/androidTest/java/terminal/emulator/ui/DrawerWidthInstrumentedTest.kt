package terminal.emulator.ui

import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.awaitBridge
import terminal.emulator.openDrawer
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 会话抽屉宽度验收：抽屉 MUST NOT 宽于可用宽度。
 *
 * 宽度口径 = 抽屉可用宽度的 84%，上限 320dp，**无下限**：ZTE 折叠屏外屏（约 236dp）
 * 上旧的 240dp 下限会把 0.84×236≈198dp 的抽屉强制抬宽到 240dp，比可用宽度还宽。
 * 三条断言：不超可用宽、不超 320dp 上限、小屏上不被下限抬宽（超出 84% 口径）。
 */
@RunWith(JUnit4::class)
class DrawerWidthInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val WIDTH_TOLERANCE_PX = 2f

        /** 与 [SessionDrawer] 的 `fillMaxWidth(0.84f)` 同口径。 */
        private const val DRAWER_WIDTH_FRACTION = 0.84f

        /** 与 [SessionDrawer] 的 `widthIn(max = 320.dp)` 同口径。 */
        private const val DRAWER_WIDTH_CAP_DP = 320

        /** 可用宽度低于该值时，240dp 下限会抬宽抽屉（0.84 × 286dp ≈ 240dp）。 */
        private const val MIN_FLOOR_OVERFLOW_THRESHOLD_DP = 286
    }

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
    }

    @Test
    fun drawerWidthFollowsFractionWithoutMinFloorOverflow() {
        composeTestRule.openDrawer()
        composeTestRule.waitForIdle()
        // Compose 语义查询必须在测试线程：runOnMainSync 里调 fetchSemanticsNode
        // 会被框架以「同步函数不得在主线程运行」拒绝（实测 CI 模拟器）。
        val drawerWidth =
            composeTestRule
                .onNodeWithTag("SessionDrawer", useUnmergedTree = true)
                .fetchSemanticsNode()
                .boundsInRoot.width
        var decorWidth = 0
        var density = 0f
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            density = composeTestRule.activity.resources.displayMetrics.density
            decorWidth = composeTestRule.activity.window.decorView.width
        }
        assertTrue("抽屉必须有宽度", drawerWidth > 0f)
        assertTrue(
            "抽屉不得超出可用宽度 (抽屉=$drawerWidth 可用=$decorWidth)",
            drawerWidth <= decorWidth + WIDTH_TOLERANCE_PX,
        )
        val capPx = DRAWER_WIDTH_CAP_DP * density
        assertTrue(
            "抽屉不得超出 ${DRAWER_WIDTH_CAP_DP}dp 上限 (抽屉=$drawerWidth 上限=$capPx)",
            drawerWidth <= capPx + WIDTH_TOLERANCE_PX,
        )
        if (decorWidth < MIN_FLOOR_OVERFLOW_THRESHOLD_DP * density) {
            val fractionPx = DRAWER_WIDTH_FRACTION * decorWidth
            assertTrue(
                "小屏上抽屉不得被宽度下限抬宽 (抽屉=$drawerWidth 84%=$fractionPx)",
                drawerWidth <= fractionPx + WIDTH_TOLERANCE_PX,
            )
        }
    }
}
