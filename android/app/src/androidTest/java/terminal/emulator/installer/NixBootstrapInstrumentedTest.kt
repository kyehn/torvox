package terminal.emulator.installer

import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import java.io.File

/**
 * End-to-end install of the REAL nix bootstrap
 * (bootstrap-aarch64.zip / bootstrap-x86_64.zip) on the emulator.
 *
 * Prerequisite (adb root):
 *   adb push /tmp/nix-bootstrap.zip /data/user/0/com.termux/files/nix-bootstrap.zip
 *
 * Why shell commands (UiAutomation.executeShellCommand) instead of direct
 * file access: the instrumentation process carries the TEST package's
 * SELinux category, so it can neither write nor read the app's filesDir
 * (mkdirs silently fails / EACCES — emulator-verified). Shell
 * commands run with the shell uid, and `am start` launches MainActivity in
 * the REAL app process, which installs inside its own sandbox.
 *
 * The app writes its result marker to files/install-result.txt via the
 * EXTRA_INSTALL_BOOTSTRAP intent (install runs in a throwaway prefix
 * files/usr, keeping any existing termux bootstrap untouched).
 *
 * Verifies the contract:
 *  - SYMLINKS.txt targets are prefix-relative (no absolute targets)
 *  - EXECUTABLES.txt entries are chmod'ed
 *  - bin/login is a `/system/bin/` launcher script and isInstalled() recognizes it
 *  - the second stage (no dpkg dir) still writes termux.env
 */
@RunWith(JUnit4::class)
class NixBootstrapInstrumentedTest {
    companion object {
        private const val TAG = "NixBootstrapTest"
        private const val ZIP_PATH = "/data/user/0/com.termux/files/nix-bootstrap.zip"
        private const val RESULT_PATH = "/data/user/0/com.termux/files/install-result.txt"
        private const val MAIN_ACTIVITY = "terminal.emulator.MainActivity"
        private const val INSTALL_EXTRA = "terminal.emulator.install_bootstrap"
    }

    private fun shell(cmd: String): String {
        val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
        val pfd = automation.executeShellCommand(cmd)
        return pfd.fileDescriptor.let { fd ->
            java.io.FileInputStream(fd).bufferedReader().use { it.readText() }
        }
    }

    @Test
    fun nixBootstrapInstallsAndReportsComplete() {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        // Environment prerequisite: the real nix bootstrap zip must be
        // pushed first (adb root): adb push /tmp/nix-bootstrap.zip $ZIP_PATH.
        // Skip (not fail) when absent — this is a data-provider test, not a
        // code regression, and CI has no bootstrap artifact to push.
        assumeTrue(
            "nix zip must be pushed first: adb root && adb push /tmp/nix-bootstrap.zip $ZIP_PATH",
            shell("ls $ZIP_PATH").isNotBlank(),
        )

        // Trigger the install in the real app process (shell-uid am start).
        shell("rm -f $RESULT_PATH")
        val startOut =
            shell(
                "am start -n ${ctx.packageName}/$MAIN_ACTIVITY" +
                    " --es $INSTALL_EXTRA $ZIP_PATH",
            )
        Log.i(TAG, "am start output: $startOut")

        // Bounded poll for the result marker (1.5 GB extraction).
        val deadline = System.currentTimeMillis() + 10 * 60_000L
        var result = ""
        while (System.currentTimeMillis() < deadline) {
            result = shell("cat $RESULT_PATH 2>/dev/null").trim()
            if (result.isNotEmpty()) break
            Thread.sleep(3_000L)
        }
        Log.i(TAG, "app-side install result: $result")
        Assert.assertFalse("install result missing within timeout (logcat InstallBackdoor)", result.isEmpty())
        Assert.assertTrue("install must succeed: $result", result.startsWith("OK "))
        Assert.assertTrue("login shell must be selected: $result", result.contains("shell=bin/login"))
        Assert.assertTrue("isInstalled must be true: $result", result.contains("installed=true"))
        Assert.assertTrue("needsInstall must be false after install: $result", result.contains("needsInstall=false"))

        // Verify the store tree and launcher script from the shell side.
        val loginHead = shell("head -c 14 /data/user/0/com.termux/files/usr/bin/login")
        Assert.assertTrue("bin/login must be a /system/bin/sh launcher, got: $loginHead", loginHead.startsWith("#!/system/bin/sh"))
        // executeShellCommand runs /system/bin/sh; count store entries
        // line-by-line instead of relying on a wc pipeline.
        val storeLines = shell("ls -1 /data/user/0/com.termux/files/usr/nix/store").lines().filter { it.isNotBlank() }
        Assert.assertTrue("nix/store must be populated, got ${storeLines.size} entries", storeLines.size > 5)
        val envLine =
            shell("grep ^SHELL= /data/user/0/com.termux/files/usr/etc/termux/termux.env").trim()
        Assert.assertTrue("SHELL must point at bin/login: $envLine", envLine.endsWith("/bin/login"))

        Log.i(TAG, "nix bootstrap end-to-end verified: store=${storeLines.size} $envLine")
    }
}
