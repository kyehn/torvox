package io.torvox.screenshot

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.RoborazziRule
import com.github.takahirom.roborazzi.captureRoboImage
import io.torvox.RobolectricActivityRule
import io.torvox.TestActivity
import io.torvox.ui.ModifierBar
import io.torvox.ui.theme.BuiltInThemes
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
class TerminalScreenScreenshotTest {
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
    fun terminalScreen_simulatedPromptRenders() {
        composeTestRule.setContent {
            MaterialTheme {
                Box(
                    modifier =
                    Modifier
                        .fillMaxSize()
                        .background(BuiltInThemes.catppuccinMocha.background)
                        .testTag("screenshot_terminal"),
                ) {
                    Column(modifier = Modifier.fillMaxSize()) {
                        Box(
                            modifier =
                            Modifier
                                .weight(1f)
                                .fillMaxWidth()
                                .background(BuiltInThemes.catppuccinMocha.background),
                        ) {
                            Column(modifier = Modifier.padding(16.dp)) {
                                Text(
                                    text = "total 42",
                                    color = BuiltInThemes.catppuccinMocha.foreground,
                                    fontFamily = FontFamily.Monospace,
                                    fontSize = 14.sp,
                                )
                            }
                        }
                    }
                }
            }
        }
        composeTestRule.onNodeWithText("total 42").assertIsDisplayed()
        composeTestRule
            .onNodeWithTag("screenshot_terminal")
            .captureRoboImage()
    }

    @Test
    fun modifierBar_allKeysExist() {
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = {},
                    modifier = Modifier.testTag("screenshot_modifier_bar"),
                )
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
        composeTestRule
            .onNodeWithTag("screenshot_modifier_bar")
            .captureRoboImage()
    }
}
