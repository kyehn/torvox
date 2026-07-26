package io.term.screenshot

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
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
class ThemeScreenshotTest {
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

    @Test
    fun allThemesRenderCorrectly() {
        composeTestRule.setContent {
            MaterialTheme {
                Column(
                    modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1E1E2E))
                        .padding(16.dp)
                        .verticalScroll(rememberScrollState())
                        .testTag("all_themes_screenshot"),
                ) {
                    Text(
                        text = "All Terminal Themes",
                        color = Color.White,
                        fontSize = 18.sp,
                        fontWeight = FontWeight.Bold,
                    )
                    Spacer(modifier = Modifier.height(16.dp))

                    BuiltInThemes.all.chunked(4).forEach { row ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            row.forEach { theme ->
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    modifier =
                                    Modifier
                                        .width(72.dp)
                                        .clip(RoundedCornerShape(8.dp))
                                        .background(theme.background)
                                        .padding(6.dp),
                                ) {
                                    Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                                        theme.ansi.take(16).forEach { color ->
                                            Box(
                                                modifier =
                                                Modifier
                                                    .size(6.dp)
                                                    .clip(RoundedCornerShape(1.dp))
                                                    .background(color),
                                            )
                                        }
                                    }
                                    Spacer(modifier = Modifier.height(4.dp))
                                    Text(
                                        text = theme.name,
                                        color = theme.foreground,
                                        fontSize = 7.sp,
                                        maxLines = 2,
                                    )
                                }
                            }
                        }
                        Spacer(modifier = Modifier.height(8.dp))
                    }
                }
            }
        }

        composeTestRule
            .onNodeWithTag("all_themes_screenshot")
            .captureRoboImage()
    }

    @Test
    fun dayThemePreviewShowsCorrectColors() {
        val dayTheme = BuiltInThemes.byName("Catppuccin Latte")

        composeTestRule.setContent {
            MaterialTheme {
                Box(
                    modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1E1E2E))
                        .padding(16.dp)
                        .testTag("day_theme_preview"),
                ) {
                    Column {
                        Text(
                            text = "Day Theme: ${dayTheme.name}",
                            color = Color.White,
                            fontSize = 16.sp,
                            fontWeight = FontWeight.Bold,
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        Box(
                            modifier =
                            Modifier
                                .fillMaxWidth()
                                .height(100.dp)
                                .clip(RoundedCornerShape(8.dp))
                                .background(dayTheme.background)
                                .padding(12.dp),
                        ) {
                            Text(
                                text = "Sample Text",
                                color = dayTheme.foreground,
                                fontSize = 16.sp,
                            )
                        }
                        Spacer(modifier = Modifier.height(12.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            dayTheme.ansi.forEachIndexed { index, color ->
                                Box(
                                    modifier =
                                    Modifier
                                        .size(20.dp)
                                        .clip(RoundedCornerShape(4.dp))
                                        .background(color),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        text = "$index",
                                        color = if (index < 8) Color.Black else Color.White,
                                        fontSize = 8.sp,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }

        composeTestRule
            .onNodeWithTag("day_theme_preview")
            .captureRoboImage()
    }

    @Test
    fun nightThemePreviewShowsCorrectColors() {
        val nightTheme = BuiltInThemes.byName("Dracula Plus")

        composeTestRule.setContent {
            MaterialTheme {
                Box(
                    modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1E1E2E))
                        .padding(16.dp)
                        .testTag("night_theme_preview"),
                ) {
                    Column {
                        Text(
                            text = "Night Theme: ${nightTheme.name}",
                            color = Color.White,
                            fontSize = 16.sp,
                            fontWeight = FontWeight.Bold,
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        Box(
                            modifier =
                            Modifier
                                .fillMaxWidth()
                                .height(100.dp)
                                .clip(RoundedCornerShape(8.dp))
                                .background(nightTheme.background)
                                .padding(12.dp),
                        ) {
                            Text(
                                text = "Sample Text",
                                color = nightTheme.foreground,
                                fontSize = 16.sp,
                            )
                        }
                        Spacer(modifier = Modifier.height(12.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            nightTheme.ansi.forEachIndexed { index, color ->
                                Box(
                                    modifier =
                                    Modifier
                                        .size(20.dp)
                                        .clip(RoundedCornerShape(4.dp))
                                        .background(color),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        text = "$index",
                                        color = if (index < 8) Color.Black else Color.White,
                                        fontSize = 8.sp,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }

        composeTestRule
            .onNodeWithTag("night_theme_preview")
            .captureRoboImage()
    }
}
