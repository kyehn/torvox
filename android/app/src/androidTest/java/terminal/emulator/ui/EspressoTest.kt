package terminal.emulator.ui

import android.annotation.SuppressLint
import android.view.View
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.UiController
import androidx.test.espresso.ViewAction
import androidx.test.espresso.action.CoordinatesProvider
import androidx.test.espresso.action.GeneralClickAction
import androidx.test.espresso.action.Press
import androidx.test.espresso.action.Tap
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import androidx.test.espresso.matcher.ViewMatchers.withClassName
import androidx.test.espresso.matcher.ViewMatchers.withId
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.hamcrest.CoreMatchers.`is`
import org.hamcrest.Matcher
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import terminal.emulator.MainActivity

/**
 * Espresso instrumentation tests.
 *
 * These run on a tablet emulator or real device (the Compose UI is driven through Espresso
 * framework calls, not injected `adb` taps — see AGENTS.md pitfall #15). UIAutomator is only
 * used for the post-click state assertion, where the session drawer exposes a stable resource id.
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class EspressoTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    @Before
    fun grantNotificationsPermission() {
        // MainActivity.onCreate() requests POST_NOTIFICATIONS on Android 13+;
        // the system dialog covers the UI and blocks UiAutomator lookups.
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.uiAutomation.grantRuntimePermission(
            instrumentation.targetContext.packageName,
            android.Manifest.permission.POST_NOTIFICATIONS,
        )
    }

    /** The root content view should be displayed once the activity is launched. */
    @Test
    fun contentViewIsDisplayedOnLaunch() {
        onView(withId(android.R.id.content))
            .check(matches(isDisplayed()))
    }

    /**
     * Clicking the session drawer button (top-left menu) via Espresso should open the session
     * drawer, changing the UI state. The state change is asserted by checking the drawer node
     * becomes present in the view hierarchy.
     */
    @Test
    fun clickingDrawerButtonOpensSessionDrawer() {
        onView(withId(android.R.id.content))
            .check(matches(isDisplayed()))

        // The modifier bar (and its drawer button) only exists after the
        // first Compose frame renders; on the software-GPU emulator that
        // can take several seconds, and an early coordinate tap lands on
        // nothing.
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        device.wait(Until.hasObject(By.desc("Open session drawer")), 15000)

        onView(withClassName(`is`("androidx.compose.ui.platform.AndroidComposeView")))
            .perform(clickTopLeftMenu())

        val drawer =
            device.wait(Until.findObject(By.res("SessionDrawer")), 5000)
        assertNotNull("Session drawer should be visible after clicking the menu button", drawer)
    }

    private fun clickTopLeftMenu(): ViewAction {
        val menuCoordinates =
            CoordinatesProvider { view: View ->
                val location = IntArray(2)
                view.getLocationOnScreen(location)
                floatArrayOf(
                    location[0] + view.width * MENU_BUTTON_RELATIVE_X,
                    location[1] + view.height * MENU_BUTTON_RELATIVE_Y,
                )
            }

        // GeneralClickAction is deprecated but remains the only way to
        // click at explicit coordinates; the replacement suggestions are
        // all coordinate-less.
        @SuppressLint("DeprecatedCall")
        @Suppress("DEPRECATION")
        fun createClickAction() = GeneralClickAction(Tap.SINGLE, menuCoordinates, Press.FINGER)
        return androidx.test.espresso.action.ViewActions.actionWithAssertions(
            createClickAction(),
        )
    }

    private companion object {
        // The drawer launcher lives in the bottom modifier bar (Key_DRAWER),
        // not the top-left corner this test predates. Relative position of a
        // ~155x95 dp button at ~[155,2147]/[1080,2400] on the phone emulator.
        const val MENU_BUTTON_RELATIVE_X = 0.21f

        // ModifierBar is anchored to the bottom of the window (bottom modifier
        // bar), and the drawer button sits at ~91.5% of the window height
        // (bounds [155,2147][310,2242] on the 1080x2400 emulator).
        const val MENU_BUTTON_RELATIVE_Y = 0.915f
    }
}
