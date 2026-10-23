package terminal.emulator

import android.graphics.Bitmap
import android.graphics.Rect
import android.os.SystemClock
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import terminal.emulator.bridge.Bridge
import terminal.emulator.runtime.TerminalRuntime
import java.io.File

/**
 * Shared helpers for the UX-quantified verification suite.
 *
 * Every test in this suite follows the same contract demanded by the review: a previously
 * user-reported behavior is only "fixed" when an automated, MEASURABLE assertion passes on the
 * emulator — never on code reading alone. Each assertion also logs its measured value as a
 * `UX_METRIC key=value` logcat line so trends are trackable across runs (grep -e UX_METRIC).
 */
object UxTestUtils {
    private const val TAG = "UX_METRIC"

    /** PixelCopy 单次读回的上限（毫秒）：软件渲染设备上读一屏要几百毫秒。 */
    private const val PIXEL_COPY_TIMEOUT_MS = 5_000L

    /**
     * Inject a drag gesture as a real event stream into [view] (the house TestUtils
     * dispatchTouchEvent pattern): ACTION_DOWN at ([x0], [y0]), [steps] evenly spaced ACTION_MOVEs
     * with [stepDelayMs] pacing, then ACTION_UP at ([x1], [y1]). Unlike UiDevice.swipe this exposes
     * the per-step pacing needed for live-highlight sampling.
     */
    fun injectDrag(
        view: android.view.View,
        x0: Float,
        y0: Float,
        x1: Float,
        y1: Float,
        steps: Int = 6,
        stepDelayMs: Long = 120,
    ) {
        val downTime = SystemClock.uptimeMillis()
        fun post(action: Int, x: Float, y: Float) {
            val t = SystemClock.uptimeMillis()
            view.post {
                val event = android.view.MotionEvent.obtain(downTime, t, action, x, y, 0)
                try {
                    view.dispatchTouchEvent(event)
                } finally {
                    event.recycle()
                }
            }
        }
        post(android.view.MotionEvent.ACTION_DOWN, x0, y0)
        try {
            for (step in 1..steps) {
                Thread.sleep(stepDelayMs)
                val frac = step.toFloat() / steps
                post(android.view.MotionEvent.ACTION_MOVE, x0 + (x1 - x0) * frac, y0 + (y1 - y0) * frac)
            }
            Thread.sleep(stepDelayMs)
        } finally {
            post(android.view.MotionEvent.ACTION_UP, x1, y1)
            // Let the main thread drain UP before the caller samples anything.
            Thread.sleep(150)
        }
    }

    /** Capture the full device screen as a bitmap via UiDevice. */
    fun screenshot(device: UiDevice): Bitmap {
        val file = File.createTempFile("uxquant", ".png")
        device.takeScreenshot(file)
        val bitmap = android.graphics.BitmapFactory.decodeFile(file.path)
        file.delete()
        check(bitmap != null) { "takeScreenshot produced an undecodable file" }
        return bitmap
    }

    /**
     * Count pixels that differ materially between two same-sized captures. Materially = any channel
     * delta > [tolerance], which absorbs codec noise while still counting real content changes.
     */
    fun changedPixelCount(before: Bitmap, after: Bitmap, tolerance: Int = 12): Int {
        require(before.width == after.width && before.height == after.height) {
            "capture size mismatch: ${before.width}x${before.height} vs ${after.width}x${after.height}"
        }
        val w = before.width
        val h = before.height
        val a = IntArray(w * h)
        val b = IntArray(w * h)
        before.getPixels(a, 0, w, 0, 0, w, h)
        after.getPixels(b, 0, w, 0, 0, w, h)
        var changed = 0
        for (i in a.indices) {
            val pa = a[i]
            val pb = b[i]
            if (
                kotlin.math.abs((pa shr 16 and 0xFF) - (pb shr 16 and 0xFF)) > tolerance ||
                kotlin.math.abs((pa shr 8 and 0xFF) - (pb shr 8 and 0xFF)) > tolerance ||
                kotlin.math.abs((pa and 0xFF) - (pb and 0xFF)) > tolerance
            ) {
                changed++
            }
        }
        return changed
    }

    /**
     * 两截图在纵向区间 [top, bottom) 内按 [step] 步长抽样的差异像素数：
     * 三通道差和超 [threshold] 记为差异（步长只影响统计速度，不放宽判定）。
     * 稀疏条带比对与 SGR 斜体裁剪区同款口径，故收归此处一处实现。
     */
    fun countDiffInBand(first: Bitmap, second: Bitmap, top: Int, bottom: Int, step: Int, threshold: Int): Int {
        var count = 0
        for (y in top until bottom step step) {
            for (x in 0 until first.width step step) {
                if (pixelChannelDelta(first.getPixel(x, y), second.getPixel(x, y)) > threshold) count++
            }
        }
        return count
    }

    /**
     * Bounding box of pixels differing between [before] and [after], or null when nothing changed
     * beyond [tolerance]. Used by the cursor geometry test: diffing cursor-at-col-A vs
     * cursor-at-col-B isolates exactly the two cursor quads — their bbox IS the measurable cursor
     * geometry, independent of theme colors.
     */
    fun changedBoundingBox(before: Bitmap, after: Bitmap, tolerance: Int = 12): Rect? {
        require(before.width == after.width && before.height == after.height) {
            "capture size mismatch: ${before.width}x${before.height} vs ${after.width}x${after.height}"
        }
        val w = before.width
        val h = before.height
        val a = IntArray(w * h)
        val b = IntArray(w * h)
        before.getPixels(a, 0, w, 0, 0, w, h)
        after.getPixels(b, 0, w, 0, 0, w, h)
        var minX = Int.MAX_VALUE
        var minY = Int.MAX_VALUE
        var maxX = Int.MIN_VALUE
        var maxY = Int.MIN_VALUE
        for (y in 0 until h) {
            for (x in 0 until w) {
                val i = y * w + x
                val pa = a[i]
                val pb = b[i]
                if (
                    kotlin.math.abs((pa shr 16 and 0xFF) - (pb shr 16 and 0xFF)) > tolerance ||
                    kotlin.math.abs((pa shr 8 and 0xFF) - (pb shr 8 and 0xFF)) > tolerance ||
                    kotlin.math.abs((pa and 0xFF) - (pb and 0xFF)) > tolerance
                ) {
                    if (x < minX) minX = x
                    if (x > maxX) maxX = x
                    if (y < minY) minY = y
                    if (y > maxY) maxY = y
                }
            }
        }
        return if (maxX < minX) null else Rect(minX, minY, maxX, maxY)
    }

    /**
     * Poll [predicate] until it turns true or [timeoutMs] elapses. Returns the elapsed milliseconds
     * on success (the measurable latency), null on timeout. [intervalMs] bounds polling cost.
     */
    inline fun pollUntilTrue(timeoutMs: Long, intervalMs: Long = 15, predicate: () -> Boolean): Long? {
        val start = SystemClock.uptimeMillis()
        while (SystemClock.uptimeMillis() - start <= timeoutMs) {
            if (predicate()) return SystemClock.uptimeMillis() - start
            Thread.sleep(intervalMs)
        }
        return null
    }

    /** Record one measurement for trend tracking (grep -e UX_METRIC). */
    fun metric(name: String, value: Number) {
        Log.i(TAG, "$name=$value")
    }

    /**
     * 以 shell 身份执行一条命令并读回标准输出。
     *
     * 仪器化进程本身没有改系统设置的权限，shell 有：`cmd uimode night yes` 这类
     * 系统状态切换必须经它执行，测试进程内的任何 API 都做不到。
     */
    fun runShellCommand(command: String): String = InstrumentationRegistry.getInstrumentation()
        .uiAutomation
        .executeShellCommand(command)
        .use { descriptor ->
            android.os.ParcelFileDescriptor.AutoCloseInputStream(descriptor)
                .bufferedReader()
                .readText()
        }

    /**
     * 读自 [sinceEpochMs] 起的 logcat 行（`-v epoch`，行首为 epoch 秒）。
     *
     * 生产端已有按窗口输出的计量（帧节奏、慢循环分解），用例要断言它们就必须能
     * 按时间切片取回来；`logcat -T` 让切片在设备侧完成，避免把整份缓冲搬进进程。
     */
    fun logcatSince(sinceEpochMs: Long): List<String> {
        val sinceSeconds = String.format(java.util.Locale.US, "%.3f", sinceEpochMs / 1000.0)
        return try {
            InstrumentationRegistry.getInstrumentation()
                .uiAutomation
                .executeShellCommand("logcat -d -v epoch -T $sinceSeconds")
                // 读取必须在 use 块内完成：块外再读同一 fd 只会拿到 EBADF。
                .use { descriptor ->
                    android.os.ParcelFileDescriptor.AutoCloseInputStream(descriptor)
                        .bufferedReader()
                        .readLines()
                }
        } catch (error: Exception) {
            throw AssertionError("读取 logcat 失败（since=$sinceSeconds）：$error", error)
        }
    }

    /**
     * 生产端 `loop timing window` 一行里的计量（渲染循环整循环的节奏）。
     *
     * 该行由 `TerminalRuntime` 每 60 帧输出一次，覆盖「门控→推状态→渲染→
     * pollAll→尾部队列」整循环——这是唯一能量到整循环的计量：主线程的
     * `OnFrameMetricsAvailableListener` 在终端内容直写 Surface 时采不到帧，
     * `PixelCopy` 读回单次要 ~300ms，都量不了这个循环。
     */
    data class LoopTimingWindow(
        val epochMs: Long,
        val frames: Int,
        val avgMs: Double,
        val p95Ms: Double,
        val maxMs: Double,
        val preMaxMs: Double,
        val fps: Int,
    ) {
        /** 窗口的采样区间是否与 [fromMs]..[toMs] 有交集（区间由自身计量反推）。 */
        fun covers(fromMs: Long, toMs: Long): Boolean {
            val startMs = epochMs - (frames * avgMs).toLong()
            return startMs <= toMs && epochMs >= fromMs
        }

        companion object {
            private val pattern =
                Regex(
                    """^(\d+)\.\d+\s+\d+\s+\d+\s+I Runtime\s*:\s*session \d+ loop timing window \((\d+) frames\): """ +
                        """avg=([\d.]+)ms p95=([\d.]+)ms max=([\d.]+)ms pre_max=([\d.]+)ms ≈(\d+)fps""",
                )

            fun parse(line: String): LoopTimingWindow? {
                val match = pattern.find(line.trim()) ?: return null
                return LoopTimingWindow(
                    epochMs = match.groupValues[1].toLong() * 1000,
                    frames = match.groupValues[2].toInt(),
                    avgMs = match.groupValues[3].toDouble(),
                    p95Ms = match.groupValues[4].toDouble(),
                    maxMs = match.groupValues[5].toDouble(),
                    preMaxMs = match.groupValues[6].toDouble(),
                    fps = match.groupValues[7].toInt(),
                )
            }
        }
    }

    /**
     * 终端 Surface 视图：终端内容由原生渲染线程直绘 SurfaceView，不经过主线程
     * ViewRootImpl 遍历，系统截图拍不到这一层。
     */
    fun terminalSurface(activity: android.app.Activity): android.view.View? {
        val content = activity.findViewById<android.view.ViewGroup>(android.R.id.content)
        return findSurfaceInViewTree(content)
    }

    private fun findSurfaceInViewTree(group: android.view.ViewGroup): android.view.View? {
        for (index in 0 until group.childCount) {
            val child = group.getChildAt(index)
            if (child is terminal.emulator.ui.TerminalSurface) return child
            if (child is android.view.ViewGroup) {
                findSurfaceInViewTree(child)?.let { return it }
            }
        }
        return null
    }

    /**
     * 用 PixelCopy 读回 Surface 当前呈现的像素。
     *
     * 必须用 PixelCopy 而非 `UiAutomation.takeScreenshot()`：终端内容由原生渲染线程
     * 直接画进 SurfaceView 的 Surface，系统截图只拍到应用 UI 层（实测系统截图里
     * 没有图像，Surface 里有）。
     */
    fun captureSurface(view: android.view.View): Bitmap? {
        val surfaceView = view as? android.view.SurfaceView ?: return null
        if (surfaceView.width <= 0 || surfaceView.height <= 0) return null
        val bitmap = Bitmap.createBitmap(surfaceView.width, surfaceView.height, Bitmap.Config.ARGB_8888)
        val latch = java.util.concurrent.CountDownLatch(1)
        var copied = false
        android.view.PixelCopy.request(
            surfaceView,
            bitmap,
            { result: Int ->
                copied = result == android.view.PixelCopy.SUCCESS
                latch.countDown()
            },
            android.os.Handler(android.os.Looper.getMainLooper()),
        )
        latch.await(PIXEL_COPY_TIMEOUT_MS, java.util.concurrent.TimeUnit.MILLISECONDS)
        return if (copied) bitmap else null
    }

    /** 一次颜色扫描的结果：相符像素数与其包围盒。 */
    data class ColorMatch(val count: Int, val bounds: Rect?)

    /**
     * 扫描整幅图，统计与 [color] 相符（每通道差 ≤ [tolerance]）的像素。
     *
     * 按通道差判定而非精确相等：抗锯齿边缘与合成器色彩转换都会带来少量偏差。
     */
    fun matchColor(bitmap: Bitmap, color: Int, tolerance: Int = 24): ColorMatch {
        val targetRed = android.graphics.Color.red(color)
        val targetGreen = android.graphics.Color.green(color)
        val targetBlue = android.graphics.Color.blue(color)
        val width = bitmap.width
        val pixels = IntArray(width * bitmap.height)
        bitmap.getPixels(pixels, 0, width, 0, 0, width, bitmap.height)
        var count = 0
        var minX = Int.MAX_VALUE
        var minY = Int.MAX_VALUE
        var maxX = Int.MIN_VALUE
        var maxY = Int.MIN_VALUE
        for (index in pixels.indices) {
            val pixel = pixels[index]
            if (
                kotlin.math.abs(android.graphics.Color.red(pixel) - targetRed) <= tolerance &&
                kotlin.math.abs(android.graphics.Color.green(pixel) - targetGreen) <= tolerance &&
                kotlin.math.abs(android.graphics.Color.blue(pixel) - targetBlue) <= tolerance
            ) {
                count++
                val x = index % width
                val y = index / width
                if (x < minX) minX = x
                if (x > maxX) maxX = x
                if (y < minY) minY = y
                if (y > maxY) maxY = y
            }
        }
        if (count == 0) return ColorMatch(0, null)
        return ColorMatch(count, Rect(minX, minY, maxX + 1, maxY + 1))
    }
}

// ── 像素判读 ─────────────────────────────────────────

/**
 * 色相判定：某通道显著高于其余两通道即计入该色相计数。
 *
 * 阈值同时挡住抗锯齿边缘与主题自身的强调色；三个色相共用同一判据，
 * 免得「红按红判、绿按绿判」各自漂移成不可比的数。
 */
private fun countHuePixels(shot: Bitmap, hue: (red: Int, green: Int, blue: Int) -> Boolean): Int {
    var count = 0
    // 步长 2 采样：步长 3 在阈值边缘会漏掉真实字形，背景纯色区加密后仍为 0。
    for (y in 0 until shot.height step 2) {
        for (x in 0 until shot.width step 2) {
            val pixel = shot.getPixel(x, y)
            if (hue(
                    android.graphics.Color.red(pixel),
                    android.graphics.Color.green(pixel),
                    android.graphics.Color.blue(pixel),
                )
            ) {
                count++
            }
        }
    }
    return count
}

fun countReddishPixels(shot: Bitmap): Int = countHuePixels(shot) { red, green, blue ->
    red > 110 && red - blue > 50 && red - green > 30
}

fun countGreenishPixels(shot: Bitmap): Int = countHuePixels(shot) { red, green, blue ->
    green > 110 && green - red > 50 && green - blue > 30
}

fun countBluishPixels(shot: Bitmap): Int = countHuePixels(shot) { red, green, blue ->
    blue > 110 && blue - red > 50 && blue - green > 30
}

/** 单像素三通道差和（阈值由调用方判定）。 */
fun pixelChannelDelta(first: Int, second: Int): Int =
    kotlin.math.abs(android.graphics.Color.red(first) - android.graphics.Color.red(second)) +
        kotlin.math.abs(android.graphics.Color.green(first) - android.graphics.Color.green(second)) +
        kotlin.math.abs(android.graphics.Color.blue(first) - android.graphics.Color.blue(second))

/** 屏幕像素的感知亮度（0..255）；坐标越界返回 -1。 */
fun pixelLuminance(shot: Bitmap, x: Int, y: Int): Int {
    if (x < 0 || x >= shot.width || y < 0 || y >= shot.height) return -1
    val pixel = shot.getPixel(x, y)
    return (
        (pixel shr 16 and 0xFF) * 299 +
            (pixel shr 8 and 0xFF) * 587 +
            (pixel and 0xFF) * 114
        ) / 1000
}

// ── 网格定位写入 ─────────────────────────────────────

/**
 * 视口归位到底部，并等到**会话真的接受了这次归位**。
 *
 * 屏幕行号与绝对网格行的换算 `视口行 = 绝对行 - (回滚长度 - 偏移)` 只在偏移为 0 时
 * 成立；共用会话里任何翻阅/搜索留下的非零偏移都让按行号算出的点击/长按落到另一行——
 * 落到空行即退化为仅粘贴菜单，外部表现是「长按未打开选择菜单」，与长按本身无关。
 *
 * 判据取 `sessionScrollOffset() == 0`：那是 VT 侧**已应用**的偏移。
 * 两个被否决的旧判据，都是把「不可见的中间态」当成了真值：
 *  - `activeSessionScrollOffset()` 是渲染线程尚未消费的命令槽，实测 CI 恒读 943；
 *  - 向网格注入 `BOTTOM-<nanos>` 标记行虽然能证明视口在底部，却往用户屏幕里写垃圾，
 *    并把真实内容一行行挤出视口——回滚为 0 的会话里，连 shell 提示符都会被挤丢，
 *    后续用例等提示符只能超时（跨类跑时实测「shell prompt 未出现」）。
 */
fun scrollViewportToBottom(bridge: Bridge, runtime: TerminalRuntime, timeoutMs: Long = 5_000) {
    runtime.setScrollOffset(0)
    val atBottom =
        UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 50) {
            bridge.sessionScrollOffset() == 0
        }
    assertNotNull("视口必须归位到底部（会话滚动偏移未回到 0）", atBottom)
}

/**
 * 等 shell 提示符出现；等不到就先向 PTY 要一个新的。
 *
 * 直接轮询全量文本有个隐藏前提：提示符**还在网格里**。共用会话跑多个用例时，前一个
 * 用例的整屏输出可以把那一行挤出网格；回滚为空时会话里连痕迹都不剩，于是后续用例
 * 白等 30s 后报「shell prompt 未出现」——这与被测行为无关，纯粹是把上一用例的残留
 * 当成了本用例的前提。空行的 Enter 一定会让 shell 重画提示符，故先给一次机会，
 * 只在真的等不到时才发（常见路径不额外多出一行提示符）。
 */
fun awaitShellPrompt(bridge: Bridge, timeoutMs: Long = 30_000) {
    val seen = {
        UxTestUtils.pollUntilTrue(timeoutMs = minOf(timeoutMs, 3_000), intervalMs = 100) {
            bridge.getTerminalText()?.let { it.contains("$") || it.contains("#") } == true
        }
    }
    if (seen() != null) return
    bridge.writeToPty("\n".toByteArray(Charsets.UTF_8))
    assertNotNull(
        "shell prompt 未出现",
        UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 100) {
            bridge.getTerminalText()?.let { it.contains("$") || it.contains("#") } == true
        },
    )
}

/** SGR 转义序列：`ESC [ … m`。不占列，故落格判据须先剥除。 */
private val SGR_SEQUENCE = Regex("\u001B\\[[0-9;]*m")

/**
 * 让共享 shell 回到可预期状态：结束占着 tty 的前台进程、恢复 tty 回显、关掉全部鼠标上报模式。
 *
 * 共享会话是所有仪器测试的公共资源，而**命令回显**正是它的状态之一：任何用例把
 * tty 改成 `-echo`，都会**静默**污染后续用例——依赖「命令回显出现」的用例会一直
 * 等到超时。本仓踩过这个坑：鼠标用例用一次写入同时送 ETX 与复位命令，而 tty 处于
 * `-icanon` 时前台 `cat` 一次 read 就把两者一起读走（非行缓冲），命令就此丢失——
 * 鼠标上报没关、`stty sane` 没执行，回显再没打开，后续用例以「命令回显未出现」判红。
 *
 * 三步必须**分次**写：
 * 1. ETX 单独一次：结束可能占着 tty 的前台进程（`cat` 之类）；
 * 2. 等 shell 真的能执行命令——判据取命令的**输出**而非回显，因为此刻回显可能仍是关的；
 * 3. `stty sane` 恢复 tty，并关掉全部鼠标上报模式（终端级模式，`stty` 管不着）。
 */
fun Bridge.restoreSharedShellState(timeoutMs: Long = 30_000) {
    val nonce = "restore-${System.nanoTime()}"
    val deadline = System.nanoTime() + timeoutMs * 1_000_000
    var responsive = false
    while (System.nanoTime() < deadline && !responsive) {
        writeToPty("\u0003".toByteArray(Charsets.UTF_8))
        writeToPty("printf 'RESTORE_%s\\n' $nonce\n".toByteArray(Charsets.UTF_8))
        responsive =
            UxTestUtils.pollUntilTrue(timeoutMs = 5_000, intervalMs = 200) {
                getTerminalText()?.contains(nonce) == true
            } != null
    }
    assertNotNull("共享 shell 无法恢复可交互（前台进程未退出？）", responsive)
    writeToPty(
        (
            "stty sane; printf '\\033[?1000l\\033[?1002l\\033[?1003l" +
                "\\033[?1005l\\033[?1015l\\033[?1006l'\n"
            ).toByteArray(Charsets.UTF_8),
    )
    // 鼠标上报是终端级状态：模式关掉后镜像必须回落，否则滚动手势会一直被转发给远端。
    assertNotNull(
        "DECRST 之后鼠标上报镜像必须回落",
        UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 200) {
            !isMouseTrackingActive()
        },
    )
}

/**
 * 视口 [row] 行首列起写入 [text] 并轮询至**渲染光标**落在该行文本末列。
 *
 * 落格判据取渲染光标而非文本包含：光标位置由 VT 产出、与单元格内容同源，
 * 文本包含则在标记被换行截断时同样成立。写入前只把光标归位、**不清屏**：
 * 清屏会抹掉同一次调用序列里先前写下的标记（实测三色只剩最后一个，红/绿像素
 * 恒为 0、蓝独活）；光标归位已足够让每轮轮询从可区分的状态出发。
 * SGR 不占列，期望列按剥除转义后的可见长度计。
 */
fun placeTextAtRow(bridge: Bridge, row: Int, text: String, timeoutMs: Long = 15_000) {
    val expectedColumn = SGR_SEQUENCE.replace(text, "").length
    val homed = bridge.feedTerminal("\u001B[1;1H".toByteArray(Charsets.UTF_8))
    assertTrue("光标归位送显失败", homed)
    val fed = bridge.feedTerminal("\u001B[${row + 1};1H$text".toByteArray(Charsets.UTF_8))
    assertTrue("标记送显失败", fed)
    val landed = UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 50) {
        val packed = bridge.cursorViewportPacked()
        packed >= 0 &&
            (packed shr 32).toInt() == row &&
            (packed and 0xffffffffL).toInt() == expectedColumn
    }
    // 断言信息不得含裸控制字符：ESC 会让 JUnit XML 报告无法解析，失败详情随之整体丢失。
    assertNotNull(
        "标记必须落格于 ($row,$expectedColumn): [${SGR_SEQUENCE.replace(text, "")}]",
        landed,
    )
}

/** Minimal int rect used by [UxTestUtils.changedBoundingBox]. */
class Rect(val left: Int, val top: Int, val right: Int, val bottom: Int) {
    val width: Int
        get() = right - left + 1

    val height: Int
        get() = bottom - top + 1

    override fun toString(): String = "Rect(l=$left,t=$top,r=$right,b=$bottom ${width}x$height)"
}
