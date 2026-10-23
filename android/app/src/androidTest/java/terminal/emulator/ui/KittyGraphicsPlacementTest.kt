package terminal.emulator.ui

import android.graphics.Bitmap
import android.graphics.Rect
import android.util.Base64
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.bridge.Bridge
import terminal.emulator.waitForSession
import java.io.ByteArrayOutputStream
import java.io.File

/**
 * Kitty 图像放置：图像必须跟着内容走，Unicode 占位符必须真的画出图像。
 *
 * ## 曾经的缺陷
 *
 * 1. 用户报障（kitten icat）：「图像部分位置不会自动移动（手势滚动无问题），后面
 *    显示的内容会逐渐被图像覆盖」。图像钉在屏幕上的固定位置，文本从它下面滚过去。
 *    根因：渲染线程的放置缓存键漏了「回滚长度」——新输出把行推入回滚时放置的视口
 *    行号随之变化，而用户滚动偏移仍为 0，缓存不失效 → 放置不重查 → 图像不移动。
 * 2. 用户报障（yazi）：「无法显示图像，实际为数个方框叉号」。Unicode 占位符
 *    （U=1）路径没有把占位单元画成图像。
 *
 * ## 量具
 *
 * 真实呈现像素（PixelCopy 读回 Surface），不是内部状态：
 *
 * 1. 图像上屏后记下包围盒；内容把视口向上滚后，包围盒必须同步上移同样的
 *    行数（以渲染帧的回滚长度为口径）——不移动即失败（缺陷 1 的判据）。
 * 2. Unicode 占位符：占位单元处必须出现图像像素，且落在该单元的格子里
 *    （缺陷 2 的判据）。
 */
class KittyGraphicsPlacementTest : TerminalLogcatTest() {
    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private companion object {
        /** 注入图像的边长（像素）。 */
        const val IMAGE_SIZE = 64

        /** 普通放置的图像颜色（蓝）：与终端底色差异极大，便于按颜色定位。 */
        const val IMAGE_COLOR = 0xFF2878DC.toInt()

        /** Unicode 占位符放置的图像颜色（绿）：与普通放置区分开。 */
        const val PLACEHOLDER_COLOR = 0xFF18A558.toInt()

        /** 图像上屏的等待上限（毫秒）。 */
        const val ON_SCREEN_TIMEOUT_MS = 15_000L

        /** 视口向上滚动的行数。 */
        const val SCROLL_ROWS = 3

        /** 普通放置的锚点行（1 基）。 */
        const val IMAGE_ROW = 8

        /** 占位符所在行（1 基）与列（1 基）。 */
        const val PLACEHOLDER_ROW = 6
        const val PLACEHOLDER_COL = 3

        /** 位置判定的容差（像素）：抗锯齿边缘与像素取整。 */
        const val POSITION_TOLERANCE_PX = 4
    }

    @Before
    fun setUp() {
        composeTestRule.waitForSession()
    }

    /**
     * 图像必须跟着内容上移：内容把视口向上滚后，图像的包围盒同步上移同样的行数。
     *
     * 判别力：放置缓存漏掉「回滚长度」时缓存不失效，图像钉在原处不动，后续文本
     * 从它下面滚过去（用户实测：内容被图像覆盖）。
     */
    @Test
    fun kitty_image_moves_up_with_scrolled_content() {
        val bridge = composeTestRule.awaitBridge()
        val cellHeight = cellSize(bridge).second
        // 脚本自己清屏并把图像放到已知行：放置锚点由光标位置决定。
        val script = imageScript(imageId = 21, row = IMAGE_ROW, color = IMAGE_COLOR)
        bridge.writeToPty("sh ${script.absolutePath}\n".toByteArray(Charsets.US_ASCII))

        val before = awaitSettledImage(bridge, IMAGE_COLOR, minRenderedScrollback = 0)
        assertNotNull(
            "kitty 图像没有在 ${ON_SCREEN_TIMEOUT_MS}ms 内上屏\n" +
                "终端文本：\n${bridge.getTerminalText().orEmpty().takeLast(600)}",
            before,
        )
        val sampleBefore = checkNotNull(before)

        // 让内容把视口向上滚：光标移到末行后逐行换行，每行一次上滚。
        val rows = (bridge.getGridRowsColsPacked() shr 32).toInt()
        assertTrue("视口行数不足以验证滚动：$rows", rows > IMAGE_ROW + SCROLL_ROWS)
        // 直接喂 VT 解析器而不经 shell：shell 的命令回显与提示符会额外占行，
        // 滚动量就不再是常数（实测多滚一行），断言只能对着实测滚动量。
        assertTrue(
            "喂入滚动序列失败",
            bridge.feedTerminal(scrollSequence(rows, SCROLL_ROWS).toByteArray(Charsets.US_ASCII)),
        )

        // 滚动必须真的发生：渲染帧里的回滚长度增长就是证据（它与屏幕像素同源）。
        val after = awaitSettledImage(
            bridge,
            IMAGE_COLOR,
            minRenderedScrollback = sampleBefore.renderedScrollback + SCROLL_ROWS,
        )
        assertNotNull(
            "视口没有上滚（渲染帧回滚长度 ${bridge.renderedScrollbackLength()}，" +
                "滚动前 ${sampleBefore.renderedScrollback}）",
            after,
        )
        val sampleAfter = checkNotNull(after)
        val scrolledRows = sampleAfter.renderedScrollback - sampleBefore.renderedScrollback
        UxTestUtils.metric("kitty_image_scrolled_rows", scrolledRows.toLong())

        // 滚动后的图像位置：必须上移同样的行数，且图像本身仍然完整可见（没有被文本盖住）。
        val expectedShift = scrolledRows * cellHeight
        val actualShift = sampleBefore.bounds.top - sampleAfter.bounds.top
        UxTestUtils.metric("kitty_image_scroll_shift_px", actualShift.toLong())
        assertTrue(
            "图像必须随内容上移 ${expectedShift}px（实际 ${actualShift}px）：" +
                "放置缓存漏掉回滚长度时图像会钉在原处，后续内容被它覆盖",
            kotlin.math.abs(actualShift - expectedShift) <= cellHeight / 2,
        )
        UxTestUtils.metric("kitty_image_scroll_pixels", sampleAfter.pixels.toLong())
        assertTrue(
            "滚动后图像像素数应保持不变（此前 ${sampleBefore.pixels}，" +
                "此后 ${sampleAfter.pixels}）：图像被文本覆盖说明它没有跟着内容走",
            kotlin.math.abs(sampleAfter.pixels - sampleBefore.pixels) <= sampleBefore.pixels / 4,
        )
    }

    /**
     * Unicode 占位符（U=1）必须把占位单元画成图像，而不是占位字符本身。
     *
     * 判别力：yazi 走的就是这条路径；不实现时占位单元显示为缺字方框/叉号
     * （用户实测：数个方框叉号）。
     */
    @Test
    fun unicode_placeholder_cell_is_drawn_as_image() {
        val bridge = composeTestRule.awaitBridge()
        val (cellWidth, cellHeight) = cellSize(bridge)
        val script = placeholderScript(imageId = 22, row = PLACEHOLDER_ROW, col = PLACEHOLDER_COL)
        bridge.writeToPty("sh ${script.absolutePath}\n".toByteArray(Charsets.US_ASCII))

        val shot = awaitSettledImage(bridge, PLACEHOLDER_COLOR, minRenderedScrollback = 0)
        assertNotNull(
            "Unicode 占位符图像没有上屏（占位单元仍显示为方框/叉号）\n" +
                "终端文本：\n${bridge.getTerminalText().orEmpty().takeLast(600)}",
            shot,
        )
        val sample = checkNotNull(shot)
        val bounds = sample.bounds
        UxTestUtils.metric("kitty_placeholder_pixels", sample.pixels.toLong())

        // 图像必须落在占位单元的格子里：左上角 ≈ (列-1)×单元格宽, (行-1)×单元格高。
        val expectedLeft = (PLACEHOLDER_COL - 1) * cellWidth
        val expectedTop = (PLACEHOLDER_ROW - 1) * cellHeight
        assertTrue(
            "占位符图像必须画在占位单元处（期望左上 ≈ ${expectedLeft.toInt()},${expectedTop.toInt()}，" +
                "实际 ${bounds.left},${bounds.top}）",
            kotlin.math.abs(bounds.left - expectedLeft) <= POSITION_TOLERANCE_PX &&
                kotlin.math.abs(bounds.top - expectedTop) <= POSITION_TOLERANCE_PX,
        )
        // 按像素尺寸绘制：包围盒宽高都应等于图像边长。
        assertTrue(
            "占位符图像宽度必须是 ${IMAGE_SIZE}px，实际 ${bounds.width()}px",
            kotlin.math.abs(bounds.width() - IMAGE_SIZE) <= POSITION_TOLERANCE_PX,
        )
        assertTrue(
            "占位符图像高度必须是 ${IMAGE_SIZE}px，实际 ${bounds.height()}px",
            kotlin.math.abs(bounds.height() - IMAGE_SIZE) <= POSITION_TOLERANCE_PX,
        )
    }

    /**
     * 网格单元格尺寸（设备像素）：渲染与放置几何都以它为口径。
     *
     * 字体管线就绪前拿不到（上报随渲染器初始化发生），故轮询等待。
     */
    private fun cellSize(bridge: Bridge): Pair<Float, Float> {
        val ready =
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                bridge.getFontInfo() != null
            }
        assertNotNull("字体信息未就绪（拿不到单元格尺寸）", ready)
        val info = checkNotNull(bridge.getFontInfo())
        return info.cellWidthPx to info.cellHeightPx
    }

    /**
     * 一帧的取样：图像包围盒、图像像素数与该帧的回滚长度。
     *
     * 回滚长度取自渲染帧（与屏幕像素同源）：它就是「这一帧里内容往上滚了多少行」，
     * 用它算出的期望位移与像素位移同口径——不拿 VT 线程的当前值去比像素，
     * 两者可能不在同一帧。
     */
    private data class ImageSample(val bounds: Rect, val pixels: Int, val renderedScrollback: Int)

    /**
     * 等图像上屏且位置稳定，返回一帧的取样；超时返回 null。
     *
     * 连续两次取样（包围盒与回滚长度）都相同才算稳定：渲染线程可能还没追上
     * 最新的回滚长度，此时截到的仍是上一帧。
     *
     * 截图很慢（软件渲染设备上整屏合成一次要几百毫秒），故按固定节奏取，
     * 不用它做高频轮询——否则量具本身就把渲染循环按住了。
     */
    private fun awaitSettledImage(bridge: Bridge, color: Int, minRenderedScrollback: Int): ImageSample? {
        val surface = UxTestUtils.terminalSurface(composeTestRule.activity) ?: return null
        var lastBounds: Rect? = null
        var lastScrollback = Int.MIN_VALUE
        var settled: ImageSample? = null
        UxTestUtils.pollUntilTrue(timeoutMs = ON_SCREEN_TIMEOUT_MS, intervalMs = 300) {
            val rendered = bridge.renderedScrollbackLength()
            val captured = UxTestUtils.captureSurface(surface)
            val sample =
                captured?.let { shot ->
                    val match = UxTestUtils.matchColor(shot, color)
                    match.bounds?.let { ImageSample(it, match.count, rendered) }
                }
            if (sample != null && sample.bounds == lastBounds && rendered == lastScrollback) {
                settled = sample
                true
            } else {
                lastBounds = sample?.bounds
                lastScrollback = rendered
                false
            }
        }
        return settled
    }

    /**
     * 把视口向上滚 [scrollRows] 行的 VT 序列。
     *
     * 光标移到末行（[rows]）后逐行换行：末行换行即上滚一次，故 N 次换行
     * ＝N 行上滚。序列直接喂给 VT 解析器（[Bridge.feedTerminal]）而不经 shell：
     * shell 的命令回显与提示符会额外占行，滚动量就不再是常数。
     */
    private fun scrollSequence(rows: Int, scrollRows: Int): String = "\u001B[" + rows + ";1H" + "\n".repeat(scrollRows)

    /**
     * 写出打印 kitty 传输序列的脚本，返回脚本文件。
     *
     * 单引号里的 `\\033` 由 printf 解释成 ESC；末尾 `\\033\\\\` 产出 ESC + 反斜杠
     * （kitty 协议的 APC 终止符）。
     */
    private fun imageScript(imageId: Int, row: Int, color: Int): File {
        val payload = base64Png(color)
        return writeScript(
            "kitty-image-$imageId.sh",
            "#!/system/bin/sh\n" +
                "printf '\\033[2J\\033[H\\033[$row;1H" +
                "\\033_Ga=T,f=100,t=d,i=$imageId,q=1;$payload\\033\\\\'\n",
        )
    }

    /**
     * 写出 Unicode 占位符（U=1）的脚本：先传图像，再写占位单元。
     *
     * 占位单元的编码按 kitty 协议：前景色编码图像 id（RGB 24 位），下划线色编码
     * 放置 id，组合记号 U+0305 标记占位起点。占位字符 U+10EEEE 是代理对。
     */
    private fun placeholderScript(imageId: Int, row: Int, col: Int): File {
        val payload = base64Png(PLACEHOLDER_COLOR)
        val placeholder = "\uDBFB\uDEEE\u0305\u0305"
        return writeScript(
            "kitty-placeholder-$imageId.sh",
            "#!/system/bin/sh\n" +
                "printf '\\033[2J\\033[H" +
                "\\033_Ga=T,f=100,t=d,i=$imageId,U=1,p=1,q=1;$payload\\033\\\\'\n" +
                "printf '\\033[$row;${col}H\\033[38;2;0;0;${imageId}m\\033[58;2;0;0;1m$placeholder'\n",
        )
    }

    /** 纯色 PNG 的 base64 载荷（kitty 直接传输通道）。 */
    private fun base64Png(color: Int): String {
        val bitmap = Bitmap.createBitmap(IMAGE_SIZE, IMAGE_SIZE, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(color)
        val stream = ByteArrayOutputStream()
        check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream)) { "PNG 编码失败" }
        bitmap.recycle()
        return Base64.encodeToString(stream.toByteArray(), Base64.NO_WRAP)
    }

    /** 脚本写进应用 home 目录（真实 shell 才能执行它）。 */
    private fun writeScript(name: String, content: String): File {
        val home = File(InstrumentationRegistry.getInstrumentation().targetContext.filesDir, "home")
        check(home.isDirectory || home.mkdirs()) { "无法创建 $home" }
        val script = File(home, name)
        script.writeText(content)
        return script
    }
}
