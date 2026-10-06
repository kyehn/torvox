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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.collections.immutable.ImmutableList
import terminal.emulator.R

/** Screen-width threshold below which settings render in compact mode. */
val SMALL_SCREEN_WIDTH_DP = 400.dp

/** Convenience: whether the current screen is narrow (compact layout). */
@Composable
@ReadOnlyComposable
fun rememberIsSmallScreen(): Boolean {
    val screenWidthDp =
        with(LocalDensity.current) { LocalWindowInfo.current.containerSize.width.toDp() }
    return screenWidthDp < SMALL_SCREEN_WIDTH_DP
}

/** 设置响应式样式：小屏标记与标题/数值文本样式，三处行骨架共用。 */
@Composable
private fun rememberSettingsResponsiveStyles(): Triple<Boolean, TextStyle, TextStyle> {
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    val valueStyle =
        if (isSmallScreen) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium
    return Triple(isSmallScreen, labelStyle, valueStyle)
}

/** Color bundle threaded into settings rows; replaces 5-parameter threading. */
data class SettingsColors(
    val textColor: Color,
    val secondaryText: Color,
    val accentColor: Color,
    val cardBackground: Color,
)

/** Row skeleton shared by every setting: label + value + control. */
@Composable
fun SettingsRow(
    title: String,
    valueText: String?,
    colors: SettingsColors,
    modifier: Modifier = Modifier,
    testTag: String? = null,
    control: @Composable () -> Unit,
) {
    val (_, labelStyle, valueStyle) = rememberSettingsResponsiveStyles()
    Row(
        modifier = modifier.then(if (testTag != null) Modifier.testTag(testTag) else Modifier),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = labelStyle,
                color = colors.textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (valueText != null) {
                Text(
                    text = valueText,
                    style = valueStyle,
                    color = colors.secondaryText,
                )
            }
        }
        Spacer(modifier = Modifier.width(8.dp))
        control()
    }
}

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
    valueFormatter: (Float) -> String = { "%.0f".format(java.util.Locale.US, it) },
    testTag: String? = null,
    enabled: Boolean = true,
    onValueChangeFinished: () -> Unit = {},
) {
    val (_, labelStyle, valueStyle) = rememberSettingsResponsiveStyles()
    Column(modifier = modifier) {
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

/** 开关行：标题 + 可选描述 + 带强调色的开关。[enabled] 为假时开关置灰、文本暗淡 50% 且忽略切换。 */
@Composable
fun SettingsSwitchRow(
    title: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
    colors: SettingsColors,
    modifier: Modifier = Modifier,
    description: String? = null,
    testTag: String? = null,
    enabled: Boolean = true,
) {
    Row(
        modifier = modifier.then(if (testTag != null) Modifier.testTag(testTag) else Modifier),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = colors.textColor.copy(alpha = if (enabled) 1f else 0.5f),
            )
            if (description != null) {
                Text(
                    text = description,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.textColor.copy(alpha = if (enabled) 0.6f else 0.5f),
                )
            }
        }
        Switch(
            checked = checked,
            enabled = enabled,
            onCheckedChange = { newChecked: Boolean -> if (enabled) onToggle(newChecked) },
            colors =
            SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = colors.accentColor,
                uncheckedThumbColor = colors.textColor.copy(alpha = 0.6f),
                uncheckedTrackColor = colors.cardBackground,
            ),
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
            )
            .weight(1f)
            .clip(RoundedCornerShape(8.dp))
            .background(
                (if (isSelected) colors.accentColor else colors.cardBackground)
                    .copy(alpha = if (enabled) 1f else 0.5f),
            )
            .clickable(enabled = enabled) { onOptionSelected(key) }
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
