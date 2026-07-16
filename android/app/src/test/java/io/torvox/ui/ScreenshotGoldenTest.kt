package io.torvox.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import com.github.takahirom.roborazzi.RoborazziRule
import com.github.takahirom.roborazzi.captureRoboImage
import io.torvox.RobolectricActivityRule
import io.torvox.TestActivity
import io.torvox.ui.theme.BuiltInThemes
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import kotlin.coroutines.EmptyCoroutineContext

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = android.app.Application::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ScreenshotGoldenTest {
    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    @get:Rule
    val roborazziRule =
        RoborazziRule(
            RoborazziRule.Options(
                outputDirectoryPath = "src/test/resources/roborazzi",
            ),
        )

    @Test
    fun modifierBar_screenshot() {
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(onKeyClick = {})
            }
        }
        composeTestRule.onNodeWithTag("ModifierBar").captureRoboImage()
    }

    @Test
    fun modifierBar_ctrlActive_screenshot() {
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(onKeyClick = {})
            }
        }
        composeTestRule.onNodeWithTag("Key_CTRL").performClick()
        composeTestRule.onNodeWithTag("ModifierBar").captureRoboImage()
    }

    @Test
    fun modifierBar_altActive_screenshot() {
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(onKeyClick = {})
            }
        }
        composeTestRule.onNodeWithTag("Key_ALT").performClick()
        composeTestRule.onNodeWithTag("ModifierBar").captureRoboImage()
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Test
    fun terminalScreen_placeholder_screenshot() {
        composeTestRule.setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    Column(Modifier.fillMaxSize()) {
                        TopAppBar(
                            title = {
                                Text(
                                    "Torvox",
                                    modifier = Modifier.testTag("TerminalTitle"),
                                )
                            },
                        )
                        Text(
                            "Terminal output would render here via wgpu + Vulkan",
                            modifier = Modifier.weight(1f),
                        )
                        ModifierBar(
                            modifier = Modifier.testTag("ModifierBar"),
                            onKeyClick = {},
                        )
                    }
                }
            }
        }
        composeTestRule.onNodeWithTag("TerminalTitle").captureRoboImage()
    }

    @Test
    fun allThemes_screenshot() {
        var currentTheme by mutableStateOf(BuiltInThemes.catppuccinMocha)
        composeTestRule.setContent {
            MaterialTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = currentTheme.background,
                ) {
                    Column(Modifier.fillMaxSize()) {
                        Text(
                            text = "Terminal Output",
                            color = currentTheme.foreground,
                        )
                        Text(
                            text = "Red Green Yellow Blue",
                            color = currentTheme.ansi[1],
                        )
                    }
                }
            }
        }
        for (theme in BuiltInThemes.all) {
            currentTheme = theme
            composeTestRule.onRoot().captureRoboImage("theme_${theme.name}")
        }
    }
}
