package terminal.emulator.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import terminal.emulator.BuildConfig
import terminal.emulator.R
import terminal.emulator.runtime.LogLevel

/**
 * 日志等级分区。
 *
 * 等级列表按构建类型区分（DESIGN 日志节：「Release 和 Debug 的列表应该不同」）：
 * Release 构建的调试级与详细级日志代码已被 R8 剥离（见 `proguard-rules.pro`），
 * 列出它们只会让用户选中一个**不生效**的档位。
 */
@Composable
internal fun LogLevelSection(
    logLevel: Int,
    onLogLevelSelected: (LogLevel) -> Unit,
    textColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
) {
    val levels = if (BuildConfig.DEBUG) LogLevel.entries else LogLevel.releaseLevels
    val selected = LogLevel.fromOrdinal(logLevel)
    Column {
        SectionHeader(stringResource(R.string.log_level), sectionTitleColor)
        SettingsCard(cardBackground) {
            Text(
                text = stringResource(R.string.log_level_desc),
                style = MaterialTheme.typography.bodySmall,
                color = textColor.copy(alpha = 0.6f),
            )
            Column(modifier = Modifier.testTag("LogLevelList")) {
                levels.forEach { level ->
                    LogLevelRow(
                        level = level,
                        label = stringResource(logLevelLabelRes(level)),
                        isSelected = level == selected,
                        onClick = { onLogLevelSelected(level) },
                        textColor = textColor,
                    )
                }
            }
        }
    }
}

/** 等级的中文标签资源。 */
private fun logLevelLabelRes(level: LogLevel): Int = when (level) {
    LogLevel.OFF -> R.string.log_level_off
    LogLevel.ERROR -> R.string.log_level_error
    LogLevel.WARN -> R.string.log_level_warn
    LogLevel.INFO -> R.string.log_level_info
    LogLevel.DEBUG -> R.string.log_level_debug
    LogLevel.VERBOSE -> R.string.log_level_verbose
}

@Composable
private fun LogLevelRow(level: LogLevel, label: String, isSelected: Boolean, onClick: () -> Unit, textColor: Color) {
    Row(
        modifier =
        Modifier
            .fillMaxWidth()
            .testTag("LogLevel_${level.name}")
            .clickable(onClick = onClick)
            .padding(vertical = 10.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = if (isSelected) MaterialTheme.colorScheme.primary else textColor,
        )
        if (isSelected) {
            Text(
                text = stringResource(R.string.log_level_selected),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.primary,
            )
        }
    }
}
