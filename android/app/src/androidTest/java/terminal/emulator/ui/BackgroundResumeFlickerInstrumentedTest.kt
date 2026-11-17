package terminal.emulator.ui

import android.view.WindowInsets
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
import terminal.emulator.isImeVisible
import terminal.emulator.requireBridge
import terminal.emulator.showCursor
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.roundToInt

/**
 * 切后台返回的闪烁验收：真实 HOME 往返期间开孔不露底，回来后画面与离开时一致。
 *
 * 判据分三层：
 *
 * 1. 切后台时平台必须拆掉 surface，且运行期必须报「无缓冲区」——窗口层据此铺终端
 *    底色（`TerminalSurface.onDraw`），开孔才不会透出桌面/上一个应用；
 * 2. 回前台必须重新出帧，且首帧是原生保留的最后一帧（`warmup` 的保留分支）而不是
 *    清屏——清屏那一拍开孔里没有内容；
 * 3. 终判：往返前后同一批像素一致，且条带里仍有内容像素（防止「一致」实为两帧空白）。
 *
 * 不用截图做瞬态判定：闪烁只有一两帧，模拟器上一次截图要 100ms 以上，轮询线程在
 * 后台又会被进程冻结吞掉。故瞬态看运行期状态（事件驱动的 StateFlow），终判看像素。
 */
@RunWith(JUnit4::class)
class BackgroundResumeFlickerInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val GRID_TIMEOUT_MS = 15_000L
        private const val BACKGROUND_TIMEOUT_MS = 20_000L
        private const val RESUME_TIMEOUT_MS = 30_000L
        private const val SETTLE_MILLIS = 800L

        /** 条带顶边相对表面顶部的余量（避开状态栏圆角阴影，与输入法用例同口径）。 */
        private const val STRIP_TOP_MARGIN_PX = 8

        /** 像素抽样步长：只影响统计速度。 */
        private const val PIXEL_SAMPLE_STEP_PX = 3

        /** 单像素三通道差和阈值：低于视为同一内容。 */
        private const val PIXEL_DELTA_THRESHOLD = 40

        /** 往返前后条带允许的差异像素（采样噪声 + 光标残影）。 */
        private const val MAX_STRIP_DIFF = 300

        /** 条带里至少要有多少种颜色：平底色/桌面都能凑出高「一致」，只有内容才有色。 */
        private const val MIN_DISTINCT_COLORS = 8

        /**
         * 等输入法信号的上限。GBoard 首次构建键盘可能真的很慢（冷启动实测约 3 秒）。
         */
        private const val IME_SHOWN_TIMEOUT_MS = 20_000L

        /** 颜色统计的抽样步长。 */
        private const val COLOR_SAMPLE_STEP_PX = 6

        /** 原生 warmup 两种分支的日志锚点（`native::render::pass::frame`）。 */
        private const val WARMUP_RETAINED = "warmup: presenting the retained last frame"
        private const val WARMUP_CLEARED = "warmup: no retained frame to present"

        /** `logcat -v threadtime` 的时间戳格式（设备本地时区，定长前缀）。 */
        private val THREAD_TIME_FORMATTER: DateTimeFormatter =
            DateTimeFormatter.ofPattern("MM-dd HH:mm:ss.SSS", Locale.US)
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
     * 共用同一会话：光标一直藏着，后继用例就拿不到帧内光标，退格与组字两条用例
     * 的「起点」判据整段失效（实测报「渲染帧未追上当前光标」「帧内光标一直取不到」）。
     * 故离开前必须还原——与 [paintColoredContent] 的藏光标成对；还原夹具与判据见
     * [showCursor]。
     */
    @After
    fun restoreCursorVisibility() {
        composeTestRule.requireBridge().showCursor()
    }

    /** 开孔在屏幕上的纵向条带（像素坐标）。 */
    private data class Band(val top: Int, val bottom: Int)

    /** 清屏并铺满彩色多行文本：开孔比对需要可区分的颜色，且不得依赖 shell 回显。 */
    private fun paintColoredContent() {
        val payload =
            buildString {
                append("\u001b[2J\u001b[H")
                append("\u001b[?25l")
                for (line in 1..24) {
                    val color = 31 + (line % 6)
                    append("\u001b[${color}mFLICKER_${line.toString().padStart(2, '0')}_")
                    append("ABCDEFGHIJKLMNOPQRSTUVWXYZ_0123456789\u001b[0m\r\n")
                }
            }
        composeTestRule.requireBridge().feedTerminal(payload.toByteArray(Charsets.UTF_8))
        assertNotNull(
            "彩色内容必须落格",
            UxTestUtils.pollUntilTrue(GRID_TIMEOUT_MS, 100) {
                (composeTestRule.requireBridge().getTerminalText() ?: "").contains("FLICKER_01_")
            },
        )
        composeTestRule.waitForIdle()
        Thread.sleep(SETTLE_MILLIS)
    }

    /** 当前开孔的屏幕纵向条带。 */
    private fun holeBand(): Band {
        var top = 0
        var bottom = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val surface = findTerminalSurface(composeTestRule.activity)
            val location = IntArray(2)
            surface.getLocationOnScreen(location)
            top = location[1] + STRIP_TOP_MARGIN_PX
            bottom = location[1] + surface.height
        }
        return Band(top, bottom)
    }

    /** 条带里的颜色种数（抽样）：用于区分「有内容」与「一块平底色/桌面」。 */
    private fun distinctColors(bitmap: android.graphics.Bitmap, band: Band): Int {
        val colors = HashSet<Int>()
        var row = band.top.coerceAtLeast(0)
        val bottom = band.bottom.coerceAtMost(bitmap.height)
        while (row < bottom) {
            var column = 0
            while (column < bitmap.width) {
                colors.add(bitmap.getPixel(column, row))
                column += COLOR_SAMPLE_STEP_PX
            }
            row += COLOR_SAMPLE_STEP_PX
        }
        return colors.size
    }

    /** 终端表面因输入法位移的像素数（未位移时为 0）。 */
    private fun surfaceShiftPx(): Int {
        var shift = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            shift = (-findTerminalSurface(composeTestRule.activity).translationY).roundToInt()
        }
        return shift
    }

    /**
     * 收起输入法并等终端回到未位移状态。
     *
     * 启动会按用户设置自动弹键盘；位移中的表面顶边是负坐标（被推出屏幕），
     * 按它算条带会越界。基准帧必须取「未位移」形态，往返后也按同一形态比对。
     */
    private fun hideImeAndSettle() {
        assertTrue("输入法必须收起", hideImeAndConfirm(composeTestRule.activity, 5_000))
        assertNotNull(
            "输入法收起后终端必须回到未位移状态",
            UxTestUtils.pollUntilTrue(5_000, 100) { surfaceShiftPx() == 0 },
        )
        composeTestRule.waitForIdle()
        Thread.sleep(SETTLE_MILLIS)
    }

    /** `logcat -v threadtime` 的时间戳前缀（与设备日志同一时钟）。 */
    private fun wallClockStamp(): String = THREAD_TIME_FORMATTER.format(LocalDateTime.now())

    /**
     * 自 [since] 起是否有输入法画上（`ImeTracker` 的 `onShown` 锚点）。
     *
     * 平台在 GBoard 真正画出来之前就把 ime 的 `visible` insets 派发给应用（实测在
     * 请求后一拍就可见、面板约 3 秒后才画），所以这个锚点是这个设备上唯一可靠的
     * 「键盘真的在屏幕上」信号。`ImeTracker` 是平台的跟踪器，与输入法品牌无关。
     */
    private fun imeShownSince(since: String): Boolean = UxTestUtils
        .runShellCommand("logcat -d -v threadtime -t 5000")
        .lineSequence()
        .any { it.contains("ImeTracker") && it.contains("onShown") && it.take(18) >= since }

    /** 输入法覆盖屏幕底部的像素（insets 的 ime 高度）。 */
    private fun imeInsetBottomPx(): Int {
        var bottom = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            bottom =
                composeTestRule.activity.window.decorView
                    .rootWindowInsets
                    ?.getInsets(WindowInsets.Type.ime())
                    ?.bottom ?: 0
        }
        return bottom
    }

    /**
     * 内容比对的条带：排除被输入法覆盖的行。
     *
     * 键盘上画什么（布局、皮肤、文案）不属于应用可渲染的终端画面，且可以随往返
     * 变化（实测同一次往返前后 GBoard 换了皮肤）。这些行不计入内容一致性。
     */
    private fun contentBand(band: Band, shotHeightPx: Int, imeInsetBottomPx: Int): Band =
        Band(band.top.coerceAtLeast(0), minOf(band.bottom, shotHeightPx - imeInsetBottomPx))

    /** 取时间戳不早于 [since] 的原生 warmup 日志行。 */
    private fun warmupLinesSince(since: String): List<String> = UxTestUtils
        .runShellCommand("logcat -d -v threadtime -t 5000")
        .lineSequence()
        // tag 超过平台的 23 字符上限会被截断，故只取不会被截掉的前缀。
        .filter { it.contains("native::render::pass") }
        // threadtime 的时间戳是定长前缀（`MM-dd HH:mm:ss.SSS`），字典序即时间序。
        .filter { it.take(18) >= since }
        .toList()

    /**
     * 弹出输入法并等到键盘真的画上屏幕（`STATUS.md` 报的场景就是键盘在场）。
     *
     * 本用例比对的内容区排除输入法覆盖行（见 [contentBand]），键盘画没画上屏不影响
     * 比对口径；insets 可见足够表达「键盘按用户要求在场」。
     */
    private fun showImeAndSettle() {
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val decor = composeTestRule.activity.window.decorView
            decor.windowInsetsController?.show(WindowInsets.Type.ime())
        }
        assertNotNull(
            "输入法必须可见（insets）",
            UxTestUtils.pollUntilTrue(IME_SHOWN_TIMEOUT_MS, 100) { isImeVisible(composeTestRule.activity) },
        )
        composeTestRule.waitForIdle()
        Thread.sleep(SETTLE_MILLIS)
    }

    /**
     * 真实 HOME → `am start` 往返，并断言开孔在这条路径上的两个事实。
     *
     * @param expectRetainedWarmup 回前台首帧是否必须来自原生保留帧。注入设备丢失时
     *   累加器随旧设备一起没了，首帧只能是清屏，故该用例传 false。
     * @return 离开的时刻戳（供调用方附加日志断言）
     */
    private fun roundTrip(expectRetainedWarmup: Boolean): String {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        val leaveStamp = wallClockStamp()

        // 真实切后台：HOME 键。后台应用的 startActivity 会被平台拦截，回前台只能
        // 交回 shell（`am start`），与用户从桌面图标启动同一条路径。
        device.pressHome()
        assertNotNull(
            "必须真的退到后台",
            UxTestUtils.pollUntilTrue(BACKGROUND_TIMEOUT_MS, 50) {
                device.currentPackageName != packageName
            },
        )
        assertNotNull(
            "切后台后平台必须拆掉 surface（否则本用例前提不成立）",
            UxTestUtils.pollUntilTrue(BACKGROUND_TIMEOUT_MS, 20) {
                !runtime.surfaceBufferPresent.value
            },
        )

        UxTestUtils.runShellCommand("am start -n $packageName/terminal.emulator.MainActivity")
        assertNotNull(
            "必须回到前台",
            UxTestUtils.pollUntilTrue(RESUME_TIMEOUT_MS, 50) {
                device.currentPackageName == packageName
            },
        )
        assertNotNull(
            "回前台必须重新出帧（否则终端停在底色上）",
            UxTestUtils.pollUntilTrue(RESUME_TIMEOUT_MS, 20) {
                runtime.surfaceBufferPresent.value
            },
        )
        Thread.sleep(SETTLE_MILLIS)

        // 回前台首帧的来源：保留帧让开孔从离开时的画面接上，清屏那一拍没有内容。
        val warmup = warmupLinesSince(leaveStamp)
        if (expectRetainedWarmup) {
            assertTrue(
                "回前台首帧 MUST 呈现保留的最后一帧（原生 warmup 无对应日志）",
                warmup.any { it.contains(WARMUP_RETAINED) },
            )
            assertTrue(
                "回前台首帧 MUST NOT 清屏（出现 `${WARMUP_CLEARED}`）",
                warmup.none { it.contains(WARMUP_CLEARED) },
            )
        }
        return leaveStamp
    }

    /**
     * 往返前后同一批像素的终判：内容一致且不是空白。
     */
    private fun assertContentUnchanged(before: android.graphics.Bitmap, band: Band) {
        val after = UxTestUtils.screenshot(device)
        assertTrue(
            "回前台后条带必须有内容（不得是桌面或空白）",
            distinctColors(after, band) >= MIN_DISTINCT_COLORS,
        )
        val content = contentBand(band, after.height, imeInsetBottomPx())
        val diff =
            UxTestUtils.countDiffInBand(
                before,
                after,
                content.top,
                content.bottom,
                PIXEL_SAMPLE_STEP_PX,
                PIXEL_DELTA_THRESHOLD,
            )
        assertTrue(
            "回前台后内容条带必须与离开时一致（实测差 $diff 像素）",
            diff <= MAX_STRIP_DIFF,
        )
    }

    @Test
    fun backgroundResumeKeepsTheTerminalContent() {
        paintColoredContent()
        hideImeAndSettle()
        val band = holeBand()
        val before = UxTestUtils.screenshot(device)
        assertTrue(
            "离开前条带必须有内容像素（否则「无闪烁」断言无意义）",
            distinctColors(before, band) >= MIN_DISTINCT_COLORS,
        )

        roundTrip(expectRetainedWarmup = true)

        assertContentUnchanged(before, band)
    }

    /**
     * `docs/STATUS.md` 报的场景：**输入法在场**时切后台返回。
     *
     * 键盘在场时开孔高度被子窗口/位移改过，回前台首帧必须画回离开时的位置：
     * 位移若在后台被归零，同一批内容会整体错位——正是「输入法弹出时切后台返回
     * 终端闪烁」。故本用例不收起键盘，另加「往返前后终端位移相同」。
     */
    @Test
    fun backgroundResumeWithImeVisibleKeepsTheTerminalContent() {
        paintColoredContent()
        showImeAndSettle()
        val band = holeBand()
        val before = UxTestUtils.screenshot(device)
        val tripStartStamp = wallClockStamp()
        val shiftBefore = surfaceShiftPx()
        assertTrue(
            "键盘在场时离开前条带必须有内容像素",
            distinctColors(before, band) >= MIN_DISTINCT_COLORS,
        )

        roundTrip(expectRetainedWarmup = true)

        val shiftAfter = surfaceShiftPx()
        assertNotNull(
            "回前台后输入法必须画上（ImeTracker onShown）",
            UxTestUtils.pollUntilTrue(IME_SHOWN_TIMEOUT_MS, 200) { imeShownSince(tripStartStamp) },
        )
        assertTrue(
            "往返前后终端位移必须相同（否则像素比对不是同一批可见行：$shiftBefore → $shiftAfter）",
            shiftBefore == shiftAfter,
        )
        assertContentUnchanged(before, band)
    }

    /**
     * 设备丢失路径：渲染线程已死、帧累加器随旧设备一起没了。
     *
     * `docs/STATUS.md` 报的「输入法弹出时切后台返回大概率闪烁」在真机日志里的触发链
     * 是：切后台时 Vulkan 设备丢失 → 重建后没有保留帧可呈现。本用例注入设备丢失，
     * 要求回前台后照常重建、出帧，且画面与离开时一致——不允许停在底色上。
     */
    @Test
    fun deviceLostInBackgroundStillRestoresTheTerminalContent() {
        paintColoredContent()
        hideImeAndSettle()
        val band = holeBand()
        val before = UxTestUtils.screenshot(device)

        val bridge = composeTestRule.requireBridge()
        bridge.setDeviceLostInjectedForTest(true)
        try {
            roundTrip(expectRetainedWarmup = false)
        } finally {
            bridge.setDeviceLostInjectedForTest(false)
        }

        assertContentUnchanged(before, band)
    }
}
