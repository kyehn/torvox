package terminal.emulator.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import terminal.emulator.R
import terminal.emulator.settings.ShellEntry

/** 终端设置分区：具名 Shell 启动入口。 */
@Composable
internal fun TerminalConfigSection(
    activeShellName: String,
    shellEntries: List<ShellEntry>,
    onShellEntrySaved: (String, String) -> Unit,
    onShellEntryRemoved: (String) -> Unit,
    onShellEntryActivated: (String) -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
) {
    Column {
        SectionHeader(stringResource(R.string.terminal), sectionTitleColor)
        SettingsCard(cardBackground) {
            if (shellEntries.isEmpty()) {
                Text(stringResource(R.string.shell_entries_empty), color = textColor)
                Spacer(modifier = Modifier.height(8.dp))
            }
            shellEntries.forEach { entry ->
                val active = entry.name == activeShellName
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(entry.name, style = MaterialTheme.typography.bodyLarge, color = textColor)
                        Text(entry.command, style = MaterialTheme.typography.bodySmall, color = textColor)
                    }
                    Button(
                        onClick = { onShellEntryActivated(entry.name) },
                        enabled = !active,
                        colors = ButtonDefaults.buttonColors(containerColor = accentColor),
                        modifier = Modifier.testTag("ShellActivate-${entry.name}"),
                    ) {
                        val actionLabel =
                            stringResource(if (active) R.string.shell_entry_active else R.string.shell_entry_activate)
                        Text(actionLabel)
                    }
                    IconButton(onClick = { onShellEntryRemoved(entry.name) }) {
                        Icon(
                            Icons.Default.Delete,
                            contentDescription = stringResource(R.string.shell_entry_delete),
                            tint = textColor,
                        )
                    }
                }
            }
            ShellEntryInput(onSaved = onShellEntrySaved, textColor = textColor, accentColor = accentColor)
        }
    }
}

@Composable
private fun ShellEntryInput(onSaved: (String, String) -> Unit, textColor: Color, accentColor: Color) {
    var name by rememberSaveable { mutableStateOf("") }
    var command by rememberSaveable { mutableStateOf("") }
    val fieldColors =
        OutlinedTextFieldDefaults.colors(
            focusedTextColor = textColor,
            unfocusedTextColor = textColor,
            cursorColor = accentColor,
            focusedBorderColor = accentColor,
            unfocusedBorderColor = textColor.copy(alpha = 0.5f),
        )
    Column {
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text(stringResource(R.string.shell_entry_name)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag("ShellEntryName"),
            colors = fieldColors,
        )
        OutlinedTextField(
            value = command,
            onValueChange = { command = it },
            label = { Text(stringResource(R.string.shell_entry_command)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag("ShellEntryInput"),
            colors = fieldColors,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = {
                // 只校验名称：名称是入口的标识（唯一且非空），命令行按规范原样保存，
                // 空命令行表示「走默认入口探测链」，不得因命令行为空而拒绝保存。
                if (name.isNotBlank()) {
                    onSaved(name, command)
                    name = ""
                    command = ""
                }
            },
            modifier = Modifier.fillMaxWidth().testTag("ShellEntrySaveButton"),
            colors = ButtonDefaults.buttonColors(containerColor = accentColor),
        ) {
            Text(stringResource(R.string.shell_entry_add))
        }
    }
}
