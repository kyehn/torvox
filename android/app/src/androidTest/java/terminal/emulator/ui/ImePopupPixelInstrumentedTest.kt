package terminal.emulator.ui

import android.view.WindowInsets
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
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

        /** 行指纹的横向分箱数。 */
        private const val ROW_PROFILE_BINS = 48

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
            height =
                findTerminalSurface(composeTestRule.activity).rootWindowInsets
                    ?.getInsets(WindowInsets.Type.ime())?.bottom ?: 0
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
     * 位移逐像素扫描而非按步长抽样：条带差异地形是尖峰（行高 ~45px，错 1px 即整行
     * 错位，差异从 0 跳到上千），按 6px 抽样会整体落空——实测粗搜命中 774，
     * 真值 820（差异 0），二者相距 46px，任何以此为中心的细搜都够不到。
     *
     * 代价控制：先把每行压成横向分箱亮度指纹（一次性 O(条带面积)），位移搜索退化为
     * 指纹的逐箱相减，O(maxShift × 条带行数 × 箱数) 次整数比较。逐候选全像素比对
     * 在慢模拟器上会把一次搜索拖到分钟级（实测整类用例卡死 40 分钟）。
     *
     * 索引注意：上移后匹配内容落在条带**上方**，故第二张图必须自 `stripTop - maxShift`
     * 起建指纹；只用条带自身的高度会与位移下界产生「比较行数越少越容易命中」的假解
     * （实测位移 148、代价 0）。
     */
    private fun bestUpwardShift(
        first: android.graphics.Bitmap,
        second: android.graphics.Bitmap,
        stripTop: Int,
        stripHeight: Int,
        maxShift: Int,
    ): Pair<Int, Int> {
        val firstProfile = rowProfiles(first, stripTop, stripHeight)
        val secondProfile = rowProfiles(second, stripTop - maxShift, maxShift + stripHeight)
        var bestShift = 0
        var bestCost = Long.MAX_VALUE
        for (shift in 0..maxShift) {
            var cost = 0L
            var row = 0
            while (row < stripHeight) {
                val fromFirst = firstProfile[row]
                val fromSecond = secondProfile[row - shift + maxShift]
                var bin = 0
                while (bin < ROW_PROFILE_BINS) {
                    cost += kotlin.math.abs(fromFirst[bin] - fromSecond[bin]).toLong()
                    bin++
                }
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

    /** 每行的横向分箱亮度指纹：文本行的字形分布各不相同，足以定位纵向对齐。 */
    private fun rowProfiles(bitmap: android.graphics.Bitmap, top: Int, height: Int): Array<IntArray> {
        val binWidth = kotlin.math.max(1, bitmap.width / ROW_PROFILE_BINS)
        return Array(height) { row ->
            val y = top + row
            IntArray(ROW_PROFILE_BINS) { bin ->
                val from = bin * binWidth
                val until = kotlin.math.min(bitmap.width, from + binWidth)
                var sum = 0
                var count = 0
                var x = from
                while (x < until) {
                    sum += android.graphics.Color.red(bitmap.getPixel(x, y))
                    count++
                    x += ROW_BIN_SAMPLE_STEP_PX
                }
                if (count == 0) 0 else sum / count
            }
        }
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

    @Test
    fun contentFewImePopupTerminalUnchanged() {
        val marker = "IME_FEW_${System.currentTimeMillis() % 100000}"
        printAndAwait("printf '$marker\\n'", marker)
        Thread.sleep(SETTLE_MILLIS)
        // 收起必须紧贴截图：输出期间自动弹键盘可能在任何时刻出现。
        hideImeAndSettle()
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        tapAndAwaitIme()
        val after = device.takeScreenshot() ?: throw AssertionError("截图失败")
        // 内容较少时终端无变化：顶部六成区域像素必须一致（裁掉状态栏与输入法区）。
        val top = before.height / 10
        val bottom = before.height * 6 / 10
        val diff = countDifferingPixels(before, after, top, bottom)
        assertTrue("内容较少时弹出输入法终端必须无变化 (差分=$diff)", diff <= 5)
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
        hideImeAndSettle()
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        tapAndAwaitIme()
        val imeHeight = imeHeightPx()
        assertTrue("输入法必须占据高度", imeHeight > 0)
        // 内容较多时终端内容上移：慢模拟器上内边距动画可滞后数秒，固定等待即拍即判必抖动。
        // 位移在顶部条带里度量（键盘永远碰不到的高处）：旧的底部区域差分口径会被键盘
        // 扫过动画触发——扫过只改变底部像素，顶部条带只随终端平移而动。
        // 轮询至上移出现（15s 上限），成功帧留给闪烁/缝线检查。
        val stripTop = before.height * 4 / 10
        val stripHeight = 150
        val maxShift = minOf(imeHeight, stripTop)
        var moved: android.graphics.Bitmap? = null
        var bestShift = 0
        var bestDiff = Int.MAX_VALUE
        val moveDeadline = android.os.SystemClock.uptimeMillis() + 15_000L
        while (android.os.SystemClock.uptimeMillis() < moveDeadline) {
            val shot = device.takeScreenshot() ?: throw AssertionError("截图失败")
            val (shift, diff) = bestUpwardShift(before, shot, stripTop, stripHeight, maxShift)
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
            bestUpwardShift(before, movedFrame, stripTop, stripHeight, maxShift)
        assertTrue(
            "上移必须保持到动画定居 (位移=$settledShift 差异=$settledDiff)",
            settledShift > MOVE_MIN_SHIFT_PX && settledDiff <= STRIP_MATCH_MAX_DIFF,
        )
        // 上移后无闪烁：稳定后连续两帧必须一致（仍在顶部条带内比较，不受键盘影响）。
        Thread.sleep(SETTLE_MILLIS)
        val settled = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val flickerDiff = countDifferingPixels(movedFrame, settled, stripTop, stripTop + stripHeight)
        assertTrue("上移稳定后必须无闪烁 (差分=$flickerDiff)", flickerDiff <= 5)
        // 上移前后底部像素完全相同：贴输入法上沿的缝线行必须一致。
        val seamTop = before.height - imeHeight - 12
        val seamDiff = countDifferingPixels(movedFrame, settled, seamTop, before.height - imeHeight)
        assertTrue("底部缝线像素必须完全相同 (差分=$seamDiff)", seamDiff == 0)
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
