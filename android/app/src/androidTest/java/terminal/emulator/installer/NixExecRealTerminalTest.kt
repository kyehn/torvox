package terminal.emulator.installer

import android.util.Log
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4
import terminal.emulator.MainActivity
import terminal.emulator.ShizukuGate
import terminal.emulator.UxTestUtils
import terminal.emulator.getBridge
import terminal.emulator.openDrawer
import terminal.emulator.waitForSession

/**
 * Real-terminal verification of the shizuku + nix bootstrap path.
 *
 * Installs [ZIP_NAME] through the app's own install entry, enables the Shizuku switch through the
 * product ViewModel path (which fires the manager authorization prompt, approved here via UI), then
 * drives the privileged session with REAL terminal input
 * ([writeToPty][terminal.emulator.bridge.Bridge.writeToPty]) and asserts on REAL terminal output
 * ([getTerminalText][terminal.emulator.bridge.Bridge.getTerminalText]): `nix --version` and `proot
 * --version` must report versions.
 *
 * No `adb shell run-as` command execution, no screenshots, no OCR: the shell uid is used only for
 * install setup/observation, never for running validation commands — every validated byte travels
 * through the PTY.
 *
 * Hard fail (never skip) when prerequisites are absent (zip not staged, Shizuku server not running,
 * authorization denied).
 */
@RunWith(JUnit4::class)
class NixExecRealTerminalTest {
  companion object {
    private const val TAG = "NixExecTest"
    private const val ZIP_NAME = "nix-bootstrap-x86_64.zip"
    private const val MAIN_ACTIVITY = "terminal.emulator.MainActivity"
    private const val INSTALL_EXTRA = "terminal.emulator.install_bootstrap"
    private const val INSTALL_TIMEOUT_MS = 15 * 60_000L
    private const val OUTPUT_TIMEOUT_MS = 60_000L
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

  @Test
  fun shizukuNixBootstrap_reports_nix_and_proot_versions_over_real_terminal() {
    val ctx = InstrumentationRegistry.getInstrumentation().targetContext
    val packageName = ctx.packageName
    val zipPath = "/sdcard/Download/$ZIP_NAME"
    val loginPath = "/data/user/0/$packageName/files/usr/bin/login"
    assertTrue(
        "bootstrap zip must be staged first: adb push <nix bootstrap-x86_64.zip> $zipPath",
        shell("ls $zipPath").isNotBlank(),
    )
    assertTrue(
        "Shizuku server must be running (start it from the Shizuku manager first)",
        ShizukuGate.isAuthorized() ||
            shell("ps -A | grep shizuku_server").contains("shizuku_server"),
    )

    // Install through the real app process. The installer atomically
    // replaces usr/, so re-running over a previous install is fine.
    val startOut = shell("am start -n $packageName/$MAIN_ACTIVITY --es $INSTALL_EXTRA $zipPath")
    Log.i(TAG, "am start output: $startOut")
    val deadline = System.currentTimeMillis() + INSTALL_TIMEOUT_MS
    var present = false
    while (System.currentTimeMillis() < deadline) {
      present = shell("[ -f $loginPath ] && echo yes").contains("yes")
      if (present) break
      Thread.sleep(5_000L)
    }
    assertTrue("usr/bin/login missing within timeout (logcat BootstrapInstallService)", present)

    // Enable the Shizuku switch through the product path (persists +
    // fires the manager authorization prompt), then approve the prompt.
    composeTestRule.waitForSession()
    composeTestRule.activityRule.scenario.onActivity { activity ->
      (activity as MainActivity).terminalViewModel.setShizukuEnabled(true)
    }
    approveShizukuPrompt()
    assertTrue(
        "Shizuku must be authorized for $packageName (approve in the manager)",
        ShizukuGate.isAuthorized(),
    )

    // Fresh session AFTER install + authorization: the entry must be
    // the Shizuku wrapper running bin/login (product path via drawer).
    composeTestRule.openDrawer()
    composeTestRule.onNodeWithTag("AddSessionButton").performClick()
    composeTestRule.waitForIdle()
    val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null after add")

    // Guest shell alive?
    val marker = "NIX_ALIVE_${System.currentTimeMillis() % 100000}"
    val markerSeen =
        UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
          bridge.writeToPty("echo $marker\n".toByteArray(Charsets.UTF_8))
          bridge.getTerminalText()?.contains(marker) == true
        }
    assertNotNull("privileged guest shell never echoed marker $marker", markerSeen)

    // THE verdict: nix --version over the real terminal.
    bridge.writeToPty("nix --version\n".toByteArray(Charsets.UTF_8))
    val nixSeen =
        UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
          bridge.getTerminalText()?.contains("nix (Nix)") == true
        }
    val nixText = bridge.getTerminalText().orEmpty()
    Log.i(TAG, "nix --version terminal output tail: ${nixText.takeLast(300)}")
    assertNotNull("nix --version never reported 'nix (Nix)' over the terminal", nixSeen)

    // Static proot must report its version too.
    bridge.writeToPty("proot --version\n".toByteArray(Charsets.UTF_8))
    val prootSeen =
        UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
          bridge.getTerminalText()?.contains("proot") == true
        }
    val prootText = bridge.getTerminalText().orEmpty()
    Log.i(TAG, "proot --version terminal output tail: ${prootText.takeLast(300)}")
    assertNotNull("proot --version never reported over the terminal", prootSeen)
    assertTrue(
        "proot version line must appear, got: ${prootText.takeLast(300)}",
        prootText.lines().any { it.trimStart().startsWith("proot ") || it.contains("PRoot") },
    )
  }

  /** Approve the Shizuku manager authorization prompt if it appears. */
  private fun approveShizukuPrompt() {
    val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    val deadline = System.currentTimeMillis() + 30_000L
    while (System.currentTimeMillis() < deadline) {
      if (ShizukuGate.isAuthorized()) return
      val allow =
          runCatching {
                when {
                  device.hasObject(By.text("Allow")) -> device.findObject(By.text("Allow"))
                  device.hasObject(By.text("ALLOW")) -> device.findObject(By.text("ALLOW"))
                  device.hasObject(By.text("始终允许")) -> device.findObject(By.text("始终允许"))
                  else -> null
                }
              }
              .getOrNull()
      if (allow != null) {
        runCatching { allow.click() }
        Thread.sleep(1_000L)
      } else {
        Thread.sleep(500L)
      }
    }
  }
}
