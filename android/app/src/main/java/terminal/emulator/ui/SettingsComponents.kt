package terminal.emulator.ui

// Shared building blocks for the settings screen. Collapses the duplicated
// row skeletons that previously existed per setting (3 slider rows, 2 switch
// rows, 3 selector rows — each with the same isSmallScreen/labelStyle/
// valueStyle/color logic). Modeled on ghostty-android's declarative Setting
// pattern (kotlin-architecture-deepening C4).

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/** Screen-width threshold below which settings render in compact mode. */
const val SMALL_SCREEN_WIDTH_DP = 400

/** Convenience: whether the current screen is narrow (compact layout). */
@Composable
fun rememberIsSmallScreen(): Boolean {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    return screenWidthDp < SMALL_SCREEN_WIDTH_DP
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
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    val valueStyle =
        if (isSmallScreen) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium
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

/** Slider row: title + formatted value + Slider with the accent colors. */
@Composable
fun SettingsSliderRow(
    title: String,
    value: Float,
    valueRange: ClosedFloatingPointRange<Float>,
    steps: Int,
    colors: SettingsColors,
    onValueChange: (Float) -> Unit,
    valueFormatter: (Float) -> String = { "%.0f".format(it) },
    modifier: Modifier = Modifier,
    testTag: String? = null,
) {
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    val valueStyle =
        if (isSmallScreen) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium
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
                color = colors.secondaryText,
            )
        }
        Slider(
            value = value,
            onValueChange = onValueChange,
            valueRange = valueRange,
            steps = steps,
            colors = SliderDefaults.colors(thumbColor = colors.accentColor, activeTrackColor = colors.accentColor),
        )
    }
}

/** Switch row: title + optional description + Switch with the accent colors. */
@Composable
fun SettingsSwitchRow(
    title: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
    colors: SettingsColors,
    description: String? = null,
    modifier: Modifier = Modifier,
    testTag: String? = null,
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
                color = colors.textColor,
            )
            if (description != null) {
                Text(
                    text = description,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.textColor.copy(alpha = 0.6f),
                )
            }
        }
        Switch(
            checked = checked,
            onCheckedChange = onToggle,
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

/** Selector row: title + a row of pill buttons; one option selected. */
@Composable
fun SettingsSelectorRow(
    title: String,
    selectedKey: String,
    options: List<Pair<String, String>>,
    colors: SettingsColors,
    onOptionSelected: (String) -> Unit,
    modifier: Modifier = Modifier,
    testTag: String? = null,
    optionTestTagPrefix: String? = null,
) {
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
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
                val isSelected = selectedKey == key
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
                        .background(if (isSelected) colors.accentColor else colors.cardBackground)
                        .clickable { onOptionSelected(key) }
                        .padding(vertical = 10.dp, horizontal = 4.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = label,
                        color = if (isSelected) Color.White else colors.textColor,
                        style =
                        if (isSmallScreen) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium,
                    )
                }
            }
        }
    }
}
