package terminal.emulator.ui

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.ReadOnlyComposable
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
import terminal.emulator.BuildConfig
import terminal.emulator.R
import terminal.emulator.detectDeviceArchitecture
import terminal.emulator.installer.BootstrapProgress
import terminal.emulator.installer.TermuxBootstrap

/** Bootstrap 安装分区：URL / 本地文件安装、预设与进度反馈。 */
@Composable
internal fun BootstrapSectionFromSettings(
    bootstrapUrl: String,
    bootstrapRunning: Boolean,
    bootstrapResult: String?,
    bootstrapProgress: BootstrapProgress?,
    onUrlChanged: (String) -> Unit,
    onRunBootstrap: () -> Unit,
    onInstallOffline: (Uri) -> Unit,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
) {
    Column {
        SectionHeader(stringResource(R.string.bootstrap), sectionTitleColor)
        SettingsCard(cardBackground, Modifier.testTag("BootstrapSection")) {
            BootstrapSection(
                bootstrapUrl = bootstrapUrl,
                onUrlChanged = { onUrlChanged(it) },
                onRunBootstrap = { onRunBootstrap() },
                onInstallOffline = { uri -> onInstallOffline(uri) },
                bootstrapRunning = bootstrapRunning,
                bootstrapResult = bootstrapResult,
                bootstrapProgress = bootstrapProgress,
                textColor = textColor,
                accentColor = accentColor,
                secondaryText = secondaryText,
            )
        }
    }
}

@Composable
private fun BootstrapSection(
    bootstrapUrl: String,
    onUrlChanged: (String) -> Unit,
    onRunBootstrap: () -> Unit,
    onInstallOffline: (Uri) -> Unit,
    bootstrapRunning: Boolean,
    bootstrapResult: String?,
    bootstrapProgress: BootstrapProgress?,
    textColor: Color,
    accentColor: Color,
    secondaryText: Color,
) {
    // 编辑中不采纳回流值：每次击键经 300ms 防抖落盘，回流带着**上一次**的值抵达；
    // 无论用 `LaunchedEffect` 无条件覆盖还是 `remember(bootstrapUrl)` 重新播种，
    // 都会把用户在写入窗口内继续敲进去的字符抹掉，且重新播种不经 `onValueChange`，
    // 于是输入框与设置静默分叉，用户接着就把被截断的 URL 拿去安装。
    // 故只吞掉「自己刚写下去的那一次」回流（[Self::pendingEcho]），而不是像先前那样
    // 用布尔闩把整个编辑期都钉住——清除应用数据后存储值变空串、闩仍是真，输入框继续
    // 显示清除前的旧 URL，而安装读到的却是空串，两者静默分叉。
    // `rememberSaveable`：与上方 shell 入口同理，LazyColumn 滚出再滚回不得丢输入。
    var url by rememberSaveable { mutableStateOf(bootstrapUrl) }
    var pendingEcho by rememberSaveable { mutableStateOf<String?>(null) }
    LaunchedEffect(bootstrapUrl) {
        if (pendingEcho != bootstrapUrl) {
            pendingEcho = null
            url = bootstrapUrl
        }
    }

    Column {
        OutlinedTextField(
            value = url,
            onValueChange = {
                url = it
                pendingEcho = it
                onUrlChanged(it)
            },
            label = { Text(stringResource(R.string.bootstrap_url_label)) },
            placeholder = { Text(stringResource(R.string.bootstrap_url_placeholder)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag("BootstrapUrlInput"),
            colors =
            OutlinedTextFieldDefaults.colors(
                focusedBorderColor = accentColor,
                unfocusedBorderColor = textColor.copy(alpha = 0.5f),
                cursorColor = accentColor,
                focusedLabelColor = accentColor,
            ),
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = stringResource(R.string.bootstrap_desc),
            style = MaterialTheme.typography.bodySmall,
            color = secondaryText,
        )
        Spacer(modifier = Modifier.height(8.dp))
        if (BuildConfig.TERMUX_BOOTSTRAP_PRESET) {
            Text(
                text = stringResource(R.string.bootstrap_presets),
                style = MaterialTheme.typography.bodyMedium,
                color = textColor,
            )
            Spacer(modifier = Modifier.height(4.dp))

            val architecture = detectDeviceArchitecture()
            val termuxUrl = TermuxBootstrap.downloadUrl(architecture)
            val presetLabel = stringResource(R.string.bootstrap_preset_termux)
            val presetDescription = stringResource(R.string.bootstrap_preset_termux_desc)
            BootstrapPresetItem(
                label = presetLabel,
                description = presetDescription,
                colors = PresetColors(accentColor, textColor, secondaryText),
                modifier = Modifier.testTag("BootstrapPreset_TermuxDefault"),
                onAction = {
                    url = termuxUrl
                    onUrlChanged(termuxUrl)
                },
            )

            Spacer(modifier = Modifier.height(8.dp))
        }
        BootstrapInstallButton(
            onRunBootstrap,
            bootstrapRunning,
            bootstrapResult,
            bootstrapProgress,
            accentColor,
            textColor,
        )

        // 离线安装：经 SAF 选择 .zip 文件，无需网络。
        Spacer(modifier = Modifier.height(8.dp))
        val offlineLauncher =
            rememberLauncherForActivityResult(
                contract = ActivityResultContracts.OpenDocument(),
            ) { uri ->
                uri?.let { onInstallOffline(it) }
            }
        OutlinedButton(
            onClick = { offlineLauncher.launch(arrayOf("application/zip", "application/octet-stream")) },
            enabled = !bootstrapRunning,
            modifier = Modifier.fillMaxWidth().testTag("OfflineInstallButton"),
            colors = ButtonDefaults.outlinedButtonColors(contentColor = textColor),
        ) {
            Text(stringResource(R.string.bootstrap_install_offline))
        }
    }
}

private const val BYTES_PER_KB = 1024L
private const val BYTES_PER_MB = BYTES_PER_KB * 1024

@Composable
@ReadOnlyComposable
private fun formatBytes(bytes: Long): String = when {
    bytes >= BYTES_PER_MB -> stringResource(R.string.byte_unit_mb, bytes / BYTES_PER_MB)
    bytes >= BYTES_PER_KB -> stringResource(R.string.byte_unit_kb, bytes / BYTES_PER_KB)
    else -> stringResource(R.string.byte_unit_b, bytes)
}

@Composable
@ReadOnlyComposable
private fun bootstrapStepText(progress: BootstrapProgress): String = when (progress) {
    is BootstrapProgress.Downloading -> {
        val percent =
            if (progress.contentLength > 0) {
                " (${(progress.bytesWritten * 100 / progress.contentLength)}%)"
            } else {
                ""
            }
        stringResource(
            R.string.bootstrap_downloading,
            percent,
            formatBytes(progress.bytesWritten),
            if (progress.contentLength > 0) formatBytes(progress.contentLength) else "",
        )
    }

    is BootstrapProgress.Extracting -> {
        val percent =
            if (progress.totalEntries > 0) {
                " (${(progress.entriesExtracted * 100 / progress.totalEntries)}%)"
            } else {
                ""
            }
        stringResource(
            R.string.bootstrap_extracting,
            percent,
            progress.entriesExtracted,
            progress.totalEntries,
        )
    }

    is BootstrapProgress.RunningPostInstall -> {
        stringResource(
            R.string.bootstrap_running_postinstall,
            progress.scriptsCompleted,
            progress.totalScripts,
        )
    }

    BootstrapProgress.CreatingSymlinks -> {
        stringResource(R.string.bootstrap_creating_symlinks)
    }

    BootstrapProgress.Complete -> {
        stringResource(R.string.bootstrap_complete)
    }

    is BootstrapProgress.Error -> {
        progress.message
    }
}

@Composable
private fun BootstrapInstallButton(
    onRunBootstrap: () -> Unit,
    bootstrapRunning: Boolean,
    bootstrapResult: String?,
    bootstrapProgress: BootstrapProgress?,
    accentColor: Color,
    textColor: Color,
) {
    val progress = bootstrapProgress
    Column {
        if (bootstrapRunning) {
            LinearProgressIndicator(
                progress = { progress?.overallProgress() ?: 0f },
                modifier = Modifier.fillMaxWidth().height(4.dp).testTag("BootstrapProgressBar"),
                color = textColor,
                trackColor = textColor.copy(alpha = 0.2f),
            )
            Spacer(modifier = Modifier.height(8.dp))
        }
        Button(
            onClick = onRunBootstrap,
            enabled = !bootstrapRunning,
            modifier = Modifier.fillMaxWidth().testTag("BootstrapInstallButton"),
            colors = ButtonDefaults.buttonColors(containerColor = accentColor),
        ) {
            Text(
                text =
                if (bootstrapRunning) {
                    progress?.let { bootstrapStepText(it) }
                        ?: stringResource(R.string.bootstrap_installing)
                } else {
                    stringResource(R.string.bootstrap_install)
                },
                color = textColor,
            )
        }
        if (!bootstrapResult.isNullOrEmpty()) {
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = bootstrapResult,
                style = MaterialTheme.typography.bodySmall,
                color = textColor.copy(alpha = 0.7f),
                modifier = Modifier.testTag("BootstrapResultText"),
            )
        }
    }
}

private data class PresetColors(val accent: Color, val text: Color, val secondary: Color)

@Composable
private fun BootstrapPresetItem(
    label: String,
    description: String,
    colors: PresetColors,
    modifier: Modifier = Modifier,
    onAction: () -> Unit,
) {
    Surface(
        onClick = onAction,
        shape = RoundedCornerShape(8.dp),
        color = colors.accent.copy(alpha = 0.08f),
        border = BorderStroke(1.dp, colors.accent),
        modifier = modifier.fillMaxWidth().padding(vertical = 2.dp),
    ) {
        Row(
            modifier = Modifier.padding(12.dp).fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(text = label, style = MaterialTheme.typography.bodyMedium, color = colors.accent)
                Text(
                    text = description,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.secondary,
                )
            }
            Text(
                text = stringResource(R.string.install),
                style = MaterialTheme.typography.labelMedium,
                color = colors.accent,
                modifier = Modifier.padding(start = 8.dp),
            )
        }
    }
}
