@file:Suppress("LocalContextGetResourceValueCall")

package io.torvox.ui

import android.util.Log
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import io.torvox.R
import io.torvox.TerminalViewModel
import io.torvox.ui.theme.BuiltInThemes
import kotlinx.coroutines.launch
import kotlin.math.max
import kotlin.math.min

private const val FONT_SIZE_MIN = 8f
private const val FONT_SIZE_MAX = 48f
private const val SEARCH_MATCH_ALPHA = 0.25f

/**
 * Consolidated search state for text search within the terminal.
 * Replaces 6 independent remember variables.
 */
private data class SearchState(
    val query: String = "",
    val results: List<SearchResult> = emptyList(),
    val currentIndex: Int = 0,
    val caseSensitive: Boolean = false,
    val fuzzyMatch: Boolean = false,
    val previousQuery: String = "",
    val highlightsActive: Boolean = false,
) {
    val hasResults: Boolean get() = results.isNotEmpty()
    val resultCount: Int get() = results.size
    val currentMatch: SearchResult? get() = results.getOrNull(currentIndex)
}

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Suppress("DEPRECATION", "CyclomaticComplexMethod", "LongMethod")
@Composable
fun TerminalScreen(
    modifier: Modifier = Modifier,
    viewModel: TerminalViewModel = hiltViewModel(),
    onSettings: () -> Unit = {},
    isOverlayVisible: Boolean = false,
) {
    val state by viewModel.state.collectAsState()
    val viewModelThemeMode by viewModel.themeMode.collectAsState()
    val viewModelThemeName by viewModel.themeName.collectAsState()
    val viewModelDayThemeName by viewModel.dayThemeName.collectAsState()
    val viewModelNightThemeName by viewModel.nightThemeName.collectAsState()
    val useNerdFontGlyphs by viewModel.useNerdFontGlyphs.collectAsState()
    val runtimeState by viewModel.runtime.state.collectAsState()
    val isSettingsDark =
        when (viewModel.appThemeMode.collectAsState().value) {
            "night" -> true
            "day" -> false
            else -> androidx.compose.foundation.isSystemInDarkTheme()
        }
    val terminalBg =
        when (viewModelThemeMode) {
            "fixed" -> {
                BuiltInThemes.byName(viewModelThemeName).background
            }

            "day" -> {
                BuiltInThemes.byName(viewModelDayThemeName).background
            }

            "night" -> {
                BuiltInThemes.byName(viewModelNightThemeName).background
            }

            "follow_system" -> {
                if (isSettingsDark) {
                    BuiltInThemes.byName(viewModelNightThemeName).background
                } else {
                    BuiltInThemes.byName(viewModelDayThemeName).background
                }
            }

            else -> {
                if (isSettingsDark) {
                    BuiltInThemes.byName(viewModelNightThemeName).background
                } else {
                    BuiltInThemes.byName(viewModelDayThemeName).background
                }
            }
        }
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    var searchJob by remember { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    val context = androidx.compose.ui.platform.LocalContext.current
    val hostView = androidx.compose.ui.platform.LocalView.current
    var showTextSearch by remember { mutableStateOf(false) }
    var composeScrollOffset by remember { mutableIntStateOf(0) }
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    val view = LocalView.current
    val surfaceRef = remember { mutableStateOf<TerminalSurface?>(null) }
    DisposableEffect(lifecycleOwner) {
        val observer =
            LifecycleEventObserver { _, event ->
                if (event == Lifecycle.Event.ON_PAUSE) {
                    (hostView as? TerminalSurface)?.finishComposing()
                    scope.launch { viewModel.runtime.saveSession() }
                    val inputMethodManager =
                        context.getSystemService(
                            android.content.Context.INPUT_METHOD_SERVICE,
                        ) as android.view.inputmethod.InputMethodManager
                    inputMethodManager.hideSoftInputFromWindow(hostView.windowToken, 0)
                } else if (event == Lifecycle.Event.ON_RESUME) {
                    val surfaceView = hostView
                    (surfaceView as? TerminalSurface)?.postDelayedUnpause(200L)
                    hostView.requestFocus()
                }
            }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(state.sessions.size) {
        val count = state.sessions.size
        if (count > 0) {
            hostView.announceForAccessibility(
                context.getString(R.string.sessions_accessible, count),
            )
        }
    }

    LaunchedEffect(state.title) {
        val title = state.title
        if (title.isNotEmpty() && title != context.getString(R.string.terminal_title) && title != context.getString(R.string.terminal)) {
            hostView.announceForAccessibility(
                context.getString(R.string.title_changed, title),
            )
        }
    }

    BackHandler(enabled = drawerState.isOpen) {
        scope.launch { drawerState.close() }
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            ModalDrawerSheet(
                drawerContainerColor = MaterialTheme.colorScheme.surface,
            ) {
                SessionDrawer(
                    viewModel = viewModel,
                    onSettings = {
                        scope.launch { drawerState.close() }
                        onSettings()
                    },
                    onSearch = {
                        showTextSearch = true
                        surfaceRef.value?.searchActive = true
                    },
                    onClose = {
                        scope.launch { drawerState.close() }
                    },
                )
            }
        },
        modifier = modifier,
    ) {
        val snackbarHostState = remember { SnackbarHostState() }

        Box(
            modifier =
                Modifier
                    .fillMaxSize()
                    .testTag("TerminalScreen")
                    .background(terminalBg)
                    .statusBarsPadding(),
        ) {
            LaunchedEffect(drawerState.isOpen) {
                surfaceRef.value?.drawerOpen = drawerState.isOpen
            }
            val selection = state.selection
            val selectionActive = selection.active && selection.start != null && selection.end != null

            // Consolidated text search state
            var searchState by remember { mutableStateOf(SearchState()) }

            LaunchedEffect(state.activeSessionId) {
                showTextSearch = false
                searchState = SearchState()
                surfaceRef.value?.searchActive = false
            }

            fun scrollToMatchIfNeeded(match: SearchResult) {
                val surface = surfaceRef.value ?: return
                val visibleRows = surface.getRows()
                val scrollbackLen = surface.getMaxScrollOffset()
                val scrollOffset = surface.getScrollOffset()
                val firstVisibleRow = scrollbackLen - scrollOffset
                val lastVisibleRow = firstVisibleRow + visibleRows - 1
                if (match.lineIndex !in firstVisibleRow..lastVisibleRow) {
                    val centeredRow = (match.lineIndex - visibleRows / 2).coerceAtLeast(0)
                    surface.scrollToRow(centeredRow)
                }
            }

            suspend fun performSearch() {
                val query = searchState.query
                if (query.isEmpty()) {
                    searchState = searchState.copy(results = emptyList())
                    return
                }
                val bridge =
                    viewModel.runtime.bridge() ?: run {
                        searchState = searchState.copy(results = emptyList())
                        return
                    }
                val effectiveCaseSensitive = searchState.caseSensitive || (query.any { it.isUpperCase() } && !searchState.fuzzyMatch)
                val matches =
                    bridge.searchAllInScrollback(query, effectiveCaseSensitive, searchState.fuzzyMatch) ?: run {
                        searchState = searchState.copy(results = emptyList())
                        return
                    }
                val results =
                    matches.map { (row, startCol, endCol) ->
                        SearchResult(lineIndex = row, startIndex = startCol, endIndex = endCol)
                    }
                val isNarrowing =
                    query.isNotEmpty() && searchState.previousQuery.isNotEmpty() &&
                        query.length < searchState.previousQuery.length && searchState.previousQuery.startsWith(query)
                val newIndex =
                    if (isNarrowing && results.isNotEmpty()) {
                        searchState.currentIndex.coerceIn(0, results.size - 1)
                    } else {
                        0
                    }
                searchState =
                    searchState.copy(
                        results = results,
                        currentIndex = newIndex,
                        previousQuery = query,
                    )
                if (results.isNotEmpty()) {
                    scrollToMatchIfNeeded(results[newIndex])
                }
            }

            LaunchedEffect(searchState.caseSensitive, searchState.fuzzyMatch) {
                if (searchState.query.isNotEmpty()) {
                    searchJob?.cancel()
                    searchJob = scope.launch { performSearch() }
                }
            }

            val resolvedTerminalTheme =
                BuiltInThemes.byName(
                    when (viewModelThemeMode) {
                        "fixed" -> viewModelThemeName
                        "day" -> viewModelDayThemeName
                        "night" -> viewModelNightThemeName
                        "follow_system" -> if (isSettingsDark) viewModelNightThemeName else viewModelDayThemeName
                        else -> if (isSettingsDark) viewModelNightThemeName else viewModelDayThemeName
                    },
                )

            Box(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .testTag("TerminalContent"),
            ) {
                AndroidView(
                    factory = { context ->
                        io.torvox.ui
                            .TerminalSurface(context)
                            .apply { setTag("TerminalSurfaceView") }
                            .also { surface ->
                                surfaceRef.value = surface
                                surface.onScrollChanged = { offset ->
                                    composeScrollOffset = offset
                                    viewModel.runtime.setScrollOffset(offset.toUInt())
                                    viewModel.runtime.bridge()?.render()
                                }
                            }.apply {
                                initialize(viewModel)
                                setDimensions(runtimeState.rows, runtimeState.cols)
                                onSwipeLeft = {
                                    viewModel.writeToPty("\u001b".toByteArray())
                                }
                                onSwipeRight = {
                                    viewModel.writeToPty("\t".toByteArray())
                                }
                                onCopyRequested = { text ->
                                    scope.launch {
                                        snackbarHostState.currentSnackbarData?.dismiss()
                                        snackbarHostState.showSnackbar(
                                            message = context.getString(R.string.copied_chars, text.length),
                                            duration = SnackbarDuration.Short,
                                        )
                                    }
                                }
                                onPasteRequested = {
                                    val count = viewModel.pasteFromClipboard()
                                    if (count > 0) {
                                        scope.launch {
                                            snackbarHostState.currentSnackbarData?.dismiss()
                                            snackbarHostState.showSnackbar(
                                                message = context.getString(R.string.pasted_chars, count),
                                                duration = SnackbarDuration.Short,
                                            )
                                        }
                                    }
                                }
                                onZoomChanged = { increase ->
                                    val current = viewModel.fontSize.value
                                    val step = if (increase) 2f else -2f
                                    val newSize = (current + step).coerceIn(FONT_SIZE_MIN, FONT_SIZE_MAX)
                                    viewModel.setFontSize(newSize)
                                }
                                post {
                                    requestFocus()
                                }
                            }
                    },
                    update = { surface ->
                        surface.touchEnabled = !isOverlayVisible
                        if (runtimeState.rows > 0 && runtimeState.cols > 0) {
                            surface.setDimensions(runtimeState.rows, runtimeState.cols)
                        }
                        surface.requestLayout()
                    },
                    modifier = Modifier.fillMaxSize(),
                )

                if (selectionActive) {
                    val selStart = selection.start
                    val selEnd = selection.end
                    val loRow = min(selStart.row, selEnd.row)
                    val hiRow = max(selStart.row, selEnd.row)
                    val loCol: Int
                    val hiCol: Int
                    if (selStart.row <= selEnd.row) {
                        loCol = selStart.col
                        hiCol = selEnd.col
                    } else {
                        loCol = selEnd.col
                        hiCol = selStart.col
                    }
                    val themeAccent = if (state.selectionAccent != 0) Color(state.selectionAccent) else resolvedTerminalTheme.foreground

                    fun colorToArgb(color: androidx.compose.ui.graphics.Color): Int =
                        android.graphics.Color.argb(
                            (color.alpha * 255).toInt(),
                            (color.red * 255).toInt(),
                            (color.green * 255).toInt(),
                            (color.blue * 255).toInt(),
                        )
                    val themeAccentArgb = colorToArgb(themeAccent)

                    if (selection.dragging) {
                        LaunchedEffect(true) {
                            surfaceRef.value?.hideSelectionHandles()
                        }
                    } else {
                        LaunchedEffect(loRow, loCol, hiRow, hiCol, themeAccentArgb) {
                            surfaceRef.value?.showSelectionHandles(loRow, loCol, hiRow, hiCol, themeAccentArgb)
                        }
                    }
                } else {
                    LaunchedEffect(selectionActive) {
                        surfaceRef.value?.hideSelectionHandles()
                    }
                }

                val pasteReq = state.pastePopupRequest
                if (!selectionActive && pasteReq != null) {
                    val surface = surfaceRef.value
                    if (surface != null) {
                        PasteChipOverlay(
                            row = pasteReq.row,
                            col = pasteReq.col,
                            cellWidth = surface.cellWidth,
                            cellHeight = surface.cellHeight,
                            scrollOffset = surface.getScrollOffset(),
                            onPaste = {
                                viewModel.pasteFromClipboard()
                                viewModel.consumePastePopupRequest()
                            },
                            accentColor = Color(state.selectionAccent),
                            backgroundColor = Color(state.selectionBg),
                        )
                    }
                }

                if (showTextSearch && searchState.hasResults) {
                    val surface = surfaceRef.value
                    if (surface != null) {
                        val rows = surface.getRows()
                        val scrollbackCount = surface.getMaxScrollOffset()
                        val scrollOffset = surface.getScrollOffset()
                        val themeForeground = resolvedTerminalTheme.foreground
                        val themeSelectionBg = resolvedTerminalTheme.selectionBg

                        val writer = io.torvox.bridge.WireWriter()
                        writer.writeI32(searchState.resultCount)
                        for ((index, match) in searchState.results.withIndex()) {
                            val gridRow = match.lineIndex - scrollbackCount + scrollOffset
                            if (gridRow < 0 || gridRow >= rows) continue
                            val isCurrent = index == searchState.currentIndex
                            writer.writeI32(gridRow)
                            writer.writeI32(match.startIndex)
                            writer.writeI32(match.endIndex.coerceAtLeast(match.startIndex + 1))
                            if (isCurrent) {
                                // Current match: use foreground color at moderate opacity
                                // so the text appears "lit up" — distinctly different from
                                // the subtle selectionBg overlay of other matches.
                                writer.writeByte((themeForeground.red * 255).toInt().toByte())
                                writer.writeByte((themeForeground.green * 255).toInt().toByte())
                                writer.writeByte((themeForeground.blue * 255).toInt().toByte())
                                writer.writeByte(160.toByte()) // ~63% opacity
                            } else {
                                // Other matches: selection_bg semi-transparent overlay
                                writer.writeByte((themeSelectionBg.red * 255).toInt().toByte())
                                writer.writeByte((themeSelectionBg.green * 255).toInt().toByte())
                                writer.writeByte((themeSelectionBg.blue * 255).toInt().toByte())
                                writer.writeByte((SEARCH_MATCH_ALPHA * 255).toInt().toByte()) // 25%
                            }
                        }
                        val highlightBytes = writer.toByteArray()
                        // Single call: surface.setSearchHighlights internally calls bridge.setSearchHighlights + bridge.render
                        surface.setSearchHighlights(highlightBytes)
                        searchState = searchState.copy(highlightsActive = true)
                    }
                } else if (searchState.highlightsActive) {
                    surfaceRef.value?.clearSearchHighlights()
                    searchState = searchState.copy(highlightsActive = false)
                }
            }

            Box(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .imePadding(),
                contentAlignment = Alignment.BottomCenter,
            ) {
                if (showTextSearch) {
                    TextSearchBar(
                        query = searchState.query,
                        onQueryChange = { query ->
                            searchState = searchState.copy(query = query)
                            searchJob?.cancel()
                            searchJob =
                                scope.launch { performSearch() }
                        },
                        resultCount = searchState.resultCount,
                        currentResultIndex = searchState.currentIndex,
                        onPrevious = {
                            if (searchState.hasResults) {
                                val newIndex =
                                    if (searchState.currentIndex > 0) {
                                        searchState.currentIndex - 1
                                    } else {
                                        searchState.resultCount - 1
                                    }
                                val match = searchState.results[newIndex]
                                scrollToMatchIfNeeded(match)
                                Log.d("TerminalScreen", "Search prev: match row=${match.lineIndex}")
                                searchState = searchState.copy(currentIndex = newIndex)
                            }
                        },
                        onNext = {
                            if (searchState.hasResults) {
                                val newIndex =
                                    if (searchState.currentIndex < searchState.resultCount - 1) {
                                        searchState.currentIndex + 1
                                    } else {
                                        0
                                    }
                                val match = searchState.results[newIndex]
                                scrollToMatchIfNeeded(match)
                                Log.d("TerminalScreen", "Search next: match row=${match.lineIndex}")
                                searchState = searchState.copy(currentIndex = newIndex)
                            }
                        },
                        onClose = {
                            showTextSearch = false
                            searchState = SearchState()
                            surfaceRef.value?.searchActive = false
                            surfaceRef.value?.clearSearchHighlights()
                        },
                        caseSensitive = searchState.caseSensitive,
                        onCaseSensitiveToggle = { searchState = searchState.copy(caseSensitive = it) },
                        fuzzyMatch = searchState.fuzzyMatch,
                        onFuzzyMatchToggle = { searchState = searchState.copy(fuzzyMatch = it) },
                        autoCaseSensitive = !searchState.caseSensitive && searchState.query.any { it.isUpperCase() },
                        modifier = Modifier.testTag("TextSearchBar"),
                    )
                } else {
                    val barMode =
                        if (selectionActive) {
                            io.torvox.ui.ModifierBarMode.SelectionActions
                        } else {
                            io.torvox.ui.ModifierBarMode.Normal
                        }
                    val clipboard =
                        context.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                            as android.content.ClipboardManager
                    val hasClipboard = clipboard.hasPrimaryClip()

                    ModifierBar(
                        modifier =
                            Modifier
                                .testTag("ModifierBar"),
                        onKeyClick = { data ->
                            viewModel.writeToPty(data.toByteArray())
                        },
                        onDrawerClick = {
                            scope.launch { drawerState.open() }
                        },
                        onScrollClick = {
                            viewModel.toggleScrollMode()
                        },
                        ctrlState = state.ctrlState,
                        altState = state.altState,
                        onToggleCtrl = {
                            viewModel.cycleCtrlState()
                        },
                        onToggleAlt = {
                            viewModel.cycleAltState()
                        },
                        textColor = resolvedTerminalTheme.foreground,
                        backgroundColor = resolvedTerminalTheme.background,
                        useNerdFontGlyphs = useNerdFontGlyphs,
                        toolbarLayout = rememberToolbarLayout(),
                        barMode = barMode,
                        onCopy =
                            if (selectionActive) {
                                {
                                    viewModel.copySelectionToClipboard()
                                    viewModel.clearSelection()
                                }
                            } else {
                                null
                            },
                        onSelectAll =
                            if (selectionActive) {
                                { viewModel.selectAll() }
                            } else {
                                null
                            },
                        onPaste =
                            if (selectionActive && hasClipboard) {
                                { viewModel.pasteFromClipboard() }
                            } else {
                                null
                            },
                        onShare =
                            if (selectionActive) {
                                { viewModel.shareSelection() }
                            } else {
                                null
                            },
                        onDismiss =
                            if (selectionActive) {
                                { viewModel.clearSelection() }
                            } else {
                                null
                            },
                    )
                }
            }
        }
    }
}
