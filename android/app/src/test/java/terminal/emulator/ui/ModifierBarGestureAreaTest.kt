package terminal.emulator.ui

import androidx.compose.ui.geometry.Rect
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 按键的按下反馈是否该被系统手势区吞掉（[isInsideSystemGestureArea]）。
 *
 * 修饰键栏贴着屏幕底边，全面屏手势导航的 home 手势区就在键栏底边那一带。系统认领
 * 手势时先给应用 `ACTION_DOWN`，再补发 `ACTION_CANCEL`——中间任何一帧画出的按下
 * 反馈，都是用户看到的「上滑触发了按键动画」。区内不给反馈，问题便从根上消失，
 * 不必再猜系统认领手势要多久。
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
}
