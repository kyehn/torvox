package terminal.emulator.ui

import android.view.MotionEvent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
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
import androidx.compose.runtime.Immutable
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
import kotlinx.collections.immutable.toImmutableList
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.R
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.ModifierState
import terminal.emulator.runtime.LogUtil
import terminal.emulator.settings.ExtraKey
import terminal.emulator.settings.ExtraKeysStyle
import terminal.emulator.settings.TerminalKeyCatalog
import terminal.emulator.settings.TermuxProperties

/** 单个修饰键按钮高度（dp）。网格预留高度按行数由它派生（见 TerminalRuntime 的 MODIFIER_BAR_HEIGHT_DP）。 */
internal const val BUTTON_HEIGHT_DP = 36
private const val BUTTON_FONT_SIZE_SP = 10

/**
 * 键栏上的一个按键，由 `extra-keys` 配置解析而来。
 *
 * 三种来源互斥：内置键码（[keyCode]）、特殊动作（[special]）、字面量文本
 * （两者皆空，直接发送 [label] 的原文）。[testTag] 取逻辑键名而非显示字形：
 * `extra-keys-style` 会把 `HOME` 画成 `⇱`，但测试与无障碍定位必须跨样式稳定。
 */
@Immutable
data class ExtraKeySlot(
    val name: String,
    val label: String,
    val keyCode: Int? = null,
    val special: TerminalKeyCatalog.Special? = null,
    val repeatable: Boolean = false,
    /** 长按时发送的文本（`{key: '-', popup: '|'}`）。 */
    val popup: String? = null,
    val testTag: String,
)

/**
 * `extra-keys` 配置 → 可渲染的按键槽位。
 *
 * 不可渲染的键跳过并记警告：把它当字面量渲染会让用户点出一串无意义的字符，静默丢弃
 * 至少留下证据。判据统一走 [unrenderableReason]，与 [isRenderableKey] 同源——两者一旦
 * 分家，预留高度就会与实际键栏高度不符。
 */
internal fun buildExtraKeySlots(
    rows: List<List<ExtraKey>>,
    style: ExtraKeysStyle,
    allCaps: Boolean,
): List<List<ExtraKeySlot>> = rows.mapNotNull { row ->
    val slots =
        row.mapNotNull { key ->
            unrenderableReason(key)?.let { reason ->
                LogUtil.warn("ModifierBar", "extra-keys: $reason; key hidden")
                return@mapNotNull null
            }
            ExtraKeySlot(
                name = key.name,
                label = key.display?.let { display -> if (allCaps) display.uppercase() else display }
                    ?: TerminalKeyCatalog.displayLabel(key.name, style, allCaps),
                keyCode = TerminalKeyCatalog.keyCodeFor(key.name),
                special = TerminalKeyCatalog.specialFor(key.name),
                repeatable = TerminalKeyCatalog.isRepetitive(key.name),
                popup = key.popup,
                testTag = testTagFor(key.name),
            )
        }
    slots.takeIf { it.isNotEmpty() }
}

/**
 * 测试标签：方向键沿用字形（`Key_↑`，与既有 UI 测试一致），其余用逻辑键名。
 * 字面量按键用其字符本身。
 */
private fun testTagFor(name: String): String {
    val arrow =
        mapOf(
            "UP" to "↑",
            "DOWN" to "↓",
            "LEFT" to "←",
            "RIGHT" to "→",
        )[name.uppercase()]
    return "Key_" + (arrow ?: name)
}

/** 默认布局（termux 默认值），供未配置 `extra-keys` 时使用。 */
internal fun defaultExtraKeySlots(
    style: ExtraKeysStyle = ExtraKeysStyle.DEFAULT,
    allCaps: Boolean = true,
): List<List<ExtraKeySlot>> = buildExtraKeySlots(TermuxProperties.DEFAULT_EXTRA_KEYS, style, allCaps)

/**
 * 这个按键不可渲染的原因；null 表示可渲染。
 *
 * 单一判据：宏没有播放器、FN 需要一整套二次映射（未实现）、未知键名按字面量渲染
 * 会点出无意义字符，三者都不渲染。[buildExtraKeySlots] 与 [extraKeyRowCount] 都经
 * 此处，任何一侧单独写一份判定都会让「预留的行数」与「实际画出的行数」分家。
 */
private fun unrenderableReason(key: ExtraKey): String? {
    if (key.macro != null) return "macro '${key.macro}' is not supported"
    val special = TerminalKeyCatalog.specialFor(key.name)
    return when {
        special == TerminalKeyCatalog.Special.FN -> "FN is not supported"
        special != null -> null
        TerminalKeyCatalog.keyCodeFor(key.name) != null -> null
        key.name.length == 1 -> null
        else -> "unknown key '${key.name}'"
    }
}

/** 这个按键是否会被渲染。 */
internal fun isRenderableKey(key: ExtraKey): Boolean = unrenderableReason(key) == null

/**
 * 键栏实际渲染的行数，供网格预留高度使用。
 *
 * 必须与 [buildExtraKeySlots] 给出同一个数：预留高度与实际键栏高度一旦分家，键栏就会
 * 盖住终端底部若干行——它盖在 Surface 之上，被盖的行既看不见也点不到。空行与整行都
 * 被过滤掉的行（例如只写了 `FN`）不算行。
 */
internal fun extraKeyRowCount(rows: List<List<ExtraKey>>): Int = rows.count { row -> row.any { isRenderableKey(it) } }

/** 单键高度（dp）：`terminal-toolbar-height` 是相对默认值的缩放系数。 */
internal fun buttonHeightDp(scale: Float): Float = BUTTON_HEIGHT_DP * scale

// Termux ExtraKeysView 语义：长按阈值取系统值（随无障碍「长按时长」变化），
// 超出合理区间时退回 400ms；自动重复在同一阈值后开始，以 80ms 节奏重复。
private const val MIN_LONG_PRESS_MS = 200L
private const val MAX_LONG_PRESS_MS = 3000L
private const val FALLBACK_LONG_PRESS_MS = 400L
private const val AUTO_REPEAT_INTERVAL_MS = 80L

internal fun modifierLongPressMillis(systemTimeoutMs: Long): Long =
    systemTimeoutMs.takeIf { it in MIN_LONG_PRESS_MS..MAX_LONG_PRESS_MS } ?: FALLBACK_LONG_PRESS_MS

// 按下反馈阈值。
internal const val PRESS_BG_TWEEN_MS = 30
internal const val PRESS_SCALE_TWEEN_MS = 90

// 按下反馈的延迟：覆盖系统底部上滑从 ACTION_DOWN 到 ACTION_CANCEL 的窗口。
internal const val PRESS_FEEDBACK_DELAY_MS = 40

// 按下态的缩放。
private const val PRESSED_SCALE = 0.90f

// 按下态的灰色遮罩（无主题色时的中性反馈）。
private val PRESSED_SCRIM = Color(0xFF7F7F7F)

/** 横向分页：第 0 页按键，第 1 页文本输入。 */
private const val TEXT_INPUT_PAGE_INDEX = 1
private const val KEY_PAGE_COUNT = TEXT_INPUT_PAGE_INDEX + 1

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
        Modifier
            .fillMaxWidth()
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

/**
 * 修饰键栏：按 `extra-keys` 布局渲染，[layout] 为 null 时使用 termux 默认两行。
 *
 * 每个按键的语义由 [ExtraKeySlot] 决定，布局本身不再写死在组合里；`extra-keys-style`
 * 只改变显示字形（在构建槽位时已应用），`extra-keys-text-all-caps` 同理。
 * 所有按钮无边框、背景透明，权重相等以保证尺寸一致。
 */
@Composable
fun ModifierBar(
    onSubmit: (String) -> Unit,
    /**
     * 发送一个会产生终端输入的按键。`modifiers` 取上游 `key.Mods` 原始位。
     *
     * 返回 false 表示该键未被发送（无对应键码或终端的当前模式决定它不产生字节）：
     * 调用方据此保留证据并消耗一次性粘滞态。
     */
    onKey: (keyCode: Int, modifiers: Int) -> Boolean,
    modifier: Modifier = Modifier,
    onDrawerClick: () -> Unit = {},
    onScrollClick: () -> Unit = {},
    /** SCROLL 键锁定状态：决定按钮的选中与高亮。 */
    scrollActive: Boolean = false,
    ctrlState: ModifierState = ModifierState.Off,
    altState: ModifierState = ModifierState.Off,
    onToggleCtrl: () -> Unit = {},
    onToggleAlt: () -> Unit = {},
    /** Termux 同款长按锁定 CTRL/ALT（轻点只切换一次性态）。 */
    onLockCtrl: () -> Unit = {},
    onLockAlt: () -> Unit = {},
    /** Termux `DRAWER` 键长按：粘贴剪贴板（`popup: 'PASTE'`）。 */
    onPasteClick: () -> Unit = {},
    /** `KEYBOARD` 键：切换软键盘（termux 的软键盘行为键）。 */
    onKeyboardClick: () -> Unit = {},
    textColor: Color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.85f),
    backgroundColor: Color = MaterialTheme.colorScheme.surface,
    /** 发送带修饰键的按键后，消耗一次性（`Once`）粘滞修饰键。 */
    onConsumeModifiers: () -> Unit = {},
    /** 键栏布局；null 表示 termux 默认两行。 */
    layout: List<List<ExtraKeySlot>>? = null,
    /** 单键高度相对默认值的缩放（`terminal-toolbar-height`）。 */
    buttonHeightScale: Float = 1f,
) {
    val buttonHeight = buttonHeightDp(buttonHeightScale).dp
    val rows = layout ?: remember { defaultExtraKeySlots() }
    // 第 0 页是按键页，第 1 页是文本输入页（DESIGN：左滑进入文本输入框）。
    val pagerState = rememberPagerState(pageCount = { KEY_PAGE_COUNT })
    val actions =
        ModifierBarActions(
            onSubmit = onSubmit,
            onDrawerClick = onDrawerClick,
            onScrollClick = onScrollClick,
            onToggleCtrl = onToggleCtrl,
            onToggleAlt = onToggleAlt,
            onLockCtrl = onLockCtrl,
            onLockAlt = onLockAlt,
            onPasteClick = onPasteClick,
            onKeyboardClick = onKeyboardClick,
            onKey = onKey,
            onConsumeModifiers = onConsumeModifiers,
        )
    val modifierStates =
        ModifierBarStates(
            ctrlState = ctrlState,
            altState = altState,
            scrollActive = scrollActive,
        )
    val presentation = { slot: ExtraKeySlot ->
        toolbarKeyPresentation(
            slot = slot,
            actions = actions,
            modifierStates = modifierStates,
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
                    onSubmit = onSubmit,
                )
                return@HorizontalPager
            }
            Column {
                for (row in rows) {
                    ModifierBarButtonRow(row.map(presentation).toImmutableList(), buttonHeight, textColor)
                }
            }
        }
    }
}

/** 一个按键按钮的完整呈现状态，由 [ExtraKeySlot] 派生。 */
private data class ToolbarKeyPresentation(
    val label: String,
    val onClick: () -> Unit,
    val modifierState: ModifierState?,
    val testTag: String,
    val contentDescription: String?,
    val onRepeat: (() -> Unit)?,
    val secondaryAction: (() -> Unit)?,
)

/** 修饰键栏可触发的回调集合。 */
@Immutable
internal data class ModifierBarActions(
    val onSubmit: (String) -> Unit,
    val onDrawerClick: () -> Unit,
    val onScrollClick: () -> Unit,
    val onToggleCtrl: () -> Unit,
    val onToggleAlt: () -> Unit,
    /** Termux 同款长按锁定 CTRL/ALT（轻点只切换一次性态）。 */
    val onLockCtrl: () -> Unit = {},
    val onLockAlt: () -> Unit = {},
    /** Termux `DRAWER` 键的 `popup: 'PASTE'`：长按粘贴剪贴板。 */
    val onPasteClick: () -> Unit = {},
    /** `KEYBOARD` 键：切换软键盘。 */
    val onKeyboardClick: () -> Unit = {},
    /** 发送会产生终端输入的按键；返回 false 表示未被发送。 */
    val onKey: (keyCode: Int, modifiers: Int) -> Boolean = { _, _ -> false },
    /** 发送带修饰键的按键后，消耗一次性（`Once`）粘滞修饰键。 */
    val onConsumeModifiers: () -> Unit = {},
)

/** 修饰键的实时切换状态。 */
@Immutable
private data class ModifierBarStates(
    val ctrlState: ModifierState,
    val altState: ModifierState,
    val scrollActive: Boolean,
)

/** 长按动作：CTRL/ALT 锁定、DRAWER 粘贴；配置了 popup 的按键发送 popup 文本。 */
private fun secondaryLongPressAction(slot: ExtraKeySlot, actions: ModifierBarActions): (() -> Unit)? =
    when (slot.special) {
        TerminalKeyCatalog.Special.CTRL -> actions.onLockCtrl

        TerminalKeyCatalog.Special.ALT -> actions.onLockAlt

        TerminalKeyCatalog.Special.DRAWER -> actions.onPasteClick

        else ->
            slot.popup?.let { popup ->
                { actions.onSubmit(popup) }
            }
    }

/** 单个按键的实时切换状态；非切换类按键为 `null`。 */
private fun modifierStateFor(slot: ExtraKeySlot, states: ModifierBarStates): ModifierState? = when (slot.special) {
    TerminalKeyCatalog.Special.CTRL -> states.ctrlState
    TerminalKeyCatalog.Special.ALT -> states.altState
    TerminalKeyCatalog.Special.SCROLL -> if (states.scrollActive) ModifierState.Locked else null
    else -> null
}

private fun toolbarKeyPresentation(
    slot: ExtraKeySlot,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
): ToolbarKeyPresentation {
    val onRepeat =
        if (slot.repeatable) {
            {
                sendKeyThroughEncoder(slot, actions, modifierStates)
            }
        } else {
            null
        }
    return ToolbarKeyPresentation(
        label = slot.label,
        onClick = toolbarKeyClickHandler(slot, actions, modifierStates),
        modifierState = modifierStateFor(slot, modifierStates),
        testTag = slot.testTag,
        contentDescription = slot.label,
        onRepeat = onRepeat,
        secondaryAction = secondaryLongPressAction(slot, actions),
    )
}

/**
 * 按键发送：键码一律经上游键编码器（`onKey` → `Bridge.processKeyEvent` → 原生
 * `key::Encoder`），修饰键由编码器与终端状态共同决定形态；字面量（`/`、`|` 等）
 * 直接作为文本写出——它们没有可编码的逻辑键。
 *
 * 键栏不再自带转义序列字面量：方向键在 DECCKM 下须走 SS3、Ctrl 组合在
 * modifyOtherKeys 下须走 `CSI 27;mod;code~`，这些形态只有编码器知道，写死在
 * Kotlin 的串在对应模式下就是错的。
 */
private fun sendKeyThroughEncoder(slot: ExtraKeySlot, actions: ModifierBarActions, modifierStates: ModifierBarStates) {
    val keyCode = slot.keyCode
    if (keyCode == null) {
        actions.onSubmit(slot.name)
        return
    }
    val modifiers = KeyModifiers.fromStickyStates(modifierStates.ctrlState, modifierStates.altState)
    if (actions.onKey(keyCode, modifiers)) {
        actions.onConsumeModifiers()
        return
    }
    // 按下 CTRL/ALT 再点某个键却什么都没发生——用户只能以为设备卡了。
    // 键栏的每个键都应可编码，故这是编码缺口，须留证据（键码 + 修饰态）。
    LogUtil.warn(
        "ModifierBar",
        "no encoding for ${slot.name} (keyCode=$keyCode modifiers=$modifiers); keystroke dropped",
    )
}

private fun toolbarKeyClickHandler(
    slot: ExtraKeySlot,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
): () -> Unit = when (slot.special) {
    TerminalKeyCatalog.Special.CTRL -> actions.onToggleCtrl

    TerminalKeyCatalog.Special.ALT -> actions.onToggleAlt

    TerminalKeyCatalog.Special.DRAWER -> actions.onDrawerClick

    TerminalKeyCatalog.Special.SCROLL -> actions.onScrollClick

    TerminalKeyCatalog.Special.KEYBOARD -> actions.onKeyboardClick

    TerminalKeyCatalog.Special.PASTE -> actions.onPasteClick

    TerminalKeyCatalog.Special.FN, null -> {
        if (slot.keyCode == null && slot.popup == null) {
            // 字面量：没有键码，直接写文本。
            { actions.onSubmit(slot.name) }
        } else {
            { sendKeyThroughEncoder(slot, actions, modifierStates) }
        }
    }
}

/** 由预计算的呈现结果组成的一整行附加键按钮。 */
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
                secondaryAction = item.secondaryAction,
                height = buttonHeight,
            )
        }
    }
}

@Composable
private fun RowScope.ExtraKeyButton(
    text: String,
    onClick: () -> Unit,
    textColor: androidx.compose.ui.graphics.Color,
    modifierState: ModifierState? = null,
    testTag: String = "",
    contentDescription: String? = null,
    onRepeat: (() -> Unit)? = null,
    secondaryAction: (() -> Unit)? = null,
    height: Dp = BUTTON_HEIGHT_DP.dp,
) {
    val isLocked = modifierState == ModifierState.Locked
    val isOnce = modifierState == ModifierState.Once

    var isPressed by remember { mutableStateOf(false) }

    val scale by
        animateFloatAsState(
            targetValue = if (isPressed) PRESSED_SCALE else 1f,
            // 按下反馈延迟启动：系统底部上滑先给 ACTION_DOWN、再把触摸认领走并补发
            // ACTION_CANCEL。按下即缩放会让取消前的一两帧闪一次动画（用户看到
            // 「上滑触发按键动画」）。延迟覆盖这段窗口，取消时目标未变、无位移。
            //
            // 复位一律瞬时（snap）：手势被系统认领后若再插值一遍复位，按键会
            // 先缩后弹地「动」一下，正是用户看到的按键动画。抬手同样瞬时复位，
            // 按下反馈仍由按下方向的插值给出。
            animationSpec =
            if (isPressed) {
                tween(
                    durationMillis = PRESS_SCALE_TWEEN_MS,
                    delayMillis = PRESS_FEEDBACK_DELAY_MS,
                )
            } else {
                snap()
            },
            label = "btnScale",
        )

    val targetBg =
        when {
            isLocked -> MaterialTheme.colorScheme.primary
            isOnce -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.5f)
            isPressed -> PRESSED_SCRIM
            else -> Color.Transparent
        }
    val animatedBg by
        animateColorAsState(
            targetValue = targetBg,
            // 与缩放同一口径：按下才插值，抬起与取消瞬时复位。
            animationSpec =
            if (isPressed) {
                tween(
                    durationMillis = PRESS_BG_TWEEN_MS,
                    delayMillis = PRESS_FEEDBACK_DELAY_MS,
                )
            } else {
                snap()
            },
            label = "btnBg",
        )
    val activeFg =
        when {
            isLocked -> MaterialTheme.colorScheme.onPrimary
            isOnce -> MaterialTheme.colorScheme.primary
            else -> textColor
        }
    val fontWeight =
        when {
            isLocked -> FontWeight.Bold
            isOnce -> FontWeight.Bold
            else -> FontWeight.Normal
        }

    val view = LocalView.current

    // 旧的 pointerInput(onRepeat, secondaryAction) 以新分配的 lambda 实例为 key，
    // 于是长按期间（CTRL 切换、分页状态变化）的任何重组都会取消并重启手势协程
    // ——静默地杀死自动重复。改以 Unit 为 key 并经 rememberUpdatedState 读取最新回调。
    val currentOnClick by rememberUpdatedState(onClick)
    val currentOnRepeat by rememberUpdatedState(onRepeat)
    val currentSecondaryAction by rememberUpdatedState(secondaryAction)
    val gestureModifier =
        Modifier.pointerInput(Unit) {
            awaitEachGesture {
                val down = awaitFirstDown()
                val downPos = down.position
                // 长按阈值取手势事件时间（单调 uptime）而非墙钟：用户改系统时间
                // 不会让长按误判，测试的虚拟时钟也推进同一条时间线。
                val downEventTime = down.uptimeMillis
                val touchSlop = viewConfiguration.touchSlop
                val longPressMs = modifierLongPressMillis(viewConfiguration.longPressTimeoutMillis)
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
                        // 带次级动作的按键（DRAWER → 粘贴）：轻点立即触发 onClick
                        // （无长按确认窗口），而超过长按阈值的持续按压
                        // 只触发一次 secondaryAction。对应 Android 的
                        // onClick/onLongClick 划分。
                        var longPressTriggered = false

                        fun maybeFireLongPress(eventTimeMillis: Long) {
                            if (
                                !longPressTriggered &&
                                eventTimeMillis - downEventTime >= longPressMs
                            ) {
                                longPressTriggered = true
                                view.performHapticFeedback(
                                    android.view.HapticFeedbackConstants.LONG_PRESS,
                                )
                                currentSecondaryAction?.invoke()
                            }
                        }
                        while (true) {
                            val pointerEvent = awaitPointerEvent()
                            if (pointerEvent.isGestureCancelled()) {
                                gestureValid = false
                                break
                            }
                            val pointerInput = pointerEvent.changes.first()
                            // 完全静止的长按不产生任何 MOVE 事件，
                            // 故阈值已过时松手本身也必须计入——否则长按会被静默丢失。
                            if (!pointerInput.pressed) {
                                maybeFireLongPress(pointerInput.uptimeMillis)
                                break
                            }
                            if ((pointerInput.position - downPos).getDistance() > touchSlop) {
                                gestureValid = false
                                break
                            }
                            maybeFireLongPress(pointerInput.uptimeMillis)
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
                                val pointerEvent = awaitPointerEvent()
                                if (pointerEvent.isGestureCancelled()) {
                                    tapValid = false
                                    break
                                }
                                val pointerInput = pointerEvent.changes.first()
                                if (!pointerInput.pressed) break
                                if ((pointerInput.position - downPos).getDistance() > touchSlop) {
                                    tapValid = false
                                    break
                                }
                            }
                            if (tapValid) {
                                view.performHapticFeedback(
                                    android.view.HapticFeedbackConstants.KEYBOARD_TAP,
                                )
                                currentOnClick()
                            } else if (currentEvent.changes.any { it.pressed }) {
                                // 滑出后指针仍按下：排空到抬手；取消时已全部释放，
                                // 不得再等事件，否则会吞掉下一次手势的 DOWN。
                                while (currentEvent.changes.any { it.pressed }) {
                                    awaitPointerEvent()
                                }
                            }
                        } else {
                            // 单调 uptime，与上方长按阈值同一时间线：墙钟前跳会让
                            // remaining 变负，循环再不 await 事件而是空转连发。
                            var nextRepeatAt =
                                android.os.SystemClock.uptimeMillis() + longPressMs
                            var repeatFired = false
                            var repeatValid = true
                            while (currentOnRepeat != null) {
                                val remaining = nextRepeatAt - android.os.SystemClock.uptimeMillis()
                                // withTimeout(0) 在 PointerEventHandlerCoroutine 内与事件送达竞态，
                                // 触发重复恢复崩溃（Already resumed）；期限已过直接按超时走。
                                val pointerEvent =
                                    if (remaining > 0) {
                                        withTimeoutOrNull(remaining) {
                                            awaitPointerEvent()
                                        }
                                    } else {
                                        null
                                    }
                                if (pointerEvent == null) {
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
                                if (pointerEvent.isGestureCancelled()) {
                                    repeatValid = false
                                    break
                                }
                                val pointerInput = pointerEvent.changes.first()
                                if (!pointerInput.pressed) break
                                if ((pointerInput.position - downPos).getDistance() > touchSlop) {
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
        Modifier
            .weight(1f)
            .height(height)
            .then(if (testTag.isNotEmpty()) Modifier.testTag(testTag) else Modifier)
            .then(
                Modifier.background(
                    animatedBg,
                    RoundedCornerShape(4.dp),
                ),
            ).then(
                Modifier.semantics {
                    if (contentDescription != null) this.contentDescription = contentDescription
                    // 切换类按键（CTRL/ALT/FN）暴露其置起状态，
                    // 使 UI 测试（与无障碍）无需探测颜色即可验证切换。
                    selected = modifierState != null && modifierState != ModifierState.Off
                },
            ).graphicsLayer {
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
