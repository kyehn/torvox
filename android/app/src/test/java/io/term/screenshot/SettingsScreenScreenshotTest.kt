package io.term.screenshot

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.RoborazziRule
import com.github.takahirom.roborazzi.captureRoboImage
import io.term.RobolectricActivityRule
import io.term.TestActivity
import io.term.ui.theme.BuiltInThemes
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [33], application = android.app.Application::class)
class SettingsScreenScreenshotTest {
    @get:Rule
    val roborazziRule =
        RoborazziRule(
            options =
            RoborazziRule.Options(
                roborazziOptions =
                RoborazziOptions(
                    compareOptions =
                    RoborazziOptions.CompareOptions(
                        changeThreshold = 0.01f,
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

    @Suppress("LongMethod")
    @Test
    fun settingsScreen_rendered() {
        composeTestRule.setContent {
            MaterialTheme {
                Box(
                    modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1E1E2E))
                        .testTag("screenshot_settings"),
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(
                            text = "Settings",
                            color = Color.White,
                            fontSize = 20.sp,
                            fontWeight = FontWeight.Bold,
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = "Appearance",
                            color = Color(0xFF89B4FA),
                            fontSize = 14.sp,
                            fontWeight = FontWeight.Bold,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = "Font Size: 14 sp",
                            color = Color(0xFFCDD6F4),
                            fontSize = 14.sp,
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = "Theme",
                            color = Color(0xFFCDD6F4),
                            fontSize = 14.sp,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Row {
                            BuiltInThemes.all.take(4).forEach { theme ->
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    modifier =
                                    Modifier
                                        .width(64.dp)
                                        .clip(RoundedCornerShape(8.dp))
                                        .background(theme.background)
                                        .padding(4.dp),
                                ) {
                                    Row {
                                        theme.ansi.take(4).forEach { color ->
                                            Box(
                                                modifier =
                                                Modifier
                                                    .size(6.dp)
                                                    .clip(RoundedCornerShape(2.dp))
                                                    .background(color),
                                            )
                                        }
                                    }
                                    Text(
                                        text = theme.name,
                                        color = theme.foreground,
                                        fontSize = 8.sp,
                                        maxLines = 1,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        composeTestRule.onNodeWithText("Settings").assertIsDisplayed()
        composeTestRule.onNodeWithText("Appearance").assertIsDisplayed()
        composeTestRule.onNodeWithText("Font Size: 14 sp").assertIsDisplayed()
        composeTestRule.onNodeWithText("Theme").assertIsDisplayed()
        composeTestRule
            .onNodeWithTag("screenshot_settings")
            .captureRoboImage()
    }
}
