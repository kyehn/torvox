package io.torvox.selection

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.github.takahirom.roborazzi.RoborazziOptions
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
import kotlin.math.roundToInt

private val MOCHA = BuiltInThemes.catppuccinMocha
private val ACCENT = Color(0xFF89B4FA)
private val SELECTION_BG = Color(0xFF45475A)
private const val CELL_WIDTH = 12f
private const val CELL_HEIGHT = 20f

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [33], application = android.app.Application::class)
class SelectionScreenshotTest {
    @get:Rule
    val roborazziRule =
        RoborazziRule(
            options =
            RoborazziRule.Options(
                roborazziOptions =
                RoborazziOptions(
                    compareOptions =
                    RoborazziOptions.CompareOptions(
                        changeThreshold = 1.0f,
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
    fun selection_singleChar_highlighted() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_char") {
                    TerminalLine(
                        text = "hello world",
                        selectionStart = 3,
                        selectionEnd = 4,
                    )
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_char")
            .captureRoboImage()
    }

    @Test
    fun selection_word_highlighted() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_word") {
                    TerminalLine(
                        text = "hello world",
                        selectionStart = 0,
                        selectionEnd = 5,
                    )
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_word")
            .captureRoboImage()
    }

    @Test
    fun selection_line_highlighted() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_line") {
                    Column {
                        TerminalLine("export PATH=\$HOME/bin:\$PATH", 0, 31)
                        TerminalLine("cd projects/torvox", 0, 18)
                        TerminalLine(
                            "cargo build --release",
                            selectionStart = 0,
                            selectionEnd = 22,
                        )
                        TerminalLine("echo \"done\"", 0, 11)
                    }
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_line")
            .captureRoboImage()
    }

    @Test
    fun selection_block_highlighted() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_block") {
                    Column {
                        TerminalLine("function hello() {", 10, 16)
                        TerminalLine("  print(\"hello\");", 10, 16)
                        TerminalLine("  print(\"world\");", 10, 16)
                        TerminalLine("}", 0, 1)
                    }
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_block")
            .captureRoboImage()
    }

    @Test
    fun selection_url_highlighted() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_url") {
                    Column {
                        TerminalLine("Cloning into 'torvox'...", 0, 0)
                        TerminalLine(
                            "https://github.com/user/torvox.git",
                            selectionStart = 0,
                            selectionEnd = 35,
                        )
                        TerminalLine("Receiving objects:  42%", 0, 0)
                    }
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_url")
            .captureRoboImage()
    }

    @Test
    fun pasteChipOverlay_rendered() {
        composeTestRule.setContent {
            MaterialTheme {
                PasteChipPreview(
                    tag = "paste_chip",
                    row = 2,
                    col = 4,
                )
            }
        }
        composeTestRule
            .onNodeWithTag("paste_chip")
            .captureRoboImage()
    }

    @Test
    fun selectionHandles_visualIndicators() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_handles") {
                    Column {
                        Spacer(modifier = Modifier.height(40.dp))
                        TerminalLine(
                            "select this text",
                            selectionStart = 7,
                            selectionEnd = 11,
                        )
                    }
                    Box(
                        modifier =
                        Modifier
                            .offset { IntOffset(7 * CELL_WIDTH.roundToInt(), 0) }
                            .size(14.dp, 24.dp)
                            .background(ACCENT.copy(alpha = 0.3f), RoundedCornerShape(topStart = 4.dp, bottomStart = 4.dp))
                            .border(1.dp, ACCENT, RoundedCornerShape(topStart = 4.dp, bottomStart = 4.dp)),
                    )
                    Box(
                        modifier =
                        Modifier
                            .offset { IntOffset(11 * CELL_WIDTH.roundToInt() - 14, 0) }
                            .size(14.dp, 24.dp)
                            .background(ACCENT.copy(alpha = 0.3f), RoundedCornerShape(topEnd = 4.dp, bottomEnd = 4.dp))
                            .border(1.dp, ACCENT, RoundedCornerShape(topEnd = 4.dp, bottomEnd = 4.dp)),
                    )
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_handles")
            .captureRoboImage()
    }

    @Test
    fun selectionContextMenu_toolbar() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_toolbar") {
                    Column {
                        Spacer(modifier = Modifier.height(48.dp))
                        TerminalLine(
                            "selected text appears here",
                            selectionStart = 0,
                            selectionEnd = 8,
                        )
                    }
                    ContextMenuToolbar()
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_toolbar")
            .captureRoboImage()
    }

    @Test
    fun selectionMultiLine_highlighted() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_multiline") {
                    Column {
                        TerminalLine("first line here", 0, 6)
                        TerminalLine("second line content", 0, 8)
                        TerminalLine("third line data", 0, 6)
                        TerminalLine("fourth", 0, 0)
                    }
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_multiline")
            .captureRoboImage()
    }

    @Test
    fun selection_wrappedHighlight_differentRows() {
        composeTestRule.setContent {
            MaterialTheme {
                SelectionPreviewBox(tag = "selection_wrapped") {
                    Column {
                        TerminalLine(
                            "this is a very long line that wraps across multiple display",
                            6,
                            50,
                        )
                        TerminalLine(
                            "rows because the terminal width cannot contain it all",
                            0,
                            20,
                        )
                        TerminalLine("short", 0, 0)
                    }
                }
            }
        }
        composeTestRule
            .onNodeWithTag("selection_wrapped")
            .captureRoboImage()
    }
}

@Composable
private fun SelectionPreviewBox(
    tag: String,
    content: @Composable () -> Unit,
) {
    Box(
        modifier =
        Modifier
            .fillMaxSize()
            .background(MOCHA.background)
            .padding(12.dp)
            .testTag(tag),
    ) {
        Box(
            modifier =
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(6.dp))
                .background(Color(0xFF11111B))
                .padding(8.dp),
        ) {
            content()
        }
    }
}

@Composable
private fun TerminalLine(
    text: String,
    selectionStart: Int = 0,
    selectionEnd: Int = 0,
) {
    val safeStart = selectionStart.coerceIn(0, text.length)
    val safeEnd = selectionEnd.coerceIn(0, text.length)
    Row(
        modifier =
        Modifier
            .fillMaxWidth()
            .height(CELL_HEIGHT.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val promptColor = Color(0xFFA6E3A1)
        Text(
            text = "$ ",
            color = promptColor,
            fontFamily = FontFamily.Monospace,
            fontSize = 12.sp,
        )
        if (safeStart < safeEnd) {
            if (safeStart > 0) {
                Text(
                    text = text.substring(0, safeStart),
                    color = MOCHA.foreground,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 12.sp,
                )
            }
            Box(
                modifier =
                Modifier
                    .background(SELECTION_BG, RoundedCornerShape(2.dp))
                    .padding(horizontal = 1.dp),
            ) {
                Text(
                    text = text.substring(safeStart, safeEnd),
                    color = MOCHA.foreground,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 12.sp,
                )
            }
            if (safeEnd < text.length) {
                Text(
                    text = text.substring(safeEnd),
                    color = MOCHA.foreground,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 12.sp,
                )
            }
        } else {
            Text(
                text = text,
                color = MOCHA.foreground,
                fontFamily = FontFamily.Monospace,
                fontSize = 12.sp,
            )
        }
    }
}

@Composable
private fun PasteChipPreview(
    tag: String,
    row: Int,
    col: Int,
) {
    Box(
        modifier =
        Modifier
            .fillMaxSize()
            .background(MOCHA.background)
            .padding(12.dp)
            .testTag(tag),
    ) {
        Box(
            modifier =
            Modifier
                .fillMaxWidth()
                .height(120.dp)
                .clip(RoundedCornerShape(6.dp))
                .background(Color(0xFF11111B))
                .padding(8.dp),
        ) {
            Box(
                modifier =
                Modifier
                    .offset {
                        IntOffset(
                            (col * CELL_WIDTH).roundToInt(),
                            ((row + 1) * CELL_HEIGHT + 4f).roundToInt(),
                        )
                    }.clip(RoundedCornerShape(6.dp))
                    .background(SELECTION_BG)
                    .border(1.dp, ACCENT, RoundedCornerShape(6.dp))
                    .clickable { }
                    .padding(horizontal = 12.dp, vertical = 6.dp),
            ) {
                Text(
                    text = "Paste",
                    color = ACCENT,
                    fontSize = 12.sp,
                )
            }
        }
    }
}

@Composable
private fun ContextMenuToolbar() {
    Row(
        modifier =
        Modifier
            .fillMaxWidth()
            .padding(top = 4.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(Color(0xFF2E2E3E))
            .padding(4.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
    ) {
        val actions =
            listOf(
                "Copy" to ACCENT,
                "Select All" to MOCHA.foreground,
                "Paste" to ACCENT,
            )
        actions.forEach { (label, color) ->
            Box(
                modifier =
                Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .clickable { }
                    .padding(horizontal = 12.dp, vertical = 8.dp),
            ) {
                Text(
                    text = label,
                    color = color,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Medium,
                )
            }
        }
    }
}
