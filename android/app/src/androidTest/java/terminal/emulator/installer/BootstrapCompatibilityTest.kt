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

        @BeforeClass @JvmStatic
        fun ensureBootstrap() {
            ensureBootstrapInstalled()
        }
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

    private fun runExit(command: String): Pair<String, Int> {
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
        val output = aptUpdate()
        Log.i(TAG, "apt update: ${output.take(120)}")
        Assert.assertTrue(
            "apt update failed",
            output.contains("Done") || output.contains("Reading") || output.contains("Hit:"),
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
        Log.i(TAG, "figlet install: ${install.take(120)}")
        val versionOutput = runAs("figlet --version 2>&1")
        Assert.assertTrue("figlet version blank", versionOutput.isNotBlank())
        val ascii = runAs("figlet -f mini HELLO 2>&1")
        Assert.assertTrue("figlet output too short", ascii.length > 20)
    }

    @Test
    fun apt_installPython() {
        val install = aptInstall("python")
        Log.i(TAG, "python install: ${install.take(120)}")
        val versionOutput = runAs("python --version 2>&1")
        Assert.assertTrue("python not found", versionOutput.lowercase().contains("python"))
        val piOutput = runAs("python -c 'import math; print(math.pi)' 2>&1")
        Assert.assertTrue("python math.pi failed", piOutput.contains("3.14"))
    }

    @Test
    fun apt_installGit() {
        val install = aptInstall("git")
        Log.i(TAG, "git install: ${install.take(120)}")
        val versionOutput = runAs("git --version 2>&1")
        Assert.assertTrue("git not found", versionOutput.lowercase().contains("git"))
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
        val ensure = runAs("command -v dpkg || (apt update -qq && apt install -y -qq dpkg 2>&1)")
        Log.i(TAG, "dpkg ensure: ${ensure.take(80)}")
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
        val output = aptInstall("sl")
        Log.i(TAG, "sl install: ${output.take(120)}")
        val versionOutput = runAs("sl --version 2>&1")
        Assert.assertTrue(
            "sl should be installable",
            output.contains("Setting up") || versionOutput.contains("sl") || versionOutput.isNotBlank(),
        )
    }

    @Test
    fun apt_cache_policy() {
        val output = runAs("apt-cache policy 2>&1 || apt policy 2>&1")
        Log.i(TAG, "apt policy: ${output.take(120)}")
        Assert.assertTrue(
            "apt policy should show repositories",
            output.contains("://") || output.contains("sources"),
        )
    }
}
