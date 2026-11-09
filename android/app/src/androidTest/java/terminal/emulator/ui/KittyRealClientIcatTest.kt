package terminal.emulator.ui

import android.graphics.Bitmap
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.activeSessionId
import terminal.emulator.awaitBridge
import terminal.emulator.bridge.Bridge
import terminal.emulator.openDrawer
import terminal.emulator.waitForSession
import java.io.File

/**
 * 真实 kitty 客户端（termux-packages 的 kitty）端到端验证：`kitten icat` 出图、
 * `clear` 清图。
 *
 * 为什么要真实客户端：协议自造的用例（[KittyGraphicsPlacementTest]）只覆盖本仓自己
 * 写出的那几种字节序列，而真实客户端的编码方式不同——PNG 直传（`f=100`）、分块续传
 * （`m=1`）、静默抑制回显（`q=2`）、传输后移动光标。协议解析对不上时，自造用例全绿
 * 而真实客户端照样出不来图。
 *
 * 量具是真实像素：用 PixelCopy 读回 Surface，图像颜色必须出现在屏幕上；
 * `clear` 之后必须消失（用户报障：第一次 `clear` 只清文本，图还在，第二次才没）。
 */
class KittyRealClientIcatTest : ComposeTerminalLogcatTest() {
    private companion object {
        /** 图像边长（像素）：足够大到一眼可见，又不至于溢出小屏。 */
        const val IMAGE_SIZE = 128

        /** 图像颜色（洋红）：与终端底色、文字颜色都相距甚远。 */
        const val IMAGE_COLOR = 0xFFFF00FF.toInt()

        /** 判定「图像已上屏」的最小洋红像素数：低于此值只能算噪声。 */
        const val MIN_IMAGE_PIXELS = 1_000

        /** 单条命令的等待上限（毫秒）。 */
        const val OUTPUT_TIMEOUT_MS = 60_000L

        /** kitty 安装（含依赖下载）的等待上限（毫秒）。 */
        const val INSTALL_TIMEOUT_MS = 15 * 60_000L

        /** 新建会话后等会话切换的上限（毫秒）。 */
        const val SESSION_SWITCH_TIMEOUT_MS = 30_000L

        /** 安装日志文件名（prefix 家目录内）。 */
        const val INSTALL_LOG_NAME = "kitty-install.log"

        /** 安装退出码文件名（prefix 家目录内）。 */
        const val INSTALL_RESULT_NAME = "kitty-install.result"

        /** `kitten` 可执行文件路径的探测输出文件名（prefix 家目录内）。 */
        const val PROBE_OUTPUT_NAME = "kitty-probe.txt"

        /** `kitten icat` 的标准错误输出文件名（prefix 家目录内）。 */
        const val ICAT_ERROR_NAME = "kitty-icat.err"

        /** `kitten icat` 的退出码文件名（prefix 家目录内）。 */
        const val ICAT_RESULT_NAME = "kitty-icat.result"

        /** `clear` 的退出码文件名（prefix 家目录内）。 */
        const val CLEAR_RESULT_NAME = "kitty-clear.result"

        /**
         * kitty 只在 termux 的 x11 仓库里（主仓库没有这个包），故要临时加一条源。
         *
         * 仓库路径与发行代号取自 <https://packages.termux.dev/apt/> 的实际布局：
         * `termux-x11/dists/x11/`。
         */
        const val X11_REPOSITORY = "deb https://packages.termux.dev/apt/termux-x11/ x11 main"
    }

    private lateinit var homeDirectory: File

    /** prefix 里的 apt 源文件路径（x11 仓库的临时源写在这里）。 */
    private lateinit var x11SourcePath: String

    @Before
    fun setUp() {
        // waitForSession 会先把 prefix 装好（幂等），家目录随之就位。
        composeTestRule.waitForSession()
        val filesDirectory = InstrumentationRegistry.getInstrumentation().targetContext.filesDir
        homeDirectory = File(filesDirectory, "home")
        x11SourcePath =
            File(File(filesDirectory, "usr/etc/apt/sources.list.d"), "x11.list").absolutePath
    }

    @Test
    fun real_kitty_icat_shows_image_and_clear_removes_it() {
        val bridge = openPrefixSession()
        ensureKittyInstalled(bridge)

        val imageFile = writeTestImage()
        val icatExitCode =
            runCommand(
                bridge,
                "kitten icat ${imageFile.absolutePath} 2> ${homeDirectory.absolutePath}/$ICAT_ERROR_NAME",
                ICAT_RESULT_NAME,
                OUTPUT_TIMEOUT_MS,
            )
        assertEquals(
            "kitten icat 退出码非零，标准错误：\n${readHomeFile(ICAT_ERROR_NAME).takeLast(2000)}",
            0,
            icatExitCode,
        )

        val surface = checkNotNull(UxTestUtils.terminalSurface(composeTestRule.activity)) {
            "拿不到终端 Surface"
        }
        val shown =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 300) {
                val shot = UxTestUtils.captureSurface(surface)
                shot != null && UxTestUtils.matchColor(shot, IMAGE_COLOR).count >= MIN_IMAGE_PIXELS
            }
        assertNotNull(
            "kitten icat 的图像没有上屏（${MIN_IMAGE_PIXELS} 个洋红像素都没出现）\n" +
                "终端文本：\n${bridge.getTerminalText().orEmpty().takeLast(600)}",
            shown,
        )
        val bounds =
            UxTestUtils.captureSurface(surface)?.let { UxTestUtils.matchColor(it, IMAGE_COLOR).bounds }
        UxTestUtils.metric("kitty_real_client_image_width", bounds?.width()?.toLong() ?: 0L)
        UxTestUtils.metric("kitty_real_client_image_height", bounds?.height()?.toLong() ?: 0L)
        assertNotNull("图像像素不在 Surface 范围内", bounds)

        val clearExitCode = runCommand(bridge, "clear", CLEAR_RESULT_NAME, OUTPUT_TIMEOUT_MS)
        assertEquals("clear 退出码非零", 0, clearExitCode)

        val cleared =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 300) {
                val shot = UxTestUtils.captureSurface(surface)
                shot == null || UxTestUtils.matchColor(shot, IMAGE_COLOR).count == 0
            }
        assertNotNull("clear 之后图像像素仍在屏幕上（首次 clear 未清除图像）", cleared)
    }

    /**
     * 新建一个跑在 prefix 上的会话。
     *
     * 首个会话早于 prefix 安装（活动先起，prefix 后装），用的是系统 shell；
     * 只有安装之后新建的会话才会走 prefix shell 与它的 PATH。
     */
    private fun openPrefixSession(): Bridge {
        composeTestRule.openDrawer()
        val previousSession = composeTestRule.activeSessionId()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        assertNotNull(
            "点击新增会话后当前会话未在 ${SESSION_SWITCH_TIMEOUT_MS}ms 内切换",
            UxTestUtils.pollUntilTrue(timeoutMs = SESSION_SWITCH_TIMEOUT_MS, intervalMs = 200) {
                composeTestRule.activeSessionId() != previousSession
            },
        )
        composeTestRule.waitForIdle()
        return composeTestRule.awaitBridge()
    }

    /**
     * 装 termux-packages 的 kitty：已装则复用，未装则走 prefix 里的 apt。
     *
     * 不静默跳过：装不上就是硬失败（网络不通、包名不存在都会在这里暴露）。
     */
    private fun ensureKittyInstalled(bridge: Bridge) {
        val probeExitCode =
            runCommand(
                bridge,
                // 末尾的 true：探测命令本身没装成功时退出码是 1，那不是失败——
                // 「有没有装」由探测输出文件的内容决定。
                "command -v kitten > ${homeDirectory.absolutePath}/$PROBE_OUTPUT_NAME; true",
                "kitty-probe.result",
                OUTPUT_TIMEOUT_MS,
            )
        assertEquals("探测 kitten 失败", 0, probeExitCode)
        if (readHomeFile(PROBE_OUTPUT_NAME).isNotBlank()) return

        val installExitCode =
            runCommand(
                bridge,
                "printf '%s\\n' '$X11_REPOSITORY' > $x11SourcePath && " +
                    "apt update > ${homeDirectory.absolutePath}/$INSTALL_LOG_NAME 2>&1 && " +
                    "apt install -y kitty >> ${homeDirectory.absolutePath}/$INSTALL_LOG_NAME 2>&1",
                INSTALL_RESULT_NAME,
                INSTALL_TIMEOUT_MS,
            )
        assertEquals(
            "kitty 安装失败，日志尾部：\n${readHomeFile(INSTALL_LOG_NAME).takeLast(3000)}",
            0,
            installExitCode,
        )
    }

    /**
     * 在 PTY 里跑一条命令，把退出码写进 [resultName]，等它落盘后返回退出码。
     *
     * 为什么用文件而不是等屏幕上的标记：命令会被 shell 回显，回显里就含标记文本，
     * 按屏幕等标记会在命令还没跑完时就「命中」。命令自身的重定向由调用方写全，
     * 这里只追加退出码落盘这一步。
     */
    private fun runCommand(
        bridge: Bridge,
        command: String,
        resultName: String,
        timeoutMs: Long,
    ): Int {
        val resultFile = File(homeDirectory, resultName)
        resultFile.delete()
        assertTrue(
            "PTY 写入失败: $command",
            bridge.writeToPty(
                "$command; echo \$? > ${homeDirectory.absolutePath}/$resultName\n"
                    .toByteArray(Charsets.UTF_8),
            ),
        )
        val finished =
            UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 200) { resultFile.isFile }
        assertNotNull("命令未在 ${timeoutMs}ms 内结束: $command", finished)
        return resultFile.readText().trim().toIntOrNull() ?: -1
    }

    /** 读 prefix 家目录里的文件内容（不存在时返回空串）。 */
    private fun readHomeFile(name: String): String {
        val file = File(homeDirectory, name)
        return if (file.isFile) file.readText() else ""
    }

    /** 写一张纯洋红 PNG，供真实客户端显示。 */
    private fun writeTestImage(): File {
        val bitmap = Bitmap.createBitmap(IMAGE_SIZE, IMAGE_SIZE, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(IMAGE_COLOR)
        val imageFile = File(homeDirectory, "kitty-real-client.png")
        imageFile.outputStream().use { output ->
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) { "PNG 编码失败" }
        }
        bitmap.recycle()
        return imageFile
    }
}
