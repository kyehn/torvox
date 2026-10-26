package terminal.emulator

import android.os.SystemClock
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.bridge.Bridge
import terminal.emulator.cleanUpTerminalState
import terminal.emulator.ui.TerminalSurface

/**
 * Quantified verification of the two scroll behaviors the user reported broken and that were only
 * ever verified by code reading:
 *
 * 1. "新命令按回车不自动滚动到底部" — Enter must snap the viewport to the live screen. Metric: ms from Enter
 *    write until surface offset == 0. Budget on the software emulator: ≤ 2000 ms.
 * 2. "滚动闪烁" — PTY output arriving DURING an active scroll gesture must never reset the viewport.
 *    Metric: sampled offset stream during real fling gestures; a "collapse" = one sample losing >
 *    40% of the current offset toward 0 while the finger is still down. Assert zero collapses.
 *
 * Every measured value is logged as `UX_METRIC ...` for trend tracking.
 */
class ScrollBehaviorQuantifiedTest : TerminalLogcatTest() {
    companion object {
        /** 洪流行数上限：400 行 × 约 35ms ≈ 14s，远长于手势时长（约 3.5s）。 */
        private const val FLOOD_LINE_LIMIT = 400

        /** 洪流停止判定的上限（自限流循环到点即停，Ctrl+C 通常让它更早结束）。 */
        private const val FLOOD_SETTLE_TIMEOUT_MS = 30_000L

        /** 连续多少轮回滚长度不变即认定洪流已停。 */
        private const val FLOOD_SETTLE_STABLE_SAMPLES = 3

        /**
         * 等一条覆盖拖动期的帧节奏窗口的上限：窗口满 60 帧才输出，
         * 555ms/帧的软渲染设备需要 33s，留一倍余量。
         */
        private const val WINDOW_WAIT_TIMEOUT_MS = 60_000L
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
    }

    @After
    fun resetSession() = composeTestRule.cleanUpTerminalState()

    private fun surface(): TerminalSurface {
        val content =
            composeTestRule.activity.findViewById<android.view.ViewGroup>(android.R.id.content)
        return findSurfaceInViewTree(content)
            ?: throw AssertionError("TerminalSurface not found in view tree")
    }

    private fun findSurfaceInViewTree(group: android.view.ViewGroup): TerminalSurface? {
        for (i in 0 until group.childCount) {
            val child = group.getChildAt(i)
            if (child is TerminalSurface) return child
            if (child is android.view.ViewGroup) {
                findSurfaceInViewTree(child)?.let {
                    return it
                }
            }
        }
        return null
    }

    /** Fill the scrollback with numbered lines so there is history to scroll into. */
    private fun seedScrollback(bridge: Bridge) {
        // 等待 shell 就绪后再发 seq：冷启动后会话孵化中写入会丢失，导致回滚永不满。
        UxTestUtils.pollUntilTrue(timeoutMs = 15000) {
            (bridge.getTerminalText().orEmpty().contains("$")) || bridge.scrollbackLength() > 0
        }
        bridge.writeToPty("seq 1 400\n".toByteArray(Charsets.UTF_8))
        UxTestUtils.pollUntilTrue(timeoutMs = 15000) {
            bridge.scrollbackLength() > 150
        } ?: throw AssertionError("scrollback did not fill (len=${bridge.scrollbackLength()})")
        // Wait until the initial flood settles so gesture sampling is clean.
        Thread.sleep(800)
    }

    @Test
    fun enter_snaps_viewport_to_bottom_within_budget() {
        var bridge: terminal.emulator.bridge.Bridge? = null
        val deadline = System.currentTimeMillis() + 15000
        while (System.currentTimeMillis() < deadline) {
            bridge = composeTestRule.getBridge()
            if (bridge != null) break
            Thread.sleep(500)
        }
        val activeBridge = bridge ?: throw AssertionError("bridge null after wait")
        seedScrollback(activeBridge)
        val view = surface()

        // Scroll INTO history with a downward finger drag (older content):
        // 经 view 管线直发触摸序列(与 flood 测试同路径),避免 UiDevice 系统滑动被抽屉/手势拦截导致零位移 flake。
        val centerX = device.displayWidth / 2
        val downTime = android.os.SystemClock.uptimeMillis()

        fun postToView(action: Int, x: Float, y: Float) {
            val eventTime = android.os.SystemClock.uptimeMillis()
            view.post {
                view.dispatchTouchEvent(
                    android.view.MotionEvent.obtain(downTime, eventTime, action, x, y, 0),
                )
            }
        }
        postToView(android.view.MotionEvent.ACTION_DOWN, centerX.toFloat(), 500f)
        for (i in 1..14) {
            Thread.sleep(30)
            postToView(android.view.MotionEvent.ACTION_MOVE, centerX.toFloat(), (500 + i * 50).toFloat())
        }
        Thread.sleep(300)
        postToView(android.view.MotionEvent.ACTION_UP, centerX.toFloat(), 1200f)
        Thread.sleep(600)
        val scrolledUpOffset = view.getScrollOffset()
        assertTrue(
            "precondition failed: swipe did not scroll into history (offset=$scrolledUpOffset)",
            scrolledUpOffset > 0,
        )

        // 回车经输入路径（ViewModel）即时贴底，不等 PTY 回显：直接调桥会绕过该路径，
        // 在过载机上靠输出回路贴底必然漂移。按产品真实路径送回车。
        val enterSessionId = composeTestRule.activeSessionId()
        androidx.test.platform.app.InstrumentationRegistry.getInstrumentation().runOnMainSync {
            composeTestRule.activity.terminalViewModel.writeToPty(enterSessionId, "\n".toByteArray(Charsets.UTF_8))
        }
        val elapsed = UxTestUtils.pollUntilTrue(timeoutMs = 2_000) { view.getScrollOffset() == 0 }
        assertNotNull("viewport never snapped to bottom after Enter", elapsed)
        val elapsedMs = requireNotNull(elapsed)
        UxTestUtils.metric("enter_snap_ms", elapsedMs)
        assertTrue("enter snap took ${elapsedMs}ms (>2000ms budget)", elapsedMs <= 2_000)
    }

    @Test
    fun pty_flood_never_resets_viewport_mid_gesture() {
        var floodBridge: terminal.emulator.bridge.Bridge? = null
        val floodDeadline = System.currentTimeMillis() + 15000
        while (System.currentTimeMillis() < floodDeadline) {
            floodBridge = composeTestRule.getBridge()
            if (floodBridge != null) break
            Thread.sleep(500)
        }
        val bridge = floodBridge ?: throw AssertionError("bridge null after wait")
        seedScrollback(bridge)
        val view = surface()
        val centerX = device.displayWidth / 2

        // 持续输出洪流 —— 被报闪烁的触发条件。循环自带行数上限：`while true` +
        // Ctrl+C 停流并不可靠，SIGINT 送到前台进程组时循环可能已 fork 出新的
        // `sleep`，随即再跑一轮，流一直打到整个测试进程结束。上限取手势时长
        // （4 轮 × 14 步 × 30ms + 落定）的一个数量级以上，保证手势期间流仍在。
        bridge.writeToPty(
            (
                "i=0; while [ \$i -lt $FLOOD_LINE_LIMIT ]; do echo FLOOD_\$(date +%s%N); " +
                    "sleep 0.03; i=\$((i+1)); done\n"
                ).toByteArray(Charsets.UTF_8),
        )
        Thread.sleep(500)

        var gestures = 0
        var maxOffsetSeen = 0
        var collapses = 0
        repeat(4) {
            // Sample offsets every ~30ms while the finger is down, driving the
            // gesture through the view's own touch pipeline (house pattern).
            val dt = SystemClock.uptimeMillis()

            fun post(action: Int, x: Float, y: Float) {
                val t = SystemClock.uptimeMillis()
                view.post {
                    view.dispatchTouchEvent(
                        android.view.MotionEvent.obtain(dt, t, action, x, y, 0),
                    )
                }
            }
            post(android.view.MotionEvent.ACTION_DOWN, centerX.toFloat(), 900f)
            var previous = view.getScrollOffset()
            var sawGestureMovement = false
            val sampleCount = 14
            for (i in 0 until sampleCount) {
                Thread.sleep(30)
                // Drag downward slowly (into older content):手指下移 distanceY 为负,
                // 取反累加后偏移增加(older),与 termux doScroll 一致。
                post(
                    android.view.MotionEvent.ACTION_MOVE,
                    centerX.toFloat(),
                    (900 + (i + 1) * 25).toFloat(),
                )
                val current = view.getScrollOffset()
                maxOffsetSeen = maxOf(maxOffsetSeen, current)
                if (current != previous) sawGestureMovement = true
                // Collapse definition: while the finger is DOWN the viewport
                // suddenly loses >40% of its offset toward 0 — that is the
                // reported flicker (new output resetting scroll), not a
                // user action.
                if (previous > 20 && current < previous * 0.6f && current < 10) {
                    collapses++
                    android.util.Log.i(
                        "UX_METRIC",
                        "scroll_collapse_at_offset prev=$previous now=$current",
                    )
                }
                previous = current
            }
            post(android.view.MotionEvent.ACTION_UP, centerX.toFloat(), 1250f)
            if (sawGestureMovement) gestures++
            Thread.sleep(400)
        }

        // 到点自停；Ctrl+C 只作提前结束的补充手段。
        bridge.writeToPty("\u0003".toByteArray(Charsets.UTF_8))
        // 等回滚停止增长：确认洪流真的停了，@After 的清屏才不会与在途输出竞态。
        var settledLength = bridge.scrollbackLength()
        var stableSamples = 0
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = FLOOD_SETTLE_TIMEOUT_MS, intervalMs = 300) {
                val current = bridge.scrollbackLength()
                if (current == settledLength) {
                    stableSamples++
                } else {
                    settledLength = current
                    stableSamples = 0
                }
                stableSamples >= FLOOD_SETTLE_STABLE_SAMPLES
            }
        assertNotNull("洪流未在 ${FLOOD_SETTLE_TIMEOUT_MS}ms 内停止增长", settled)
        Thread.sleep(400)

        assertTrue("no gesture produced scroll movement (max=$maxOffsetSeen)", maxOffsetSeen > 0)
        assertTrue("at least one gesture should register movement", gestures >= 1)
        UxTestUtils.metric("scroll_collapses_under_flood", collapses)
        assertTrue(
            "viewport collapsed $collapses time(s) mid-gesture under PTY flood — the flicker bug",
            collapses == 0,
        )
    }

    /**
     * 持续滚动期间的循环帧节奏：客户端报障「滚动卡顿」。
     *
     * 仪器复用生产代码已有的 loop timing 窗口（`TerminalRuntime` 每 60 帧输出一行
     * `loop timing window ... p95=.. max=.. ≈Nfps`）——那是唯一覆盖「门控→推状态→
     * 渲染→pollAll→尾部队列」整循环的计量，真机与模拟器同款。
     *
     * 两条被证伪的路子记在这里，避免后来者重走：
     * - `Window.OnFrameMetricsAvailableListener` 在滚动期采到 **0 帧**：终端内容由原生
     *   渲染线程直接画进 SurfaceView 的 Surface，不经过主线程 `ViewRootImpl` 遍历。
     * - `PixelCopy` 读回已呈现缓冲：360×420 的条带单次也要 ~300ms，只能到 3 样本/秒。
     *
     * 采样窗口只取**手指按住且正在移动**期间完整结束的那些：窗口跨到松手后的驻留帧
     * 会混入 500ms 的合法空转帧，那测的是空闲节拍而非滚动。持续按住 2.5s 保证至少
     * 一个窗口在中途收满 60 帧。
     */
    @Test
    fun drag_loop_frame_pacing_stays_smooth() {
        var bridge: Bridge? = null
        val deadline = System.currentTimeMillis() + 15000
        while (System.currentTimeMillis() < deadline) {
            bridge = composeTestRule.getBridge()
            if (bridge != null) break
            Thread.sleep(500)
        }
        val activeBridge = bridge ?: throw AssertionError("bridge null after wait")
        seedScrollback(activeBridge)
        val view = surface()
        val centerX = device.displayWidth / 2f
        val before = view.getScrollOffset()

        val dragStartMs = System.currentTimeMillis()
        val dt = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            android.view.MotionEvent.obtain(dt, dt, android.view.MotionEvent.ACTION_DOWN, centerX, 300f, 0),
        )
        var y = 300f
        while (System.currentTimeMillis() - dragStartMs < 2500) {
            Thread.sleep(16)
            // 单向匀速下移（手指下移 = 翻向更早的内容）。全程不折返：反向移动会
            // 让视口回落，末态可能又回到 0，使「是否滚动」这一前置断言失去意义。
            y = minOf(y + 10f, device.displayHeight - 200f)
            val now = SystemClock.uptimeMillis()
            view.dispatchTouchEvent(
                android.view.MotionEvent.obtain(dt, now, android.view.MotionEvent.ACTION_MOVE, centerX, y, 0),
            )
        }
        val dragEndMs = System.currentTimeMillis()
        val upAt = SystemClock.uptimeMillis()
        view.dispatchTouchEvent(
            android.view.MotionEvent.obtain(dt, upAt, android.view.MotionEvent.ACTION_UP, centerX, y, 0),
        )

        assertTrue(
            "持续拖动未翻向历史（offset $before -> ${view.getScrollOffset()}），帧节奏测量无意义",
            view.getScrollOffset() > before,
        )

        val initial = UxTestUtils.logcatSince(dragStartMs)
        // 原生侧的呈现诊断按 500ms 窗口汇总：每行一条，窗口内的呈现帧数记在该行里。
        val presentSummaries = initial.count { it.contains("render_frame: presented") }
        assertTrue(
            "持续滚动 2.5s 内没有任何一帧被呈现（呈现汇总 0 条）：\n" +
                initial.joinToString("\n").takeLast(800),
            presentSummaries > 0,
        )
        // 阶段耗时窗口满 60 帧才输出：真机 60 FPS 下约 1s，软件渲染模拟器一帧 ~110ms
        // 就要 6s 以上，必然晚于 2.5s 的拖动结束。要求「窗口在拖动期内结束」等于把
        // 设备速度写进用例——慢设备上永远采不到样本。改为等一条**覆盖**拖动期的窗口：
        // 覆盖区间由窗口自身计量反推（epoch − frames × avg），与设备快慢无关。
        var lines = initial
        val sawWindow =
            UxTestUtils.pollUntilTrue(timeoutMs = WINDOW_WAIT_TIMEOUT_MS, intervalMs = 500) {
                lines = UxTestUtils.logcatSince(dragStartMs)
                lines
                    .mapNotNull { UxTestUtils.LoopTimingWindow.parse(it) }
                    .any { it.covers(dragStartMs, dragEndMs) }
            }
        assertNotNull(
            "等待 ${WINDOW_WAIT_TIMEOUT_MS}ms 仍没有一条帧节奏窗口覆盖拖动期" +
                "（呈现汇总 $presentSummaries 条）——仪器失效，不能当作通过",
            sawWindow,
        )
        // 阶段耗时有两个来源，按设备速度各自出现：慢帧走 SLOW_LOOP（逐帧、500ms 节流），
        // 快设备只在整窗汇总里留下 pre_max。两者都取上界。
        val slowLoopPreMs =
            lines.mapNotNull {
                loopPrePattern
                    .find(it)
                    ?.groupValues
                    ?.get(1)
                    ?.toDoubleOrNull()
            }
        val windows =
            lines
                .mapNotNull { UxTestUtils.LoopTimingWindow.parse(it) }
                .filter { it.covers(dragStartMs, dragEndMs) }
        val worstPre = (slowLoopPreMs + windows.map { it.preMaxMs }).maxOrNull()
        assertTrue(
            "滚动期没有采到任何阶段耗时样本（呈现汇总 $presentSummaries 条）——仪器失效，不能当作通过",
            worstPre != null,
        )
        windows.maxByOrNull { it.p95Ms }?.let { window ->
            UxTestUtils.metric("drag_loop_p95_ms", window.p95Ms.toLong())
            UxTestUtils.metric("drag_loop_fps", window.fps.toLong())
        }
        val preMax = worstPre ?: 0.0
        UxTestUtils.metric("drag_loop_pre_max_ms", preMax.toLong())
        // 按 p95 判定：单帧调度抖动（GC/抢占）不属于循环自身工作， sustained 的阻塞
        // 才是回归。worstPre 仍用于仪器有效性与诊断。
        val slowLoopPreP95 =
            if (slowLoopPreMs.isEmpty()) {
                0.0
            } else {
                val sorted = slowLoopPreMs.sorted()
                val idx = (sorted.size * 95 + 99) / 100 - 1
                sorted[idx.coerceIn(0, sorted.lastIndex)]
            }
        val windowPreP95 = windows.map { it.preP95Ms }.maxOrNull() ?: 0.0
        val worstPreP95 = maxOf(slowLoopPreP95, windowPreP95)
        UxTestUtils.metric("drag_loop_pre_p95_ms", worstPreP95.toLong())
        // 只断言生产代码控制得了的那一段：门控之后、渲染 JNI 之前的循环自身工作
        // （推选区 / 滚动偏移 / 滚动余量）。帧率与整窗 p95 取决于 GPU —— 本模拟器
        // （SwiftShader）单帧渲染就要 ~130ms，那是设备能力而不是回归，故只记指标。
        assertTrue(
            "滚动期循环自身工作 p95 用掉 ${worstPreP95}ms，超过 8ms：\n" + windows.joinToString("\n"),
            worstPreP95 <= 8.0,
        )
    }

    /** `SLOW_LOOP ... pre=<ms>ms ...` 里的循环自身工作时长。 */
    private val loopPrePattern = Regex("""SLOW_LOOP .*?\bpre=([\d.E-]+)ms""")
}
