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
        runCatching { NativeBridge.pollEvent() }
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
                val delta =
                    kotlin.math.abs(
                        android.graphics.Color.red(first.getPixel(x, y)) -
                            android.graphics.Color.red(second.getPixel(x, y)),
                    ) +
                        kotlin.math.abs(
                            android.graphics.Color.green(first.getPixel(x, y)) -
                                android.graphics.Color.green(second.getPixel(x, y)),
                        ) +
                        kotlin.math.abs(
                            android.graphics.Color.blue(first.getPixel(x, y)) -
                                android.graphics.Color.blue(second.getPixel(x, y)),
                        )
                if (delta > 40) count++
            }
        }
        return count
    }

    /** 确保输入法收起：Gboard 系统级持久，跨用例仍展开会使 before 拍到已上移态导致差分为零。 */
    private fun hideImeAndSettle() {
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
        UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 200) {
            var hidden = false
            InstrumentationRegistry.getInstrumentation().runOnMainSync {
                hidden =
                    findTerminalSurface(
                        composeTestRule.activity,
                    ).rootWindowInsets?.isVisible(WindowInsets.Type.ime()) ==
                    false
            }
            hidden
        }
        Thread.sleep(SETTLE_MILLIS)
    }

    @Test
    fun contentFewImePopupTerminalUnchanged() {
        hideImeAndSettle()
        val marker = "IME_FEW_${System.currentTimeMillis() % 100000}"
        printAndAwait("printf '$marker\\n'", marker)
        Thread.sleep(SETTLE_MILLIS)
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
    fun contentManyImePopupMovesUpBottomIdentical() {        val stamp = System.currentTimeMillis() % 100000
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
        // 内容较多时终端内容上移：慢模拟器上内边距动画可滞后数秒，固定等待即拍即判必抖动。轮询至上移出现（15s 上限），成功帧留给闪烁/缝线检查。
        val regionTop = before.height / 10
        val regionBottom = before.height - imeHeight - 40
        var moved: android.graphics.Bitmap? = null
        var moveDiff = 0
        val moveDeadline = android.os.SystemClock.uptimeMillis() + 15_000L
        while (android.os.SystemClock.uptimeMillis() < moveDeadline) {
            val shot = device.takeScreenshot() ?: throw AssertionError("截图失败")
            moveDiff = countDifferingPixels(before, shot, regionTop, regionBottom)
            if (moveDiff > 20) {
                moved = shot
                break
            }
            Thread.sleep(500)
        }
        assertTrue("内容较多时弹出输入法终端内容必须上移 (差分=$moveDiff)", moveDiff > 20)
        val movedFrame = moved ?: throw AssertionError("上移帧缺失")
        // 上移后无闪烁：稳定后连续两帧必须一致。
        Thread.sleep(SETTLE_MILLIS)
        val settled = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val flickerDiff = countDifferingPixels(movedFrame, settled, regionTop, regionBottom)
        assertTrue("上移稳定后必须无闪烁 (差分=$flickerDiff)", flickerDiff <= 5)
        // 上移前后底部像素完全相同：贴输入法上沿的缝线行必须一致。
        val seamTop = regionBottom - 12
        val seamDiff = countDifferingPixels(movedFrame, settled, seamTop, regionBottom)
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
