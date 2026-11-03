package terminal.emulator.ui

import android.view.WindowInsets
import androidx.compose.ui.test.onNodeWithTag
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
import terminal.emulator.findTerminalSurface
import terminal.emulator.injectTap
import terminal.emulator.pumpedTerminalText
import terminal.emulator.requireBridge
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels
import kotlin.math.roundToInt

/**
 * 输入法弹出像素验收（TESTING.md 输入法两条）。
 *
 * 内容较少时弹出输入法终端无动画无闪烁无变化；内容较多时终端内容上移且无闪烁
 * 无卡顿无撕裂，上移前后底部像素完全相同，弹出时输入文本正确显示底部不被吞。
 * 输入法经真实点击手势弹出（与用户点击终端同路径），文本经输入连接提交。
 */
@RunWith(JUnit4::class)
class ImePopupPixelInstrumentedTest : ComposeTerminalLogcatTest() {
    companion object {
        private const val GRID_TIMEOUT_MS = 15_000L
        private const val IME_TIMEOUT_MS = 10_000L
        private const val SETTLE_MILLIS = 600L
        private const val MOVE_MIN_SHIFT_PX = 20
        private const val STRIP_MATCH_MAX_DIFF = 300

        /** 差分抽样步长：只影响统计速度。 */
        private const val PIXEL_SAMPLE_STEP_PX = 3

        /** 单像素三通道差和阈值：低于视为同一内容。 */
        private const val PIXEL_DELTA_THRESHOLD = 40

        /** 墨量统计的横向抽样步长：只用于判断「条带里有没有内容像素」。 */
        private const val INK_SAMPLE_STEP_PX = 12

        /**
         * 稀疏条带覆盖的网格行数：首行标记与提示符所在，且恒在底部键栏行程区之上。
         * 固定 400px 在小屏上会伸进键栏位移区，把键栏抬升误判成内容变化
         * （CI 稳定差分约 370 即此形状）；按实际行高只取内容行。
         */
        private const val SPARSE_STRIP_ROWS = 4

        /** 稀疏内容弹出前后条带允许的差分像素（与闪烁断言同口径）。 */
        private const val SPARSE_STRIP_MAX_DIFF = 5

        /** 稀疏内容条带顶边相对状态栏底部的余量（避开状态栏圆角阴影）。 */
        private const val SPARSE_STRIP_TOP_MARGIN_PX = 8

        /** 键栏底边相对键盘顶边的容差：亚像素取整误差。 */
        private const val BAR_SEAM_TOLERANCE_PX = 8
    }

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        composeTestRule.waitForTerminalPixels()
        composeTestRule.awaitBridge()
    }

    /** 等 shell 提示符就绪后经 shell 打印输出（writeToPty 口径），回显重发防冷 stdin 丢失。 */
    private fun printAndAwait(command: String, needle: String) {
        // 共用会话的行编辑状态跨用例残留（见 PasteButton 同因说明）：
        // 先送 ETX 中止再等 prompt，否则门控把 shell 污染误判成未就绪。
        composeTestRule.requireBridge().writeToPty("\u0003".toByteArray(Charsets.UTF_8))
        awaitShellPrompt(composeTestRule.requireBridge(), GRID_TIMEOUT_MS)
        // 冷启动 stdin 竞态：prompt 落格不等于 shell 已消费 stdin，单次写入
        // 可能丢失（粘贴案定案同类）。幂等重发至多 3 次，回显即停。
        var seen: Long? = null
        repeat(3) {
            composeTestRule.requireBridge().writeToPty("$command\n".toByteArray(Charsets.UTF_8))
            seen =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    composeTestRule.pumpedTerminalText()?.contains(needle) == true
                }
            if (seen != null) return
        }
        assertNotNull("标记必须落格: $needle", seen)
    }

    /** 真实点击终端中央弹出输入法（与用户点击同路径），等待窗口内边距稳定。 */
    private fun tapAndAwaitIme() {
        val surface = findTerminalSurface(composeTestRule.activity)
        val windowLocation = IntArray(2)
        surface.getLocationOnScreen(windowLocation)
        injectTap(
            surface,
            (windowLocation[0] + surface.width / 2).toFloat(),
            (windowLocation[1] + surface.height / 2).toFloat(),
        )
        val visible =
            UxTestUtils.pollUntilTrue(timeoutMs = IME_TIMEOUT_MS, intervalMs = 200) {
                var shown = false
                InstrumentationRegistry.getInstrumentation().runOnMainSync {
                    shown = surface.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == true
                }
                shown
            }
        assertNotNull("输入法必须弹出", visible)
        // 高度定居：Gboard 工具栏行后于按键弹出，键盘总高度动画可延续数秒。
        // 高度未稳即测，应用内边距与截图键盘错位，位移比较必抖动。
        // 连续 3 次采样一致才放行，15s 内不定居则失败（键盘真有问题，不是测试问题）。
        var stableReads = 0
        var lastHeight = -1
        val stableDeadline = android.os.SystemClock.uptimeMillis() + 15_000L
        while (android.os.SystemClock.uptimeMillis() < stableDeadline) {
            val height = imeHeightPx()
            stableReads = if (height == lastHeight && height > 0) stableReads + 1 else 1
            lastHeight = height
            if (stableReads >= 3) break
            Thread.sleep(500)
        }
        assertTrue("输入法高度必须定居 (末次=$lastHeight)", stableReads >= 3)
        Thread.sleep(SETTLE_MILLIS)
        // 键栏是组合覆盖层：它按 `imeInsetFlow` 上移，而该流只在重组时才落到
        // 画面上。不等组合空闲，截图里键栏还停在旧位置，按键栏边界算出的
        // 条带就会伸进键栏区——比对必然失配（实测键栏悬在屏幕中部）。
        composeTestRule.waitForIdle()
    }

    private fun imeHeightPx(): Int {
        var height = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val surface = findTerminalSurface(composeTestRule.activity)
            height =
                surface.rootWindowInsets
                    ?.getInsets(WindowInsets.Type.ime())
                    ?.bottom ?: 0
        }
        return height
    }

    /**
     * 比对条带的上沿（px）：终端内容起点即表面顶部，其下即无系统像素。
     *
     * 窗口可见区上沿在边到边下为 0，状态栏内边距又小于实际绘制带，
     * 两者取大仍会把状态栏图标算进来。用表面屏幕坐标直接避开整条系统栏。
     */
    private fun contentStripTopPx(): Int = surfaceTopPx() + SPARSE_STRIP_TOP_MARGIN_PX

    /** 稀疏条带高度（px）：首 [SPARSE_STRIP_ROWS] 行网格的实际像素高度。 */
    // 缩放须与运行时同口径（密度 × 系统字体缩放，见 terminalCellSizePx）。
    private fun sparseStripHeightPx(): Int {
        val metrics = composeTestRule.activity.resources
        val spToPxScale = metrics.displayMetrics.density * metrics.configuration.fontScale
        val (_, cellHeightSp) = composeTestRule.requireBridge().cellSize()
        val cellHeight = cellHeightSp * spToPxScale
        assertTrue("单元格高度不可用 ($cellHeight)", cellHeight > 0f)
        return (SPARSE_STRIP_ROWS * cellHeight).toInt()
    }

    /** 单像素 RGB 差分和（阈值由调用方判定）。 */
    private fun pixelDelta(first: Int, second: Int): Int = kotlin.math.abs(
        android.graphics.Color.red(first) -
            android.graphics.Color.red(second),
    ) +
        kotlin.math.abs(
            android.graphics.Color.green(first) -
                android.graphics.Color.green(second),
        ) +
        kotlin.math.abs(
            android.graphics.Color.blue(first) -
                android.graphics.Color.blue(second),
        )

    /** 步长采样统计两截图在纵向区间内的差异像素数。 */
    private fun countDifferingPixels(
        first: android.graphics.Bitmap,
        second: android.graphics.Bitmap,
        top: Int,
        bottom: Int,
    ): Int = UxTestUtils.countDiffInBand(
        first,
        second,
        top,
        bottom,
        PIXEL_SAMPLE_STEP_PX,
        PIXEL_DELTA_THRESHOLD,
    )

    /**
     * 终端表面的实际平移量（px，向上为正）。
     *
     * 位移取产品施加的平移，而非从像素反推：内容多时各行文本同形（`IME_MANY_*`），
     * 行墨量在行间几乎相同，按墨量搜索位移的代价曲线在任意行对齐处近乎相同，
     * 会锁在错误的局部极小值（实测位移=38，真值=225）。平移量是产品行为的权威
     * 来源，像素比对据此定位：内容没跟着平移量走即判红。
     */
    private fun surfaceShiftPx(): Int {
        var shift = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            shift = (-findTerminalSurface(composeTestRule.activity).translationY).roundToInt()
        }
        return shift
    }

    /** 终端表面的屏幕上沿（px）：平移会把它一起带走，故须在未位移时取。 */
    private fun surfaceTopPx(): Int {
        var surfaceTop = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val location = IntArray(2)
            findTerminalSurface(composeTestRule.activity).getLocationOnScreen(location)
            surfaceTop = location[1]
        }
        return surfaceTop
    }

    /**
     * 键栏覆盖层的顶边（**屏幕**坐标）。
     *
     * 键栏按输入法高度上移，与终端内容的位移量不同：条带含进键栏就不是同一批
     * 像素，比对必然失配。
     *
     * 必须取屏幕坐标：条带的上下边界都按屏幕坐标算（终端表面用
     * `getLocationOnScreen`），而 `boundsInRoot` 是组合根坐标——两者差一个状态栏
     * 高度，混用会把键栏算低 24px，条带因此伸进键栏区。
     */
    private fun modifierBarTopPx(): Int = composeTestRule
        .onNodeWithTag("ModifierBarOverlay", useUnmergedTree = true)
        .fetchSemanticsNode()
        .positionOnScreen
        .y
        .roundToInt()

    /**
     * 统计「上移后」[moved] 的 [top, top+height) 行与「上移前」[reference] 中
     * 行号差 [shift] 的同一批像素的差异数。
     *
     * 内容上移 [shift] 像素后，[moved] 第 y 行显示的是 [reference] 第 y+shift 行。
     */
    private fun shiftedDiff(
        reference: android.graphics.Bitmap,
        moved: android.graphics.Bitmap,
        top: Int,
        height: Int,
        shift: Int,
    ): Int {
        var diff = 0
        var row = 0
        while (row < height) {
            val sourceY = top + row + shift
            if (sourceY < reference.height) {
                var column = 0
                while (column < reference.width) {
                    if (pixelDelta(moved.getPixel(column, top + row), reference.getPixel(column, sourceY)) >
                        PIXEL_DELTA_THRESHOLD
                    ) {
                        diff++
                    }
                    column += PIXEL_SAMPLE_STEP_PX
                }
            }
            row += PIXEL_SAMPLE_STEP_PX
        }
        return diff
    }

    /** 条带墨量总和：> 0 表示该条带确有内容像素（非纯背景）。 */
    private fun stripInk(bitmap: android.graphics.Bitmap, top: Int, height: Int): Int {
        var sum = 0
        var row = top
        while (row < top + height) {
            var column = 0
            while (column < bitmap.width) {
                sum += android.graphics.Color.red(bitmap.getPixel(column, row))
                column += INK_SAMPLE_STEP_PX
            }
            row++
        }
        return sum
    }

    private fun isImeVisible(): Boolean {
        var visible = false
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            visible =
                findTerminalSurface(
                    composeTestRule.activity,
                ).rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == true
        }
        return visible
    }

    /** 确保输入法收起：Gboard 系统级持久，跨用例仍展开会使 before 拍到已上移态导致差分为零。 */
    private fun hideImeAndSettle() {
        // 启动期自动弹键盘会与收起竞态：只确认“某一刻隐藏”不够，before 可能在
        // 自动弹出后拍到已上移态，后续差分恒为零。先等自动弹出出现（若出现），再收起；
        // 收起后必须经过静默窗口复核，自动弹出则重试；仍不稳定就直接失败，
        // 而不是继续走像素比较。
        UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 200) {
            isImeVisible()
        }
        repeat(3) {
            composeTestRule.activity.runOnUiThread {
                val inputMethodManager =
                    composeTestRule.activity.getSystemService(
                        android.content.Context.INPUT_METHOD_SERVICE,
                    ) as android.view.inputmethod.InputMethodManager
                inputMethodManager.hideSoftInputFromWindow(
                    composeTestRule.activity.window.decorView.windowToken,
                    0,
                )
            }
            val hidden =
                UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 200) {
                    !isImeVisible()
                }
            assertNotNull("输入法必须收起", hidden)
            val stableUntil = android.os.SystemClock.uptimeMillis() + 2_000L
            var reshown = false
            while (android.os.SystemClock.uptimeMillis() < stableUntil) {
                if (isImeVisible()) {
                    reshown = true
                    break
                }
                Thread.sleep(200)
            }
            if (!reshown) {
                Thread.sleep(SETTLE_MILLIS)
                // 组合必须跟上：键栏按 `imeInsetFlow` 上移，组合没重组时它仍
                // 停在键盘高度处（悬在屏幕中部），截图与条带边界都按旧位置算。
                composeTestRule.waitForIdle()
                return
            }
        }
        throw AssertionError("输入法隐藏后仍自动弹出")
    }

    /**
     * 内容较少时弹出输入法：终端内容 MUST 每像素原位、且完整可见于键盘上方，
     * 键栏 MUST 完整位于键盘上方不被遮挡。
     *
     * 位移量按内容下沿裁剪（`computeImeSurfaceShift`）：放得下的内容不动，
     * 放不下的才上移。无条件按整块键盘高度上移会把首行提示符推出屏幕上边界
     * ——终端区实测全空（提示符由 row=134 落到 row=−686），即本用例要钉住的回归。
     *
     * 故此处度量「弹出前后同一像素条带是否完全一致」而非位移像素：稀疏会话
     * 位移为 0，两帧条带必须逐像素相同（差分 ≤ 采样噪声）；同时条带在两帧
     * 都必须有墨迹，否则「一致」只是两帧同样空白。
     */
    @Test
    fun contentFewImePopupTerminalStaysPutAndVisible() {
        val marker = "IME_FEW_${System.currentTimeMillis() % 100000}"
        printAndAwait("printf '$marker\\n'", marker)
        // 「内容较少」必须由用例自己保证：类内前一用例（内容较多）会在同一会话留下
        // 满屏 IME_MANY_*，此时本用例测到的其实是满屏内容的位移（实测条带整行都在
        // 变，位移恰为一格行高），断言前提根本不成立。清屏后在首行只留本标记。
        clearScreenAndPrint(marker)
        // 光标闪烁是逐像素比对的噪声源（块光标约一格，差分数千像素）：
        // 比对期间隐藏光标，测完恢复。try-finally 保证断言失败也不污染后继用例。
        composeTestRule.requireBridge().writeToPty("\u001b[?25l".toByteArray(Charsets.UTF_8))
        try {
            contentFewImePopupTerminalStaysPutAndVisibleBody(marker)
        } finally {
            composeTestRule.requireBridge().writeToPty("\u001b[?25h".toByteArray(Charsets.UTF_8))
        }
    }

    /**
     * 清屏（`ED2` + `CUP home`）后在首行重打标记，使「内容较少」成为可验证前提。
     *
     * 仪器化套件跨用例共用同一会话，前一用例的输出仍在屏上；不清屏时本用例量到的
     * 是满屏内容的位移，而断言却按稀疏内容写——前提与断言不一致，红灯不代表缺陷。
     */
    private fun clearScreenAndPrint(marker: String) {
        composeTestRule.requireBridge().feedTerminal("\u001b[2J\u001b[H$marker\r\n".toByteArray(Charsets.UTF_8))
        val landed =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                (composeTestRule.requireBridge().getTerminalText() ?: "").contains(marker)
            }
        assertNotNull("清屏后标记必须落格", landed)
        Thread.sleep(SETTLE_MILLIS)
    }

    private fun contentFewImePopupTerminalStaysPutAndVisibleBody(marker: String) {
        Thread.sleep(SETTLE_MILLIS)
        hideImeAndSettle()
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        // 条带取在状态栏之下（避开系统时钟像素）、键盘之上（保证内容确实可见）。
        val stripTop = contentStripTopPx()
        val stripHeight = sparseStripHeightPx()
        assertTrue(
            "比对条带必须位于键盘上方 (条带底=${stripTop + stripHeight} 键盘顶边=${device.displayHeight - imeHeightPx()})",
            stripTop + stripHeight <= device.displayHeight - imeHeightPx(),
        )
        assertTrue(
            "弹出前条带必须有内容像素（否则「无变化」断言无意义）",
            stripInk(before, stripTop, stripHeight) > 0,
        )
        tapAndAwaitIme()
        val imeHeight = imeHeightPx()
        assertTrue("输入法必须占据高度", imeHeight > 0)
        // 键栏完整位于键盘上方：底边不得低于键盘顶边（差一个取整容差内）。
        val keyboardTop = device.displayHeight - imeHeight
        val barBottom =
            composeTestRule
                .onNodeWithTag("ModifierBarOverlay", useUnmergedTree = true)
                .fetchSemanticsNode()
                .boundsInRoot.bottom
        assertTrue(
            "键栏必须完整位于输入法上方 (键栏底边=$barBottom 键盘顶边=$keyboardTop)",
            barBottom <= keyboardTop + BAR_SEAM_TOLERANCE_PX,
        )
        val after = device.takeScreenshot() ?: throw AssertionError("截图失败")
        assertTrue(
            "弹出后条带必须有内容像素（内容不得被键盘吞掉或推离屏幕）",
            stripInk(after, stripTop, stripHeight) > 0,
        )
        val stayedDiff =
            countDifferingPixels(before, after, stripTop, stripTop + stripHeight)
        assertTrue(
            "内容较少时弹出输入法终端必须无变化 (差分=$stayedDiff)",
            stayedDiff <= SPARSE_STRIP_MAX_DIFF,
        )
        // 定居后无闪烁：键盘上方整片区域连续两帧必须一致。上沿同样避开系统状态栏：
        // 状态栏带（图标与底色）随输入法状态换色，计入后是系统像素噪声而非闪烁。
        Thread.sleep(SETTLE_MILLIS)
        val settled = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val band = countDifferingPixels(after, settled, stripTop, after.height - imeHeight)
        assertTrue("内容较少时弹出输入法必须无闪烁 (差分=$band)", band <= 5)
    }

    @Test
    fun contentManyImePopupMovesUpBottomIdentical() {
        val stamp = System.currentTimeMillis() % 100000
        val last = "IME_MANY_120_$stamp"
        printAndAwait(
            "for i in \$(seq 1 120); do echo IME_MANY_\${i}" + "_$stamp; done",
            last,
        )
        Thread.sleep(SETTLE_MILLIS)
        // 启动期自动弹键盘与本用例竞态（实测 spawn 后 3s 才 show）：截图前一刻强制收起并确认，否则 before 即上移态差分为零。
        // 收起与截图之间仍可能自动重弹（慢模拟器上见过验证通过后 1s 内重弹），
        // 故 hide→shot→验前提包成整体重试：前提不过就重来，最多 3 轮。
        var before: android.graphics.Bitmap? = null
        for (attempt in 1..3) {
            hideImeAndSettle()
            before = device.takeScreenshot() ?: throw AssertionError("截图失败")
            if (!isImeVisible()) break
        }
        val beforeShot = before ?: throw AssertionError("截图失败")
        // 前提钉死：before 必须是无键盘态，否则「位移=0」是 before 已上移的假象
        // 而非产品未动（自动弹键盘与收起竞态，见 hideImeAndSettle 注释）。
        assertTrue(
            "before 截图时输入法必须已收起，否则位移比较无意义",
            !isImeVisible(),
        )
        // 前提钉死：输入法收起后终端必须回到未位移状态——残留位移既让比较失真，
        // 也在屏幕上留下键栏下方的空白带（键栏按残留高度上移）。
        assertTrue(
            "before 截图时终端必须处于未位移状态 (位移=${surfaceShiftPx()})",
            surfaceShiftPx() == 0,
        )
        // 未位移时的几何：表面顶边（= 状态栏下沿）与键栏顶边。
        val surfaceTop = surfaceTopPx()
        val barTopBefore = modifierBarTopPx()
        val beforeText = composeTestRule.pumpedTerminalText() ?: throw AssertionError("弹出前终端文本不可读")
        tapAndAwaitIme()
        val imeHeight = imeHeightPx()
        assertTrue("输入法必须占据高度", imeHeight > 0)
        // 内容较多时终端内容上移：慢模拟器上内边距动画可滞后数秒，固定等待即拍即判必抖动。
        // 轮询至平移生效并稳定（15s 上限）。
        var shift = 0
        var stableReads = 0
        var lastShift = -1
        val moveDeadline = android.os.SystemClock.uptimeMillis() + 15_000L
        while (android.os.SystemClock.uptimeMillis() < moveDeadline) {
            val current = surfaceShiftPx()
            stableReads = if (current == lastShift) stableReads + 1 else 1
            lastShift = current
            shift = current
            if (current >= MOVE_MIN_SHIFT_PX && stableReads >= 3) break
            Thread.sleep(300)
        }
        assertTrue(
            "内容较多时弹出输入法终端内容必须上移 (位移=$shift)",
            shift >= MOVE_MIN_SHIFT_PX,
        )
        // 条带取终端内容区：两端都不含键栏（键栏按输入法高度上移，与内容位移量
        // 不同，含进来就不是同一批像素），也不含状态栏（系统栏随输入法状态换色）。
        val stripTop = surfaceTop + SPARSE_STRIP_TOP_MARGIN_PX
        val stripBottom =
            minOf(modifierBarTopPx(), barTopBefore - shift) - SPARSE_STRIP_TOP_MARGIN_PX
        assertTrue(
            "比对条带高度必须足够 (顶=$stripTop 底=$stripBottom)",
            stripBottom - stripTop >= 64,
        )
        val stripHeight = stripBottom - stripTop
        // 动画定居后再取成功帧：移动中途的帧不能作为闪烁基准。
        Thread.sleep(1_000)
        val movedFrame = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val diff = shiftedDiff(beforeShot, movedFrame, stripTop, stripHeight, shift)
        assertTrue(
            "上移后条带必须与上移前同一批像素一致 (差异=$diff)",
            diff <= STRIP_MATCH_MAX_DIFF,
        )
        // 反证：错位一像素即须明显失配——否则上面的「一致」可能来自整片同色背景。
        val wrongDiff = shiftedDiff(beforeShot, movedFrame, stripTop, stripHeight, shift - 1)
        assertTrue(
            "错位像素必须明显失配（否则一致来自同色背景）(错位差异=$wrongDiff 一致差异=$diff)",
            wrongDiff > STRIP_MATCH_MAX_DIFF,
        )
        // 上移后无闪烁：稳定后连续两帧必须一致（仍在顶部条带内比较，不受键盘影响）。
        Thread.sleep(SETTLE_MILLIS)
        val settled = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val flickerDiff = countDifferingPixels(movedFrame, settled, stripTop, stripTop + stripHeight)
        assertTrue("上移稳定后必须无闪烁 (差分=$flickerDiff)", flickerDiff <= 5)
        // 上移前后底部内容完全相同：弹出只平移容器，不改变网格内容。
        // 弹出后的两帧互比恒为零，无法发现吞底；像素逐行比对在位移下恒不等，
        // 故以终端文本落实 TESTING 底部像素条款。
        val settledText = composeTestRule.pumpedTerminalText() ?: throw AssertionError("定居后终端文本不可读")
        assertTrue(
            "上移前后底部内容必须完全相同",
            settledText.replace("\n", "") == beforeText.replace("\n", ""),
        )
        // 弹出时输入文本正确显示，底部不被吞。
        // 回车后缀：输入即执行，断言执行输出而非行回显——行回显依赖从机回显开关（mksh 自管理），内边距动画期的 SIGWINCH 重绘会擦掉未提交行并造成抖动；执行输出稳定可断言，覆盖同一“输入正确显示、底部不被吞”条款。
        val typed = "IME_TYPED_$stamp"
        val typedEnter = "$typed\n"
        composeTestRule.activity.runOnUiThread {
            val editorInfo = android.view.inputmethod.EditorInfo()
            findTerminalSurface(composeTestRule.activity)
                .onCreateInputConnection(editorInfo)
                ?.commitText(typedEnter, 1)
        }
        // 回显行可能在视口边缘换行（提示符 39 列 + 输入 15 列 > 48 列宽），逐行 contains 会把换行切断的针误判为缺失：压平换行后再判。
        val typedSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                composeTestRule.pumpedTerminalText()?.replace("\n", "")?.contains(typed) == true
            }
        assertNotNull("输入文本必须落格: $typed", typedSeen)
        val packed = composeTestRule.requireBridge().getGridRowsColsPacked()
        val rows = (packed shr 32).toInt()
        val cols = (packed and 0xFFFFFFFFL).toInt()
        val flattened = composeTestRule.requireBridge().getTerminalText().orEmpty().replace("\n", "")
        assertTrue(
            "输入文本必须在可见视口内（底部不被吞）, 视口=${rows}x$cols",
            flattened.takeLast(rows * cols).contains(typed),
        )
    }

    @Test
    fun imeCommitChineseTextGridded() {
        // 中文 commitText 经 InputConnection→PTY 必须落格（拼音候选上屏口径）。
        composeTestRule.waitForSession()
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                composeTestRule.pumpedTerminalText().orEmpty().contains("$") ||
                    composeTestRule.pumpedTerminalText().orEmpty().contains("#")
            }
        assertNotNull("shell prompt 未出现", promptSeen)
        // 提交前必须确认连接受理：只有「切换余波」的有界抑制窗口会让 `commitText`
        // 据实返回 false（契约如此，输入法据此重发）。旧写法丢弃返回值，于是
        // 「窗口内被拒→静默不写→中文未落格」会被误读成产品缺陷。轮询到连接受理为止，
        // 且只成功提交一次。
        // 探测必须走 runOnMainSync：Activity.runOnUiThread 在测试线程上投递即返回，
        // 紧随其后的同步读取与主线程上的任务执行是一场毫秒级竞速——读几乎必输，
        // 结果「commitText 已受理但读到 false」被误判为产品缺陷，15 秒重试耗尽
        // （CI 同款失败实录，PROBE 时间线实证：task got=true 后 1ms 读才是 false）。
        // runOnMainSync 保证任务执行完、返回值可见后才返回，读取因此确定。
        val committed =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                var accepted = false
                InstrumentationRegistry.getInstrumentation().runOnMainSync {
                    val editorInfo = android.view.inputmethod.EditorInfo()
                    accepted =
                        findTerminalSurface(composeTestRule.activity)
                            .onCreateInputConnection(editorInfo)
                            ?.commitText("中文\n", 1) == true
                }
                accepted
            }
        assertNotNull("中文提交必须被输入连接接受（有界抑制窗口内返回 false，需重试）", committed)
        val chineseSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                composeTestRule.pumpedTerminalText()?.replace("\n", "")?.contains("中文") == true
            }
        assertNotNull("中文提交必须落格", chineseSeen)
    }
}
