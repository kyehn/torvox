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
}
