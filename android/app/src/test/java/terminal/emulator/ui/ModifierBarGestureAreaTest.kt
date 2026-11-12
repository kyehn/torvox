package terminal.emulator.ui

import androidx.compose.ui.geometry.Rect
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 按键的按下反馈是否落在系统手势区（[isInsideSystemGestureArea]）。
 *
 * 修饰键栏贴着屏幕底边，全面屏手势导航的 home 手势区就在键栏底边那一带。系统认领
 * 手势时先给应用 `ACTION_DOWN`，再补发 `ACTION_CANCEL`——中间任何一帧画出的按下
 * 反馈，都是用户看到的「上滑触发了按键动画」。认领时机取决于手指移动快慢，没有上界，
 * 故区内不在按下时给反馈（轻点抬手给脉冲、长按/连发触发时再亮），区外的触摸立即给反馈。
 */
class ModifierBarGestureAreaTest {

    /** 典型全面屏手势布局：左右各 30px 的 back 手势区，底部 32px 的 home 手势区。 */
    private val gestureArea =
        SystemGestureArea(left = 30, top = 0, right = 30, bottom = 32)

    private val geometry =
        PressFeedbackGeometry(
            keyBoundsInRoot = Rect(left = 40f, top = 560f, right = 100f, bottom = 596f),
            viewportWidth = 320,
            viewportHeight = 640,
        )

    @Test
    fun `touch in the bottom home gesture area is inside`() {
        assertTrue(
            "底部 home 手势区内的触摸必须判为区内",
            isInsideSystemGestureArea(
                touchX = 70f,
                touchY = 620f,
                geometry = geometry,
                gestureArea = gestureArea,
            ),
        )
    }

    @Test
    fun `touch above the home gesture area is outside`() {
        assertFalse(
            "手势区上方的触摸必须判为区外",
            isInsideSystemGestureArea(
                touchX = 70f,
                touchY = 600f,
                geometry = geometry,
                gestureArea = gestureArea,
            ),
        )
    }

    @Test
    fun `touch in the left back gesture area is inside`() {
        assertTrue(
            "左侧 back 手势区内的触摸必须判为区内",
            isInsideSystemGestureArea(
                touchX = 10f,
                touchY = 570f,
                geometry = geometry,
                gestureArea = gestureArea,
            ),
        )
    }

    @Test
    fun `touch in the right back gesture area is inside`() {
        assertTrue(
            "右侧 back 手势区内的触摸必须判为区内",
            isInsideSystemGestureArea(
                touchX = 310f,
                touchY = 570f,
                geometry = geometry,
                gestureArea = gestureArea,
            ),
        )
    }

    @Test
    fun `touch on the gesture area border is outside`() {
        assertFalse(
            "恰好落在手势区边界上的触摸不算区内",
            isInsideSystemGestureArea(
                touchX = 30f,
                touchY = 608f,
                geometry = geometry,
                gestureArea = gestureArea,
            ),
        )
    }

    @Test
    fun `touch is outside when the viewport is not laid out yet`() {
        assertFalse(
            "视口尺寸未就绪时不得吞掉按键反馈",
            isInsideSystemGestureArea(
                touchX = 70f,
                touchY = 620f,
                geometry = geometry.copy(viewportWidth = 0, viewportHeight = 0),
                gestureArea = gestureArea,
            ),
        )
    }

    @Test
    fun `no gesture area means every touch is outside`() {
        assertFalse(
            "没有手势区时不得吞掉按键反馈",
            isInsideSystemGestureArea(
                touchX = 70f,
                touchY = 620f,
                geometry = geometry,
                gestureArea = SystemGestureArea(left = 0, top = 0, right = 0, bottom = 0),
            ),
        )
    }

    @Test
    fun `gesture area presses only give feedback after a still hold`() {
        // 区内不在按下时给反馈：认领时机取决于手指移动快慢，任何固定延迟窗口
        // 都会被慢速起滑穿过；区外的触摸系统不会认领，立即给反馈。
        assertFalse(
            "手势区内的按下不得立即启动反馈",
            pressFeedbackStartsImmediately(insideSystemGestureArea = true),
        )
        assertTrue(
            "手势区外的按下必须立即给出反馈",
            pressFeedbackStartsImmediately(insideSystemGestureArea = false),
        )
    }

    @Test
    fun `the still hold threshold is shorter than the long press`() {
        // 静止按住的反馈必须早于长按/连发亮起：否则用户看到的是「连发」而不是
        // 「按压」，而底边那排不连发的按键就永远没有按下反馈。
        assertTrue(
            "静止按住的反馈阈值必须短于长按阈值",
            PRESS_FEEDBACK_HOLD_MS < modifierLongPressMillis(500L),
        )
        assertTrue(
            "静止按住的反馈阈值必须为正，否则退化成按下即亮（等于上滑会触发动画）",
            PRESS_FEEDBACK_HOLD_MS > 0,
        )
    }

    @Test
    fun `the jitter radius is half of the system touch slop`() {
        // 抖动半径取系统 touchSlop 的一半：足够大到容得下生理抖动，又足够小到
        // 慢速上滑（每步只挪一两像素）也会越过它。
        assertEquals(4f, stillHoldJitterRadiusPx(8f), 0.01f)
        assertEquals(0f, stillHoldJitterRadiusPx(0f), 0.01f)
    }

    @Test
    fun `jitter around the press point is not a significant movement`() {
        // 生理抖动只在按下点附近振荡，累计距离不会越过半径：它 MUST NOT 推迟反馈
        // （被计成移动的话，抖动的用户永远看不到按下反馈）。
        val radius = stillHoldJitterRadiusPx(8f)
        assertFalse(
            "按下点未动不得算作有效移动",
            isSignificantMovement(distanceFromDown = 0f, jitterRadiusPx = radius),
        )
        assertFalse(
            "抖动半径内的累计距离不得算作有效移动",
            isSignificantMovement(distanceFromDown = radius, jitterRadiusPx = radius),
        )
    }

    @Test
    fun `moving away from the press point is a significant movement`() {
        // 上滑一路远离按下点，累计距离很快越过半径：计时随之重算，慢速上滑因此
        // 全程等不到反馈亮起。
        val radius = stillHoldJitterRadiusPx(8f)
        assertTrue(
            "越过抖动半径的累计距离必须算作有效移动",
            isSignificantMovement(distanceFromDown = radius + 0.5f, jitterRadiusPx = radius),
        )
    }
}
