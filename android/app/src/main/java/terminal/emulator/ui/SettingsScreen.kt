package terminal.emulator.ui

import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.flow.collectLatest
import terminal.emulator.R
import terminal.emulator.TerminalViewModel
import terminal.emulator.installer.BootstrapProgress

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
    // 应用内瞬时提示：订阅 ViewModel 持有的通道并以 Snackbar 呈现。
    // 设置页只在覆盖层打开时在场，故与终端页不会同时呈现同一条消息。
    val snackbarHostState = remember { SnackbarHostState() }
    LaunchedEffect(Unit) {
        viewModel.transientMessages.flow.collectLatest { text ->
            // 新提示到达即撤下当前那条：连切字体时不会排出一列旧反馈。
            snackbarHostState.currentSnackbarData?.dismiss()
            snackbarHostState.showSnackbar(text)
        }
    }
    BackHandler(enabled = true) { onBack() }
    // 系统栏图标明暗按**设置页自己的底色**判定：设置页是浅色面，若沿用终端页
    // （深底）的判定，状态栏时钟会是浅色压浅底，几乎看不清。
    SystemBarsFollowBackground(LocalView.current, backgroundColor)
    Surface(
        modifier =
        modifier
            .fillMaxSize()
            .testTag("SettingsScreen")
            // 消费设置背景上的点击，使其不穿透到下层的 TerminalScreen。
            // 必须用 pointerInput 而非 clickable：clickable 会强制
            // mergeDescendants 语义，从合并树中吞掉所有后代的 testTag，
            // 使整个界面对 Compose UI 测试与 TalkBack 不可读。
            .pointerInput(Unit) { detectTapGestures(onTap = {}) },
        color = backgroundColor,
    ) {
        Box(modifier = Modifier.fillMaxSize()) {
            Column(modifier = Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
                // 固定表头：SettingsHeader 必须在 LazyColumn 滚动时保持可见，
                // 否则返回按钮会滚出可达范围
                // （maestro 的 open-settings 流程在向下滚动后找不到 SettingsBackButton）。
                SettingsHeader(onBack, textColor, isSmallScreen)
                LazyColumn(
                    modifier =
                    Modifier
                        .fillMaxSize()
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
                            cardBackground = cardBackground,
                            sectionTitleColor = sectionTitleColor,
                        )
                    }
                    item {
                        TerminalConfigSection(
                            selectedShell = settings.shell,
                            onShellSaved = { viewModel.setShell(it) },
                            textColor = textColor,
                            accentColor = accentColor,
                            cardBackground = cardBackground,
                            sectionTitleColor = sectionTitleColor,
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
                        )
                    }
                    item {
                        LogLevelSection(
                            logLevel = settings.logLevel,
                            onLogLevelSelected = { viewModel.setLogLevel(it) },
                            textColor = textColor,
                            cardBackground = cardBackground,
                            backgroundColor = backgroundColor,
                            sectionTitleColor = sectionTitleColor,
                        )
                    }
                    item {
                        ClearAppDataSectionItem(
                            onClearAppData = { viewModel.clearAppData() },
                            textColor = textColor,
                            cardBackground = cardBackground,
                            sectionTitleColor = sectionTitleColor,
                        )
                    }
                    item { Spacer(modifier = Modifier.height(24.dp)) }
                }
            }
            // 应用内瞬时提示：贴在内容底部，避开系统导航条。
            SnackbarHost(
                hostState = snackbarHostState,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .navigationBarsPadding(),
            )
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
