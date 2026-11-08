package terminal.emulator.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.collections.immutable.ImmutableList
import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toImmutableList
import terminal.emulator.R
import terminal.emulator.bridge.CjkState
import terminal.emulator.bridge.FontInfo
import terminal.emulator.bridge.fontSpToPx
import terminal.emulator.settings.SettingsRepository

/** 外观设置：字号调节、字体族选择与字体实际信息。 */
@Composable
internal fun AppearanceSectionContent(
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
            SettingsRepository.clampFontSize(fontSize, spToPxScale, screenWidthDp),
        )
    }
    // 区间随屏宽、系统字体缩放变化，而 Activity 自行处理 orientation/screenSize/fontScale
    // （见清单），组合不会重建，`rememberSaveable` 里的旧值于是可能落在新上界之外。
    // 不就地收敛时：拇指被 Slider 钳在轨末、读数仍打印旧值、实际渲染由原生钳到上界，
    // 三者互不相同，而首次触摸会把上界写回。区间一收紧就同步收敛。
    LaunchedEffect(fontSizeUpperBound) {
        sliderFontSize =
            SettingsRepository.clampFontSize(sliderFontSize, spToPxScale, screenWidthDp)
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
            // 原生尚未上报度量时用它占位：必须与调节条同值，否则占位行会报出
            // 一个实际并未渲染的字号（越界存量值的情形）。
            fontSize = sliderFontSize,
            spToPxScale = spToPxScale,
            textColor = textColor,
            secondaryText = secondaryText,
        )
        Spacer(modifier = Modifier.height(12.dp))
    }
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
            text = stringResource(R.string.font_family),
            style = labelStyle,
            color = textColor,
            maxLines = 1,
        )
        Spacer(modifier = Modifier.height(4.dp))

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
                        // 完整字体列表只在此刻真正要显示时才枚举（DESIGN 字体节）：
                        // 外观项本身只显示当前族，而 LazyColumn 项每次滚进滚出都会重组，
                        // 挂在组合上等于每滚一次就重跑一轮 JNI 列举 + 目录注册。
                        onFontsVisible()
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
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel), color = textColor) }
        },
        containerColor = cardBackground,
    )
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
