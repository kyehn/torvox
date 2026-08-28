package terminal.emulator.diag

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.UiDevice
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import terminal.emulator.MainActivity
import terminal.emulator.getBridge
import terminal.emulator.grantNotificationPermission

/** Dumps the terminal grid rows around the last output for echo-path diagnosis. */
class EchoGridDumpTest {
    @get:Rule
    val notificationPermission =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    @get:Rule val composeTestRule = createAndroidComposeRule<MainActivity>()

    private lateinit var device: UiDevice

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        grantNotificationPermission()
        composeTestRule.waitUntil(timeoutMillis = 60_000) {
            try {
                composeTestRule.onNodeWithTag("TerminalScreen").assertIsDisplayed()
                true
            } catch (_: Exception) {
                false
            }
        }
        val deadline = System.currentTimeMillis() + 20_000
        while (System.currentTimeMillis() < deadline) {
            if (!bridge().getTerminalText().isNullOrBlank()) break
            Thread.sleep(200)
        }
        Thread.sleep(1_000)
    }

    private fun bridge() = composeTestRule.getBridge() ?: throw AssertionError("bridge null")

    @Test
    fun dumpGridRows() {
        val b = bridge()
        // Drive the input through the REAL system key-event pipeline (same as
        // `adb shell input text` / hardware keyboard): Instrumentation injects
        // KeyEvents into the focused window, exercising onKeyDown → writeToPty.
        val instrumentation = androidx.test.platform.app.InstrumentationRegistry.getInstrumentation()
        instrumentation.sendStringSync("echo abcdefghijklmnopqrstuvwxyz")
        instrumentation.sendCharacterSync(android.view.KeyEvent.KEYCODE_ENTER)
        Thread.sleep(1_500)
        val length = b.scrollbackLength()
        // The viewport is the last `rows` grid rows; dump the last 12 rows
        // with explicit column ruler so off-by-one/phantom cells are visible.
        val rows = 48
        val startRow = (length - 12).coerceAtLeast(0)
        for (row in startRow until length) {
            val line = b.scrollbackLine(row) ?: ""
            android.util.Log.d(
                "EchoDump",
                "row=$row abs=[$row] len=${line.length} <$line>",
            )
        }
        // Viewport rows (grid rows length until length+rows)
        for (row in length until (length + 8).coerceAtMost(length + rows)) {
            val line = b.scrollbackLine(row) ?: ""
            android.util.Log.d(
                "EchoDump",
                "row=$row viewport <$line>",
            )
        }
        android.util.Log.d("EchoDump", "scrollbackLength=$length")
    }
}
