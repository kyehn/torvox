package terminal.emulator.installer

import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4

/**
 * End-to-end install of the REAL nix bootstrap (bootstrap-aarch64.zip / bootstrap-x86_64.zip) on
 * the emulator.
 *
 * Prerequisite (adb root): adb push /tmp/nix-bootstrap.zip
 * /data/user/0/<package>/files/nix-bootstrap.zip (the files dir is derived from the runtime package
 * name below — never hardcoded).
 *
 * Why shell commands (UiAutomation.executeShellCommand) instead of direct file access: the
 * instrumentation process carries the TEST package's SELinux category, so it can neither write nor
 * read the app's filesDir (mkdirs silently fails / EACCES — emulator-verified). Shell commands run
 * with the shell uid, and `am start` launches MainActivity in the REAL app process, which installs
 * inside its own sandbox.
 *
 * The install is triggered through the launch entry (EXTRA_INSTALL_BOOTSTRAP intent → MainActivity
 * → BootstrapInstallService) and verified through the installed entry (usr/bin/login + populated
 * prefix tree) — no marker file.
 */
@RunWith(JUnit4::class)
class NixBootstrapInstrumentedTest {
    companion object {
        private const val TAG = "NixBootstrapTest"
        private const val ZIP_NAME = "nix-bootstrap.zip"
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
        val filesDir = "/data/user/0/${ctx.packageName}/files"
        val zipPath = "$filesDir/$ZIP_NAME"
        val loginPath = "$filesDir/usr/bin/login"
        // Environment prerequisite: the real bootstrap zip must be pushed
        // first (adb root): adb push /tmp/nix-bootstrap.zip $zipPath.
        // Hard fail (never skip) when absent — an untested install path
        // must stay red, not silently green.
        Assert.assertTrue(
            "bootstrap zip must be pushed first: adb root && adb push /tmp/nix-bootstrap.zip $zipPath",
            shell("ls $zipPath").isNotBlank(),
        )

        // Trigger the install in the real app process (shell-uid am start).
        // Clear any previous login first so the poll below cannot see a
        // stale entry from an earlier install.
        shell("rm -f $loginPath")
        val startOut =
            shell(
                "am start -n ${ctx.packageName}/$MAIN_ACTIVITY" + " --es $INSTALL_EXTRA $zipPath",
            )
        Log.i(TAG, "am start output: $startOut")

        // Bounded poll for the installed entry (1.5 GB extraction).
        // bin/login is written last by the second stage, so its presence
        // means the install completed.
        val deadline = System.currentTimeMillis() + 10 * 60_000L
        var present = false
        while (System.currentTimeMillis() < deadline) {
            present = shell("[ -f $loginPath ] && echo yes").contains("yes")
            if (present) break
            Thread.sleep(3_000L)
        }
        Log.i(TAG, "app-side install entry present: $present")
        Assert.assertTrue(
            "bin/login missing within timeout (logcat BootstrapInstallService)",
            present,
        )

        // Verify the installed tree and launcher script from the shell side.
        // Layout-agnostic file count (never name the closure layout: the same
        // install path serves every bootstrap flavor).
        val loginHead = shell("head -c 14 $loginPath")
        Assert.assertTrue(
            "bin/login must be a /system/bin/sh launcher, got: $loginHead",
            loginHead.startsWith("#!/system/bin/sh"),
        )
        val installedFiles = shell("find $filesDir/usr -type f | wc -l").trim().toIntOrNull() ?: 0
        Assert.assertTrue(
            "prefix tree must be populated, got $installedFiles files",
            installedFiles > 500,
        )
        Log.i(TAG, "bootstrap end-to-end verified: files=$installedFiles")
    }
}
