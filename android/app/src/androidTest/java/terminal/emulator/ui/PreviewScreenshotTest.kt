package terminal.emulator.ui

import android.graphics.Bitmap
import android.util.Log
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.ComposeTerminalLogcatTest
import terminal.emulator.UxTestUtils
import terminal.emulator.activeSessionId
import terminal.emulator.awaitBridge
import terminal.emulator.closeDrawer
import terminal.emulator.installer.ensureBootstrapInstalled
import terminal.emulator.openDrawer
import terminal.emulator.sessionIds
import terminal.emulator.waitForSession
import java.io.File
import java.io.FileOutputStream

/**
 * 预览截图：在**真实引导 + 真实包管理器**下生成用户可见界面的屏幕截图。
 *
 * 三步，每步都走产品路径而非测试专用捷径：
 *
 * 1. 装好 bootstrap 后经 `pkg install fastfetch` 安装外部软件包并运行（终端像素）。
 * 2. 打开会话抽屉（`SessionDrawer`）。
 * 3. 从抽屉进入设置页（`SettingsScreen`）。
 *
 * 为什么用 [android.app.UiAutomation.takeScreenshot] 而不是 Compose 的
 * `captureToImage()`：终端画面在 `SurfaceView` 的独立合成层里，Compose 的
 * 截图只拿得到 View 树，终端区域会是空白。只有合成后的屏幕截图包含真实终端像素。
 *
 * 截图先写应用外部私有目录，再经 `UiAutomation` 的 shell 通道拷到
 * `/data/local/tmp/termux-preview-screenshots/`——`:app:connectedAndroidTest`
 * 跑完会卸载应用，私有目录随之消失，直接 `adb pull` 会一无所获。
 *
 * ```sh
 * (cd android && ./gradlew :app:connectedDebugAndroidTest \
 *   -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.ui.PreviewScreenshotTest)
 * adb pull /data/local/tmp/termux-preview-screenshots ./screenshots
 * ```
 *
 * 不能改成 `adb shell am instrument`：本模块的 runner 是 Cucumber 包装器
 * （`io.cucumber.android.runner.CucumberAndroidJUnitRunner`），直接跑会报
 * `Unable to find instrumentation info`；Gradle 会带上 `cucumberUseAndroidJUnitRunner`
 * 与 `notClass` 两个参数，缺一个都起不来。
 *
 * 该用例刻意**不跳过**：拿不到 bootstrap 或装不上 fastfetch 时以终端文本硬失败，
 * 让「预览截图」不能在没有真实内容的情况下悄悄变绿。
 */
@RunWith(JUnit4::class)
class PreviewScreenshotTest : ComposeTerminalLogcatTest() {
    private companion object {
        private const val TAG = "PreviewScreenshot"
        private const val OUTPUT_TIMEOUT_MS = 30_000L

        /** `pkg update` + `pkg install fastfetch` 在模拟器上的实测上限（含镜像索引下载）。 */
        private const val PACKAGE_INSTALL_TIMEOUT_MS = 15 * 60_000L
        private const val SCREENSHOT_DIR = "screenshots"
        private const val PUBLISH_DIR = "/data/local/tmp/termux-preview-screenshots"

        /** 纯色画面（黑屏/空白）的唯一颜色数远低于此值；真实界面必然超过。 */
        private const val MIN_UNIQUE_COLORS = 16
    }

    /**
     * 在真实 shell 里执行一条命令，等它**真正结束**，返回退出码。
     *
     * 完成判据是**文件**而不是屏幕文本。屏幕文本有两个不可靠之处，都实测发生过：
     * PTY 的行编辑会把键入的命令原样回显（标记在命令开始执行前就"出现"了），
     * 而 apt 的输出会把视口整屏冲走、40 列的 CI 屏幕还会把长行折成多段
     * （`PREVIEW_…` 被拆行后既搜不到，也让失败信息只剩安装日志的中段）。
     * 退出码先写临时文件再 `mv` 到终点：重命名原子，轮询读不到半截数字。
     */
    private fun runCommand(command: String, timeoutMs: Long = OUTPUT_TIMEOUT_MS): Int {
        val home = shellHome()
        val commandFile = File(home, "preview-cmd.rc")
        val tempFile = File(home, "preview-cmd.rc.tmp")
        commandFile.delete()
        tempFile.delete()
        val bridge = composeTestRule.awaitBridge()
        val line =
            "$command; printf '%s\\n' $? > ${quote(tempFile)} && mv ${quote(tempFile)} ${quote(commandFile)}"
        assertTrue("PTY write rejected: $command", bridge.writeToPty("$line\n".toByteArray(Charsets.UTF_8)))
        val done = UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 100) { commandFile.length() > 0 }
        assertNotNull("命令在 ${timeoutMs}ms 内未结束：$command", done)
        val code = commandFile.readText().trim().toIntOrNull()
        assertNotNull("退出码文件内容不可解析：${commandFile.readText().take(32)}", code)
        return requireNotNull(code)
    }

    /**
     * 等终端上真的出现 [minLines] 行非空内容。
     *
     * 不用「屏幕上含某个词」做判据：列数随设备宽度与字体变化，CI 的小屏只有 40 列，
     * `Kernel:` / `Uptime:` 这类键名会在词中断开（实测折成 `Ke` + `rnel:`），
     * 按整词匹配必然假失败。行数判据不受折行影响——折得越多行越多。
     * 执行是否成功仍由 [runCommand] 的退出码判定，这里只确认字节已落到屏幕上，供截图使用。
     */
    private fun awaitRenderedLines(minLines: Int, timeoutMs: Long = OUTPUT_TIMEOUT_MS) {
        val bridge = composeTestRule.awaitBridge()
        val seen =
            UxTestUtils.pollUntilTrue(timeoutMs = timeoutMs, intervalMs = 100) {
                val lines = bridge.getTerminalText()?.lines()?.count { it.isNotBlank() } ?: 0
                lines >= minLines
            }
        assertNotNull(
            "终端在 ${timeoutMs}ms 内未出现 $minLines 行内容：\n${tail(
                composeTestRule.awaitBridge().getTerminalText().orEmpty(),
            )}",
            seen,
        )
    }

    /** prefix 的 `$HOME`：测试与被测应用同进程，可直接按同一路径读写。 */
    private fun shellHome(): File {
        val home = File(InstrumentationRegistry.getInstrumentation().targetContext.filesDir, "home")
        assertTrue("无法创建 $home", home.isDirectory || home.mkdirs())
        return home
    }

    private fun quote(file: File): String = "'${file.absolutePath}'"

    /**
     * 设置页是否已挂到组合树上。
     *
     * 点按到设置页首帧之间有一小段动画，`waitForIdle` 不覆盖这段过渡，
     * 故以「节点存在」为轮询条件，而不是一次断言定生死；节点缺席是这里的正常中间态，
     * 用显式捕获把它与真正的失败区分开。
     */
    private fun settingsScreenShown(): Boolean = try {
        composeTestRule
            .onNodeWithTag("SettingsScreen", useUnmergedTree = true)
            .assertExists()
        true
    } catch (expected: AssertionError) {
        false
    }

    /** 断言上下文：命令失败时只带最后一段终端文本，避免刷屏。 */
    private fun tail(text: String): String = text.takeLast(2_000)

    private fun saveScreenshot(name: String) {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val screenshotDir = File(context.getExternalFilesDir(null), SCREENSHOT_DIR)
        assertTrue("无法创建截图目录 $screenshotDir", screenshotDir.isDirectory || screenshotDir.mkdirs())
        val bitmap =
            InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot()
                ?: throw AssertionError("UiAutomation.takeScreenshot 返回 null（屏幕未合成？）")
        try {
            val file = File(screenshotDir, name)
            FileOutputStream(file).use { output ->
                check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) { "PNG 编码失败：$file" }
            }
            assertTrue("截图文件为空：$file", file.length() > 0)
            publishForPull(file)
            val uniqueColors = countUniqueColors(bitmap)
            Log.i(TAG, "screenshot ${file.absolutePath} ${bitmap.width}x${bitmap.height} colors=$uniqueColors")
            assertTrue(
                "截图 $name 只有 $uniqueColors 种颜色，疑似空屏/黑屏（${bitmap.width}x${bitmap.height}）",
                uniqueColors >= MIN_UNIQUE_COLORS,
            )
        } finally {
            bitmap.recycle()
        }
    }

    /**
     * 拷到 `/data/local/tmp`：instrumentation 的 UiAutomation 以 shell 身份执行命令，
     * 该目录在测试 APK 被卸载后仍然保留，`adb pull` 无需 root。
     *
     * 注意 `executeShellCommand` 不经过 shell 解析：`&&`、`>` 和引号都会被当成普通
     * 参数（实测 `mkdir -p a && ... > b/'x.png'` 直接造出了名为 `'x.png'` 的目录），
     * 所以这里只发单条、无空格、无元字符的命令。
     */
    private fun publishForPull(file: File) {
        val uiAutomation = InstrumentationRegistry.getInstrumentation().uiAutomation

        fun shell(command: String): String = uiAutomation.executeShellCommand(command).use { fileDescriptor ->
            // 读到 EOF 才代表命令结束；不读会与进程退出竞态。
            java.io
                .FileInputStream(fileDescriptor.fileDescriptor)
                .readBytes()
                .toString(Charsets.UTF_8)
        }
        val mkdirError = shell("mkdir -p $PUBLISH_DIR")
        check(mkdirError.isBlank()) { "创建 $PUBLISH_DIR 失败：$mkdirError" }
        val copyError = shell("cp ${file.absolutePath} $PUBLISH_DIR/${file.name}")
        check(copyError.isBlank()) { "发布 ${file.name} 到 $PUBLISH_DIR 失败：$copyError" }
        Log.i(TAG, "publish ${file.name} -> $PUBLISH_DIR")
    }

    private fun countUniqueColors(bitmap: Bitmap): Int {
        val seen = HashSet<Int>()
        // 步长 4：真实界面在 1080x2400 上有数万个采样点，纯色屏仍是 1 种。
        for (y in 0 until bitmap.height step 4) {
            for (x in 0 until bitmap.width step 4) {
                seen.add(bitmap.getPixel(x, y))
                if (seen.size >= MIN_UNIQUE_COLORS) return seen.size
            }
        }
        return seen.size
    }

    /**
     * 关闭除当前活跃会话以外的全部会话（抽屉里的 `×`）。
     *
     * 每个 `×` 都带 `CloseSession-<id>` 的 testTag，按 id 定位而不是按列表位置，
     * 这样即便将来会话排序或标题变化也不会点错。
     */
    private fun closeStaleSessions() {
        composeTestRule.waitForSession()
        val active = composeTestRule.activeSessionId()
        while (true) {
            val stale = composeTestRule.sessionIds().firstOrNull { it != active } ?: return
            composeTestRule.openDrawer()
            composeTestRule.waitForIdle()
            composeTestRule.onNodeWithTag("$CLOSE_SESSION_TAG_PREFIX$stale", useUnmergedTree = true).performClick()
            composeTestRule.waitForIdle()
            assertTrue(
                "会话 $stale 未在 10s 内关闭",
                UxTestUtils.pollUntilTrue(timeoutMs = 10_000, intervalMs = 100) {
                    composeTestRule.sessionIds().none { it == stale }
                } != null,
            )
            composeTestRule.closeDrawer()
        }
    }

    /** 新开一个会话：首个会话可能在 prefix 装好之前就已以 failsafe shell 启动。 */
    private fun openInstalledPrefixSession() {
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        val previous = composeTestRule.activeSessionId()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        assertTrue(
            "新增会话后 activeSessionId 未切换（仍为 $previous）",
            UxTestUtils.pollUntilTrue(timeoutMs = 30_000, intervalMs = 200) {
                composeTestRule.activeSessionId() != previous
            } != null,
        )
        composeTestRule.waitForIdle()
        assertEquals("新会话的 shell 不可用", 0, runCommand("true"))
    }

    @Test
    fun previewScreenshots() {
        // 设备可能从未装过引导包：预览必须建立在真实 prefix 上，故先按产品路径安装。
        // 首个会话可能已以 failsafe shell 启动，安装后由 openInstalledPrefixSession 新开一个。
        ensureBootstrapInstalled()
        openInstalledPrefixSession()
        val bridge = composeTestRule.awaitBridge()

        // 包管理器安装 fastfetch。安装输出落到文件而不是屏幕：apt 的下载进度会把
        // 视口整屏冲走（截图前反正要 clear），失败时再从文件里取尾部进断言。
        val installLog = File(shellHome(), "preview-pkg-install.log")
        val installRc =
            runCommand(
                "{ pkg update -y && pkg install -y fastfetch; } > ${quote(installLog)} 2>&1",
                timeoutMs = PACKAGE_INSTALL_TIMEOUT_MS,
            )
        assertTrue(
            "pkg install fastfetch 失败（RC=$installRc）。安装日志尾部：\n${tail(installLog.readText())}",
            installRc == 0,
        )
        val binaryPath = File(shellHome(), "preview-fastfetch-path")
        val lookupRc = runCommand("command -v fastfetch > ${quote(binaryPath)}")
        assertTrue(
            "fastfetch 未安装到 PATH（command -v 退出码 $lookupRc，" +
                "输出 '${binaryPath.readText().trim()}'）",
            lookupRc == 0 && binaryPath.readText().trim().endsWith("/bin/fastfetch"),
        )

        // 清屏后运行：截图里只有 fastfetch 的预览内容，不叠加安装日志或完成标记。
        // 命令与 docs/specification/TESTING.md 的预览规范一致：无 logo、无颜色、
        // 固定模块顺序，保证截图可复现。数据仍是本机真实值。
        val structure =
            "Title:Separator:OS:Host:Kernel:Uptime:Packages:Shell:Terminal:CPU:Memory:Swap:Disk:Locale"
        val fetchRc = runCommand("clear; fastfetch --logo none --pipe -s \"$structure\"")
        assertEquals("fastfetch 运行失败（RC=$fetchRc）", 0, fetchRc)
        // 模块数即下界；Swap 等在无对应资源时可能不输出。
        awaitRenderedLines(10)
        Thread.sleep(800) // fastfetch 的 logo 与系统信息分行打印，等最后一帧落屏。
        saveScreenshot("01-fastfetch.png")

        // 装 bootstrap 之前启动的那个会话跑的是 failsafe shell，对新装的 prefix 无意义。
        // 它留在抽屉里会让预览图出现一个用户看不懂的 `[1]`，故先按 id 关掉。
        closeStaleSessions()
        composeTestRule.openDrawer()
        composeTestRule.waitForIdle()
        Thread.sleep(400) // 抽屉滑入动画（约 250ms）收尾。
        saveScreenshot("02-session-drawer.png")

        composeTestRule.onNodeWithTag("SettingsButton").performClick()
        composeTestRule.waitForIdle()
        UxTestUtils.pollUntilTrue(timeoutMs = 5_000) {
            settingsScreenShown()
        }
        Thread.sleep(400) // 设置页首帧与滚动位置稳定。
        saveScreenshot("03-settings.png")

        // 保证会话仍活着：截图后的崩溃不应被误判为「截图成功」。
        assertTrue("终端桥在截图后已消失", bridge.getTerminalText() != null)
        Log.i(TAG, "预览截图完成")
    }
}
