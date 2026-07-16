package io.torvox.screenshot

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.RoborazziRule
import com.github.takahirom.roborazzi.captureRoboImage
import io.torvox.RobolectricActivityRule
import io.torvox.TestActivity
import io.torvox.ui.ModifierBar
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import kotlin.coroutines.EmptyCoroutineContext

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [33], application = android.app.Application::class)
class ModifierBarScreenshotTest {
    @get:Rule
    val roborazziRule =
        RoborazziRule(
            options =
            RoborazziRule.Options(
                roborazziOptions =
                RoborazziOptions(
                    compareOptions =
                    RoborazziOptions.CompareOptions(
                        changeThreshold = 0.05f,
                    ),
                ),
            ),
        )

    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    @Test
    fun modifierBar_allKeyLabelsExist() {
        composeTestRule.setContent {
            MaterialTheme {
                Box(
                    modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1E1E2E))
                        .testTag("screenshot_modifier_bar"),
                ) {
                    ModifierBar(
                        onKeyClick = {},
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
        composeTestRule.onNodeWithText("ESC").assertIsDisplayed()
        composeTestRule.onNodeWithText("CTRL").assertIsDisplayed()
        composeTestRule.onNodeWithText("ALT").assertIsDisplayed()
        composeTestRule.onNodeWithText("TAB").assertIsDisplayed()
        composeTestRule.onNodeWithText("HOME").assertIsDisplayed()
        composeTestRule.onNodeWithText("END").assertIsDisplayed()
        composeTestRule.onNodeWithText("PGUP").assertIsDisplayed()
        composeTestRule.onNodeWithText("PGDN").assertIsDisplayed()
    }

    @Test
    fun modifierBar_captured() {
        composeTestRule.setContent {
            MaterialTheme {
                Box(
                    modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1E1E2E))
                        .testTag("screenshot_modifier_bar"),
                ) {
                    ModifierBar(
                        onKeyClick = {},
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
        composeTestRule
            .onNodeWithTag("screenshot_modifier_bar")
            .captureRoboImage()
    }
}
