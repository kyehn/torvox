package terminal.emulator.ui

import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import terminal.emulator.R
import terminal.emulator.TerminalViewModel
import terminal.emulator.installer.BootstrapProgress
import terminal.emulator.ui.theme.TerminalTheme

private const val FONT_SIZE_RANGE_MIN = 8f
private const val FONT_SIZE_RANGE_MAX = 48f
private const val FONT_SIZE_RANGE_STEPS = 23
private const val SCROLLBACK_RANGE_MIN = 1000f
private const val SCROLLBACK_RANGE_MAX = 100_000f
private const val SCROLLBACK_RANGE_STEPS = 98
private val WARNING_ORANGE = Color(0xFFFF9800)

@OptIn(ExperimentalMaterial3Api::class) // Material3 experimental API used intentionally
@Composable
@Suppress("LongMethod")
fun SettingsScreen(
    viewModel: TerminalViewModel,
    onBack: () -> Unit,
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val isSmallScreen = screenWidthDp < SMALL_SCREEN_WIDTH_DP
    val horizontalPadding = if (isSmallScreen) 8.dp else 16.dp
    val backgroundColor = MaterialTheme.colorScheme.surface
    val textColor = MaterialTheme.colorScheme.onSurface
    val secondaryText = MaterialTheme.colorScheme.onSurfaceVariant
    val cardBackground = MaterialTheme.colorScheme.surfaceContainerLow
    val accentColor = MaterialTheme.colorScheme.primary
    val sectionTitleColor = MaterialTheme.colorScheme.primary
    val customFontLauncher =
        rememberLauncherForActivityResult(
            contract = ActivityResultContracts.OpenDocument(),
        ) { uri: Uri? ->
            if (uri != null) viewModel.installFontFile(uri)
        }
    BackHandler(enabled = true) { onBack() }
    Surface(
        modifier =
        Modifier.fillMaxSize().testTag("SettingsScreen").clickable(
            indication = null,
            interactionSource = remember { MutableInteractionSource() },
            onClick = {},
        ),
        color = backgroundColor,
    ) {
        Column(modifier = Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
            // Fixed header: SettingsHeader must stay visible while the
            // LazyColumn scrolls, otherwise the back button scrolls out of
            // reach (round-119: maestro open-settings flow failed to find
            // SettingsBackButton after scrolling down).
            SettingsHeader(onBack, textColor, isSmallScreen)
            LazyColumn(
                modifier = Modifier.fillMaxSize().padding(horizontal = horizontalPadding).testTag("SettingsLazyColumn"),
                verticalArrangement = Arrangement.spacedBy(if (isSmallScreen) 8.dp else 12.dp),
                contentPadding = PaddingValues(bottom = 32.dp),
            ) {
                item {
                    SectionHeader(stringResource(R.string.appearance), sectionTitleColor)
                    SettingsCard(cardBackground, isSmallScreen) {
                        AppearanceSectionContent(viewModel, customFontLauncher, textColor, secondaryText, accentColor, backgroundColor)
                    }
                }
                item { AppThemeSection(viewModel, cardBackground, textColor, accentColor, sectionTitleColor, isSmallScreen) }
                item { TerminalThemeSection(viewModel, textColor, secondaryText, cardBackground, sectionTitleColor, isSmallScreen) }
                item {
                    BackgroundSection(
                        viewModel,
                        textColor,
                        secondaryText,
                        accentColor,
                        cardBackground,
                        sectionTitleColor,
                        isSmallScreen,
                    )
                }
                item {
                    TerminalConfigSection(
                        viewModel,
                        textColor,
                        secondaryText,
                        accentColor,
                        cardBackground,
                        backgroundColor,
                        sectionTitleColor,
                        isSmallScreen,
                    )
                }
                item {
                    BootstrapSectionFromSettings(
                        viewModel,
                        textColor,
                        secondaryText,
                        accentColor,
                        cardBackground,
                        sectionTitleColor,
                        isSmallScreen,
                    )
                }
                item { ClearAppDataSectionItem(viewModel, textColor, cardBackground, sectionTitleColor, isSmallScreen) }
                item { Spacer(modifier = Modifier.height(24.dp)) }
            }
        }
    }
}

@Composable
private fun SettingsHeader(
    onBack: () -> Unit,
    textColor: Color,
    isSmallScreen: Boolean,
) {
    Spacer(modifier = Modifier.height(8.dp))
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack, modifier = Modifier.testTag("SettingsBackButton")) {
            Icon(
                Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = stringResource(R.string.back),
                tint = textColor,
            )
        }
        Spacer(modifier = Modifier.width(4.dp))
        Text(
            text = stringResource(R.string.settings),
            style = if (isSmallScreen) MaterialTheme.typography.titleLarge else MaterialTheme.typography.headlineSmall,
            color = textColor,
            fontWeight = FontWeight.Bold,
        )
    }
}

@Composable
private fun AppearanceSectionContent(
    viewModel: TerminalViewModel,
    customFontLauncher: androidx.activity.result.ActivityResultLauncher<Array<String>>,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    backgroundColor: Color,
) {
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val fontSize = settings.fontSize
    val fontFamily = settings.fontFamily
    val cursorBlinkEnabled = settings.cursorBlink
    val cursorSpeedMs = settings.cursorSpeed
    val cursorStyleValue = settings.cursorStyle
    val availableFonts by viewModel.availableFonts.collectAsStateWithLifecycle()
    val defaultFontName by viewModel.defaultFontName.collectAsStateWithLifecycle()
    val fontInfo by viewModel.fontInfo.collectAsStateWithLifecycle()
    FontSizeSlider(
        modifier = Modifier.testTag("FontSizeSlider"),
        value = fontSize,
        onValueChange = { viewModel.setFontSize(it) },
        textColor = textColor,
        secondaryText = secondaryText,
        accentColor = accentColor,
    )
    Spacer(modifier = Modifier.height(12.dp))
    SystemFontSelector(
        selectedFamily = fontFamily,
        onFamilySelected = { viewModel.setFontFamily(it) },
        textColor = textColor,
        cardBackground = backgroundColor,
        accentColor = accentColor,
        fonts = availableFonts,
        defaultFontName = defaultFontName,
        fontInfo = fontInfo,
        onPickFontFile = { customFontLauncher.launch(arrayOf("font/*", "application/octet-stream")) },
    )
    if (fontInfo.isNotEmpty() || defaultFontName.isNotEmpty()) {
        Spacer(modifier = Modifier.height(8.dp))
        val densityDpi = LocalDensity.current.density
        FontInfoSection(
            fontInfo =
            fontInfo.ifEmpty {
                val pixelSize = (fontSize * densityDpi).toInt()
                "Active: $defaultFontName\n(CJK fallback info available after session starts)\nFont size: ${fontSize.toInt()}SP (~${pixelSize}px)"
            },
            textColor = textColor,
            secondaryText = secondaryText,
        )
    }
    Spacer(modifier = Modifier.height(12.dp))
    CursorBlinkToggle(
        enabled = cursorBlinkEnabled,
        onToggle = { viewModel.setCursorBlink(it) },
        textColor = textColor,
        accentColor = accentColor,
        cardBackground = backgroundColor,
    )
    if (cursorBlinkEnabled) {
        Spacer(modifier = Modifier.height(8.dp))
        CursorSpeedSlider(
            value = cursorSpeedMs.toFloat(),
            onValueChange = { viewModel.setCursorSpeed(it.toInt()) },
            textColor = textColor,
            secondaryText = secondaryText,
            accentColor = accentColor,
        )
    }
    Spacer(modifier = Modifier.height(8.dp))
    CursorStyleSelector(
        selectedStyle = cursorStyleValue,
        onStyleSelected = { viewModel.setCursorStyle(it) },
        textColor = textColor,
        accentColor = accentColor,
        cardBackground = backgroundColor,
    )
}

@Composable
private fun AppThemeSection(
    viewModel: TerminalViewModel,
    cardBackground: Color,
    textColor: Color,
    accentColor: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val appThemeMode = settings.appThemeMode
    SectionHeader(stringResource(R.string.software_theme), sectionTitleColor)
    SettingsCard(cardBackground, isSmallScreen) {
        AppThemeSelector(
            selectedMode = appThemeMode,
            onModeSelected = { viewModel.setAppThemeMode(it) },
            textColor = textColor,
            cardBackground = cardBackground,
            accentColor = accentColor,
        )
    }
}

@Composable
private fun TerminalThemeSection(
    viewModel: TerminalViewModel,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val themeMode = settings.themeMode
    val dayThemeName = settings.dayThemeName
    val nightThemeName = settings.nightThemeName
    val themeName = settings.themeName
    SectionHeader(stringResource(R.string.theme), sectionTitleColor)
    SettingsCard(cardBackground, isSmallScreen) {
        TerminalThemeModeSelector(
            selectedMode = themeMode,
            onModeSelected = { viewModel.setThemeMode(it) },
            textColor = textColor,
            cardBackground = cardBackground,
            accentColor = MaterialTheme.colorScheme.primary,
        )
        Spacer(modifier = Modifier.height(8.dp))
        when (themeMode) {
            "follow_system", "day", "night" -> {
                Column(modifier = Modifier.testTag("DayNightThemeSection")) {
                    ThemeSelector(
                        label = stringResource(R.string.day_theme),
                        selectedTheme = dayThemeName,
                        themes = terminal.emulator.ui.theme.BuiltInThemes.all,
                        onThemeSelected = { viewModel.setDayThemeName(it) },
                        textColor = textColor,
                        secondaryText = secondaryText,
                        cardBackground = cardBackground,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    ThemeSelector(
                        label = stringResource(R.string.night_theme),
                        selectedTheme = nightThemeName,
                        themes = terminal.emulator.ui.theme.BuiltInThemes.all,
                        onThemeSelected = { viewModel.setNightThemeName(it) },
                        textColor = textColor,
                        secondaryText = secondaryText,
                        cardBackground = cardBackground,
                    )
                }
            }

            "fixed" -> {
                ThemeSelector(
                    label = "",
                    selectedTheme = themeName,
                    themes = terminal.emulator.ui.theme.BuiltInThemes.all,
                    onThemeSelected = { viewModel.setThemeName(it) },
                    textColor = textColor,
                    secondaryText = secondaryText,
                    cardBackground = cardBackground,
                )
            }
        }
    }
}

@Composable
private fun BackgroundSection(
    viewModel: TerminalViewModel,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val backgroundImagePath = settings.backgroundImagePath
    val backgroundBlurRadius = settings.backgroundBlurRadius
    val backgroundAlpha = settings.backgroundAlpha
    val imagePickerLauncher =
        rememberLauncherForActivityResult(contract = ActivityResultContracts.GetContent()) { uri: Uri? ->
            uri?.let { viewModel.setBackgroundImagePath(it.toString()) }
        }
    SectionHeader("Background", sectionTitleColor)
    SettingsCard(cardBackground, isSmallScreen) {
        Text(
            text = if (backgroundImagePath.isNotEmpty()) "Background image set" else "No background image",
            color = textColor,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag("BackgroundImageStatus"),
        )
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = { imagePickerLauncher.launch("image/*") },
                colors = ButtonDefaults.buttonColors(containerColor = accentColor),
                modifier = Modifier.testTag("ChooseImageButton"),
            ) { Text("Choose Image", color = MaterialTheme.colorScheme.onPrimary) }
            if (backgroundImagePath.isNotEmpty()) {
                TextButton(onClick = { viewModel.setBackgroundImagePath("") }) { Text("Clear", color = accentColor) }
            }
        }
        if (backgroundImagePath.isNotEmpty()) {
            Spacer(modifier = Modifier.height(12.dp))
            Text("Blur: $backgroundBlurRadius", color = secondaryText, style = MaterialTheme.typography.bodySmall)
            Slider(
                value = backgroundBlurRadius.toFloat(),
                onValueChange = { viewModel.setBackgroundBlurRadius(it.toInt()) },
                valueRange = 0f..20f,
                colors = SliderDefaults.colors(thumbColor = accentColor, activeTrackColor = accentColor),
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text("Opacity: ${(backgroundAlpha * 100).toInt()}%", color = secondaryText, style = MaterialTheme.typography.bodySmall)
            Slider(
                value = backgroundAlpha,
                onValueChange = { viewModel.setBackgroundAlpha(it) },
                valueRange = 0.1f..1.0f,
                colors = SliderDefaults.colors(thumbColor = accentColor, activeTrackColor = accentColor),
            )
        }
    }
}

@Composable
@Suppress("LongParameterList")
private fun TerminalConfigSection(
    viewModel: TerminalViewModel,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    cardBackground: Color,
    backgroundColor: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val selectedShell = settings.shell
    val scrollbackLines = settings.scrollbackLines
    SectionHeader(stringResource(R.string.terminal), sectionTitleColor)
    SettingsCard(cardBackground, isSmallScreen) {
        ShellInput(shellPath = selectedShell, onShellChanged = { viewModel.setShell(it) }, textColor = textColor, accentColor = accentColor)
        Spacer(modifier = Modifier.height(12.dp))
        ScrollbackSlider(
            value = scrollbackLines.toFloat(),
            onValueChange = { viewModel.setScrollbackLines(it.toInt()) },
            textColor = textColor,
            secondaryText = secondaryText,
            accentColor = accentColor,
        )
        Spacer(modifier = Modifier.height(8.dp))
        McpServerToggle(
            enabled = settings.mcpServerEnabled,
            onToggle = { viewModel.setMcpServerEnabled(it) },
            textColor = textColor,
            accentColor = accentColor,
            cardBackground = backgroundColor,
        )
    }
}

@Composable
private fun BootstrapSectionFromSettings(
    viewModel: TerminalViewModel,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val bootstrapUrl = settings.bootstrapUrl
    val bootstrapRunning by viewModel.bootstrapRunning.collectAsStateWithLifecycle()
    val bootstrapResult by viewModel.bootstrapResult.collectAsStateWithLifecycle()
    val bootstrapProgress: BootstrapProgress? by viewModel.bootstrapProgress.collectAsStateWithLifecycle()
    SectionHeader(stringResource(R.string.bootstrap), sectionTitleColor)
    SettingsCard(cardBackground, isSmallScreen, Modifier.testTag("BootstrapSection")) {
        BootstrapSection(
            bootstrapUrl = bootstrapUrl,
            onUrlChanged = { viewModel.setBootstrapUrl(it) },
            onRunBootstrap = { viewModel.runBootstrap() },
            bootstrapRunning = bootstrapRunning,
            bootstrapResult = bootstrapResult,
            bootstrapProgress = bootstrapProgress,
            textColor = textColor,
            accentColor = accentColor,
            secondaryText = secondaryText,
        )
    }
}

@Composable
private fun ClearAppDataSectionItem(
    viewModel: TerminalViewModel,
    textColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    SectionHeader(stringResource(R.string.clear_app_data), sectionTitleColor)
    SettingsCard(cardBackground, isSmallScreen) {
        ClearAppDataSection(viewModel = viewModel, textColor = textColor)
    }
}

@Composable
private fun SettingsCard(
    cardBackground: Color,
    isSmallScreen: Boolean = false,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
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

@Composable
private fun SectionHeader(
    title: String,
    textColor: Color,
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val isSmallScreen = screenWidthDp < SMALL_SCREEN_WIDTH_DP
    Text(
        text = title,
        style = if (isSmallScreen) MaterialTheme.typography.titleSmall else MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.Bold,
        color = textColor,
        modifier = Modifier.padding(vertical = 4.dp),
    )
}

@Composable
private fun FontSizeSlider(
    modifier: Modifier = Modifier,
    value: Float,
    onValueChange: (Float) -> Unit,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
) {
    SettingsSliderRow(
        title = stringResource(R.string.font_size),
        value = value,
        valueRange = FONT_SIZE_RANGE_MIN..FONT_SIZE_RANGE_MAX,
        steps = FONT_SIZE_RANGE_STEPS,
        colors = SettingsColors(textColor, secondaryText, accentColor, cardBackground = Color.Transparent),
        onValueChange = onValueChange,
        modifier = modifier,
    )
}

@Composable
private fun SystemFontSelector(
    selectedFamily: String,
    onFamilySelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
    fonts: List<String> = emptyList(),
    defaultFontName: String = "",
    fontInfo: String = "",
    onPickFontFile: (() -> Unit)? = null,
) {
    val systemFonts = remember(fonts) { fonts.distinct().sorted() }
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val isSmallScreen = screenWidthDp < SMALL_SCREEN_WIDTH_DP
    val labelStyle = if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    val displayName = if (defaultFontName.isEmpty()) "Noto Sans Mono" else defaultFontName

    Spacer(modifier = Modifier.height(4.dp))
    Text(stringResource(R.string.font_family), style = labelStyle, color = textColor, maxLines = 1)
    Spacer(modifier = Modifier.height(4.dp))
    var showFontPicker by remember { mutableStateOf(false) }

    Box {
        Row(
            modifier =
            Modifier
                .testTag("FontFamilySelector")
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                .background(cardBackground)
                .clickable {
                    showFontPicker = true
                }.padding(horizontal = 12.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = if (selectedFamily.isEmpty()) displayName else selectedFamily,
                color = textColor,
                modifier = Modifier.weight(1f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(text = stringResource(R.string.change), color = accentColor, style = MaterialTheme.typography.bodySmall)
        }

        if (showFontPicker) {
            FontPickerDialog(
                fonts = systemFonts,
                selectedFamily = selectedFamily,
                onFamilySelected = { font ->
                    onFamilySelected(font)
                    showFontPicker = false
                },
                onDismiss = { showFontPicker = false },
                textColor = textColor,
                cardBackground = cardBackground,
                accentColor = accentColor,
                onPickFontFile = { onPickFontFile?.invoke() },
            )
        }
    }

    val cjkFallback =
        fontInfo
            .lines()
            .find { it.startsWith("CJK fallback:") }
            ?.substringAfter("CJK fallback:")
            ?.trim()
            ?: ""
    val activeFontCjk =
        fontInfo.lines().any { line ->
            line.startsWith("Active:") && (
                line.contains("CJK", ignoreCase = true) ||
                    line.contains("SC", ignoreCase = true) ||
                    line.contains("TC", ignoreCase = true) ||
                    line.contains("JP", ignoreCase = true) ||
                    line.contains("KR", ignoreCase = true)
                )
        }
    if (cjkFallback.isNotEmpty() && cjkFallback != "none" && cjkFallback != "skipped") {
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = "CJK: $cjkFallback",
            style = MaterialTheme.typography.bodySmall,
            color = textColor.copy(alpha = 0.6f),
        )
    } else if (cjkFallback == "none" && !activeFontCjk) {
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = stringResource(R.string.cjk_fallback_missing_warning),
            style = MaterialTheme.typography.bodySmall,
            color = WARNING_ORANGE,
        )
    }
}

@Composable
private fun FontPickerDialog(
    fonts: List<String>,
    selectedFamily: String,
    onFamilySelected: (String) -> Unit,
    onDismiss: () -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
    onPickFontFile: (() -> Unit)? = null,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.select_font_family), color = textColor) },
        text = {
            LazyColumn {
                item {
                    Row(
                        modifier =
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .clickable {
                                onPickFontFile?.invoke()
                                onDismiss()
                            }.padding(horizontal = 12.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(text = stringResource(R.string.pick_font_file), color = accentColor, fontWeight = FontWeight.Bold)
                    }
                }
                items(fonts) { font ->
                    Row(
                        modifier =
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .clickable { onFamilySelected(font) }
                            .background(
                                if (selectedFamily == font) {
                                    accentColor.copy(alpha = 0.2f)
                                } else {
                                    Color.Transparent
                                },
                            ).padding(horizontal = 12.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = font,
                            color = if (selectedFamily == font) accentColor else textColor,
                            fontWeight = if (selectedFamily == font) FontWeight.Bold else FontWeight.Normal,
                        )
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel), color = textColor) } },
        containerColor = cardBackground,
    )
}

@Composable
internal fun AppThemeSelector(
    selectedMode: String,
    onModeSelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val isSmall = screenWidthDp < SMALL_SCREEN_WIDTH_DP
    Column(modifier = Modifier.testTag("AppThemeSelector")) {
        val buttons =
            listOf(
                "day" to stringResource(R.string.day),
                "night" to stringResource(R.string.night),
                "follow_system" to stringResource(R.string.follow_system),
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
                        Text(text = label, color = if (isSelected) Color.White else textColor, style = MaterialTheme.typography.bodySmall)
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
                        Text(text = label, color = if (isSelected) Color.White else textColor, style = MaterialTheme.typography.bodyMedium)
                    }
                }
            }
        }
    }
}

@Composable
internal fun TerminalThemeModeSelector(
    selectedMode: String,
    onModeSelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
) {
    val isFollowSystem = selectedMode != "fixed"
    Row(
        modifier = Modifier.fillMaxWidth().testTag("TerminalThemeModeSelector"),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.follow_system),
            style = MaterialTheme.typography.bodyLarge,
            color = textColor,
        )
        Switch(
            checked = isFollowSystem,
            onCheckedChange = { checked ->
                onModeSelected(if (checked) "follow_system" else "fixed")
            },
            modifier = Modifier.testTag("TerminalThemeFollowSystemSwitch"),
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
private fun ShellInput(
    shellPath: String,
    onShellChanged: (String) -> Unit,
    textColor: Color,
    accentColor: Color,
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val isSmallScreen = screenWidthDp < SMALL_SCREEN_WIDTH_DP
    val labelStyle = if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    Column {
        Text(stringResource(R.string.shell), style = labelStyle, color = textColor)
        Spacer(modifier = Modifier.height(4.dp))
        var text by remember(shellPath) { mutableStateOf(shellPath) }
        OutlinedTextField(
            value = text,
            onValueChange = {
                text = it
                onShellChanged(it)
            },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            placeholder = { Text(stringResource(R.string.shell_placeholder), color = textColor.copy(alpha = 0.5f)) },
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
    }
}

@Composable
private fun ScrollbackSlider(
    value: Float,
    onValueChange: (Float) -> Unit,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
) {
    SettingsSliderRow(
        title = stringResource(R.string.scrollback_lines),
        value = value,
        valueRange = SCROLLBACK_RANGE_MIN..SCROLLBACK_RANGE_MAX,
        steps = SCROLLBACK_RANGE_STEPS,
        colors = SettingsColors(textColor, secondaryText, accentColor, cardBackground = Color.Transparent),
        onValueChange = onValueChange,
        valueFormatter = { value ->
            value.toInt().let {
                if (it >= 1000) "${it / 1000}K" else "$it"
            }
        },
    )
}

@Composable
internal fun ThemeSelector(
    label: String,
    selectedTheme: String,
    themes: List<TerminalTheme>,
    onThemeSelected: (String) -> Unit,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
) {
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val isSmallScreen = screenWidthDp < SMALL_SCREEN_WIDTH_DP
    val labelStyle = if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    Column(modifier = Modifier.testTag("ThemeSelector")) {
        if (label.isNotEmpty()) {
            Text(label, style = labelStyle, color = textColor)
            Spacer(modifier = Modifier.height(4.dp))
        }
        LazyRow(horizontalArrangement = Arrangement.spacedBy(if (isSmallScreen) 6.dp else 8.dp)) {
            items(themes) { theme ->
                ThemePreview(
                    theme = theme,
                    isSelected =
                    theme.name == selectedTheme ||
                        terminal.emulator.ui.theme.BuiltInThemes
                            .byName(selectedTheme)
                            .name == theme.name,
                    onClick = { onThemeSelected(theme.name) },
                    isSmallScreen = isSmallScreen,
                    textColor = textColor,
                    secondaryText = secondaryText,
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
                .background(if (isSelected) theme.background else theme.background.copy(alpha = 0.7f))
                .padding(padding),
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                theme.ansi.take(8).forEach { color ->
                    Box(modifier = Modifier.size(dotSize).clip(RoundedCornerShape(2.dp)).background(color))
                }
            }
        }
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = theme.name,
            style = MaterialTheme.typography.labelSmall,
            color = if (isSelected) textColor else textColor.copy(alpha = 0.7f),
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Suppress("LongParameterList")
@Composable
private fun BootstrapSection(
    bootstrapUrl: String,
    onUrlChanged: (String) -> Unit,
    onRunBootstrap: () -> Unit,
    bootstrapRunning: Boolean,
    bootstrapResult: String?,
    bootstrapProgress: BootstrapProgress?,
    textColor: Color,
    accentColor: Color,
    secondaryText: Color,
) {
    var url by remember { mutableStateOf(bootstrapUrl) }
    LaunchedEffect(bootstrapUrl) { url = bootstrapUrl }

    OutlinedTextField(
        value = url,
        onValueChange = {
            url = it
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
    Text(
        text = stringResource(R.string.bootstrap_presets),
        style = MaterialTheme.typography.bodyMedium,
        color = textColor,
    )
    Spacer(modifier = Modifier.height(4.dp))

    val arch = terminal.emulator.detectArchFromAbi()
    val termuxUrl = "https://github.com/termux/termux-packages/releases/download/bootstrap-2026.06.21-r1%2Bapt.android-7/bootstrap-$arch.zip"

    val presets =
        listOf(
            Triple(
                stringResource(R.string.bootstrap_preset_termux),
                termuxUrl,
                stringResource(R.string.bootstrap_preset_termux_desc),
            ),
        )
    presets.forEachIndexed { index, preset ->
        BootstrapPresetItem(
            preset = preset,
            colors = PresetColors(accentColor, textColor, secondaryText),
            modifier =
            Modifier.testTag(
                "BootstrapPreset_TermuxDefault",
            ),
            onAction = {
                url = preset.second
                onUrlChanged(preset.second)
            },
        )
    }

    Spacer(modifier = Modifier.height(8.dp))
    BootstrapInstallButton(onRunBootstrap, bootstrapRunning, bootstrapResult, bootstrapProgress, accentColor, textColor)
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
                    progress?.stepDescription()
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

private data class PresetColors(
    val accent: Color,
    val text: Color,
    val secondary: Color,
)

@Composable
private fun BootstrapPresetItem(
    preset: Triple<String, String, String>,
    colors: PresetColors,
    modifier: Modifier = Modifier,
    onAction: () -> Unit,
) {
    val (label, _, description) = preset
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
                Text(text = description, style = MaterialTheme.typography.bodySmall, color = colors.secondary)
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

@Composable
private fun McpServerToggle(
    enabled: Boolean,
    onToggle: (Boolean) -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
) {
    SettingsSwitchRow(
        title = stringResource(R.string.mcp_server),
        description = stringResource(R.string.mcp_server_desc),
        checked = enabled,
        onToggle = onToggle,
        colors = SettingsColors(textColor, textColor, accentColor, cardBackground),
    )
}

@Composable
private fun ClearAppDataSection(
    viewModel: TerminalViewModel,
    textColor: Color,
) {
    val context = LocalContext.current
    // Resolve once in composable scope: LocalContext-based resource reads
    // are not configuration-aware (lint LocalContextGetResourceValueCall),
    // and stringResource() cannot be called inside the onClick lambda.
    val clearAppDataDone = stringResource(R.string.clear_app_data_done)
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
                        viewModel.clearAppData {
                            // In-process StateFlows still hold the old
                            // values; a restart is required for a full reset.
                            android.widget.Toast
                                .makeText(
                                    context,
                                    clearAppDataDone,
                                    android.widget.Toast.LENGTH_LONG,
                                )
                                .show()
                        }
                    },
                    colors = ButtonDefaults.textButtonColors(contentColor = Color.Red),
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
                color = Color.Red,
            )
        }
    }
}

@Composable
private fun FontInfoSection(
    fontInfo: String,
    textColor: Color,
    secondaryText: Color,
) {
    if (fontInfo.isEmpty()) return

    val lines = fontInfo.split("\n")
    Column(modifier = Modifier.testTag("FontInfoSection")) {
        for (line in lines) {
            when {
                line.startsWith("Active:") -> {
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodyMedium,
                        color = textColor,
                    )
                }

                line.startsWith("CJK fallback:") -> {
                    val cjkValue = line.substringAfter("CJK fallback:").trim()
                    val primaryIsCjk =
                        lines.any { l ->
                            l.startsWith("Active:") && (
                                l.contains("CJK", ignoreCase = true) ||
                                    l.contains("SC", ignoreCase = true) ||
                                    l.contains("TC", ignoreCase = true) ||
                                    l.contains("JP", ignoreCase = true) ||
                                    l.contains("KR", ignoreCase = true)
                                )
                        }
                    val hasCjk = cjkValue.isNotEmpty() && cjkValue != "none" && cjkValue != "skipped"
                    val displayColor =
                        when {
                            hasCjk -> secondaryText
                            primaryIsCjk -> secondaryText
                            else -> WARNING_ORANGE
                        }
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodySmall,
                        color = displayColor,
                    )
                    if (!hasCjk && !primaryIsCjk) {
                        Spacer(modifier = Modifier.height(4.dp))
                        Text(
                            text = stringResource(R.string.cjk_fallback_missing_warning),
                            style = MaterialTheme.typography.bodySmall,
                            color = WARNING_ORANGE,
                        )
                    }
                }

                line.startsWith("Cell:") -> {
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }

                line.startsWith("Font size:") -> {
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }

                else -> {
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }
            }
        }
    }
}

private const val CURSOR_SPEED_RANGE_MIN = 100f
private const val CURSOR_SPEED_RANGE_MAX = 1000f
private const val CURSOR_SPEED_RANGE_STEPS = 17

@Composable
private fun CursorBlinkToggle(
    enabled: Boolean,
    onToggle: (Boolean) -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = stringResource(R.string.cursor_blink),
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
            )
            Text(
                text = stringResource(R.string.cursor_blink_desc),
                style = MaterialTheme.typography.bodySmall,
                color = textColor.copy(alpha = 0.6f),
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = if (enabled) stringResource(R.string.on) else stringResource(R.string.off),
                style = MaterialTheme.typography.labelSmall,
                color = if (enabled) accentColor else textColor.copy(alpha = 0.5f),
            )
            Spacer(modifier = Modifier.width(8.dp))
            Switch(
                checked = enabled,
                onCheckedChange = onToggle,
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
}

@Composable
private fun CursorSpeedSlider(
    value: Float,
    onValueChange: (Float) -> Unit,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
) {
    SettingsSliderRow(
        title = stringResource(R.string.cursor_speed),
        value = value,
        valueRange = CURSOR_SPEED_RANGE_MIN..CURSOR_SPEED_RANGE_MAX,
        steps = CURSOR_SPEED_RANGE_STEPS,
        colors = SettingsColors(textColor, secondaryText, accentColor, cardBackground = Color.Transparent),
        onValueChange = onValueChange,
        valueFormatter = { "${it.toInt()}ms" },
    )
}

@Composable
private fun CursorStyleSelector(
    selectedStyle: String,
    onStyleSelected: (String) -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
) {
    val styles =
        listOf(
            "block" to "Block",
            "bar" to "Bar",
            "underline" to "Underline",
        )
    SettingsSelectorRow(
        title = stringResource(R.string.cursor_style_label),
        selectedKey = selectedStyle,
        options = styles,
        colors = SettingsColors(textColor, textColor, accentColor, cardBackground),
        onOptionSelected = onStyleSelected,
        testTag = "CursorStyleSelector",
        optionTestTagPrefix = "CursorStyle",
    )
}
