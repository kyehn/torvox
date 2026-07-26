package io.term.selection

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import io.term.RobolectricActivityRule
import io.term.TestActivity
import io.term.ui.ModifierBar
import io.term.ui.ModifierBarMode
import io.term.ui.PasteChipOverlay
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = android.app.Application::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class SelectionComposeUiTest {
    private val defaultAccent = Color(0xFF2196F3)
    private val defaultBg = Color(0xFF45475A)

    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    @Test
    fun pasteChipOverlay_pasteTextExists() {
        composeTestRule.setContent {
            Box(Modifier.size(800.dp, 1200.dp)) {
                PasteChipOverlay(
                    row = 5,
                    col = 10,
                    cellWidth = 13.5f,
                    cellHeight = 66.8f,
                    scrollOffset = 0,
                    onPaste = {},
                    accentColor = defaultAccent,
                    backgroundColor = defaultBg,
                )
            }
        }
        composeTestRule.onNodeWithText("Paste").assertIsDisplayed()
    }

    @Test
    fun pasteChipOverlay_triggersPasteCallbackOnClick() {
        var pasteCalled = false
        composeTestRule.setContent {
            Box(Modifier.size(800.dp, 1200.dp)) {
                PasteChipOverlay(
                    row = 5,
                    col = 10,
                    cellWidth = 13.5f,
                    cellHeight = 66.8f,
                    scrollOffset = 0,
                    onPaste = { pasteCalled = true },
                    accentColor = defaultAccent,
                    backgroundColor = defaultBg,
                )
            }
        }
        composeTestRule.onNodeWithText("Paste").performClick()
        assertEquals(true, pasteCalled)
    }

    @Test
    fun pasteChipOverlay_withScrollOffset() {
        composeTestRule.setContent {
            Box(Modifier.size(800.dp, 1200.dp)) {
                PasteChipOverlay(
                    row = 2,
                    col = 1,
                    cellWidth = 13.5f,
                    cellHeight = 66.8f,
                    scrollOffset = 1,
                    onPaste = {},
                    accentColor = defaultAccent,
                    backgroundColor = defaultBg,
                )
            }
        }
        composeTestRule.onNodeWithText("Paste").assertIsDisplayed()
    }

    @Test
    fun pasteChipOverlay_negativeVisibleRow() {
        composeTestRule.setContent {
            Box(Modifier.size(800.dp, 1200.dp)) {
                PasteChipOverlay(
                    row = 0,
                    col = 1,
                    cellWidth = 13.5f,
                    cellHeight = 66.8f,
                    scrollOffset = 5,
                    onPaste = {},
                    accentColor = defaultAccent,
                    backgroundColor = defaultBg,
                )
            }
        }
        composeTestRule.onNodeWithText("Paste").assertIsDisplayed()
    }

    @Test
    fun modifierBar_selectionModeShowsCopyButton() {
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.SelectionActions,
                onCopy = {},
                onSelectAll = {},
                onPaste = null,
                onDismiss = {},
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithText("Copy").assertIsDisplayed()
        composeTestRule.onNodeWithText("Select All").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Action_Copy").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Action_SelectAll").assertIsDisplayed()
    }

    @Test
    fun modifierBar_selectionModeShowsPasteWhenClipboardAvailable() {
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.SelectionActions,
                onCopy = {},
                onSelectAll = {},
                onPaste = {},
                onDismiss = {},
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithText("Paste").assertIsDisplayed()
        composeTestRule.onNodeWithTag("Action_Paste").assertIsDisplayed()
    }

    @Test
    fun modifierBar_selectionModeShowsDismissButton() {
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.SelectionActions,
                onCopy = {},
                onSelectAll = {},
                onPaste = null,
                onDismiss = {},
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithTag("Action_Dismiss").assertIsDisplayed()
    }

    @Test
    fun modifierBar_normalModeDoesNotShowSelectionButtons() {
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.Normal,
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithTag("Action_Copy").assertDoesNotExist()
        composeTestRule.onNodeWithTag("Action_SelectAll").assertDoesNotExist()
        composeTestRule.onNodeWithTag("Action_Paste").assertDoesNotExist()
    }

    @Test
    fun modifierBar_copyButtonTriggersCallback() {
        var copyCalled = false
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.SelectionActions,
                onCopy = { copyCalled = true },
                onSelectAll = {},
                onPaste = null,
                onDismiss = {},
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithTag("Action_Copy").performClick()
        assertEquals(true, copyCalled)
    }

    @Test
    fun modifierBar_selectAllButtonTriggersCallback() {
        var selectAllCalled = false
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.SelectionActions,
                onCopy = {},
                onSelectAll = { selectAllCalled = true },
                onPaste = null,
                onDismiss = {},
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithTag("Action_SelectAll").performClick()
        assertEquals(true, selectAllCalled)
    }

    @Test
    fun modifierBar_dismissButtonTriggersCallback() {
        var dismissCalled = false
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.SelectionActions,
                onCopy = {},
                onSelectAll = {},
                onPaste = null,
                onDismiss = { dismissCalled = true },
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithTag("Action_Dismiss").performClick()
        assertEquals(true, dismissCalled)
    }

    @Test
    fun modifierBar_pasteButtonTriggersCallback() {
        var pasteCalled = false
        composeTestRule.setContent {
            ModifierBar(
                onKeyClick = {},
                barMode = ModifierBarMode.SelectionActions,
                onCopy = {},
                onSelectAll = {},
                onPaste = { pasteCalled = true },
                onDismiss = {},
                modifier = Modifier,
            )
        }
        composeTestRule.onNodeWithTag("Action_Paste").performClick()
        assertEquals(true, pasteCalled)
    }
}
