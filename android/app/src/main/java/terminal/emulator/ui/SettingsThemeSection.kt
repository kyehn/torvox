package terminal.emulator.ui

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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.collections.immutable.ImmutableList
import terminal.emulator.R
import terminal.emulator.ui.theme.AppThemeMode
import terminal.emulator.ui.theme.TerminalTheme
import terminal.emulator.ui.theme.TerminalThemeMode
import terminal.emulator.ui.theme.ThemeRepository

/** 软件主题（日间/夜间/跟随系统）设置分区。 */
@Composable
internal fun AppThemeSection(
    appThemeMode: String,
    onAppThemeModeSelected: (String) -> Unit,
    cardBackground: Color,
    textColor: Color,
    accentColor: Color,
    sectionTitleColor: Color,
) {
    Column {
        SectionHeader(stringResource(R.string.software_theme), sectionTitleColor)
        SettingsCard(cardBackground) {
            AppThemeSelector(
                selectedMode = appThemeMode,
                onModeSelected = { onAppThemeModeSelected(it) },
                textColor = textColor,
                cardBackground = cardBackground,
                accentColor = accentColor,
            )
        }
    }
}

/** 终端主题分区：日夜切换开关与主题列表。 */
@Composable
internal fun TerminalThemeSection(
    themeMode: String,
    dayThemeName: String,
    nightThemeName: String,
    themeName: String,
    onThemeModeSelected: (String) -> Unit,
    onDayThemeSelected: (String) -> Unit,
    onNightThemeSelected: (String) -> Unit,
    onFixedThemeSelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
) {
    // 主题单源在 Rust 侧，经仓库取回；自定义主题为禁止实现。取进程内缓存而非
    // `remember`：本组合在 LazyColumn 列表项内，滚出再滚回会重跑整轮 JNI。
    val allThemes = ThemeRepository.allCached

    Column {
        SectionHeader(stringResource(R.string.theme), sectionTitleColor)
        SettingsCard(cardBackground) {
            TerminalThemeModeSelector(
                selectedMode = themeMode,
                onModeSelected = { onThemeModeSelected(it) },
                textColor = textColor,
                cardBackground = cardBackground,
                accentColor = MaterialTheme.colorScheme.primary,
            )
            Spacer(modifier = Modifier.height(8.dp))
            if (TerminalThemeMode.normalize(themeMode) == TerminalThemeMode.DAY_NIGHT) {
                Column(modifier = Modifier.testTag("DayNightThemeSection")) {
                    ThemeSelector(
                        label = stringResource(R.string.day_theme),
                        selectedTheme = dayThemeName,
                        themes = allThemes,
                        onThemeSelected = { onDayThemeSelected(it) },
                        textColor = textColor,
                        cardBackground = cardBackground,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    ThemeSelector(
                        label = stringResource(R.string.night_theme),
                        selectedTheme = nightThemeName,
                        themes = allThemes,
                        onThemeSelected = { onNightThemeSelected(it) },
                        textColor = textColor,
                        cardBackground = cardBackground,
                    )
                }
            } else {
                ThemeSelector(
                    label = "",
                    selectedTheme = themeName,
                    themes = allThemes,
                    onThemeSelected = { onFixedThemeSelected(it) },
                    textColor = textColor,
                    cardBackground = cardBackground,
                )
            }
            Spacer(modifier = Modifier.height(12.dp))
        }
    }
}

@Composable
private fun AppThemeSelector(
    selectedMode: String,
    onModeSelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
) {
    val isSmall = rememberIsSmallScreen()
    Column(modifier = Modifier.testTag("AppThemeSelector")) {
        val buttons =
            listOf(
                AppThemeMode.DAY to stringResource(R.string.day),
                AppThemeMode.NIGHT to stringResource(R.string.night),
                AppThemeMode.FOLLOW_SYSTEM to stringResource(R.string.follow_system),
            )
        val spacing = if (isSmall) 4.dp else 6.dp
        if (isSmall) {
            Column(verticalArrangement = Arrangement.spacedBy(spacing)) {
                buttons.forEach { (mode, label) ->
                    val isSelected = selectedMode == mode
                    Box(
                        modifier =
                        Modifier
                            .testTag("AppTheme_$mode")
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .background(if (isSelected) accentColor else cardBackground)
                            .clickable { onModeSelected(mode) }
                            .padding(vertical = 8.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = label,
                            color = if (isSelected) Color.White else textColor,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
        } else {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(spacing),
            ) {
                buttons.forEach { (mode, label) ->
                    val isSelected = selectedMode == mode
                    Box(
                        modifier =
                        Modifier
                            .testTag("AppTheme_$mode")
                            .weight(1f)
                            .clip(RoundedCornerShape(8.dp))
                            .background(if (isSelected) accentColor else cardBackground)
                            .clickable { onModeSelected(mode) }
                            .padding(vertical = 10.dp, horizontal = 4.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = label,
                            color = if (isSelected) Color.White else textColor,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun TerminalThemeModeSelector(
    selectedMode: String,
    onModeSelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
) {
    val isDayNight = TerminalThemeMode.normalize(selectedMode) == TerminalThemeMode.DAY_NIGHT
    Row(
        modifier = Modifier.fillMaxWidth().testTag("TerminalThemeModeSelector"),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.day_night_switch),
            style = MaterialTheme.typography.bodyLarge,
            color = textColor,
        )
        Switch(
            checked = isDayNight,
            onCheckedChange = { checked ->
                onModeSelected(if (checked) TerminalThemeMode.DAY_NIGHT else TerminalThemeMode.FIXED)
            },
            modifier = Modifier.testTag("TerminalThemeDayNightSwitch"),
            colors =
            SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = accentColor,
                uncheckedThumbColor = textColor.copy(alpha = 0.6f),
                uncheckedTrackColor = cardBackground,
            ),
        )
    }
}

@Composable
private fun ThemeSelector(
    label: String,
    selectedTheme: String,
    themes: ImmutableList<TerminalTheme>,
    onThemeSelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
) {
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    Column(modifier = Modifier.testTag("ThemeSelector")) {
        if (label.isNotEmpty()) {
            Text(label, style = labelStyle, color = textColor)
            Spacer(modifier = Modifier.height(4.dp))
        }
        LazyRow(
            modifier = Modifier.testTag("ThemeList"),
            horizontalArrangement = Arrangement.spacedBy(if (isSmallScreen) 6.dp else 8.dp),
        ) {
            items(themes) { theme ->
                ThemePreview(
                    theme = theme,
                    // `selectedTheme` 始终是主题名；此处只做字符串相等比较，
                    // 绝不要回退成默认主题——否则选中任一未知主题时
                    // 都会高亮默认那张卡片。
                    isSelected = theme.name == selectedTheme,
                    onClick = { onThemeSelected(theme.name) },
                    isSmallScreen = isSmallScreen,
                    textColor = textColor,
                )
            }
        }
    }
}

@Composable
private fun ThemePreview(
    theme: TerminalTheme,
    isSelected: Boolean,
    onClick: () -> Unit,
    isSmallScreen: Boolean = false,
    textColor: Color = MaterialTheme.colorScheme.onSurface,
    secondaryText: Color = MaterialTheme.colorScheme.onSurfaceVariant,
) {
    val previewWidth = if (isSmallScreen) 72.dp else 88.dp
    val dotSize = if (isSmallScreen) 6.dp else 8.dp
    val padding = if (isSmallScreen) 4.dp else 6.dp
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier =
        Modifier
            .testTag("theme_preview_${theme.name}")
            .width(previewWidth)
            .clickable(onClick = onClick),
    ) {
        Box(
            modifier =
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                .background(
                    if (isSelected) theme.background else theme.background.copy(alpha = 0.7f),
                ).padding(padding),
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                theme.ansi.take(8).forEach { color ->
                    Box(modifier = Modifier.size(dotSize).clip(RoundedCornerShape(2.dp)).background(color))
                }
            }
        }
        Spacer(modifier = Modifier.height(4.dp))
        // 主题名位于预览框下方，长名称必须完整可读（DESIGN 终端主题节），不截断。
        Text(
            text = theme.name,
            style = MaterialTheme.typography.labelSmall,
            color = if (isSelected) textColor else textColor.copy(alpha = 0.7f),
        )
    }
}
