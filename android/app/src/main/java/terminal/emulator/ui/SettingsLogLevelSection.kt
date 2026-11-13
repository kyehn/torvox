package terminal.emulator.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.collections.immutable.toImmutableList
import terminal.emulator.BuildConfig
import terminal.emulator.R
import terminal.emulator.runtime.LogLevel

/**
 * 日志等级分区。
 *
 * 等级列表按构建类型区分（DESIGN 日志节：「Release 和 Debug 的列表应该不同」）：
 * Release 构建里 Kotlin 侧的调试级调用点已被 R8 剥离（见 `proguard-rules.pro`），
 * 选中「调试」不会有任何调试日志出现——列出它只会让用户选中一个**不生效**的档位。
 *
 * 外观与字体选择一致（卡片块 + 「更改」+ 选择对话框）：两者都是「一项多值」的
 * 设置，外观不同会让人以为它们不是同一类控件。
 */
@Composable
internal fun LogLevelSection(
    logLevel: Int,
    onLogLevelSelected: (LogLevel) -> Unit,
    textColor: Color,
    cardBackground: Color,
    backgroundColor: Color,
    sectionTitleColor: Color,
) {
    val levels = if (BuildConfig.DEBUG) LogLevel.entries else LogLevel.releaseLevels
    val selected = LogLevel.fromOrdinal(logLevel)
    var showPicker by remember { mutableStateOf(false) }
    val colors =
        SettingsColors(textColor, textColor.copy(alpha = 0.6f), MaterialTheme.colorScheme.primary, cardBackground)

    Column {
        SectionHeader(stringResource(R.string.log_level), sectionTitleColor)
        SettingsCard(cardBackground) {
            Text(
                text = stringResource(R.string.log_level_desc),
                style = MaterialTheme.typography.bodySmall,
                color = textColor.copy(alpha = 0.6f),
            )
            Spacer(modifier = Modifier.height(8.dp))
            SettingsPickerRow(
                title = null,
                value = stringResource(logLevelLabelRes(selected)),
                rowBackground = backgroundColor,
                colors = colors,
                onOpen = { showPicker = true },
                testTag = "LogLevelSelector",
            )
        }
    }

    if (showPicker) {
        SettingsPickerDialog(
            title = stringResource(R.string.log_level),
            options = levels.map { it.name to stringResource(logLevelLabelRes(it)) }.toImmutableList(),
            selectedKey = selected.name,
            colors = colors,
            onSelected = { key ->
                levels.firstOrNull { it.name == key }?.let(onLogLevelSelected)
                showPicker = false
            },
            onDismiss = { showPicker = false },
            testTag = "LogLevelDialog",
        )
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
