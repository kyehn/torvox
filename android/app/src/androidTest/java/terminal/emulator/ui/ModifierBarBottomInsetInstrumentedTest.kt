package terminal.emulator.ui

import android.view.WindowInsets
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.awaitBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.hideImeAndConfirm
import terminal.emulator.isImeVisible
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 键栏底部内缩口径验收（STATUS「输入法未弹出时，修饰键栏底部存在过高的空区域背景」）。
 *
 * 内容盒只按导航栏内缩：ZTE 折叠屏外屏等「导航栏 inset=0 的纯手势设备」上不再出现
 * safeGestures 附加内缩造成的键栏下空带（修复依据：该设备 logcat 显示
 * navigationBars 底部为 0，而键栏下仍有背景色空带，即空带来自手势区附加内缩）。
 *
 * 输入法隐藏时断言三条结构关系：
 * 1. 内容盒底边恰在导航栏上沿（无手势区附加内缩，也不欠内缩）；
 * 2. 终端 Surface 填满内容盒（屏幕坐标下底边与内容盒齐平）；
 * 3. 键栏覆盖层贴内容盒底边（键栏与内容盒之间不允许出现任何内缩带）。
 *
 * 键位因此可能落在全面屏手势区内：落在系统手势区（safeGestures）内的按下
 * 一律不启动按下反馈——系统随时可能接管这次触摸并补发 ACTION_CANCEL，已画出的
 * 缩放就是用户看到的「上滑触发了按键动画」（ModifierBarGestureAreaTest 有
 * 对应用例）——与上游 termux ExtraKeysView 的同一取舍。
 */
@RunWith(JUnit4::class)
class ModifierBarBottomInsetInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val IME_HIDDEN_TIMEOUT_MS = 10_000L

        /** 视图坐标取整容差：组合边界为浮点，视图为整数像素。 */
        private const val LAYOUT_TOLERANCE_PX = 1f
    }

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
        hideImeAndSettle()
    }

    /** 收起输入法并复核隐藏：启动期自动弹键盘（hide-soft-keyboard-on-startup=false 路径）会与本用例竞态。 */
    private fun hideImeAndSettle() {
        repeat(3) {
            if (hideImeAndConfirm(composeTestRule.activity, IME_HIDDEN_TIMEOUT_MS)) return
        }
        assertTrue("输入法必须收起才能测键栏落位", !isImeVisible(composeTestRule.activity))
    }

    @Test
    fun barAndSurfaceReachContentBottomWithoutGestureBand() {
        // Compose 语义查询必须在测试线程：runOnMainSync 里调 fetchSemanticsNode
        // 会被框架以「同步函数不得在主线程运行」拒绝（实测 CI 模拟器）。
        // 两组读数同处一次 settle 之后的静止布局，先后取不会互相漂移。
        val contentBottom =
            composeTestRule
                .onNodeWithTag("TerminalContent", useUnmergedTree = true)
                .fetchSemanticsNode()
                .boundsInRoot.bottom
        val barBottom =
            composeTestRule
                .onNodeWithTag("ModifierBarOverlay", useUnmergedTree = true)
                .fetchSemanticsNode()
                .boundsInRoot.bottom
        var navBottom = 0
        var decorHeight = 0
        var surfaceScreenBottom = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val activity = composeTestRule.activity
            val decor = activity.window.decorView
            navBottom =
                decor.rootWindowInsets
                    ?.getInsets(WindowInsets.Type.navigationBars())
                    ?.bottom ?: 0
            decorHeight = decor.height
            val surface = findTerminalSurface(activity)
            val location = IntArray(2)
            surface.getLocationOnScreen(location)
            surfaceScreenBottom = location[1] + surface.height
        }
        assertTrue("装饰视图必须有高度", decorHeight > 0)
        // 内容盒底边 = 导航栏上沿：手势区附加内缩若被重新引入，导航栏 inset=0 的
        // 手势设备上这里会多出一段（ZTE 外屏实测形态），欠内缩则贴到屏幕底。
        assertEquals(
            "内容盒底边必须贴导航栏上沿 (nav=$navBottom decor=$decorHeight)",
            (decorHeight - navBottom).toFloat(),
            contentBottom,
            LAYOUT_TOLERANCE_PX,
        )
        // Surface 填满内容盒：屏幕坐标含状态栏，组合根坐标不含，用装饰视图高度统一口径。
        assertEquals(
            "终端 Surface 底边必须与内容盒齐平",
            decorHeight - navBottom,
            surfaceScreenBottom,
        )
        // 键栏贴内容盒底边：键栏与内容盒之间不允许出现任何内缩带。
        assertEquals(
            "键栏覆盖层底边必须贴内容盒底边",
            contentBottom,
            barBottom,
            LAYOUT_TOLERANCE_PX,
        )
    }
}
