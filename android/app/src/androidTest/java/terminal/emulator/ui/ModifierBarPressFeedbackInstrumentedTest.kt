package terminal.emulator.ui

import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.hideImeAndConfirm
import terminal.emulator.isImeVisible
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 修饰键栏底边那排按键的按下反馈必须看得见（STATUS「修饰键底部列触发无动画反馈」）。
 *
 * 键栏底边正落在系统底部手势区（home 手势）里，系统认领手势时先给应用
 * `ACTION_DOWN`、再补发 `ACTION_CANCEL`。反馈若整个吞掉，底边那排按键就永远没有
 * 反馈；现在的口径是**延后**一段启动，手势没被认领时照常给出来。
 *
 * 用例量两件事：
 * 1. 按键区域本身取到样（不是空白处）——否则量到的「无变化」毫无意义；
 * 2. 按住期间该区域**逐像素**有足够比例的变化——按下反馈是给整块按键铺一层底色
 *    并整体缩放，只看「颜色集合变了」放不住别的画法。
 *
 * 按的键 MUST NOT 是切换类按键（CTRL/ALT）：切换会改变按键自身的置起外观，
 * 「按下前后颜色不同」就与按下反馈无关了，故取不切换的方向键。
 *
 * 触摸经 View 派发（不经系统注入）：注入到系统的那一路会被底部 home 手势认领走——
 * 实测按下瞬间系统就把应用送回桌面，量到的全是桌面像素（按键本身没有毛病）。
 *
 * 取样前必须推进组合时钟（`waitForIdle`）：按下反馈是动画，不推进时钟它根本不走，
 * 量到的永远是未按下的画面（实测不推进时钟时按键区域逐像素无变化）。
 */
@RunWith(JUnit4::class)
class ModifierBarPressFeedbackInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val IME_HIDDEN_TIMEOUT_MS = 10_000L

        /** 取样区相对按键四边的内缩（像素）：躲开圆角，按键内部才是一片整色。 */
        private const val SAMPLE_INSET_PX = 4

        /** 按下点离按键左边多远（像素）：靠左按，避开右侧的 back 手势区。 */
        private const val PRESS_OFFSET_FROM_LEFT_PX = 10

        /** 按下点离按键底边多远（像素）：取最底下，确保落在底部 home 手势区内。 */
        private const val PRESS_OFFSET_FROM_BOTTOM_PX = 2

        /** 派发触摸后等界面消化的时间（毫秒）。 */
        private const val DISPATCH_SETTLE_MS = 80L

        /** 按住后等反馈出现的上限（毫秒）。 */
        private const val FEEDBACK_TIMEOUT_MS = 5_000L

        /** 轮询间隔（毫秒）。 */
        private const val POLL_INTERVAL_MS = 100L

        /**
         * 按下时按键区域里变了的像素占比下限。
         *
         * 按下反馈给整块按键铺一层底色并整体缩放，除去圆角几乎每个像素都变；
         * 留一半余量给「反馈只覆盖一部分」的画法。
         */
        private const val MIN_CHANGED_RATIO = 0.5
    }

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
        // 启动期自动弹键盘会与本用例竞态：键盘把键栏顶上去，量到的不是底边那排。
        // 收键盘可能赶在输入法弹出之前，先确认一次，失败再复核最终状态。
        if (!hideImeAndConfirm(composeTestRule.activity, IME_HIDDEN_TIMEOUT_MS)) {
            assertTrue("输入法必须收起才能量键栏底边", !isImeVisible(composeTestRule.activity))
        }
    }

    /** 经 View 派发一次触摸：坐标取组合根坐标（组合根即内容视图的原点）。 */
    private fun dispatchMotion(downTime: Long, action: Int, x: Float, y: Float) {
        val content = composeTestRule.activity.findViewById<View>(android.R.id.content)
        val eventTime = SystemClock.uptimeMillis()
        content.post {
            content.dispatchTouchEvent(
                MotionEvent.obtain(downTime, eventTime, action, x, y, 0),
            )
        }
        Thread.sleep(DISPATCH_SETTLE_MS)
    }

    /** 取一块矩形里的像素（行优先）。 */
    private fun sampleBand(bitmap: android.graphics.Bitmap, left: Int, right: Int, top: Int, bottom: Int): IntArray {
        val width = (right - left).coerceAtLeast(0)
        val height = (bottom - top).coerceAtLeast(0)
        val pixels = IntArray(width * height)
        var index = 0
        for (y in top until bottom) {
            for (x in left until right) {
                pixels[index++] = bitmap.getPixel(x, y)
            }
        }
        return pixels
    }

    @Test
    fun bottomRowKeyGivesVisiblePressFeedback() {
        // PGDN 不切换状态：按下前后按键自身的颜色只会被「按下反馈」改变。
        val node = composeTestRule.onNodeWithTag("Key_PGDN", useUnmergedTree = true)
        val bounds = node.fetchSemanticsNode().boundsInRoot
        val left = bounds.left.toInt() + SAMPLE_INSET_PX
        val right = bounds.right.toInt() - SAMPLE_INSET_PX
        val top = bounds.top.toInt() + SAMPLE_INSET_PX
        val bottom = bounds.bottom.toInt() - SAMPLE_INSET_PX
        val pressX = bounds.left.toInt() + PRESS_OFFSET_FROM_LEFT_PX
        val pressY = bounds.bottom.toInt() - PRESS_OFFSET_FROM_BOTTOM_PX

        val before = sampleBand(UxTestUtils.screenshot(device), left, right, top, bottom)
        assertTrue("按键底部必须取到样（截图或定位失败）", before.isNotEmpty())

        val downTime = SystemClock.uptimeMillis()
        dispatchMotion(downTime, MotionEvent.ACTION_DOWN, pressX.toFloat(), pressY.toFloat())
        var during: IntArray? = null
        try {
            // 反馈有启动延迟（手势区内），动画还要几帧才走完：轮询到变化为止。
            val elapsed = UxTestUtils.pollUntilTrue(
                timeoutMs = FEEDBACK_TIMEOUT_MS,
                intervalMs = POLL_INTERVAL_MS,
            ) {
                // 按下反馈是动画：不推进组合时钟，动画根本不走，量到的永远是未按下
                // 的画面（实测不 waitForIdle 时按键区域逐像素无变化）。
                composeTestRule.waitForIdle()
                val band = sampleBand(UxTestUtils.screenshot(device), left, right, top, bottom)
                if (band.contentEquals(before)) {
                    false
                } else {
                    during = band
                    true
                }
            }
            val band = checkNotNull(during) {
                "键栏底边那排按键的按下反馈必须看得见：按住 $FEEDBACK_TIMEOUT_MS ms 内该区域" +
                    "逐像素无变化（按下点 ($pressX, $pressY)，按键底边 y=${bounds.bottom.toInt()}）"
            }
            UxTestUtils.metric("modifier_press_feedback_latency_ms", elapsed ?: -1L)
            val changed = before.indices.count { before[it] != band[it] }
            UxTestUtils.metric("modifier_press_changed_ratio", changed * 100 / before.size)
            assertTrue(
                "按下时按键区域的变化必须够大（实测只有 $changed/${before.size} 个像素变了，" +
                    "要求至少 ${(MIN_CHANGED_RATIO * 100).toInt()}%）",
                changed * 100 >= MIN_CHANGED_RATIO * before.size,
            )
        } finally {
            dispatchMotion(downTime, MotionEvent.ACTION_UP, pressX.toFloat(), pressY.toFloat())
        }
    }
}
