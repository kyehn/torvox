package terminal.emulator.ui

import android.graphics.Bitmap
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.hideImeAndConfirm
import terminal.emulator.requireBridge
import terminal.emulator.showCursor
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 切后台过渡期间，**窗口缓冲里的开孔**必须已经被遮罩填满——直接量像素。
 *
 * 与既有两个用例的分工：`BackgroundResumeFlickerInstrumentedTest` 量产品自己的开孔
 * 状态转换序列（不丢事件，但那是状态不是像素）；`TerminalHoleCoverInstrumentedTest` 量
 * 「遮罩画出来的那一帧就是离开时的画面」（注入 surface 失效后的稳态）。本用例量的是
 * **过渡期那一拍**：平台拆掉 surface 之前，窗口自己的缓冲里那个洞到底有没有被填上。
 *
 * 为什么必须量窗口缓冲而不是截图：`SurfaceView` 的原生 surface 位于窗口**之下**，用户
 * 看到的是「窗口缓冲 + 下层 surface」的合成结果。开孔漏了内容时，截图里看到的是后方
 * 桌面（一两帧的闪），但窗口缓冲本身仍然是「洞里什么都没有」——只有直接读窗口缓冲才
 * 分得清「洞是空的」与「洞被填了但填的是别的东西」。
 *
 * 判据（实测值来自模拟器，2026-11-15）：
 *
 * - 前台稳态：开孔在窗口缓冲里是**透明**的（原生 surface 在下层供像素），不透明占比
 *   实测 0.000。遮罩若在这时画上去就是盖住活内容，必须为 0。
 * - 切后台：遮罩发布后实测 **79~98ms** 内开孔被填满（不透明占比跃到 1.000），其后到
 *   平台拆 surface 之前整段过渡都保持填满（实测未填满样本占比 1.2%~3.7%）。
 * - 反向对照（去掉「遮罩发布即重绘」与「暂停期间照画遮罩」两处）：遮罩发布后整个过渡
 *   段（实测 72~88 帧、约 380ms）开孔**一直透明**，直到回前台才被填上——那就是用户
 *   看到的「切后台时窗口里的洞透出桌面」。
 *
 * 故上限取 500ms（实测最差 98ms 的五倍余量）与 20% 未填满占比（实测最差 3.7% 的五倍
 * 余量）；反向对照的实测值是 2000ms 级与 82%~100%，两侧余量都够。
 */
@RunWith(JUnit4::class)
class TerminalHoleTransitionFillInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        /** 认定「开孔已被填满」的不透明占比下限。 */
        private const val OPAQUE_THRESHOLD = 0.95

        /** 认定「开孔是空的（透出后方）」的不透明占比上限。 */
        private const val TRANSPARENT_THRESHOLD = 0.5

        /** 遮罩发布后开孔必须被填满的时限（毫秒）。 */
        private const val MAX_FILL_LATENCY_MS = 500L

        /** 过渡段里允许的未填满样本占比上限。 */
        private const val MAX_UNFILLED_RATIO = 0.2

        /** 后台停留时长：必须长于填满时限，反向对照的「迟到填满」才无从混入。 */
        private const val BACKGROUND_HOLD_MS = 2_000L

        /** 前台稳态的采样时长。 */
        private const val FOREGROUND_SAMPLE_MS = 800L

        /** 回前台后的采样时长。 */
        private const val RESUME_SAMPLE_MS = 1_500L

        /** 采样步长（像素）：只影响统计速度。 */
        private const val SAMPLE_STEP_PX = 4

        /** 采样区域相对开孔四边的内缩（像素）：避开边界抗锯齿。 */
        private const val REGION_INSET_PX = 8

        /** 采样区域取开孔高度的多少（从顶部算）：避开输入法在场时被顶上去的部分。 */
        private const val REGION_HEIGHT_RATIO = 0.34f

        /** 采样失败（窗口没有 backing surface）后的重试间隔（毫秒）。 */
        private const val RETRY_INTERVAL_MS = 20L
    }

    private lateinit var device: UiDevice
    private lateinit var packageName: String

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        packageName = InstrumentationRegistry.getInstrumentation().targetContext.packageName
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
    }

    /**
     * 还原光标可见性。
     *
     * 像素比对期间必须藏起光标（块光标闪烁是逐像素噪声源），但仪器化套件跨用例
     * 共用同一会话：光标一直藏着，后继用例就拿不到光标位置（隐藏光标时
     * `cursorViewportPacked` 返回 -1），落格判据整段失效（实测报「标记必须落格于
     * (2,30)」）。故离开前必须还原——与 [paintColoredContent] 的藏光标成对；还原夹具
     * 与判据见 [showCursor]。
     */
    @After
    fun restoreCursorVisibility() {
        composeTestRule.requireBridge().showCursor()
    }

    /** 窗口缓冲的一次采样。 */
    private class Sample(
        /** 采样时刻（进程内单调时钟，毫秒）。 */
        val stampMs: Long,
        /** 开孔区域里不透明像素的占比。 */
        val opaqueRatio: Double,
        /** 原生 surface 是否已有缓冲。 */
        val bufferPresent: Boolean,
        /** 开孔遮罩是否在位。 */
        val coverPresent: Boolean,
    )

    private val samples = mutableListOf<Sample>()
    private var sampling = false

    /**
     * 连续采样窗口缓冲。
     *
     * 采样走 `PixelCopy.request(window, ...)`：读的是**窗口自己的缓冲**，不含 `SurfaceView`
     * 的独立图层，故洞里有没有东西一目了然。回调落在主线程——读取视图位置与运行期状态都
     * 得在主线程上做。
     */
    private fun startSampling() {
        val activity = composeTestRule.activity
        val window = activity.window
        val handler = Handler(Looper.getMainLooper())
        val bitmap =
            Bitmap.createBitmap(
                window.decorView.width.coerceAtLeast(1),
                window.decorView.height.coerceAtLeast(1),
                Bitmap.Config.ARGB_8888,
            )
        sampling = true
        fun request() {
            if (!sampling) return
            try {
                PixelCopy.request(
                    window,
                    bitmap,
                    { result ->
                        if (result == PixelCopy.SUCCESS) record(bitmap)
                        if (sampling) handler.post { request() }
                    },
                    handler,
                )
            } catch (_: Throwable) {
                // 窗口没有 backing surface（平台在后台拆掉了它）：这一拍采不到，等下一拍。
                if (sampling) handler.postDelayed({ request() }, RETRY_INTERVAL_MS)
            }
        }
        handler.post { request() }
    }

    /** 记一次采样：开孔区域的不透明占比 + 当时的运行期状态。 */
    private fun record(bitmap: Bitmap) {
        val activity = composeTestRule.activity
        val runtime = activity.terminalViewModel.runtime
        val surface = findTerminalSurface(activity)
        val location = IntArray(2)
        surface.getLocationInWindow(location)
        val left = location[0] + REGION_INSET_PX
        val top = location[1] + REGION_INSET_PX
        val right = (location[0] + surface.width - REGION_INSET_PX).coerceAtMost(bitmap.width)
        val bottom =
            (location[1] + (surface.height * REGION_HEIGHT_RATIO).toInt()).coerceAtMost(bitmap.height)
        if (right <= left || bottom <= top) return
        var opaque = 0
        var total = 0
        var y = top
        while (y < bottom) {
            var x = left
            while (x < right) {
                if (Color.alpha(bitmap.getPixel(x, y)) >= 200) opaque++
                total++
                x += SAMPLE_STEP_PX
            }
            y += SAMPLE_STEP_PX
        }
        if (total == 0) return
        samples.add(
            Sample(
                stampMs = android.os.SystemClock.uptimeMillis(),
                opaqueRatio = opaque.toDouble() / total,
                bufferPresent = runtime.surfaceBufferPresent.value,
                coverPresent = runtime.coverFrame.value != null,
            ),
        )
    }

    /** 铺一批彩色内容：开孔里必须有东西可遮，空洞与实心才分得开。 */
    private fun paintColoredContent() {
        val payload =
            buildString {
                append("\u001b[2J\u001b[H")
                append("\u001b[?25l")
                for (line in 1..24) {
                    val color = 31 + (line % 6)
                    append("\u001b[${color}mHOLE_${line.toString().padStart(2, '0')}_")
                    append("ABCDEFGHIJKLMNOPQRSTUVWXYZ_0123456789\u001b[0m\r\n")
                }
            }
        composeTestRule.requireBridge().feedTerminal(payload.toByteArray(Charsets.UTF_8))
        assertNotNull(
            "彩色内容必须落格",
            UxTestUtils.pollUntilTrue(15_000, 100) {
                (composeTestRule.requireBridge().getTerminalText() ?: "").contains("HOLE_01_")
            },
        )
        composeTestRule.waitForIdle()
    }

    @Test
    fun holeIsFilledWhileTheSurfaceIsBeingTornDown() {
        paintColoredContent()
        assertTrue("输入法必须收起", hideImeAndConfirm(composeTestRule.activity, 5_000))
        Thread.sleep(500)

        startSampling()
        Thread.sleep(FOREGROUND_SAMPLE_MS)

        // 前台稳态：原生 surface 在下层供像素，窗口缓冲里的洞必须是透明的。
        val foreground = samples.toList()
        assertTrue("必须采到前台样本（实测 ${foreground.size} 条）", foreground.size >= 8)
        val filledInForeground = foreground.count { it.opaqueRatio >= OPAQUE_THRESHOLD }
        assertTrue(
            "前台稳态的开孔 MUST 保持透明（遮罩不得盖住活内容）：实测 $filledInForeground/${foreground.size} 条样本不透明占比 ≥ $OPAQUE_THRESHOLD",
            filledInForeground == 0,
        )

        // 真实切后台：HOME 键。回前台只能交回 shell（`am start`），与用户从桌面启动同一条路径。
        device.pressHome()
        assertNotNull(
            "必须真的退到后台",
            UxTestUtils.pollUntilTrue(20_000, 50) { device.currentPackageName != packageName },
        )
        Thread.sleep(BACKGROUND_HOLD_MS)
        UxTestUtils.runShellCommand("am start -n $packageName/terminal.emulator.MainActivity")
        assertNotNull(
            "必须回到前台",
            UxTestUtils.pollUntilTrue(30_000, 50) { device.currentPackageName == packageName },
        )
        Thread.sleep(RESUME_SAMPLE_MS)
        sampling = false
        Thread.sleep(200)

        // —— 判据 ——
        val publishIndex = samples.indexOfFirst { it.coverPresent }
        assertTrue("必须采到遮罩发布的时刻（实测 ${samples.size} 条样本）", publishIndex >= 0)
        val publish = samples[publishIndex]
        val fillIndex =
            samples.withIndex().indexOfFirst { (index, sample) ->
                index > publishIndex && sample.opaqueRatio >= OPAQUE_THRESHOLD
            }
        assertTrue(
            "遮罩发布后开孔 MUST 被填满（实测 ${samples.size} 条样本，无一条达到不透明占比 $OPAQUE_THRESHOLD）",
            fillIndex > publishIndex,
        )
        val latency = samples[fillIndex].stampMs - publish.stampMs
        assertTrue(
            "遮罩发布后开孔 MUST 在 $MAX_FILL_LATENCY_MS 毫秒内被填满（实测 $latency 毫秒）",
            latency <= MAX_FILL_LATENCY_MS,
        )

        // 填满 MUST 发生在平台拆 surface 之前：拆掉之后窗口不再有帧，画了也上不了屏。
        val teardownIndex =
            samples.withIndex().indexOfFirst { (index, sample) ->
                index > publishIndex && !sample.bufferPresent
            }
        if (teardownIndex > publishIndex) {
            assertTrue(
                "开孔 MUST 在平台拆 surface 之前就被填满（填满于第 $fillIndex 条，" +
                    "拆 surface 于第 $teardownIndex 条）",
                fillIndex < teardownIndex,
            )
        }

        // 过渡段（遮罩在位）不得长时间留着空开孔。
        val clearIndex =
            samples.withIndex().indexOfFirst { (index, sample) ->
                index > publishIndex && !sample.coverPresent
            }
        val segmentEnd = if (clearIndex > publishIndex) clearIndex else samples.size
        val segment = samples.subList(publishIndex, segmentEnd)
        val unfilled = segment.count { it.opaqueRatio <= TRANSPARENT_THRESHOLD }
        val ratio = unfilled.toDouble() / segment.size
        assertTrue(
            "过渡段里的空开孔占比 MUST NOT 超过 $MAX_UNFILLED_RATIO（实测 $ratio，" +
                "共 ${segment.size} 条样本、其中未填满 $unfilled 条）",
            ratio <= MAX_UNFILLED_RATIO,
        )

        // 原生无缓冲时开孔 MUST NOT 露出后方：那正是用户看到的一闪。
        val uncovered = samples.filter {
            !it.bufferPresent && it.coverPresent && it.opaqueRatio <= TRANSPARENT_THRESHOLD
        }
        assertTrue(
            "原生无缓冲且遮罩在位时开孔 MUST 被填满（实测 ${uncovered.size} 条样本是空开孔）",
            uncovered.isEmpty(),
        )
    }
}
