package terminal.emulator.installer

import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert
import org.junit.BeforeClass
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.TerminalLogcatTest
import java.io.File

@RunWith(JUnit4::class)
class BootstrapCompatibilityTest : TerminalLogcatTest() {
    companion object {
        private const val TAG = "BootstrapTest"

        /**
         * 走网络的 apt 参数。同一份镜像上 `apt update` 时好时坏（同一 commit 的
         * 两次 CI：一次 236 例全绿，一次这两条失败），这里用 apt 自带的
         * `Acquire::Retries` 重试瞬时失败并给下载加超时——比自己写重试循环更贴近
         * 工具语义，真错误（源缺失、包不存在）仍然照常失败。
         */
        private const val APT_NET =
            "-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30"

        /** 按需预热后所有安装类用例的共同前提，同时也是它们失败时唯一的取证来源。 */
        private var aptUpdateResult: ShellResult? = null

        @BeforeClass @JvmStatic
        fun ensureBootstrap() {
            ensureBootstrapInstalled()
        }
    }

    /**
     * 子进程的退出码与全文。此前只保留文本，于是「装不上」与「装上但跑不起来」
     * 在断言里长得一模一样：`bash: python: command not found` 既非空也含包名，
     * `isNotBlank()` / `contains("git")` 全部成立——三个安装用例因此在没有仓库
     * 索引的环境里一直是绿的。
     */
    private data class ShellResult(val output: String, val code: Int)

    /**
     * 仓库索引是安装类用例的共同前提，且只能取一次：JUnit4 不保证方法顺序，
     * 逐用例各跑一遍 `apt update` 既慢、也容易撞上 apt 的文件锁。按需预热把
     * 「索引就绪」变成确定性状态，全文留给断言失败时取证。
     */
    private fun aptIndex(): ShellResult {
        aptUpdateResult?.let { return it }
        val result = runExit("apt update $APT_NET 2>&1")
        aptUpdateResult = result
        Log.i(TAG, "apt update exit=${result.code}: ${result.output.takeLast(200)}")
        return result
    }

    private fun exec(command: String): String {
        val parcelFileDescriptor =
            InstrumentationRegistry
                .getInstrumentation()
                .uiAutomation
                .executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(parcelFileDescriptor).bufferedReader().readText()
    }

    private fun prefixPath(): String = "${InstrumentationRegistry.getInstrumentation().targetContext.filesDir}/usr"

    private fun homePath(): String = "${InstrumentationRegistry.getInstrumentation().targetContext.filesDir}/home"

    private fun makeScript(body: String): File {
        val testFile = File("/data/data/com.termux/cache/tc.sh")
        testFile.parentFile?.mkdirs()
        val environment = (
            "export PREFIX=" + prefixPath() + "\n" +
                "export PATH=\$PREFIX/bin:\$PREFIX/bin/applets:/system/bin" + "\n" +
                "export HOME=" + homePath() + "\n" +
                "export SHELL=\$PREFIX/bin/bash" + "\n" +
                "export TERM=vt100"
            )
        testFile.writeText(environment + "\n" + body + "\n")
        testFile.setExecutable(true)
        return testFile
    }

    private fun runAs(command: String): String = exec(
        "run-as com.termux /data/data/com.termux/files/usr/bin/bash " +
            makeScript(command).absolutePath,
    )

    private fun runExit(command: String): ShellResult {
        val script = makeScript("($command)\necho EXITCODE=$?")
        val output =
            exec(
                "run-as com.termux /data/data/com.termux/files/usr/bin/bash " +
                    script.absolutePath,
            )
        val lines = output.trim().lines()
        val last = lines.lastOrNull()?.trim() ?: ""
        val code =
            if (last.startsWith("EXITCODE=")) {
                last.removePrefix("EXITCODE=").trim().toIntOrNull() ?: -1
            } else {
                -1
            }
        val text = if (lines.size > 1) lines.dropLast(1).joinToString("\n") else ""
        return ShellResult(text.trim(), code)
    }

    private fun pkgTermux(args: String): String = runAs("pkg $args 2>&1")

    /**
     * 装包前先确保索引就绪：空索引下 `apt install` 会立刻报 `Unable to locate
     * package`，那正是这两条用例在 CI 上的失败形态。索引只取一次，第二次起
     * 直接复用。
     */
    private fun aptInstall(args: String): ShellResult {
        aptIndex()
        return runExit(
            "DEBIAN_FRONTEND=noninteractive apt install -y --allow-unauthenticated " +
                "$APT_NET $args 2>&1",
        )
    }

    /**
     * 断言包真的进了 dpkg 数据库。安装退出码与 `dpkg -s` 缺一不可：前者能是 0
     * 却只装了个空壳依赖，后者能因 `command not found` 之外的任何输出骗过
     * `contains(包名)` 这类弱断言。
     */
    private fun assertInstalled(pkg: String, install: ShellResult) {
        val status = runAs("dpkg -s $pkg 2>&1")
        val update = aptIndex()
        Assert.assertTrue(
            "$pkg 未安装：install exit=${install.code}\n${install.output.takeLast(1200)}\n" +
                "--- dpkg -s $pkg ---\n${status.take(400)}\n" +
                "--- apt update exit=${update.code} ---\n${update.output.takeLast(600)}",
            status.contains("Status: install ok installed"),
        )
    }

    @Test
    fun bootstrap_dirsExist() {
        for (d in listOf("bin", "etc", "lib", "tmp")) {
            Assert.assertTrue("$d in PREFIX missing", File("${prefixPath()}/$d").isDirectory)
        }
        Assert.assertTrue("home missing", File(homePath()).isDirectory)
    }

    @Test
    fun bash_echo() {
        Assert.assertEquals("TEST_OK", runAs("echo TEST_OK").trim())
    }

    @Test
    fun bash_exitCode() {
        Assert.assertEquals(42, runExit("exit 42").code)
    }

    @Test
    fun bash_prefix() {
        Assert.assertEquals(prefixPath(), runAs("echo \$PREFIX").trim())
    }

    @Test
    fun bash_path() {
        Assert.assertTrue(runAs("echo \$PATH").contains(prefixPath()))
    }

    @Test
    fun bash_which() {
        Assert.assertEquals("${prefixPath()}/bin/bash", runAs("which bash").trim())
    }

    @Test
    fun apt_version() {
        Assert.assertTrue(runAs("apt --version 2>&1").lowercase().contains("apt"))
    }

    @Test
    fun apt_update() {
        val result = aptIndex()
        // `Reading package lists... Done` 在拉取失败时同样会打印，只查它等于
        // 让「没有网络也能过」成为默认行为。这里查退出码与实际失败行。
        val failures =
            result.output.lines().filter { line ->
                line.startsWith("Err:") || line.startsWith("E:") || line.contains("Failed to fetch")
            }
        Assert.assertEquals("apt update 退出码非 0：\n${result.output.takeLast(1200)}", 0, result.code)
        Assert.assertTrue(
            "apt update 有失败行：\n${failures.joinToString("\n").take(800)}\n" +
                "---\n${result.output.takeLast(1200)}",
            failures.isEmpty(),
        )
    }

    @Test
    fun apt_listBash() {
        val output = pkgTermux("list-installed")
        Assert.assertTrue("bash not listed in packages", output.contains("bash"))
    }

    @Test
    fun apt_installFiglet() {
        val install = aptInstall("figlet")
        Log.i(TAG, "figlet install exit=${install.code}: ${install.output.takeLast(200)}")
        assertInstalled("figlet", install)
        val ascii = runAs("figlet -f mini HELLO 2>&1")
        Assert.assertTrue("figlet 渲染过短：${ascii.take(200)}", ascii.length > 20)
    }

    @Test
    fun apt_installPython() {
        val install = aptInstall("python")
        Log.i(TAG, "python install exit=${install.code}: ${install.output.takeLast(200)}")
        assertInstalled("python", install)
        val piOutput = runAs("python -c 'import math; print(math.pi)' 2>&1")
        Assert.assertTrue(
            "python 算不出 math.pi：${piOutput.take(300)}\n" +
                "--- install exit=${install.code} ---\n${install.output.takeLast(600)}",
            piOutput.contains("3.14"),
        )
    }

    @Test
    fun apt_installGit() {
        val install = aptInstall("git")
        Log.i(TAG, "git install exit=${install.code}: ${install.output.takeLast(200)}")
        assertInstalled("git", install)
        val versionOutput = runAs("git --version 2>&1").trim()
        Assert.assertTrue("git --version 异常：${versionOutput.take(200)}", versionOutput.startsWith("git version"))
    }

    @Test
    fun bash_whichExisting() {
        for (prog in listOf("bash", "apt", "dpkg")) {
            Assert.assertTrue(
                "which $prog failed",
                runAs("which $prog 2>&1").trim().isNotEmpty(),
            )
        }
    }

    @Test
    fun bash_whichNonexistent() {
        Assert.assertTrue(
            "nonexistent should be empty",
            runAs("which nonexistent_cmd_xyz 2>&1").isEmpty(),
        )
    }

    @Test
    fun bash_stderr() {
        Assert.assertEquals(
            "STDOUT_ONLY",
            runAs("echo STDOUT_ONLY 2>/dev/null").trim(),
        )
    }

    @Test
    fun dpkg_version() {
        Assert.assertTrue(runAs("dpkg --version 2>&1").lowercase().contains("dpkg"))
    }

    @Test
    fun pkg_listInstalled() {
        val output = pkgTermux("list-installed")
        Assert.assertTrue("pkg list-installed failed", output.contains("bash"))
    }

    @Test
    fun dpkg_query() {
        val output = runAs("dpkg -L bash 2>&1")
        Assert.assertTrue(
            "dpkg -L bash should list files",
            output.contains("bin/bash") || output.contains("/usr/bin/"),
        )
    }

    @Test
    fun bash_env_HOME() {
        Assert.assertEquals(homePath(), runAs("echo \$HOME").trim())
    }

    @Test
    fun bash_env_SHELL() {
        Assert.assertEquals("${prefixPath()}/bin/bash", runAs("echo \$SHELL").trim())
    }

    @Test
    fun bash_env_TERM() {
        Assert.assertEquals("vt100", runAs("echo \$TERM").trim())
    }

    @Test
    fun bash_heredoc() {
        val script = makeScript("cat <<EOF\nhello\nworld\nEOF\n")
        val output =
            exec(
                "run-as com.termux /data/data/com.termux/files/usr/bin/bash " + script.absolutePath,
            )
        Assert.assertTrue("heredoc should contain hello", output.contains("hello"))
        Assert.assertTrue("heredoc should contain world", output.contains("world"))
    }

    @Test
    fun filesystem_bin_permissions() {
        val output = runAs("ls -la /data/data/com.termux/files/usr/bin/ 2>&1 | head -5")
        Assert.assertFalse("bin directory should not be empty", output.isBlank())
        Assert.assertTrue("bin entries should be files", output.contains("-") || output.contains("l"))
    }

    @Test
    fun bash_long_running() {
        val output = runAs("for i in 1 2 3 4 5; do echo \"line-\$i\"; done 2>&1")
        val lines = output.trim().lines()
        Assert.assertEquals(5, lines.size)
        Assert.assertEquals("line-3", lines[2].trim())
    }

    @Test
    fun apt_unauthenticated_install_fallback() {
        val install = aptInstall("sl")
        Log.i(TAG, "sl install exit=${install.code}: ${install.output.takeLast(200)}")
        assertInstalled("sl", install)
        val location = runAs("command -v sl 2>&1").trim()
        Assert.assertTrue("sl 不在 PATH 里：${location.take(200)}", location.endsWith("/sl"))
    }

    @Test
    fun apt_cache_policy() {
        val output = runAs("apt-cache policy 2>&1 || apt policy 2>&1")
        val update = aptIndex()
        // 空索引下 `apt-cache policy` 只打印 `/var/lib/dpkg/status` 那段，
        // 没有任何 `://`；能列出仓库才说明 sources 与索引都到位。
        Assert.assertTrue(
            "apt-cache policy 未列出仓库：\n${output.take(1200)}\n" +
                "--- apt update exit=${update.code} ---\n${update.output.takeLast(800)}",
            output.contains("://"),
        )
    }
}
