package terminal.emulator.ui

// 设置界面的共享构件。收敛了此前各设置项重复的行骨架
// （3 个滑块行、2 个开关行、3 个选择器行——每处都有相同的
// isSmallScreen/labelStyle/valueStyle/color 逻辑）。参照 ghostty-android 的声明式 Setting 模式。

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.collections.immutable.ImmutableList
import terminal.emulator.R
import terminal.emulator.bridge.CjkState
import terminal.emulator.bridge.FontInfo
import java.util.Locale

/** 设置界面进入紧凑模式的屏幕宽度阈值。 */
val SMALL_SCREEN_WIDTH_DP = 400.dp

/** 便捷方法：当前屏幕是否偏窄（紧凑布局）。 */
@Composable
@ReadOnlyComposable
fun rememberIsSmallScreen(): Boolean {
    val screenWidthDp =
        with(LocalDensity.current) {
            LocalWindowInfo.current.containerSize.width
                .toDp()
        }
    return screenWidthDp < SMALL_SCREEN_WIDTH_DP
}

/** 设置响应式样式：小屏标记与标题/数值文本样式，三处行骨架共用。 */
@Composable
@ReadOnlyComposable
private fun rememberSettingsResponsiveStyles(): Triple<Boolean, TextStyle, TextStyle> {
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    val valueStyle =
        if (isSmallScreen) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium
    return Triple(isSmallScreen, labelStyle, valueStyle)
}

/** 传递到设置各行的颜色集合，取代 5 个参数的层层传递。 */
data class SettingsColors(
    val textColor: Color,
    val secondaryText: Color,
    val accentColor: Color,
    val cardBackground: Color,
)

/** 滑块行：标题 + 格式化数值 + 带强调色的滑块。[enabled] 为假时滑块置灰、数值文本暗淡 50% 且忽略改动。 */
@Composable
fun SettingsSliderRow(
    title: String,
    value: Float,
    valueRange: ClosedFloatingPointRange<Float>,
    steps: Int,
    colors: SettingsColors,
    onValueChange: (Float) -> Unit,
    modifier: Modifier = Modifier,
    valueFormatter: (Float) -> String = { "%.0f".format(Locale.US, it) },
    testTag: String? = null,
    enabled: Boolean = true,
    onValueChangeFinished: () -> Unit = {},
) {
    val (_, labelStyle, valueStyle) = rememberSettingsResponsiveStyles()
    // testTag 必须真的挂上：本函数声明了该参数却直接把 modifier 传下去，
    // 调用方给的标签会被静默吞掉，按 tag 找不到节点（同文件 SettingsSelectorRow 是对的）。
    Column(modifier = modifier.then(if (testTag != null) Modifier.testTag(testTag) else Modifier)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = title,
                style = labelStyle,
                color = colors.textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = valueFormatter(value),
                style = valueStyle,
                color = colors.secondaryText.copy(alpha = if (enabled) 1f else 0.5f),
            )
        }
        Slider(
            value = value,
            enabled = enabled,
            onValueChange = { newValue: Float -> if (enabled) onValueChange(newValue) },
            onValueChangeFinished = { if (enabled) onValueChangeFinished() },
            valueRange = valueRange,
            steps = steps,
            colors = SliderDefaults.colors(thumbColor = colors.accentColor, activeTrackColor = colors.accentColor),
        )
    }
}

/** 选择器行：标题 + 一排药丸按钮，其中一项被选中。[enabled] 为假时药丸暗淡 50% 且忽略点击。 */
@Composable
fun SettingsSelectorRow(
    title: String,
    selectedKey: String,
    options: ImmutableList<Pair<String, String>>,
    colors: SettingsColors,
    onOptionSelected: (String) -> Unit,
    modifier: Modifier = Modifier,
    testTag: String? = null,
    optionTestTagPrefix: String? = null,
    enabled: Boolean = true,
) {
    val (isSmallScreen, labelStyle, _) = rememberSettingsResponsiveStyles()
    Column(modifier = modifier.then(if (testTag != null) Modifier.testTag(testTag) else Modifier)) {
        Text(
            text = title,
            style = labelStyle,
            color = colors.textColor,
        )
        Spacer(modifier = Modifier.height(4.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            options.forEach { (key, label) ->
                SettingsSelectorPill(
                    key = key,
                    label = label,
                    isSelected = selectedKey == key,
                    enabled = enabled,
                    colors = colors,
                    onOptionSelected = onOptionSelected,
                    optionTestTagPrefix = optionTestTagPrefix,
                    isSmallScreen = isSmallScreen,
                )
            }
        }
    }
}

/** [SettingsSelectorRow] 内的单个药丸。 */
@Composable
private fun RowScope.SettingsSelectorPill(
    key: String,
    label: String,
    isSelected: Boolean,
    enabled: Boolean,
    colors: SettingsColors,
    onOptionSelected: (String) -> Unit,
    optionTestTagPrefix: String?,
    isSmallScreen: Boolean,
) {
    Box(
        modifier =
        Modifier
            .then(
                if (optionTestTagPrefix != null) {
                    Modifier.testTag("${optionTestTagPrefix}_$key")
                } else {
                    Modifier
                },
            ).weight(1f)
            .clip(RoundedCornerShape(8.dp))
            .background(
                (if (isSelected) colors.accentColor else colors.cardBackground)
                    .copy(alpha = if (enabled) 1f else 0.5f),
            ).clickable(enabled = enabled) { onOptionSelected(key) }
            .padding(vertical = 10.dp, horizontal = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color =
            (if (isSelected) Color.White else colors.textColor).copy(alpha = if (enabled) 1f else 0.5f),
            style =
            if (isSmallScreen) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium,
        )
    }
}

/** 设置分区卡片容器：圆角底色 + 内边距，各分区共用同一套留白。 */
@Composable
internal fun SettingsCard(cardBackground: Color, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val isSmallScreen = rememberIsSmallScreen()
    Column(
        modifier =
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(if (isSmallScreen) 8.dp else 12.dp))
            .background(cardBackground)
            .padding(if (isSmallScreen) 12.dp else 16.dp),
    ) {
        content()
    }
}

/** 分区标题。 */
@Composable
internal fun SectionHeader(title: String, textColor: Color) {
    val isSmallScreen = rememberIsSmallScreen()
    Text(
        text = title,
        style =
        if (isSmallScreen) {
            MaterialTheme.typography.titleSmall
        } else {
            MaterialTheme.typography.titleMedium
        },
        fontWeight = FontWeight.Bold,
        color = textColor,
        modifier = Modifier.padding(vertical = 4.dp),
    )
}

/**
 * 选择器行（卡片式）：标题 + 当前值 +「更改」，点开选择对话框。
 *
 * 字体选择与日志等级共用这一套外观：两者都是「一项多值」的设置，外观不一致
 * 会让人以为它们不是同一类控件（用户反馈：日志等级应与字体选择同样式）。
 *
 * @param title 行标题；为 null 时不画标题（分区标题已经说明这一项是什么）
 * @param rowBackground 选择器行的底色：分区卡片内的块状选择器用页面底色，
 *   与卡片底色拉开层次（与字体选择一致）
 * @param testTag 挂在**可点击的行**上：调用方要按 tag 点它
 */
@Composable
internal fun SettingsPickerRow(
    title: String?,
    value: String,
    rowBackground: Color,
    colors: SettingsColors,
    onOpen: () -> Unit,
    testTag: String? = null,
) {
    val (isSmallScreen, labelStyle, _) = rememberSettingsResponsiveStyles()
    Column {
        if (title != null) {
            Text(
                text = title,
                style = labelStyle,
                color = colors.textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(4.dp))
        }
        Row(
            modifier =
            Modifier
                .then(if (testTag != null) Modifier.testTag(testTag) else Modifier)
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                .background(rowBackground)
                .clickable(onClick = onOpen)
                .padding(horizontal = 12.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = value,
                color = colors.textColor,
                modifier = Modifier.weight(1f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = stringResource(R.string.change),
                color = colors.accentColor,
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

/**
 * 选择对话框：标题 + 一列选项，选中项高亮并加粗。
 *
 * 选项是 (键, 标签) 对：键用于回调（字体族名、日志等级名），标签是显示文本。
 */
@Composable
internal fun SettingsPickerDialog(
    title: String,
    options: ImmutableList<Pair<String, String>>,
    selectedKey: String,
    colors: SettingsColors,
    onSelected: (String) -> Unit,
    onDismiss: () -> Unit,
    testTag: String,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, color = colors.textColor) },
        text = {
            LazyColumn(
                modifier = Modifier.testTag(testTag),
            ) {
                items(options) { (key, label) ->
                    val isSelected = key == selectedKey
                    Row(
                        modifier =
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .clickable { onSelected(key) }
                            .background(
                                if (isSelected) {
                                    colors.accentColor.copy(alpha = 0.2f)
                                } else {
                                    Color.Transparent
                                },
                            ).padding(horizontal = 12.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = label,
                            color = if (isSelected) colors.accentColor else colors.textColor,
                            fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Normal,
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel), color = colors.textColor) }
        },
        containerColor = colors.cardBackground,
    )
}

/** 字体缺 CJK 回退时的警示橙，与 [CjkFallbackMissingWarning] 配套。 */
internal val WARNING_ORANGE = Color(0xFFFF9800)

/**
 * 字体缺 CJK 回退的警告行。「无任何 CJK 回退」与「有回退但不覆盖当前字体」共用同一句提示。
 *
 * 两块内容必须挂在同一个 [Column] 下：本函数要作为**单一发射源**被重组跟踪
 * （compose-lints 的 `ComposeMultipleContentEmitters`），顶层散开发射会让
 * 调用处的重组粒度变粗。Column 默认 wrap 内容，故列入父容器后的测量结果不变。
 */
@Composable
internal fun CjkFallbackMissingWarning() {
    Column {
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = stringResource(R.string.cjk_fallback_missing_warning),
            style = MaterialTheme.typography.bodySmall,
            color = WARNING_ORANGE,
        )
    }
}

/**
 * CJK 回退状态的**唯一**呈现：状态行 + 缺失警告。
 *
 * 状态与提示必须出自同一处：同一事实发两次（字体选择卡一处、字体信息区一处）
 * 会让人以为是两个问题；而「按设计跳过」与「确实缺失」混为一谈，则会把正常状态
 * 报成故障。判定规则见 [terminal.emulator.bridge.FontInfo.isCjkFallbackMissing]。
 */
@Composable
internal fun CjkFallbackStatusLine(fontInfo: FontInfo, secondaryText: Color) {
    val cjkText = fontInfo.cjkFallbackText()
    // 缺失判定、文字颜色与警告行必须同源：三者若各判一次，「按状态判缺失」与
    // 「按族名判缺失」的差异就会让界面自相矛盾（黑字写着「未检测到」、却不给警告）。
    val missing = fontInfo.isCjkFallbackMissing
    val statusLine =
        if (cjkText != null) {
            stringResource(R.string.font_info_cjk_fallback, cjkText)
        } else {
            when (fontInfo.cjkState) {
                CjkState.SKIPPED_PRIMARY_COVERS -> stringResource(R.string.font_info_cjk_skipped)
                CjkState.SKIPPED_NON_CJK_LOCALE -> stringResource(R.string.font_info_cjk_skipped_non_cjk_locale)
                CjkState.SKIPPED_LOCALE_UNKNOWN -> stringResource(R.string.font_info_cjk_skipped_locale_unknown)
                CjkState.NOT_REPORTED -> stringResource(R.string.font_info_cjk_not_reported)
                else -> stringResource(R.string.font_info_cjk_none)
            }
        }
    Text(
        text = statusLine,
        style = MaterialTheme.typography.bodySmall,
        color = if (missing) WARNING_ORANGE else secondaryText,
        modifier = Modifier.testTag("CjkFallbackStatusLine"),
    )
    if (missing) {
        CjkFallbackMissingWarning()
    }
}
