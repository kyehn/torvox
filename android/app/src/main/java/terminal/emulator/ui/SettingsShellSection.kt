package terminal.emulator.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import terminal.emulator.R

/** 终端设置分区：Shell 启动入口。 */
@Composable
internal fun TerminalConfigSection(
    selectedShell: String,
    onShellSaved: (String) -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
) {
    Column {
        SectionHeader(stringResource(R.string.terminal), sectionTitleColor)
        SettingsCard(cardBackground) {
            ShellInput(
                shellPath = selectedShell,
                onShellSaved = { onShellSaved(it) },
                textColor = textColor,
                accentColor = accentColor,
            )
            Spacer(modifier = Modifier.height(8.dp))
        }
    }
}

@Composable
private fun ShellInput(shellPath: String, onShellSaved: (String) -> Unit, textColor: Color, accentColor: Color) {
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    Column {
        Text(stringResource(R.string.shell), style = labelStyle, color = textColor)
        Spacer(modifier = Modifier.height(4.dp))
        // `rememberSaveable` 而非 `remember`：本项在 LazyColumn 的 item 内，滚出再滚回
        // 会离开组合并丢弃普通 remember 的状态——用户已敲入但未保存的内容被静默吞掉。
        var text by rememberSaveable(shellPath) { mutableStateOf(shellPath) }
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            modifier = Modifier.fillMaxWidth().testTag("ShellEntryInput"),
            singleLine = true,
            placeholder = {
                Text(stringResource(R.string.shell_placeholder), color = textColor.copy(alpha = 0.5f))
            },
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = textColor),
            colors =
            OutlinedTextFieldDefaults.colors(
                focusedTextColor = textColor,
                unfocusedTextColor = textColor,
                cursorColor = accentColor,
                focusedBorderColor = accentColor,
                unfocusedBorderColor = textColor.copy(alpha = 0.5f),
            ),
        )
        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = { onShellSaved(text) },
            modifier = Modifier.fillMaxWidth().testTag("ShellSaveButton"),
            colors = ButtonDefaults.buttonColors(containerColor = accentColor),
        ) {
            Text(text = stringResource(R.string.save), color = textColor)
        }
    }
}
