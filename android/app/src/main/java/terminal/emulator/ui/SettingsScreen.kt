package terminal.emulator.ui

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
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
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.collections.immutable.ImmutableList
import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toImmutableList
import kotlinx.coroutines.launch
import terminal.emulator.R
import terminal.emulator.TerminalViewModel
import terminal.emulator.bridge.CjkState
import terminal.emulator.bridge.FontInfo
import terminal.emulator.bridge.fontSpToPx
import terminal.emulator.installer.BootstrapProgress
import terminal.emulator.installer.TermuxBootstrap
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.TerminalTheme
import terminal.emulator.ui.theme.ThemeMode

/** 危险操作（清除应用数据）的警示红色。 */
private val DANGER_RED = Color.Red

@OptIn(ExperimentalMaterial3Api::class) // Material3 experimental API used intentionally
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: TerminalViewModel = hiltViewModel(),
) {
    val isSmallScreen = rememberIsSmallScreen()
    val horizontalPadding = if (isSmallScreen) 8.dp else 16.dp
    val backgroundColor = MaterialTheme.colorScheme.surface
    val textColor = MaterialTheme.colorScheme.onSurface
    val secondaryText = MaterialTheme.colorScheme.onSurfaceVariant
    val cardBackground = MaterialTheme.colorScheme.surfaceContainerLow
    val accentColor = MaterialTheme.colorScheme.primary
    val sectionTitleColor = MaterialTheme.colorScheme.primary
    // 单一订阅：SettingsRepository 只暴露一条设置 flow，在 LazyColumn 外订阅一次
    // 向下传递，避免每个 item 各自 collect 造成多条并行管线。
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val availableFonts by viewModel.availableFonts.collectAsStateWithLifecycle()
    val defaultFontName by viewModel.defaultFontName.collectAsStateWithLifecycle()
    val fontInfo by viewModel.fontInfo.collectAsStateWithLifecycle()
    val bootstrapRunning by viewModel.bootstrapRunning.collectAsStateWithLifecycle()
    val bootstrapResult by viewModel.bootstrapResult.collectAsStateWithLifecycle()
    val bootstrapProgress: BootstrapProgress? by viewModel.bootstrapProgress.collectAsStateWithLifecycle()
    BackHandler(enabled = true) { onBack() }
    // 系统栏图标明暗按**设置页自己的底色**判定：设置页是浅色面，若沿用终端页
    // （深底）的判定，状态栏时钟会是浅色压浅底，几乎看不清。
    SystemBarsFollowBackground(LocalView.current, backgroundColor)
    Surface(
        modifier =
        modifier.fillMaxSize()
            .testTag("SettingsScreen")
            // 消费设置背景上的点击，使其不穿透到下层的 TerminalScreen。
            // 必须用 pointerInput 而非 clickable：clickable 会强制
            // mergeDescendants 语义，从合并树中吞掉所有后代的 testTag，
            // 使整个界面对 Compose UI 测试与 TalkBack 不可读。
            .pointerInput(Unit) { detectTapGestures(onTap = {}) },
        color = backgroundColor,
    ) {
        Column(modifier = Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
            // 固定表头：SettingsHeader 必须在 LazyColumn 滚动时保持可见，
            // 否则返回按钮会滚出可达范围
            // （maestro 的 open-settings 流程在向下滚动后找不到 SettingsBackButton）。
            SettingsHeader(onBack, textColor, isSmallScreen)
            LazyColumn(
                modifier =
                Modifier.fillMaxSize()
                    .padding(horizontal = horizontalPadding)
                    .testTag("SettingsLazyColumn"),
                verticalArrangement = Arrangement.spacedBy(if (isSmallScreen) 8.dp else 12.dp),
                contentPadding = PaddingValues(bottom = 32.dp),
            ) {
                item {
                    SectionHeader(stringResource(R.string.appearance), sectionTitleColor)
                    SettingsCard(cardBackground) {
                        AppearanceSectionContent(
                            fontSize = settings.fontSize,
                            spToPxScale = viewModel.runtime.spToPxScale,
                            screenWidthDp = viewModel.runtime.screenWidthDp,
                            fontFamily = settings.fontFamily,
                            availableFonts = availableFonts,
                            defaultFontName = defaultFontName,
                            fontInfo = fontInfo,
                            onFontSizePreview = { viewModel.setFontSizeInPlacePreview(it) },
                            onFontSizeCommitted = { viewModel.setFontSize(it) },
                            onFontFamilySelected = { viewModel.setFontFamily(it) },
                            onFontsVisible = { viewModel.refreshFontList() },
                            textColor = textColor,
                            secondaryText = secondaryText,
                            accentColor = accentColor,
                            backgroundColor = backgroundColor,
                        )
                    }
                }
                item {
                    AppThemeSection(
                        appThemeMode = settings.appThemeMode,
                        onAppThemeModeSelected = { viewModel.setAppThemeMode(it) },
                        cardBackground = cardBackground,
                        textColor = textColor,
                        accentColor = accentColor,
                        sectionTitleColor = sectionTitleColor,
                    )
                }
                item {
                    TerminalThemeSection(
                        themeMode = settings.themeMode,
                        dayThemeName = settings.dayThemeName,
                        nightThemeName = settings.nightThemeName,
                        themeName = settings.themeName,
                        onThemeModeSelected = { viewModel.setThemeMode(it) },
                        onDayThemeSelected = { viewModel.setDayThemeName(it) },
                        onNightThemeSelected = { viewModel.setNightThemeName(it) },
                        onFixedThemeSelected = { viewModel.setThemeName(it) },
                        textColor = textColor,
                        secondaryText = secondaryText,
                        cardBackground = cardBackground,
                        sectionTitleColor = sectionTitleColor,
                        isSmallScreen = isSmallScreen,
                    )
                }
                item {
                    TerminalConfigSection(
                        selectedShell = settings.shell,
                        onShellSaved = { viewModel.setShell(it) },
                        textColor = textColor,
                        secondaryText = secondaryText,
                        accentColor = accentColor,
                        cardBackground = cardBackground,
                        backgroundColor = backgroundColor,
                        sectionTitleColor = sectionTitleColor,
                        isSmallScreen = isSmallScreen,
                    )
                }
                item {
                    BootstrapSectionFromSettings(
                        bootstrapUrl = settings.bootstrapUrl,
                        bootstrapRunning = bootstrapRunning,
                        bootstrapResult = bootstrapResult,
                        bootstrapProgress = bootstrapProgress,
                        onUrlChanged = { viewModel.setBootstrapUrl(it) },
                        onRunBootstrap = { viewModel.runBootstrap() },
                        onInstallOffline = { viewModel.installOffline(it) },
                        textColor = textColor,
                        secondaryText = secondaryText,
                        accentColor = accentColor,
                        cardBackground = cardBackground,
                        sectionTitleColor = sectionTitleColor,
                        isSmallScreen = isSmallScreen,
                    )
                }
                item {
                    ClearAppDataSectionItem(
                        onClearAppData = { viewModel.clearAppData() },
                        textColor = textColor,
                        cardBackground = cardBackground,
                        sectionTitleColor = sectionTitleColor,
                        isSmallScreen = isSmallScreen,
                    )
                }
                item { Spacer(modifier = Modifier.height(24.dp)) }
            }
        }
    }
}

@Composable
private fun SettingsHeader(onBack: () -> Unit, textColor: Color, isSmallScreen: Boolean) {
    Column {
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
                style =
                if (isSmallScreen) {
                    MaterialTheme.typography.titleLarge
                } else {
                    MaterialTheme.typography.headlineSmall
                },
                color = textColor,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

@Composable
private fun AppearanceSectionContent(
    fontSize: Float,
    spToPxScale: Float,
    screenWidthDp: Float,
    fontFamily: String,
    availableFonts: List<String>,
    defaultFontName: String,
    fontInfo: FontInfo?,
    onFontSizePreview: (Float) -> Unit,
    onFontSizeCommitted: (Float) -> Unit,
    onFontFamilySelected: (String) -> Unit,
    onFontsVisible: () -> Unit,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    backgroundColor: Color,
) {
    // 拖动过程只走轻量路径做预览（setFontSizeInPlace + 单元格度量刷新，
    // 不写 DataStore、不重排网格）；数值在松手时一次性提交。
    // 每一步拖动都提交会并发运行完整的 applyFontSettings 链（IO 调度器），
    // 使 JNI setFontSizeInPlace 调用乱序交错（实测 96..280）并逐步重排网格
    // ——即「滑块跳动/布局错乱」的来源。预览让拖动保持廉价且单线程。
    // 初值钳到可选区间：历史版本存过的超限值只影响本次显示，不回写覆盖用户数据。
    val fontSizeUpperBound = SettingsRepository.fontSizeMaxSp(spToPxScale, screenWidthDp)
    var sliderFontSize by rememberSaveable {
        mutableFloatStateOf(
            fontSize.coerceIn(SettingsRepository.FONT_SIZE_MIN_SP, fontSizeUpperBound),
        )
    }
    Column {
        FontSizeSlider(
            modifier = Modifier.testTag("FontSizeSlider"),
            value = sliderFontSize,
            onValueChange = {
                sliderFontSize = it
                onFontSizePreview(it)
            },
            onValueChangeFinished = { onFontSizeCommitted(sliderFontSize) },
            spToPxScale = spToPxScale,
            screenWidthDp = screenWidthDp,
            textColor = textColor,
            secondaryText = secondaryText,
            accentColor = accentColor,
        )
        Spacer(modifier = Modifier.height(12.dp))
        SystemFontSelector(
            selectedFamily = fontFamily,
            onFamilySelected = onFontFamilySelected,
            onFontsVisible = onFontsVisible,
            textColor = textColor,
            cardBackground = backgroundColor,
            accentColor = accentColor,
            fonts = availableFonts.toImmutableList(),
            defaultFontName = defaultFontName,
        )
        FontInfoSectionIfAvailable(
            fontInfo = fontInfo,
            defaultFontName = defaultFontName,
            fontSize = fontSize,
            spToPxScale = spToPxScale,
            textColor = textColor,
            secondaryText = secondaryText,
        )
        Spacer(modifier = Modifier.height(12.dp))
    }
}

@Composable
private fun AppThemeSection(
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

@Composable
private fun TerminalThemeSection(
    themeMode: String,
    dayThemeName: String,
    nightThemeName: String,
    themeName: String,
    onThemeModeSelected: (String) -> Unit,
    onDayThemeSelected: (String) -> Unit,
    onNightThemeSelected: (String) -> Unit,
    onFixedThemeSelected: (String) -> Unit,
    textColor: Color,
    secondaryText: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    // 主题单源在 Rust 侧，经仓库取回；自定义主题为禁止实现。取进程内缓存而非
    // `remember`：本组合在 LazyColumn 列表项内，滚出再滚回会重跑整轮 JNI。
    val allThemes = terminal.emulator.ui.theme.ThemeRepository.allCached

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
            when (themeMode) {
                ThemeMode.FOLLOW_SYSTEM,
                ThemeMode.DAY,
                ThemeMode.NIGHT,
                -> {
                    Column(modifier = Modifier.testTag("DayNightThemeSection")) {
                        ThemeSelector(
                            label = stringResource(R.string.day_theme),
                            selectedTheme = dayThemeName,
                            themes = allThemes,
                            onThemeSelected = { onDayThemeSelected(it) },
                            textColor = textColor,
                            secondaryText = secondaryText,
                            cardBackground = cardBackground,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        ThemeSelector(
                            label = stringResource(R.string.night_theme),
                            selectedTheme = nightThemeName,
                            themes = allThemes,
                            onThemeSelected = { onNightThemeSelected(it) },
                            textColor = textColor,
                            secondaryText = secondaryText,
                            cardBackground = cardBackground,
                        )
                    }
                }

                ThemeMode.FIXED -> {
                    ThemeSelector(
                        label = "",
                        selectedTheme = themeName,
                        themes = allThemes,
                        onThemeSelected = { onFixedThemeSelected(it) },
                        textColor = textColor,
                        secondaryText = secondaryText,
                        cardBackground = cardBackground,
                    )
                }
            }
            Spacer(modifier = Modifier.height(12.dp))
        }
    }
}

@Composable
private fun TerminalConfigSection(
    selectedShell: String,
    onShellSaved: (String) -> Unit,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    cardBackground: Color,
    backgroundColor: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
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
private fun BootstrapSectionFromSettings(
    bootstrapUrl: String,
    bootstrapRunning: Boolean,
    bootstrapResult: String?,
    bootstrapProgress: BootstrapProgress?,
    onUrlChanged: (String) -> Unit,
    onRunBootstrap: () -> Unit,
    onInstallOffline: (android.net.Uri) -> Unit,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
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
private fun ClearAppDataSectionItem(
    onClearAppData: () -> Unit,
    textColor: Color,
    cardBackground: Color,
    sectionTitleColor: Color,
    isSmallScreen: Boolean,
) {
    Column {
        SectionHeader(stringResource(R.string.clear_app_data), sectionTitleColor)
        SettingsCard(cardBackground) {
            ClearAppDataSection(onClearAppData = onClearAppData, textColor = textColor)
        }
    }
}

@Composable
private fun SettingsCard(cardBackground: Color, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
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
private fun SectionHeader(title: String, textColor: Color) {
    val isSmallScreen = rememberIsSmallScreen()
    Text(
        text = title,
        style =
        if (isSmallScreen) {
            MaterialTheme.typography.titleSmall
        } else {
            MaterialTheme.typography.titleMedium
        },
        fontWeight = FontWeight.Bold,
        color = textColor,
        modifier = Modifier.padding(vertical = 4.dp),
    )
}

@Composable
private fun FontSizeSlider(
    value: Float,
    onValueChange: (Float) -> Unit,
    spToPxScale: Float,
    screenWidthDp: Float,
    textColor: Color,
    secondaryText: Color,
    accentColor: Color,
    modifier: Modifier = Modifier,
    onValueChangeFinished: () -> Unit = {},
) {
    SettingsSliderRow(
        title = stringResource(R.string.font_size),
        value = value,
        valueRange =
        SettingsRepository.FONT_SIZE_MIN_SP..SettingsRepository.fontSizeMaxSp(spToPxScale, screenWidthDp),
        steps = SettingsRepository.fontSizeRangeSteps(spToPxScale, screenWidthDp),
        colors =
        SettingsColors(textColor, secondaryText, accentColor, cardBackground = Color.Transparent),
        onValueChange = onValueChange,
        onValueChangeFinished = onValueChangeFinished,
        modifier = modifier,
    )
}

@Composable
private fun FontInfoSectionIfAvailable(
    fontInfo: FontInfo?,
    defaultFontName: String,
    fontSize: Float,
    spToPxScale: Float,
    textColor: Color,
    secondaryText: Color,
) {
    if (fontInfo != null || defaultFontName.isNotEmpty()) {
        Spacer(modifier = Modifier.height(8.dp))
        if (fontInfo != null) {
            FontInfoSection(
                fontInfo = fontInfo,
                pixelPerSp = spToPxScale,
                textColor = textColor,
                secondaryText = secondaryText,
            )
        } else {
            // 原生尚未上报度量时，只用设置里的字体族名与字号占位，不伪造单元格尺寸。
            // CJK 状态同理：此刻还没有结论，故报「尚未上报」而非「无」——
            // 报「无」会凭空造出一个不存在的缺失问题。
            FontInfoSection(
                fontInfo =
                FontInfo(
                    activeName = defaultFontName,
                    activeMonospaced = false,
                    cjkState = CjkState.NOT_REPORTED,
                    cjkFamilies = emptyList(),
                    otherFamilies = emptyList(),
                    cellWidthPx = 0f,
                    cellHeightPx = 0f,
                    fontSizeSp = fontSize,
                ),
                pixelPerSp = spToPxScale,
                textColor = textColor,
                secondaryText = secondaryText,
            )
        }
    }
}

@Composable
private fun SystemFontSelector(
    selectedFamily: String,
    onFamilySelected: (String) -> Unit,
    onFontsVisible: () -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
    fonts: ImmutableList<String> = persistentListOf(),
    defaultFontName: String = "",
    titleOverride: String? = null,
) {
    // 保持传入顺序（fonts.xml 文档顺序在前），只做精确去重。
    val systemFonts = remember(fonts) { fonts.distinct() }
    val isSmallScreen = rememberIsSmallScreen()
    val labelStyle =
        if (isSmallScreen) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge
    val displayName = defaultFontName.ifEmpty { systemFonts.firstOrNull() ?: "" }
    var showFontPicker by remember { mutableStateOf(false) }

    Column {
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = titleOverride ?: stringResource(R.string.font_family),
            style = labelStyle,
            color = textColor,
            maxLines = 1,
        )
        Spacer(modifier = Modifier.height(4.dp))

        Box {
            Row(
                modifier =
                Modifier.testTag("FontFamilySelector")
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(8.dp))
                    .background(cardBackground)
                    .clickable {
                        showFontPicker = true
                        // 完整字体列表只在此刻真正要显示时才枚举（DESIGN 字体节）：
                        // 外观项本身只显示当前族，而 LazyColumn 项每次滚进滚出都会重组，
                        // 挂在组合上等于每滚一次就重跑一轮 JNI 列举 + 目录注册。
                        onFontsVisible()
                    }
                    .padding(horizontal = 12.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = if (selectedFamily.isEmpty()) displayName else selectedFamily,
                    color = textColor,
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = stringResource(R.string.change),
                    color = accentColor,
                    style = MaterialTheme.typography.bodySmall,
                )
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
                )
            }
        }
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
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.select_font_family), color = textColor) },
        text = {
            LazyColumn {
                items(fonts) { font ->
                    Row(
                        modifier =
                        Modifier.fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .clickable { onFamilySelected(font) }
                            .background(
                                if (selectedFamily == font) {
                                    accentColor.copy(alpha = 0.2f)
                                } else {
                                    Color.Transparent
                                },
                            )
                            .padding(horizontal = 12.dp, vertical = 10.dp),
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
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel), color = textColor) }
        },
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
                ThemeMode.DAY to stringResource(R.string.day),
                ThemeMode.NIGHT to stringResource(R.string.night),
                ThemeMode.FOLLOW_SYSTEM to stringResource(R.string.follow_system),
            )
        val spacing = if (isSmall) 4.dp else 6.dp
        if (isSmall) {
            Column(verticalArrangement = Arrangement.spacedBy(spacing)) {
                buttons.forEach { (mode, label) ->
                    val isSelected = selectedMode == mode
                    Box(
                        modifier =
                        Modifier.testTag("AppTheme_$mode")
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
                        Modifier.testTag("AppTheme_$mode")
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
internal fun TerminalThemeModeSelector(
    selectedMode: String,
    onModeSelected: (String) -> Unit,
    textColor: Color,
    cardBackground: Color,
    accentColor: Color,
) {
    val isFollowSystem = selectedMode != ThemeMode.FIXED
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
                onModeSelected(if (checked) ThemeMode.FOLLOW_SYSTEM else ThemeMode.FIXED)
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
        Modifier.testTag("theme_preview_${theme.name}")
            .width(previewWidth)
            .clickable(onClick = onClick),
    ) {
        Box(
            modifier =
            Modifier.fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                .background(
                    if (isSelected) theme.background else theme.background.copy(alpha = 0.7f),
                )
                .padding(padding),
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
        Text(
            text = stringResource(R.string.bootstrap_presets),
            style = MaterialTheme.typography.bodyMedium,
            color = textColor,
        )
        Spacer(modifier = Modifier.height(4.dp))

        val arch = terminal.emulator.detectArchFromAbi()
        val termuxUrl = TermuxBootstrap.downloadUrl(arch)
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
        stringResource(
            R.string.bootstrap_extracting,
            pct,
            progress.entriesExtracted,
            progress.totalEntries,
        )
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

@Composable
private fun FontInfoSection(fontInfo: FontInfo, pixelPerSp: Float, textColor: Color, secondaryText: Color) {
    Column(modifier = Modifier.testTag("FontInfoSection")) {
        if (fontInfo.activeName.isNotEmpty()) {
            Text(
                text = stringResource(R.string.font_info_active, fontInfo.activeName),
                style = MaterialTheme.typography.bodyMedium,
                color = textColor,
            )
        }
        // CJK 状态行与缺失警告同出一处（见 CjkFallbackStatusLine）：
        // 同一事实发两次会让人以为是两个问题。
        CjkFallbackStatusLine(fontInfo = fontInfo, secondaryText = secondaryText)
        if (fontInfo.otherFamilies.isNotEmpty()) {
            Text(
                text = stringResource(R.string.font_info_other_families, fontInfo.otherFamilies.joinToString(", ")),
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
            )
        }
        if (fontInfo.cellWidthPx > 0f && fontInfo.cellHeightPx > 0f) {
            Text(
                text =
                stringResource(R.string.font_info_cell, fontInfo.cellWidthPx, fontInfo.cellHeightPx),
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
            )
        }
        if (fontInfo.fontSizeSp > 0f) {
            val sizePx = fontSpToPx(fontInfo.fontSizeSp, pixelPerSp)
            Text(
                text = stringResource(R.string.font_info_size_sp, fontInfo.fontSizeSp, sizePx),
                style = MaterialTheme.typography.bodySmall,
                color = secondaryText,
            )
        }
    }
}
