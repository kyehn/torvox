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
 * Hard fail (never skip) when the prerequisite zip is absent — an untested install path must stay
 * red, not silently green.
 */
@RunWith(JUnit4::class)
class TermuxBootstrapRealTerminalTest {
  companion object {
    private const val TAG = "TermuxBootstrapTest"
    private const val ZIP_NAME = "termux-bootstrap-x86_64.zip"
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

  @Test
  fun termuxBootstrap_shell_runs_real_commands_with_asserted_output() {
    val ctx = InstrumentationRegistry.getInstrumentation().targetContext
    val packageName = ctx.packageName
    val zipPath = "/sdcard/Download/$ZIP_NAME"
    val bashPath = "/data/user/0/$packageName/files/usr/bin/bash"
    assertTrue(
        "bootstrap zip must be staged first: adb push <termux bootstrap-x86_64.zip> $zipPath",
        shell("ls $zipPath").isNotBlank(),
    )

    // Trigger the install in the real app process (shell-uid am start).
    // The installer atomically replaces usr/, so re-running over a
    // previous install is the product path (no stale state possible).
    val startOut =
        shell(
            "am start -n $packageName/$MAIN_ACTIVITY --es $INSTALL_EXTRA $zipPath",
        )
    Log.i(TAG, "am start output: $startOut")

    // Bounded poll for the installed entry.
    val deadline = System.currentTimeMillis() + INSTALL_TIMEOUT_MS
    var present = false
    while (System.currentTimeMillis() < deadline) {
      present = shell("[ -f $bashPath ] && echo yes").contains("yes")
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
    val envText = ptyLines("echo SHELL=\$SHELL PREFIX=\$PREFIX")
    assertTrue(
        "SHELL must be the prefix shell, got: ${envText.takeLast(400)}",
        envText.contains("/files/usr/bin/"),
    )
    assertTrue(
        "PREFIX must point at usr, got: ${envText.takeLast(400)}",
        envText.contains("PREFIX=/data/data/com.termux/files/usr") ||
            envText.contains("PREFIX=/data/user/0/$packageName/files/usr"),
    )

    // Prefix binaries list and run.
    val lsText = ptyLines("ls \$PREFIX/bin | head -8; echo LS_RC=\$?")
    assertTrue(
        "bin listing must succeed, got: ${lsText.takeLast(400)}",
        lsText.contains("LS_RC=0"),
    )
    assertTrue(
        "bin listing must include bash, got: ${lsText.takeLast(400)}",
        lsText.contains("bash"),
    )
    val bashVersion = ptyLines("bash --version | head -1")
    assertTrue(
        "bash must report GNU version, got: ${bashVersion.takeLast(200)}",
        bashVersion.contains("GNU bash"),
    )
    Log.i(TAG, "termux bootstrap real-terminal verified: ${bashVersion.lines().lastOrNull()}")
  }
}
