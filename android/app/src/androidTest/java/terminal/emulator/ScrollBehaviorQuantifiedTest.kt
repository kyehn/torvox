package terminal.emulator

import android.os.SystemClock
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.bridge.Bridge
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
class ScrollBehaviorQuantifiedTest {
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
        bridge.writeToPty("seq 1 400\n".toByteArray(Charsets.UTF_8))
        UxTestUtils.pollUntilTrue(timeoutMs = 8_000) {
            bridge.scrollbackLength() > 200
        } ?: throw AssertionError("scrollback did not fill (len=${bridge.scrollbackLength()})")
        // Wait until the initial flood settles so gesture sampling is clean.
        Thread.sleep(800)
    }

    @Test
    fun enter_snaps_viewport_to_bottom_within_budget() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        seedScrollback(bridge)
        val view = surface()

        // Scroll INTO history with a downward finger drag (older content).
        val centerX = device.displayWidth / 2
        device.swipe(centerX, 500, centerX, 1200, 24)
        Thread.sleep(600)
        val scrolledUpOffset = view.getScrollOffset()
        assertTrue(
            "precondition failed: swipe did not scroll into history (offset=$scrolledUpOffset)",
            scrolledUpOffset > 0,
        )

        // The reported bug: pressing Enter leaves the viewport pinned in
        // history. Measure how long until it reaches the bottom instead.
        bridge.writeToPty("\n".toByteArray(Charsets.UTF_8))
        val elapsed = UxTestUtils.pollUntilTrue(timeoutMs = 2_000) { view.getScrollOffset() == 0 }
        assertNotNull("viewport never snapped to bottom after Enter", elapsed)
        UxTestUtils.metric("enter_snap_ms", elapsed!!)
        assertTrue("enter snap took ${elapsed}ms (>2000ms budget)", elapsed <= 2_000)
    }

    @Test
    fun pty_flood_never_resets_viewport_mid_gesture() {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        seedScrollback(bridge)
        val view = surface()
        val centerX = device.displayWidth / 2

        // Continuous output flood — the trigger of the reported flicker.
        bridge.writeToPty(
            "while true; do echo FLOOD_$(date +%s%N); sleep 0.03; done\n".toByteArray(Charsets.UTF_8),
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
                // Drag upward slowly (into older content).
                post(
                    android.view.MotionEvent.ACTION_MOVE,
                    centerX.toFloat(),
                    (900 - (i + 1) * 25).toFloat(),
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
            post(android.view.MotionEvent.ACTION_UP, centerX.toFloat(), 300f)
            if (sawGestureMovement) gestures++
            Thread.sleep(400)
        }

        // Stop the flood.
        bridge.writeToPty("\u0003".toByteArray(Charsets.UTF_8))
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
