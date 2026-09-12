package terminal.emulator.ui

import android.content.ClipData
import android.content.Context
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
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
import terminal.emulator.waitForSession

/**
 * End-to-end OSC 52 clipboard write with a real assertion.
 *
 * Feeds `ESC ] 52 ; c ; <base64> BEL` through the REAL terminal input
 * ([writeToPty][terminal.emulator.bridge.Bridge.writeToPty]) and asserts the Android system
 * clipboard contains the decoded marker — Ghostty ClipboardEvent → JNI → ClipboardManager, no
 * screenshots, no OCR.
 */
@RunWith(JUnit4::class)
class Osc52ClipboardInstrumentedTest {
  companion object {
    private const val OUTPUT_TIMEOUT_MS = 15_000L
  }

  @get:Rule
  val notificationPermission =
      GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

  @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

  @Test
  fun osc52_sequence_sets_system_clipboard() {
    composeTestRule.waitForSession()
    val bridge = composeTestRule.getBridge() ?: throw AssertionError("bridge null")
    val activity = composeTestRule.activity
    val clipboard =
        activity.getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
    // Clear first so the assertion cannot pass on stale content.
    activity.runOnUiThread { clipboard.setPrimaryClip(ClipData.newPlainText("test", "")) }

    val marker = "OSC52_ALIVE_${System.currentTimeMillis() % 100000}"
    val encoded =
        android.util.Base64.encodeToString(
            marker.toByteArray(Charsets.UTF_8),
            android.util.Base64.NO_WRAP,
        )
    val sequence = "\u001b]52;c;$encoded\u0007"
    assertTrue(
        "PTY write rejected",
        bridge.writeToPty(sequence.toByteArray(Charsets.UTF_8)),
    )
    val seen =
        UxTestUtils.pollUntilTrue(timeoutMs = OUTPUT_TIMEOUT_MS, intervalMs = 100) {
          clipboard.primaryClip?.getItemAt(0)?.text?.toString() == marker
        }
    val actual = clipboard.primaryClip?.getItemAt(0)?.text?.toString()
    assertNotNull("clipboard never received OSC52 marker $marker (got: $actual)", seen)
    assertTrue(
        "clipboard must equal marker, got: $actual",
        actual == marker,
    )
  }
}
