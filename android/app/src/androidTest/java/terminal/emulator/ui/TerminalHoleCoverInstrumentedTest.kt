package terminal.emulator.ui

import android.graphics.Bitmap
import androidx.lifecycle.Lifecycle
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.awaitShellPrompt
import terminal.emulator.bridge.Bridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.pumpedTerminalText
import terminal.emulator.requireBridge
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels

/**
 * 切后台返回时，终端开孔里必须是**离开时的画面**，而不是一块平底色。
 *
 * 开孔（`SurfaceView` 挖在窗口上的透明洞）在原生 surface 尚无缓冲时透出后方桌面，
 * 用户看到的就是「切后台返回闪一下」。遮罩由 `SurfaceView` 自己在 `onDraw` 里补画：
 * 平台先在窗口上打洞、再走 `onDraw`，故 `onDraw` 画的内容落在洞**之后**，能被看见；
 * 组合层画的则相反——必然被洞抹掉（实测窗口层的底色只在 SurfaceView 之外的边距可见）。
 *
 * 遮罩的像素由原生侧在渲染线程停下前回读累加器得到（`TerminalRuntime.captureCoverFrame`）。
 * 本用例覆盖「画出来了」：**逐像素**比对开孔里的内容与遮罩帧——只要求「颜色够多」
 * 是不够的，一块带花纹的平底色也能通过那种判据。
 */
@RunWith(JUnit4::class)
class TerminalHoleCoverInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        /** 采样步长（x 与 y 同用）。 */
        private const val SAMPLE_STEP_PX = 3

        /**
         * 开孔里的像素与遮罩帧一致的比例下限。
         *
         * 屏幕合成与色彩管理会让颜色偏几级，故留了容差；比例本身要求很高：
         * 一块平底色只能与遮罩的底色那部分吻合，文字像素全不吻合。
         */
        private const val MIN_COVER_MATCH_RATIO = 0.9

        /**
         * 单通道的色差容差（0-255）。
         *
         * 合成管线对**开孔区域**做了一次色彩变换（实测：缓冲里的 `#212121` 在屏幕上
         * 显示为 `#3C3C3C`，即整片区域按同一系数提亮——变换可写作
         * `screen = 0.878 × buffer + 31`）。遮罩帧是原始缓冲像素，与屏幕像素天然
         * 差这一截：最暗的底色差 27 级，最亮的文字色只差 9 级。故容差取 34：
         * 覆盖该变换的最大偏差，又不至于大到放过另一种画面。
         */
        private const val COLOR_TOLERANCE = 34

        /**
         * 开孔里至少要有的颜色种数。
         *
         * 用来挡住「平底色」这一种蒙混过关的画法：终端画面绝大部分是底色，平底色
         * 与遮罩的底色那一部分照样吻合，吻合比例可以很高；只有「颜色种数」这条能
         * 把它区分开——遮罩帧里有文字，文字各有各的色。
         */
        private const val MIN_DISTINCT_COLORS = 8
    }

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
    }

    /** 开孔的一次取样结果。 */
    private data class HoleSample(
        /** 与遮罩帧逐像素吻合的比例。 */
        val matchRatio: Double,
        /** 采样点总数。 */
        val sampled: Int,
        /** 开孔区域里出现最多的颜色。 */
        val dominant: Int,
        /** 出现最多的颜色占比。 */
        val dominantShare: Double,
        /** 开孔区域里出现过的颜色种数。 */
        val distinct: Int,
    )

    /**
     * 取一次开孔的样，并**同时**与 [cover] 逐像素比对。
     *
     * 只截一次图：截两次会跨过一次转场动画，两张图量的是不同的画面（实测第二张
     * 截到的是桌面而不是终端，比对结果全不相干）。
     */
    private fun sampleHole(cover: Bridge.CoverFrame): HoleSample? = runCatchingCancellable {
        // 必须确认应用在前台：截屏拿到的是**合成后的画面**，应用不在前台时截到的是
        // 后方应用/桌面（实测截到的是桌面灰 #3C3C3C），量出来的颜色与遮罩毫无关系。
        val packageName = InstrumentationRegistry.getInstrumentation().targetContext.packageName
        check(device.currentPackageName == packageName) {
            "应用不在前台（当前 ${device.currentPackageName}），截屏量到的不是终端画面"
        }
        val bitmap: Bitmap = UxTestUtils.screenshot(device)
        dumpForensics(bitmap, cover)
        val surface = findTerminalSurface(composeTestRule.activity)
        val location = IntArray(2)
        surface.getLocationOnScreen(location)
        val left = location[0]
        val top = location[1]
        var matched = 0
        var sampled = 0
        val counts = HashMap<Int, Int>()
        var y = 0
        while (y < cover.height / 2) {
            var x = 0
            while (x < cover.width) {
                val screenX = left + x
                val screenY = top + y
                if (screenX < bitmap.width && screenY < bitmap.height) {
                    val screen = bitmap.getPixel(screenX, screenY)
                    counts[screen] = (counts[screen] ?: 0) + 1
                    val offset = (y * cover.width + x) * 4
                    val deltaR = kotlin.math.abs((screen shr 16 and 0xFF) - (cover.pixels[offset].toInt() and 0xFF))
                    val deltaG = kotlin.math.abs((screen shr 8 and 0xFF) - (cover.pixels[offset + 1].toInt() and 0xFF))
                    val deltaB = kotlin.math.abs((screen and 0xFF) - (cover.pixels[offset + 2].toInt() and 0xFF))
                    if (deltaR <= COLOR_TOLERANCE &&
                        deltaG <= COLOR_TOLERANCE &&
                        deltaB <= COLOR_TOLERANCE
                    ) {
                        matched += 1
                    }
                    sampled += 1
                }
                x += SAMPLE_STEP_PX
            }
            y += SAMPLE_STEP_PX
        }
        if (sampled == 0) return@runCatchingCancellable null
        val dominant = counts.entries.sortedByDescending { it.value }.first()
        HoleSample(
            matchRatio = matched.toDouble() / sampled,
            sampled = sampled,
            dominant = dominant.key,
            dominantShare = dominant.value.toDouble() / sampled,
            distinct = counts.size,
        )
    }.getOrNull()

    /** 把截屏与遮罩各自存成 PNG，便于离线比对。 */
    private fun dumpForensics(bitmap: Bitmap, cover: Bridge.CoverFrame) {
        runCatchingCancellable {
            val dir = java.io.File(composeTestRule.activity.getExternalFilesDir(null), "hole")
            dir.mkdirs()
            java.io.FileOutputStream(java.io.File(dir, "screen.png")).use {
                bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
            }
            val coverBitmap = android.graphics.Bitmap.createBitmap(
                cover.width,
                cover.height,
                android.graphics.Bitmap.Config.ARGB_8888,
            )
            coverBitmap.copyPixelsFromBuffer(java.nio.ByteBuffer.wrap(cover.pixels))
            java.io.FileOutputStream(java.io.File(dir, "cover.png")).use {
                coverBitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
            }
            android.util.Log.i("HOLE_DUMP", "dumped to ${dir.absolutePath}")
        }
    }

    @Test
    fun theHoleShowsTheLastFrameAfterResume() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        composeTestRule.waitForIdle()
        awaitShellPrompt(composeTestRule.requireBridge())
        composeTestRule.requireBridge().writeToPty("echo HOLE_COVER_MARK\n".toByteArray(Charsets.UTF_8))
        composeTestRule.pumpedTerminalText()

        // 诊断：原生 surface 的**缓冲本身**（PixelCopy 读回，绕过显示管线）是什么颜色，
        // 与窗口层截屏同一位置的颜色相比，差值即显示管线的色彩变换。
        val surface = findTerminalSurface(composeTestRule.activity)
        val raw = UxTestUtils.captureSurface(surface)
        if (raw != null) {
            val screen = UxTestUtils.screenshot(device)
            val location = IntArray(2)
            surface.getLocationOnScreen(location)
            android.util.Log.i(
                "HOLE_DUMP",
                "raw(0,0)=#%08X raw(40,40)=#%08X screen(0,0)=#%08X screen(40,40)=#%08X at %d,%d".format(
                    raw.getPixel(0, 0),
                    raw.getPixel(40, 40),
                    screen.getPixel(location[0], location[1]),
                    screen.getPixel(location[0] + 40, location[1] + 40),
                    location[0],
                    location[1],
                ),
            )
        } else {
            android.util.Log.i("HOLE_DUMP", "PixelCopy failed")
        }

        // 持续注入 surface 级失败：取纹理失败，宿主会换新的 SurfaceView，开孔稳定地
        // 没有新帧——「无缓冲区」成为稳定态，才量得到遮罩在开孔里的样子。
        assertTrue(
            "失效注入必须被原生接受",
            composeTestRule.requireBridge().setSurfaceLossInjectedForTest(true),
        )
        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        assertNotNull(
            "切后台后必须回读出开孔遮罩",
            UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                runtime.coverFrame.value != null
            },
        )
        composeTestRule.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        composeTestRule.waitForIdle()

        // 遮罩挂在开孔上的窗口：无缓冲区、且遮罩帧还在。
        assertNotNull(
            "切回前台后遮罩必须挂在开孔上（无缓冲区且遮罩帧在）",
            UxTestUtils.pollUntilTrue(timeoutMs = 15_000, intervalMs = 100) {
                composeTestRule.waitForIdle()
                !runtime.surfaceBufferPresent.value && runtime.coverFrame.value != null
            },
        )
        val cover = checkNotNull(runtime.coverFrame.value) { "切回前台后遮罩帧必须还在" }
        // 开孔里画出来的那一帧必须**就是**遮罩帧。
        val sample = checkNotNull(sampleHole(cover)) { "开孔区必须取到样（截图或定位失败）" }
        UxTestUtils.metric("hole_cover_match_ratio", (sample.matchRatio * 100).toLong())
        UxTestUtils.metric("hole_distinct_colors", sample.distinct.toLong())
        UxTestUtils.metric("hole_dominant_share", (sample.dominantShare * 100).toLong())
        // 开孔里画出来的那一帧必须**就是**遮罩帧。
        assertTrue(
            "开孔里画出来的那一帧必须就是遮罩帧，实测只有 ${sample.matchRatio * 100}% 的采样点" +
                "与遮罩一致（要求 ${MIN_COVER_MATCH_RATIO * 100}%；开孔 dominant=${sample.dominant} " +
                "占比 ${sample.dominantShare * 100}% 颜色种数 ${sample.distinct} 采样点 ${sample.sampled}）",
            sample.matchRatio >= MIN_COVER_MATCH_RATIO,
        )
        // 颜色种数：单靠「与遮罩吻合的比例」放不住平底色——终端画面绝大部分是底色，
        // 平底色与遮罩的底色那一部分照样吻合，比例可能很高。故另钉一条：开孔里必须
        // 有多种颜色（遮罩帧里有文字，文字各有各的色）。
        assertTrue(
            "开孔里必须画的是画面而不是一块平底色（实测只有 ${sample.distinct} 种颜色）",
            sample.distinct >= MIN_DISTINCT_COLORS,
        )

        // 收尾：撤掉注入让帧恢复，遮罩随之撤下（否则后面的用例会带着遮罩跑）。
        // 注入期间原生 surface 已判死，宿主要换一个新的 SurfaceView 才拿得到新窗口，
        // 而换视图由组合重组驱动——只轮询不推进组合时钟，信号一直排在队列里。
        assertTrue(
            "关闭失效注入必须被原生接受",
            composeTestRule.requireBridge().setSurfaceLossInjectedForTest(false),
        )
        assertNotNull(
            "帧恢复后遮罩必须撤下",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 100) {
                composeTestRule.requireBridge().writeToPty("echo HOLE_COVER_END\n".toByteArray(Charsets.UTF_8))
                composeTestRule.pumpedTerminalText()
                composeTestRule.waitForIdle()
                runtime.surfaceBufferPresent.value && runtime.coverFrame.value == null
            },
        )
    }
}
