package terminal.emulator.ui

import android.graphics.Bitmap
import android.graphics.Color
import android.util.Base64
import android.view.PixelCopy
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.TerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.awaitBridge
import terminal.emulator.waitForSession
import java.io.ByteArrayOutputStream
import java.io.File

/**
 * Kitty 图像叠加层：既要真的上屏，又不得把渲染循环拖垮。
 *
 * ## 曾经的缺陷
 *
 * 渲染门控把「本帧存在 kitty 图像」当成必须整帧重绘的理由（叠加层被当作
 * 全屏覆盖物）。于是**一张静止的图片**就让每一帧都走全量重绘：SwiftShader
 * 模拟器上全量帧约 500ms，终端直接掉到 1-2fps。yazi 这类全屏 TUI 更是
 * 几乎不可用——用户报障的「kitty 图像不工作」正是这个现象：图像要么迟迟
 * 不出现，要么出现后终端整体僵住。
 *
 * ## 现在的语义
 *
 * kitty 图像是独立的图集叠加层，每帧整层重画在累加器之上，文本层照旧走
 * 脏带增量。**只有叠加层内容发生变化的那一帧**（新增/替换/删除图像）才需要
 * 整帧重绘——上一帧的图像像素还留在累加器里，不清就会留下残影。
 *
 * ## 量具
 *
 * 1. 图像上屏：把 kitty 传输序列经 **PTY** 送出（与真实程序同一条路径——
 *    VT 线程收到 Write 才会推送带 kitty 生成戳的帧），随后屏幕必须出现
 *    那块指定颜色。量的是**真实呈现像素**，不是内部状态。
 * 2. 循环节奏：沿用生产代码自己的 `loop timing window` 计量（覆盖
 *    「门控→推状态→渲染→pollAll→尾部队列」整循环），断言帧率不低于
 *    [MIN_FPS]。缺陷版本在此处是 1-2fps。
 */
class KittyGraphicsOverlayTest : TerminalLogcatTest() {
    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private companion object {
        /** 注入图像的边长（像素）：够小以便快速上屏，又够大以便在截图里定位。 */
        const val IMAGE_SIZE = 64

        /**
         * 图像颜色（ARGB）：与终端底色差异极大，便于在截图里按颜色定位。
         *
         * 必须是**不透明**的颜色：alpha 为 0 的位图经 PNG 编码后是完全透明的，
         * 画到屏幕上什么也看不见——测试会一直报「图像没有上屏」，而终端侧一切正常。
         */
        const val IMAGE_COLOR = 0xFF2878DC.toInt()

        /** 图像上屏的等待上限（毫秒）。 */
        const val ON_SCREEN_TIMEOUT_MS = 15_000L

        /** 帧率下限：缺陷版本是 1-2fps；本模拟器的全量帧本身就要 ~500ms，
         * 故这个下限只用来区分「全量帧钉死」与「增量帧正常」。 */
        const val MIN_FPS = 20

        /** 计量窗口的等待上限（毫秒）。 */
        const val WINDOW_TIMEOUT_MS = 20_000L

        /**
         * 帧率计量前先让终端持续输出这么多行，使渲染循环处在持续工作状态。
         *
         * 取 8 行：从屏幕顶部往下写 16 行（含命令回显）仍在一屏之内，不会触发
         * 滚动强制的整帧重绘。
         */
        const val BUSY_LINES = 8

        /** PixelCopy 单次读回的上限（毫秒）：软件渲染设备上读一屏要几百毫秒。 */
        const val PIXEL_COPY_TIMEOUT_MS = 5_000L
    }

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        // 就绪门槛必须齐三样：bootstrap（真实 shell 才能 printf 出 kitty 序列）、
        // 通知权限（首次运行的权限对话框会让 Activity 进入 PAUSED，渲染随之暂停，
        // 图像永远上不了屏）、以及界面组合完成。
        composeTestRule.waitForSession()
    }

    @Test
    fun kitty_overlay_is_drawn_and_keeps_the_render_loop_smooth() {
        val bridge = composeTestRule.awaitBridge()

        // 1) 经 PTY 送出 kitty 传输序列（与真实程序同一条路径）。
        //    序列写进脚本再执行：命令本身有几百字符，直接在命令行里发会被回显
        //    折成多行、把图像推出视口（放置随内容滚动，滚出视口即不可见）。
        val script = kittyProbeScript()
        bridge.writeToPty("sh ${script.absolutePath}\n".toByteArray(Charsets.US_ASCII))

        // 2) 图像必须真的上屏（呈现像素，不是内部状态）。
        //    截图很慢（整屏合成一次要几百毫秒），故只按固定节奏取，
        //    不用它做高频轮询——否则量具本身就把渲染循环按住了。
        var pixels = 0
        var shotDump: String? = null
        val onScreen =
            UxTestUtils.pollUntilTrue(timeoutMs = ON_SCREEN_TIMEOUT_MS, intervalMs = 500) {
                val shot = captureSurface()
                if (shot == null) {
                    shotDump = "未能从 Surface 读回像素"
                    false
                } else {
                    pixels = countImagePixels(shot)
                    if (pixels == 0) {
                        shotDump = "Surface 读回 ${shot.width}x${shot.height}，颜色数=${distinctColors(shot)}"
                    }
                    pixels > 0
                }
            }
        assertNotNull(
            "kitty 图像没有在 ${ON_SCREEN_TIMEOUT_MS}ms 内上屏（传输序列被终端接受但没有绘制）\n" +
                "终端文本：\n${bridge.getTerminalText().orEmpty().takeLast(600)}\n" +
                "截图：$shotDump",
            onScreen,
        )
        UxTestUtils.metric("kitty_overlay_pixels", pixels.toLong())

        // 3) 叠加层存在时，渲染循环必须仍然是增量的。
        //
        //    两条前提，缺一不可：
        //    - 终端要**持续输出**，否则循环停在 500ms 的空闲闭锁上，量到的是
        //      驻留节拍而非渲染能力；
        //    - 输出**不能把屏幕滚出**（滚动强制整帧重绘，设备速度就成了唯一
        //      变量，测不出「叠加层是否额外拖累了每帧」）。故先把光标送回
        //      原点，让后续输出从顶部往下长。
        bridge.writeToPty("printf '\\033[H'\n".toByteArray(Charsets.US_ASCII))
        Thread.sleep(200)
        val startMs = System.currentTimeMillis()
        repeat(BUSY_LINES) { index ->
            bridge.writeToPty("printf 'kitty-overlay-line-$index\\n'\n".toByteArray())
            Thread.sleep(30)
        }
        val endMs = System.currentTimeMillis()

        var measured: UxTestUtils.LoopTimingWindow? = null
        val found =
            UxTestUtils.pollUntilTrue(timeoutMs = WINDOW_TIMEOUT_MS, intervalMs = 500) {
                measured =
                    UxTestUtils.logcatSince(startMs)
                        .mapNotNull { UxTestUtils.LoopTimingWindow.parse(it) }
                        .firstOrNull { it.covers(startMs, endMs) }
                measured != null
            }
        assertNotNull(
            "等待 ${WINDOW_TIMEOUT_MS}ms 仍没有一条帧节奏窗口覆盖采样期——仪器失效，不能当作通过",
            found,
        )
        val window = requireNotNull(measured) { "帧节奏窗口缺失（仪器失效）" }
        UxTestUtils.metric("kitty_overlay_fps", window.fps.toLong())
        UxTestUtils.metric("kitty_overlay_p95_ms", window.p95Ms.toLong())
        assertTrue(
            "kitty 叠加层在场时渲染循环只有 ${window.fps}fps（p95=${window.p95Ms}ms）：" +
                "叠加层又把终端钉回了全量帧",
            window.fps >= MIN_FPS,
        )
    }

    /**
     * 读回终端 Surface 已呈现的像素。
     *
     * 必须用 `PixelCopy` 而非 `UiAutomation.takeScreenshot()`：终端内容由原生
     * 渲染线程直接画进 SurfaceView 的 Surface，不经过主线程 `ViewRootImpl` 遍历，
     * 系统截图只拍到应用 UI 层，拍不到这一层（实测：系统截图里没有图像，
     * Surface 里有）。PixelCopy 直接向合成器要该 Surface 的当前内容。
     */
    private fun captureSurface(): Bitmap? {
        val view = surface() as? android.view.SurfaceView ?: return null
        if (view.width <= 0 || view.height <= 0) return null
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        val latch = java.util.concurrent.CountDownLatch(1)
        var copied = false
        PixelCopy.request(
            view,
            bitmap,
            { result: Int ->
                copied = result == PixelCopy.SUCCESS
                latch.countDown()
            },
            android.os.Handler(android.os.Looper.getMainLooper()),
        )
        latch.await(PIXEL_COPY_TIMEOUT_MS, java.util.concurrent.TimeUnit.MILLISECONDS)
        return if (copied) bitmap else null
    }

    /** 终端 Surface 视图。 */
    private fun surface(): android.view.View? {
        val content =
            composeTestRule.activity.findViewById<android.view.ViewGroup>(android.R.id.content)
        return findSurfaceInViewTree(content)
    }

    private fun findSurfaceInViewTree(group: android.view.ViewGroup): android.view.View? {
        for (i in 0 until group.childCount) {
            val child = group.getChildAt(i)
            if (child is terminal.emulator.ui.TerminalSurface) return child
            if (child is android.view.ViewGroup) {
                findSurfaceInViewTree(child)?.let { return it }
            }
        }
        return null
    }

    /** 截图里的颜色数（粗略判空屏）。 */
    private fun distinctColors(bitmap: Bitmap): Int {
        val seen = HashSet<Int>()
        for (y in 0 until bitmap.height step 7) {
            for (x in 0 until bitmap.width step 7) {
                seen.add(bitmap.getPixel(x, y))
            }
        }
        return seen.size
    }

    /**
     * 截图里那块图像的像素数。
     *
     * 按通道差判定（容差 24），避免抗锯齿边缘被漏掉；整屏扫描并跳步，
     * 保证 64x64 的图像块仍能命中上百个采样点。
     */
    private fun countImagePixels(bitmap: Bitmap): Int {
        val target = Color.red(IMAGE_COLOR)
        val targetGreen = Color.green(IMAGE_COLOR)
        val targetBlue = Color.blue(IMAGE_COLOR)
        var count = 0
        for (y in 0 until bitmap.height step 3) {
            for (x in 0 until bitmap.width step 3) {
                val pixel = bitmap.getPixel(x, y)
                if (
                    kotlin.math.abs(Color.red(pixel) - target) <= 24 &&
                    kotlin.math.abs(Color.green(pixel) - targetGreen) <= 24 &&
                    kotlin.math.abs(Color.blue(pixel) - targetBlue) <= 24
                ) {
                    count++
                }
            }
        }
        return count
    }

    /**
     * 写出打印 kitty 传输序列的脚本，返回脚本文件。
     *
     * 单引号里 `\\033` 由 printf 解释成 ESC；末尾 `\\033\\\\` 产出 ESC + 反斜杠
     * （kitty 协议的 APC 终止符）。开头三个换行把光标推到接近屏幕底部，
     * 放置随内容滚动，落在视口之外就看不见了。
     */
    private fun kittyProbeScript(): File {
        val bitmap = Bitmap.createBitmap(IMAGE_SIZE, IMAGE_SIZE, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(IMAGE_COLOR)
        val stream = ByteArrayOutputStream()
        check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream)) { "PNG 编码失败" }
        bitmap.recycle()
        val payload = Base64.encodeToString(stream.toByteArray(), Base64.NO_WRAP)
        val home = File(InstrumentationRegistry.getInstrumentation().targetContext.filesDir, "home")
        check(home.isDirectory || home.mkdirs()) { "无法创建 $home" }
        val script = File(home, "kitty-probe.sh")
        script.writeText("#!/system/bin/sh\nprintf '\\n\\n\\n\\033_Ga=T,f=100,t=d,i=9,q=1;$payload\\033\\\\'\n")
        return script
    }
}
