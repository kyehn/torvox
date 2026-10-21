package terminal.emulator.ui

import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.cleanUpTerminalState
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.injectTap
import terminal.emulator.terminalCellSizePx
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 鼠标上报中的触摸端到端（yazi 等 TUI 的点击与滚动）。
 *
 * shell 装载 DECSET 1000/1006 用户鼠标上报后 `cat -v` 把收到的字节按可读
 * 字符倒出来（ESC 显示为 `^[`），随后真实触摸手势必须产生对应的 SGR 序列。
 * 全程走应用级入口（真实触摸事件注入），不直调内部方法。
 */
@RunWith(JUnit4::class)
class MouseReportingInstrumentedTest : TerminalLogcatTest() {
    companion object {
        private const val ARM_TIMEOUT_MS = 15_000L
        private const val OUTPUT_TIMEOUT_MS = 15_000L
        private const val STEP_MILLIS = 50L

        /** 触摸落点：第 6 列（远在 32dp 抽屉边缘区外）、视口第 5 行。 */
        private const val TAP_COL = 6
        private const val TAP_ROW = 5
    }

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", composeTestRule.getBridge())
    }

    private fun bridge(): Bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    /** 泵送事件队列后读全量文本（运行时输出收割需 pollEvent 驱动）。 */
    private fun currentText(): String? {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return composeTestRule.getBridge()?.getTerminalText()
    }

    /**
     * 等 shell 就绪并在其中装载鼠标上报监听：`-echo -icanon` 让 cat 立即
     * 拿到触摸字节且 tty 不二次回显；`cat > mouse.bin` 把原始字节落到文件，
     * 断言直接读字节，不经过终端渲染。ETX 仍能结束 cat（整体 raw 会关信号）。
     */
    private fun armMouseReporting() {
        bridge().writeToPty("\u0003".toByteArray(Charsets.UTF_8))
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = 60_000, intervalMs = 200) {
                val text = currentText()
                text != null && (text.contains("$") || text.contains("#"))
            }
        assertNotNull("shell prompt 必须先就绪, 实际: ${currentText()?.takeLast(200)}", promptSeen)
        bridge().writeToPty(
            (
                "stty -echo -icanon; printf '\\033[?1000h\\033[?1006h'; printf 'MOUSE_ARMED\\n'; " +
                    "cat > \$HOME/mouse.bin\n"
                ).toByteArray(Charsets.UTF_8),
        )
        // MOUSE_ARMED 进入网格 = 其前的 DECSET 已被 VT 消费（同一字节流的顺序保证）。
        val armed =
            UxTestUtils.pollUntilTrue(timeoutMs = ARM_TIMEOUT_MS, intervalMs = 200) {
                currentText()?.contains("MOUSE_ARMED") == true
            }
        assertNotNull("鼠标上报装载失败, 实际: ${currentText()?.takeLast(200)}", armed)
        bridge().setScrollOffset(0)
    }

    /** shell 收到的原始字节（cat > $HOME/mouse.bin 的落盘内容，拉丁 1 解码逐字节保真）。 */
    private fun receivedBytes(): String {
        val file = java.io.File(composeTestRule.activity.dataDir, "files/home/mouse.bin")
        if (!file.exists()) return ""
        return String(file.readBytes(), Charsets.ISO_8859_1)
    }

    /**
     * 复位共享 shell：ETX 结束 cat（-icanon 保留 ISIG），再显式关掉鼠标上报并恢复
     * tty 回显。DECRST 必须真的发出去：DECSET 1000 是终端级模式，`stty sane` 不会
     * 清它，漏掉会让同一会话里后续用例凭空继承「应用在接收鼠标上报」。
     */
    private fun disarmMouseReporting() {
        bridge().writeToPty("\u0003printf '\\033[?1000l\\033[?1006l'; stty sane\n".toByteArray(Charsets.UTF_8))
    }

    /** `ESC[<BUTTON;COL;ROWM`/`m` 的原始 SGR 序列（1 基单元格坐标）。 */
    private fun sgr(button: Int, col: Int, row: Int, release: Boolean): String =
        "\u001b[<$button;${col + 1};${row + 1}${if (release) "m" else "M"}"

    /** 滚轮上行（button 64）的按下次数：批量编码必须成对，故按下数即档位数。 */
    private fun wheelPresses(bytes: String): Int = Regex("\u001b\\[<64;\\d+;\\d+M").findAll(bytes).count()

    private fun wheelReleases(bytes: String): Int = Regex("\u001b\\[<64;\\d+;\\d+m").findAll(bytes).count()

    /** 断言消息用的可读化：ESC 显示为 `^[`。 */
    private fun readable(bytes: String): String = bytes.replace("\u001b", "^[")

    /** 分步拖动：DOWN → 逐格 MOVE → UP（字面量步进，落点与断言同式换算）。 */
    private fun dragGesture(view: View, x: Float, fromY0: Float, toY0: Float) {
        val t0 = SystemClock.uptimeMillis()
        view.post {
            view.dispatchTouchEvent(MotionEvent.obtain(t0, t0, MotionEvent.ACTION_DOWN, x, fromY0, 0))
        }
        Thread.sleep(STEP_MILLIS)
        val y = FloatArray(2)
        // 两段等距步进：第一段的一部分被框架 touch slop 吸收，第二段整格入账。
        for (step in 1..2) {
            val t = SystemClock.uptimeMillis()
            val yStep = fromY0 + (toY0 - fromY0) * step / 2f
            y[0] = yStep
            view.post {
                view.dispatchTouchEvent(MotionEvent.obtain(t0, t, MotionEvent.ACTION_MOVE, x, yStep, 0))
            }
            Thread.sleep(STEP_MILLIS)
        }
        val tUp = SystemClock.uptimeMillis()
        view.post {
            view.dispatchTouchEvent(MotionEvent.obtain(t0, tUp, MotionEvent.ACTION_UP, x, y[0], 0))
        }
        Thread.sleep(2 * STEP_MILLIS)
    }

    @Test
    fun fingerTapInMouseTuiSendsLeftClickAtSameCell() {
        armMouseReporting()
        val (cellWidth, cellHeight) = terminalCellSizePx(composeTestRule.activity, bridge())
        val surface = findTerminalSurface(composeTestRule.activity)
        val tapX = (TAP_COL + 0.5f) * cellWidth
        val tapY = (TAP_ROW + 0.5f) * cellHeight
        val expectPress = sgr(0, TAP_COL, TAP_ROW, release = false)
        val expectRelease = sgr(0, TAP_COL, TAP_ROW, release = true)
        // 抽屉关闭后的 350ms 防护窗会按设计吞掉窗内的首次轻点（防关闭动画穿透误触），
        // 用户会再点一次；用例同形重试，判据仍是「点击最终到达应用」。
        var clicked = false
        repeat(3) { attempt ->
            if (clicked) return@repeat
            injectTap(surface, tapX, tapY)
            clicked =
                UxTestUtils.pollUntilTrue(timeoutMs = 4_000, intervalMs = 200) {
                    val bytes = receivedBytes()
                    val press = bytes.indexOf(expectPress)
                    val release = bytes.indexOf(expectRelease)
                    press >= 0 && release > press
                } != null
            if (!clicked) {
                android.util.Log.w("MouseReportingInstrumentedTest", "轻点第 ${attempt + 1} 次未到达应用，重试")
            }
        }
        assertNotNull(
            "轻点必须产生同坐标的 SGR 左键按下/抬起对 " +
                "(期望 [${readable(expectPress)}] ... [${readable(expectRelease)}]), " +
                "实际[${readable(receivedBytes())}]",
            clicked,
        )
        // 转发成功的轻点整条消费掉：不得再走终端自身的聚焦路径拉起输入法。
        val imeHidden =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 100) {
                findTerminalSurface(composeTestRule.activity)
                    .rootWindowInsets?.isVisible(android.view.WindowInsets.Type.ime()) != true
            }
        assertNotNull("鼠标上报中的轻点属于应用，不得拉起输入法", imeHidden)
        disarmMouseReporting()
        composeTestRule.cleanUpTerminalState()
    }

    @Test
    fun fingerDragInMouseTuiSendsWheelInsteadOfScrollingScrollback() {
        armMouseReporting()
        val (cellWidth, cellHeight) = terminalCellSizePx(composeTestRule.activity, bridge())
        val surface = findTerminalSurface(composeTestRule.activity)
        // 手指上滑两行 = 滚轮上行（SGR button 64）：y 从 row+2 中心滑到 row 中心。
        dragGesture(
            surface,
            x = (TAP_COL + 0.5f) * cellWidth,
            fromY0 = (TAP_ROW + 2 + 0.5f) * cellHeight,
            toY0 = (TAP_ROW + 0.5f) * cellHeight,
        )
        // 手指上滑 = 滚轮上行（SGR button 64，ghostty Button::Four）。
        val wheeled =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 200) {
                receivedBytes().contains("\u001b[<64;")
            }
        assertNotNull(
            "拖动必须转发为滚轮上行, 实际[${readable(receivedBytes())}]",
            wheeled,
        )
        val scrollOffset = composeTestRule.activity.terminalViewModel.runtime.activeSessionScrollOffset()
        assertTrue("应用启用鼠标上报时本地回滚不得被滚动 (offset=$scrollOffset)", scrollOffset == 0)
        disarmMouseReporting()
        composeTestRule.cleanUpTerminalState()
    }

    /**
     * 一次拖动跨多行：每个滚过的行都要有对应的滚轮档，且按下/抬起严格成对。
     *
     * 逐档往返曾是「滚动卡顿」的来源；批量接口改变的只是往返次数，档位一个不能少。
     */
    @Test
    fun multiRowDragForwardsOneReleasePerRow() {
        armMouseReporting()
        val (cellWidth, cellHeight) = terminalCellSizePx(composeTestRule.activity, bridge())
        val surface = findTerminalSurface(composeTestRule.activity)
        // 四行行程：两段 MOVE 各两行，每段一次 onScroll 各编两档。
        dragGesture(
            surface,
            x = (TAP_COL + 0.5f) * cellWidth,
            fromY0 = (TAP_ROW + 4 + 0.5f) * cellHeight,
            toY0 = (TAP_ROW + 0.5f) * cellHeight,
        )
        val forwarded =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 200) {
                wheelPresses(receivedBytes()) >= 3
            }
        val bytes = receivedBytes()
        assertNotNull("四行拖动必须转发多档滚轮, 实际[${readable(bytes)}]", forwarded)
        assertEquals(
            "每个滚轮档必须有对应抬起（按下=抬起）",
            wheelPresses(bytes),
            wheelReleases(bytes),
        )
        disarmMouseReporting()
        composeTestRule.cleanUpTerminalState()
    }

    /** 鼠标上报镜像：DECSET 装载后为真、DECRST 后为假——滚动手势据此免去一次查询往返。 */
    @Test
    fun mouseTrackingMirrorFollowsDecsetState() {
        val bridge = bridge()
        // 同一会话可能被前序用例留在上报态：先归零，断言只针对本用例造成的两次跃迁。
        if (bridge.isMouseTrackingActive()) {
            disarmMouseReporting()
            assertNotNull(
                "前置归零失败：DECRST 之后镜像必须回落",
                UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 200) {
                    !bridge.isMouseTrackingActive()
                },
            )
        }
        assertFalse("未装载 DECSET 时不得声称应用在接收鼠标上报", bridge.isMouseTrackingActive())
        armMouseReporting()
        assertTrue("DECSET 1000 装载后镜像必须为真", bridge.isMouseTrackingActive())
        disarmMouseReporting()
        val cleared =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 200) {
                !bridge.isMouseTrackingActive()
            }
        assertNotNull("DECRST 之后镜像必须回落", cleared)
        composeTestRule.cleanUpTerminalState()
    }
}
