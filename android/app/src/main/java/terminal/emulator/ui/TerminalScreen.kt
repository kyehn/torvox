@file:Suppress("LocalContextGetResourceValueCall")

package terminal.emulator.ui

import android.graphics.RectF
import android.util.Log
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.VisibleForTesting
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import terminal.emulator.R
import terminal.emulator.SelectionAnchor
import terminal.emulator.SelectionState
import terminal.emulator.TerminalViewModel
import terminal.emulator.input.ModifierState
import terminal.emulator.input.next
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

private const val FONT_SIZE_MIN = 8f
private const val FONT_SIZE_MAX = 48f
private const val SEARCH_MATCH_ALPHA = 0.25f

/** Selection drag-handle drawable width (Material `text_select_handle_*`, 48dp). */
private const val SELECTION_HANDLE_WIDTH_DP = 48f

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
    val state by viewModel.state.collectAsStateWithLifecycle()
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    // Height of the terminal content Box (below: the context menu
    // positions itself with an offset relative to this Box — the menu must
    // be placed against the BOX height, not the full screen height, or a
    // Select-All selection (selBottom == box height)
    // pushes the menu off-screen below the ModifierBar).
    var terminalBoxSize by remember { mutableStateOf(IntSize(0, 0)) }
    val viewModelThemeMode = settings.themeMode
    val viewModelThemeName = settings.themeName
    val viewModelDayThemeName = settings.dayThemeName
    val viewModelNightThemeName = settings.nightThemeName
    val useNerdFontGlyphs = settings.useNerdFontGlyphs
    val runtimeState by viewModel.runtime.state.collectAsStateWithLifecycle()
    val isSettingsDark = resolveAppDarkMode(settings.appThemeMode, androidx.compose.foundation.isSystemInDarkTheme())
    val resolvedTerminalTheme =
        BuiltInThemes.byName(
            resolveTerminalThemeName(
                mode = viewModelThemeMode,
                fixedName = viewModelThemeName,
                dayName = viewModelDayThemeName,
                nightName = viewModelNightThemeName,
                isDark = isSettingsDark,
            ),
        )
    val terminalBg = resolvedTerminalTheme.background
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    var searchJob by remember { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    val context = androidx.compose.ui.platform.LocalContext.current
    val hostView = androidx.compose.ui.platform.LocalView.current
    var showTextSearch by remember { mutableStateOf(false) }
    // Sticky FN layer (ModifierBar): when Locked the bar shows the F1-F12
    // second layer; tapping an F-key or FN again exits it.
    var fnState by remember { mutableStateOf(ModifierState.Off) }
    val onToggleFn: () -> Unit = { fnState = fnState.next() }
    var composeScrollOffset by remember { mutableIntStateOf(0) }
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    val view = LocalView.current
    val surfaceRef = remember { mutableStateOf<TerminalSurface?>(null) }
    DisposableEffect(lifecycleOwner) {
        val observer =
            LifecycleEventObserver { _, event ->
                if (event == Lifecycle.Event.ON_PAUSE) {
                    // LocalView.current at composition top level is the
                    // AndroidComposeView, not the TerminalSurface; use the
                    // surfaceRef captured from the AndroidView factory.
                    surfaceRef.value?.finishComposing()
                    val inputMethodManager =
                        context.getSystemService(
                            android.content.Context.INPUT_METHOD_SERVICE,
                        ) as android.view.inputmethod.InputMethodManager
                    inputMethodManager.hideSoftInputFromWindow(view.windowToken, 0)
                } else if (event == Lifecycle.Event.ON_RESUME) {
                    surfaceRef.value?.postDelayedUnpause(200L)
                    view.requestFocus()
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

    LaunchedEffect(state.selection.active, state.selection.start, state.selection.end) {
        val sel = state.selection
        if (sel.active && sel.hasSelection) {
            val text = sel.selectedText
            if (text.isNotEmpty()) {
                val preview = if (text.length > 100) text.take(100) + "..." else text
                hostView.announceForAccessibility(
                    context.getString(R.string.selection_accessible, preview),
                )
            }
        }
    }

    // Selection accessibility is announced from the search effect below (after searchState is declared).

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

        // ── Paste confirmation dialog ─────────────────────────────────
        val pasteConfirmation by viewModel.pasteConfirmation.collectAsStateWithLifecycle()
        if (pasteConfirmation.visible) {
            AlertDialog(
                onDismissRequest = { viewModel.cancelPaste() },
                title = { Text(stringResource(R.string.paste_confirm_title)) },
                text = {
                    Column {
                        Text(
                            stringResource(
                                R.string.paste_confirm_detail,
                                pasteConfirmation.lineCount,
                                pasteConfirmation.charCount,
                            ),
                        )
                        Spacer(Modifier.height(8.dp))
                        Text(
                            text = pasteConfirmation.text.take(200) +
                                if (pasteConfirmation.text.length > 200) "..." else "",
                            style = MaterialTheme.typography.bodySmall,
                            maxLines = 10,
                        )
                    }
                },
                confirmButton = {
                    TextButton(onClick = { viewModel.confirmPaste() }) {
                        Text(stringResource(R.string.paste_confirm_yes))
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.cancelPaste() }) {
                        Text(stringResource(R.string.paste_confirm_no))
                    }
                },
            )
        }

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

            val exportLauncher = rememberLauncherForActivityResult(
                contract = ActivityResultContracts.CreateDocument("text/plain"),
            ) { uri ->
                uri?.let { viewModel.exportTerminalOutput(it) }
            }

            // Round-219: with the legacy View.startActionMode(Callback) the
            // system does NOT intercept BACK to finish the ActionMode (only
            // TYPE_FLOATING modes do); BACK therefore fell through to the
            // Activity and exited the app while a selection was active
            // (emulator-verified). Intercept BACK while a selection is
            // active and end it first — ghostty-android onDestroyActionMode
            // clears the selection; Termux's first BACK dismisses the
            // toolbar. The drawer's own BackHandler takes priority (it is
            // registered earlier and LIFO runs ours first, so gate ours on
            // the drawer being closed: BACK with the drawer open closes the
            // drawer and must NOT clear the selection).
            BackHandler(enabled = selectionActive && !drawerState.isOpen) {
                viewModel.clearSelection()
                surfaceRef.value?.hideSelectionMenu()
            }

            // Consolidated text search state
            var searchState by remember { mutableStateOf(SearchState()) }

            LaunchedEffect(state.activeSessionId) {
                showTextSearch = false
                searchState = SearchState()
                surfaceRef.value?.searchActive = false
                // Round-209 P2-8: the surface keeps a private scrollOffset
                // used for selection coordinate math; it must follow the
                // session's own offset on switch, otherwise the first
                // gesture after a switch computes wrong grid rows.
                surfaceRef.value?.resetScrollOffset()
            }

            // Announce search result count changes for TalkBack.
            LaunchedEffect(searchState.resultCount, searchState.currentIndex, searchState.query) {
                if (searchState.query.isNotEmpty()) {
                    if (searchState.resultCount > 0) {
                        hostView.announceForAccessibility(
                            context.getString(
                                R.string.search_result_accessible,
                                searchState.currentIndex + 1,
                                searchState.resultCount,
                            ),
                        )
                    } else {
                        hostView.announceForAccessibility(
                            context.getString(R.string.search_no_results_accessible),
                        )
                    }
                }
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
                surfaceRef.value?.finishComposing()
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
                // narrowing_down: GNOME Console (kgx) uses g_strrstr() to check
                // if the last_search string *contains* the current query — not just
                // prefix matching. This allows narrowing to work when the user
                // deletes characters from the middle or end of a search string,
                // not only when they remove the last characters.
                // See: kgx-tab.c:191-250 (search_changed callback).
                val isNarrowing = SearchResult.isNarrowingDown(query, searchState.previousQuery)
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

            Column(
                modifier =
                Modifier
                    .fillMaxSize()
                    .testTag("TerminalContent")
                    .navigationBarsPadding(),
            ) {
                // Terminal content area — stays full height behind IME
                Box(
                    modifier =
                    Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .onSizeChanged { terminalBoxSize = it },
                ) {
                    AndroidView(
                        factory = { context ->
                            terminal.emulator.ui
                                .TerminalSurface(context)
                                .apply { setTag("TerminalSurfaceView") }
                                .also { surface ->
                                    surfaceRef.value = surface
                                    surface.onScrollChanged = { offset ->
                                        composeScrollOffset = offset
                                        viewModel.runtime.setScrollOffset(offset)
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
                                        val current = viewModel.settings.value.fontSize
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
                            // Only re-layout when the terminal grid dimensions
                            // actually change (resize / font change). The
                            // AndroidView update block runs on every
                            // recomposition of TerminalScreen, so an
                            // unconditional requestLayout() here forced a
                            // full View layout pass on every selection drag
                            // and scroll event — a key source of UI jank.
                            if (runtimeState.rows > 0 &&
                                runtimeState.cols > 0 &&
                                (
                                    surface.getRows() != runtimeState.rows ||
                                        surface.getCols() != runtimeState.cols
                                    )
                            ) {
                                surface.setDimensions(runtimeState.rows, runtimeState.cols)
                                surface.requestLayout()
                            }
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

                        fun colorToArgb(color: androidx.compose.ui.graphics.Color): Int = android.graphics.Color.argb(
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

                    // ── Selection context menu (PopupWindow) ──
                    // Round-217: the menu must be a PopupWindow, not a
                    // Compose overlay — the terminal is a SurfaceView whose
                    // surface punches a hole over the whole terminal area,
                    // hiding any in-window Compose drawing. PopupWindows are
                    // separate system windows that render above it.
                    val menuSurface = surfaceRef.value
                    if (menuSurface != null && selectionActive && !selection.dragging) {
                        val configuration = LocalConfiguration.current
                        val density = LocalDensity.current
                        val screenWidthPx = with(density) { configuration.screenWidthDp.dp.toPx() }
                        val boxHeightPx = terminalBoxSize.height.takeIf { it > 0 }?.toFloat()
                            ?: with(density) { configuration.screenHeightDp.dp.toPx() }
                        val pos =
                            computeMenuPosition(
                                start = selection.start,
                                end = selection.end,
                                cellWidth = menuSurface.cellWidth,
                                cellHeight = menuSurface.cellHeight,
                                scrollOffset = menuSurface.getScrollOffset(),
                                screenWidthPx = screenWidthPx,
                                screenHeightPx = boxHeightPx,
                                handleWidthPx = with(density) { SELECTION_HANDLE_WIDTH_DP.dp.toPx() },
                                pasteOnly = selection.pasteOnly,
                            )
                        val menuVisible = !selection.menuDismissed
                        if (menuVisible) {
                            val themeAccentArgb =
                                if (state.selectionAccent != 0) {
                                    Color(state.selectionAccent)
                                } else {
                                    resolvedTerminalTheme.foreground
                                }.let { color ->
                                    android.graphics.Color.argb(
                                        (color.alpha * 255).toInt(),
                                        (color.red * 255).toInt(),
                                        (color.green * 255).toInt(),
                                        (color.blue * 255).toInt(),
                                    )
                                }
                            // Round-216: the selection menu is the system
                            // ActionMode toolbar (like Termux): the system
                            // positions it, colors it from the theme, and
                            // handles item layout — no custom popup, no
                            // accent-colored text, no dividers.
                            LaunchedEffect(selection.pasteOnly, selection.menuDismissed) {
                                if (selection.menuDismissed) {
                                    menuSurface.hideSelectionMenu()
                                } else {
                                    menuSurface.showSelectionMenu(selection.pasteOnly)
                                }
                            }
                        } else {
                            LaunchedEffect(Unit) { menuSurface.hideSelectionMenu() }
                        }
                    } else {
                        LaunchedEffect(selectionActive) {
                            menuSurface?.hideSelectionMenu()
                        }
                    }

                    // Search-highlight painting must run as a side effect,
                    // not inline during composition: it calls into native
                    // (bridge.render) and mutates searchState, which would
                    // otherwise re-execute on every recomposition and
                    // trigger a state write during composition.
                    LaunchedEffect(
                        showTextSearch,
                        searchState.hasResults,
                        searchState.resultCount,
                        searchState.currentIndex,
                        searchState.results,
                        surfaceRef.value?.let { Triple(it.getRows(), it.getMaxScrollOffset(), it.getScrollOffset()) },
                        resolvedTerminalTheme.foreground,
                        resolvedTerminalTheme.selectionBg,
                    ) {
                        if (showTextSearch && searchState.hasResults) {
                            val surface = surfaceRef.value
                            if (surface != null) {
                                val rows = surface.getRows()
                                val scrollbackCount = surface.getMaxScrollOffset()
                                val scrollOffset = surface.getScrollOffset()
                                val themeForeground = resolvedTerminalTheme.foreground
                                val themeSelectionBg = resolvedTerminalTheme.selectionBg

                                val buf = java.io.ByteArrayOutputStream()
                                fun writeI32(v: Int) {
                                    buf.write(v and 0xFF)
                                    buf.write((v ushr 8) and 0xFF)
                                    buf.write((v ushr 16) and 0xFF)
                                    buf.write((v ushr 24) and 0xFF)
                                }
                                fun writeByte(v: Byte) {
                                    buf.write(v.toInt())
                                }
                                writeI32(searchState.resultCount)
                                for ((index, match) in searchState.results.withIndex()) {
                                    val gridRow = match.lineIndex - scrollbackCount + scrollOffset
                                    if (gridRow < 0 || gridRow >= rows) continue
                                    val isCurrent = index == searchState.currentIndex
                                    writeI32(gridRow)
                                    writeI32(match.startIndex)
                                    writeI32(match.endIndex.coerceAtLeast(match.startIndex + 1))
                                    if (isCurrent) {
                                        // Current match: use foreground color at moderate opacity
                                        // so the text appears "lit up" — distinctly different from
                                        // the subtle selectionBg overlay of other matches.
                                        writeByte((themeForeground.red * 255).toInt().toByte())
                                        writeByte((themeForeground.green * 255).toInt().toByte())
                                        writeByte((themeForeground.blue * 255).toInt().toByte())
                                        writeByte(160.toByte()) // ~63% opacity
                                    } else {
                                        // Other matches: selection_bg semi-transparent overlay
                                        writeByte((themeSelectionBg.red * 255).toInt().toByte())
                                        writeByte((themeSelectionBg.green * 255).toInt().toByte())
                                        writeByte((themeSelectionBg.blue * 255).toInt().toByte())
                                        writeByte((SEARCH_MATCH_ALPHA * 255).toInt().toByte()) // 25%
                                    }
                                }
                                val highlightBytes = buf.toByteArray()
                                // Single call: surface.setSearchHighlights internally calls bridge.setSearchHighlights + bridge.render
                                surface.setSearchHighlights(highlightBytes)
                                searchState = searchState.copy(highlightsActive = true)
                            }
                        } else if (searchState.highlightsActive) {
                            surfaceRef.value?.clearSearchHighlights()
                            searchState = searchState.copy(highlightsActive = false)
                        }
                    }
                }

                // END OF COLUMN — no bottom bar inside Column
            } // close Column

            // Floating overlay for bottom bar — sits above IME.
            // IME padding is animated manually (animateImePadding was
            // removed from foundation 1.11): WindowInsets.ime drives a
            // spring so the bar glides up when the keyboard opens
            // instead of jumping.
            val imeBottomPx = WindowInsets.ime.getBottom(LocalDensity.current)
            val animatedImeBottom by animateDpAsState(
                targetValue = with(LocalDensity.current) { imeBottomPx.toDp() },
                // Round-215: a fixed 220ms tween restarts on EVERY inset
                // change, and the IME reports its height in steps (measured
                // 380->850->881->883dp, ~270ms apart on this device). Each
                // restart + slow frame pacing made the bar jump in big
                // steps (2-3 animation frames total) — the reported "IME
                // animation jank". A spring does NOT restart on target
                // changes: velocity carries over, so stepped targets
                // produce one continuous smooth motion.
                animationSpec =
                spring(
                    dampingRatio = Spring.DampingRatioNoBouncy,
                    stiffness = Spring.StiffnessMediumLow,
                ),
                label = "imeBarOffset",
            )
            Box(
                modifier =
                Modifier
                    .fillMaxWidth()
                    .align(Alignment.BottomCenter)
                    // Round-216: the background must be applied BEFORE the
                    // IME padding so it covers the animated inset area —
                    // with background after padding, the spring-animated
                    // gap between the ModifierBar and the keyboard showed
                    // the window's black backdrop while the IME slid in/out.
                    .background(resolvedTerminalTheme.background)
                    .navigationBarsPadding()
                    .padding(bottom = animatedImeBottom)
                    .testTag("ModifierBarOverlay"),
            ) {
                // Bottom bar — below terminal, above IME
                if (showTextSearch) {
                    TextSearchBar(
                        query = searchState.query,
                        onQueryChange = { query ->
                            searchState = searchState.copy(query = query)
                            searchJob?.cancel()
                            // TextSearchBar already debounces input with its
                            // own 150ms Handler-based debounce; run the search
                            // immediately here.
                            searchJob = scope.launch { performSearch() }
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
                    // Keep the ModifierBar in Normal mode during selection:
                    // the floating selection context menu (near the selection,
                    // never covering it, rendered as a PopupWindow) already
                    // offers Copy/Select All/Paste. Switching the whole bar
                    // to SelectionActions rendered a SECOND, redundant menu
                    // at the bottom (round-214).
                    val barMode = terminal.emulator.ui.ModifierBarMode.Normal
                    val clipboardManager =
                        context.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                            as? android.content.ClipboardManager
                    var hasClipboard by remember { mutableStateOf(false) }

                    LaunchedEffect(Unit) {
                        hasClipboard = clipboardManager?.hasPrimaryClip() == true
                    }

                    DisposableEffect(context) {
                        val listener =
                            android.content.ClipboardManager.OnPrimaryClipChangedListener {
                                hasClipboard = clipboardManager?.hasPrimaryClip() == true
                            }
                        clipboardManager?.addPrimaryClipChangedListener(listener)
                        onDispose { clipboardManager?.removePrimaryClipChangedListener(listener) }
                    }

                    ModifierBar(
                        modifier =
                        Modifier
                            .testTag("ModifierBar")
                            .navigationBarsPadding(),
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
                        fnState = fnState,
                        onToggleFn = onToggleFn,
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
                        copyEnabled = selectionActive,
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
                        onExport =
                        if (selectionActive) {
                            exportLauncher.let { launcher ->
                                { launcher.launch("terminal_output.txt") }
                            }
                        } else {
                            null
                        },
                        onAnchorLeft =
                        if (selectionActive) {
                            { viewModel.moveSelectionAnchor(-1) }
                        } else {
                            null
                        },
                        onAnchorRight =
                        if (selectionActive) {
                            { viewModel.moveSelectionAnchor(1) }
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

@VisibleForTesting
internal data class MenuPosition(
    val menuX: Float,
    val menuY: Float,
    val menuW: Float,
    val menuH: Float,
    val selLeft: Float,
    val selTop: Float,
    val selRight: Float,
    val selBottom: Float,
    val flipAbove: Boolean,
    val coversSelection: Boolean,
)

@VisibleForTesting
internal fun computeMenuPosition(
    start: SelectionAnchor,
    end: SelectionAnchor,
    cellWidth: Float,
    cellHeight: Float,
    scrollOffset: Int,
    screenWidthPx: Float,
    screenHeightPx: Float,
    handleWidthPx: Float,
    pasteOnly: Boolean = false,
): MenuPosition {
    val loCol = min(start.col, end.col)
    val hiCol = max(start.col, end.col)
    val visibleLoRow = (min(start.row, end.row) - scrollOffset).coerceAtLeast(0)
    val visibleHiRow = (max(start.row, end.row) - scrollOffset).coerceAtLeast(0)

    val selLeft = loCol * cellWidth
    val selRight = (hiCol + 1) * cellWidth
    val selTop = visibleLoRow * cellHeight
    val selBottom = (visibleHiRow + 1) * cellHeight
    val selRect = RectF(selLeft, selTop, selRight, selBottom)

    val menuW =
        // Round-215: adapt to narrow screens — a fixed 260px menu occupies
        // more than half of a 480px (360dp) display. Three buttons
        // (Copy/Select All/Paste) at ~76px each plus padding fit in 240px;
        // clamp to the screen so the clamp math below never goes negative.
        260f.coerceAtMost((screenWidthPx - 16f).coerceAtLeast(120f))
    val menuH = 48f
    val selMidX = (selLeft + selRight) / 2f
    var menuX = (selMidX - menuW / 2f).coerceIn(0f, (screenWidthPx - menuW).coerceAtLeast(0f))
    // Place the menu BELOW the drag handles (which sit at the bottom edge
    // of the selection, ~handleWidthPx tall): a menu at selBottom+8 was
    // covered by the start-handle PopupWindow, so taps on Copy/Select All
    // dragged the handle instead (round-214). Paste-only popups have no
    // handles, so they hug the selection directly.
    var menuY = selBottom + 8f + if (pasteOnly) 0f else handleWidthPx
    val flipAbove = menuY + menuH > screenHeightPx && (selTop - menuH - 8f) >= 0f
    if (flipAbove) menuY = selTop - menuH - 8f
    menuY = menuY.coerceIn(0f, (screenHeightPx - menuH).coerceAtLeast(0f))

    var menuRect = RectF(menuX, menuY, menuX + menuW, menuY + menuH)
    var coversSelection = RectF.intersects(selRect, menuRect)
    if (coversSelection) {
        // Try right of selection
        val rightX = (selRight + 8f).coerceIn(0f, (screenWidthPx - menuW).coerceAtLeast(0f))
        val rightRect = RectF(rightX, menuY, rightX + menuW, menuY + menuH)
        if (!RectF.intersects(selRect, rightRect)) {
            menuX = rightX
            menuRect = rightRect
            coversSelection = false
        } else {
            // Try left of selection
            val leftX = (selLeft - menuW - 8f).coerceIn(0f, (screenWidthPx - menuW).coerceAtLeast(0f))
            val leftRect = RectF(leftX, menuY, leftX + menuW, menuY + menuH)
            if (!RectF.intersects(selRect, leftRect)) {
                menuX = leftX
                menuRect = leftRect
                coversSelection = false
            } else {
                // Try below selection (even if off-screen, coerce to bottom)
                val belowY = (screenHeightPx - menuH - 8f).coerceAtLeast(0f)
                if (belowY >= selBottom + handleWidthPx + 8f) {
                    menuY = belowY
                    menuRect = RectF(menuX, menuY, menuX + menuW, menuY + menuH)
                    coversSelection = RectF.intersects(selRect, menuRect)
                }
                // If still overlapping, accept (menu is small relative to huge selection)
            }
        }
    }
    return MenuPosition(
        menuX,
        menuY,
        menuW,
        menuH,
        selLeft,
        selTop,
        selRight,
        selBottom,
        flipAbove,
        coversSelection,
    )
}

@Composable
private fun SelectionMenuItem(
    text: String,
    onClick: () -> Unit,
) {
    Box(
        modifier =
        Modifier
            .clickable { onClick() }
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Text(
            text = text,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.labelMedium,
        )
    }
}
