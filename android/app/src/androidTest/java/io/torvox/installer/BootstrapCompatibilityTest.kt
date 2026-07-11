package io.torvox.installer

import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.AfterClass
import org.junit.Assert
import org.junit.BeforeClass
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import java.io.File

@RunWith(JUnit4::class)
class BootstrapCompatibilityTest {
    companion object {
        private const val PREFIX = "/data/data/com.termux/files/usr"
        private const val BASH_PATH = "$PREFIX/bin/bash"
        private const val HOME_DIR = "$PREFIX/home"
        private val BOOTSTRAP_URL by lazy {
            System.getProperty("torvox.test.bootstrapUrl")
                ?: "https://github.com/termux/termux-packages/releases/download/bootstrap-2026.06.21-r1%2Bapt.android-7/bootstrap-x86_64.zip"
        }
        private const val TAG = "BootstrapTest"

        @BeforeClass @JvmStatic
        fun ensureBootstrap() {
            val bash = File(BASH_PATH)
            if (bash.exists()) {
                File(HOME_DIR).mkdirs()
                return
            }
            val ctx = InstrumentationRegistry.getInstrumentation().targetContext
            Log.i(TAG, "installing bootstrap from $BOOTSTRAP_URL")
            val result =
                runBlocking {
                    val prefixDir = java.io.File("/data/data/com.termux/files/usr")
                    val homeDir = java.io.File("/data/data/com.termux/files/home")
                    val stagingDir = java.io.File(ctx.cacheDir, "bootstrap-staging")
                    BootstrapOrchestrator(
                        BootstrapDownloader(ctx),
                        BootstrapInstaller(prefixDir, homeDir, stagingDir),
                        SecondStageRunner(prefixDir, homeDir),
                    ).ensureBootstrap(BOOTSTRAP_URL)
                }
            Log.i(TAG, "bootstrap result: $result")
            Assert.assertTrue("bootstrap failed", result.isSuccess)
            Assert.assertTrue("bash not found after bootstrap", bash.exists())
            File(HOME_DIR).mkdirs()
        }

        @AfterClass @JvmStatic
        fun cleanupBootstrapUrl() {
            System.clearProperty("torvox.test.bootstrapUrl")
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

    private fun makeScript(body: String): File {
        val testFile = File("/data/data/com.termux/cache/tc.sh")
        testFile.parentFile?.mkdirs()
        val env = (
            "export PREFIX=/data/data/com.termux/files/usr" + "\n" +
                "export PATH=\$PREFIX/bin:\$PREFIX/bin/applets:/system/bin" + "\n" +
                "export HOME=\$PREFIX/home" + "\n" +
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

    private fun aptInstall(args: String): String = runAs("DEBIAN_FRONTEND=noninteractive apt install -y --allow-unauthenticated $args 2>&1")

    private fun aptUpdate(): String = runAs("apt update 2>&1")

    @Test
    fun bootstrap_dirsExist() {
        for (d in listOf("bin", "etc", "lib", "tmp")) {
            Assert.assertTrue("$d in PREFIX missing", File("$PREFIX/$d").isDirectory)
        }
        Assert.assertTrue("home missing", File(HOME_DIR).isDirectory)
    }

    @Test
    fun bash_echo() {
        Assert.assertEquals("TORVOX_TEST_OK", runAs("echo TORVOX_TEST_OK").trim())
    }

    @Test
    fun bash_exitCode() {
        val (_, code) = runExit("exit 42")
        Assert.assertEquals(42, code)
    }

    @Test
    fun bash_prefix() {
        Assert.assertEquals(PREFIX, runAs("echo \$PREFIX").trim())
    }

    @Test
    fun bash_path() {
        Assert.assertTrue(runAs("echo \$PATH").contains(PREFIX))
    }

    @Test
    fun bash_which() {
        Assert.assertEquals(BASH_PATH, runAs("which bash").trim())
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
}
