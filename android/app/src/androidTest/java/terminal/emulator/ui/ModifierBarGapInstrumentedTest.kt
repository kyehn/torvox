package terminal.emulator.ui

import androidx.compose.ui.test.onNodeWithTag
import org.junit.Assert.assertEquals
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
import terminal.emulator.showImeAndAwaitShown
import terminal.emulator.tapToFocus
import kotlin.math.abs

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
// 输入法 inset 已确认后，等网格重算（若存在）落地的观察窗。
private const val IME_SETTLE_MS = 500L

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

    /**
     * 主屏网格不得随输入法变化：只有备用屏按键盘高度缩行（见 imeGridReserve）。
     *
     * 两种形态各跑一条用例只能证明「各自算对了」，证明不了「切换时不重排」；
     * 主屏一旦跟着缩行，键盘收起后就会留下整段空白行，直到新输出把视口填满。
     */
    @Test
    fun mainScreenGridStaysUnchangedAcrossImeToggle() {
        composeTestRule.awaitBridge()
        awaitShellPrompt(composeTestRule.requireBridge())
        composeTestRule.hideImeAndAwaitHidden()
        composeTestRule.waitForIdle()
        val surface = findTerminalSurface(composeTestRule.activity)
        require(surface is TerminalSurface) { "终端视图必须是 TerminalSurface，实测 ${surface.javaClass.name}" }
        Thread.sleep(IME_SETTLE_MS)
        val rowsHidden = onUiThread { surface.getRows() }

        tapToFocus(surface)
        composeTestRule.showImeAndAwaitShown()
        composeTestRule.waitForIdle()
        Thread.sleep(IME_SETTLE_MS)
        val rowsShown = onUiThread { surface.getRows() }

        assertEquals(
            "主屏网格不得随输入法变化（隐藏 $rowsHidden 行 vs 显示 $rowsShown 行）",
            rowsHidden,
            rowsShown,
        )
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

        // 网格算式扣的是运行期预留量，不是键栏的实际布局高度；两者一旦分叉，
        // 算式再正确也会在屏幕底部留下（或吞掉）一段真实空间。
        val barBounds = composeTestRule.onNodeWithTag("ModifierBar").fetchSemanticsNode().boundsInRoot
        assertTrue(
            "键栏实际高度 ${barBounds.height}px 必须等于预留高度 ${barHeight}px" +
                "（输入法${if (imeVisible) "显示" else "隐藏"}）",
            abs(barBounds.height - barHeight) <= 1f,
        )

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
