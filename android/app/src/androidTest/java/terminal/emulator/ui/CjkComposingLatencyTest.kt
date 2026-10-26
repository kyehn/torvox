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
import terminal.emulator.findTerminalSurface
import terminal.emulator.injectTap
import terminal.emulator.waitForSession

/**
 * STATUS ⑥「简体中文退格缓慢」的量化验证：输入法逐键回调在主线程上的耗时。
 *
 * TRIGGER 逐键 `setComposingText`（拼音回退＝一次退格），METRIC 该回调自身在**主线程**
 * 上的纳秒耗时，分「光标监视开」与「光标监视关」两组，其余字节路径完全相同。
 *
 * 组字变化时终端会把光标锚点回报给输入法。若该回报走同步 VT 查询，每次组字都要在
 * 主线程上等一整帧（PTY 回显 → 渲染帧 → VT 循环节拍），开启监视的那组随之为数十
 * 毫秒——这正是「退格明显缓慢」的机制。两组的 p50 必须同量级：差值即锚点上报的
 * 等待成本，也是本用例钉住的那条回归线。
 */
class CjkComposingLatencyTest : TerminalLogcatTest() {
    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private companion object {
        /** 每组样本数：每个样本一次组字回退。 */
        const val SAMPLES = 20

        /** 样本间隔：大于锚点回报的 100ms 限频窗口，保证每个样本都真的发起一次查询。 */
        const val SAMPLE_GAP_MS = 120L

        /** 组字串：逐键回退即 `zhongguo` → `zhonggu` → … 的单字符退格序列。 */
        const val PINYIN = "zhongguo"

        /**
         * 两组 p50 之间允许的差值（微秒）。
         *
         * 校准依据：本机模拟器上（软件渲染）锚点走同步 VT 查询时该差值为**数万微秒**
         * （等待一整帧），改走帧内已发布的光标后落到千微秒以内。取 8ms（半帧）既
         * 远离修复后的实测值，又低于缺陷态一个数量级——它钉的是「锚点上报不得吞掉
         * 半帧以上」，不是模拟器的绝对速度。
         */
        const val GAP_BUDGET_US = 8_000L
    }

    /**
     * 一次组字回退：自身耗时（微秒）、是否被受理（false＝切换余波的抑制窗口内被拒），
     * 以及逃出回调的异常原文（非空即被测路径自己抛了，样本据此判不可信）。
     */
    private data class Edit(val elapsedUs: Long, val accepted: Boolean, val error: String? = null)

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    @Test
    fun composing_backspace_does_not_wait_on_cursor_anchor_query() {
        val bridge = composeTestRule.awaitBridge()
        awaitShellPrompt(bridge)

        val surface = onUiThread { findTerminalSurface(composeTestRule.activity) }
        tapToFocus(surface)
        val input = onUiThread { surface.onCreateInputConnection(EditorInfo()) }
        assertNotNull("onCreateInputConnection 返回 null", input)
        val connection = requireNotNull(input)

        // 输入法的真实用法：请求光标监视（`CURSOR_UPDATE_MONITOR` 位）后才逐键回报。
        onUiThread { connection.requestCursorUpdates(InputConnection.CURSOR_UPDATE_MONITOR) }
        // 触摸到焦点落地之间有一段输入抑制窗口（获焦/失焦都置位 50ms），期间输入法
        // 写入被静默拒收（返回 false 且不发声）。等第一次真的走到锚点回报那一步再开始
        // 采样——否则整段采样都在空转，两组耗时都漂亮得像假的。
        //
        // 判据是「受理**或**抛异常」：抑制窗口只静默返回 false，而走到锚点回报的调用
        // 要么正常返回 true，要么在回报里抛（后者由下方断言判为缺陷，其耗时仍然作数）。
        var firstWriteError: String? = null
        val reached =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
                try {
                    onUiThread { connection.setComposingText(PINYIN, 1) }
                } catch (reachedWithThrow: RuntimeException) {
                    // 走到锚点回报的调用可能在这里抛：仍算「到达」，但异常原文必须留住——
                    // 它是缺陷证据，由采样结束后的断言一并判死（探针期异常=…）。
                    firstWriteError = describe(reachedWithThrow)
                    true
                }
            }
        assertNotNull("输入法写入一直被拒（切换余波的抑制窗口），样本会退化成空转", reached)
        // 组字字节真的写进 PTY 才算测到了这条路径：空转的循环也会给出漂亮的耗时。
        val echoSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                bridge.getTerminalText()?.contains(PINYIN) == true
            }
        assertNotNull(
            "组字未落格，样本无效（终端文本尾部=${bridge.getTerminalText()?.takeLast(80)}）",
            echoSeen,
        )

        // 锚点坐标取自**最近一帧**的光标：此刻必须取得到，否则锚点上报整段静默跳过，
        // 两组耗时都会「漂亮」得像没事——先把坐标源本身钉住。
        val renderedCursor =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 50) {
                bridge.renderedCursorPacked() >= 0L
            }
        assertNotNull("帧内光标一直取不到，锚点上报会整段跳过，样本无效", renderedCursor)

        val growthSteps = mutableListOf<Edit>()
        // 预热一轮（结果丢弃）：让限频窗口与组字基线先落定，正式两组才好对照。
        sampleEdits(connection, growthSteps)
        val monitoringOn = sampleEdits(connection, growthSteps)
        onUiThread { connection.requestCursorUpdates(0) }
        sampleEdits(connection, growthSteps)
        val monitoringOff = sampleEdits(connection, growthSteps)

        val monitoringOnSamples = monitoringOn.map { it.elapsedUs }
        val monitoringOffSamples = monitoringOff.map { it.elapsedUs }
        val onP50 = percentile(monitoringOnSamples, 0.50)
        val offP50 = percentile(monitoringOffSamples, 0.50)
        UxTestUtils.metric("composing_edit_on_p50_us", onP50)
        UxTestUtils.metric("composing_edit_on_p95_us", percentile(monitoringOnSamples, 0.95))
        UxTestUtils.metric("composing_edit_off_p50_us", offP50)
        UxTestUtils.metric("composing_edit_off_p95_us", percentile(monitoringOffSamples, 0.95))

        // 先记指标再判受理：异常样本的耗时同样是证据，先落日志才读得到。
        val firstBad = (monitoringOn + monitoringOff).firstOrNull { it.error != null || !it.accepted }
        // 生长步与探针期的异常同样进判据：它们不进耗时统计，但「回调以异常收场」是缺陷，
        // 不能因为不计时就丢掉（此前两者都吞在 `runCatching` 里，见 lint 的 DenyListedApi）。
        val firstGrowthError = growthSteps.firstOrNull { it.error != null }
        assertTrue(
            "组字回调未正常受理（${firstBad?.error ?: "返回 false"}；" +
                "探针期异常=${firstWriteError ?: "无"}；生长步异常=${firstGrowthError?.error ?: "无"}）" +
                "：锚点回报不得让输入法回调以异常收场",
            firstBad == null && firstGrowthError == null && firstWriteError == null,
        )

        assertTrue(
            "光标监视开启后每次组字回退的主线程耗时 p50=${onP50}us，" +
                "关闭监视的对照组 p50=${offP50}us，差值超出 ${GAP_BUDGET_US}us —— " +
                "锚点回报又走回了会等一整帧的同步查询（on=$monitoringOnSamples off=$monitoringOffSamples）",
            onP50 - offP50 <= GAP_BUDGET_US,
        )
    }

    /**
     * 采集一组样本：每个样本先长回完整拼音（不计时），隔开限频窗口后再回退一个字符（计时）。
     *
     * 生长步必须先于间隔：它自己也会发起一次锚点回报，紧邻的计时步会被限频挡掉，
     * 那样两组都测不到查询，用例就变成恒真的。
     *
     * 生长步的结果不进耗时统计，但异常收进 [growthSteps]：抛点在锚点回报里，与紧随的计时步
     * 同一个（字节与组字基线在抛点之前就已更新），记下来才不至于把证据藏起来。
     */
    private fun sampleEdits(input: InputConnection, growthSteps: MutableList<Edit>): List<Edit> = buildList {
        repeat(SAMPLES) {
            growthSteps += recordEdit(input, PINYIN)
            Thread.sleep(SAMPLE_GAP_MS)
            add(recordEdit(input, PINYIN.dropLast(1)))
        }
    }

    /** 在 UI 线程执行一次组字写入，返回该回调自身耗时（微秒）与受理/异常结果。 */
    private fun recordEdit(input: InputConnection, text: String): Edit {
        var elapsedUs = 0L
        var accepted = false
        var error: String? = null
        onUiThread {
            val start = System.nanoTime()
            try {
                accepted = input.setComposingText(text, 1)
            } catch (throwable: Throwable) {
                // 采样不因被测路径抛异常而中断：异常本身是证据（实测修正前锚点回报在
                // `CursorAnchorInfo.Builder.build()` 处必抛，异常逃出回调），
                // 断言据 [Edit.error] 报出原文。耗时仍照记——抛异常前的等待真发生过。
                error = describe(throwable)
            }
            elapsedUs = (System.nanoTime() - start) / 1_000
        }
        return Edit(elapsedUs, accepted, error)
    }

    private fun describe(throwable: Throwable): String = "${throwable.javaClass.simpleName}: ${throwable.message}"

    /**
     * 触摸一次让窗口真正拿到焦点，并清掉切换余波的抑制窗口。
     *
     * 交付本身不因失焦被拒（`isPaused` 只用于保住组字基线），但窗口获得焦点会开一个
     * 有界的余波窗口；单次抬起的处理会清掉该窗口并 `requestFocus()`，与
     * [terminal.emulator.ui.ImePopupPixelInstrumentedTest] 驱动输入法前的那一次触摸同路。
     */
    private fun tapToFocus(surface: View) {
        val location = IntArray(2)
        onUiThread { surface.getLocationOnScreen(location) }
        injectTap(
            surface,
            (location[0] + surface.width / 2).toFloat(),
            (location[1] + surface.height / 2).toFloat(),
        )
        val focused =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
                onUiThread { surface.hasWindowFocus() }
            }
        assertNotNull("触摸后窗口仍未获得焦点，输入法写入会被拒收", focused)
    }

    private fun percentile(samples: List<Long>, fraction: Double): Long {
        val sorted = samples.sorted()
        val index = ((sorted.size - 1) * fraction).toInt()
        return sorted[index]
    }

    /**
     * 同步地在 UI 线程执行 [block]。
     *
     * 必须用 `runOnMainSync` 而不是 `Activity.runOnUiThread`：后者只是 `post`，立即返回，
     * 读到的结果恒为空（本用例第一版即因此报「onCreateInputConnection 返回 null」，
     * 而视图其实拿对了）——耗时也就量成了零。
     */
    private fun <T> onUiThread(block: () -> T): T {
        var result: T? = null
        InstrumentationRegistry.getInstrumentation().runOnMainSync { result = block() }
        @Suppress("UNCHECKED_CAST")
        return result as T
    }
}
