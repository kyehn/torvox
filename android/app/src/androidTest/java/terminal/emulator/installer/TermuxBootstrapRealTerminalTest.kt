package terminal.emulator.installer

import android.util.Log
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.UxTestUtils
import terminal.emulator.getBridge
import terminal.emulator.openDrawer
import terminal.emulator.waitForSession

/**
 * Real-terminal verification of the standard termux bootstrap.
 *
 * Installs [ZIP_NAME] from shared storage through the app's own install entry
 * (EXTRA_INSTALL_BOOTSTRAP intent → MainActivity → BootstrapInstallService), then drives the
 * installed prefix shell with REAL terminal input
 * ([writeToPty][terminal.emulator.bridge.Bridge.writeToPty]) and asserts on REAL terminal output
 * ([getTerminalText][terminal.emulator.bridge.Bridge.getTerminalText]).
 *
 * No `adb shell run-as` command execution, no screenshots, no OCR: the shell uid is used only for
 * install setup/observation (staging check, install trigger, completion poll), never for running
 * validation commands — every validated byte travels through the PTY.
 *
 * Hard fail (never skip): the install path must stay red when the prerequisite zip can neither be
 * downloaded nor found pre-staged — an untested install path must not go silently green.
 */
@RunWith(JUnit4::class)
class TermuxBootstrapRealTerminalTest {
    companion object {
        private const val TAG = "TermuxBootstrapTest"
        private const val ZIP_NAME = "termux-bootstrap-x86_64.zip"
        private const val OFFICIAL_URL =
            "https://github.com/termux/termux-packages/releases/download/" +
                "bootstrap-2026.06.21-r1%2Bapt.android-7/bootstrap-x86_64.zip"
        private const val MAIN_ACTIVITY = "terminal.emulator.MainActivity"
        private const val INSTALL_EXTRA = "terminal.emulator.install_bootstrap"
        private const val INSTALL_TIMEOUT_MS = 10 * 60_000L
        private const val OUTPUT_TIMEOUT_MS = 30_000L
    }

    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private fun shell(cmd: String): String {
        val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
        val pfd = automation.executeShellCommand(cmd)
        return pfd.fileDescriptor.let { fd ->
            java.io.FileInputStream(fd).bufferedReader().use { it.readText() }
        }
    }

    private fun ptyLines(command: String): String {
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
        assertTrue(
            "PTY write rejected: $command",
            bridge.writeToPty("$command\n".toByteArray(Charsets.UTF_8)),
        )
        val settled =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                bridge.getTerminalText()?.contains(command.trim()) == true
            }
        assertNotNull("terminal never echoed: $command", settled)
        Thread.sleep(1_500)
        return bridge.getTerminalText().orEmpty()
    }

    /**
     * 把 bootstrap zip 取到应用私有 `cacheDir`，返回它的路径供安装入口使用。
     *
     * 为什么落在 cacheDir：`:install` 服务与主进程同 uid，能直接读应用私有目录；
     * 而 `/data/local/tmp` 只有 shell uid 写得进去（0771），从 app 侧无法落位——
     * 过去这个文件只能靠人 `adb push`，CI 与 `./gradlew connectedAndroidTest` 都没人推，
     * 测试直接硬失败。这里让测试自己取文件，把「预置」从人工步骤变成测试自身的前置。
     *
     * 取不到就大声失败，并给出无外网时的人工预置办法（`-e test.bootstrapUrl=file://`）：
     * 安装路径不允许被跳过。
     */
    private fun stageBootstrapZip(): String {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val staged = java.io.File(context.cacheDir, ZIP_NAME)
        if (staged.length() > 0) {
            Log.i(TAG, "reusing staged bootstrap zip ${staged.path} (${staged.length()} bytes)")
            return staged.path
        }
        val url =
            InstrumentationRegistry.getArguments().getString("test.bootstrapUrl") ?: OFFICIAL_URL
        Log.i(TAG, "fetching bootstrap zip from $url")
        // 不用 runCatching：失败原因必须原样出现在断言信息里（无网、404、
        // 证书问题各自不同），不能被压成一个 isSuccess=false。
        val failure =
            try {
                java.net.URL(url).openStream().use { input ->
                    staged.outputStream().use { output -> input.copyTo(output) }
                }
                null
            } catch (exception: Exception) {
                Log.w(TAG, "bootstrap zip fetch from $url failed", exception)
                "${exception.javaClass.simpleName}: ${exception.message}"
            }
        assertTrue(
            "cannot fetch bootstrap zip from $url ($failure). With no network stage it manually: " +
                "adb push termux-bootstrap-x86_64.zip /data/local/tmp/termux-bootstrap-x86_64.zip" +
                " then rerun with -e test.bootstrapUrl=file:///data/local/tmp/$ZIP_NAME",
            failure == null && staged.length() > 0,
        )
        return staged.path
    }

    @Test
    fun termuxBootstrap_shell_runs_real_commands_with_asserted_output() {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val packageName = ctx.packageName
        // zip 已在应用私有 cacheDir 就位（:install 进程同 uid 可读），无需外部预置。
        val zipPath = stageBootstrapZip()
        val bashPath = "/data/user/0/$packageName/files/usr/bin/bash"

        // Trigger the install in the real app process (shell-uid am start).
        // The installer atomically replaces usr/, so re-running over a
        // previous install is the product path (no stale state possible).
        val startOut =
            shell(
                "am start -n $packageName/$MAIN_ACTIVITY --es $INSTALL_EXTRA $zipPath",
            )
        Log.i(TAG, "am start output: $startOut")

        // Bounded poll for the installed entry. `run-as` is required: the shell uid
        // cannot stat app-private files (plain `[ -f ]` is false forever and burns
        // the whole timeout even on success). Quote-free `ls`: the sibling
        // BootstrapCompatibilityTest proves stderr is NOT merged into the returned
        // stdout, so missing file ⇒ blank, present ⇒ path.
        val deadline = System.currentTimeMillis() + INSTALL_TIMEOUT_MS
        var present = false
        while (System.currentTimeMillis() < deadline) {
            present = shell("run-as $packageName ls $bashPath").isNotBlank()
            if (present) break
            Thread.sleep(3_000L)
        }
        assertTrue("usr/bin/bash missing within timeout (logcat BootstrapInstallService)", present)

        // Spawn a fresh session on the installed prefix: the activity's
        // initial session predates the install (failsafe shell), so open
        // the drawer and add a session through the product path.
        composeTestRule.waitForSession()
        composeTestRule.openDrawer()
        composeTestRule.onNodeWithTag("AddSessionButton").performClick()
        composeTestRule.waitForIdle()
        val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null after add")

        // Marker round-trip: the installed shell is alive and echoes.
        val marker = "TERMUX_ALIVE_${System.currentTimeMillis() % 100000}"
        val markerSeen =
            UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 50) {
                bridge.writeToPty("echo $marker\n".toByteArray(Charsets.UTF_8))
                bridge.getTerminalText()?.contains(marker) == true
            }
        assertNotNull("installed shell never echoed marker $marker", markerSeen)

        // Environment: SHELL under the prefix, PREFIX pointing at usr.
        // PTY 文本按列宽换行：长路径会被切断（如 `.../com.termux/f` + `iles/usr`），
        // 故断言前先去掉全部空白再包含匹配。
        val envText = ptyLines("echo SHELL=\$SHELL PREFIX=\$PREFIX").filterNot { it.isWhitespace() }
        assertTrue(
            "SHELL must be the prefix shell, got: ${envText.takeLast(400)}",
            envText.contains("/files/usr/bin/"),
        )
        assertTrue(
            "PREFIX must point at usr, got: ${envText.takeLast(400)}",
            envText.contains("PREFIX=/data/data/com.termux/files/usr") ||
                envText.contains("PREFIX=/data/user/0/$packageName/files/usr"),
        )

        // Prefix binaries list and run (`head -8` 按字母截断会漏掉 bash，用 `command -v`)。
        // 空白已在外层去不掉（`command -v` 输出短），此处不断言前不需再过滤。
        val lsText = ptyLines("command -v bash; echo LS_RC=$?").filterNot { it.isWhitespace() }
        assertTrue(
            "bin listing must succeed, got: ${lsText.takeLast(400)}",
            lsText.contains("LS_RC=0"),
        )
        assertTrue(
            "bin listing must include bash, got: ${lsText.takeLast(400)}",
            lsText.contains("/bin/bash"),
        )
        val bashVersion = ptyLines("bash --version | head -1").filterNot { it.isWhitespace() }
        assertTrue(
            "bash must report GNU version, got: ${bashVersion.takeLast(200)}",
            bashVersion.contains("GNUbash"),
        )
        Log.i(TAG, "termux bootstrap real-terminal verified: ${bashVersion.lines().lastOrNull()}")
    }
}
