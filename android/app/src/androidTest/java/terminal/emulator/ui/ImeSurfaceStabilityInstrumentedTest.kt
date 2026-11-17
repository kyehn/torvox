package terminal.emulator.ui

import android.view.View
import android.view.WindowInsets
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitShellPrompt
import terminal.emulator.findTerminalSurface
import terminal.emulator.hideImeAndConfirm
import terminal.emulator.onUiThread
import terminal.emulator.requireBridge
import terminal.emulator.showCursor
import terminal.emulator.tapToFocus

/**
 * 输入法显隐不得替换原生窗口。
 *
 * 平台拆掉 `SurfaceView` 的 surface 后会交付新的 `ANativeWindow`，原生随之重建交换链并
 * 重新呈现保留帧——这中间有一段「开孔里没有缓冲」的窗口。切后台往返时这段窗口无法避免
 * （平台必定拆 surface），但**输入法显隐不应当**触发它：键盘只是改变 insets 与窗口布局，
 * SurfaceView 的 surface 应当原地保留。
 *
 * 判据取三处，任何一处变化都说明视图被重建：
 *
 * 1. `TerminalSurface` 实例（`AndroidView` 的 factory 是否被重新执行）；
 * 2. `SurfaceHolder.getSurface()` 的 Java 对象（`SurfaceView` 是否释放了 surface）；
 * 3. 原生 `attachWindow` 日志（是否交付了新的 `ANativeWindow`）。
 *
 * 只测「不重建」而不测像素：重建后呈现的是同一帧保留帧，像素一致发现不了它，而重建
 * 本身就是那段空白窗口的成因。
 */
@RunWith(JUnit4::class)
class ImeSurfaceStabilityInstrumentedTest : ComposeTerminalLogcatTest() {
    private companion object {
        const val TOGGLE_CYCLES = 3
        const val IME_TIMEOUT_MS = 15_000L
        const val SETTLE_MILLIS = 600L

        /** 原生交付新窗口时打印的锚点（`attachWindow` 收到新指针即一行）。 */
        const val ATTACH_LOG_ANCHOR = "attachWindow ptr="

        /**
         * 切片回看的余量：`logcat -T` 以秒为精度，切片前多回看一点避免漏掉边界行；
         * 余量内的旧行用「切片起点之后才可能发生」这一前提排除——调用方在开始
         * 切换输入法之前记录时间戳，故余量只影响取数，不影响判定对象。
         */
        const val SLICE_SLACK_MS = 1_000L
    }

    @Test
    fun ime_toggle_keeps_surface_and_native_window() {
        val bridge = composeTestRule.requireBridge()
        awaitShellPrompt(bridge)
        bridge.showCursor()

        val activity = composeTestRule.activity
        val surface = composeTestRule.findTerminalSurface()
        tapToFocus(surface)
        assertTrue("测试前提：输入法必须能弹出", showIme(activity))
        // 首次弹出之后的布局/附着先落定：冷启动的首次 `attachWindow` 可能晚于
        // 「输入法可见」一拍，若把它算进判定窗，正常启动会被当成「显隐交付了新窗口」
        // （实测误报过一次，当时应用进程刚起 1.0 秒）。
        Thread.sleep(SETTLE_MILLIS)

        // 测试前提：SurfaceView 必须已经拿到 surface。否则前后两次都拿到 null，
        // `assertSame(null, null)` 恒成立，用例就成了空跑。
        val holderBefore =
            onUiThread { (surface as android.view.SurfaceView).holder.surface }
                ?: throw AssertionError("测试前提：SurfaceView 必须已经拿到 surface")
        val togglesStartedAt = System.currentTimeMillis()
        // 基线：判定窗内已有的原生窗口交付行。显隐前已存在的实例不算「显隐交付了
        // 新窗口」；差分后仍出现就说明真的换了窗口。
        val attachBaseline = attachLinesSince(togglesStartedAt)

        repeat(TOGGLE_CYCLES) { cycle ->
            assertTrue("第 $cycle 轮：输入法必须收起", hideImeAndConfirm(activity, IME_TIMEOUT_MS))
            assertTrue("第 $cycle 轮：输入法必须能再次弹出", showIme(activity))
            Thread.sleep(SETTLE_MILLIS)
        }

        val surfaceAfter = composeTestRule.findTerminalSurface()
        assertSame("输入法显隐不得重建 TerminalSurface 视图", surface, surfaceAfter)
        val holderAfter = onUiThread { (surfaceAfter as android.view.SurfaceView).holder.surface }
        assertSame("输入法显隐不得替换 SurfaceHolder 的 Surface", holderBefore, holderAfter)

        val attachLines = attachLinesSince(togglesStartedAt)
        assertEquals(
            "输入法显隐不得交付新的 ANativeWindow（显隐前 $attachBaseline，显隐后 $attachLines）",
            attachBaseline,
            attachLines,
        )
    }

    /** 判定窗（含切片回看余量）内的原生窗口交付行。 */
    private fun attachLinesSince(startedAtMs: Long): List<String> = UxTestUtils
        .logcatSince(startedAtMs - SLICE_SLACK_MS)
        .filter { it.contains(ATTACH_LOG_ANCHOR) }

    private fun showIme(activity: android.app.Activity): Boolean {
        val visible =
            UxTestUtils.pollUntilTrue(timeoutMs = IME_TIMEOUT_MS, intervalMs = 200) {
                onUiThread {
                    val decor = activity.window.decorView
                    val shown = decor.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == true
                    if (!shown) decor.windowInsetsController?.show(WindowInsets.Type.ime())
                    shown
                }
            }
        return visible != null
    }
}

private fun AndroidComposeTestRule<*, *>.findTerminalSurface(): View = findTerminalSurface(activity)
