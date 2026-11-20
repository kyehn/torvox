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
import terminal.emulator.awaitTerminalReady
import terminal.emulator.findTerminalSurface
import terminal.emulator.hideImeAndConfirm
import terminal.emulator.requireBridge
import terminal.emulator.runtime.holeStateCycle
import terminal.emulator.showCursor
import java.util.Collections

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
 * 判据分两段，各量各的：
 *
 * - **时延与先后**取运行期自己的开孔日志（`TerminalRuntime.holeStateJournal`）：
 *   切后台期间平台会拆掉窗口缓冲，外部采样整段丢失（实测空档 0.8~2.7s），
 *   量到的「时延」是采样空档而不是真实时延；日志是应用侧事件流，一拍不丢。
 *   要求：遮罩发布后 ≤500ms 内有一次「已画进窗口」的记录，且该记录早于平台拆
 *   surface 的记录（拆掉之后窗口不再有帧，画了也上不了屏）。
 * - **像素**取窗口缓冲：前台稳态必须是**透明**的（原生 surface 在下层供像素，
 *   遮罩若盖住活内容就是另一个 bug）；遮罩**画进窗口之后**，原生无缓冲期间必须
 *   始终**不透明**（那正是用户看到的一闪）——发布到画进窗口之间的那一小段空档
 *   由上面的时延判据单独兜住；遮罩交回原生图层后必须重新透明。
 *
 * 实测（模拟器，2026-11-17）：遮罩发布后同一帧内即被画进窗口，窗口缓冲里的开孔
 * 不透明占比随即跃到 1.000；原生无缓冲且遮罩在位的样本里没有一条是空开孔；
 * 交回原生图层后不透明占比回到 0.000。反向对照（去掉「遮罩发布即重绘」与
 * 「暂停期间照画遮罩」）时过渡段的开孔一直透明，直到回前台才被填上——那就是
 * 用户看到的「切后台时窗口里的洞透出桌面」。故上限取 500ms（实测个位数毫秒的
 * 数十倍余量）。
 */
@RunWith(JUnit4::class)
class TerminalHoleTransitionFillInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        /** 认定「开孔已被填满」的不透明占比下限。 */
        private const val OPAQUE_THRESHOLD = 0.95

        /** 认定「开孔是空的（透出后方）」的不透明占比上限。 */
        private const val TRANSPARENT_THRESHOLD = 0.5

        /** 遮罩发布后必须被画进窗口的时限（毫秒）。 */
        private const val MAX_FILL_LATENCY_MS = 500L

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
        composeTestRule.awaitTerminalReady()
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
        /** 采样时刻（墙钟，毫秒）：与 logcat 的时间戳同一口径，便于对齐。 */
        val wallMs: Long,
        /** 开孔区域里不透明像素的占比。 */
        val opaqueRatio: Double,
        /** 原生 surface 是否已有缓冲。 */
        val bufferPresent: Boolean,
        /** 开孔遮罩是否在位。 */
        val coverPresent: Boolean,
    )

    /**
     * 采样序列。
     *
     * 写发生在主线程（`PixelCopy` 回调），读发生在测试线程：普通 `MutableList` 在
     * 两边同时用会抛 `ConcurrentModificationException`，或读到写了一半的列表。
     * 故用同步列表，读时整体加锁取快照。
     */
    private val samples = Collections.synchronizedList(mutableListOf<Sample>())

    /** 采样开关：主线程读、测试线程写，必须可见，否则主线程会一直续采。 */
    @Volatile private var sampling = false

    /** 采样快照：与主线程的写入互斥。 */
    private fun sampleSnapshot(): List<Sample> = synchronized(samples) { samples.toList() }

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
                wallMs = android.os.SystemClock.elapsedRealtime(),
                opaqueRatio = opaque.toDouble() / total,
                bufferPresent = runtime.surfaceBufferPresent.value,
                coverPresent = runtime.coverFrame.value != null,
            ),
        )
    }

    /**
     * 采样序列的可读转储：失败时直接打进断言消息，省去「重跑一次猜时间」的一轮。
     *
     * 每行给出「相对遮罩发布的毫秒偏移 / 不透明占比 / 缓冲位 / 遮罩位」；
     * 偏移为负表示该样本早于遮罩发布。
     */
    private fun dumpSamples(anchorMs: Long): String = samples.joinToString(separator = "\n") { sample ->
        val offset = sample.stampMs - anchorMs
        val sign = if (offset >= 0) "+" else ""
        val ratio = "%.3f".format(sample.opaqueRatio)
        "  $sign$offset ms ratio=$ratio buffer=${sample.bufferPresent} " +
            "cover=${sample.coverPresent} wall=${sample.wallMs}"
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
        val foreground = sampleSnapshot()
        assertTrue("必须采到前台样本（实测 ${foreground.size} 条）", foreground.size >= 8)
        val filledInForeground = foreground.count { it.opaqueRatio >= OPAQUE_THRESHOLD }
        assertTrue(
            "前台稳态的开孔 MUST 保持透明（遮罩不得盖住活内容）：实测 $filledInForeground/${foreground.size} 条样本不透明占比 ≥ $OPAQUE_THRESHOLD",
            filledInForeground == 0,
        )

        // 真实切后台：HOME 键。回前台只能交回 shell（`am start`），与用户从桌面启动同一条路径。
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        val journalBefore = runtime.holeStateJournal()
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
        // 时延与先后取**运行期自己的开孔日志**：切后台期间平台会拆掉窗口缓冲，
        // 外部采样整段丢失（实测空档 0.8~2.7s），量到的「时延」是采样空档而不是
        // 真实时延；日志是应用侧事件流，一拍不丢。
        val cycle = holeStateCycle(journalBefore, runtime.holeStateJournal())
        val publish = requireNotNull(cycle.firstOrNull { it.coverPresent }) {
            "本次往返必须发布开孔遮罩（实测开孔日志：$cycle）"
        }
        val painted = requireNotNull(
            cycle.firstOrNull { it.painted && it.coverPresent && it.sequence > publish.sequence },
        ) {
            "遮罩发布后 MUST 有一次「已画进窗口」的记录（实测开孔日志：$cycle）"
        }
        val latency = painted.stampMs - publish.stampMs
        assertTrue(
            "遮罩发布后 MUST 在 $MAX_FILL_LATENCY_MS 毫秒内画进窗口（实测 $latency 毫秒）\n" +
                "开孔日志：$cycle",
            latency <= MAX_FILL_LATENCY_MS,
        )
        // 画进窗口 MUST 发生在平台拆 surface 之前：拆掉之后窗口不再有帧，画了也上不了屏。
        val teardown = cycle.firstOrNull { !it.bufferPresent && it.sequence > publish.sequence }
        if (teardown != null) {
            assertTrue(
                "遮罩 MUST 在平台拆 surface 之前画进窗口（画于第 ${painted.sequence} 条，" +
                    "拆 surface 于第 ${teardown.sequence} 条）\n开孔日志：$cycle",
                painted.sequence < teardown.sequence,
            )
        }

        // 像素判据：遮罩在位期间，开孔里 MUST 真的有东西（不透明）。
        //
        // 判据取「发布之后（含发布本身）第一条达标的样本」：发布那一拍往往就已经
        // 填满（发布与视图重绘在同一帧内完成），要求「发布之后再出现一条」会把
        // 正常行为判红。
        val snapshot = sampleSnapshot()
        val publishIndex = snapshot.indexOfFirst { it.coverPresent }
        assertTrue("必须采到遮罩发布的时刻（实测 ${snapshot.size} 条样本）", publishIndex >= 0)
        val publishSample = snapshot[publishIndex]
        val fillIndex =
            snapshot.withIndex().indexOfFirst { (index, sample) ->
                index >= publishIndex && sample.opaqueRatio >= OPAQUE_THRESHOLD
            }
        assertTrue(
            "遮罩在位期间开孔 MUST 被填满（实测 ${snapshot.size} 条样本，无一条达到不透明占比 $OPAQUE_THRESHOLD）\n" +
                dumpSamples(publishSample.stampMs),
            fillIndex >= 0,
        )

        // 原生无缓冲时开孔 MUST NOT 露出后方：那正是用户看到的一闪。
        //
        // 原生**有**缓冲时开孔是透明的：那不是闪——原生图层在下层供像素，洞里透出
        // 的是终端内容，不是桌面。故只判「无缓冲且遮罩在位」这一种组合。
        //
        // 只判「首次填满之后」的样本：发布与真正画进窗口之间隔着一次重绘（十几毫秒），
        // 这一小段里状态已是「遮罩在位」而窗口缓冲还是旧的空开孔——那是**过渡时延**，
        // 由上面的时限判据单独兜住，不该在这里当成「闪」。
        val uncovered = snapshot.drop(fillIndex).filter {
            !it.bufferPresent && it.coverPresent && it.opaqueRatio <= TRANSPARENT_THRESHOLD
        }
        assertTrue(
            "原生无缓冲且遮罩在位时开孔 MUST 被填满（实测 ${uncovered.size} 条样本是空开孔）\n" +
                dumpSamples(publishSample.stampMs),
            uncovered.isEmpty(),
        )

        // 交回原生图层后开孔 MUST 重新透明：遮罩是补救，不是常驻底色。
        val handedBack = snapshot.any {
            !it.coverPresent && it.bufferPresent && it.opaqueRatio <= TRANSPARENT_THRESHOLD
        }
        assertTrue(
            "遮罩交回原生图层后开孔 MUST 重新透明（实测未观察到）\n" +
                dumpSamples(publishSample.stampMs),
            handedBack,
        )
    }
}
