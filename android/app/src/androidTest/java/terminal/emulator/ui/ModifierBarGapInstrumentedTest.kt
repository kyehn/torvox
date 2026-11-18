package terminal.emulator.ui

import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.findTerminalSurface
import terminal.emulator.hideImeAndAwaitHidden
import terminal.emulator.onUiThread
import terminal.emulator.requireBridge

/**
 * 网格底部与修饰键栏之间不得留下超过一行的空隙（STATUS「输入法隐藏时空隙过大」）。
 *
 * 判据来自两侧共同的算式：可见网格高 = Surface 高 − 键栏高 − 输入法预留，
 * 行数 = 该高度按单元高向下取整。若实现少算行数，剩余高度会以整行倍数留白，
 * 而滚动又按同一视口高度重算，于是「空白看着很大、滚一下又用上了」。
 * 这条用例对两种键盘形态取同一判据：余量必须落在 [0, 单行高) 内。
 *
 * 主屏的网格不随输入法变化（输入法只平移 Surface，见 ime-animation-smoothness），
 * 故两种形态验的是同一份高度；「键栏底边恰好贴键盘顶边」由
 * `ImePopupPixelInstrumentedTest` 与 `ImeSurfaceStabilityInstrumentedTest` 覆盖。
 */
@RunWith(JUnit4::class)
class ModifierBarGapInstrumentedTest : ComposeTerminalLogcatTest() {
    @Test
    fun gapBelowGridStaysWithinOneRowWithImeShown() {
        composeTestRule.awaitBridge()
        awaitShellPrompt(composeTestRule.requireBridge())
        // 启动期自动弹出输入法（hide-soft-keyboard-on-startup = false），无需额外操作。
        composeTestRule.waitForIdle()
        assertGapWithinOneRow(imeVisible = true)
    }

    @Test
    fun gapBelowGridStaysWithinOneRowWithImeHidden() {
        composeTestRule.awaitBridge()
        awaitShellPrompt(composeTestRule.requireBridge())
        composeTestRule.hideImeAndAwaitHidden()
        composeTestRule.waitForIdle()
        assertGapWithinOneRow(imeVisible = false)
    }

    /** 断言网格底边与可见高度之间只剩不足一行的余量。 */
    private fun assertGapWithinOneRow(imeVisible: Boolean) {
        val found = findTerminalSurface(composeTestRule.activity)
        require(found is TerminalSurface) { "终端视图必须是 TerminalSurface，实测 ${found.javaClass.name}" }
        val surface = found
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        val viewHeight = onUiThread { surface.height }
        val gridRows = onUiThread { surface.getRows() }
        val cellHeight = runtime.cellHeight
        val barHeight = runtime.modifierBarHeightPx
        assertTrue("行数必须为正（实测 $gridRows）", gridRows > 0)
        assertTrue("单元高必须为正（实测 $cellHeight）", cellHeight > 0f)
        assertTrue("键栏占高必须为正（实测 $barHeight）", barHeight > 0)

        val available = viewHeight - barHeight - runtime.imeGridReserve()
        assertTrue(
            "输入法${if (imeVisible) "显示" else "隐藏"}时可见网格高 $available 必须为正",
            available > 0,
        )
        val gridHeight = gridRows * cellHeight
        assertTrue(
            "网格 $gridRows 行 × ${cellHeight}px = ${gridHeight}px 不得越过可见高度 ${available}px" +
                "（输入法${if (imeVisible) "显示" else "隐藏"}）",
            gridHeight <= available,
        )
        val gap = available - gridHeight
        android.util.Log.i(
            "GapProbe",
            "ime=$imeVisible view=$viewHeight bar=$barHeight shift=${onUiThread { surface.translationY.toInt() }}" +
                " reserve=${runtime.imeGridReserve()} rows=$gridRows cell=$cellHeight gap=$gap",
        )
        assertTrue(
            "网格底部与键栏间余量 ${gap}px 必须小于一行 ${cellHeight}px" +
                "（输入法${if (imeVisible) "显示" else "隐藏"}，可见高 ${available}px）",
            gap < cellHeight,
        )
    }
}
