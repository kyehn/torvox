package terminal.emulator.ui

import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.bridge.Bridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.onUiThread
import terminal.emulator.scrollViewportToBottom
import terminal.emulator.tapToFocus
import terminal.emulator.waitForSession

/**
 * STATUS ②「简体中文退格稍微缓慢」的端到端量化与不变式。
 *
 * 起点是软键盘退格键的真实通道 `deleteSurroundingText(1, 0)`，终点是**真的上屏**：
 * 以 `getRenderedCursorPacked` 为准——那是最近一帧绘制取走的光标，光标列退到删除前
 * 之左，说明「少一个字」的那一帧已经提交给合成器。它不掺 VT 当前状态的猜测，也不
 * 依赖文本抽取的字符串形态。
 *
 * 修复前渲染循环静置时按 500ms 空闲闭锁轮询：回显早已就绪，画面却要再等最多半秒，
 * 这才是体感「退格慢」的主因（旧路径的等待直接叠加在这个指标上）。现在 PTY 输出
 * 直接唤醒渲染线程（见 [IdleOutputWakeTest]），上屏只差一两帧。
 *
 * 第二个测试钉住中文与 ASCII 的**相对**代价：一个汉字与一个 ASCII 字符在 PTY 上
 * 都是一条 0x7F，故两者的上屏耗时必须同量级。这条不变式比任何绝对毫秒预算都稳，
 * 也让「中文退格比英文慢」这类回归无处藏身。
 */
class BackspaceLatencyTest : TerminalLogcatTest() {
    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private companion object {
        /** 末四位是汉字：逐次退格各删一个码点。 */
        const val CJK_TYPED = "bs中文字幕"

        /** 纯 ASCII 对照：同样六个字符，退格同样是六条 0x7F。 */
        const val ASCII_TYPED = "bsabcde"

        /** 删除的字符数：CJK 取 `中文字幕`，ASCII 取 `abcde` 的最后四个。 */
        const val SAMPLES = 4

        /**
         * 上屏延迟预算（毫秒）：从退格回调到那一帧呈现。
         *
         * 校准依据：模拟器（软件渲染、bootstrap bash）实测 p50 约 200ms、p95 约
         * 250ms，主体是 shell 的 readline 往返。旧的 500ms 空闲闭锁会在这之上
         * 再加最多半秒，p50 直接越过这里，故预算取实测值的一点五倍：既钉住回归，
         * 又不钉模拟器的绝对速度。
         */
        const val PRESENT_P50_BUDGET_MS = 300L
        const val PRESENT_P95_BUDGET_MS = 700L

        /** 回调自身的主线程耗时上限（微秒）：超过半帧说明里面又做了同步等待。 */
        const val CALLBACK_BUDGET_US = 50_000L

        /** 汉字与 ASCII 的上屏 p50 比值上限：一倍再加一次轮询粒度，远超噪声。 */
        const val CJK_OVER_ASCII_RATIO = 2.0

        /** 比值判据的绝对容差（毫秒）：覆盖轮询粒度与帧抖动的固定开销。 */
        const val RATIO_SLACK_MS = 50.0

        /** 上屏后确认字符真的消失的等待上限（毫秒）：只做正确性检查，不计时。 */
        const val DISAPPEAR_TIMEOUT_MS = 3_000L
    }

    private data class Sample(val presentMs: Long, val callbackUs: Long)

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun cjk_backspace_reaches_screen_within_budget() {
        val bridge = composeTestRule.awaitBridge()
        val connection = focusTerminalAndWaitForPrompt(bridge)

        val samples = sampleDeletes(bridge, connection, CJK_TYPED)
        val present = samples.map { it.presentMs }
        val callback = samples.map { it.callbackUs }

        UxTestUtils.metric("backspace_present_p50_ms", percentile(present, 0.50))
        UxTestUtils.metric("backspace_present_p95_ms", percentile(present, 0.95))
        UxTestUtils.metric("backspace_callback_p50_us", percentile(callback, 0.50))
        UxTestUtils.metric("backspace_callback_p95_us", percentile(callback, 0.95))

        assertTrue(
            "汉字退格上屏 p50=${percentile(present, 0.50)}ms 超出 ${PRESENT_P50_BUDGET_MS}ms" +
                "（$present）：渲染循环没有跟上回显",
            percentile(present, 0.50) <= PRESENT_P50_BUDGET_MS,
        )
        assertTrue(
            "汉字退格上屏 p95=${percentile(present, 0.95)}ms 超出 ${PRESENT_P95_BUDGET_MS}ms（$present）",
            percentile(present, 0.95) <= PRESENT_P95_BUDGET_MS,
        )
        assertTrue(
            "退格回调 p50=${percentile(callback, 0.50)}us 超出 ${CALLBACK_BUDGET_US}us（$callback）",
            percentile(callback, 0.50) <= CALLBACK_BUDGET_US,
        )

        val finalText = bridge.getTerminalText().orEmpty()
        assertTrue("四个汉字必须全部删掉，屏幕尾部=${finalText.takeLast(80)}", !finalText.atPromptEnd("bs中"))
    }

    /**
     * 中文与 ASCII 退格同量级——同一会话内先量 ASCII 再量中文，排除机器状态差异。
     *
     * 判据用「中文 p50 ≤ ASCII p50 × 2 + 50ms」：任何按列宽加倍、逐码点等帧或按
     * 字节长度分流的实现都会把比值拉开。
     */
    @Test
    fun cjk_backspace_is_not_slower_than_ascii() {
        val bridge = composeTestRule.awaitBridge()
        val connection = focusTerminalAndWaitForPrompt(bridge)

        val ascii = percentile(sampleDeletes(bridge, connection, ASCII_TYPED).map { it.presentMs }, 0.50)
        val cjkMedian = percentile(sampleDeletes(bridge, connection, CJK_TYPED).map { it.presentMs }, 0.50)
        UxTestUtils.metric("backspace_ascii_present_p50_ms", ascii)
        UxTestUtils.metric("backspace_cjk_present_p50_ms", cjkMedian)

        val budget = ascii * CJK_OVER_ASCII_RATIO + RATIO_SLACK_MS
        assertTrue(
            "中文退格上屏 p50=${cjkMedian}ms 比 ASCII 的 ${ascii}ms 慢出预算（上限 ${budget.toLong()}ms）：" +
                "汉字与 ASCII 都是一条 0x7F，不应有量级差别",
            cjkMedian <= budget,
        )
    }

    /**
     * 键入 [typed] 后逐次退格，返回每次的上屏耗时与回调耗时。
     *
     * 每个样本开始前有两条前提：
     * 1. 光标行尾就是待删除的整串——不然「少一个字」无从判定；
     * 2. 最近一帧的光标已追上 VT 的当前光标——否则拿到的是上一帧的陈旧列，
     *    会把已经上屏的帧算进这一次（或永远等不到左移）。
     */
    private fun sampleDeletes(bridge: Bridge, connection: InputConnection, typed: String): List<Sample> {
        writeText(connection, typed)
        val typedOnScreen =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 50) {
                bridge.getTerminalText().orEmpty().atPromptEnd(typed)
            }
        assertNotNull("键入「$typed」未落格（尾部=${bridge.getTerminalText()?.takeLast(80)}）", typedOnScreen)

        val samples = mutableListOf<Sample>()
        var current = typed
        repeat(SAMPLES) {
            val before = current
            current = before.dropLast(1)

            val atTail =
                UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 8) {
                    bridge.getTerminalText().orEmpty().atPromptEnd(before)
                }
            assertNotNull(
                "删除前光标行尾不是「$before」（尾部=${bridge.getTerminalText()?.takeLast(80)}）",
                atTail,
            )
            val settled =
                UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 8) {
                    val rendered = bridge.renderedCursorPacked()
                    rendered >= 0 && rendered == bridge.cursorViewportPacked()
                }
            assertNotNull("删除前渲染帧未追上当前光标，无法确定起点", settled)
            val cursorBefore = bridge.renderedCursorPacked().column()

            var accepted = false
            var elapsedUs = 0L
            val callbackNs = System.nanoTime()
            onUiThread {
                val start = System.nanoTime()
                accepted = connection.deleteSurroundingText(1, 0)
                elapsedUs = (System.nanoTime() - start) / 1_000
            }
            assertTrue("第 ${it + 1} 次退格被输入通道拒收", accepted)

            val presented =
                UxTestUtils.pollUntilTrue(timeoutMs = 3_000, intervalMs = 4) {
                    val column = bridge.renderedCursorPacked().column()
                    column >= 0 && cursorBefore >= 0 && column < cursorBefore
                }
            val presentAt = System.nanoTime()
            assertNotNull(
                "退格后 3s 内渲染帧的光标未左移（删除前列=$cursorBefore，" +
                    "当前列=${bridge.renderedCursorPacked().column()}）",
                presented,
            )

            // 正确性（不计时）：上屏的那一帧之后，终端里也必须真的少了这个字。
            val disappeared =
                UxTestUtils.pollUntilTrue(timeoutMs = DISAPPEAR_TIMEOUT_MS, intervalMs = 8) {
                    !bridge.getTerminalText().orEmpty().atPromptEnd(before)
                }
            assertNotNull(
                "退格已上屏但终端文本仍以「$before」结尾（尾部=${bridge.getTerminalText()?.takeLast(80)}）",
                disappeared,
            )

            samples += Sample(presentMs = (presentAt - callbackNs) / 1_000_000, callbackUs = elapsedUs)
        }
        return samples
    }

    /** 等会话就绪、拿到输入连接并让窗口真正持有焦点。 */
    private fun focusTerminalAndWaitForPrompt(bridge: Bridge): InputConnection {
        // 归位到底部：上一个用例可能把视口翻上去了（长按拖柄/翻页都会），此时提示符
        // 在视口之外，文本抽取里根本没有它。
        scrollViewportToBottom(bridge, composeTestRule.activity.terminalViewModel.runtime)
        // 等提示符：等不到时由共享工具向 PTY 要一个新的（见 awaitShellPrompt 的说明）。
        awaitShellPrompt(bridge)

        val surface = onUiThread { findTerminalSurface(composeTestRule.activity) }
        tapToFocus(surface)
        return requireNotNull(onUiThread { surface.onCreateInputConnection(EditorInfo()) }) {
            "onCreateInputConnection 返回 null"
        }
    }

    /** 提交一次文本；失败（抑制窗口内返回 false）时重试到受理。 */
    private fun writeText(connection: InputConnection, text: String) {
        val accepted =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) {
                onUiThread { connection.commitText(text, 1) }
            }
        assertNotNull("commitText 一直被拒（切换余波的抑制窗口）", accepted)
    }

    private fun percentile(samples: List<Long>, fraction: Double): Long {
        val sorted = samples.sorted()
        return sorted[((sorted.size - 1) * fraction).toInt()]
    }

    /** 打包光标 `(row << 32) | col` 的列；未呈现过时为 -1。 */
    private fun Long.column(): Int = (this and 0xFFFF_FFFFL).toInt()

    /** 去掉行尾空白后是否以 [suffix] 结尾——shell 回显与光标在同一行行尾。 */
    private fun String.atPromptEnd(suffix: String): Boolean = trimEnd().endsWith(suffix)
}
