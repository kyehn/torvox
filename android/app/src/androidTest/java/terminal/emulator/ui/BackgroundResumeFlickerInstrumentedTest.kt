package terminal.emulator.ui

import android.view.WindowInsets
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
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
import terminal.emulator.runtime.TerminalRuntime.HoleStateEntry
import terminal.emulator.runtime.holeStateCycle
import terminal.emulator.showCursor
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.roundToInt

/**
 * 切后台返回的闪烁验收：**真实** HOME 往返期间，开孔任何时刻都不得既无缓冲区又无遮罩。
 *
 * 与 `TerminalHoleCoverInstrumentedTest` 的分工：后者注入 surface 失效并验证「遮罩
 * 画出来的是离开时的画面」；本用例不注入，走用户真实的 HOME → 重新启动路径。
 *
 * 为什么不用截图/轮询采样做瞬态判定：闪烁只有一两帧，模拟器上一次
 * `UiDevice.takeScreenshot()` 要 100ms 以上；轮询线程在后台会被进程冻结吞掉，回
 * 前台的关键路径又会压过采样线程（实测采样序列从「CREATED/无缓冲/有遮罩」一步跳到
 * 「RESUMED/有缓冲/无遮罩」，中间的瞬态一条都没采到）。故判据取产品自己的状态机：
 * `TerminalRuntime` 在缓冲区与遮罩每次变化时记一条开孔状态，事件不丢、无采样竞态。
 * 断言：
 *
 * 1. 切后台必须出现「无缓冲 + 有遮罩」——遮罩真的在开孔上；
 * 2. 整段往返 MUST NOT 出现「无缓冲 + 无遮罩」——那一拍开孔透出桌面，就是闪烁；
 * 3. 回前台首帧 MUST 是保留的最后一帧（原生 `warmup` 的保留分支），不是清屏；
 * 4. 终判：往返前后同一批像素一致，且条带里仍有内容像素（防止「一致」实为两帧空白）。
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
         * `after` 一侧的断言走 [imeShownSince] 的 `onShown` 锚点（强信号：键盘真的
         * 画上才算）；`showImeAndSettle` 走 insets 可见门槛（弱信号但足够——内容区
         * 排除输入法行，键盘画没画不影响比对口径）。
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
     * 变化（实测同一次往返前后 GBoard 换了皮肤）。这些行不计入内容一致性；
     * 开孔行为本身由状态序列与遮罩断言覆盖。
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
     * 真实 HOME → `am start` 往返，并用开孔状态机自己的转换序列断言全程不露底。
     *
     * @param journalBefore 离开前的状态快照；差额即本次往返。
     * @return 本次往返新增的转换记录，供调用方附加断言。
     */
    private fun roundTripAndAssertNeverUncovered(journalBefore: List<HoleStateEntry>): List<HoleStateEntry> {
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
        assertNotNull(
            "切后台必须回读出开孔遮罩",
            UxTestUtils.pollUntilTrue(BACKGROUND_TIMEOUT_MS, 20) {
                runtime.coverFrame.value != null
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
            "回前台首帧上屏后遮罩必须撤下",
            UxTestUtils.pollUntilTrue(RESUME_TIMEOUT_MS, 20) {
                runtime.surfaceBufferPresent.value && runtime.coverFrame.value == null
            },
        )
        Thread.sleep(SETTLE_MILLIS)

        // 1-2. 状态机自己的转换序列：本次往返新增的条目即整段后台往返。
        // 差分按序号取：日志是环形缓冲，前后都满时按条数差分恒为空。
        val cycle = holeStateCycle(journalBefore, runtime.holeStateJournal())
        android.util.Log.i("FlickerTest", "开孔状态转换序列（buffer,cover）：$cycle")
        assertTrue("本次往返必须产生开孔状态转换记录（实测 ${cycle.size} 条）", cycle.isNotEmpty())
        assertTrue(
            "切后台必须进入「无缓冲 + 有遮罩」：遮罩真的遮在开孔上（实测 $cycle）",
            cycle.any { !it.bufferPresent && it.coverPresent },
        )
        assertTrue(
            "回前台终态必须是「有缓冲 + 无遮罩」（实测 ${cycle.lastOrNull()}）",
            cycle.last().bufferPresent && !cycle.last().coverPresent,
        )
        // 核心判据一：遮罩必须**真的画进窗口**，而且要在平台拆 surface 之前画完。
        // 状态机里有遮罩不等于屏幕上有遮罩：遮罩发布发生在渲染线程，窗口不会自己
        // 重绘；漏掉这次重绘，窗口留着的就是离开时那个空开孔。
        assertTrue(
            "切后台时遮罩 MUST 在 surface 仍在时画进窗口（实测 $cycle）",
            cycle.any { it.painted && it.bufferPresent && it.coverPresent },
        )
        // 核心判据二：回前台这一侧，遮罩必须在**当前窗口**真的画过才撤。
        // 原生侧提交帧可能早于窗口重新可见，旧窗口的缓冲（连同遮罩）已被平台丢弃；
        // 若见「有缓冲」就撤遮罩，首个可见窗口帧就是空开孔——这正是闪烁。
        val resumeCover = cycle.indexOfLast { it.bufferPresent && it.coverPresent }
        val resumeCleared = cycle.indexOfLast { it.bufferPresent && !it.coverPresent }
        assertTrue(
            "回前台必须留下终态「有缓冲 + 无遮罩」（实测 $cycle）",
            resumeCover >= 0 && resumeCleared > resumeCover,
        )
        assertTrue(
            "回前台的遮罩 MUST 在当前窗口画过再撤（实测 $cycle）",
            cycle.subList(resumeCover, resumeCleared).any { it.painted },
        )
        // 核心判据三：无覆盖的那段时间里既不得缺缓冲、也不得缺遮罩与底色——缺席的
        // 那一拍开孔透出桌面。
        val uncovered = cycle.filter { !it.bufferPresent && !it.coverPresent && !it.painted }
        assertTrue(
            "开孔 MUST NOT 在无缓冲区时同时无遮罩（实测 ${uncovered.size} 条：$cycle）",
            uncovered.isEmpty(),
        )

        // 3. 回前台首帧必须是保留的最后一帧：清屏分支会让开孔空一拍。
        val warmup = warmupLinesSince(leaveStamp)
        assertTrue(
            "回前台首帧 MUST 呈现保留的最后一帧（原生 warmup 无对应日志）",
            warmup.any { it.contains(WARMUP_RETAINED) },
        )
        assertTrue(
            "回前台首帧 MUST NOT 清屏（出现 `${WARMUP_CLEARED}`）",
            warmup.none { it.contains(WARMUP_CLEARED) },
        )
        return cycle
    }

    @Test
    fun backgroundResumeNeverExposesUncoveredHole() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        paintColoredContent()
        hideImeAndSettle()
        val band = holeBand()
        val before = UxTestUtils.screenshot(device)
        assertTrue(
            "离开前条带必须有内容像素（否则「无闪烁」断言无意义）",
            distinctColors(before, band) >= MIN_DISTINCT_COLORS,
        )

        roundTripAndAssertNeverUncovered(runtime.holeStateJournal())

        // 4. 终判：往返前后同一批像素一致，且条带里仍有内容。
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

    /**
     * `docs/STATUS.md` 报的场景：**输入法在场**时切后台返回。
     *
     * 键盘在场时开孔高度被子窗口/位移改过，遮罩与开孔尺寸不一致——那条路上整块放弃
     * 遮罩就退回平底色，正是「大概率闪烁」。故本用例不收起键盘：状态序列判据与上
     * 一用例相同，另加「往返前后终端位移相同」以保证像素比对的是同一批可见行。
     */
    @Test
    fun backgroundResumeWithImeVisibleNeverExposesUncoveredHole() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        paintColoredContent()
        showImeAndSettle()
        val band = holeBand()
        val before = UxTestUtils.screenshot(device)
        // 输入法的重现事件必须晚于这个时间点；往返前的键盘画在此前，不是往返后的证据。
        val tripStartStamp = wallClockStamp()
        val shiftBefore = surfaceShiftPx()
        assertTrue(
            "键盘在场时离开前条带必须有内容像素",
            distinctColors(before, band) >= MIN_DISTINCT_COLORS,
        )

        roundTripAndAssertNeverUncovered(runtime.holeStateJournal())

        val shiftAfter = surfaceShiftPx()
        assertNotNull(
            "回前台后输入法必须画上（ImeTracker onShown）",
            UxTestUtils.pollUntilTrue(IME_SHOWN_TIMEOUT_MS, 200) { imeShownSince(tripStartStamp) },
        )
        assertTrue(
            "往返前后终端位移必须相同（否则像素比对不是同一批可见行：$shiftBefore → $shiftAfter）",
            shiftBefore == shiftAfter,
        )
        val after = UxTestUtils.screenshot(device)
        assertTrue(
            "键盘在场时回前台后条带必须有内容",
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
            "键盘在场时回前台后内容条带必须与离开时一致（输入法区域已排除；实测差 $diff 像素）",
            diff <= MAX_STRIP_DIFF,
        )
    }

    /**
     * 设备丢失路径：渲染线程已死、无法再回读时，切后台仍必须把**已缓存**的画面
     * 发布成开孔遮罩。
     *
     * `docs/STATUS.md` 报的「输入法弹出时切后台返回大概率闪烁」在 2026-11-17 真机
     * 日志里的触发链是：切后台时 Vulkan 设备丢失 → 渲染线程退出路径上的遮罩回读
     * 必然失败（`Parent device is lost`）→ 无遮罩 → 回前台首个窗口帧是空开孔。
     * 回读必须在设备还活着时完成，切后台只发布缓存。
     *
     * 注入设备丢失后，旧实现（渲染线程退出时才回读）在这里拿不到任何遮罩帧；
     * 新实现回读循环早已缓存了上一帧。判据不依赖截图：直接量状态机里的遮罩像素。
     */
    @Test
    fun deviceLostInBackgroundStillPublishesStoredCoverFrame() {
        val runtime = composeTestRule.activity.terminalViewModel.runtime
        paintColoredContent()
        hideImeAndSettle()
        composeTestRule.waitForIdle()
        assertNull("前置：前台不得留着上一轮的遮罩", runtime.coverFrame.value)
        assertNotNull(
            "出帧后必须已缓存一帧遮罩（回读循环未运行）",
            UxTestUtils.pollUntilTrue(10_000, 100) { runtime.hasStoredCoverFrame() },
        )

        val bridge = composeTestRule.requireBridge()
        bridge.setDeviceLostInjectedForTest(true)
        try {
            device.pressHome()
            assertNotNull(
                "必须真的退到后台",
                UxTestUtils.pollUntilTrue(BACKGROUND_TIMEOUT_MS, 50) {
                    device.currentPackageName != packageName
                },
            )
            assertNotNull(
                "切后台后平台必须拆掉 surface",
                UxTestUtils.pollUntilTrue(BACKGROUND_TIMEOUT_MS, 20) {
                    !runtime.surfaceBufferPresent.value
                },
            )
            assertNotNull(
                "设备丢失时切后台必须发布缓存的遮罩（实测没有）",
                UxTestUtils.pollUntilTrue(BACKGROUND_TIMEOUT_MS, 20) {
                    runtime.coverFrame.value != null
                },
            )
            val cover = checkNotNull(runtime.coverFrame.value)
            assertTrue(
                "遮罩必须是画面而非单色（实测 ${cover.pixels.toHashSet().size} 种像素值）",
                cover.pixels.toHashSet().size > 4,
            )
        } finally {
            bridge.setDeviceLostInjectedForTest(false)
        }

        UxTestUtils.runShellCommand("am start -n $packageName/terminal.emulator.MainActivity")
        assertNotNull(
            "必须回到前台",
            UxTestUtils.pollUntilTrue(RESUME_TIMEOUT_MS, 50) {
                device.currentPackageName == packageName
            },
        )
        assertNotNull(
            "设备丢失后回前台首帧必须重新出帧并撤下遮罩",
            UxTestUtils.pollUntilTrue(RESUME_TIMEOUT_MS, 20) {
                runtime.surfaceBufferPresent.value && runtime.coverFrame.value == null
            },
        )
    }
}
