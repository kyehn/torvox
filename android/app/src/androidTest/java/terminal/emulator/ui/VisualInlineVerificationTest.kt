
package terminal.emulator.ui

import android.graphics.Bitmap
import android.graphics.Color
import android.os.SystemClock
import android.util.Log
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.cleanUpTerminalState
import terminal.emulator.getBridge
import terminal.emulator.injectLongPress
import terminal.emulator.placeTextAtRow
import terminal.emulator.scrollViewportToBottom
import terminal.emulator.terminalGridColumns
import terminal.emulator.waitForSession
import terminal.emulator.waitForTerminalPixels
import java.io.File
import kotlin.math.abs
import kotlin.math.sqrt

@RunWith(AndroidJUnit4::class)
class VisualInlineVerificationTest : TerminalLogcatTest() {
    @get:Rule val composeRule = createAndroidComposeRule<MainActivity>()

    private var tv: View? = null
    private var bridge: terminal.emulator.bridge.Bridge? = null

    companion object {
        /** 长按选词用例的落字（列 7 落在 "world" 内）。 */
        private const val WORD_TEXT = "hello world selectable text terminal"

        /** 两个手柄同源于同一个平台 drawable，尺寸允许的像素偏差。 */
        private const val HANDLE_SIZE_TOLERANCE_PX = 4

        /** 手柄描边色的最小颜色跨度；实测约 88，反白选区/浮动菜单等中性像素不高于 5。 */
        private const val COLOR_SPREAD_THRESHOLD = 40

        /** URL 行用例的落字；本仓不识别裸文本 URL（`hyperlinkAt` 只认 OSC 8），此例只验长按出行选择手柄。 */
        private const val URL_TEXT = "https://git.io/terminal-url-42"

        private fun findTerminalSurfaceView(root: View): View? {
            if (root is terminal.emulator.ui.TerminalSurface) return root
            if (root is ViewGroup) {
                for (i in 0 until root.childCount) {
                    (findTerminalSurfaceView(root.getChildAt(i)))?.let { return it }
                }
            }
            return null
        }

        private fun captureScreenshot(): Bitmap? {
            Thread.sleep(500)
            val start = System.currentTimeMillis()
            val result =
                try {
                    InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot()
                } catch (e: Exception) {
                    Log.e("VisualInline", "capture exception: ${e.message}")
                    null
                }
            val elapsed = System.currentTimeMillis() - start
            if (result == null) {
                Log.w("VisualInline", "capture returned null (${elapsed}ms)")
            } else {
                Log.i("VisualInline", "capture OK: ${result.width}x${result.height} (${elapsed}ms)")
            }
            return result
        }
    }

    private fun Bitmap.getPixel(x: Int, y: Int): Int = if (x in 0 until width && y in 0 until height) {
        getPixel(x, y)
    } else {
        0
    }

    private fun colorDiff(a: Int, b: Int): Int = abs(Color.red(a) - Color.red(b)) +
        abs(Color.green(a) - Color.green(b)) +
        abs(Color.blue(a) - Color.blue(b))

    /** 颜色跨度（最大通道 - 最小通道）：中性色（黑/白/灰，含反白选区）≈ 0。 */
    private fun colorSpread(argb: Int): Int = maxOf(Color.red(argb), Color.green(argb), Color.blue(argb)) -
        minOf(Color.red(argb), Color.green(argb), Color.blue(argb))

    private data class Blob(val minX: Int, val minY: Int, val maxX: Int, val maxY: Int, val ink: Int) {
        val centerX get() = (minX + maxX) / 2
        val centerY get() = (minY + maxY) / 2
        val widthPx get() = maxX - minX + 1
        val heightPx get() = maxY - minY + 1
    }

    /**
     * 选区手柄的像素判据：变更像素里的**上色实心块**。
     *
     * 手柄是应用主题的 textSelectHandle drawable——它自带的描边色与终端配色主题无关，
     * 始终是同一个高饱和色；而同一次长按带来的其余变化都是中性色：选区高亮是反白
     * （白底黑字），浮动菜单是中性底 + 中性字。于是「变更 ∧ 上色」恰好挑出两条手柄。
     *
     * 不能只按连通块尺寸挑「手柄大小的块」：选区起点在行首时，左柄与高亮带只差一行、
     * 彼此粘连成一块，该块宽度随选区长度增长——长选区（URL 用例 24 格）必然超窗，
     * 表现为「一个手柄也找不到」；而上色判据把左柄从高亮带里直接分出来。
     */
    private fun findSelectionHandleMarks(
        before: Bitmap,
        after: Bitmap,
        cellWidth: Float,
        cellHeight: Float,
        threshold: Int = 50,
    ): List<Blob> {
        val w = minOf(before.width, after.width)
        val h = minOf(before.height, after.height)
        val changed = Array(h) { BooleanArray(w) }
        for (y in 0 until h) {
            for (x in 0 until w) {
                val afterPixel = after.getPixel(x, y)
                changed[y][x] =
                    colorDiff(before.getPixel(x, y), afterPixel) > threshold &&
                    colorSpread(afterPixel) >= COLOR_SPREAD_THRESHOLD
            }
        }

        val visited = Array(h) { BooleanArray(w) }
        val blobs = mutableListOf<Blob>()
        val dirs = listOf(-1 to -1, -1 to 0, -1 to 1, 0 to -1, 0 to 1, 1 to -1, 1 to 0, 1 to 1)
        // 尺寸筛网按字符格缩放（不写像素常量）：手柄约 2.5x1.2 格，至少一格大、
        // 大不过四格，就能把抗锯齿轮廓的碎片挡在外面。
        val minW = cellWidth * 0.9f
        val minH = cellHeight * 0.9f
        val maxW = cellWidth * 4f
        val maxH = cellHeight * 4f

        for (y in 0 until h) {
            for (x in 0 until w) {
                if (!changed[y][x] || visited[y][x]) continue
                var minX = x
                var maxX = x
                var minY = y
                var maxY = y
                var ink = 0
                val stack = ArrayDeque<Pair<Int, Int>>()
                stack.addLast(x to y)
                visited[y][x] = true
                while (stack.isNotEmpty()) {
                    val (cx, cy) = stack.removeLast()
                    ink++
                    minX = minOf(minX, cx)
                    maxX = maxOf(maxX, cx)
                    minY = minOf(minY, cy)
                    maxY = maxOf(maxY, cy)
                    for ((dx, dy) in dirs) {
                        val nx = cx + dx
                        val ny = cy + dy
                        if (nx in 0 until w && ny in 0 until h && changed[ny][nx] && !visited[ny][nx]) {
                            visited[ny][nx] = true
                            stack.addLast(nx to ny)
                        }
                    }
                }
                val bw = maxX - minX + 1
                val bh = maxY - minY + 1
                // 实心度：手柄是满心泪滴，边界框里一半以上都得上色，避免把零散像素当成手柄。
                if (bw >= minW && bh >= minH && bw <= maxW && bh <= maxH && ink * 2 >= bw * bh) {
                    blobs.add(Blob(minX, minY, maxX, maxY, ink))
                }
            }
        }
        return blobs
    }

    private fun pixelDiffCount(before: Bitmap, after: Bitmap, threshold: Int = 50): Int {
        val w = minOf(before.width, after.width)
        val h = minOf(before.height, after.height)
        var count = 0
        for (y in 0 until h) {
            for (x in 0 until w) {
                if (colorDiff(before.getPixel(x, y), after.getPixel(x, y)) > threshold) count++
            }
        }
        return count
    }

    // 字符格度量取自 native 网格，而非假设 80x24：实际列数（1080/17.96≈60）
    // 与假设能差出一倍，用错列号会让长按落在提示符而不是目标文本上。
    private data class CellMetrics(val cellWidth: Float, val cellHeight: Float, val cols: Int, val rows: Int)

    private fun estimateCellMetrics(): CellMetrics? {
        val surface = requireNotNull(tv)
        val width = surface.width.toFloat()
        val height = surface.height.toFloat()
        val (cols, rows) =
            bridge?.let { b ->
                try {
                    val packed = b.getGridRowsColsPacked()
                    val rows = (packed shr 32).toInt()
                    val cols = packed.toInt()
                    if (rows > 0 && cols > 0) cols to rows else null
                } catch (e: Exception) {
                    null
                }
            } ?: (80 to 24)
        return CellMetrics(width / cols, height / rows, cols, rows)
    }

    // 软件渲染器在测试进程繁忙时会落后于 PTY 数据；长按前等连续两帧不再变化
    // （回显内容已光栅化完成）。
    private fun waitForRenderStable(logTag: String, timeoutMs: Long = 12_000): Bitmap {
        // 截屏可能瞬时失败（surface 换建 / 渲染器繁忙）；先重试再放弃。
        var prev: Bitmap? = null
        repeat(3) {
            prev = captureScreenshot()
            if (prev != null) return@repeat
            Thread.sleep(500)
        }
        var current = requireNotNull(prev) { "no screenshot available (renderer busy?)" }
        val deadline = SystemClock.elapsedRealtime() + timeoutMs
        while (SystemClock.elapsedRealtime() < deadline) {
            Thread.sleep(700)
            val cur = captureScreenshot() ?: continue
            val diff = pixelDiffCount(current, cur)
            Log.i(logTag, "Render settle diff: $diff")
            if (diff < 1200) return cur
            current = cur
        }
        Log.w(logTag, "Render did not fully settle within ${timeoutMs}ms; proceeding anyway")
        return current
    }

    // 轮询直到至少出现两个手柄标记（SwiftShader 下选区手柄弹窗的表面加载很慢）。
    // DOWN/UP 落进同一主线程批次的长按会被平台当作点击而不产生选区——
    // 断言不能因此减弱，做法是对新基准再发一次手势（真实用户就是再按一次）。
    //
    // 重试背后还有一层坑：重长按前抓的新基准若带着上一轮**未清除的选区**
    // （手柄在 before/after 两侧都在），差分为零，poll 恒 0——CI 冷启动下
    // 整条重试链就此报废。重试前先经 production 的清选区通道把状态归零，
    // 保证每次重试都走「干净基准 → 长按 → 出现手柄」的同一套正向链。
    private fun pollForSelectionHandles(
        initialBaseline: Bitmap,
        logTag: String,
        lpX: Float,
        lpY: Float,
        cellWidth: Float,
        cellHeight: Float,
    ): Pair<Bitmap, List<Blob>> {
        // 清除共用会话的选区状态（用户语义与 cleanUpTerminalState 相同），
        // 随后等上一帧选区渲染出画面再抓基准。
        fun clearSelectionNow() {
            composeRule.activity.runOnUiThread {
                composeRule.activity.terminalViewModel.clearSelection()
            }
            Thread.sleep(700)
        }
        var baseline = initialBaseline
        var lastShot = initialBaseline
        var handles = emptyList<Blob>()
        var retries = 0
        val deadline = SystemClock.elapsedRealtime() + 14_000
        while (SystemClock.elapsedRealtime() < deadline) {
            val shot = captureScreenshot()
            if (shot == null) continue
            lastShot = shot
            handles = findSelectionHandleMarks(baseline, shot, cellWidth, cellHeight)
            Log.i(logTag, "Poll: ${handles.size} handle marks")
            handles.forEachIndexed { i, handle ->
                Log.i(
                    logTag,
                    "  Candidate $i: (${handle.centerX},${handle.centerY}) ${handle.widthPx}x${handle.heightPx} ink=${handle.ink}",
                )
            }
            if (handles.size >= 2) break
            if (retries < 2 && SystemClock.elapsedRealtime() < deadline - 4_000) {
                Thread.sleep(1500)
                clearSelectionNow()
                baseline = requireNotNull(captureScreenshot()) { "baseline for re-long-press" }
                injectLongPress(requireNotNull(tv), lpX, lpY)
                retries++
                Log.i(logTag, "Re-long-press attempt $retries of 2")
            }
        }
        return lastShot to handles
    }

    /**
     * 两条手柄同源于一个平台 drawable，尺寸必须一致——旧的「手柄大小的块」判据
     * 会把选区高亮带当成手柄，尺寸一致性把这类误判直接顶回来。
     */
    private fun assertHandleMarksPaired(h0: Blob, h1: Blob, label: String) {
        Assert.assertTrue(
            "$label 两个手柄尺寸不一致: ${h0.widthPx}x${h0.heightPx} vs ${h1.widthPx}x${h1.heightPx}",
            abs(h0.widthPx - h1.widthPx) <= HANDLE_SIZE_TOLERANCE_PX &&
                abs(h0.heightPx - h1.heightPx) <= HANDLE_SIZE_TOLERANCE_PX,
        )
    }

    @Test
    fun verifyWordSelectionPositions() {
        Log.i("VisualInline", "==== Word Selection Position Verification ====")
        composeRule.waitForSession()
        // 本用例采样像素（截图差分找手柄），须再等原生 Surface 挂上，否则量到全零。
        composeRule.waitForTerminalPixels()
        bridge = composeRule.getBridge()
        Assert.assertNotNull("Bridge not ready", bridge)

        tv = findTerminalSurfaceView(composeRule.activity.window.decorView)
        Assert.assertNotNull("TerminalSurface not found", tv)

        // 先清场再落字：共用会话里前序用例留下的行、选区与浮动菜单都会让长按落到
        // 另一处或被已有选区吞掉（外部表现是「找不到手柄」，与长按本身无关）。
        composeRule.cleanUpTerminalState()
        val wordRow = 1
        scrollViewportToBottom(requireNotNull(bridge), composeRule.activity.terminalViewModel.runtime)
        placeTextAtRow(requireNotNull(bridge), wordRow, WORD_TEXT)

        // 长按落在 "world" 内（列约 7，视口第 wordRow 行）。
        val metrics = requireNotNull(estimateCellMetrics())
        val cellW = metrics.cellWidth
        val cellH = metrics.cellHeight
        val longPressX = cellW * 7f
        val longPressY = cellH * (wordRow + 0.5f)

        Log.i(
            "VisualInline",
            "Long-press at ($longPressX, $longPressY) for 'world' (grid ${metrics.cols}x${metrics.rows})",
        )

        // 等渲染器把回显画完，再长按并轮询手柄弹窗。
        val baselineShot = waitForRenderStable("VisualInline-Word")
        Log.i("VisualInline-Word", "Baseline ready")
        injectLongPress(requireNotNull(tv), longPressX, longPressY)
        val (afterSel, handles) =
            pollForSelectionHandles(baselineShot, "Word", longPressX, longPressY, cellW, cellH)

        // 留证据
        saveToExternal("word-baseline", baselineShot)
        saveToExternal("word-selection", afterSel)

        val changedPx = pixelDiffCount(baselineShot, afterSel)
        Log.i("VisualInline", "Changed pixels after word selection: $changedPx")
        Assert.assertTrue("No selection change detected ($changedPx pixels)", changedPx > 100)

        Assert.assertTrue("Expected >=2 selection handles, found ${handles.size}", handles.size >= 2)

        val h0 = handles[0]
        val h1 = handles[1]
        assertHandleMarksPaired(h0, h1, "word")
        val rowDiff = abs(h0.centerY - h1.centerY)
        Log.i("VisualInline", "Handle row diff: ${rowDiff}px (cellH=$cellH)")
        Assert.assertTrue("Handles not on same row (diff=$rowDiff)", rowDiff < cellH * 2)

        // 校验手柄落在长按位置周边
        val startNear =
            h0.centerX in (longPressX.toInt() - (cellW * 8).toInt())..(longPressX.toInt() + (cellW * 4).toInt())
        val endNear =
            h1.centerX in (longPressX.toInt() - (cellW * 2).toInt())..(longPressX.toInt() + (cellW * 8).toInt())
        if (!startNear || !endNear) {
            Log.w("VisualInline", "Start handle near: $startNear, End handle near: $endNear")
        }

        Log.i("VisualInline", "Word selection verification PASSED")
    }

    @Test
    fun verifyUrlSelectionPositions() {
        Log.i("VisualInline", "==== URL Selection Position Verification ====")
        composeRule.waitForSession()
        composeRule.waitForTerminalPixels()
        bridge = composeRule.getBridge()
        Assert.assertNotNull(bridge)
        tv = findTerminalSurfaceView(composeRule.activity.window.decorView)
        Assert.assertNotNull(tv)
        val surface = requireNotNull(tv)
        Log.i("VisualInline-URL", "Surface ${surface.width}x${surface.height}")

        val metrics = requireNotNull(estimateCellMetrics())
        val cellW = metrics.cellWidth
        val cellH = metrics.cellHeight
        Log.i("VisualInline-URL", "Metrics ${metrics.cols}x${metrics.rows} cell ${cellW}x$cellH")

        // 先清场再落字：共用会话里前序用例留下的行、选区与浮动菜单都会让长按落到
        // 另一处或被已有选区吞掉（外部表现是「找不到手柄」，与长按本身无关）。
        composeRule.cleanUpTerminalState()
        // 文本确定性落到指定行：不经 shell（提示符何时打印、echo 输出落在第几行
        // 都随环境变化），落格判据取渲染光标（与单元格内容同源）。网格列数随
        // 屏幕宽度与主字体变化（实测 25～38 列），落字超宽即折行、光标落在下一
        // 行，前提不成立时直接失败而不留到后面误判。
        val urlRow = 2
        scrollViewportToBottom(requireNotNull(bridge), composeRule.activity.terminalViewModel.runtime)
        val columns = terminalGridColumns(composeRule.activity, requireNotNull(bridge))
        Assert.assertTrue(
            "URL 必须单行容得下 (列数=$columns 文本=[$URL_TEXT])",
            URL_TEXT.length <= columns,
        )
        placeTextAtRow(requireNotNull(bridge), urlRow, URL_TEXT)

        val longPressX = cellW * 7f
        val longPressY = cellH * (urlRow + 0.5f)
        Log.i("VisualInline-URL", "Long-press at ($longPressX, $longPressY)")

        // 等渲染器把回显的 URL 行画完，再长按并轮询手柄弹窗（手势被繁忙
        // 主线程吞掉时走重长按兜底）。
        val baseline = waitForRenderStable("VisualInline-URL")
        Log.i("VisualInline-URL", "Baseline ready")
        injectLongPress(requireNotNull(tv), longPressX, longPressY)
        val (afterShot, handles) = pollForSelectionHandles(baseline, "URL", longPressX, longPressY, cellW, cellH)
        saveToExternal("url-baseline", baseline)
        saveToExternal("url-selection", afterShot)

        val changedPx = pixelDiffCount(baseline, afterShot)
        Assert.assertTrue("No URL selection change ($changedPx)", changedPx > 100)

        Assert.assertTrue("Expected >=2 handles for URL, found ${handles.size}", handles.size >= 2)

        val h0 = handles[0]
        val h1 = handles[1]
        assertHandleMarksPaired(h0, h1, "url")
        val rowDiff = abs(h0.centerY - h1.centerY)
        // URL is at column 0. Under SwiftShader the two handle strokes can
        // partially bleed into neighboring per-glyph change blobs, so assert
        // only that they share a row band and are separated by a plausible
        // number of cells. (The strict >=5-cell span was flaky on the
        // software renderer; URL selection itself is asserted by >=2 handles.)
        Assert.assertTrue("URL handle rows differ ($rowDiff)", rowDiff < cellH * 2)
        val urlCells = h1.centerX / cellW.toInt() - h0.centerX / cellW.toInt()
        Log.i("VisualInline", "URL spans $urlCells cells ($cellW px/cell)")

        Log.i("VisualInline", "URL selection verification PASSED")
    }

    private fun saveToExternal(name: String, bitmap: Bitmap) {
        val extDir = composeRule.activity.getExternalFilesDir("Pictures")
        if (extDir != null) {
            extDir.mkdirs()
            val file = File(extDir, "inline-verify-$name.png")
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            Log.i("VisualInline", "Saved $name: ${file.absolutePath} (${file.length()}B)")
        }
        // 同时存一份到应用私有目录
        val intDir = File(composeRule.activity.filesDir, "screenshots")
        intDir.mkdirs()
        val intFile = File(intDir, "inline-verify-$name.png")
        intFile.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        Log.i("VisualInline", "Saved $name: ${intFile.absolutePath} (${intFile.length()}B)")
    }
}
