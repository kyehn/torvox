package terminal.emulator.ui

import android.view.KeyEvent
import android.view.MotionEvent
import androidx.annotation.StringRes
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
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
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEvent
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.collections.immutable.ImmutableList
import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toImmutableList
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.R
import terminal.emulator.input.ModifierState

private const val BUTTON_HEIGHT_DP = 36
private const val BUTTON_FONT_SIZE_SP = 10

/**
 * 修饰键栏按键种类（固定）：布局不可配置（PROHIBITED 禁止布局编辑器），
 * 故只有枚举本身，没有「自定义按键」与「按键宽度」概念。
 */
enum class ToolbarKey(
    val defaultLabel: String,
    val sequence: String,
    /** 显示符号覆盖（抽屉图标与标签不同）。 */
    val symbol: String? = null,
    /** 测试标签覆盖，缺省为 "Key_<defaultLabel>"。 */
    val testTag: String? = null,
    /** 无障碍描述资源，缺省取 defaultLabel。 */
    @StringRes val contentDescriptionRes: Int? = null,
    /** 按住时重复发送按键序列（方向键）。 */
    val repeatable: Boolean = false,
    /** 由 [ModifierState] 驱动的切换型修饰键。 */
    val modifier: Boolean = false,
) {
    ESC("ESC", "\u001b", contentDescriptionRes = R.string.escape),
    DRAWER(
        "\u2261",
        "",
        symbol = "\u2630",
        testTag = "Key_DRAWER",
        contentDescriptionRes = R.string.open_session_drawer,
    ),
    SCROLL("SCROLL", "", contentDescriptionRes = R.string.toggle_scroll),
    HOME("HOME", "\u001b[H", contentDescriptionRes = R.string.home_key),
    ARROW_UP("\u2191", "\u001b[A", contentDescriptionRes = R.string.arrow_up, repeatable = true),
    END("END", "\u001b[F", contentDescriptionRes = R.string.end_key),
    PGUP("PGUP", "\u001b[5~", contentDescriptionRes = R.string.page_up),
    TAB("TAB", "\t", contentDescriptionRes = R.string.tab_key),
    CTRL("CTRL", "", contentDescriptionRes = R.string.control_toggle, modifier = true),
    ALT("ALT", "", contentDescriptionRes = R.string.alt_toggle, modifier = true),
    ARROW_LEFT("\u2190", "\u001b[D", contentDescriptionRes = R.string.arrow_left, repeatable = true),
    ARROW_DOWN("\u2193", "\u001b[B", contentDescriptionRes = R.string.arrow_down, repeatable = true),
    ARROW_RIGHT("\u2192", "\u001b[C", contentDescriptionRes = R.string.arrow_right, repeatable = true),
    PGDN("PGDN", "\u001b[6~", contentDescriptionRes = R.string.page_down),
    PIPE("|", "|"),
    SLASH("/", "/"),
    DASH("-", "-"),
    UNDERSCORE("_", "_"),
    DOT(".", "."),
    EQUALS("=", "="),
    HASH("#", "#"),
    AT("@", "@"),
    AMPERSAND("&", "&"),
    TILDE("~", "~"),
    BACKTICK("`", "`"),
    BANG("!", "!"),
    QUESTION("?", "?"),
}

/**
 * 固定布局，对应 termux-app v0.119.0-beta.3 的 extra_keys：
 * 第一行 ESC DRAWER SCROLL HOME ↑ END PGUP，第二行 TAB CTRL ALT ← ↓ → PGDN。
 * 渲染时从中点切分为两行。
 *
 * DRAWER 位于左侧第二个键，长按粘贴剪贴板（termux 默认 `popup: 'PASTE'`）。
 */
internal val TERMUX_EXTRA_KEYS: ImmutableList<ToolbarKey> = persistentListOf(
    ToolbarKey.ESC,
    ToolbarKey.DRAWER,
    ToolbarKey.SCROLL,
    ToolbarKey.HOME,
    ToolbarKey.ARROW_UP,
    ToolbarKey.END,
    ToolbarKey.PGUP,
    ToolbarKey.TAB,
    ToolbarKey.CTRL,
    ToolbarKey.ALT,
    ToolbarKey.ARROW_LEFT,
    ToolbarKey.ARROW_DOWN,
    ToolbarKey.ARROW_RIGHT,
    ToolbarKey.PGDN,
)

// Termux ExtraKeysView parity: long-press threshold 400ms
// (FALLBACK_LONG_PRESS_DURATION), repeat starts after the same delay
// and repeats at 80ms cadence (DEFAULT_LONG_PRESS_REPEAT_DELAY).
private const val LONG_PRESS_MS = 400L

// (spec modifier-bar-interaction "press-down fires immediately"):
// termux ExtraKeysView semantics — the key fires on ACTION_DOWN, auto-repeat
// starts after an initial delay and repeats at a fixed cadence until UP.
private const val AUTO_REPEAT_INITIAL_DELAY_MS = 400L
private const val AUTO_REPEAT_INTERVAL_MS = 80L

// spec modifier-bar-interaction press-feedback thresholds.
private const val PRESS_BG_TWEEN_MS = 30
private const val PRESS_SCALE_SPRING_DAMPING = 0.55f
private const val PRESS_SCALE_SPRING_STIFFNESS = 5000f
private const val SECONDARY_FONT_SIZE_SP = 8

/** 横向分页：第 0 页按键，第 1 页文本输入。 */
private const val TEXT_INPUT_PAGE_INDEX = 1
private const val KEY_PAGE_COUNT = TEXT_INPUT_PAGE_INDEX + 1

/**
 * Termux v0.119.0-beta.3 extra_keys layout: Row 1: ESC, DRAWER, SCROLL, HOME, ↑, END, PGUP Row 2:
 * TAB, CTRL, ALT, ←, ↓, →, PGDN
 *
 * Session button (DRAWER) is on the LEFT as the second button and has the termux default `popup:
 * 'PASTE'` (long-press pastes the clipboard). All buttons are borderless with transparent
 * background. Each button has equal weight for uniform sizing.
 */
@Composable
fun ModifierBar(
    onKeyClick: (String) -> Unit,
    modifier: Modifier = Modifier,
    onDrawerClick: () -> Unit = {},
    onScrollClick: () -> Unit = {},
    /** SCROLL-button lock state — drives the button's selected/highlight. */
    scrollActive: Boolean = false,
    ctrlState: ModifierState = ModifierState.Off,
    altState: ModifierState = ModifierState.Off,
    onToggleCtrl: () -> Unit = {},
    onToggleAlt: () -> Unit = {},
    /** Termux 同款长按锁定 CTRL/ALT（轻点只切换一次性态）。 */
    onLockCtrl: () -> Unit = {},
    onLockAlt: () -> Unit = {},
    textColor: Color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.85f),
    backgroundColor: Color = MaterialTheme.colorScheme.surface,
    /** DECCKM application-cursor state — queried on each arrow tap so vim/less arrows work. */
    isAppCursorMode: () -> Boolean = { false },
    /** Raw-byte channel for modifier-combined keys (avoids String charset round-trip). */
    onKeyBytesClick: ((ByteArray) -> Unit)? = null,
    /** Consumes Once sticky modifiers after a modified key is sent. */
    onConsumeModifiers: () -> Unit = {},
) {
    ConfigurableModifierBar(
        onKeyClick = onKeyClick,
        onDrawerClick = onDrawerClick,
        onScrollClick = onScrollClick,
        scrollActive = scrollActive,
        ctrlState = ctrlState,
        altState = altState,
        onToggleCtrl = onToggleCtrl,
        onToggleAlt = onToggleAlt,
        onLockCtrl = onLockCtrl,
        onLockAlt = onLockAlt,
        isAppCursorMode = isAppCursorMode,
        onKeyBytesClick = onKeyBytesClick,
        onConsumeModifiers = onConsumeModifiers,
        textColor = textColor,
        backgroundColor = backgroundColor,
        modifier = modifier,
    )
}

@Composable
private fun ModifierBarTextInputPage(
    buttonHeight: Dp,
    textColor: Color,
    backgroundColor: Color,
    onSubmit: (String) -> Unit,
) {
    var textInput by remember { mutableStateOf("") }
    TextField(
        value = textInput,
        onValueChange = { textInput = it },
        modifier =
        Modifier.fillMaxWidth()
            .height(buttonHeight * 2)
            .testTag("TextInputPage"),
        label = { Text(stringResource(R.string.text_input_box)) },
        singleLine = true,
        textStyle =
        androidx.compose.ui.text.TextStyle(
            color = textColor,
            fontSize = BUTTON_FONT_SIZE_SP.sp,
        ),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions =
        KeyboardActions(
            onDone = {
                onSubmit(if (textInput.isEmpty()) "\r" else textInput)
                textInput = ""
            },
        ),
        colors =
        TextFieldDefaults.colors(
            focusedTextColor = textColor,
            unfocusedTextColor = textColor,
            cursorColor = textColor,
            focusedContainerColor = backgroundColor,
            unfocusedContainerColor = backgroundColor,
            focusedIndicatorColor = Color.Transparent,
            unfocusedIndicatorColor = Color.Transparent,
        ),
    )
}

@Composable
private fun ConfigurableModifierBar(
    onKeyClick: (String) -> Unit,
    onDrawerClick: () -> Unit,
    onScrollClick: () -> Unit,
    scrollActive: Boolean,
    ctrlState: ModifierState,
    altState: ModifierState,
    onToggleCtrl: () -> Unit,
    onToggleAlt: () -> Unit,
    textColor: Color,
    backgroundColor: Color,
    modifier: Modifier = Modifier,
    onLockCtrl: () -> Unit = {},
    onLockAlt: () -> Unit = {},
    isAppCursorMode: () -> Boolean = { false },
    onKeyBytesClick: ((ByteArray) -> Unit)? = null,
    onConsumeModifiers: () -> Unit = {},
) {
    val buttonHeight = BUTTON_HEIGHT_DP.dp
    // 固定 2 行 7 列：从中点切分（termux extra_keys 顺序）。
    val row1 = TERMUX_EXTRA_KEYS.take(TERMUX_EXTRA_KEYS.size / 2)
    val row2 = TERMUX_EXTRA_KEYS.drop(TERMUX_EXTRA_KEYS.size / 2)
    // 第 0 页是按键页，第 1 页是文本输入页（DESIGN：左滑进入文本输入框）。
    val pagerState = rememberPagerState(pageCount = { KEY_PAGE_COUNT })
    val actions =
        ModifierBarActions(
            onKeyClick = onKeyClick,
            onDrawerClick = onDrawerClick,
            onScrollClick = onScrollClick,
            onToggleCtrl = onToggleCtrl,
            onToggleAlt = onToggleAlt,
            onLockCtrl = onLockCtrl,
            onLockAlt = onLockAlt,
            isAppCursorMode = isAppCursorMode,
            onKeyBytesClick = onKeyBytesClick,
            onConsumeModifiers = onConsumeModifiers,
        )
    val modifierStates =
        ModifierBarStates(
            ctrlState = ctrlState,
            altState = altState,
            scrollActive = scrollActive,
        )
    val defaultContentDescriptions: Map<ToolbarKey, String> =
        ToolbarKey.entries.associateWith { key ->
            key.contentDescriptionRes?.let { stringResource(it) } ?: key.defaultLabel
        }
    val presentation = { key: ToolbarKey ->
        toolbarKeyPresentation(
            key = key,
            actions = actions,
            modifierStates = modifierStates,
            contentDescriptionResolver = { key ->
                defaultContentDescriptions[key] ?: key.defaultLabel
            },
            isAppCursorMode = isAppCursorMode,
        )
    }

    Column(
        modifier = modifier.fillMaxWidth().background(backgroundColor).testTag("ModifierBar"),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        // 末页为文本输入页（termux TerminalToolbarViewPager 行为）：滑过按键页
        // 输入文本，Done 原样发送（空则发送回车）并清空。
        HorizontalPager(
            state = pagerState,
            modifier = Modifier.fillMaxWidth().testTag("ModifierBarPager"),
        ) { page ->
            if (page == TEXT_INPUT_PAGE_INDEX) {
                ModifierBarTextInputPage(
                    buttonHeight = buttonHeight,
                    textColor = textColor,
                    backgroundColor = backgroundColor,
                    onSubmit = onKeyClick,
                )
                return@HorizontalPager
            }
            Column {
                ModifierBarButtonRow(row1.map(presentation).toImmutableList(), buttonHeight, textColor)
                ModifierBarButtonRow(row2.map(presentation).toImmutableList(), buttonHeight, textColor)
            }
        }
    }
}

/** 一个按键按钮的完整呈现状态，由 [ToolbarKey] 派生。 */
private data class ToolbarKeyPresentation(
    val label: String,
    val onClick: () -> Unit,
    val modifierState: ModifierState?,
    val testTag: String,
    val contentDescription: String?,
    val onRepeat: (() -> Unit)?,
    val secondaryLabel: String?,
    val secondaryAction: (() -> Unit)?,
)

/** 修饰键栏可触发的回调集合。 */
private data class ModifierBarActions(
    val onKeyClick: (String) -> Unit,
    val onDrawerClick: () -> Unit,
    val onScrollClick: () -> Unit,
    val onToggleCtrl: () -> Unit,
    val onToggleAlt: () -> Unit,
    /** Termux 同款长按锁定 CTRL/ALT（轻点只切换一次性态）。 */
    val onLockCtrl: () -> Unit = {},
    val onLockAlt: () -> Unit = {},
    /** DECCKM application-cursor state — queried on each arrow tap so vim/less arrows work. */
    val isAppCursorMode: () -> Boolean = { false },
    /** Raw-byte channel for modifier-combined keys (avoids String charset round-trip). */
    val onKeyBytesClick: ((ByteArray) -> Unit)? = null,
    /** Consumes Once sticky modifiers after a modified key is sent. */
    val onConsumeModifiers: () -> Unit = {},
)

/** The live toggle states of the modifier keys. */
private data class ModifierBarStates(
    val ctrlState: ModifierState,
    val altState: ModifierState,
    val scrollActive: Boolean,
)

/** 长按动作：CTRL/ALT 锁定（termux 长按）。其余按键无长按动作。 */
private fun secondaryLongPressAction(key: ToolbarKey, actions: ModifierBarActions): (() -> Unit)? = when (key) {
    ToolbarKey.CTRL -> actions.onLockCtrl
    ToolbarKey.ALT -> actions.onLockAlt
    else -> null
}

/** The live toggle state for one [ToolbarKey], or null for non-toggle keys. */
private fun modifierStateFor(key: ToolbarKey?, states: ModifierBarStates): ModifierState? = when (key) {
    ToolbarKey.CTRL -> states.ctrlState
    ToolbarKey.ALT -> states.altState
    ToolbarKey.SCROLL -> if (states.scrollActive) ModifierState.Locked else null
    else -> null
}

private fun toolbarKeyPresentation(
    key: ToolbarKey,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
    contentDescriptionResolver: (ToolbarKey) -> String,
    isAppCursorMode: () -> Boolean = { false },
): ToolbarKeyPresentation {
    val onRepeat =
        if (key.repeatable) {
            {
                sendPlainOrModified(
                    key,
                    arrowOrPlainSequence(arrowKeyCode(key), key.sequence, actions.isAppCursorMode),
                    actions,
                    modifierStates,
                )
            }
        } else {
            null
        }
    return ToolbarKeyPresentation(
        label = key.symbol ?: key.defaultLabel,
        onClick = toolbarKeyClickHandler(key, actions, modifierStates, isAppCursorMode),
        modifierState = modifierStateFor(key, modifierStates),
        testTag = key.testTag ?: "Key_${key.defaultLabel}",
        contentDescription = contentDescriptionResolver(key),
        onRepeat = onRepeat,
        secondaryLabel = null,
        secondaryAction = secondaryLongPressAction(key, actions),
    )
}

/** DECCKM 感知的方向键码，无对应返回空。 */
private fun arrowKeyCode(key: ToolbarKey): Int? = when (key) {
    ToolbarKey.ARROW_UP -> KeyEvent.KEYCODE_DPAD_UP
    ToolbarKey.ARROW_DOWN -> KeyEvent.KEYCODE_DPAD_DOWN
    ToolbarKey.ARROW_LEFT -> KeyEvent.KEYCODE_DPAD_LEFT
    ToolbarKey.ARROW_RIGHT -> KeyEvent.KEYCODE_DPAD_RIGHT
    else -> null
}

/** 方向键按应用光标模式编码，其余键保持原序列。 */
private fun arrowOrPlainSequence(keyCode: Int?, fallbackSequence: String, isAppCursorMode: () -> Boolean): String {
    if (keyCode == null) return fallbackSequence
    return TerminalInputEncoder.arrowSequence(keyCode, isAppCursorMode())
}

/** 可配置键栏普通按键的键码，无对应返回空（修饰键/功能键不参与组合编码）。 */
private fun plainKeyCode(key: ToolbarKey): Int? = when (key) {
    ToolbarKey.ESC -> KeyEvent.KEYCODE_ESCAPE
    ToolbarKey.TAB -> KeyEvent.KEYCODE_TAB
    ToolbarKey.HOME -> KeyEvent.KEYCODE_MOVE_HOME
    ToolbarKey.END -> KeyEvent.KEYCODE_MOVE_END
    ToolbarKey.PGUP -> KeyEvent.KEYCODE_PAGE_UP
    ToolbarKey.PGDN -> KeyEvent.KEYCODE_PAGE_DOWN
    else -> arrowKeyCode(key)
}

/**
 * 普通按键发送：无修饰时原序列直发（零行为变化）；CTRL/ALT 激活时经编码器
 * 组合编码后走字节通道，并消费 Once 粘滞态（Locked 不受影响）。
 */
private fun sendPlainOrModified(
    key: ToolbarKey,
    sequence: String,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
) {
    val ctrlActive =
        modifierStates.ctrlState == ModifierState.Locked || modifierStates.ctrlState == ModifierState.Once
    val altActive =
        modifierStates.altState == ModifierState.Locked || modifierStates.altState == ModifierState.Once
    val keyCode = plainKeyCode(key)
    val bytesClick = actions.onKeyBytesClick
    if ((!ctrlActive && !altActive) || keyCode == null || bytesClick == null) {
        actions.onKeyClick(sequence)
        return
    }
    val encoded =
        TerminalInputEncoder.encodeKeyEvent(
            keyCode = keyCode,
            unicodeChar = 0,
            ctrlActive = ctrlActive,
            altActive = altActive,
            appCursorMode = actions.isAppCursorMode(),
        ) ?: return
    bytesClick(encoded)
    actions.onConsumeModifiers()
}

private fun toolbarKeyClickHandler(
    key: ToolbarKey,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
    isAppCursorMode: () -> Boolean = { false },
): () -> Unit = when (key) {
    ToolbarKey.CTRL -> actions.onToggleCtrl

    ToolbarKey.ALT -> actions.onToggleAlt

    ToolbarKey.DRAWER -> actions.onDrawerClick

    ToolbarKey.SCROLL -> actions.onScrollClick

    ToolbarKey.ARROW_UP,
    ToolbarKey.ARROW_DOWN,
    ToolbarKey.ARROW_LEFT,
    ToolbarKey.ARROW_RIGHT,
    -> {
        // 无修饰走 DECCKM 感知序列；有修饰走 CSI mod 编码（与硬件路径一致）。
        val keyCode = arrowKeyCode(key) ?: return { }
        {
            sendPlainOrModified(
                key,
                arrowOrPlainSequence(keyCode, key.sequence, isAppCursorMode),
                actions,
                modifierStates,
            )
        }
    }

    else ->
        if (key.sequence.isEmpty()) {
            { }
        } else {
            { sendPlainOrModified(key, key.sequence, actions, modifierStates) }
        }
}

/** One full-width row of extra-key buttons from pre-computed presentations. */
@Composable
private fun ModifierBarButtonRow(items: ImmutableList<ToolbarKeyPresentation>, buttonHeight: Dp, textColor: Color) {
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
                secondaryLabel = item.secondaryLabel,
                secondaryAction = item.secondaryAction,
            )
        }
    }
}

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
    widthWeight: Int = 1,
    secondaryLabel: String? = null,
    secondaryAction: (() -> Unit)? = null,
) {
    val isLocked = modifierState == ModifierState.Locked
    val isOnce = modifierState == ModifierState.Once

    var isPressed by remember { mutableStateOf(false) }

    val scale by
        animateFloatAsState(
            targetValue = if (isPressed) 0.90f else 1f,
            animationSpec =
            spring(
                dampingRatio = PRESS_SCALE_SPRING_DAMPING,
                stiffness = PRESS_SCALE_SPRING_STIFFNESS,
            ),
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
    val animatedBg by
        animateColorAsState(
            targetValue = targetBg,
            animationSpec = tween(durationMillis = PRESS_BG_TWEEN_MS),
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

    // the old pointerInput(onRepeat,
    // secondaryAction) keyed on freshly-allocated lambda instances, so ANY
    // recomposition during a hold (CTRL toggle, pager state change) cancelled
    // and restarted the gesture coroutine — silently killing auto-repeat.
    // Key on Unit and read the latest callbacks via rememberUpdatedState.
    val currentOnClick by rememberUpdatedState(onClick)
    val currentOnRepeat by rememberUpdatedState(onRepeat)
    val currentSecondaryAction by rememberUpdatedState(secondaryAction)
    val gestureModifier =
        Modifier.pointerInput(Unit) {
            awaitEachGesture {
                awaitFirstDown()
                val downPos = currentEvent.changes.first().position
                val slop = viewConfiguration.touchSlop
                // ACTION_CANCEL 会被 MotionEventAdapter 直接丢弃，改由 processCancel 合成
                // 无 MotionEvent 的全释放事件送达；真实抬手/移动始终携带原始 MotionEvent。
                var sawRawMotionEvent = currentEvent.motionEvent != null
                fun PointerEvent.isGestureCancelled(): Boolean {
                    val raw = motionEvent
                    if (raw != null) {
                        sawRawMotionEvent = true
                        return raw.actionMasked == MotionEvent.ACTION_CANCEL
                    }
                    return sawRawMotionEvent && changes.all { !it.pressed }
                }
                isPressed = true
                try {
                    var gestureValid = true
                    if (currentSecondaryAction != null) {
                        // Keys with a secondary action (DRAWER → paste):
                        // a quick tap fires onClick IMMEDIATELY (no
                        // long-press confirmation window), while a
                        // sustained press past LONG_PRESS_MS triggers
                        // secondaryAction once. Mirrors Android's
                        // onClick/onLongClick split.
                        var longPressTriggered = false
                        val downTime = System.currentTimeMillis()
                        fun maybeFireLongPress() {
                            if (
                                !longPressTriggered &&
                                System.currentTimeMillis() - downTime >= LONG_PRESS_MS
                            ) {
                                longPressTriggered = true
                                view.performHapticFeedback(
                                    android.view.HapticFeedbackConstants.LONG_PRESS,
                                )
                                currentSecondaryAction?.invoke()
                            }
                        }
                        while (true) {
                            val ev = awaitPointerEvent()
                            if (ev.isGestureCancelled()) {
                                gestureValid = false
                                break
                            }
                            val ch = ev.changes.first()
                            // A perfectly still hold produces no MOVE events,
                            // so the release itself must also count when the
                            // threshold already passed — otherwise the
                            // long-press is silently lost.
                            if (!ch.pressed) {
                                maybeFireLongPress()
                                break
                            }
                            if ((ch.position - downPos).getDistance() > slop) {
                                gestureValid = false
                                break
                            }
                            maybeFireLongPress()
                        }
                        if (!longPressTriggered && gestureValid) {
                            view.performHapticFeedback(
                                android.view.HapticFeedbackConstants.KEYBOARD_TAP,
                            )
                            currentOnClick()
                        }
                    } else {
                        // 底部上滑绝不能触发按键：松手确认，滑出即取消。单击在抬手时触发一次；
                        // 可重复键按住超阈值后以 80ms 节奏重复，任一阶段滑出均吞掉剩余手势。
                        if (currentOnRepeat == null) {
                            var tapValid = true
                            while (true) {
                                val ev = awaitPointerEvent()
                                if (ev.isGestureCancelled()) {
                                    tapValid = false
                                    break
                                }
                                val ch = ev.changes.first()
                                if (!ch.pressed) break
                                if ((ch.position - downPos).getDistance() > slop) {
                                    tapValid = false
                                    break
                                }
                            }
                            if (tapValid) {
                                view.performHapticFeedback(
                                    android.view.HapticFeedbackConstants.KEYBOARD_TAP,
                                )
                                currentOnClick()
                            } else if (!tapValid && currentEvent.changes.any { it.pressed }) {
                                // 滑出后指针仍按下：排空到抬手；取消时已全部释放，
                                // 不得再等事件，否则会吞掉下一次手势的 DOWN。
                                while (currentEvent.changes.any { it.pressed }) {
                                    awaitPointerEvent()
                                }
                            }
                        } else {
                            var nextRepeatAt =
                                System.currentTimeMillis() + AUTO_REPEAT_INITIAL_DELAY_MS
                            var repeatFired = false
                            var repeatValid = true
                            while (currentOnRepeat != null) {
                                val remaining = nextRepeatAt - System.currentTimeMillis()
                                // withTimeout(0) 在 PointerEventHandlerCoroutine 内与事件送达竞态，
                                // 触发重复恢复崩溃（Already resumed）；期限已过直接按超时走。
                                val ev =
                                    if (remaining > 0) {
                                        withTimeoutOrNull(remaining) {
                                            awaitPointerEvent()
                                        }
                                    } else {
                                        null
                                    }
                                if (ev == null) {
                                    if (repeatValid) {
                                        if (!repeatFired) {
                                            view.performHapticFeedback(
                                                android.view.HapticFeedbackConstants.KEYBOARD_TAP,
                                            )
                                            currentOnClick()
                                            repeatFired = true
                                        } else {
                                            currentOnRepeat?.invoke()
                                        }
                                    }
                                    nextRepeatAt += AUTO_REPEAT_INTERVAL_MS
                                    continue
                                }
                                if (ev.isGestureCancelled()) {
                                    repeatValid = false
                                    break
                                }
                                val ch = ev.changes.first()
                                if (!ch.pressed) break
                                if ((ch.position - downPos).getDistance() > slop) {
                                    repeatValid = false
                                    break
                                }
                            }
                            if (!repeatFired && repeatValid) {
                                view.performHapticFeedback(
                                    android.view.HapticFeedbackConstants.KEYBOARD_TAP,
                                )
                                currentOnClick()
                            }
                            if (!repeatValid && currentEvent.changes.any { it.pressed }) {
                                while (currentEvent.changes.any { it.pressed }) {
                                    awaitPointerEvent()
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
        Modifier.weight(weight = widthWeight.coerceAtLeast(1).toFloat())
            .height(BUTTON_HEIGHT_DP.dp)
            .then(if (testTag.isNotEmpty()) Modifier.testTag(testTag) else Modifier)
            .then(
                Modifier.background(
                    animatedBg,
                    RoundedCornerShape(4.dp),
                ),
            )
            .then(
                Modifier.semantics {
                    if (contentDescription != null) this.contentDescription = contentDescription
                    // Toggle keys (CTRL/ALT/FN) expose their armed state so
                    // UI tests (and accessibility) can verify the toggle
                    // without probing colors.
                    selected = modifierState != null && modifierState != ModifierState.Off
                },
            )
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .then(gestureModifier),
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
