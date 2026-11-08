package terminal.emulator.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import terminal.emulator.R

/** 危险操作（清除应用数据）的警示红色。 */
private val DANGER_RED = Color.Red

/** 清除应用数据分区。 */
@Composable
internal fun ClearAppDataSectionItem(
    onClearAppData: () -> Unit,
    textColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
) {
    Column {
        SectionHeader(stringResource(R.string.clear_app_data), sectionTitleColor)
        SettingsCard(cardBackground) {
            ClearAppDataSection(onClearAppData = onClearAppData, textColor = textColor)
        }
    }
}

@Composable
private fun ClearAppDataSection(onClearAppData: () -> Unit, textColor: Color) {
    var showConfirmDialog by remember { mutableStateOf(false) }

    if (showConfirmDialog) {
        AlertDialog(
            onDismissRequest = { showConfirmDialog = false },
            title = { Text(stringResource(R.string.clear_app_data)) },
            text = { Text(stringResource(R.string.clear_app_data_confirm)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        showConfirmDialog = false
                        onClearAppData()
                    },
                    colors = ButtonDefaults.textButtonColors(contentColor = DANGER_RED),
                ) {
                    Text(stringResource(R.string.clear_app_data_action))
                }
            },
            dismissButton = {
                TextButton(onClick = { showConfirmDialog = false }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = stringResource(R.string.clear_app_data),
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
            )
            Text(
                text = stringResource(R.string.clear_app_data_desc),
                style = MaterialTheme.typography.bodySmall,
                color = textColor.copy(alpha = 0.6f),
            )
        }
        TextButton(onClick = { showConfirmDialog = true }) {
            Text(
                text = stringResource(R.string.clear_app_data_action),
                color = DANGER_RED,
            )
        }
    }
}
