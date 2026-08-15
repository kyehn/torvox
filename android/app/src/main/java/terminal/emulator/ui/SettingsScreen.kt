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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import terminal.emulator.FONT_SLOT_BOLD
import terminal.emulator.FONT_SLOT_ITALIC
import terminal.emulator.FONT_SLOT_REGULAR
import terminal.emulator.R
import terminal.emulator.TerminalViewModel
import terminal.emulator.bell.BellMode
import terminal.emulator.bridge.FontActiveDto
import terminal.emulator.bridge.FontInfoDto
import terminal.emulator.installer.BootstrapProgress
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.isElf
import terminal.emulator.settings.parseEnvironmentVariables
import terminal.emulator.settings.serializeEnvironmentVariables
import terminal.emulator.ui.theme.TerminalTheme
import java.io.File
import kotlinx.collections.immutable.ImmutableList
import kotlinx.collections.immutable.ImmutableMap
import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toImmutableList
import kotlinx.collections.immutable.toImmutableMap
import android.annotation.SuppressLint

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
    val isSmallScreen = rememberIsSmallScreen()
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
            // reach: maestro open-settings flow failed to find
            // SettingsBackButton after scrolling down).
            SettingsHeader(onBack, textColor, isSmallScreen)
            LazyColumn(
                modifier = Modifier.fillMaxSize().padding(horizontal = horizontalPadding).testTag("SettingsLazyColumn"),
                verticalArrangement = Arrangement.spacedBy(if (isSmallScreen) 8.dp else 12.dp),
                contentPadding = PaddingValues(bottom = 32.dp),
            ) {
                item {
                    SectionHeader(stringResource(R.string.appearance), sectionTitleColor)
                    SettingsCard(cardBackground) {
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
                item { KeyboardShortcutsSection(viewModel, textColor, secondaryText, cardBackground, sectionTitleColor, isSmallScreen) }
                item {
                    ModifierBarSettingsSection(
                        viewModel,
                        textColor,
                        secondaryText,
                        cardBackground,
                        sectionTitleColor,
                        isSmallScreen,
                    )
                }
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
    val boldFontFamily = settings.boldFontFamily
    val italicFontFamily = settings.italicFontFamily
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
    FontFamilySelectors(
        regularFamily = fontFamily,
        boldFamily = boldFontFamily,
        italicFamily = italicFontFamily,
        onFamilySelected = { family, slot -> viewModel.setFontFamilyForStyle(family, slot) },
        customFontLauncher = customFontLauncher,
        colors = SettingsColors(textColor, secondaryText, accentColor, backgroundColor),
        availableFonts = availableFonts.toImmutableList(),
        defaultFontName = defaultFontName,
        fontInfo = fontInfo,
    )
    FontInfoSectionIfAvailable(
        fontInfo = fontInfo,
        defaultFontName = defaultFontName,
        fontSize = fontSize,
        textColor = textColor,
        secondaryText = secondaryText,
    )
    Spacer(modifier = Modifier.height(12.dp))
    SettingsSwitchRow(
        title = stringResource(R.string.cursor_blink),
        description = stringResource(R.string.cursor_blink_desc),
        checked = cursorBlinkEnabled,
        onToggle = { viewModel.setCursorBlink(it) },
        colors = SettingsColors(textColor, textColor, accentColor, backgroundColor),
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
    Spacer(modifier = Modifier.height(8.dp))
    BellModeSelector(
        selectedModeId = settings.bellMode,
        onModeSelected = { viewModel.setBellMode(it) },
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
    SettingsCard(cardBackground) {
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
    // User-created themes (ghostty-android ThemeStore pattern) merged into
    // the pick list; `remember` keyed on the flow so a save re-renders.
    val userThemes by viewModel.userThemes.collectAsStateWithLifecycle()
    val allThemes = remember(userThemes) { (terminal.emulator.ui.theme.BuiltInThemes.all + userThemes).toImmutableList() }
    var saveThemeName by rememberSaveable { mutableStateOf("") }
    val context = LocalContext.current
    val systemInDarkTheme = androidx.compose.foundation.isSystemInDarkTheme()

    // Theme editor dialog state — opened when user taps "Edit" on a user theme.
    var editingTheme by rememberSaveable { mutableStateOf<terminal.emulator.ui.theme.TerminalTheme?>(null) }

    SectionHeader(stringResource(R.string.theme), sectionTitleColor)
    SettingsCard(cardBackground) {
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
                        themes = allThemes,
                        onThemeSelected = { viewModel.setDayThemeName(it) },
                        textColor = textColor,
                        secondaryText = secondaryText,
                        cardBackground = cardBackground,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    ThemeSelector(
                        label = stringResource(R.string.night_theme),
                        selectedTheme = nightThemeName,
                        themes = allThemes,
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
                    themes = allThemes,
                    onThemeSelected = { viewModel.setThemeName(it) },
                    textColor = textColor,
                    secondaryText = secondaryText,
                    cardBackground = cardBackground,
                )
            }
        }
        Spacer(modifier = Modifier.height(12.dp))
        // User themes (ghostty-android ThemeStore pattern): save the current
        // resolved theme under a name, or delete a saved user theme.
        Row(
            verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedTextField(
                value = saveThemeName,
                onValueChange = { saveThemeName = it },
                placeholder = { Text(stringResource(R.string.new_theme_name), color = secondaryText) },
                singleLine = true,
                modifier = Modifier.weight(1f).testTag("SaveThemeName"),
            )
            Button(
                onClick = {
                    val name = saveThemeName.trim()
                    if (name.isNotEmpty()) {
                        viewModel.saveCurrentThemeAs(name, systemInDarkTheme)
                        saveThemeName = ""
                    }
                },
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
                modifier = Modifier.testTag("SaveThemeButton"),
            ) { Text(stringResource(R.string.save), color = MaterialTheme.colorScheme.onPrimary) }
        }
        if (userThemes.isNotEmpty()) {
            Spacer(modifier = Modifier.height(8.dp))
            Text(stringResource(R.string.user_themes), color = secondaryText, style = MaterialTheme.typography.bodySmall)
            Spacer(modifier = Modifier.height(4.dp))
            userThemes.forEach { theme ->
                Row(
                    verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        theme.name,
                        color = textColor,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.weight(1f).testTag("UserTheme_${theme.name}"),
                    )
                    TextButton(
                        onClick = { editingTheme = theme },
                        modifier = Modifier.testTag("EditTheme_${theme.name}"),
                    ) { Text(stringResource(R.string.edit_theme), color = MaterialTheme.colorScheme.primary) }
                    TextButton(
                        onClick = { viewModel.deleteUserTheme(theme.name) },
                        modifier = Modifier.testTag("DeleteTheme_${theme.name}"),
                    ) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }
                }
            }
        }
    }

    // Theme editor dialog — edits a working copy of the theme.
    editingTheme?.let { theme ->
        terminal.emulator.ui.theme.ThemeEditorDialog(
            theme = theme,
            isOverwriteExisting = true,
            onSaveAsNew = { edited ->
                viewModel.saveEditedThemeAsNew(edited.name, edited)
                editingTheme = null
            },
            onOverwrite = { edited ->
                viewModel.overwriteUserTheme(edited)
                editingTheme = null
            },
            onDismiss = { editingTheme = null },
        )
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
    val context = LocalContext.current
    // ACTION_OPEN_DOCUMENT (not GetContent) so the picked URI can be
    // persisted: without FLAG_GRANT_PERSISTABLE_URI_PERMISSION +
    // takePersistableUriPermission the read permission is lost on app
    // restart and the wallpaper silently disappears (emulator-verified,
    // `consumed bg image` never logged after a relaunch).
    val imagePickerLauncher =
        rememberLauncherForActivityResult(contract = ActivityResultContracts.StartActivityForResult()) { result ->
            val uri = result.data?.data
            uri?.let {
                try {
                    context.contentResolver.takePersistableUriPermission(
                        it,
                        android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION,
                    )
                } catch (e: SecurityException) {
                    LogUtil.e("Settings", "takePersistableUriPermission failed", e)
                }
                viewModel.setBackgroundImagePath(it.toString())
            }
        }
    SectionHeader(stringResource(R.string.background), sectionTitleColor)
    SettingsCard(cardBackground) {
        Text(
            text = if (backgroundImagePath.isNotEmpty()) stringResource(R.string.bg_image_set) else stringResource(R.string.bg_image_none),
            color = textColor,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag("BackgroundImageStatus"),
        )
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val intent = android.content.Intent(android.content.Intent.ACTION_OPEN_DOCUMENT).apply {
                        addCategory(android.content.Intent.CATEGORY_OPENABLE)
                        type = "image/*"
                        addFlags(
                            android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or
                                android.content.Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
                        )
                    }
                    imagePickerLauncher.launch(intent)
                },
                colors = ButtonDefaults.buttonColors(containerColor = accentColor),
                modifier = Modifier.testTag("ChooseImageButton"),
            ) { Text(stringResource(R.string.bg_image_choose), color = MaterialTheme.colorScheme.onPrimary) }
            if (backgroundImagePath.isNotEmpty()) {
                TextButton(onClick = { viewModel.setBackgroundImagePath("") }) { Text(stringResource(R.string.clear), color = accentColor) }
            }
        }
        if (backgroundImagePath.isNotEmpty()) {
            Spacer(modifier = Modifier.height(12.dp))
            Text(stringResource(R.string.bg_blur_label, backgroundBlurRadius), color = secondaryText, style = MaterialTheme.typography.bodySmall)
            Slider(
                value = backgroundBlurRadius.toFloat(),
                onValueChange = { viewModel.setBackgroundBlurRadius(it.toInt()) },
                // kernel taps scale linearly with radius
                // (2*ceil(r)+1 per pass); 20 was 82 taps/frame on a
                // Mali-G57 — capped at 10 (42 taps) for interactive
                // framerates. Downsampled blur is a future optimization.
                valueRange = 0f..10f,
                colors = SliderDefaults.colors(thumbColor = accentColor, activeTrackColor = accentColor),
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(stringResource(R.string.bg_opacity_label, (backgroundAlpha * 100).toInt()), color = secondaryText, style = MaterialTheme.typography.bodySmall)
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
    SettingsCard(cardBackground) {
        PrefixShellStatus(secondaryText = secondaryText)
        Spacer(modifier = Modifier.height(4.dp))
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
        Spacer(modifier = Modifier.height(8.dp))
        EnvironmentVariablesEditor(
            vars = settings.environmentVariables.toImmutableMap(),
            onSave = { viewModel.setEnvironmentVariables(it) },
            textColor = textColor,
            accentColor = accentColor,
            cardBackground = cardBackground,
            secondaryText = secondaryText,
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
    SettingsCard(cardBackground, Modifier.testTag("BootstrapSection")) {
        BootstrapSection(
            bootstrapUrl = bootstrapUrl,
            onUrlChanged = { viewModel.setBootstrapUrl(it) },
            onRunBootstrap = { viewModel.runBootstrap() },
            onInstallOffline = { uri -> viewModel.installOffline(uri) },
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
    SettingsCard(cardBackground) {
        ClearAppDataSection(viewModel = viewModel, textColor = textColor)
    }
}

@Composable
private fun SettingsCard(
    cardBackground: Color,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val isSmallScreen = rememberIsSmallScreen()
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
    val isSmallScreen = rememberIsSmallScreen()
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
private fun FontInfoSectionIfAvailable(
    fontInfo: String,
    defaultFontName: String,
    fontSize: Float,
    textColor: Color,
    secondaryText: Color,
) {
    if (fontInfo.isNotEmpty() || defaultFontName.isNotEmpty()) {
        Spacer(modifier = Modifier.height(8.dp))
        val densityDpi = LocalDensity.current.density
        val dto = FontInfoDto.fromJson(fontInfo)
        when {
            dto != null ->
                FontInfoSection(
                    fontInfo = dto,
                    pixelPerSp = densityDpi,
                    textColor = textColor,
                    secondaryText = secondaryText,
                )

            fontInfo.isEmpty() ->
                FontInfoSection(
                    fontInfo = FontInfoDto(
                        active = FontActiveDto(name = defaultFontName, monospaced = false),
                        fontSizePx = fontSize * densityDpi,
                    ),
                    pixelPerSp = densityDpi,
                    textColor = textColor,
                    secondaryText = secondaryText,
                )

            else -> {
                Text(
                    text = stringResource(R.string.no_font_loaded),
                    style = MaterialTheme.typography.bodySmall,
                    color = secondaryText,
                )
            }
        }
    }
}

@Composable
private fun FontFamilySelectors(
    regularFamily: String,
    boldFamily: String,
    italicFamily: String,
    onFamilySelected: (String, Int) -> Unit,
    customFontLauncher: androidx.activity.result.ActivityResultLauncher<Array<String>>,
    colors: SettingsColors,
    availableFonts: ImmutableList<String>,
    defaultFontName: String,
    fontInfo: String,
) {
    val pickFont = { customFontLauncher.launch(arrayOf("font/*", "application/octet-stream")) }
    SystemFontSelector(
        selectedFamily = regularFamily,
        onFamilySelected = { onFamilySelected(it, FONT_SLOT_REGULAR) },
        textColor = colors.textColor,
        cardBackground = colors.cardBackground,
        accentColor = colors.accentColor,
        fonts = availableFonts,
        defaultFontName = defaultFontName,
        fontInfo = fontInfo,
        onPickFontFile = pickFont,
    )
    Spacer(modifier = Modifier.height(12.dp))
    // Independent bold/italic families — ghostty-android TerminalFontStore
    // 4-slot design (research-ghostty-android-extra.md:80). Empty selection
    // clears the slot (falls back to same-family lookup + synthesis).
    SystemFontSelector(
        selectedFamily = boldFamily,
        onFamilySelected = { onFamilySelected(it, FONT_SLOT_BOLD) },
        textColor = colors.textColor,
        cardBackground = colors.cardBackground,
        accentColor = colors.accentColor,
        fonts = availableFonts,
        defaultFontName = defaultFontName,
        fontInfo = fontInfo,
        titleOverride = stringResource(R.string.bold_font_family),
        onPickFontFile = pickFont,
    )
    Spacer(modifier = Modifier.height(12.dp))
    SystemFontSelector(
        selectedFamily = italicFamily,
        onFamilySelected = { onFamilySelected(it, FONT_SLOT_ITALIC) },
        textColor = colors.textColor,
        cardBackground = colors.cardBackground,
        accentColor = colors.accentColor,
        fonts = availableFonts,
        defaultFontName = defaultFontName,
        fontInfo = fontInfo,
        titleOverride = stringResource(R.string.italic_font_family),
        onPickFontFile = pickFont,
    )
}

@Composable
private fun SystemFontSelector(
    selectedFamily: String,
    onFamilySelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
    fonts: ImmutableList<String> = persistentListOf(),
    defaultFontName: String = "",
    fontInfo: String = "",
    titleOverride: String? = null,
    onPickFontFile: (() -> Unit)? = null,
) {
    val systemFonts = remember(fonts) { fonts.distinct().sorted() }
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle = if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    val displayName = if (defaultFontName.isEmpty()) "Noto Sans Mono" else defaultFontName

    Spacer(modifier = Modifier.height(4.dp))
    Text(
        text = titleOverride ?: stringResource(R.string.font_family),
        style = labelStyle,
        color = textColor,
        maxLines = 1,
    )
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
                fonts = systemFonts.toImmutableList(),
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

    val fontInfoDto = FontInfoDto.fromJson(fontInfo)
    if (fontInfoDto?.hasRealCjkFallback == true) {
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = stringResource(R.string.cjk_value_label, fontInfoDto.cjkFallbackText() ?: ""),
            style = MaterialTheme.typography.bodySmall,
            color = textColor.copy(alpha = 0.6f),
        )
    } else if (fontInfoDto?.cjkState == "none") {
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
    fonts: ImmutableList<String>,
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
    val isSmall = rememberIsSmallScreen()
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
private fun PrefixShellStatus(secondaryText: Color) {
    // Show what the runtime actually resolves as the launch location:
    // the prefix bootstrap (nix-on-droid bin/login or termux bin/bash) or
    // the system fallback. Mirrors TerminalRuntime prefixShell resolution.
    val context = LocalContext.current
    val prefixDir = File(context.filesDir, "usr")
    // Pure logic first (no resource access inside remember — lint
    // LocalContextGetResourceValueCall): pick the string resource id.
    val (statusResId, prefixArg) =
        remember(prefixDir) {
            val login = File(prefixDir, "bin/login")
            val bash = File(prefixDir, "bin/bash")
            // Only real ELF binaries count: termux ships bin/login as a
            // shebang script (motd), nix-on-droid as a static ELF.
            val loginIsNix = login.isFile && isElf(login)
            when {
                loginIsNix ->
                    R.string.launch_location_status_nix to prefixDir.absolutePath

                bash.exists() ->
                    R.string.launch_location_status_termux to prefixDir.absolutePath

                else -> R.string.launch_location_status_none to ""
            }
        }
    val status = stringResource(statusResId, prefixArg)
    Text(status, style = MaterialTheme.typography.bodySmall, color = secondaryText)
}

@Composable
private fun ShellInput(
    shellPath: String,
    onShellChanged: (String) -> Unit,
    textColor: Color,
    accentColor: Color,
) {
    val isSmallScreen = rememberIsSmallScreen()
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
    themes: ImmutableList<TerminalTheme>,
    onThemeSelected: (String) -> Unit,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
) {
    val isSmallScreen = rememberIsSmallScreen()
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
                    // `selectedTheme` is always a theme *name* (built-in or
                    // user-created); do NOT fall back through byName() here —
                    // its default returns Catppuccin Mocha, which would
                    // highlight that card whenever a user theme is selected.
                    isSelected = theme.name == selectedTheme,
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
    onInstallOffline: (android.net.Uri) -> Unit,
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

    // Offline install: pick a.zip file via SAF, no network required
    Spacer(modifier = Modifier.height(8.dp))
    val offlineLauncher = rememberLauncherForActivityResult(
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

private const val BYTES_PER_KB = 1024L
private const val BYTES_PER_MB = BYTES_PER_KB * 1024

@Composable
private fun formatBytes(bytes: Long): String = when {
    bytes >= BYTES_PER_MB -> stringResource(R.string.byte_unit_mb, bytes / BYTES_PER_MB)
    bytes >= BYTES_PER_KB -> stringResource(R.string.byte_unit_kb, bytes / BYTES_PER_KB)
    else -> stringResource(R.string.byte_unit_b, bytes)
}

@Composable
private fun bootstrapStepText(progress: BootstrapProgress): String = when (progress) {
    is BootstrapProgress.Downloading -> {
        val pct =
            if (progress.contentLength > 0) {
                " (${(progress.bytesWritten * 100 / progress.contentLength)}%)"
            } else {
                ""
            }
        stringResource(
            R.string.bootstrap_downloading,
            pct,
            formatBytes(progress.bytesWritten),
            if (progress.contentLength > 0) formatBytes(progress.contentLength) else "",
        )
    }

    is BootstrapProgress.Extracting -> {
        val pct =
            if (progress.totalEntries > 0) {
                " (${(progress.entriesExtracted * 100 / progress.totalEntries)}%)"
            } else {
                ""
            }
        stringResource(R.string.bootstrap_extracting, pct, progress.entriesExtracted, progress.totalEntries)
    }

    is BootstrapProgress.RunningPostInstall ->
        stringResource(
            R.string.bootstrap_running_postinstall,
            progress.scriptsCompleted,
            progress.totalScripts,
        )

    BootstrapProgress.CreatingSymlinks -> stringResource(R.string.bootstrap_creating_symlinks)

    BootstrapProgress.Complete -> stringResource(R.string.bootstrap_complete)

    is BootstrapProgress.Error -> progress.message
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

/**
 * Environment variables editor: clickable row opening a dialog with a
 * "KEY=VALUE" (one per line) text field. Parsing mirrors the native
 * `parse_env_entries` (shell_env.rs) so both sides agree on the shape.
 */
@Composable
private fun EnvironmentVariablesEditor(
    vars: ImmutableMap<String, String>,
    onSave: (ImmutableMap<String, String>) -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
    secondaryText: Color,
) {
    var showEditor by rememberSaveable { mutableStateOf(false) }

    Column {
        Row(
            modifier =
            Modifier
                .testTag("EnvironmentVariablesEditor")
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                .background(cardBackground)
                .clickable { showEditor = true }
                .padding(horizontal = 12.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = stringResource(R.string.environment_variables),
                    style = MaterialTheme.typography.bodyLarge,
                    color = textColor,
                )
                Text(
                    text = stringResource(R.string.environment_variables_desc),
                    style = MaterialTheme.typography.bodySmall,
                    color = textColor.copy(alpha = 0.6f),
                )
            }
            Text(
                text = "${vars.size}",
                color = secondaryText,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(end = 8.dp),
            )
            Text(
                text = stringResource(R.string.change),
                color = accentColor,
                style = MaterialTheme.typography.bodySmall,
            )
        }

        if (showEditor) {
            EnvironmentVariablesDialog(
                initialText = serializeEnvironmentVariables(vars),
                onSave = {
                    onSave(parseEnvironmentVariables(it).toImmutableMap())
                    showEditor = false
                },
                onDismiss = { showEditor = false },
                textColor = textColor,
                accentColor = accentColor,
                cardBackground = cardBackground,
            )
        }
    }
}

@Composable
private fun EnvironmentVariablesDialog(
    initialText: String,
    onSave: (String) -> Unit,
    onDismiss: () -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
) {
    var text by rememberSaveable { mutableStateOf(initialText) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.environment_variables)) },
        text = {
            Column {
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    placeholder = { Text(stringResource(R.string.environment_variables_hint)) },
                    modifier =
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 160.dp),
                    colors =
                    OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = accentColor,
                        cursorColor = accentColor,
                        focusedTextColor = textColor,
                        unfocusedTextColor = textColor,
                    ),
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onSave(text) },
                modifier = Modifier.testTag("SaveEnvironmentVariables"),
            ) {
                Text(stringResource(R.string.environment_variables_save), color = accentColor)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(android.R.string.cancel), color = textColor)
            }
        },
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
    fontInfo: FontInfoDto,
    pixelPerSp: Float,
    textColor: Color,
    secondaryText: Color,
) {
    Column(modifier = Modifier.testTag("FontInfoSection")) {
        fontInfo.active?.let { active ->
            Text(
                text = stringResource(R.string.font_info_active, active.name),
                style = MaterialTheme.typography.bodyMedium,
                color = textColor,
            )
        }
        val cjkText = fontInfo.cjkFallbackText()
        val cjkCoveredByPrimary = fontInfo.cjkState == "skipped"
        val hasCjk = fontInfo.hasRealCjkFallback
        val cjkDisplayColor =
            when {
                hasCjk -> secondaryText
                cjkCoveredByPrimary -> secondaryText
                else -> WARNING_ORANGE
            }
        val cjkLine =
            when {
                cjkText != null -> stringResource(R.string.font_info_cjk_fallback, cjkText)
                cjkCoveredByPrimary -> stringResource(R.string.font_info_cjk_skipped)
                else -> stringResource(R.string.font_info_cjk_none)
            }
        Text(
            text = cjkLine,
            style = MaterialTheme.typography.bodySmall,
            color = cjkDisplayColor,
        )
        if (!hasCjk && !cjkCoveredByPrimary) {
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = stringResource(R.string.cjk_fallback_missing_warning),
                style = MaterialTheme.typography.bodySmall,
                color = WARNING_ORANGE,
            )
        }
        if (fontInfo.cellWidthPx > 0f && fontInfo.cellHeightPx > 0f) {
            Text(
                text = stringResource(R.string.font_info_cell, fontInfo.cellWidthPx, fontInfo.cellHeightPx),
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
            )
        }
        if (fontInfo.fontSizePx > 0f) {
            val sizeSp = fontInfo.fontSizePx / pixelPerSp
            Text(
                text = stringResource(R.string.font_info_size_sp, sizeSp, fontInfo.fontSizePx),
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
            )
        }
    }
}

private const val CURSOR_SPEED_RANGE_MIN = 100f
private const val CURSOR_SPEED_RANGE_MAX = 1000f
private const val CURSOR_SPEED_RANGE_STEPS = 17

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
            "block" to stringResource(R.string.cursor_block),
            "bar" to stringResource(R.string.cursor_bar),
            "underline" to stringResource(R.string.cursor_underline),
        )
    SettingsSelectorRow(
        title = stringResource(R.string.cursor_style_label),
        selectedKey = selectedStyle,
        options = styles.toImmutableList(),
        colors = SettingsColors(textColor, textColor, accentColor, cardBackground),
        onOptionSelected = onStyleSelected,
        testTag = "CursorStyleSelector",
        optionTestTagPrefix = "CursorStyle",
    )
}

@Composable
private fun BellModeSelector(
    selectedModeId: Int,
    onModeSelected: (Int) -> Unit,
    textColor: Color,
    accentColor: Color,
    cardBackground: Color,
) {
    val options =
        BellMode.entries.map { mode ->
            mode.id.toString() to mode.displayName
        }
    SettingsSelectorRow(
        title = stringResource(R.string.bell_mode),
        selectedKey = selectedModeId.toString(),
        options = options.toImmutableList(),
        colors = SettingsColors(textColor, textColor, accentColor, cardBackground),
        onOptionSelected = { key -> onModeSelected(key.toInt()) },
        testTag = "BellModeSelector",
        optionTestTagPrefix = "BellMode",
    )
}

// ══════════════════════════════════════════════════════════════════════
// Keyboard Shortcuts Section
// ══════════════════════════════════════════════════════════════════════

@Composable
private fun KeyboardShortcutsSection(
    viewModel: TerminalViewModel,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    val bindings by viewModel.shortcutBindings.collectAsStateWithLifecycle()
    var capturingAction by rememberSaveable { mutableStateOf<String?>(null) }
    val labelStyle = if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge

    SectionHeader(stringResource(R.string.keyboard_shortcuts), sectionTitleColor)
    SettingsCard(cardBackground) {
        terminal.emulator.shortcut.KeyShortcutHandler.Action.entries.forEach { action ->
            val actionId = when (action) {
                terminal.emulator.shortcut.KeyShortcutHandler.Action.Paste ->
                    terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_PASTE

                terminal.emulator.shortcut.KeyShortcutHandler.Action.NewSession ->
                    terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_NEW_SESSION

                terminal.emulator.shortcut.KeyShortcutHandler.Action.CloseSession ->
                    terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_CLOSE_SESSION

                terminal.emulator.shortcut.KeyShortcutHandler.Action.Copy ->
                    terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_COPY

                terminal.emulator.shortcut.KeyShortcutHandler.Action.ToggleScroll ->
                    terminal.emulator.shortcut.KeyShortcutHandler.Defaults.ACTION_ID_TOGGLE_SCROLL
            }
            val binding = bindings[actionId] ?: terminal.emulator.shortcut.ShortcutBinding.EMPTY
            val actionLabel = when (action) {
                terminal.emulator.shortcut.KeyShortcutHandler.Action.Paste -> stringResource(R.string.paste)
                terminal.emulator.shortcut.KeyShortcutHandler.Action.NewSession -> stringResource(R.string.cd_new_session)
                terminal.emulator.shortcut.KeyShortcutHandler.Action.CloseSession -> stringResource(R.string.close_session)
                terminal.emulator.shortcut.KeyShortcutHandler.Action.Copy -> stringResource(R.string.copy)
                terminal.emulator.shortcut.KeyShortcutHandler.Action.ToggleScroll -> stringResource(R.string.toggle_scroll)
            }

            Row(
                verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { capturingAction = actionId }
                    .testTag("Shortcut_$actionId")
                    .padding(horizontal = 16.dp, vertical = 12.dp),
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(actionLabel, style = labelStyle, color = textColor)
                    Text(
                        binding.toDisplayString(),
                        style = MaterialTheme.typography.bodySmall,
                        color = secondaryText,
                    )
                }
                TextButton(
                    onClick = { viewModel.resetShortcutBinding(actionId) },
                    modifier = Modifier.testTag("ResetShortcut_$actionId"),
                ) {
                    Text(stringResource(R.string.shortcut_reset), color = secondaryText)
                }
            }
        }
    }

    // Shortcut capture dialog
    capturingAction?.let { actionId ->
        val currentBinding = bindings[actionId] ?: terminal.emulator.shortcut.ShortcutBinding.EMPTY
        terminal.emulator.shortcut.ShortcutCaptureDialog(
            current = currentBinding,
            conflictDetector = { binding -> viewModel.hasShortcutConflict(actionId, binding) },
            onDismiss = { capturingAction = null },
            onSave = { binding ->
                viewModel.updateShortcutBinding(actionId, binding)
                capturingAction = null
            },
        )
    }
}

/**
 * "Modifier Bar" settings section: previews the toolbar layout as key
 * chips and opens an editor dialog to add/remove keys or reset to the
 * default layout. Persists through [ToolbarPreferences.saveLayout].
 */
@Composable
private fun ModifierBarSettingsSection(
    viewModel: TerminalViewModel,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    val context = LocalContext.current
    val toolbarPreferences = remember { ToolbarPreferences(context) }
    var layout by remember { mutableStateOf(toolbarPreferences.getLayout()) }
    var showEditor by rememberSaveable { mutableStateOf(false) }

    SectionHeader(stringResource(R.string.modifier_bar), sectionTitleColor)
    SettingsCard(cardBackground) {
        Text(
            text = stringResource(R.string.modifier_bar_current_layout),
            color = secondaryText,
            style = MaterialTheme.typography.bodySmall,
        )
        Spacer(modifier = Modifier.height(4.dp))
        ToolbarLayoutPreview(layout.toImmutableList(), textColor, cardBackground)
        Spacer(modifier = Modifier.height(12.dp))
        OutlinedButton(
            onClick = { showEditor = true },
            modifier = Modifier.testTag("EditModifierBarButton"),
        ) {
            Text(stringResource(R.string.edit), color = MaterialTheme.colorScheme.primary)
        }
    }

    if (showEditor) {
        ModifierBarEditorDialog(
            layout = layout.toImmutableList(),
            onLayoutChange = { layout = it },
            onSave = {
                toolbarPreferences.saveLayout(layout)
                showEditor = false
            },
            onReset = { layout = toolbarPreferences.defaultLayout() },
            onDismiss = { showEditor = false },
            textColor = textColor,
            secondaryText = secondaryText,
            cardBackground = cardBackground,
        )
    }
}

/** Key chips laid out in wrapping rows (mirrors the toolbar's 2-row shape). */
@OptIn(ExperimentalLayoutApi::class)
@SuppressLint("DeprecatedCall") // FlowRow has a deprecated overload with
// `overflow`; our calls use the non-deprecated signature. The rule matches
// the containing file class and misreports.
@Composable
private fun ToolbarLayoutPreview(
    layout: ImmutableList<ToolbarItem>,
    textColor: Color,
    cardBackground: Color,
) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
        modifier = Modifier.testTag("ModifierBarPreview"),
    ) {
        layout.forEach { item ->
            ToolbarKeyChip(
                label = itemLabel(item),
                textColor = textColor,
                cardBackground = cardBackground,
                onClick = {},
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ModifierBarEditorDialog(
    layout: ImmutableList<ToolbarItem>,
    onLayoutChange: (ImmutableList<ToolbarItem>) -> Unit,
    onSave: () -> Unit,
    onReset: () -> Unit,
    onDismiss: () -> Unit,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
) {
    val currentKeys = layout.filterIsInstance<ToolbarItem.Default>().map { it.key }.toSet()
    val availableKeys = ToolbarKey.entries.filter { it !in currentKeys }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.edit_modifier_bar)) },
        text = {
            Column(
                modifier =
                Modifier
                    .verticalScroll(rememberScrollState())
                    .testTag("ModifierBarEditorDialog"),
            ) {
                Text(
                    text = stringResource(R.string.modifier_bar_edit_hint_remove),
                    color = secondaryText,
                    style = MaterialTheme.typography.bodySmall,
                )
                Spacer(modifier = Modifier.height(8.dp))
                CurrentLayoutEditor(
                    layout = layout,
                    onLayoutChange = onLayoutChange,
                    textColor = textColor,
                    secondaryText = secondaryText,
                    cardBackground = cardBackground,
                )
                Spacer(modifier = Modifier.height(12.dp))
                Text(
                    text = stringResource(R.string.modifier_bar_available_keys),
                    color = secondaryText,
                    style = MaterialTheme.typography.bodySmall,
                )
                Spacer(modifier = Modifier.height(8.dp))
                AvailableKeysPicker(
                    availableKeys = availableKeys.toImmutableList(),
                    onLayoutChange = onLayoutChange,
                    layout = layout,
                    textColor = textColor,
                    cardBackground = cardBackground,
                )
                Spacer(modifier = Modifier.height(8.dp))
                TextButton(
                    onClick = onReset,
                    modifier = Modifier.testTag("ResetModifierBarButton"),
                ) {
                    Text(stringResource(R.string.reset_to_default), color = MaterialTheme.colorScheme.primary)
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = onSave,
                modifier = Modifier.testTag("SaveModifierBarButton"),
            ) {
                Text(stringResource(R.string.save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
        },
    )
}

/** The current key list with per-key width steppers and secondary labels. */
@Composable
private fun CurrentLayoutEditor(
    layout: ImmutableList<ToolbarItem>,
    onLayoutChange: (ImmutableList<ToolbarItem>) -> Unit,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        layout.forEach { item ->
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                ToolbarKeyChip(
                    label = itemLabel(item),
                    textColor = textColor,
                    cardBackground = cardBackground,
                    onClick = { onLayoutChange((layout - item).toImmutableList()) },
                )
                Text(
                    text = "W",
                    color = secondaryText,
                    style = MaterialTheme.typography.labelSmall,
                )
                IconButton(
                    onClick = {
                        onLayoutChange(
                            layout.replace(
                                item,
                                item.withWidth((item.width - 1).coerceAtLeast(1)),
                            ).toImmutableList(),
                        )
                    },
                    modifier = Modifier.size(28.dp).testTag("WidthMinus_${itemLabel(item)}"),
                ) {
                    Text("−", color = secondaryText)
                }
                Text(
                    text = "${item.width}",
                    color = textColor,
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.testTag("WidthValue_${itemLabel(item)}"),
                )
                IconButton(
                    onClick = {
                        onLayoutChange(
                            layout.replace(
                                item,
                                item.withWidth((item.width + 1).coerceAtMost(4)),
                            ).toImmutableList(),
                        )
                    },
                    modifier = Modifier.size(28.dp).testTag("WidthPlus_${itemLabel(item)}"),
                ) {
                    Text("+", color = secondaryText)
                }
                BasicTextField(
                    value = item.secondaryLabel.orEmpty(),
                    onValueChange = { value ->
                        onLayoutChange(layout.replace(item, item.withSecondary(value)).toImmutableList())
                    },
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodySmall.copy(color = textColor),
                    modifier =
                    Modifier
                        .weight(1f)
                        .testTag("SecondaryLabel_${itemLabel(item)}")
                        .background(
                            cardBackground,
                            RoundedCornerShape(4.dp),
                        )
                        .padding(horizontal = 6.dp, vertical = 4.dp),
                )
            }
        }
    }
}

/** The keys not yet in the layout, tappable to append. */
@SuppressLint("DeprecatedCall") // see ToolbarLayoutPreview
@Composable
private fun AvailableKeysPicker(
    availableKeys: ImmutableList<ToolbarKey>,
    layout: ImmutableList<ToolbarItem>,
    onLayoutChange: (ImmutableList<ToolbarItem>) -> Unit,
    textColor: Color,
    cardBackground: Color,
) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        availableKeys.forEach { key ->
            ToolbarKeyChip(
                label = key.defaultLabel,
                textColor = textColor,
                cardBackground = cardBackground,
                onClick = { onLayoutChange((layout + ToolbarItem.Default(key)).toImmutableList()) },
            )
        }
    }
}

private fun itemLabel(item: ToolbarItem): String = when (item) {
    is ToolbarItem.Default -> item.key.defaultLabel
    is ToolbarItem.Custom -> item.label
}

/** Returns a copy of this item with a new row weight. */
private fun ToolbarItem.withWidth(width: Int): ToolbarItem = when (this) {
    is ToolbarItem.Default -> copy(width = width)
    is ToolbarItem.Custom -> copy(width = width)
}

/** Returns a copy with a secondary long-press key; the sequence is the
 *  label itself, matching termux extra-keys secondary key semantics. */
private fun ToolbarItem.withSecondary(label: String): ToolbarItem {
    val trimmed = label.trim()
    val secondaryLabel = trimmed.ifEmpty { null }
    val secondarySequence = trimmed.ifEmpty { null }
    return when (this) {
        is ToolbarItem.Default -> copy(secondaryLabel = secondaryLabel, secondarySequence = secondarySequence)
        is ToolbarItem.Custom -> copy(secondaryLabel = secondaryLabel, secondarySequence = secondarySequence)
    }
}

/** Replaces the first item equal to `old` with `new`. */
private fun List<ToolbarItem>.replace(old: ToolbarItem, new: ToolbarItem): List<ToolbarItem> {
    val index = indexOf(old)
    if (index < 0) return this
    return toMutableList().also { it[index] = new }
}

@Composable
private fun ToolbarKeyChip(
    label: String,
    onClick: () -> Unit,
    textColor: Color,
    cardBackground: Color,
) {
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(6.dp),
        color = cardBackground,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        modifier = Modifier.testTag("ToolbarKeyChip_$label"),
    ) {
        Text(
            text = label,
            color = textColor,
            style = MaterialTheme.typography.labelMedium,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
        )
    }
}
