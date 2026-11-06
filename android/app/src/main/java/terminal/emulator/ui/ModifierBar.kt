package terminal.emulator.ui

import android.os.SystemClock
import android.view.HapticFeedbackConstants
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
import androidx.compose.ui.text.TextStyle
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
import terminal.emulator.settings.MacroParser
import terminal.emulator.settings.MacroStep
import terminal.emulator.settings.TerminalKeyCatalog
import terminal.emulator.settings.TermuxProperties

/** 单个修饰键按钮高度（dp）。网格预留高度按行数由它派生（见 TerminalRuntime 的 MODIFIER_BAR_HEIGHT_DP）。 */
internal const val BUTTON_HEIGHT_DP = 36
private const val BUTTON_FONT_SIZE_SP = 10

/**
 * 键栏上一个键「按下时做什么」。
 *
 * 四种动作互斥：走原生键编码器的逻辑键、键栏自身的特殊动作、直接写出的字面量文本、
 * 宏。主键与长按次级键（`popup`）共用这一套动作——上游 termux 的 popup 就是一个
 * 完整的按键，故两者的语义必须一致，不能一个按键名一个当纯文本。
 */
@Immutable
sealed interface KeyAction {
    /** 走原生键编码器的逻辑键（方向键、ESC、F1……）。 */
    data class Code(val keyCode: Int, val name: String) : KeyAction

    /** 键栏自身处理的动作：CTRL/ALT/DRAWER/SCROLL/KEYBOARD/PASTE。 */
    data class Special(val special: TerminalKeyCatalog.Special) : KeyAction

    /** 直接写出的文本（`/`、`|` 等字面量按键）。 */
    data class Text(val text: String) : KeyAction

    /** 宏：按顺序发一串键（`{macro: "CTRL f d"}`）。 */
    data class Macro(val steps: List<MacroStep>) : KeyAction
}

/**
 * 键栏上的一个按键，由 `extra-keys` 配置解析而来。
 *
 * [action] 是这个键按下时执行的动作；[popup] 是长按时执行的次级键（同样是一个
 * 动作）。[testTag] 取逻辑键名而非显示字形：`extra-keys-style` 会把 `HOME` 画成
 * `⇱`，但测试与无障碍定位必须跨样式稳定。
 */
@Immutable
data class ExtraKeySlot(
    /** 逻辑键名（测试标签与日志用）；字面量键为其原文。 */
    val name: String,
    /** 键帽文本。 */
    val label: String,
    /** 按下时执行的动作。 */
    val action: KeyAction,
    /** 按住是否连发（上游 `PRIMARY_REPETITIVE_KEYS`）。 */
    val repeatable: Boolean = false,
    /** 长按时执行的次级键（`popup`）；null 表示长按无动作。 */
    val popup: KeyAction? = null,
    /** 测试与无障碍定位用的稳定标签。 */
    val testTag: String,
) {
    /** 这个键的特殊动作（CTRL/ALT/DRAWER/…）；不是特殊键时为 null。 */
    val special: TerminalKeyCatalog.Special? get() = (action as? KeyAction.Special)?.special
}

/**
 * `extra-keys` 配置 → 可渲染的按键槽位。
 *
 * 不可渲染的键（`FN`/`SHIFT`/含 `FN` 的宏）跳过并记警告：静默丢弃至少留下证据。
 * 判据统一走 [resolveKey]，与 [isRenderableKey] 同源——两者一旦分家，预留高度就会与
 * 实际键栏高度不符。
 */
internal fun buildExtraKeySlots(
    rows: List<List<ExtraKey>>,
    style: ExtraKeysStyle,
    allCaps: Boolean,
): List<List<ExtraKeySlot>> = rows.mapNotNull { row ->
    val slots =
        row.mapNotNull { key ->
            val resolved =
                when (val resolution = resolveKey(key)) {
                    is KeyResolution.Resolved -> resolution

                    is KeyResolution.Unusable -> {
                        LogUtil.warn("ModifierBar", "extra-keys: ${resolution.reason}; key hidden")
                        return@mapNotNull null
                    }
                }
            val action = resolved.action
            ExtraKeySlot(
                // 字面量保留原文的大小写；键帽的全大写只影响 label，不改变发送内容。
                name =
                when (action) {
                    is KeyAction.Macro -> key.macro.orEmpty()
                    is KeyAction.Text -> action.text
                    else -> resolved.normalizedName
                },
                label =
                key.display?.let { display -> if (allCaps) display.uppercase() else display }
                    ?: if (action is KeyAction.Macro) {
                        MacroParser.display(key.macro.orEmpty(), style, allCaps)
                    } else {
                        // 传原名（不是归一名）：`extra-keys-text-all-caps = false` 时
                        // 键帽要保留用户写的大小写（`Tab` → `Tab`）。
                        TerminalKeyCatalog.displayLabel(key.name, style, allCaps)
                    },
                action = action,
                repeatable = action !is KeyAction.Macro && TerminalKeyCatalog.isRepetitive(resolved.normalizedName),
                popup = key.popup?.let { popupKey -> resolvePopup(popupKey) },
                testTag = if (action is KeyAction.Macro) macroTestTag(key) else testTagFor(resolved.normalizedName),
            )
        }
    slots.takeIf { it.isNotEmpty() }
}

/**
 * 一个 `extra-keys` 项的解析结果。
 *
 * 「能不能渲染」与「渲染成什么」必须同源：两处各判一次就会出现「判为可用却拿不到
 * 动作」的空档，键帽画出来却点了没反应。
 */
private sealed interface KeyResolution {
    /** 可渲染：动作与归一键名（测试标签、连发判据都用它）。 */
    data class Resolved(val action: KeyAction, val normalizedName: String) : KeyResolution

    /** 不可渲染：原因（写进日志，让用户知道这个键去哪了）。 */
    data class Unusable(val reason: String) : KeyResolution
}

/**
 * `extra-keys` 一项 → 它的动作。
 *
 * 单一判据：含 `FN` 的宏不可用（本应用没有 FN 二次映射）、`FN` 需要一整套二次映射
 * （未实现）、`SHIFT` 需要一整套粘滞修饰态（本应用只有 CTRL/ALT），三者都不渲染。其余
 * 没有键码的键名按上游语义是字面量文本——termux 里 `-_-` 就是发送这三个字符，不是未知
 * 键名。
 */
private fun resolveKey(key: ExtraKey): KeyResolution {
    val macro = key.macro
    if (macro != null) {
        val steps = MacroParser.parse(macro)
        return if (steps == null) {
            KeyResolution.Unusable("macro '$macro' is not supported")
        } else {
            KeyResolution.Resolved(KeyAction.Macro(steps), TerminalKeyCatalog.normalizeKeyName(key.name))
        }
    }
    val normalized = TerminalKeyCatalog.normalizeKeyName(key.name)
    val special = TerminalKeyCatalog.specialFor(normalized)
    val keyCode = TerminalKeyCatalog.keyCodeFor(normalized)
    return when {
        special == TerminalKeyCatalog.Special.FN -> KeyResolution.Unusable("FN is not supported")

        special != null -> KeyResolution.Resolved(KeyAction.Special(special), normalized)

        normalized == "SHIFT" -> KeyResolution.Unusable("SHIFT is not supported")

        keyCode != null -> KeyResolution.Resolved(KeyAction.Code(keyCode, normalized), normalized)

        // 上游把键名表之外的键名整串当文本发送（`TerminalExtraKeys` 的 else 分支）。
        else ->
            KeyResolution.Resolved(
                KeyAction.Text(TerminalKeyCatalog.literalText(key.name, normalized)),
                normalized,
            )
    }
}

/**
 * `popup`（长按次级键）→ 它的动作；不可用时为 null（长按无动作）。
 *
 * popup 与主键同构：键名走编码器、特殊键走键栏回调、宏走播放器。上游 termux 的
 * popup 就是完整的按键（`ExtraKeysInfo.normalizeKeyConfig`），故这里复用
 * [resolveKey] 而不是另写一套判定。
 */
private fun resolvePopup(popup: ExtraKey): KeyAction? = when (val resolution = resolveKey(popup)) {
    is KeyResolution.Resolved -> resolution.action

    is KeyResolution.Unusable -> {
        LogUtil.warn("ModifierBar", "extra-keys: popup ${resolution.reason}; long-press action dropped")
        null
    }
}

/**
 * 测试标签：方向键沿用字形（`Key_↑`，与既有 UI 测试一致），其余用逻辑键名。
 * 字面量按键用其字符本身。
 */
private val ARROW_TEST_TAGS =
    mapOf(
        "UP" to "↑",
        "DOWN" to "↓",
        "LEFT" to "←",
        "RIGHT" to "→",
    )

/** 方向键的字形标签（与既有 UI 测试一致），其余键用逻辑名。 */
private fun testTagFor(name: String): String = "Key_" + (ARROW_TEST_TAGS[name.uppercase()] ?: name)

/**
 * 宏键的测试标签。
 *
 * 宏没有单一逻辑键名，取 `display`（没有则取宏串）作为标识：两个宏的标签必须
 * 不同，否则 UI 测试按标签定位时会命中同一个按钮。
 */
private fun macroTestTag(key: ExtraKey): String = "Key_MACRO_" + (key.display ?: key.macro.orEmpty()).replace(' ', '_')

/** 默认布局（termux 默认值），供未配置 `extra-keys` 时使用。 */
internal fun defaultExtraKeySlots(
    style: ExtraKeysStyle = ExtraKeysStyle.DEFAULT,
    allCaps: Boolean = true,
): List<List<ExtraKeySlot>> = buildExtraKeySlots(TermuxProperties.DEFAULT_EXTRA_KEYS, style, allCaps)

/**
 * 这个按键是否会被渲染。
 *
 * 与 [buildExtraKeySlots] 同一判据（[resolveKey]）：任何一侧单独写一份判定都会让
 * 「预留的行数」与「实际画出的行数」分家。
 */
internal fun isRenderableKey(key: ExtraKey): Boolean = resolveKey(key) is KeyResolution.Resolved

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
        TextStyle(
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
    /** `KEYBOARD` 键：切换软键盘（termux 的软键盘行为键）。 */
    onKeyboardClick: () -> Unit,
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
    /** 播放宏（`{macro: "CTRL f d"}`）。 */
    onMacro: (steps: List<MacroStep>) -> Unit = {},
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
    val drawerDescription = stringResource(R.string.open_session_drawer)
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
            onMacro = onMacro,
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
            drawerDescription = drawerDescription,
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
    /** `KEYBOARD` 键：切换软键盘。 */
    val onKeyboardClick: () -> Unit,
    /** Termux 同款长按锁定 CTRL/ALT（轻点只切换一次性态）。 */
    val onLockCtrl: () -> Unit = {},
    val onLockAlt: () -> Unit = {},
    /** Termux `DRAWER` 键的 `popup: 'PASTE'`：长按粘贴剪贴板。 */
    val onPasteClick: () -> Unit = {},
    /** 播放宏（`{macro: "CTRL f d"}`）。 */
    val onMacro: (steps: List<MacroStep>) -> Unit = {},
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

/**
 * 长按动作：CTRL/ALT 锁定，或执行 `popup` 次级键。
 *
 * 上游 termux 的 popup 由「上滑」触发并弹出一个浮层；本应用没有浮层，长按直接执行
 * 次级键——语义一致（长按 = 次级键），只是少了中间那层浮层。
 */
private fun secondaryLongPressAction(
    slot: ExtraKeySlot,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
): (() -> Unit)? = when (slot.special) {
    TerminalKeyCatalog.Special.CTRL -> actions.onLockCtrl

    TerminalKeyCatalog.Special.ALT -> actions.onLockAlt

    else ->
        slot.popup?.let { popup ->
            { performAction(popup, actions, modifierStates) }
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
    drawerDescription: String,
): ToolbarKeyPresentation {
    val onRepeat =
        if (slot.repeatable) {
            {
                performAction(slot.action, actions, modifierStates)
            }
        } else {
            null
        }
    return ToolbarKeyPresentation(
        label = slot.label,
        onClick = { performAction(slot.action, actions, modifierStates) },
        modifierState = modifierStateFor(slot, modifierStates),
        testTag = slot.testTag,
        // DRAWER 的键帽是 ☰；无障碍与 UI 测试需要稳定的动作说明，不能读字形。
        contentDescription = if (slot.special == TerminalKeyCatalog.Special.DRAWER) drawerDescription else slot.label,
        onRepeat = onRepeat,
        secondaryAction = secondaryLongPressAction(slot, actions, modifierStates),
    )
}

/**
 * 执行一个键的动作。
 *
 * 主键与长按次级键（`popup`）走同一条路：键名走编码器、特殊键走键栏自己的回调、
 * 字面量直接写出、宏按顺序播放。
 */
private fun performAction(action: KeyAction, actions: ModifierBarActions, modifierStates: ModifierBarStates) {
    when (action) {
        is KeyAction.Macro -> actions.onMacro(action.steps)

        is KeyAction.Special ->
            when (action.special) {
                TerminalKeyCatalog.Special.CTRL -> actions.onToggleCtrl()

                TerminalKeyCatalog.Special.ALT -> actions.onToggleAlt()

                TerminalKeyCatalog.Special.DRAWER -> actions.onDrawerClick()

                TerminalKeyCatalog.Special.SCROLL -> actions.onScrollClick()

                TerminalKeyCatalog.Special.KEYBOARD -> actions.onKeyboardClick()

                TerminalKeyCatalog.Special.PASTE -> actions.onPasteClick()

                // FN 不渲染（见 resolveKey），走不到这里。
                TerminalKeyCatalog.Special.FN -> {}
            }

        is KeyAction.Code -> sendKeyThroughEncoder(action, actions, modifierStates)

        is KeyAction.Text -> actions.onSubmit(action.text)
    }
}

/**
 * 按键发送：键码一律经上游键编码器（`onKey` → `Bridge.processKeyEvent` → 原生
 * `key::Encoder`），修饰键由编码器与终端状态共同决定形态。
 *
 * 键栏不再自带转义序列字面量：方向键在 DECCKM 下须走 SS3、Ctrl 组合在
 * modifyOtherKeys 下须走 `CSI 27;mod;code~`，这些形态只有编码器知道，写死在
 * Kotlin 的串在对应模式下就是错的。
 */
private fun sendKeyThroughEncoder(
    action: KeyAction.Code,
    actions: ModifierBarActions,
    modifierStates: ModifierBarStates,
) {
    val keyCode = action.keyCode
    val modifiers = KeyModifiers.fromStickyStates(modifierStates.ctrlState, modifierStates.altState)
    if (actions.onKey(keyCode, modifiers)) {
        actions.onConsumeModifiers()
        return
    }
    // 按下 CTRL/ALT 再点某个键却什么都没发生——用户只能以为设备卡了。
    // 键栏的每个键都应可编码，故这是编码缺口，须留证据（键码 + 修饰态）。
    LogUtil.warn(
        "ModifierBar",
        "no encoding for ${action.name} (keyCode=$keyCode modifiers=$modifiers); keystroke dropped",
    )
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
    textColor: Color,
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
                                    HapticFeedbackConstants.LONG_PRESS,
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
                                HapticFeedbackConstants.KEYBOARD_TAP,
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
                                    HapticFeedbackConstants.KEYBOARD_TAP,
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
                                SystemClock.uptimeMillis() + longPressMs
                            var repeatFired = false
                            var repeatValid = true
                            while (currentOnRepeat != null) {
                                val remaining = nextRepeatAt - SystemClock.uptimeMillis()
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
                                                HapticFeedbackConstants.KEYBOARD_TAP,
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
                                    HapticFeedbackConstants.KEYBOARD_TAP,
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
