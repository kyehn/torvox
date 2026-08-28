@file:Suppress("LocalContextGetResourceValueCall")

package terminal.emulator.ui

import android.annotation.SuppressLint
import android.graphics.RectF
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.activity.compose.BackHandler
import androidx.annotation.VisibleForTesting
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import terminal.emulator.R
import terminal.emulator.SelectionAnchor
import terminal.emulator.TerminalViewModel
import terminal.emulator.input.ModifierState
import terminal.emulator.input.next
import terminal.emulator.ui.theme.BuiltInThemes
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveTerminalThemeName
import kotlin.math.max
import kotlin.math.min

private const val FONT_SIZE_MIN = 14f
private const val FONT_SIZE_MAX = 48f

// The toggleKeyboard lambda already skips the defer path when the
// drawer is fully closed; when the drawer IS open, 50ms is enough
// for the scrim tap to register before the IME competes with the
// close animation. The old 250ms was perceptible as input lag.
private const val IME_TOGGLE_DELAY_MS = 50L

// Spec ime-translation hybrid pan-then-reflow: critically-damped spring so the bar
// tracks stepped IME inset reports continuously and settles in ~120ms. Settled
// detection is 3 stable frames × 16ms = 48ms, matching TerminalSurface debounce.
private const val IME_FOLLOW_SPRING_STIFFNESS = 4500f
private const val IME_FOLLOW_SPRING_DAMPING = 0.9f
private const val IME_SETTLE_FRAMES = 3
private const val IME_POLL_INTERVAL_MS = 16L

/**
 * Consolidated search state for text search within the terminal. Replaces 6 independent remember
 * variables.
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
    val hasResults: Boolean
        get() = results.isNotEmpty()

    val resultCount: Int
        get() = results.size

    val currentMatch: SearchResult?
        get() = results.getOrNull(currentIndex)
}

@SuppressLint("DeprecatedCall")
@Suppress("DEPRECATION")
private fun announceForAccessibility(view: android.view.View, text: CharSequence) {
    // View.announceForAccessibility has no @Deprecated annotation in
    // API 37 (verified via javap); the Kotlin compiler hard-codes it as
    // deprecated (system accessibility announcement is being phased out)
    // and slack-lint mirrors that. There is no modern equivalent — the
    // platform method is still the supported talk-back path.
    view.announceForAccessibility(text)
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
    val isSettingsDark =
        resolveAppDarkMode(settings.appThemeMode, androidx.compose.foundation.isSystemInDarkTheme())
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
    val terminalBackground = resolvedTerminalTheme.background
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    var searchJob by remember { mutableStateOf<kotlinx.coroutines.Job?>(null) }
    // P0#13: real 150ms debounce for the search query (termlib / ghostty-android
    // adoption). SearchDebouncer collapses rapid keystrokes into a single
    // performSearch after a quiet period instead of firing a full-scrollback
    // search on every keystroke. The production scheduler runs on the main
    // Looper (TerminalSurface callbacks are main-thread); unit tests use a fake
    // scheduler (see SearchDebouncerTest).
    val searchDebouncer = remember {
        SearchDebouncer(
            debounceMillis = 150L,
            scheduler = HandlerDebounceScheduler(Handler(Looper.getMainLooper())),
        )
    }
    val context = androidx.compose.ui.platform.LocalContext.current
    val resources = androidx.compose.ui.platform.LocalResources.current
    val hostView = androidx.compose.ui.platform.LocalView.current
    var showTextSearch by remember { mutableStateOf(false) }
    // Sticky FN layer (ModifierBar): when Locked the bar shows the F1-F12
    // second layer; tapping an F-key or FN again exits it.
    var fnState by remember { mutableStateOf(ModifierState.Off) }
    val onToggleFn: () -> Unit = { fnState = fnState.next() }
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    val view = LocalView.current
    val surfaceRef = remember { mutableStateOf<TerminalSurface?>(null) }
    // Toggle the soft keyboard (termux KEYBOARD key): used by the session
    // drawer's keyboard button and by a KEYBOARD extra key in a custom
    // toolbar layout. No-op in Raw keyboard mode (no IME to show or hide).
    // Visibility is read from the terminal surface's last window-insets
    // frame (imeVisible), not InputMethodManager.isAcceptingText, which is
    // stable across show/hide and would make the toggle a no-op after the
    // keyboard was dismissed.
    val toggleKeyboard: () -> Unit = {
        if (state.keyboardMode != terminal.emulator.input.KeyboardMode.Raw) {
            val inputMethodManager =
                context.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                    as android.view.inputmethod.InputMethodManager
            if (surfaceRef.value?.imeVisible == true) {
                inputMethodManager.hideSoftInputFromWindow(view.windowToken, 0)
            } else {
                view.requestFocus()
                // Round-234 (spec ime-follow-animation "keyboard toggle zero
                // unnecessary delay"): the defer exists ONLY to let the
                // session drawer's close animation settle — firing while the
                // scrim is animating competes with it. With the drawer fully
                // closed there is nothing to wait for: show immediately.
                val deferForDrawer = drawerState.isOpen || drawerState.isAnimationRunning
                if (!deferForDrawer) {
                    view.windowInsetsController?.show(
                        android.view.WindowInsets.Type.ime(),
                    )
                } else {
                    // SHOW_IMPLICIT outside a user gesture is silently
                    // rejected on Android 12+ (IME visibility requires a
                    // trusted gesture or window focus trust), which made
                    // the drawer's keyboard button a no-op for showing.
                    // Use the same WindowInsetsController path as a
                    // terminal tap (proven to show); it is not gesture-
                    // restricted.
                    view.postDelayed(
                        {
                            view.windowInsetsController?.show(
                                android.view.WindowInsets.Type.ime(),
                            )
                        },
                        IME_TOGGLE_DELAY_MS,
                    )
                }
            }
        }
    }
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
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
                val surface = surfaceRef.value
                // App-switch continuity: if the Surface was retained (no surfaceDestroyed),
                // render_paused is still true from onSurfaceDestroyed or previous pause and
                // surfaceCreated was never called to clear it → black frames. Clear immediately
                // when the surface is already attached and sized; only defer 200ms when the
                // surface is not yet ready (race with layout).
                if (
                    surface != null && surface.isAttachedToWindow && surface.width > 0 && surface.height > 0
                ) {
                    viewModel.runtime.setRenderPaused(false)
                    viewModel.runtime.resumeRendering()
                } else {
                    surface?.postDelayedUnpause(200L)
                }
                view.requestFocus()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(state.sessions.size) {
        val count = state.sessions.size
        if (count > 0) {
            announceForAccessibility(
                hostView,
                resources.getQuantityString(R.plurals.sessions_accessible, count, count),
            )
        }
    }

    LaunchedEffect(state.title) {
        val title = state.title
        if (
            title.isNotEmpty() &&
            title != context.getString(R.string.terminal_title) &&
            title != context.getString(R.string.terminal)
        ) {
            announceForAccessibility(
                hostView,
                resources.getString(R.string.title_changed, title),
            )
        }
    }

    LaunchedEffect(
        state.selection.active,
        state.selection.dragging,
        state.selection.start,
        state.selection.end,
    ) {
        val sel = state.selection
        // Announce only on settle, not while the user is dragging a handle
        // (start/end change every frame during a drag — announcing each one
        // floods TalkBack).
        if (sel.active && !sel.dragging && sel.hasSelection) {
            val text = sel.selectedText
            if (text.isNotEmpty()) {
                val preview = if (text.length > 100) text.take(100) + "..." else text
                announceForAccessibility(
                    hostView,
                    context.getString(R.string.selection_accessible, preview),
                )
            }
        }
    }

    // Selection accessibility is announced from the search effect below (after searchState is
    // declared).

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
                    onKeyboardToggle = toggleKeyboard,
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
            Modifier.fillMaxSize()
                .testTag("TerminalScreen")
                .background(terminalBackground)
                .statusBarsPadding()
                .navigationBarsPadding(),
        ) {
            LaunchedEffect(drawerState.isOpen) {
                surfaceRef.value?.drawerOpen = drawerState.isOpen
            }
            val selection = state.selection
            val selectionActive = selection.active && selection.start != null && selection.end != null

            // with the legacy View.startActionMode(Callback) the
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
                // the surface keeps a private scrollOffset
                // used for selection coordinate math; it must follow the
                // session's own offset on switch, otherwise the first
                // gesture after a switch computes wrong grid rows.
                surfaceRef.value?.resetScrollOffset()
            }

            // Programmatic scroll resets (input-driven snap to bottom,
            //  terminal-scrolling spec): resync the surface's
            // private offset so selection/drag coordinate math stays in
            // sync with the runtime viewport.
            LaunchedEffect(state.scrollEpoch) {
                if (state.scrollEpoch > 0L) {
                    surfaceRef.value?.resetScrollOffset()
                }
            }

            // Announce search result count changes for TalkBack.
            LaunchedEffect(searchState.resultCount, searchState.currentIndex, searchState.query) {
                if (searchState.query.isNotEmpty()) {
                    if (searchState.resultCount > 0) {
                        announceForAccessibility(
                            hostView,
                            context.getString(
                                R.string.search_result_accessible,
                                searchState.currentIndex + 1,
                                searchState.resultCount,
                            ),
                        )
                    } else {
                        announceForAccessibility(
                            hostView,
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
                    viewModel.runtime.bridge()
                        ?: run {
                            searchState = searchState.copy(results = emptyList())
                            return
                        }
                val effectiveCaseSensitive =
                    searchState.caseSensitive || (query.any { it.isUpperCase() } && !searchState.fuzzyMatch)
                val matches =
                    bridge.searchAllInScrollback(query, effectiveCaseSensitive, searchState.fuzzyMatch)
                        ?: run {
                            searchState = searchState.copy(results = emptyList())
                            return
                        }
                val results = matches.map { (row, startCol, endCol) ->
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

            // IME follow: hybrid pan-then-reflow (spec ime-translation).
            // Animating phase: placement-phase offset (zero remeasure, no per-frame grid reflow,
            // no surface buffer scale). Settled phase: single padding + grid resize via onImeSettled.
            // WindowInsets.ime is read with a spring so stepped reports interpolate smoothly and
            // the placement lambda avoids recomposition per frame. navigationBarsPadding is applied
            // once at the outer Box; inner Column/overlay must NOT re-apply it.
            val density = LocalDensity.current
            val rawImeBottomPx = WindowInsets.ime.getBottom(density)
            var settledImePx by remember { mutableStateOf(0) }
            var isImeSettled by remember { mutableStateOf(true) }
            LaunchedEffect(rawImeBottomPx) {
                if (rawImeBottomPx == settledImePx) {
                    isImeSettled = true
                    return@LaunchedEffect
                }
                isImeSettled = false
                delay(IME_POLL_INTERVAL_MS * IME_SETTLE_FRAMES)
                settledImePx = rawImeBottomPx
                isImeSettled = true
                surfaceRef.value?.onImeSettled(rawImeBottomPx)
            }
            val animatedImeBottom by
                animateDpAsState(
                    targetValue = with(density) { rawImeBottomPx.toDp() },
                    animationSpec =
                    spring(
                        dampingRatio = IME_FOLLOW_SPRING_DAMPING,
                        stiffness = IME_FOLLOW_SPRING_STIFFNESS,
                    ),
                    label = "imeBottom",
                )
            val animatedImePx = with(density) { animatedImeBottom.roundToPx() }
            val imeSettledPadding = with(density) { settledImePx.toDp() }

            Column(
                modifier =
                Modifier.fillMaxSize()
                    .testTag("TerminalContent")
                    .then(
                        if (isImeSettled) {
                            Modifier.padding(bottom = imeSettledPadding.coerceAtLeast(0.dp))
                        } else {
                            Modifier.offset { IntOffset(0, -animatedImePx.coerceAtLeast(0)) }
                        },
                    ),
            ) {
                // Terminal content area — moves above IME via animated padding
                Box(
                    modifier = Modifier.fillMaxWidth().weight(1f).onSizeChanged { terminalBoxSize = it },
                ) {
                    AndroidView(
                        factory = { context ->
                            terminal.emulator.ui
                                .TerminalSurface(context)
                                .apply { setTag("TerminalSurfaceView") }
                                .also { surface ->
                                    surfaceRef.value = surface
                                    surface.onScrollChanged = { offset ->
                                        viewModel.runtime.setScrollOffset(offset)
                                    }
                                    surface.onScrollingStateChanged = { isScrolling ->
                                        viewModel.runtime.setScrollActive(isScrolling)
                                    }
                                }
                                .apply {
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
                                                message =
                                                context.resources.getQuantityString(
                                                    R.plurals.copied_chars,
                                                    text.length,
                                                    text.length,
                                                ),
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
                                                    message =
                                                    context.resources.getQuantityString(
                                                        R.plurals.pasted_chars,
                                                        count,
                                                        count,
                                                    ),
                                                    duration = SnackbarDuration.Short,
                                                )
                                            }
                                        }
                                    }
                                    onZoomChanged = { sizeSp ->
                                        // ⑥ pinch finalize: persist the
                                        // settled size and run the full
                                        // apply (single grid reflow).
                                        viewModel.setFontSize(sizeSp.coerceIn(FONT_SIZE_MIN, FONT_SIZE_MAX))
                                    }
                                    onZoomPreview = { sizeSp ->
                                        // ⑥ live pinch preview: push font
                                        // metrics to native without
                                        // resizing; ghostty rows/cols
                                        // settle on finalize.
                                        viewModel.runtime.setFontSizePreview(sizeSp)
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
                            if (
                                runtimeState.rows > 0 &&
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
                        val themeAccent =
                            if (state.selectionAccent != 0) {
                                Color(state.selectionAccent)
                            } else {
                                resolvedTerminalTheme.foreground
                            }

                        fun colorToArgb(color: androidx.compose.ui.graphics.Color): Int = android.graphics.Color.argb(
                            (color.alpha * 255).toInt(),
                            (color.red * 255).toInt(),
                            (color.green * 255).toInt(),
                            (color.blue * 255).toInt(),
                        )
                        val themeAccentArgb = colorToArgb(themeAccent)

                        // A single effect keyed on the whole selection state:
                        // when the keys change the running effect is
                        // cancelled first, so a drag that ends without
                        // changing the anchor cells cannot leave the handles
                        // hidden (the old split show/hide effects could run
                        // out of order on the software renderer and hide
                        // after showing).
                        LaunchedEffect(
                            selectionActive,
                            selection.dragging,
                            loRow,
                            loCol,
                            hiRow,
                            hiCol,
                            themeAccentArgb,
                        ) {
                            // Round-234 review-5 BLOCKING fix: while a handle
                            // drag is live the single overlay OWNS the touch
                            // stream and repositions handles in-process —
                            // hideSelectionHandles() here used to dismiss the
                            // window under the finger and kill the gesture
                            // after the first cross-cell move. During
                            // dragging: deliberately a no-op. The menu is
                            // hidden separately via menuDismissed.
                            if (!selection.dragging) {
                                surfaceRef.value?.showSelectionHandles(loRow, loCol, hiRow, hiCol, themeAccentArgb)
                            }
                        }
                    } else {
                        LaunchedEffect(selectionActive) {
                            surfaceRef.value?.hideSelectionHandles()
                        }
                    }

                    // ── Selection context menu (PopupWindow) ──
                    // the menu must be a PopupWindow, not a
                    // Compose overlay — the terminal is a SurfaceView whose
                    // surface punches a hole over the whole terminal area,
                    // hiding any in-window Compose drawing. PopupWindows are
                    // separate system windows that render above it.
                    val menuSurface = surfaceRef.value
                    if (menuSurface != null && selectionActive && !selection.dragging) {
                        // The selection menu is a system ActionMode toolbar
                        // that positions itself; the tested positioning
                        // reference algorithm (computeMenuPosition /
                        // TerminalScreenMenuTest) is no longer called here.
                        val menuVisible = !selection.menuDismissed
                        if (menuVisible) {
                            val themeAccentArgb =
                                if (state.selectionAccent != 0) {
                                    Color(state.selectionAccent)
                                } else {
                                    resolvedTerminalTheme.foreground
                                }
                                    .let { color ->
                                        android.graphics.Color.argb(
                                            (color.alpha * 255).toInt(),
                                            (color.red * 255).toInt(),
                                            (color.green * 255).toInt(),
                                            (color.blue * 255).toInt(),
                                        )
                                    }
                            // the selection menu is the system
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
                        surfaceRef.value?.let {
                            Triple(it.getRows(), it.getMaxScrollOffset(), it.getScrollOffset())
                        },
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
                                        // Current match: fully opaque theme foreground.
                                        // Alpha >= 128 makes the Rust renderer swap fg/bg
                                        // (inverse video) and blend the opaque color over the
                                        // background, so the current hit is unmistakable.
                                        // See SearchHighlightColors.CURRENT_MATCH_ALPHA and
                                        // native/src/render/tests.rs production-value tests.
                                        writeByte((themeForeground.red * 255).toInt().toByte())
                                        writeByte((themeForeground.green * 255).toInt().toByte())
                                        writeByte((themeForeground.blue * 255).toInt().toByte())
                                        writeByte(SearchHighlightColors.CURRENT_MATCH_ALPHA.toByte())
                                    } else {
                                        // Other matches: selection_bg tint below the 128 swap
                                        // threshold — visible overlay, no inversion.
                                        // See SearchHighlightColors.OTHER_MATCH_ALPHA.
                                        writeByte((themeSelectionBg.red * 255).toInt().toByte())
                                        writeByte((themeSelectionBg.green * 255).toInt().toByte())
                                        writeByte((themeSelectionBg.blue * 255).toInt().toByte())
                                        writeByte(SearchHighlightColors.OTHER_MATCH_ALPHA.toByte())
                                    }
                                }
                                val highlightBytes = buf.toByteArray()
                                // Single call: surface.setSearchHighlights internally calls
                                // bridge.setSearchHighlights + bridge.render
                                surface.setSearchHighlights(highlightBytes)
                                searchState = searchState.copy(highlightsActive = true)
                            }
                        } else if (searchState.highlightsActive) {
                            surfaceRef.value?.clearSearchHighlights()
                            searchState = searchState.copy(highlightsActive = false)
                        }
                    }
                }

                // END OF COLUMN — terminal and bar both above IME
            } // close Column

            // Floating overlay for bottom bar — hybrid: offset during IME animation (zero remeasure),
            // padding when settled (single reflow). Mirrors Column's hybrid so both move in sync.
            Box(
                modifier =
                Modifier.fillMaxWidth()
                    .align(Alignment.BottomCenter)
                    .background(resolvedTerminalTheme.background)
                    .then(
                        if (isImeSettled) {
                            Modifier.padding(bottom = imeSettledPadding.coerceAtLeast(0.dp))
                        } else {
                            Modifier.offset { IntOffset(0, -animatedImePx.coerceAtLeast(0)) }
                        },
                    )
                    .testTag("ModifierBarOverlay"),
            ) {
                // Bottom bar — below terminal, above IME
                if (showTextSearch) {
                    TextSearchBar(
                        query = searchState.query,
                        onQueryChange = { query ->
                            searchState = searchState.copy(query = query)
                            searchJob?.cancel()
                            // P0#13: debounce the search so rapid keystrokes
                            // collapse into a single performSearch after 150ms of
                            // quiet (termlib / ghostty-android adoption). The
                            // previous pending search is cancelled by the
                            // debouncer, not by re-launching a coroutine per
                            // keystroke.
                            searchDebouncer.submit {
                                searchJob = scope.launch { performSearch() }
                            }
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
                            searchDebouncer.cancel()
                            searchJob?.cancel()
                            surfaceRef.value?.searchActive = false
                            surfaceRef.value?.clearSearchHighlights()
                        },
                        caseSensitive = searchState.caseSensitive,
                        onCaseSensitiveToggle = { searchState = searchState.copy(caseSensitive = it) },
                        fuzzyMatch = searchState.fuzzyMatch,
                        onFuzzyMatchToggle = { searchState = searchState.copy(fuzzyMatch = it) },
                        autoCaseSensitive =
                        !searchState.caseSensitive && searchState.query.any { it.isUpperCase() },
                        modifier = Modifier.testTag("TextSearchBar"),
                    )
                } else {
                    // Keep the ModifierBar in Normal mode during selection:
                    // the floating selection context menu (near the selection,
                    // never covering it, rendered as a PopupWindow) already
                    // offers Copy/Select All/Paste. Switching the whole bar
                    // to SelectionActions rendered a SECOND, redundant menu
                    // at the bottom.
                    val barMode = terminal.emulator.ui.ModifierBarMode.Normal
                    val clipboardManager =
                        context.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                            as? android.content.ClipboardManager
                    var hasClipboard by remember { mutableStateOf(false) }

                    LaunchedEffect(Unit) {
                        // hasPrimaryClip() is deprecated without a
                        // replacement (API 36); it is the only existence
                        // query the platform exposes.
                        @SuppressLint("DeprecatedCall")
                        hasClipboard = clipboardManager?.hasPrimaryClip() == true
                    }

                    // OnPrimaryClipChangedListener and the add/remove
                    // pair are deprecated without replacement (API 36);
                    // they are still the only clip-change notification API.
                    @SuppressLint("DeprecatedCall")
                    DisposableEffect(context) {
                        val listener =
                            android.content.ClipboardManager.OnPrimaryClipChangedListener {
                                @SuppressLint("DeprecatedCall")
                                hasClipboard = clipboardManager?.hasPrimaryClip() == true
                            }
                        clipboardManager?.addPrimaryClipChangedListener(listener)
                        onDispose { clipboardManager?.removePrimaryClipChangedListener(listener) }
                    }

                    ModifierBar(
                        modifier = Modifier.testTag("ModifierBar"),
                        onKeyClick = { data ->
                            viewModel.writeToPty(data.toByteArray())
                        },
                        onDrawerClick = {
                            scope.launch { drawerState.open() }
                        },
                        onScrollClick = {
                            viewModel.toggleScrollMode()
                        },
                        scrollActive = state.scrollActive,
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
                        onToggleKeyboard = toggleKeyboard,
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
                        // During text selection the floating context menu
                        // PASTE is never shown in the bottom ModifierBar — it
                        // belongs only to the selection menu (long-press blank) or
                        // the dedicated PasteBar above the keys. Showing it near
                        // the aux keys when not long-pressing was the reported
                        // "PASTE始终出现" confusion. Gates to null here.
                        onPaste = null,
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
            // The soft-keyboard toggle moved into the session drawer
            // (SessionDrawer keyboard button — same termux KEYBOARD key
            // semantics); the old floating side button overlapped terminal
            // content on narrow screens.
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
        // adapt to narrow screens — a fixed 260px menu occupies
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
    // dragged the handle instead. Paste-only popups have no
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
        modifier = Modifier.clickable { onClick() }.padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Text(
            text = text,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.labelMedium,
        )
    }
}
