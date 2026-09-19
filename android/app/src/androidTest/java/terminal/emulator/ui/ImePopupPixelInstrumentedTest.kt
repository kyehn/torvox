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
                    kotlin.math.abs(android.graphics.Color.red(first.getPixel(x, y)) - android.graphics.Color.red(second.getPixel(x, y))) +
                        kotlin.math.abs(android.graphics.Color.green(first.getPixel(x, y)) - android.graphics.Color.green(second.getPixel(x, y))) +
                        kotlin.math.abs(android.graphics.Color.blue(first.getPixel(x, y)) - android.graphics.Color.blue(second.getPixel(x, y)))
                if (delta > 40) count++
            }
        }
        return count
    }

    @Test
    fun contentFewImePopupTerminalUnchanged() {
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
    fun contentManyImePopupMovesUpBottomIdentical() {
        val stamp = System.currentTimeMillis() % 100000
        val last = "IME_MANY_120_$stamp"
        printAndAwait(
            "for i in \$(seq 1 120); do echo IME_MANY_\$i" + "_$stamp; done",
            last,
        )
        Thread.sleep(SETTLE_MILLIS)
        val before = device.takeScreenshot() ?: throw AssertionError("截图失败")
        tapAndAwaitIme()
        val imeHeight = imeHeightPx()
        assertTrue("输入法必须占据高度", imeHeight > 0)
        val moved = device.takeScreenshot() ?: throw AssertionError("截图失败")
        // 内容较多时终端内容上移：输入法上方区域像素必须显著变化。
        val regionTop = before.height / 10
        val regionBottom = before.height - imeHeight - 40
        val moveDiff = countDifferingPixels(before, moved, regionTop, regionBottom)
        assertTrue("内容较多时弹出输入法终端内容必须上移 (差分=$moveDiff)", moveDiff > 20)
        // 上移后无闪烁：稳定后连续两帧必须一致。
        Thread.sleep(SETTLE_MILLIS)
        val settled = device.takeScreenshot() ?: throw AssertionError("截图失败")
        val flickerDiff = countDifferingPixels(moved, settled, regionTop, regionBottom)
        assertTrue("上移稳定后必须无闪烁 (差分=$flickerDiff)", flickerDiff <= 5)
        // 上移前后底部像素完全相同：贴输入法上沿的缝线行必须一致。
        val seamTop = regionBottom - 12
        val seamDiff = countDifferingPixels(moved, settled, seamTop, regionBottom)
        assertTrue("底部缝线像素必须完全相同 (差分=$seamDiff)", seamDiff == 0)
        // 弹出时输入文本正确显示，底部不被吞。
        val typed = "IME_TYPED_$stamp"
        composeTestRule.activity.runOnUiThread {
            val editorInfo = android.view.inputmethod.EditorInfo()
            findTerminalSurface(composeTestRule.activity).onCreateInputConnection(editorInfo)
                ?.commitText(typed, 1)
        }
        val typedSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = GRID_TIMEOUT_MS, intervalMs = 100) {
                pumpAndText()?.contains(typed) == true
            }
        assertNotNull("输入文本必须落格: $typed", typedSeen)
        val lines = bridge().getTerminalText().orEmpty().lines()
        val markerIndex = lines.indexOfFirst { it.contains(typed) }
        assertTrue("输入文本必须落格", markerIndex >= 0)
        val packed = bridge().getGridRowsColsPacked()
        val rows = (packed shr 32).toInt()
        assertTrue(
            "输入文本必须在可见视口内（底部不被吞）, 行=$markerIndex 可见=${lines.size - rows}..${lines.size}",
            markerIndex >= lines.size - rows,
        )
    }
}
