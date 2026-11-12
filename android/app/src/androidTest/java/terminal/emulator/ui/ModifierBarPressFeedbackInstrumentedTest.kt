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

        /** 按下点离按键右边多远（像素）：避开左侧 back 手势带（宽 30px）。 */
        private const val PRESS_OFFSET_FROM_RIGHT_PX = 10

        /** 上滑的位移（像素）：远超 touchSlop，手势随即被判为滑动而非按压。 */
        private const val SWIPE_DISTANCE_PX = 30

        /**
         * 上滑后等待的墙钟时长（毫秒）：必须晚于反馈本该出现的时刻
         * （静止按住阈值 250ms 与长按阈值），否则量到的「无变化」只是「还没到时间」。
         */
        private const val SWIPE_SETTLE_MS = 600L

        /** 派发触摸后等界面消化的时间（毫秒）。 */
        private const val DISPATCH_SETTLE_MS = 80L

        /** 按住后等反馈出现的上限（毫秒）。 */
        private const val FEEDBACK_TIMEOUT_MS = 5_000L

        /** 轮询间隔（毫秒）。 */
        private const val POLL_INTERVAL_MS = 100L

        /**
         * 每次采样前推进的组合时钟时长（毫秒）。
         *
         * 手势区内的按下反馈要「按住不动达到阈值」才亮起，该阈值与随后的动画都
         * 挂在组合时钟上；推进步长与轮询间隔同档，几次轮询即越过阈值。
         */
        private const val CLOCK_STEP_MS = 100L

        /**
         * 按下时按键区域里变了的像素占比下限。
         *
         * 按下反馈给整块按键铺一层底色并整体缩放，除去圆角几乎每个像素都变；
         * 留一半余量给「反馈只覆盖一部分」的画法。
         */
        private const val MIN_CHANGED_RATIO = 0.5
    }

    /** 一次按下的像素采样结果。 */
    private data class FeedbackSample(val changed: Int, val total: Int)

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

        // 底边那排按键落在系统手势区内：按下反馈不是立即给的，而是「按住不动
        // 达到阈值」才亮起（系统认领上滑的时机没有上界，按下即亮会被慢速起滑
        // 穿过，见 modifier-bar-gesture-cancel 规范）。
        //
        // 该阈值与随后的缩放/底色动画都挂在组合时钟上：不推进时钟就既不会亮起
        // 也不会动（实测不推进时钟时按键区域逐像素无变化）。故关掉自动推进，
        // 改为每次采样前显式推进。
        composeTestRule.mainClock.autoAdvance = false

        val downTime = SystemClock.uptimeMillis()
        dispatchMotion(downTime, MotionEvent.ACTION_DOWN, pressX.toFloat(), pressY.toFloat())
        var during: IntArray? = null
        try {
            // 反馈有启动延迟（手势区内），动画还要几帧才走完：轮询到变化为止。
            val elapsed = UxTestUtils.pollUntilTrue(
                timeoutMs = FEEDBACK_TIMEOUT_MS,
                intervalMs = POLL_INTERVAL_MS,
            ) {
                composeTestRule.mainClock.advanceTimeBy(CLOCK_STEP_MS)
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

    /**
     * 底边那排按键上的上滑 MUST NOT 触发按下反馈（STATUS「修饰键底部列被上滑手势触发动画」）。
     *
     * 底边那排按键落在系统底部手势区里，用户从按键上起滑会被系统手势认领并向应用
     * 补发 `ACTION_CANCEL`。反馈若在认领前亮起，用户看到的就是「上滑触发了按键动画」。
     *
     * 判别力：采样发生在**晚于**反馈本该出现的时刻之后。若「越界即取消」这条逻辑
     * 被删掉，按住不动的路径会在静止按住阈值（250ms）或长按阈值到点时亮起反馈，
     * 而手指仍在按键上（未抬手），采样必然量到变化——即本用例判红。采样过早
     * （阈值之前）则量到的「无变化」毫无意义：反馈本来也还没到时间。
     *
     * 本用例走 View 派发（不经系统注入）：注入到系统的那一路会被底部 home 手势当场
     * 认领走，量到的全是桌面像素（按键本身没有毛病）。手势以「越过 touchSlop」的
     * MOVE 表达，与系统的认领路径同义。
     */
    @Test
    fun bottomRowKeySwipeUpDoesNotTriggerPressFeedback() {
        // TAB 在底排且非连发键：连发键走的是另一条分支（自动重复），断言不覆盖它。
        val node = composeTestRule.onNodeWithTag("Key_TAB", useUnmergedTree = true)
        val bounds = node.fetchSemanticsNode().boundsInRoot
        val left = bounds.left.toInt() + SAMPLE_INSET_PX
        val right = bounds.right.toInt() - SAMPLE_INSET_PX
        val top = bounds.top.toInt() + SAMPLE_INSET_PX
        val bottom = bounds.bottom.toInt() - SAMPLE_INSET_PX
        // 按点取按键右侧：避开左侧 back 手势带，只留底部 home 手势带这一条路径。
        val pressX = bounds.right.toInt() - PRESS_OFFSET_FROM_RIGHT_PX
        val pressY = bounds.bottom.toInt() - PRESS_OFFSET_FROM_BOTTOM_PX

        val before = sampleBand(UxTestUtils.screenshot(device), left, right, top, bottom)
        assertTrue("按键底部必须取到样（截图或定位失败）", before.isNotEmpty())

        // 反馈的动画挂在组合时钟上，故推进时钟需要手动驱动。
        composeTestRule.mainClock.autoAdvance = false

        val downTime = SystemClock.uptimeMillis()
        dispatchMotion(downTime, MotionEvent.ACTION_DOWN, pressX.toFloat(), pressY.toFloat())
        try {
            // 上滑：位移远超 touchSlop，手势随即被判为滑动而非按压。
            dispatchMotion(downTime, MotionEvent.ACTION_MOVE, pressX.toFloat(), (pressY - SWIPE_DISTANCE_PX).toFloat())
            // 等过反馈本该出现的时刻：手势若未被取消，此时按住路径已亮起反馈。
            Thread.sleep(SWIPE_SETTLE_MS)
            composeTestRule.mainClock.advanceTimeBy(CLOCK_STEP_MS)
            composeTestRule.waitForIdle()
            composeTestRule.mainClock.advanceTimeBy(CLOCK_STEP_MS)
            composeTestRule.waitForIdle()
            val band = sampleBand(UxTestUtils.screenshot(device), left, right, top, bottom)
            val changed = before.indices.count { before[it] != band[it] }
            UxTestUtils.metric("modifier_swipe_changed_ratio", changed * 100 / before.size)
            assertTrue(
                "上滑不得触发按键的按下反馈（实测 $changed/${before.size} 个像素变了）",
                changed == 0,
            )
        } finally {
            dispatchMotion(downTime, MotionEvent.ACTION_UP, pressX.toFloat(), pressY.toFloat())
        }
    }

    /**
     * 按下反馈 MUST 只在手势区**外**立即启动。
     *
     * 系统认领手势的时机取决于手指移动快慢，没有上界：手势区内按下即亮会让慢速
     * 起滑看到「上滑触发了按键动画」；而手势区外系统根本不会认领那里的触摸，
     * 延后只是白白迟滞（见 modifier-bar-gesture-cancel 规范）。
     *
     * 两条断言放在**同一个用例**里：两个按键在同样的时钟推进与同样的采样时机下
     * 被按住。差异因此只能归因于「是否落在系统手势区内」，而不是两次采样的时序
     * 差——分开成两个用例时，采样耗时的波动足以让对比失去意义。
     *
     * 判别力：把「区内也立即亮」的判据写反（即两处都立即给反馈），带内那条即
     * 判红；把「区外也延后」的判据写反（即两处都等静止按住阈值），带外那条即
     * 判红。两条同时成立时，本用例才通过。
     *
     * 组合时钟须推进两次：一次只画到动画的起点帧（值仍是旧值），第二次才走完
     * `tween`。而带内的静止按住走**墙钟**期限（`PRESS_FEEDBACK_HOLD_MS`），
     * 故采样必须发生在该期限之内——采样越界时本用例量的已不是「按下瞬间」。
     */
    @Test
    fun pressFeedbackStartsImmediatelyOnlyOutsideSystemGestureArea() {
        composeTestRule.mainClock.autoAdvance = false

        // HOME 在第一排中部：四条手势带（左右各 30px、底部 32px、顶部 0）都不含它。
        val outside = pressFeedbackSample("Key_HOME", expectFeedback = true)
        // PGDN 在第二排最右侧，其下沿（y=614）落在底部 home 手势带（y≥608）内。
        val inside = pressFeedbackSample("Key_PGDN", expectFeedback = false)

        assertTrue(
            "手势区外的按键必须在按下后立即给出反馈（实测 ${outside.changed}/${outside.total} 个像素变了）",
            outside.changed > 0,
        )
        assertTrue(
            "手势区内的按键在按下瞬间不得给出反馈（实测 ${inside.changed}/${inside.total} 个像素变了）",
            inside.changed == 0,
        )
    }

    /**
     * 按住一个按键并采样该按键区域的像素变化。
     *
     * @param expectFeedback 期望本次按下是否给出反馈；与实测不符即判红。
     * @return 变化像素数与采样总数（供断言与指标使用）。
     */
    private fun pressFeedbackSample(tag: String, expectFeedback: Boolean): FeedbackSample {
        val node = composeTestRule.onNodeWithTag(tag, useUnmergedTree = true)
        val bounds = node.fetchSemanticsNode().boundsInRoot
        val left = bounds.left.toInt() + SAMPLE_INSET_PX
        val right = bounds.right.toInt() - SAMPLE_INSET_PX
        val top = bounds.top.toInt() + SAMPLE_INSET_PX
        val bottom = bounds.bottom.toInt() - SAMPLE_INSET_PX
        val pressX = bounds.left.toInt() + PRESS_OFFSET_FROM_LEFT_PX
        val pressY = bounds.bottom.toInt() - PRESS_OFFSET_FROM_BOTTOM_PX

        val before = sampleBand(UxTestUtils.screenshot(device), left, right, top, bottom)
        assertTrue("$tag 必须取到样（截图或定位失败）", before.isNotEmpty())

        val downTime = SystemClock.uptimeMillis()
        dispatchMotion(downTime, MotionEvent.ACTION_DOWN, pressX.toFloat(), pressY.toFloat())
        try {
            composeTestRule.mainClock.advanceTimeBy(CLOCK_STEP_MS)
            composeTestRule.waitForIdle()
            composeTestRule.mainClock.advanceTimeBy(CLOCK_STEP_MS)
            composeTestRule.waitForIdle()
            val band = sampleBand(UxTestUtils.screenshot(device), left, right, top, bottom)
            val changed = before.indices.count { before[it] != band[it] }
            // 采样必须发生在带内静止按住的期限之内，否则量的已不是「按下瞬间」。
            val elapsedWallMs = SystemClock.uptimeMillis() - downTime
            assertTrue(
                "$tag 的采样须在静止按住阈值（$PRESS_FEEDBACK_HOLD_MS ms）内完成，" +
                    "实测耗时 $elapsedWallMs ms（设备太慢，采样已越过阈值）",
                elapsedWallMs < PRESS_FEEDBACK_HOLD_MS,
            )
            UxTestUtils.metric("modifier_press_feedback_${tag}_wall_ms", elapsedWallMs)
            UxTestUtils.metric("modifier_press_feedback_${tag}_changed_ratio", changed * 100 / before.size)
            assertTrue(
                "$tag 的按下反馈与预期不符：期望有反馈=$expectFeedback，" +
                    "实测 $changed/${before.size} 个像素变了",
                (changed > 0) == expectFeedback,
            )
            return FeedbackSample(changed = changed, total = before.size)
        } finally {
            dispatchMotion(downTime, MotionEvent.ACTION_UP, pressX.toFloat(), pressY.toFloat())
        }
    }
}
