
package terminal.emulator.cucumber

import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import io.cucumber.junit.WithJunitRule
import org.junit.Rule
import terminal.emulator.MainActivity
import terminal.emulator.grantNotificationPermission

@WithJunitRule
class ComposeRuleHolder {
    init {
        // MainActivity.onCreate() requests POST_NOTIFICATIONS on first run
        // (Android 13+); the permission dialog overlays the activity and
        // blocks every UI test's waitForSession(). Grant it before the
        // activity launches so the dialog never appears. Idempotent.
        // Production behavior is untouched — this runs only in the test APK.
        grantNotificationPermission()
    }

    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()
}
