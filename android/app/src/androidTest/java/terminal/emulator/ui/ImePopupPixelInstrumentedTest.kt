package terminal.emulator.ui

import android.view.WindowInsets
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.findTerminalSurface
import terminal.emulator.getBridge
import terminal.emulator.injectTap
import terminal.emulator.util.runCatchingCancellable
import terminal.emulator.waitForSession

/**
 * 输入法弹出像素验收（TESTING.md 输入法两条）。
 *
 * 内容较少时弹出输入法终端无动画无闪烁无变化；内容较多时终端内容上移且无闪烁
 * 无卡顿无撕裂，上移前后底部像素完全相同，弹出时输入文本正确显示底部不被吞。
 * 输入法经真实点击手势弹出（与用户点击终端同路径），文本经输入连接提交。
 */
@RunWith(JUnit4::class)
class ImePopupPixelInstrumentedTest {
    companion object {
        private const val GRID_TIMEOUT_MS = 15_000L
        private const val IME_TIMEOUT_MS = 10_000L
        private const val SETTLE_MILLIS = 600L
        private const val MOVE_MIN_SHIFT_PX = 20
        private const val STRIP_MATCH_MAX_DIFF = 300

        /** 像素采样步长（x 与 y 同用）：只影响差异统计速度。 */
        private const val PIXEL_SAMPLE_STEP_PX = 3

        /** 行墨量的横向抽样步长：放宽到 12px 仍能精确命中（见 bestUpwardShift）。 */
        private const val ROW_INK_SAMPLE_STEP_PX = 12

        /** 顶部条带高度：键盘永远够不到的高处，只随终端平移而动。 */
        private const val STRIP_HEIGHT_PX = 150

        /**
         * 稀疏内容的比对条带高度（状态栏之下、键盘之上）。
         * 覆盖首行提示符与打印标记（行高 45px 实测），既含内容又远离键栏位移区。
         */
        private const val SPARSE_STRIP_HEIGHT_PX = 400

        /** 稀疏内容弹出前后条带允许的差分像素（与闪烁断言同口径）。 */
        private const val SPARSE_STRIP_MAX_DIFF = 5

        /** 稀疏内容条带顶边相对状态栏底部的余量（避开状态栏圆角阴影）。 */
        private const val SPARSE_STRIP_TOP_MARGIN_PX = 8

        /** 位移出现的轮询上限，与 contentMany 同一口径。 */
        private const val SETTLE_MOVE_TIMEOUT_MS = 15_000L

        /** 位移轮询间隔。 */
        private const val SETTLE_POLL_MILLIS = 500L

        /** 键栏底边相对键盘顶边的容差：亚像素取整误差。 */
        private const val BAR_SEAM_TOLERANCE_PX = 8

        /** 行指纹在每个箱内的抽样步长。 */
        private const val ROW_BIN_SAMPLE_STEP_PX = 2
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        composeTestRule.waitForSession()
        UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
            composeTestRule.getBridge() != null
        }
        assertNotNull("运行时桥必须就绪（30s 未孵化）", composeTestRule.getBridge())
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    private fun pumpAndText(): String? {
        runCatchingCancellable { NativeBridge.pollEvent() }
        return bridge().getTerminalText()
    }

    /** 等 shell 提示符就绪后经 shell 打印输出（writeToPty 口径），回显重发防冷 stdin 丢失。 */
    private fun printAndAwait(command: String, needle: String) {
        val promptSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                val text = pumpAndText().orEmpty()
                text.contains("$") || text.contains("#")
            }
        assertNotNull("shell prompt 未出现", promptSeen)
        // 冷启动 stdin 竞态：prompt 落格不等于 shell 已消费 stdin，单次写入
        // 可能丢失（粘贴案定案同类）。幂等重发至多 3 次，回显即停。
        var seen: Long? = null
        repeat(3) {
            bridge().writeToPty("$command\n".toByteArray(Charsets.UTF_8))
            seen =
                UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                    pumpAndText()?.contains(needle) == true
                }
            if (seen != null) return
        }
        assertNotNull("标记必须落格: $needle", seen)
    }

    /** 真实点击终端中央弹出输入法（与用户点击同路径），等待窗口内边距稳定。 */
    private fun tapAndAwaitIme() {
        val surface = findTerminalSurface(composeTestRule.activity)
        val loc = IntArray(2)
        surface.getLocationOnScreen(loc)
        injectTap(
            surface,
            (loc[0] + surface.width / 2).toFloat(),
            (loc[1] + surface.height / 2).toFloat(),
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
        Thread.sleep(SETTLE_MILLIS)
    }

    private fun imeHeightPx(): Int {
        var height = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val surface = findTerminalSurface(composeTestRule.activity)
            val surfaceBottom =
                surface.rootWindowInsets
                    ?.getInsets(WindowInsets.Type.ime())?.bottom ?: 0
            val decorBottom =
                composeTestRule.activity.window.decorView.rootWindowInsets
                    ?.getInsets(WindowInsets.Type.ime())?.bottom ?: 0
            // TODO_DIAG_IMETEST:确诊后删除。surface 与 decor 属同一窗口时两者必一致；
            // 若长期分叉，说明焦点/窗口错位，位移比较的前提即不成立。
            if (surfaceBottom != decorBottom) {
                android.util.Log.w(
                    "ImeTest",
                    "DIAG_WINDOW surfaceIme=$surfaceBottom decorIme=$decorBottom",
                )
            }
            height = surfaceBottom
        }
        return height
    }

    /** 状态栏高度（px）：稀疏内容的条带取在它之下，避开系统时钟等无关像素。 */
    private fun statusBarHeightPx(): Int {
        var height = 0
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            height =
                findTerminalSurface(composeTestRule.activity).rootWindowInsets
                    ?.getInsets(WindowInsets.Type.statusBars())?.bottom ?: 0
        }
        return height
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
    ): Int {
        var count = 0
        for (y in top until bottom step 3) {
            for (x in 0 until first.width step 3) {
                if (pixelDelta(first.getPixel(x, y), second.getPixel(x, y)) > 40) count++
            }
        }
        return count
    }

    /**
     * 在顶部条带里搜索终端内容的上移量，返回（位移像素，匹配差异）。
     * 条带取在键盘永远碰不到的高处：键盘扫过只改变底部像素，顶部条带只随终端
     * 平移而动——扫过恒得位移 0，只有真正的终端上移才能给出显著位移。
     *
     * 位移逐像素扫描而非按步长抽样：条带差异地形是尖峰——真值 820 处代价 0，
     * 相邻 818/822 处代价已到 34 万，按步长抽样会整体落空。
     *
     * 代价控制：每行压成一个墨量标量（横向抽样求和），位移搜索退化为标量序列的
     * 逐行绝对差，O(maxShift × 条带行数) 次整数减法。横向分箱或逐像素比对会把
     * getPixel 的 JNI 调用推到数十万次，在慢模拟器上单次搜索即以分钟计
     * （实测整类用例卡死 20 分钟以上）。相邻行列的字形分布不同，标量已足以定位；
     * 实测步长放宽到 12px 仍精确命中（820，代价 0）。
     *
     * 索引注意：上移后匹配内容落在条带**上方**，故第二张图必须自 `stripTop - maxShift`
     * 起建指纹；只取条带自身高度会与位移下界产生「比较行数越少越容易命中」的假解。
     */
    private fun bestUpwardShift(
        first: android.graphics.Bitmap,
        second: android.graphics.Bitmap,
        stripTop: Int,
        stripHeight: Int,
        maxShift: Int,
        firstInk: IntArray,
    ): Pair<Int, Int> {
        val secondInk = rowInk(second, stripTop - maxShift, maxShift + stripHeight)
        var bestShift = 0
        var bestCost = Long.MAX_VALUE
        for (shift in 0..maxShift) {
            var cost = 0L
            var row = 0
            while (row < stripHeight) {
                cost += kotlin.math.abs(
                    (firstInk[row] - secondInk[row - shift + maxShift]).toLong(),
                )
                row++
            }
            if (cost < bestCost) {
                bestCost = cost
                bestShift = shift
            }
            if (bestCost == 0L) break
        }
        // 返回值仍按整幅条带逐像素计分，使 STRIP_MATCH_MAX_DIFF 与旧口径一致。
        return bestShift to shiftDiff(first, second, stripTop, stripHeight, bestShift)
    }

    /** 条带墨量总和：> 0 表示该条带确有内容像素（非纯背景）。 */
    private fun stripInk(bitmap: android.graphics.Bitmap, top: Int, height: Int): Int =
        rowInk(bitmap, top, height).sum()

    /** 每行墨量：从 [top] 起连续 [height] 行，每行抽样像素的亮度之和。 */
    private fun rowInk(bitmap: android.graphics.Bitmap, top: Int, height: Int): IntArray = IntArray(height) { row ->
        val y = top + row
        var sum = 0
        var x = 0
        while (x < bitmap.width) {
            sum += android.graphics.Color.red(bitmap.getPixel(x, y))
            x += ROW_INK_SAMPLE_STEP_PX
        }
        sum
    }

    /** 给定位移下条带内的像素差异数；越界行按跳过处理，避免负下标。 */
    private fun shiftDiff(
        first: android.graphics.Bitmap,
        second: android.graphics.Bitmap,
        stripTop: Int,
        stripHeight: Int,
        shift: Int,
    ): Int {
        var diff = 0
        var y = 0
        while (y < stripHeight) {
            val sourceY = stripTop + y - shift
            if (sourceY >= 0) {
                var x = 0
                while (x < first.width) {
                    if (pixelDelta(first.getPixel(x, stripTop + y), second.getPixel(x, sourceY)) > 40) {
                        diff++
                    }
                    x += PIXEL_SAMPLE_STEP_PX
                }
            }
            y += PIXEL_SAMPLE_STEP_PX
        }
        return diff
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
                val imm =
                    composeTestRule.activity.getSystemService(
                        android.content.Context.INPUT_METHOD_SERVICE,
                    ) as android.view.inputmethod.InputMethodManager
                imm.hideSoftInputFromWindow(
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
     * ——终端区实测全空（提示符由 y=134 落到 y=−686），即本用例要钉住的回归。
     *
     * 故此处度量「弹出前后同一像素条带是否完全一致」而非位移像素：稀疏会话
     * 位移为 0，两帧条带必须逐像素相同（差分 ≤ 采样噪声）；同时条带在两帧
     * 都必须有墨迹，否则「一致」只是两帧同样空白。
     */
    @Test
    fun contentFewImePopupTerminalStaysPutAndVisible() {
        val marker = "IME_FEW_${System.currentTimeMillis() % 100000}"
        printAndAwait("printf '$marker\\n'", marker)
        // 光标闪烁是逐像素比对的噪声源（块光标约一格，差分数千像素）：
        // 比对期间隐藏光标，测完恢复。try-finally 保证断言失败也不污染后继用例。
        bridge().writeToPty("\u001b[?25l".toByteArray(Charsets.UTF_8))
        try {
            contentFewImePopupTerminalStaysPutAndVisibleBody(marker)
        } finally {
            bridge().writeToPty("\u001b[?25h".toByteArray(Charsets.UTF_8))
        }
    }

    private fun contentFewImePopupTerminalStaysPutAndVisibleBody(marker: String) {
        Thread.sleep(SETTLE_MILLIS)
        hideImeAndSettle()
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        // 条带取在状态栏之下（避开系统时钟像素）、键盘之上（保证内容确实可见）。
        val stripTop = statusBarHeightPx() + SPARSE_STRIP_TOP_MARGIN_PX
        assertTrue(
            "比对条带必须位于键盘上方 (条带底=${stripTop + SPARSE_STRIP_HEIGHT_PX} 键盘顶边=${device.displayHeight - imeHeightPx()})",
            stripTop + SPARSE_STRIP_HEIGHT_PX <= device.displayHeight - imeHeightPx(),
        )
        assertTrue(
            "弹出前条带必须有内容像素（否则「无变化」断言无意义）",
            stripInk(before, stripTop, SPARSE_STRIP_HEIGHT_PX) > 0,
        )
        tapAndAwaitIme()
        val imeHeight = imeHeightPx()
        assertTrue("输入法必须占据高度", imeHeight > 0)
        // 键栏完整位于键盘上方：底边不得低于键盘顶边（差一个取整容差内）。
        val keyboardTop = device.displayHeight - imeHeight
        val barBottom =
            composeTestRule.onNodeWithTag("ModifierBarOverlay", useUnmergedTree = true)
                .fetchSemanticsNode().boundsInRoot.bottom
        assertTrue(
            "键栏必须完整位于输入法上方 (键栏底边=$barBottom 键盘顶边=$keyboardTop)",
            barBottom <= keyboardTop + BAR_SEAM_TOLERANCE_PX,
        )
        val after = device.takeScreenshot() ?: throw AssertionError("截图失败")
        assertTrue(
            "弹出后条带必须有内容像素（内容不得被键盘吞掉或推离屏幕）",
            stripInk(after, stripTop, SPARSE_STRIP_HEIGHT_PX) > 0,
        )
        val stayedDiff =
            countDifferingPixels(before, after, stripTop, stripTop + SPARSE_STRIP_HEIGHT_PX)
        assertTrue(
            "内容较少时弹出输入法终端必须无变化 (差分=$stayedDiff)",
            stayedDiff <= SPARSE_STRIP_MAX_DIFF,
        )
        // 定居后无闪烁：键盘上方整片区域连续两帧必须一致。
        Thread.sleep(SETTLE_MILLIS)
        val settled = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val band = countDifferingPixels(after, settled, 0, after.height - imeHeight)
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
            android.util.Log.w("ImeTest", "DIAG_RACE before 截图时键盘重弹，重试收起 ($attempt/3)")
        }
        val beforeShot = before ?: throw AssertionError("截图失败")
        // 前提钉死：before 必须是无键盘态，否则「位移=0」是 before 已上移的假象
        // 而非产品未动（自动弹键盘与收起竞态，见 hideImeAndSettle 注释）。
        assertTrue(
            "before 截图时输入法必须已收起，否则位移比较无意义",
            !isImeVisible(),
        )
        val beforeText = pumpAndText() ?: throw AssertionError("弹出前终端文本不可读")
        tapAndAwaitIme()
        val imeHeight = imeHeightPx()
        assertTrue("输入法必须占据高度", imeHeight > 0)
        // 内容较多时终端内容上移：慢模拟器上内边距动画可滞后数秒，固定等待即拍即判必抖动。
        // 位移在顶部条带里度量（键盘永远碰不到的高处）：旧的底部区域差分口径会被键盘
        // 扫过动画触发——扫过只改变底部像素，顶部条带只随终端平移而动。
        // 轮询至上移出现（15s 上限），成功帧留给闪烁/缝线检查。
        val stripTop = beforeShot.height * 4 / 10
        val stripHeight = 150
        val maxShift = minOf(imeHeight, stripTop)
        // before 逐像素基线在轮询中不变，预先算一次行墨量避免重复采样。
        val beforeInk = rowInk(beforeShot, stripTop, stripHeight)
        var moved: android.graphics.Bitmap? = null
        var bestShift = 0
        var bestDiff = Int.MAX_VALUE
        val moveDeadline = android.os.SystemClock.uptimeMillis() + 15_000L
        while (android.os.SystemClock.uptimeMillis() < moveDeadline) {
            val shot = device.takeScreenshot() ?: throw AssertionError("截图失败")
            val (shift, diff) =
                bestUpwardShift(beforeShot, shot, stripTop, stripHeight, maxShift, beforeInk)
            if (shift > bestShift || (shift == bestShift && diff < bestDiff)) {
                bestShift = shift
                bestDiff = diff
            }
            if (bestShift > MOVE_MIN_SHIFT_PX && bestDiff <= STRIP_MATCH_MAX_DIFF) {
                moved = shot
                break
            }
            Thread.sleep(500)
        }
        assertTrue(
            "内容较多时弹出输入法终端内容必须上移 (位移=$bestShift 差异=$bestDiff)",
            moved != null,
        )
        // 动画定居后再取成功帧：移动中途的帧不能作为闪烁/缝线基准。
        Thread.sleep(1_000)
        val movedFrame = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val (settledShift, settledDiff) =
            bestUpwardShift(beforeShot, movedFrame, stripTop, stripHeight, maxShift, beforeInk)
        assertTrue(
            "上移必须保持到动画定居 (位移=$settledShift 差异=$settledDiff)",
            settledShift > MOVE_MIN_SHIFT_PX && settledDiff <= STRIP_MATCH_MAX_DIFF,
        )
        // 上移后无闪烁：稳定后连续两帧必须一致（仍在顶部条带内比较，不受键盘影响）。
        Thread.sleep(SETTLE_MILLIS)
        val settled = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val flickerDiff = countDifferingPixels(movedFrame, settled, stripTop, stripTop + stripHeight)
        assertTrue("上移稳定后必须无闪烁 (差分=$flickerDiff)", flickerDiff <= 5)
        // 上移前后底部内容完全相同：弹出只平移容器，不改变网格内容。
        // 弹出后的两帧互比恒为零，无法发现吞底；像素逐行比对在位移下恒不等，
        // 故以终端文本落实 TESTING 底部像素条款。
        val settledText = pumpAndText() ?: throw AssertionError("定居后终端文本不可读")
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
            findTerminalSurface(composeTestRule.activity).onCreateInputConnection(editorInfo)
                ?.commitText(typedEnter, 1)
        }
        // 回显行可能在视口边缘换行（提示符 39 列 + 输入 15 列 > 48 列宽），逐行 contains 会把换行切断的针误判为缺失：压平换行后再判。
        val typedSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText()?.replace("\n", "")?.contains(typed) == true
            }
        assertNotNull("输入文本必须落格: $typed", typedSeen)
        val packed = bridge().getGridRowsColsPacked()
        val rows = (packed shr 32).toInt()
        val cols = (packed and 0xFFFFFFFFL).toInt()
        val flattened = bridge().getTerminalText().orEmpty().replace("\n", "")
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
                pumpAndText().orEmpty().contains("$") || pumpAndText().orEmpty().contains("#")
            }
        assertNotNull("shell prompt 未出现", promptSeen)
        composeTestRule.activity.runOnUiThread {
            val editorInfo = android.view.inputmethod.EditorInfo()
            findTerminalSurface(composeTestRule.activity).onCreateInputConnection(editorInfo)
                ?.commitText("中文\n", 1)
        }
        val chineseSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText()?.replace("\n", "")?.contains("中文") == true
            }
        assertNotNull("中文提交必须落格", chineseSeen)
    }
}
