package io.torvox.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.withTimeoutOrNull

private const val BUTTON_HEIGHT_DP = 36
private const val BUTTON_FONT_SIZE_SP = 10
private const val REPEAT_TIMEOUT_MS = 500L

enum class ModifierState { Off, Once, Locked }

enum class ModifierBarMode { Normal, SelectionActions }

fun ModifierState.next(): ModifierState = when (this) {
    ModifierState.Off -> ModifierState.Once
    ModifierState.Once -> ModifierState.Locked
    ModifierState.Locked -> ModifierState.Off
}

data class ModifierKey(
    val key: String,
    val display: String,
    val ctrl: Boolean = false,
    val alt: Boolean = false,
    val isToggle: Boolean = false,
    val isSessionButton: Boolean = false,
) {
    val label: String get() = display
}

val defaultModifierKeys: List<ModifierKey> =
    listOf(
        ModifierKey("ctrl", "CTRL", isToggle = true),
        ModifierKey("alt", "ALT", isToggle = true),
        ModifierKey("esc", "ESC"),
        ModifierKey("tab", "TAB"),
        ModifierKey("up", "\u2191"),
        ModifierKey("down", "\u2193"),
        ModifierKey("left", "\u2190"),
        ModifierKey("right", "\u2192"),
        ModifierKey("pgup", "PGUP"),
        ModifierKey("pgdn", "PGDN"),
    )

@Composable
fun rememberToolbarLayout(): List<ToolbarItem>? {
    val context = LocalContext.current
    val toolbarPreferences = remember { ToolbarPreferences(context) }
    return remember { toolbarPreferences.getLayout() }
}

/**
 * Termux v0.119.0-beta.3 extra_keys layout:
 * Row 1: ESC, DRAWER, SCROLL, HOME, ↑, END, PGUP
 * Row 2: TAB, CTRL, ALT, ←, ↓, →, PGDN
 *
 * Session button (DRAWER) is on the LEFT as the second button.
 * All buttons are borderless with transparent background.
 * Each button has equal weight for uniform sizing.
 */
@Suppress("LongParameterList", "LongMethod")
@Composable
fun ModifierBar(
    onKeyClick: (String) -> Unit,
    onDrawerClick: () -> Unit = {},
    onScrollClick: () -> Unit = {},
    ctrlState: ModifierState = ModifierState.Off,
    altState: ModifierState = ModifierState.Off,
    onToggleCtrl: () -> Unit = {},
    onToggleAlt: () -> Unit = {},
    textColor: Color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.85f),
    backgroundColor: Color = MaterialTheme.colorScheme.surface,
    modifier: Modifier = Modifier,
    useNerdFontGlyphs: Boolean = false,
    toolbarLayout: List<ToolbarItem>? = null,
    barMode: ModifierBarMode = ModifierBarMode.Normal,
    onCopy: (() -> Unit)? = null,
    onSelectAll: (() -> Unit)? = null,
    onPaste: (() -> Unit)? = null,
    onShare: (() -> Unit)? = null,
    onDismiss: (() -> Unit)? = null,
) {
    fun label(key: String): String = if (useNerdFontGlyphs) NerdKeyLabels.label(key) else key
    val buttonHeight = BUTTON_HEIGHT_DP.dp

    if (barMode == ModifierBarMode.SelectionActions) {
        SelectionActionsBar(
            actions = SelectionActions(onCopy, onSelectAll, onPaste, onShare, onDismiss),
            textColor = textColor,
            backgroundColor = backgroundColor,
            buttonHeight = buttonHeight,
            modifier = modifier,
        )
        return
    }

    if (toolbarLayout != null) {
        ConfigurableModifierBar(
            toolbarLayout = toolbarLayout,
            onKeyClick = onKeyClick,
            onDrawerClick = onDrawerClick,
            onScrollClick = onScrollClick,
            ctrlState = ctrlState,
            altState = altState,
            onToggleCtrl = onToggleCtrl,
            onToggleAlt = onToggleAlt,
            textColor = textColor,
            backgroundColor = backgroundColor,
            modifier = modifier,
            label = ::label,
        )
        return
    }

    Column(
        modifier = modifier.fillMaxWidth().background(backgroundColor),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().height(buttonHeight),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ExtraKeyButton(text = label("ESC"), onClick = {
                onKeyClick("\u001b")
            }, textColor = textColor, testTag = "Key_ESC", contentDescription = "Escape")
            ExtraKeyButton(text = "\u2630", onClick = {
                onDrawerClick()
            }, textColor = textColor, testTag = "Key_DRAWER", contentDescription = "Open session drawer")
            ExtraKeyButton(text = label("SCROLL"), onClick = {
                onScrollClick()
            }, textColor = textColor, testTag = "Key_SCROLL", contentDescription = "Toggle scroll")
            ExtraKeyButton(text = label("HOME"), onClick = {
                onKeyClick("\u001b[H")
            }, textColor = textColor, testTag = "Key_HOME", contentDescription = "Home")
            ExtraKeyButton(
                text = "\u2191",
                onClick = {
                    onKeyClick("\u001b[A")
                },
                textColor = textColor,
                testTag = "Key_↑",
                contentDescription = "Arrow up",
                onRepeat = { onKeyClick("\u001b[A") },
            )
            ExtraKeyButton(text = label("END"), onClick = {
                onKeyClick("\u001b[F")
            }, textColor = textColor, testTag = "Key_END", contentDescription = "End")
            ExtraKeyButton(text = label("PGUP"), onClick = {
                onKeyClick("\u001b[5~")
            }, textColor = textColor, testTag = "Key_PGUP", contentDescription = "Page up")
        }

        Row(
            modifier = Modifier.fillMaxWidth().height(buttonHeight),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ExtraKeyButton(
                text = label("TAB"),
                onClick = { onKeyClick("\t") },
                textColor = textColor,
                testTag = "Key_TAB",
                contentDescription = "Tab",
            )
            ExtraKeyButton(
                text = label("CTRL"),
                onClick = { onToggleCtrl() },
                textColor = textColor,
                modifierState = ctrlState,
                testTag = "Key_CTRL",
                contentDescription = "Control toggle",
            )
            ExtraKeyButton(
                text = label("ALT"),
                onClick = { onToggleAlt() },
                textColor = textColor,
                modifierState = altState,
                testTag = "Key_ALT",
                contentDescription = "Alt toggle",
            )
            ExtraKeyButton(
                text = "\u2190",
                onClick = {
                    onKeyClick("\u001b[D")
                },
                textColor = textColor,
                testTag = "Key_←",
                contentDescription = "Arrow left",
                onRepeat = { onKeyClick("\u001b[D") },
            )
            ExtraKeyButton(
                text = "\u2193",
                onClick = {
                    onKeyClick("\u001b[B")
                },
                textColor = textColor,
                testTag = "Key_↓",
                contentDescription = "Arrow down",
                onRepeat = { onKeyClick("\u001b[B") },
            )
            ExtraKeyButton(
                text = "\u2192",
                onClick = {
                    onKeyClick("\u001b[C")
                },
                textColor = textColor,
                testTag = "Key_→",
                contentDescription = "Arrow right",
                onRepeat = { onKeyClick("\u001b[C") },
            )
            ExtraKeyButton(text = label("PGDN"), onClick = {
                onKeyClick("\u001b[6~")
            }, textColor = textColor, testTag = "Key_PGDN", contentDescription = "Page down")
        }
    }
}

private data class SelectionActions(
    val onCopy: (() -> Unit)?,
    val onSelectAll: (() -> Unit)?,
    val onPaste: (() -> Unit)?,
    val onShare: (() -> Unit)?,
    val onDismiss: (() -> Unit)?,
)

@Composable
private fun SelectionActionsBar(
    actions: SelectionActions,
    textColor: Color,
    backgroundColor: Color,
    buttonHeight: androidx.compose.ui.unit.Dp,
    modifier: Modifier,
) {
    val actionList = mutableListOf<Pair<String, () -> Unit>>()
    if (actions.onCopy != null) actionList.add("Copy" to actions.onCopy)
    if (actions.onSelectAll != null) actionList.add("Select All" to actions.onSelectAll)
    if (actions.onPaste != null) actionList.add("Paste" to actions.onPaste)
    if (actions.onShare != null) actionList.add("Share" to actions.onShare)

    Row(
        modifier =
        modifier
            .fillMaxWidth()
            .height(buttonHeight)
            .background(backgroundColor),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for ((label, action) in actionList) {
            ExtraKeyButton(
                text = label,
                onClick = action,
                textColor = textColor,
                testTag = "Action_${label.replace(" ", "")}",
                contentDescription = label,
            )
        }
        if (actions.onDismiss != null) {
            ExtraKeyButton(
                text = "\u00d7",
                onClick = actions.onDismiss,
                textColor = textColor,
                testTag = "Action_Dismiss",
                contentDescription = "Dismiss selection",
            )
        }
    }
}

@Suppress("LongParameterList", "CyclomaticComplexMethod")
@Composable
private fun ConfigurableModifierBar(
    toolbarLayout: List<ToolbarItem>,
    onKeyClick: (String) -> Unit,
    onDrawerClick: () -> Unit,
    onScrollClick: () -> Unit,
    ctrlState: ModifierState,
    altState: ModifierState,
    onToggleCtrl: () -> Unit,
    onToggleAlt: () -> Unit,
    textColor: Color,
    backgroundColor: Color,
    modifier: Modifier,
    label: (String) -> String,
) {
    val buttonHeight = BUTTON_HEIGHT_DP.dp
    val allKeys = toolbarLayout.toList()
    val midpoint = (allKeys.size + 1) / 2
    val row1 = allKeys.take(midpoint)
    val row2 = allKeys.drop(midpoint)

    fun getModifierState(item: ToolbarItem): ModifierState? = when (item) {
        is ToolbarItem.Default -> {
            when (item.key) {
                ToolbarKey.CTRL -> ctrlState
                ToolbarKey.ALT -> altState
                else -> null
            }
        }

        is ToolbarItem.Custom -> {
            null
        }
    }

    fun getOnRepeat(item: ToolbarItem): (() -> Unit)? = when (item) {
        is ToolbarItem.Default -> {
            when (item.key) {
                ToolbarKey.ARROW_UP,
                ToolbarKey.ARROW_DOWN,
                ToolbarKey.ARROW_LEFT,
                ToolbarKey.ARROW_RIGHT,
                -> {
                    { onKeyClick(item.key.sequence) }
                }

                else -> {
                    null
                }
            }
        }

        is ToolbarItem.Custom -> {
            null
        }
    }

    fun getItemLabel(item: ToolbarItem): String = when (item) {
        is ToolbarItem.Default -> {
            val display = label(item.key.defaultLabel)
            when (item.key) {
                ToolbarKey.ARROW_UP -> "\u2191"
                ToolbarKey.ARROW_DOWN -> "\u2193"
                ToolbarKey.ARROW_LEFT -> "\u2190"
                ToolbarKey.ARROW_RIGHT -> "\u2192"
                ToolbarKey.DRAWER -> "\u2630"
                else -> display
            }
        }

        is ToolbarItem.Custom -> {
            item.label
        }
    }

    fun getTestTag(item: ToolbarItem): String = when (item) {
        is ToolbarItem.Default -> {
            when (item.key) {
                ToolbarKey.ARROW_UP -> "Key_\u2191"
                ToolbarKey.ARROW_DOWN -> "Key_\u2193"
                ToolbarKey.ARROW_LEFT -> "Key_\u2190"
                ToolbarKey.ARROW_RIGHT -> "Key_\u2192"
                ToolbarKey.DRAWER -> "Key_DRAWER"
                else -> "Key_${item.key.defaultLabel}"
            }
        }

        is ToolbarItem.Custom -> {
            item.testTag
        }
    }

    fun getContentDescription(item: ToolbarItem): String = when (item) {
        is ToolbarItem.Default -> {
            when (item.key) {
                ToolbarKey.ESC -> "Escape"
                ToolbarKey.DRAWER -> "Open session drawer"
                ToolbarKey.SCROLL -> "Toggle scroll"
                ToolbarKey.HOME -> "Home"
                ToolbarKey.ARROW_UP -> "Arrow up"
                ToolbarKey.END -> "End"
                ToolbarKey.PGUP -> "Page up"
                ToolbarKey.TAB -> "Tab"
                ToolbarKey.CTRL -> "Control toggle"
                ToolbarKey.ALT -> "Alt toggle"
                ToolbarKey.ARROW_LEFT -> "Arrow left"
                ToolbarKey.ARROW_DOWN -> "Arrow down"
                ToolbarKey.ARROW_RIGHT -> "Arrow right"
                ToolbarKey.PGDN -> "Page down"
                else -> item.key.defaultLabel
            }
        }

        is ToolbarItem.Custom -> {
            item.label
        }
    }

    fun getKeyHandler(item: ToolbarItem): () -> Unit = when (item) {
        is ToolbarItem.Default -> {
            when (item.key) {
                ToolbarKey.CTRL -> {
                    onToggleCtrl
                }

                ToolbarKey.ALT -> {
                    onToggleAlt
                }

                ToolbarKey.DRAWER -> {
                    onDrawerClick
                }

                ToolbarKey.SCROLL -> {
                    onScrollClick
                }

                else -> {
                    val seq = item.key.sequence
                    if (seq.isNotEmpty()) {
                        { onKeyClick(seq) }
                    } else {
                        {}
                    }
                }
            }
        }

        is ToolbarItem.Custom -> {
            if (item.sequence.isNotEmpty()) {
                { onKeyClick(item.sequence) }
            } else {
                {}
            }
        }
    }

    Column(
        modifier = modifier.fillMaxWidth().background(backgroundColor),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().height(buttonHeight),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            for (item in row1) {
                ExtraKeyButton(
                    text = getItemLabel(item),
                    onClick = getKeyHandler(item),
                    textColor = textColor,
                    modifierState = getModifierState(item),
                    testTag = getTestTag(item),
                    contentDescription = getContentDescription(item),
                    onRepeat = getOnRepeat(item),
                )
            }
        }
        if (row2.isNotEmpty()) {
            Row(
                modifier = Modifier.fillMaxWidth().height(buttonHeight),
                horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                for (item in row2) {
                    ExtraKeyButton(
                        text = getItemLabel(item),
                        onClick = getKeyHandler(item),
                        textColor = textColor,
                        modifierState = getModifierState(item),
                        testTag = getTestTag(item),
                        contentDescription = getContentDescription(item),
                        onRepeat = getOnRepeat(item),
                    )
                }
            }
        }
    }
}

@Suppress("LongParameterList", "CyclomaticComplexMethod", "LongMethod")
@Composable
private fun RowScope.ExtraKeyButton(
    text: String,
    onClick: () -> Unit,
    textColor: androidx.compose.ui.graphics.Color,
    isActive: Boolean = false,
    modifierState: ModifierState? = null,
    testTag: String = "",
    contentDescription: String? = null,
    onRepeat: (() -> Unit)? = null,
) {
    val isLocked = modifierState == ModifierState.Locked
    val isOnce = modifierState == ModifierState.Once

    var isPressed by remember { mutableStateOf(false) }

    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.90f else 1f,
        animationSpec = spring(dampingRatio = 0.5f, stiffness = 800f),
        label = "btnScale",
    )

    val pressedColor = Color(0xFF7F7F7F)
    val targetBg =
        when {
            isLocked -> MaterialTheme.colorScheme.primary
            isOnce -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.5f)
            isActive -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.6f)
            isPressed -> pressedColor
            else -> Color.Transparent
        }
    val animatedBg by animateColorAsState(
        targetValue = targetBg,
        animationSpec = tween(durationMillis = 100),
        label = "btnBg",
    )
    val activeFg =
        when {
            isLocked -> MaterialTheme.colorScheme.onPrimary
            isOnce -> MaterialTheme.colorScheme.primary
            isActive -> MaterialTheme.colorScheme.primary
            else -> textColor
        }
    val fontWeight =
        when {
            isLocked -> FontWeight.Bold
            isOnce -> FontWeight.Bold
            isActive -> FontWeight.Bold
            else -> FontWeight.Normal
        }

    val view = LocalView.current
    val gestureModifier =
        if (onRepeat != null) {
            Modifier.pointerInput(Unit) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    isPressed = true
                    view.performHapticFeedback(android.view.HapticFeedbackConstants.KEYBOARD_TAP)
                    onClick()
                    var released = false
                    while (!released) {
                        val event = withTimeoutOrNull(REPEAT_TIMEOUT_MS) { waitForUpOrCancellation() }
                        if (event == null) {
                            onRepeat()
                        } else {
                            released = true
                        }
                    }
                    isPressed = false
                }
            }
        } else {
            Modifier.pointerInput(Unit) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    isPressed = true
                    view.performHapticFeedback(android.view.HapticFeedbackConstants.KEYBOARD_TAP)
                    onClick()
                    waitForUpOrCancellation()
                    isPressed = false
                }
            }
        }

    Box(
        modifier =
        Modifier
            .weight(1f)
            .height(BUTTON_HEIGHT_DP.dp)
            .then(if (testTag.isNotEmpty()) Modifier.testTag(testTag) else Modifier)
            .then(
                Modifier.background(
                    animatedBg,
                    RoundedCornerShape(4.dp),
                ),
            ).then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier)
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }.then(gestureModifier),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = text,
            color = activeFg,
            fontSize = BUTTON_FONT_SIZE_SP.sp,
            fontWeight = fontWeight,
            textAlign = TextAlign.Center,
            maxLines = 1,
        )
    }
}
