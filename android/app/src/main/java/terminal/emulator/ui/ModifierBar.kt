package terminal.emulator.ui

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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.R
import terminal.emulator.input.ModifierState

private const val BUTTON_HEIGHT_DP = 36
private const val BUTTON_FONT_SIZE_SP = 10

/** Key columns per horizontal page (default layout = 7 columns = 1 page). */
private const val MAX_COLUMNS_PER_PAGE = 7
private const val REPEAT_TIMEOUT_MS = 500L
private const val DWELL_GUARD_MS = 100L
private const val LONG_PRESS_MS = 500L
private const val SECONDARY_FONT_SIZE_SP = 8

/**
 * Compose key sequences: (first char, second char) → composed character.
 * Mirrors the classic X11 default compose table for the common Latin-1
 * accented characters plus a few symbols.
 */
private val COMPOSE_TABLE: Map<Pair<Char, Char>, Char> =
    mapOf(
        // Grave accents
        Pair('a', '`') to 'à',
        Pair('e', '`') to 'è',
        Pair('i', '`') to 'ì',
        Pair('o', '`') to 'ò',
        Pair('u', '`') to 'ù',
        // Acute accents
        Pair('a', '\'') to 'á',
        Pair('e', '\'') to 'é',
        Pair('i', '\'') to 'í',
        Pair('o', '\'') to 'ó',
        Pair('u', '\'') to 'ú',
        // Circumflex accents
        Pair('a', '^') to 'â',
        Pair('e', '^') to 'ê',
        Pair('i', '^') to 'î',
        Pair('o', '^') to 'ô',
        Pair('u', '^') to 'û',
        // Tilde
        Pair('a', '~') to 'ã',
        Pair('n', '~') to 'ñ',
        Pair('o', '~') to 'õ',
        // Diaeresis
        Pair('a', '"') to 'ä',
        Pair('e', '"') to 'ë',
        Pair('i', '"') to 'ï',
        Pair('o', '"') to 'ö',
        Pair('u', '"') to 'ü',
        Pair('y', '"') to 'ÿ',
        // Ring, ligatures, stroke
        Pair('a', 'o') to 'å',
        Pair('a', 'e') to 'æ',
        Pair('o', 'e') to 'œ',
        Pair('o', '/') to 'ø',
        Pair('s', 's') to 'ß',
        // Cedilla
        Pair('c', ',') to 'ç',
        // Symbols
        Pair('(', 'c') to '©',
        Pair('(', 'r') to '®',
        Pair('o', 'o') to '°',
        Pair('!', '!') to '¡',
        Pair('?', '?') to '¿',
        Pair('-', '-') to '\u2013',
    )

/** Look up a two-key compose sequence; null when no match exists. */
private fun composeLookup(
    first: Char,
    second: Char,
): Char? = COMPOSE_TABLE[first to second]

/** F1-F12 escape sequences (XTerm function-key codes). */
private val FN_KEY_SEQUENCES: List<Pair<String, String>> =
    listOf(
        "F1" to "\u001bOP",
        "F2" to "\u001bOQ",
        "F3" to "\u001bOR",
        "F4" to "\u001bOS",
        "F5" to "\u001b[15~",
        "F6" to "\u001b[17~",
        "F7" to "\u001b[18~",
        "F8" to "\u001b[19~",
        "F9" to "\u001b[20~",
        "F10" to "\u001b[21~",
        "F11" to "\u001b[23~",
        "F12" to "\u001b[24~",
    )

enum class ModifierBarMode { Normal, SelectionActions }

@Composable
fun rememberToolbarLayout(): List<ToolbarItem>? {
    val context = LocalContext.current
    val toolbarPreferences = remember { ToolbarPreferences(context) }
    var layout by remember { mutableStateOf(toolbarPreferences.getLayout()) }
    DisposableEffect(toolbarPreferences) {
        val listener = toolbarPreferences.registerLayoutListener { layout = it }
        onDispose { toolbarPreferences.unregisterLayoutListener(listener) }
    }
    return layout
}

/**
 * Termux v0.119.0-beta.3 extra_keys layout:
 * Row 1: ESC, DRAWER, SCROLL, HOME, ↑, END, PGUP
 * Row 2: TAB, CTRL, ALT, ←, ↓, →, PGDN
 *
 * Session button (DRAWER) is on the LEFT as the second button and has the
 * termux default `popup: 'PASTE'` (long-press pastes the clipboard).
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
    fnState: ModifierState = ModifierState.Off,
    onToggleCtrl: () -> Unit = {},
    onToggleAlt: () -> Unit = {},
    onToggleFn: () -> Unit = {},
    textColor: Color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.85f),
    backgroundColor: Color = MaterialTheme.colorScheme.surface,
    modifier: Modifier = Modifier,
    useNerdFontGlyphs: Boolean = false,
    toolbarLayout: List<ToolbarItem>? = null,
    barMode: ModifierBarMode = ModifierBarMode.Normal,
    onCopy: (() -> Unit)? = null,
    copyEnabled: Boolean = onCopy != null,
    onSelectAll: (() -> Unit)? = null,
    onPaste: (() -> Unit)? = null,
    onShare: (() -> Unit)? = null,
    onAnchorLeft: (() -> Unit)? = null,
    onAnchorRight: (() -> Unit)? = null,
    onDismiss: (() -> Unit)? = null,
) {
    fun label(key: String): String = if (useNerdFontGlyphs) NerdKeyLabels.label(key) else key
    val buttonHeight = BUTTON_HEIGHT_DP.dp

    // ── Compose key mode ──────────────────────────────────────────────
    var composeActive by remember { mutableStateOf(false) }
    var composeBuffer by remember { mutableStateOf<Char?>(null) }

    fun flushCompose() {
        composeBuffer?.let { onKeyClick(it.toString()) }
        composeBuffer = null
        composeActive = false
    }

    fun toggleCompose() {
        if (composeActive) flushCompose() else composeActive = true
    }

    /** Route every key sequence through the compose state machine. */
    fun dispatchKey(seq: String) {
        if (!composeActive) {
            onKeyClick(seq)
            return
        }
        val ch = if (seq.length == 1) seq[0] else null
        if (ch == null || ch.isISOControl()) {
            // Non-printable key: flush the buffered char and exit compose mode.
            flushCompose()
            onKeyClick(seq)
            return
        }
        val buffered = composeBuffer
        if (buffered == null) {
            composeBuffer = ch
        } else {
            composeBuffer = null
            composeActive = false
            val composed = composeLookup(buffered, ch)
            if (composed != null) {
                onKeyClick(composed.toString())
            } else {
                // No match: deliver both keys verbatim.
                onKeyClick(buffered.toString())
                onKeyClick(ch.toString())
            }
        }
    }

    // ── FN second layer (F1-F12) ──────────────────────────────────────
    if (fnState == ModifierState.Locked) {
        FnKeyRows(
            onKeyClick = ::dispatchKey,
            onToggleFn = onToggleFn,
            textColor = textColor,
            backgroundColor = backgroundColor,
            modifier = modifier,
            label = ::label,
        )
        return
    }

    if (barMode == ModifierBarMode.SelectionActions) {
        SelectionActionsBar(
            actions = SelectionActions(onCopy, copyEnabled, onSelectAll, onPaste, onShare, onAnchorLeft, onAnchorRight, onDismiss),
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
            onKeyClick = ::dispatchKey,
            onDrawerClick = onDrawerClick,
            onScrollClick = onScrollClick,
            ctrlState = ctrlState,
            altState = altState,
            fnState = fnState,
            onToggleCtrl = onToggleCtrl,
            onToggleAlt = onToggleAlt,
            onToggleFn = onToggleFn,
            composeActive = composeActive,
            onToggleCompose = ::toggleCompose,
            onPaste = onPaste,
            textColor = textColor,
            backgroundColor = backgroundColor,
            modifier = modifier,
            label = ::label,
        )
        return
    }

    Column(
        modifier = modifier.fillMaxWidth().background(backgroundColor).testTag("ModifierBar"),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().height(buttonHeight),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ExtraKeyButton(text = label("ESC"), onClick = {
                dispatchKey("\u001b")
            }, textColor = textColor, testTag = "Key_ESC", contentDescription = stringResource(R.string.escape))
            ExtraKeyButton(text = "\u2630", onClick = {
                onDrawerClick()
            }, textColor = textColor, testTag = "Key_DRAWER", contentDescription = stringResource(R.string.open_session_drawer))
            ExtraKeyButton(text = label("SCROLL"), onClick = {
                onScrollClick()
            }, textColor = textColor, testTag = "Key_SCROLL", contentDescription = stringResource(R.string.toggle_scroll))
            ExtraKeyButton(text = label("HOME"), onClick = {
                dispatchKey("\u001b[H")
            }, textColor = textColor, testTag = "Key_HOME", contentDescription = stringResource(R.string.home_key))
            ExtraKeyButton(
                text = "\u2191",
                onClick = {
                    dispatchKey("\u001b[A")
                },
                textColor = textColor,
                testTag = "Key_↑",
                contentDescription = stringResource(R.string.arrow_up),
                onRepeat = { dispatchKey("\u001b[A") },
            )
            ExtraKeyButton(text = label("END"), onClick = {
                dispatchKey("\u001b[F")
            }, textColor = textColor, testTag = "Key_END", contentDescription = stringResource(R.string.end_key))
            ExtraKeyButton(text = label("PGUP"), onClick = {
                dispatchKey("\u001b[5~")
            }, textColor = textColor, testTag = "Key_PGUP", contentDescription = stringResource(R.string.page_up))
        }

        Row(
            modifier = Modifier.fillMaxWidth().height(buttonHeight),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ExtraKeyButton(
                text = label("FN"),
                onClick = { onToggleFn() },
                textColor = textColor,
                modifierState = fnState,
                testTag = "Key_FN",
                contentDescription = stringResource(R.string.function_key_layer),
            )
            ExtraKeyButton(
                text = label("COMPOSE"),
                onClick = { toggleCompose() },
                textColor = textColor,
                modifierState = if (composeActive) ModifierState.Locked else null,
                testTag = "Key_COMPOSE",
                contentDescription = stringResource(R.string.compose_key),
            )
            ExtraKeyButton(
                text = label("TAB"),
                onClick = { dispatchKey("\t") },
                textColor = textColor,
                testTag = "Key_TAB",
                contentDescription = stringResource(R.string.tab_key),
            )
            ExtraKeyButton(
                text = label("CTRL"),
                onClick = { onToggleCtrl() },
                textColor = textColor,
                modifierState = ctrlState,
                testTag = "Key_CTRL",
                contentDescription = stringResource(R.string.control_toggle),
            )
            ExtraKeyButton(
                text = label("ALT"),
                onClick = { onToggleAlt() },
                textColor = textColor,
                modifierState = altState,
                testTag = "Key_ALT",
                contentDescription = stringResource(R.string.alt_toggle),
            )
            ExtraKeyButton(
                text = "\u2190",
                onClick = {
                    dispatchKey("\u001b[D")
                },
                textColor = textColor,
                testTag = "Key_←",
                contentDescription = stringResource(R.string.arrow_left),
                onRepeat = { dispatchKey("\u001b[D") },
            )
            ExtraKeyButton(
                text = "\u2193",
                onClick = {
                    dispatchKey("\u001b[B")
                },
                textColor = textColor,
                testTag = "Key_↓",
                contentDescription = stringResource(R.string.arrow_down),
                onRepeat = { dispatchKey("\u001b[B") },
            )
            ExtraKeyButton(
                text = "\u2192",
                onClick = {
                    dispatchKey("\u001b[C")
                },
                textColor = textColor,
                testTag = "Key_→",
                contentDescription = stringResource(R.string.arrow_right),
                onRepeat = { dispatchKey("\u001b[C") },
            )
            ExtraKeyButton(text = label("PGDN"), onClick = {
                dispatchKey("\u001b[6~")
            }, textColor = textColor, testTag = "Key_PGDN", contentDescription = stringResource(R.string.page_down))
        }
    }
}

/**
 * Second-layer rows shown when the FN modifier is Locked: F1-F12.
 * Tapping an F-key sends its escape sequence and returns to the normal
 * layer (single-shot); tapping FN again returns without sending anything.
 */
@Composable
private fun FnKeyRows(
    onKeyClick: (String) -> Unit,
    onToggleFn: () -> Unit,
    textColor: Color,
    backgroundColor: Color,
    modifier: Modifier,
    label: (String) -> String,
) {
    val buttonHeight = BUTTON_HEIGHT_DP.dp
    Column(
        modifier = modifier.fillMaxWidth().background(backgroundColor).testTag("ModifierBar"),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().height(buttonHeight),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FN_KEY_SEQUENCES.take(6).forEach { (name, seq) ->
                ExtraKeyButton(
                    text = label(name),
                    onClick = {
                        onKeyClick(seq)
                        onToggleFn()
                    },
                    textColor = textColor,
                    testTag = "Key_$name",
                    contentDescription = name,
                )
            }
        }
        Row(
            modifier = Modifier.fillMaxWidth().height(buttonHeight),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FN_KEY_SEQUENCES.drop(6).forEach { (name, seq) ->
                ExtraKeyButton(
                    text = label(name),
                    onClick = {
                        onKeyClick(seq)
                        onToggleFn()
                    },
                    textColor = textColor,
                    testTag = "Key_$name",
                    contentDescription = name,
                )
            }
            ExtraKeyButton(
                text = label("FN"),
                onClick = { onToggleFn() },
                textColor = textColor,
                modifierState = ModifierState.Locked,
                testTag = "Key_FN",
                contentDescription = stringResource(R.string.function_key_layer),
            )
        }
    }
}

private data class SelectionActions(
    val onCopy: (() -> Unit)?,
    val copyEnabled: Boolean = onCopy != null,
    val onSelectAll: (() -> Unit)?,
    val onPaste: (() -> Unit)?,
    val onShare: (() -> Unit)?,
    val onAnchorLeft: (() -> Unit)?,
    val onAnchorRight: (() -> Unit)?,
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
    val actionList = mutableListOf<Triple<String, () -> Unit, Boolean>>()
    if (actions.onAnchorLeft != null) actionList.add(Triple("\u25c0", actions.onAnchorLeft, true))
    if (actions.onAnchorRight != null) actionList.add(Triple("\u25b6", actions.onAnchorRight, true))
    if (actions.onCopy != null) actionList.add(Triple(stringResource(R.string.copy), actions.onCopy, actions.copyEnabled))
    if (actions.onSelectAll != null) actionList.add(Triple(stringResource(R.string.select_all), actions.onSelectAll, true))
    if (actions.onPaste != null) actionList.add(Triple(stringResource(R.string.paste), actions.onPaste, true))
    if (actions.onShare != null) actionList.add(Triple(stringResource(R.string.share), actions.onShare, true))

    Row(
        modifier =
        modifier
            .fillMaxWidth()
            .height(buttonHeight)
            .background(backgroundColor),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for ((label, action, enabled) in actionList) {
            ExtraKeyButton(
                text = label,
                onClick = action,
                textColor = textColor,
                enabled = enabled,
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
                contentDescription = stringResource(R.string.dismiss_selection),
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
    fnState: ModifierState,
    onToggleCtrl: () -> Unit,
    onToggleAlt: () -> Unit,
    onToggleFn: () -> Unit,
    composeActive: Boolean,
    onToggleCompose: () -> Unit,
    onPaste: (() -> Unit)?,
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
    // Page the layout horizontally so more keys can be added than fit one
    // screen width (termux ViewPager behaviour): a page holds up to
    // MAX_COLUMNS_PER_PAGE key columns (a column = one top + one bottom key);
    // swipe left/right to reach the rest. The default 14-key layout is a
    // single 7-column page.
    val columns: List<Pair<ToolbarItem?, ToolbarItem?>> =
        (0 until maxOf(row1.size, row2.size)).map { index ->
            row1.getOrNull(index) to row2.getOrNull(index)
        }
    val pages = columns.chunked(MAX_COLUMNS_PER_PAGE)
    val pagerState = rememberPagerState(pageCount = { pages.size })
    val actions =
        ModifierBarActions(
            onKeyClick = onKeyClick,
            onDrawerClick = onDrawerClick,
            onScrollClick = onScrollClick,
            onToggleCtrl = onToggleCtrl,
            onToggleAlt = onToggleAlt,
            onToggleFn = onToggleFn,
            onToggleCompose = onToggleCompose,
            onPaste = onPaste,
        )
    val modifierStates =
        ModifierBarStates(
            ctrlState = ctrlState,
            altState = altState,
            fnState = fnState,
            composeActive = composeActive,
        )
    val defaultContentDescriptions: Map<ToolbarKey, String> =
        ToolbarKey.entries.associateWith { key ->
            key.contentDescriptionRes?.let { stringResource(it) } ?: key.defaultLabel
        }
    val presentation = { item: ToolbarItem ->
        toolbarItemPresentation(
            item = item,
            actions = actions,
            modifierStates = modifierStates,
            label = label,
            contentDescriptionResolver = { key ->
                defaultContentDescriptions[key] ?: key.defaultLabel
            },
        )
    }

    Column(
        modifier = modifier.fillMaxWidth().background(backgroundColor),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        HorizontalPager(state = pagerState, modifier = Modifier.fillMaxWidth()) { page ->
            val pageColumns = pages[page]
            val pageRow1 = pageColumns.mapNotNull { it.first }
            val pageRow2 = pageColumns.mapNotNull { it.second }
            Column {
                ModifierBarButtonRow(
                    items = pageRow1.map(presentation),
                    buttonHeight = buttonHeight,
                    textColor = textColor,
                )
                if (pageRow2.isNotEmpty()) {
                    ModifierBarButtonRow(
                        items = pageRow2.map(presentation),
                        buttonHeight = buttonHeight,
                        textColor = textColor,
                    )
                }
            }
        }
    }
}

/** One rendered button's full presentation, derived from a [ToolbarItem]. */
private data class ToolbarItemPresentation(
    val label: String,
    val onClick: () -> Unit,
    val modifierState: ModifierState?,
    val testTag: String,
    val contentDescription: String?,
    val onRepeat: (() -> Unit)?,
    val widthWeight: Int,
    val secondaryLabel: String?,
    val secondaryAction: (() -> Unit)?,
)

/** The callbacks a configurable modifier bar can trigger. */
private data class ModifierBarActions(
    val onKeyClick: (String) -> Unit,
    val onDrawerClick: () -> Unit,
    val onScrollClick: () -> Unit,
    val onToggleCtrl: () -> Unit,
    val onToggleAlt: () -> Unit,
    val onToggleFn: () -> Unit,
    val onToggleCompose: () -> Unit,
    /** Long-press paste on DRAWER (termux default `popup: 'PASTE'`). */
    val onPaste: (() -> Unit)?,
)

/** The live toggle states of the modifier keys. */
private data class ModifierBarStates(
    val ctrlState: ModifierState,
    val altState: ModifierState,
    val fnState: ModifierState,
    val composeActive: Boolean,
)

/** Long-press action for a key: an explicit secondary sequence, or the
 *  DRAWER paste popup (termux default) as fallback. */
private fun secondaryLongPressAction(
    item: ToolbarItem,
    actions: ModifierBarActions,
    isDrawer: Boolean,
): (() -> Unit)? = item.secondarySequence
    ?.takeIf { it.isNotEmpty() }
    ?.let { { actions.onKeyClick(it) } }
    ?: if (isDrawer) actions.onPaste else null

private fun toolbarItemPresentation(
    item: ToolbarItem,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
    label: (String) -> String,
    contentDescriptionResolver: (ToolbarKey) -> String,
): ToolbarItemPresentation {
    val modifierState =
        when ((item as? ToolbarItem.Default)?.key) {
            ToolbarKey.CTRL -> modifierStates.ctrlState
            ToolbarKey.ALT -> modifierStates.altState
            ToolbarKey.FN -> modifierStates.fnState
            ToolbarKey.COMPOSE -> if (modifierStates.composeActive) ModifierState.Locked else null
            else -> null
        }
    val onRepeat =
        (item as? ToolbarItem.Default)
            ?.takeIf { it.key.repeatable }
            ?.let { { actions.onKeyClick(it.key.sequence) } }
    val itemLabel =
        when (item) {
            is ToolbarItem.Default -> item.key.symbol ?: label(item.key.defaultLabel)
            is ToolbarItem.Custom -> item.label
        }
    val testTag =
        when (item) {
            is ToolbarItem.Default -> item.key.testTag ?: "Key_${item.key.defaultLabel}"
            is ToolbarItem.Custom -> item.testTag
        }
    val contentDescription =
        when (item) {
            is ToolbarItem.Default -> contentDescriptionResolver(item.key)
            is ToolbarItem.Custom -> item.label
        }
    val isDrawer = (item as? ToolbarItem.Default)?.key == ToolbarKey.DRAWER
    // DRAWER long-press = paste (termux default `popup: 'PASTE'`); an
    // explicit per-item secondary sequence wins over the default popup.
    val secondaryLabel =
        item.secondaryLabel ?: if (isDrawer && actions.onPaste != null) "PASTE" else null
    val secondaryAction =
        secondaryLongPressAction(item, actions, isDrawer)
    return ToolbarItemPresentation(
        label = itemLabel,
        onClick = toolbarItemKeyHandler(item, actions),
        modifierState = modifierState,
        testTag = testTag,
        contentDescription = contentDescription,
        onRepeat = onRepeat,
        widthWeight = item.width,
        secondaryLabel = secondaryLabel,
        secondaryAction = secondaryAction,
    )
}

private fun toolbarItemKeyHandler(
    item: ToolbarItem,
    actions: ModifierBarActions,
): () -> Unit = when (item) {
    is ToolbarItem.Default ->
        when (item.key) {
            ToolbarKey.CTRL -> actions.onToggleCtrl

            ToolbarKey.ALT -> actions.onToggleAlt

            ToolbarKey.FN -> actions.onToggleFn

            ToolbarKey.COMPOSE -> actions.onToggleCompose

            ToolbarKey.DRAWER -> actions.onDrawerClick

            ToolbarKey.SCROLL -> actions.onScrollClick

            else -> {
                val seq = item.key.sequence
                if (seq.isNotEmpty()) {
                    { actions.onKeyClick(seq) }
                } else {
                    {}
                }
            }
        }

    is ToolbarItem.Custom -> {
        val macro = item.macro
        if (macro != null && ToolbarMacroExpander.isMacro(macro)) {
            val keys = ToolbarMacroExpander.expand(macro)
            if (keys.isNotEmpty()) {
                { keys.forEach(actions.onKeyClick) }
            } else {
                {}
            }
        } else if (item.sequence.isNotEmpty()) {
            { actions.onKeyClick(item.sequence) }
        } else {
            {}
        }
    }
}

/** One full-width row of extra-key buttons from pre-computed presentations. */
@Composable
private fun ModifierBarButtonRow(
    items: List<ToolbarItemPresentation>,
    buttonHeight: Dp,
    textColor: Color,
) {
    Row(
        modifier = Modifier.fillMaxWidth().height(buttonHeight),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for (item in items) {
            ExtraKeyButton(
                text = item.label,
                onClick = item.onClick,
                textColor = textColor,
                modifierState = item.modifierState,
                testTag = item.testTag,
                contentDescription = item.contentDescription,
                onRepeat = item.onRepeat,
                widthWeight = item.widthWeight,
                secondaryLabel = item.secondaryLabel,
                secondaryAction = item.secondaryAction,
            )
        }
    }
}

@Suppress("LongParameterList", "CyclomaticComplexMethod", "LongMethod", "CognitiveComplexMethod")
@Composable
private fun RowScope.ExtraKeyButton(
    text: String,
    onClick: () -> Unit,
    textColor: androidx.compose.ui.graphics.Color,
    isActive: Boolean = false,
    enabled: Boolean = true,
    modifierState: ModifierState? = null,
    testTag: String = "",
    contentDescription: String? = null,
    onRepeat: (() -> Unit)? = null,
    widthWeight: Int = 1,
    secondaryLabel: String? = null,
    secondaryAction: (() -> Unit)? = null,
) {
    val isLocked = modifierState == ModifierState.Locked
    val isOnce = modifierState == ModifierState.Once

    var isPressed by remember { mutableStateOf(false) }

    val scale by animateFloatAsState(
        targetValue = if (isPressed && enabled) 0.90f else 1f,
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
            !enabled -> textColor.copy(alpha = 0.38f)
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
        Modifier.pointerInput(onRepeat, secondaryAction) {
            awaitEachGesture {
                awaitFirstDown()
                val downPos = currentEvent.changes.first().position
                val slop = viewConfiguration.touchSlop
                isPressed = true
                try {
                    var gestureValid = true
                    if (secondaryAction != null) {
                        // Keys with a secondary action (DRAWER → paste):
                        // a quick tap fires onClick IMMEDIATELY (no
                        // long-press confirmation window), while a
                        // sustained press past LONG_PRESS_MS triggers
                        // secondaryAction once. Mirrors Android's
                        // onClick/onLongClick split.
                        var longPressTriggered = false
                        val downTime = System.currentTimeMillis()
                        while (true) {
                            val ev = awaitPointerEvent()
                            val ch = ev.changes.first()
                            if (!ch.pressed) break
                            if ((ch.position - downPos).getDistance() > slop) {
                                gestureValid = false
                                break
                            }
                            if (!longPressTriggered &&
                                System.currentTimeMillis() - downTime >= LONG_PRESS_MS
                            ) {
                                longPressTriggered = true
                                view.performHapticFeedback(
                                    android.view.HapticFeedbackConstants.LONG_PRESS,
                                )
                                secondaryAction()
                            }
                        }
                        if (!longPressTriggered && gestureValid && enabled) {
                            view.performHapticFeedback(
                                android.view.HapticFeedbackConstants.KEYBOARD_TAP,
                            )
                            onClick()
                        }
                    } else {
                        // No secondary action: keep the DWELL_GUARD_MS
                        // window so auto-repeat latency is unchanged.
                        var upConsumed = false
                        withTimeoutOrNull(DWELL_GUARD_MS) {
                            while (true) {
                                val ev = awaitPointerEvent()
                                val ch = ev.changes.first()
                                if (!ch.pressed) {
                                    upConsumed = true
                                    break
                                }
                                if ((ch.position - downPos).getDistance() > slop) {
                                    gestureValid = false
                                    break
                                }
                            }
                            false
                        }
                        if (gestureValid && enabled) {
                            view.performHapticFeedback(
                                android.view.HapticFeedbackConstants.KEYBOARD_TAP,
                            )
                            onClick()
                            if (onRepeat == null) {
                                if (!upConsumed) {
                                    waitForUpOrCancellation()
                                }
                            } else {
                                while (true) {
                                    try {
                                        withTimeout(REPEAT_TIMEOUT_MS) {
                                            waitForUpOrCancellation()
                                        }
                                        break
                                    } catch (_: TimeoutCancellationException) {
                                        onRepeat()
                                    }
                                }
                            }
                        }
                    }
                } finally {
                    isPressed = false
                }
            }
        }

    Box(
        modifier =
        Modifier
            .weight(weight = widthWeight.coerceAtLeast(1).toFloat())
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
        if (secondaryLabel != null) {
            Text(
                text = secondaryLabel,
                color = activeFg.copy(alpha = 0.55f),
                fontSize = SECONDARY_FONT_SIZE_SP.sp,
                fontWeight = FontWeight.Normal,
                textAlign = TextAlign.Center,
                maxLines = 1,
                modifier = Modifier.align(Alignment.TopCenter).padding(top = 2.dp),
            )
        }
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
