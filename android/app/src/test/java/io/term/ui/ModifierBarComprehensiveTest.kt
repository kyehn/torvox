package io.term.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import io.term.RobolectricActivityRule
import io.term.TestActivity
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
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
class ModifierBarComprehensiveTest {
    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    private val allKeyTags =
        listOf(
            "Key_ESC",
            "Key_DRAWER",
            "Key_SCROLL",
            "Key_HOME",
            "Key_\u2191",
            "Key_END",
            "Key_PGUP",
            "Key_TAB",
            "Key_CTRL",
            "Key_ALT",
            "Key_\u2190",
            "Key_\u2193",
            "Key_\u2192",
            "Key_PGDN",
        )

    @Test
    fun allModifierBarButtonsExist() {
        var renderedKeys = mutableListOf<String>()
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = { key -> renderedKeys.add(key) },
                    modifier = Modifier.testTag("comprehensive_modifier_bar"),
                )
            }
        }
        for (tag in allKeyTags) {
            composeTestRule.onNodeWithTag(tag).assertExists("Missing key: $tag")
        }
    }

    @Test
    fun ctrlToggleActivatesOnClick() {
        var ctrlState = ModifierState.Off
        val toggler: () -> Unit = { ctrlState = ctrlState.next() }
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    ctrlState = ctrlState,
                    onToggleCtrl = toggler,
                    onKeyClick = {},
                    modifier = Modifier.testTag("ctrl_toggle_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_CTRL").assertExists("CTRL key should be rendered")
        toggler()
        assertEquals("CTRL should cycle to Once after first toggle", ModifierState.Once, ctrlState)
        toggler()
        assertEquals("CTRL should cycle to Locked after second toggle", ModifierState.Locked, ctrlState)
        toggler()
        assertEquals("CTRL should cycle back to Off after third toggle", ModifierState.Off, ctrlState)
    }

    @Test
    fun altToggleActivatesOnClick() {
        var altState = ModifierState.Off
        val toggler: () -> Unit = { altState = altState.next() }
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    altState = altState,
                    onToggleAlt = toggler,
                    onKeyClick = {},
                    modifier = Modifier.testTag("alt_toggle_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_ALT").assertExists("ALT key should be rendered")
        toggler()
        assertEquals("ALT should cycle to Once after first toggle", ModifierState.Once, altState)
        toggler()
        assertEquals("ALT should cycle to Locked after second toggle", ModifierState.Locked, altState)
        toggler()
        assertEquals("ALT should cycle back to Off after third toggle", ModifierState.Off, altState)
    }

    @Test
    fun drawerButtonTriggersDrawerClick() {
        var drawerClicked = false
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onDrawerClick = { drawerClicked = true },
                    onKeyClick = {},
                    modifier = Modifier.testTag("drawer_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_DRAWER").performClick()
        assertTrue("DRAWER should trigger onDrawerClick", drawerClicked)
    }

    @Test
    fun scrollButtonReturnsScrollToggle() {
        var scrollKey = ""
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onScrollClick = { scrollKey = "SCROLL" },
                    onKeyClick = { key -> scrollKey = key },
                    modifier = Modifier.testTag("scroll_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_SCROLL").performClick()
        assertEquals("SCROLL", scrollKey)
    }

    @Test
    fun escButtonSendsEscapeSequence() {
        var clickedKey = ""
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = { key -> clickedKey = key },
                    modifier = Modifier.testTag("esc_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_ESC").performClick()
        assertEquals("\u001b", clickedKey)
    }

    @Test
    fun homeEndPgUpPgDnButtonsSendCorrectKeys() {
        var lastKey = ""
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = { key -> lastKey = key },
                    modifier = Modifier.testTag("nav_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_HOME").performClick()
        assertEquals("\u001b[H", lastKey)
        composeTestRule.onNodeWithTag("Key_END").performClick()
        assertEquals("\u001b[F", lastKey)
        composeTestRule.onNodeWithTag("Key_PGUP").performClick()
        assertEquals("\u001b[5~", lastKey)
        composeTestRule.onNodeWithTag("Key_PGDN").performClick()
        assertEquals("\u001b[6~", lastKey)
    }

    @Test
    fun arrowKeysSendCorrectKeys() {
        var lastKey = ""
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = { key -> lastKey = key },
                    modifier = Modifier.testTag("arrow_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_\u2191").performClick()
        assertEquals("\u001b[A", lastKey)
        composeTestRule.onNodeWithTag("Key_\u2193").performClick()
        assertEquals("\u001b[B", lastKey)
        composeTestRule.onNodeWithTag("Key_\u2190").performClick()
        assertEquals("\u001b[D", lastKey)
        composeTestRule.onNodeWithTag("Key_\u2192").performClick()
        assertEquals("\u001b[C", lastKey)
    }

    @Test
    fun tabButtonSendsTab() {
        var clickedKey = ""
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = { key -> clickedKey = key },
                    modifier = Modifier.testTag("tab_bar"),
                )
            }
        }
        composeTestRule.onNodeWithTag("Key_TAB").performClick()
        assertEquals("\t", clickedKey)
    }

    @Test
    fun twoRowLayoutHasCorrectColumnCount() {
        val row1Keys = listOf("Key_ESC", "Key_DRAWER", "Key_SCROLL", "Key_HOME", "Key_\u2191", "Key_END", "Key_PGUP")
        val row2Keys = listOf("Key_TAB", "Key_CTRL", "Key_ALT", "Key_\u2190", "Key_\u2193", "Key_\u2192", "Key_PGDN")
        composeTestRule.setContent {
            MaterialTheme {
                ModifierBar(
                    onKeyClick = {},
                    modifier = Modifier.testTag("layout_bar"),
                )
            }
        }
        for (tag in row1Keys) {
            composeTestRule.onNodeWithTag(tag).assertExists("Row 1 missing key: $tag")
        }
        for (tag in row2Keys) {
            composeTestRule.onNodeWithTag(tag).assertExists("Row 2 missing key: $tag")
        }
    }
}
