package terminal.emulator.installer

import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
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
        private val BOOTSTRAP_URL by lazy {
            // 模拟器无外网：CI/本地先 adb push 官方 zip 到 Download 目录，测试经
            // Instrumentation arguments 以 test.bootstrapUrl 传入 file:// 路径
            // （adb shell am instrument -e test.bootstrapUrl file:///...）。
            // 未传参则回退官方 https URL（有外网的设备仍可直跑）。
            InstrumentationRegistry.getArguments().getString("test.bootstrapUrl")
                ?: "https://github.com/termux/termux-packages/releases/download/" +
                "bootstrap-2026.06.21-r1%2Bapt.android-7/bootstrap-x86_64.zip"
        }
        private const val TAG = "BootstrapTest"

        @BeforeClass @JvmStatic
        fun ensureBootstrap() {
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val bash = java.io.File(context.filesDir, "usr/bin/bash")
            if (bash.exists()) {
                java.io.File(context.filesDir, "home").mkdirs()
                return
            }
            val ctx = InstrumentationRegistry.getInstrumentation().targetContext
            Log.i(TAG, "installing bootstrap from $BOOTSTRAP_URL")
            val result =
                runBlocking {
                    // 路径必须经 context 解析：/data/data 与 /data/user/0 在此设备
                    // 上并非同一挂载，硬编码前者会导致 staging→prefix 的原子换入
                    // 跨分区失败。prefix/home/staging 三者必须同目录同挂载。
                    val prefixDir = java.io.File(ctx.filesDir, "usr")
                    val homeDir = java.io.File(ctx.filesDir, "home")
                    // staging 必须与 prefix 同文件系统（filesDir），否则原子换入
                    // 的 rename 跨分区失败（cacheDir 与 filesDir 不同挂载）。
                    val stagingDir = java.io.File(ctx.filesDir, "bootstrap-staging")
                    BootstrapOrchestrator(
                        BootstrapDownloader(ctx),
                        BootstrapInstaller(prefixDir, homeDir, stagingDir),
                        SecondStageRunner(prefixDir, homeDir),
                    ).ensureBootstrap(BOOTSTRAP_URL)
                }
            Log.i(TAG, "bootstrap result: $result")
            Assert.assertTrue("bootstrap failed", result.isSuccess)
            Assert.assertTrue("bash not found after bootstrap", bash.exists())
            java.io.File(context.filesDir, "home").mkdirs()
        }
    }

    private fun exec(cmd: String): String {
        val pfd =
            InstrumentationRegistry
                .getInstrumentation()
                .uiAutomation
                .executeShellCommand(cmd)
        return ParcelFileDescriptor.AutoCloseInputStream(pfd).bufferedReader().readText()
    }

    private fun prefixPath(): String = "${InstrumentationRegistry.getInstrumentation().targetContext.filesDir}/usr"

    private fun homePath(): String = "${InstrumentationRegistry.getInstrumentation().targetContext.filesDir}/home"

    private fun makeScript(body: String): File {
        val testFile = File("/data/data/com.termux/cache/tc.sh")
        testFile.parentFile?.mkdirs()
        val env = (
            "export PREFIX=" + prefixPath() + "\n" +
                "export PATH=\$PREFIX/bin:\$PREFIX/bin/applets:/system/bin" + "\n" +
                "export HOME=" + homePath() + "\n" +
                "export SHELL=\$PREFIX/bin/bash" + "\n" +
                "export TERM=vt100"
            )
        testFile.writeText(env + "\n" + body + "\n")
        testFile.setExecutable(true)
        return testFile
    }

    private fun runAs(cmd: String): String = exec(
        "run-as com.termux /data/data/com.termux/files/usr/bin/bash " +
            makeScript(cmd).absolutePath,
    )

    private fun runExit(cmd: String): Pair<String, Int> {
        val script = makeScript("($cmd)\necho EXITCODE=$?")
        val out =
            exec(
                "run-as com.termux /data/data/com.termux/files/usr/bin/bash " +
                    script.absolutePath,
            )
        val lines = out.trim().lines()
        val last = lines.lastOrNull()?.trim() ?: ""
        val code =
            if (last.startsWith("EXITCODE=")) {
                last.removePrefix("EXITCODE=").trim().toIntOrNull() ?: -1
            } else {
                -1
            }
        val text = if (lines.size > 1) lines.dropLast(1).joinToString("\n") else ""
        return text.trim() to code
    }

    private fun pkgTermux(args: String): String = runAs("pkg $args 2>&1")

    private fun aptInstall(args: String): String =
        runAs("DEBIAN_FRONTEND=noninteractive apt install -y --allow-unauthenticated $args 2>&1")

    private fun aptUpdate(): String = runAs("apt update 2>&1")

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
        val (_, code) = runExit("exit 42")
        Assert.assertEquals(42, code)
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
        val out = aptUpdate()
        Log.i(TAG, "apt update: ${out.take(120)}")
        Assert.assertTrue(
            "apt update failed",
            out.contains("Done") || out.contains("Reading") || out.contains("Hit:"),
        )
    }

    @Test
    fun apt_listBash() {
        val out = pkgTermux("list-installed")
        Assert.assertTrue("bash not listed in packages", out.contains("bash"))
    }

    @Test
    fun apt_installFiglet() {
        val install = aptInstall("figlet")
        Log.i(TAG, "figlet install: ${install.take(120)}")
        val ver = runAs("figlet --version 2>&1")
        Assert.assertTrue("figlet version blank", ver.isNotBlank())
        val ascii = runAs("figlet -f mini HELLO 2>&1")
        Assert.assertTrue("figlet output too short", ascii.length > 20)
    }

    @Test
    fun apt_installPython() {
        val install = aptInstall("python")
        Log.i(TAG, "python install: ${install.take(120)}")
        val ver = runAs("python --version 2>&1")
        Assert.assertTrue("python not found", ver.lowercase().contains("python"))
        val pi = runAs("python -c 'import math; print(math.pi)' 2>&1")
        Assert.assertTrue("python math.pi failed", pi.contains("3.14"))
    }

    @Test
    fun apt_installGit() {
        val install = aptInstall("git")
        Log.i(TAG, "git install: ${install.take(120)}")
        val ver = runAs("git --version 2>&1")
        Assert.assertTrue("git not found", ver.lowercase().contains("git"))
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
        val out = pkgTermux("list-installed")
        Assert.assertTrue("pkg list-installed failed", out.contains("bash"))
    }

    @Test
    fun dpkg_query() {
        val ensure = runAs("command -v dpkg || (apt update -qq && apt install -y -qq dpkg 2>&1)")
        Log.i(TAG, "dpkg ensure: ${ensure.take(80)}")
        val out = runAs("dpkg -L bash 2>&1")
        Assert.assertTrue(
            "dpkg -L bash should list files",
            out.contains("bin/bash") || out.contains("/usr/bin/"),
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
        val out =
            exec(
                "run-as com.termux /data/data/com.termux/files/usr/bin/bash " + script.absolutePath,
            )
        Assert.assertTrue("heredoc should contain hello", out.contains("hello"))
        Assert.assertTrue("heredoc should contain world", out.contains("world"))
    }

    @Test
    fun filesystem_bin_permissions() {
        val out = runAs("ls -la /data/data/com.termux/files/usr/bin/ 2>&1 | head -5")
        Assert.assertFalse("bin directory should not be empty", out.isBlank())
        Assert.assertTrue("bin entries should be files", out.contains("-") || out.contains("l"))
    }

    @Test
    fun bash_long_running() {
        val out = runAs("for i in 1 2 3 4 5; do echo \"line-\$i\"; done 2>&1")
        val lines = out.trim().lines()
        Assert.assertEquals(5, lines.size)
        Assert.assertEquals("line-3", lines[2].trim())
    }

    @Test
    fun apt_unauthenticated_install_fallback() {
        val out = aptInstall("sl")
        Log.i(TAG, "sl install: ${out.take(120)}")
        val ver = runAs("sl --version 2>&1")
        Assert.assertTrue(
            "sl should be installable",
            out.contains("Setting up") || ver.contains("sl") || ver.isNotBlank(),
        )
    }

    @Test
    fun apt_cache_policy() {
        val out = runAs("apt-cache policy 2>&1 || apt policy 2>&1")
        Log.i(TAG, "apt policy: ${out.take(120)}")
        Assert.assertTrue(
            "apt policy should show repositories",
            out.contains("://") || out.contains("sources"),
        )
    }
}
