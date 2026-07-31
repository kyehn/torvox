package terminal.emulator

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.Surface
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.KeyboardMode
import terminal.emulator.ui.ModifierKey
import terminal.emulator.ui.ModifierState
import terminal.emulator.ui.defaultModifierKeys
import terminal.emulator.ui.next
import terminal.emulator.ui.toKeyboardMode
import terminal.emulator.ui.toSettingsString
import javax.inject.Inject

private const val CLIPBOARD_TEXT_MAX_LENGTH = 100_000

enum class SelectionMode {
    Char,
    Word,
    Line,
    Block,
    Semantic,
}

/** Mirror of selection::TouchClass. */
enum class TouchClass {
    Text,
    Whitespace,
    EmptyArea,
    Unknown,
}

data class SelectionAnchor(
    val row: Int,
    val col: Int,
)

data class SelectionState(
    val active: Boolean = false,
    val dragging: Boolean = false,
    val start: SelectionAnchor? = null,
    val end: SelectionAnchor? = null,
    val mode: SelectionMode = SelectionMode.Char,
    val selectedText: String = "",
    val touchClass: TouchClass = TouchClass.Unknown,
) {
    val pasteOnly: Boolean
        get() = touchClass == TouchClass.EmptyArea || touchClass == TouchClass.Whitespace

    val hasSelection: Boolean
        get() = active && start != null && end != null

    fun applyHandleDrag(
        draggingStart: Boolean,
        targetRow: Int,
        targetCol: Int,
    ): HandleDragResult {
        val currentEnd = end ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol)
        val currentStart = start ?: return HandleDragResult(targetRow, targetCol, targetRow, targetCol)
        if (draggingStart && (targetRow > currentEnd.row || (targetRow == currentEnd.row && targetCol >= currentEnd.col))) {
            return HandleDragResult(currentEnd.row, currentEnd.col, targetRow, targetCol)
        }
        if (!draggingStart && (targetRow < currentStart.row || (targetRow == currentStart.row && targetCol <= currentStart.col))) {
            return HandleDragResult(targetRow, targetCol, currentStart.row, currentStart.col)
        }
        if (draggingStart) {
            return HandleDragResult(targetRow, targetCol, currentEnd.row, currentEnd.col)
        }
        return HandleDragResult(currentStart.row, currentStart.col, targetRow, targetCol)
    }
}

data class HandleDragResult(
    val startRow: Int,
    val startCol: Int,
    val endRow: Int,
    val endCol: Int,
)

data class PastePopupRequest(
    val row: Int,
    val col: Int,
)

data class SessionInfo(
    val id: Long,
    val title: String,
)

data class TerminalState(
    val sessionId: Long = 0L,
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val selection: SelectionState = SelectionState(),
    val modifierKeys: List<ModifierKey> = defaultModifierKeys,
    val ctrlState: ModifierState = ModifierState.Off,
    val altState: ModifierState = ModifierState.Off,
    val scrollActive: Boolean = false,
    val sessions: List<SessionInfo> = emptyList(),
    val activeSessionId: Long = 0L,
    val pastePopupRequest: PastePopupRequest? = null,
    val keyboardMode: KeyboardMode = KeyboardMode.Raw,
    val selectionBg: Int = 0,
    val selectionAccent: Int = 0,
)

internal fun shouldCreateDefaultSession(
    surfaceValid: Boolean,
    surfaceWidth: Int,
    surfaceHeight: Int,
    uiSessions: List<SessionInfo>,
    runtimeSessionIds: List<Long>,
): Boolean = surfaceValid &&
    surfaceWidth > 0 &&
    surfaceHeight > 0 &&
    uiSessions.isEmpty() &&
    runtimeSessionIds.isEmpty()

// ═══════════════════════════════════════════════════════════════════════════
// SECTION 1: Fields & constructor
// ═══════════════════════════════════════════════════════════════════════════

@HiltViewModel
class TerminalViewModel
@Inject
constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepository: SettingsRepository,
    val runtime: TerminalRuntime,
) : ViewModel() {
    companion object {
        private const val TAG = "TerminalViewModel"
        private const val STOP_TIMEOUT_MILLIS = 5000L
        private const val DEBOUNCE_MILLIS = 300L
        private const val DEFAULT_SCROLLBACK_LINES = 50_000
        private const val DEFAULT_FONT_SIZE_TENTHS = 18f
        private const val DEFAULT_THEME_NAME = "Dracula Plus"

        // Upper bound for text extraction loops (one JNI scrollbackLine call
        // per row, on the main thread). Keeps worst case bounded even when a
        // broadcast-injected selection claims thousands of rows.
        private const val MAX_SELECTION_LINES = 2000

        // Upper bound for clipboard paste (main-thread string copies) and
        // the chunk size used to stream it (must stay well below the PTY
        // kernel buffer ~64KB so a chunk never fails wholesale on EAGAIN).
        private const val MAX_PASTE_CHARS = 1_000_000
        private const val PASTE_CHUNK_CHARS = 4_000
    }

    private val _state = MutableStateFlow(TerminalState())
    val state: StateFlow<TerminalState> = _state.asStateFlow()

    @Volatile var currentSurface: Surface? = null

    @Volatile var surfaceWidth: Int = 0

    @Volatile var surfaceHeight: Int = 0

    fun startRuntime(
        surface: Surface?,
        width: Int,
        height: Int,
    ) {
        currentSurface = surface
        surfaceWidth = width
        surfaceHeight = height
        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            runtime.start(surface, width, height)
        }
    }

    val fontSize: StateFlow<Float> =
        settingsRepository.fontSize
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), DEFAULT_FONT_SIZE_TENTHS)

    val themeName: StateFlow<String> =
        settingsRepository.themeName
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), DEFAULT_THEME_NAME)

    val shell: StateFlow<String> =
        settingsRepository.shell
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "/system/bin/sh")

    val scrollbackLines: StateFlow<Int> =
        settingsRepository.scrollbackLines
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), DEFAULT_SCROLLBACK_LINES)

    val fontFamily: StateFlow<String> =
        settingsRepository.fontFamily
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "")

    val touchBehavior: StateFlow<String> =
        settingsRepository.touchBehavior
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "right_click")

    val bootstrapUrl: StateFlow<String> =
        settingsRepository.bootstrapUrl
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "")

    val useNerdFontGlyphs: StateFlow<Boolean> =
        settingsRepository.useNerdFontGlyphs
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), false)

    val useSemanticSelection: StateFlow<Boolean> =
        settingsRepository.useSemanticSelection
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), false)

    val dayThemeName: StateFlow<String> =
        settingsRepository.dayThemeName
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), SettingsRepository.DEFAULT_DAY_THEME_NAME)

    val nightThemeName: StateFlow<String> =
        settingsRepository.nightThemeName
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), DEFAULT_THEME_NAME)

    val themeMode: StateFlow<String> =
        settingsRepository.themeMode
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "fixed")

    val appThemeMode: StateFlow<String> =
        settingsRepository.appThemeMode
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "follow_system")

    val mcpServerEnabled: StateFlow<Boolean> =
        settingsRepository.mcpServerEnabled
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), false)

    val backgroundImagePath: StateFlow<String> =
        settingsRepository.backgroundImagePath
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "")

    val backgroundBlurRadius: StateFlow<Int> =
        settingsRepository.backgroundBlurRadius
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), 0)

    val backgroundAlpha: StateFlow<Float> =
        settingsRepository.backgroundAlpha
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), 0.8f)

    val cursorBlink: StateFlow<Boolean> =
        settingsRepository.cursorBlink
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), true)

    val cursorStyle: StateFlow<String> =
        settingsRepository.cursorStyle
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), "block")

    val cursorSpeed: StateFlow<Int> =
        settingsRepository.cursorSpeed
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), 530)

    private val _availableFonts = MutableStateFlow<List<String>>(emptyList())
    val availableFonts: StateFlow<List<String>> = _availableFonts.asStateFlow()

    private val _defaultFontName = MutableStateFlow("")
    val defaultFontName: StateFlow<String> = _defaultFontName.asStateFlow()

    private val _fontInfo = MutableStateFlow("Active: monospace\n(CJK fallback info available after session starts)")
    val fontInfo: StateFlow<String> = _fontInfo.asStateFlow()

    init {
        viewModelScope.launch {
            runtime.state.collect { runtimeState ->
                val sortedIds = runtimeState.sessionIds.sorted()
                val sessions =
                    sortedIds.mapIndexed { index, id ->
                        SessionInfo(id = id, title = "Session ${index + 1}")
                    }
                val active = runtimeState.activeSessionId
                if (active != 0L) {
                    val displayIndex = sortedIds.indexOf(active) + 1
                    val title =
                        if (runtimeState.title.isNotEmpty()) {
                            runtimeState.title
                        } else {
                            "Session $displayIndex"
                        }
                    // _state.update (CAS) instead of read-modify-write:
                    // createSession/switchSession on the IO dispatcher also
                    // update _state, and a non-atomic write here could
                    // clobber their just-committed session list.
                    _state.update { current ->
                        current.copy(
                            sessionId = active,
                            isRunning = runtimeState.isRunning,
                            title = title,
                            sessions = sessions,
                            activeSessionId = active,
                            selectionBg = runtime.selectionBgColor,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                    if (runtime.state.value.sessionIds
                            .isNotEmpty()
                    ) {
                        if (_availableFonts.value.isEmpty()) {
                            loadFonts()
                        } else {
                            val bridge = runtime.bridge()
                            _defaultFontName.value = bridge?.getDefaultFontName() ?: ""
                            _fontInfo.value = bridge?.getFontInfo()?.let { it.toString() } ?: "No font loaded"
                        }
                    }
                } else {
                    _state.update { current ->
                        current.copy(sessions = sessions, activeSessionId = active)
                    }
                }
            }
        }
        viewModelScope.launch {
            settingsRepository.keyboardMode.collect { mode ->
                _state.update { it.copy(keyboardMode = mode.toKeyboardMode()) }
            }
        }
        viewModelScope.launch {
            val savedPath = settingsRepository.backgroundImagePath.first()
            if (savedPath.isNotEmpty()) {
                applyBackgroundImageFromPath(savedPath)
            }
            settingsRepository.backgroundBlurRadius.first().let { radius ->
                settingsRepository.backgroundAlpha.first().let { alpha ->
                    val bridge = runtime.bridge()
                    if (bridge != null) {
                        bridge.setBackgroundParams(radius, (alpha * 10).toInt())
                    }
                }
            }
        }
        viewModelScope.launch {
            cursorBlink.collect { enabled ->
                val bridge = runtime.bridge() ?: return@collect
                bridge.setCursorBlinkEnabled(enabled)
                runtime.forceRender()
            }
        }
        viewModelScope.launch {
            cursorSpeed.collect { speed ->
                val bridge = runtime.bridge() ?: return@collect
                bridge.setCursorBlinkSpeedMs(speed.coerceIn(100, 1000))
                runtime.forceRender()
            }
        }
    }

    fun resetCursorBlink() {
        val bridge = runtime.bridge() ?: return
        bridge.resetCursorBlink()
        runtime.forceRender()
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2: Font management (extract → FontManager)
    // ══════════════════════════════════════════════════════════════════════

    private fun loadFonts() {
        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            try {
                val bridge = runtime.bridge()
                val rustFontFamilies = bridge?.listFontFamilies() ?: emptyList()
                val fileSystemFonts = terminal.emulator.ui.fallbackSystemFonts()
                val userFonts =
                    try {
                        val allUserFonts = mutableListOf<String>()
                        val filesDir = context.filesDir.resolve("fonts")
                        if (filesDir.isDirectory) {
                            allUserFonts.addAll(
                                filesDir
                                    .listFiles()
                                    ?.filter { it.isFile && (it.extension == "ttf" || it.extension == "otf") }
                                    ?.map { it.nameWithoutExtension }
                                    ?: emptyList(),
                            )
                        }
                        val cacheDir = context.cacheDir.resolve("fonts")
                        if (cacheDir.isDirectory) {
                            cacheDir
                                .listFiles()
                                ?.filter { it.isFile && (it.extension == "ttf" || it.extension == "otf") }
                                ?.forEach { cachedFile ->
                                    val destFile = java.io.File(filesDir, cachedFile.name)
                                    if (!destFile.exists()) {
                                        cachedFile.copyTo(destFile)
                                    }
                                    cachedFile.delete()
                                    allUserFonts.add(cachedFile.nameWithoutExtension)
                                }
                            if (cacheDir.listFiles().isNullOrEmpty()) {
                                cacheDir.delete()
                            }
                        }
                        allUserFonts.distinct()
                    } catch (exception: Exception) {
                        Log.e(TAG, "Failed to load user fonts", exception)
                        emptyList()
                    }
                val allFonts =
                    (rustFontFamilies + fileSystemFonts + userFonts)
                        .distinct()
                        .sorted()
                _availableFonts.value = allFonts
                _defaultFontName.value = bridge?.getDefaultFontName() ?: fileSystemFonts.firstOrNull() ?: ""
                _fontInfo.value =
                    bridge?.getFontInfo()?.let { it.toString() } ?: "Font: ${_defaultFontName.value}\n(CJK fallback info available after session starts)"
            } catch (exception: Exception) {
                Log.e("TerminalViewModel", "Failed to load font list", exception)
                _availableFonts.value = emptyList()
            }
        }
    }

    fun ensureDefaultSession() {
        if (!shouldCreateDefaultSession(
                surfaceValid = currentSurface?.isValid == true,
                surfaceWidth = surfaceWidth,
                surfaceHeight = surfaceHeight,
                uiSessions = _state.value.sessions,
                runtimeSessionIds = runtime.state.value.sessionIds,
            )
        ) {
            return
        }
        android.util.Log.d("TerminalViewModel", "ensureDefaultSession: creating default session")
        createSession()
    }

    fun setFontSize(size: Float) {
        viewModelScope.launch(Dispatchers.IO) {
            settingsRepository.setFontSize(size)
            runtime.applyFontSettings()
            val bridge = runtime.bridge()
            if (bridge != null) {
                _fontInfo.value = bridge.getFontInfo()?.let { it.toString() } ?: "No font loaded"
            }
        }
    }

    fun setFontFamily(family: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                android.util.Log.d("Font", "Setting font family: $family")
                settingsRepository.setFontFamily(family)
                runtime.applyFontSettings()
                val bridge = runtime.bridge()
                val fontName = bridge?.getDefaultFontName() ?: "monospace"
                val fontInfo = bridge?.getFontInfo()?.let { it.toString() } ?: "No font loaded"
                _defaultFontName.value = fontName
                _fontInfo.value = fontInfo
                android.util.Log.d("Font", "Font applied: $fontName")
                kotlinx.coroutines.withContext(Dispatchers.Main) {
                    android.widget.Toast
                        .makeText(context, "Font applied: $fontName", android.widget.Toast.LENGTH_SHORT)
                        .show()
                }
            } catch (exception: Exception) {
                android.util.Log.e("Font", "setFontFamily failed for $family", exception)
                kotlinx.coroutines.withContext(Dispatchers.Main) {
                    android.widget.Toast
                        .makeText(
                            context,
                            "Font apply failed: ${exception.message}",
                            android.widget.Toast.LENGTH_SHORT,
                        ).show()
                }
            }
        }
    }

    fun installFontFile(uri: Uri) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val rawName = getFileNameFromUri(uri) ?: uri.lastPathSegment ?: "custom_font.ttf"
                // Sanitize: DISPLAY_NAME from a content provider may contain
                // path separators or "..", which would escape fontsDir and
                // overwrite app-private files.
                val fileName = sanitizeFontFileName(rawName)
                val fontsDir =
                    context.filesDir.resolve("fonts").also { dir ->
                        if (!dir.mkdirs()) {
                            Log.w("TerminalViewModel", "Failed to create fonts directory: $dir")
                        }
                    }
                val destFile = java.io.File(fontsDir, fileName)
                context.contentResolver.openInputStream(uri)?.use { input ->
                    destFile.outputStream().use { output ->
                        input.copyTo(output)
                    }
                } ?: run {
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(context, "Failed to read font file", android.widget.Toast.LENGTH_SHORT)
                            .show()
                    }
                    return@launch
                }

                android.util.Log.d("Font", "Font file copied: ${destFile.absolutePath} (${destFile.length()} bytes)")

                val familyName = runtime.loadFontFile(destFile.absolutePath)
                if (familyName != null) {
                    android.util.Log.d("Font", "Font loaded: family=$familyName")
                    settingsRepository.setFontFamily(familyName)
                    runtime.applyFontSettings()
                    loadFonts()
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(context, "Font installed: $familyName", android.widget.Toast.LENGTH_SHORT)
                            .show()
                    }
                } else {
                    android.util.Log.e("Font", "Font load failed: null family from ${destFile.absolutePath}")
                    kotlinx.coroutines.withContext(Dispatchers.Main) {
                        android.widget.Toast
                            .makeText(context, "Font not supported or corrupted", android.widget.Toast.LENGTH_SHORT)
                            .show()
                    }
                }
            } catch (exception: Exception) {
                android.util.Log.e("TerminalViewModel", "installFontFile failed", exception)
                kotlinx.coroutines.withContext(Dispatchers.Main) {
                    android.widget.Toast
                        .makeText(
                            context,
                            "Font installation failed: ${exception.message}",
                            android.widget.Toast.LENGTH_SHORT,
                        ).show()
                }
            }
        }
    }

    fun setTouchBehavior(behavior: String) {
        viewModelScope.launch {
            settingsRepository.setTouchBehavior(behavior)
        }
    }

    fun setBootstrapUrl(url: String) {
        bootstrapUrlDebounce.value = url
        bootstrapUrlEdited = true
    }

    private val _bootstrapRunning = MutableStateFlow(false)
    val bootstrapRunning: StateFlow<Boolean> = _bootstrapRunning.asStateFlow()

    private val _bootstrapResult = MutableStateFlow<String?>(null)
    val bootstrapResult: StateFlow<String?> = _bootstrapResult.asStateFlow()

    private val _bootstrapProgress = MutableStateFlow<terminal.emulator.installer.BootstrapProgress?>(null)
    val bootstrapProgress: StateFlow<terminal.emulator.installer.BootstrapProgress?> =
        _bootstrapProgress.asStateFlow()

    fun runBootstrap() {
        // CAS so a rapid double-tap of the Install button cannot start two
        // concurrent installs (round-109).
        if (!_bootstrapRunning.compareAndSet(false, true)) return
        viewModelScope.launch(Dispatchers.IO) {
            _bootstrapResult.value = null
            _bootstrapProgress.value = null
            try {
                val onProgress =
                    terminal.emulator.installer.BootstrapProgressCallback { progress ->
                        _bootstrapProgress.value = progress
                    }
                val downloader =
                    terminal.emulator.installer.BootstrapDownloader(
                        context,
                        onProgress = onProgress,
                    )
                val installer =
                    terminal.emulator.installer.BootstrapInstaller(
                        prefixDir = java.io.File(context.filesDir, "bootstrap/usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        stagingDir = java.io.File(context.filesDir, "bootstrap/usr-staging"),
                        onProgress = onProgress,
                    )
                val secondStage =
                    terminal.emulator.installer.SecondStageRunner(
                        prefixDir = java.io.File(context.filesDir, "bootstrap/usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        onProgress = onProgress,
                    )
                val orchestrator =
                    terminal.emulator.installer.BootstrapOrchestrator(
                        downloader,
                        installer,
                        secondStage,
                        onProgress = onProgress,
                    )
                // Read the debounced value directly: the DataStore write is
                // debounced by 500ms, so first() could still return the old
                // URL if the user taps Install right after typing. An edited
                // (even if cleared) field wins over the stored URL (round-109).
                val url =
                    if (bootstrapUrlEdited) {
                        bootstrapUrlDebounce.value
                    } else {
                        settingsRepository.bootstrapUrl.first()
                    }
                val result = orchestrator.ensureBootstrap(url)
                _bootstrapResult.value = result.getOrNull() ?: "Error: ${result.exceptionOrNull()?.javaClass?.simpleName}"
            } catch (exception: Exception) {
                _bootstrapResult.value = "Error: ${exception.javaClass.simpleName}"
            } finally {
                _bootstrapRunning.value = false
            }
        }
    }

    fun setUseNerdFontGlyphs(enabled: Boolean) {
        viewModelScope.launch {
            settingsRepository.setUseNerdFontGlyphs(enabled)
        }
    }

    fun setUseSemanticSelection(enabled: Boolean) {
        viewModelScope.launch {
            settingsRepository.setUseSemanticSelection(enabled)
        }
    }

    fun setKeyboardMode(mode: KeyboardMode) {
        viewModelScope.launch {
            settingsRepository.setKeyboardMode(mode.toSettingsString())
        }
    }

    fun setMcpServerEnabled(enabled: Boolean) {
        // Native stop() may join the server thread for up to ~50ms; never
        // run it on the main thread from the settings screen.
        viewModelScope.launch(Dispatchers.IO) {
            if (NativeBridge.isNativeLoaded()) {
                NativeBridge.setMcpEnabled(enabled)
            } else {
                // Native library missing: setMcpEnabled is an external fun and
                // would throw UnsatisfiedLinkError (an Error, not an Exception),
                // crashing the app from the settings screen.
                Log.w("TerminalViewModel", "setMcpServerEnabled: native library not loaded, ignoring toggle")
            }
            settingsRepository.setMcpServerEnabled(enabled)
        }
    }

    fun setBackgroundImagePath(path: String) {
        viewModelScope.launch(Dispatchers.IO) {
            settingsRepository.setBackgroundImagePath(path)
            applyBackgroundImageFromPath(path)
        }
    }

    private fun applyBackgroundImageFromPath(path: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val bridge = runtime.bridge() ?: return@launch
            if (path.isNotEmpty()) {
                try {
                    val uri = android.net.Uri.parse(path)
                    // Two-pass decode: query bounds first, then compute an
                    // inSampleSize so large photos (48MP+) never allocate
                    // hundreds of MB (OOM crash). OutOfMemoryError is an
                    // Error, not an Exception, so it must be caught too.
                    val options = android.graphics.BitmapFactory.Options()
                    options.inJustDecodeBounds = true
                    context.contentResolver.openInputStream(uri)?.use { boundsStream ->
                        android.graphics.BitmapFactory.decodeStream(boundsStream, null, options)
                    } ?: return@launch
                    options.inJustDecodeBounds = false
                    var sampleSize = 1
                    while (options.outWidth / sampleSize > 1920 || options.outHeight / sampleSize > 1080) {
                        sampleSize *= 2
                    }
                    options.inSampleSize = sampleSize
                    val inputStream = context.contentResolver.openInputStream(uri) ?: return@launch
                    val bitmap =
                        try {
                            android.graphics.BitmapFactory.decodeStream(inputStream, null, options)
                        } finally {
                            inputStream.close()
                        }
                    if (bitmap != null) {
                        val bitmapWidth = bitmap.width
                        val bitmapHeight = bitmap.height
                        val buffer = java.nio.ByteBuffer.allocate(bitmapWidth * bitmapHeight * 4)
                        bitmap.copyPixelsToBuffer(buffer)
                        bitmap.recycle()
                        val rgbaData = buffer.array()
                        bridge.setBackgroundImage(rgbaData, bitmapWidth, bitmapHeight)
                        bridge.setBackgroundParams(
                            backgroundBlurRadius.value,
                            (backgroundAlpha.value * 10).toInt(),
                        )
                    }
                } catch (e: Throwable) {
                    Log.e(TAG, "setBackgroundImagePath failed", e)
                }
            } else {
                bridge.clearBackgroundImage()
            }
        }
    }

    fun setBackgroundBlurRadius(radius: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            settingsRepository.setBackgroundBlurRadius(radius)
            val bridge = runtime.bridge() ?: return@launch
            bridge.setBackgroundParams(radius, (backgroundAlpha.value * 10).toInt())
        }
    }

    fun setBackgroundAlpha(alpha: Float) {
        viewModelScope.launch(Dispatchers.IO) {
            settingsRepository.setBackgroundAlpha(alpha)
            val bridge = runtime.bridge() ?: return@launch
            bridge.setBackgroundParams(backgroundBlurRadius.value, (alpha * 10).toInt())
        }
    }

    fun setCursorBlink(enabled: Boolean) {
        viewModelScope.launch {
            settingsRepository.setCursorBlink(enabled)
            val bridge = runtime.bridge() ?: return@launch
            bridge.setCursorBlinkEnabled(enabled)
            runtime.forceRender()
        }
    }

    fun setCursorSpeed(speedMs: Int) {
        viewModelScope.launch {
            settingsRepository.setCursorSpeed(speedMs)
            val bridge = runtime.bridge() ?: return@launch
            bridge.setCursorBlinkSpeedMs(speedMs.coerceIn(100, 1000))
            runtime.forceRender()
        }
    }

    fun setCursorStyle(style: String) {
        viewModelScope.launch {
            settingsRepository.setCursorStyle(style)
            val bridge = runtime.bridge() ?: return@launch
            bridge.setCursorStyle(style)
            runtime.forceRender()
        }
    }

    fun setThemeName(name: String) {
        viewModelScope.launch {
            settingsRepository.setThemeName(name)
            runtime.applySettings()
        }
    }

    fun setDayThemeName(name: String) {
        viewModelScope.launch {
            settingsRepository.setDayThemeName(name)
            runtime.applySettings()
        }
    }

    fun setNightThemeName(name: String) {
        viewModelScope.launch {
            settingsRepository.setNightThemeName(name)
            runtime.applySettings()
        }
    }

    fun setThemeMode(mode: String) {
        viewModelScope.launch {
            settingsRepository.setThemeMode(mode)
            runtime.applySettings()
        }
    }

    fun setAppThemeMode(mode: String) {
        viewModelScope.launch {
            settingsRepository.setAppThemeMode(mode)
            runtime.applySettings()
        }
    }

    fun getFileNameFromUri(uri: Uri): String? {
        val cursor = context.contentResolver.query(uri, null, null, null, null)
        return cursor?.use {
            if (it.moveToFirst()) {
                val index = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (index >= 0) it.getString(index) else null
            } else {
                null
            }
        }
    }

    /** Keep only the final path segment and reject any traversal. */
    private fun sanitizeFontFileName(name: String): String {
        val base = name.substringAfterLast('/').substringAfterLast('\\')
        if (base.isEmpty() || base == "." || base == "..") return "custom_font.ttf"
        return base
    }

    private val shellTextDebounce = MutableStateFlow("")
    private val bootstrapUrlDebounce = MutableStateFlow("")

    // Written on the UI thread, read on an IO coroutine; volatile makes the
    // visibility explicit instead of relying on implicit happens-before
    // (round-110).
    @Volatile
    private var bootstrapUrlEdited = false

    init {
        // Debounce free-text settings so typing does not write DataStore
        // on every keystroke (each write is a full file rewrite).
        @OptIn(kotlinx.coroutines.FlowPreview::class)
        viewModelScope.launch {
            shellTextDebounce
                .debounce(DEBOUNCE_MILLIS)
                .distinctUntilChanged()
                .collect { value -> settingsRepository.setShell(value) }
        }
        @OptIn(kotlinx.coroutines.FlowPreview::class)
        viewModelScope.launch {
            bootstrapUrlDebounce
                .debounce(DEBOUNCE_MILLIS)
                .distinctUntilChanged()
                .collect { value -> settingsRepository.setBootstrapUrl(value) }
        }
    }

    fun setShell(shell: String) {
        shellTextDebounce.value = shell
    }

    fun setScrollbackLines(lines: Int) {
        viewModelScope.launch {
            settingsRepository.setScrollbackLines(lines)
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 3: Selection management (extract → SelectionManager)
    // ══════════════════════════════════════════════════════════════════════

    fun startSelection(
        row: Int,
        col: Int,
        touchClass: TouchClass = TouchClass.Unknown,
    ) {
        // A text-selection long-press supersedes any pending paste chip;
        // drop it so it cannot reappear once the selection is cleared
        // (round-105).
        consumePastePopupRequest()
        val anchor = SelectionAnchor(row, col)
        // CAS (round-98): selection is touched from the main thread but
        // _state is also written by IO coroutines; a plain RMW could lose
        // one of their updates. mode is read inside the lambda. (The old
        // EmptyArea override to Char mode was dead — no caller passes
        // EmptyArea; round-99.)
        _state.update { state ->
            state.copy(
                selection =
                SelectionState(
                    active = true,
                    dragging = true,
                    start = anchor,
                    end = anchor,
                    mode = state.selection.mode,
                    touchClass = touchClass,
                ),
            )
        }
    }

    fun updateSelection(
        row: Int,
        col: Int,
    ) {
        // CAS with the active-check inside the lambda (round-98): the
        // selection read and the write are atomic against concurrent _state
        // updates.
        _state.update { state ->
            val current = state.selection
            if (!current.active) {
                state
            } else {
                val result = current.applyHandleDrag(draggingStart = false, targetRow = row, targetCol = col)
                state.copy(
                    selection =
                    current.copy(
                        start = SelectionAnchor(result.startRow, result.startCol),
                        end = SelectionAnchor(result.endRow, result.endCol),
                    ),
                )
            }
        }
    }

    fun updateSelectionStart(
        row: Int,
        col: Int,
    ) {
        _state.update { state ->
            val current = state.selection
            if (!current.active) {
                state
            } else {
                val result = current.applyHandleDrag(draggingStart = true, targetRow = row, targetCol = col)
                state.copy(
                    selection =
                    current.copy(
                        start = SelectionAnchor(result.startRow, result.startCol),
                        end = SelectionAnchor(result.endRow, result.endCol),
                    ),
                )
            }
        }
    }

    fun endSelection() {
        val current = _state.value.selection
        if (!current.active || current.start == null || current.end == null) return
        val text = extractSelectedText(current)
        // Only write when the selection is unchanged since our read: a
        // concurrent selection update must not be clobbered with stale text.
        // CAS loop (round-101): compareAndSet retries while the state is
        // still ours and aborts as soon as a concurrent write lands — the
        // native boundary sync below runs only for a genuinely committed
        // snapshot (no side-effect flag that could leak across retries).
        // Note (round-100): extractSelectedText itself reads the bridge and
        // cols across a snapshot — those can also go stale mid-extraction if
        // an IO coroutine switches sessions; the result is bounded by the
        // substring guards and is never written unless this CAS commits.
        val updated = current.copy(dragging = false, selectedText = text)
        while (true) {
            val state = _state.value
            if (state.selection != current) return
            if (_state.compareAndSet(state, state.copy(selection = updated))) break
        }
        val start = current.start
        val end = current.end
        val loRow = minOf(start.row, end.row)
        val hiRow = maxOf(start.row, end.row)
        val loCol = minOf(start.col, end.col)
        val hiCol = maxOf(start.col, end.col)
        runtime.setSelection(loRow, loCol, hiRow, hiCol, true, current.mode.ordinal.toByte())
    }

    fun setSelectionMode(mode: SelectionMode) {
        _state.update { it.copy(selection = it.selection.copy(mode = mode)) }
    }

    private fun getClipboardManager(): ClipboardManager? = (context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager).also {
        if (it == null) {
            Log.w(TAG, "Clipboard service not available")
        }
    }

    fun copySelectionToClipboard() {
        val rawText = _state.value.selection.selectedText
        if (rawText.isEmpty()) return
        val text = if (rawText.length > CLIPBOARD_TEXT_MAX_LENGTH) rawText.substring(0, CLIPBOARD_TEXT_MAX_LENGTH) else rawText
        val clipboard = getClipboardManager() ?: return
        clipboard.setPrimaryClip(ClipData.newPlainText("terminal selection", text))
    }

    fun clearSelection() {
        _state.update { it.copy(selection = SelectionState()) }
        // Drop any pending paste chip: with the selection cleared it would
        // reappear over the terminal (round-105).
        consumePastePopupRequest()
        syncSelectionToNative()
    }

    fun showPastePopup(
        row: Int,
        col: Int,
    ) {
        _state.update { it.copy(pastePopupRequest = PastePopupRequest(row, col)) }
    }

    fun consumePastePopupRequest(): PastePopupRequest? {
        // Explicit CAS loop (round-102): update() would re-run its lambda on
        // contention and leak a stale captured result to a second consumer;
        // compareAndSet returns exactly one winner per request.
        while (true) {
            val state = _state.value
            val req = state.pastePopupRequest ?: return null
            if (_state.compareAndSet(state, state.copy(pastePopupRequest = null))) return req
        }
    }

    private fun syncSelectionToNative() {
        val selection = _state.value.selection
        if (selection.active && selection.start != null && selection.end != null) {
            val start = selection.start
            val end = selection.end
            val loRow = minOf(start.row, end.row)
            val hiRow = maxOf(start.row, end.row)
            val loCol = minOf(start.col, end.col)
            val hiCol = maxOf(start.col, end.col)
            runtime.setSelection(loRow, loCol, hiRow, hiCol, true, selection.mode.ordinal.toByte())
        } else {
            runtime.setSelection(0, 0, 0, 0, false, 0)
        }
    }

    fun shareSelection() {
        val text = _state.value.selection.selectedText
        if (text.isEmpty()) return
        val shareIntent =
            Intent.createChooser(
                Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_TEXT, text)
                },
                null,
            )
        shareIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(shareIntent)
    }

    fun selectAll(scrollOffset: Int = 0) {
        val runtimeState = runtime.state.value
        val rows = runtimeState.rows.coerceAtLeast(1)
        val cols = runtimeState.cols.coerceAtLeast(1)
        // Selection rows are grid rows (0 = top of scrollback). The
        // visible viewport starts at grid row (scrollbackLength - scrollOffset).
        val scrollbackLength = runtime.bridge()?.scrollbackLength() ?: 0
        val viewportStart = (scrollbackLength - scrollOffset).coerceAtLeast(0)
        val start = SelectionAnchor(row = viewportStart, col = 0)
        val end = SelectionAnchor(row = viewportStart + rows - 1, col = cols - 1)
        val selectionState =
            SelectionState(
                active = true,
                dragging = false,
                start = start,
                end = end,
                mode = SelectionMode.Char,
            )
        val text = extractSelectedText(selectionState)
        _state.update { it.copy(selection = selectionState.copy(selectedText = text)) }
        syncSelectionToNative()
    }

    @Suppress("CyclomaticComplexMethod")
    private fun extractSelectedText(selection: SelectionState): String {
        val start = selection.start ?: return ""
        val end = selection.end ?: return ""
        val bridge = runtime.bridge() ?: return ""
        // Selection rows are stored in grid coordinates (0 = top of
        // scrollback), so pass them to scrollbackLine() directly — no
        // viewport conversion here.
        val visibleCols =
            runtime.state.value.cols
                .coerceAtLeast(1)
        val (lo, hi) =
            if (start.row < end.row || (start.row == end.row && start.col <= end.col)) {
                start to end
            } else {
                end to start
            }
        return when (selection.mode) {
            SelectionMode.Char, SelectionMode.Word, SelectionMode.Semantic -> {
                if (lo.row == hi.row) {
                    val line = bridge.scrollbackLine(lo.row) ?: ""
                    val visLine = if (line.length > visibleCols) line.substring(0, visibleCols) else line
                    visLine.substring(lo.col.coerceAtMost(visLine.length), (hi.col + 1).coerceAtMost(visLine.length))
                } else {
                    val parts = mutableListOf<String>()
                    // Cap the per-call JNI round-trips: a hostile broadcast
                    // (partialSelectReceiver endRow clamps to 4095) could
                    // otherwise trigger thousands of scrollbackLine() calls
                    // on the main thread. Each call is a JNI boundary +
                    // String allocation; the cap keeps worst case bounded.
                    for (r in lo.row..hi.row.coerceAtMost(lo.row + MAX_SELECTION_LINES)) {
                        val line = bridge.scrollbackLine(r) ?: ""
                        val visLine = if (line.length > visibleCols) line.substring(0, visibleCols) else line
                        val startCol = if (r == lo.row) lo.col else 0
                        val endCol = if (r == hi.row) (hi.col + 1).coerceAtMost(visLine.length) else visLine.length
                        if (startCol < visLine.length) {
                            parts.add(visLine.substring(startCol, endCol.coerceAtMost(visLine.length)))
                        }
                    }
                    smartJoinLines(parts)
                }
            }

            SelectionMode.Line -> {
                val parts = mutableListOf<String>()
                for (r in lo.row..hi.row.coerceAtMost(lo.row + MAX_SELECTION_LINES)) {
                    val line = bridge.scrollbackLine(r) ?: ""
                    val visLine = if (line.length > visibleCols) line.substring(0, visibleCols) else line
                    parts.add(visLine)
                }
                parts.joinToString("\n")
            }

            SelectionMode.Block -> {
                val parts = mutableListOf<String>()
                for (r in lo.row..hi.row.coerceAtMost(lo.row + MAX_SELECTION_LINES)) {
                    val line = bridge.scrollbackLine(r) ?: ""
                    val visLine = if (line.length > visibleCols) line.substring(0, visibleCols) else line
                    var startCol = lo.col.coerceAtMost(visLine.length)
                    var endCol = hi.col.coerceAtMost(visLine.length)
                    // Reverse drag (start below/right of end) must not produce
                    // startCol > endCol — substring() would throw
                    // StringIndexOutOfBoundsException on the main thread.
                    if (startCol > endCol) {
                        val tmp = startCol
                        startCol = endCol
                        endCol = tmp
                    }
                    if (startCol < visLine.length) {
                        parts.add(visLine.substring(startCol, endCol))
                    }
                }
                parts.joinToString("\n")
            }
        }
    }

    private fun smartJoinLines(parts: List<String>): String {
        if (parts.size <= 1) return parts.joinToString("")
        val result = StringBuilder(parts[0])
        for (index in 1 until parts.size) {
            val previousLine = parts[index - 1]
            val currentLine = parts[index]
            if (isContinuationUrl(previousLine)) {
                result.append(currentLine)
            } else if (isUrlStart(currentLine)) {
                result.append("\n").append(currentLine)
            } else if (isPathOrProtocol(currentLine)) {
                result.append(currentLine)
            } else if (isTuiBorder(currentLine)) {
                break
            } else if (shouldJoinWithNewline(previousLine, currentLine)) {
                result.append("\n").append(currentLine)
            } else {
                result.append(currentLine)
            }
        }
        return result.toString()
    }

    private fun isContinuationUrl(line: String): Boolean = line.endsWith("https://") || line.endsWith("http://")

    private fun isUrlStart(line: String): Boolean = line.startsWith("https://") || line.startsWith("http://")

    private fun isPathOrProtocol(line: String): Boolean = line.startsWith("/") || line.startsWith("http")

    private fun shouldJoinWithNewline(
        previousLine: String,
        currentLine: String,
    ): Boolean {
        if (previousLine.isBlank() || currentLine.isBlank()) return false
        if (currentLine.startsWith(" ")) return false
        if (previousLine.endsWith(" ")) return false
        return true
    }

    private fun isTuiBorder(line: String): Boolean {
        val trimmed = line.trim()
        if (trimmed.isEmpty()) return false
        val uniqueChars = trimmed.toSet().size
        if (uniqueChars <= 2 && trimmed.all { it in "│─╭╮╰╯┌┐└┘┬┴├┤┼═║╗╝╚╔╠╣╦╩╬ " }) {
            return true
        }
        return false
    }

    fun pasteFromClipboard(): Int {
        val clipboard = getClipboardManager() ?: return 0
        if (!clipboard.hasPrimaryClip()) return 0
        val clipboardText = clipboard.primaryClip?.getItemAt(0)?.text ?: return 0
        val text = clipboardText.toString()
        if (text.length > MAX_PASTE_CHARS) {
            LogUtil.w(TAG, "pasteFromClipboard: clipboard too large (${text.length} chars), truncating to $MAX_PASTE_CHARS")
        }
        val normalized = text.take(MAX_PASTE_CHARS).replace("\n", "\r")
        // Chunked write (same rationale as TerminalSurface's direct paste):
        // one synchronous feedPty call with the full payload always exceeds
        // the PTY kernel buffer (~64KB) and is dropped wholesale on EAGAIN.
        // Chunks are split on code-point boundaries (never inside a
        // surrogate pair).
        var offset = 0
        while (offset < normalized.length) {
            var end = minOf(offset + PASTE_CHUNK_CHARS, normalized.length)
            if (end < normalized.length && Character.isHighSurrogate(normalized[end - 1])) {
                end -= 1
            }
            if (end <= offset) break
            val chunk = normalized.substring(offset, end).toByteArray()
            runtime.writeToPty(chunk)
            offset = end
        }
        // Report the chars queued up to the last successful chunk boundary
        // (post-truncation). Note: a chunk dropped by PTY backpressure
        // (EAGAIN) is still counted — this is the xterm-style "accepted"
        // count, not a byte-exact delivery count (round-112).
        return offset
    }

    fun writeToPty(data: ByteArray) {
        val written = runtime.writeToPty(data)
        if (!written) {
            LogUtil.e("TerminalViewModel", "writeToPty failed for ${data.size} bytes")
        }
    }

    fun setModifierKeys(keys: List<ModifierKey>) {
        _state.update { it.copy(modifierKeys = keys) }
    }

    fun resetModifierKeys() {
        _state.update { it.copy(modifierKeys = defaultModifierKeys) }
    }

    fun cycleCtrlState() {
        _state.update { it.copy(ctrlState = it.ctrlState.next()) }
    }

    fun cycleAltState() {
        _state.update { it.copy(altState = it.altState.next()) }
    }

    fun consumeOneShotModifiers() {
        val currentState = _state.value
        var newCtrl = currentState.ctrlState
        var newAlt = currentState.altState
        if (newCtrl == ModifierState.Once) newCtrl = ModifierState.Off
        if (newAlt == ModifierState.Once) newAlt = ModifierState.Off
        if (newCtrl != currentState.ctrlState || newAlt != currentState.altState) {
            _state.update { current -> current.copy(ctrlState = newCtrl, altState = newAlt) }
        }
    }

    /**
     * Layout-aware hardware key handling: produces the correct character for the
     * device's physical keyboard layout (e.g. Shift+2 -> '"' on German QWERTZ,
     * AltGr+Q -> '@') instead of relying on a hardcoded US mapping.
     *
     * Called from [android.app.Activity.dispatchKeyEvent] for physical-keyboard
     * [KeyEvent.ACTION_DOWN]s. Special keys (arrows, F-keys, numpad), modifier
     * combos (Ctrl+, left-Alt+), and soft-keyboard events are left to the
     * standard [KeyEvent] path so the Ghostty encoder handles them via key code.
     * AltGr (right Alt) is honored as a composing character, not an ESC prefix.
     *
     * Modeled on Haven's `handleLayoutAwareKeyEvent`
     * (feature/terminal/.../TerminalScreen.kt:1320-1407).
     *
     * @return true if the event was consumed here (so it must not also reach the
     *   view hierarchy), false to let the default dispatch continue.
     */
    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4: Keyboard modifier handling (extract → KeyboardHandler)
    // ══════════════════════════════════════════════════════════════════════

    fun handleLayoutAwareHardwareKey(event: KeyEvent): Boolean {
        if (event.action != KeyEvent.ACTION_DOWN) return false

        // Only physical keyboards. Soft-keyboard / IME input flows through the
        // InputConnection (commitText); intercepting it here would turn CJK /
        // voice composition into raw Latin letters.
        if ((event.flags and KeyEvent.FLAG_SOFT_KEYBOARD) != 0) return false
        if (event.deviceId == KeyCharacterMap.VIRTUAL_KEYBOARD) return false
        if (!event.isFromSource(InputDevice.SOURCE_KEYBOARD)) return false

        val keyCode = event.keyCode
        // Skip modifier-only presses and let the view handle them.
        when (keyCode) {
            KeyEvent.KEYCODE_SHIFT_LEFT,
            KeyEvent.KEYCODE_SHIFT_RIGHT,
            KeyEvent.KEYCODE_CTRL_LEFT,
            KeyEvent.KEYCODE_CTRL_RIGHT,
            KeyEvent.KEYCODE_ALT_LEFT,
            KeyEvent.KEYCODE_ALT_RIGHT,
            KeyEvent.KEYCODE_META_LEFT,
            KeyEvent.KEYCODE_META_RIGHT,
            KeyEvent.KEYCODE_CAPS_LOCK,
            KeyEvent.KEYCODE_NUM_LOCK,
            KeyEvent.KEYCODE_SCROLL_LOCK,
            KeyEvent.KEYCODE_FUNCTION,
            -> return false
        }

        val meta = event.metaState
        val hasAltGr = (meta and KeyEvent.META_ALT_RIGHT_ON) != 0
        // Ctrl+key and left-Alt+key are key-code based (control byte / ESC prefix),
        // not layout dependent — let the encoder path handle them. AltGr is the
        // exception: it produces a composed character.
        if ((meta and KeyEvent.META_CTRL_ON) != 0 && !hasAltGr) return false
        if ((meta and KeyEvent.META_ALT_ON) != 0 && !hasAltGr) return false

        val unicodeChar = event.getUnicodeChar(meta)
        if (unicodeChar <= 0) return false

        val bridge = runtime.bridge() ?: return false

        // Build the modifier mask from the sticky toolbar state only. Shift is
        // already baked into the produced character by getUnicodeChar.
        val state = _state.value
        var mask = 0
        if (state.ctrlState == ModifierState.Locked || state.ctrlState == ModifierState.Once) {
            mask = mask or 4
        }
        if (state.altState == ModifierState.Locked || state.altState == ModifierState.Once) {
            mask = mask or 2
        }

        // The unshifted codepoint is the base key with no modifiers applied:
        // recompute the character with SHIFT removed so the encoder can detect a
        // shift-only change (e.g. Shift+; -> :) and avoid a spurious Kitty shift.
        val unshiftedChar = event.getUnicodeChar(meta and KeyEvent.META_SHIFT_MASK.inv())
        val success = bridge.processKeyEvent(keyCode, mask.toByte(), 0, unicodeChar, unshiftedChar)
        if (success) {
            // Clear the one-shot (tapped) sticky modifier so it cannot persist
            // across the next keystroke (Haven TerminalViewModel.clearStickyModifiers,
            // #298). The encoder above already saw the active modifier for THIS
            // keystroke; consumption happens after the encode.
            consumeOneShotModifiers()
            Log.d(
                "TerminalViewModel",
                // Never log the character itself — hardware keyboard input
                // may contain passwords; the payload lands in the persisted
                // logcat dump (term_*.log).
                "handleLayoutAwareHardwareKey: keyCode=$keyCode mask=$mask",
            )
        }
        return success
    }

    fun setScrollActive(active: Boolean) {
        _state.update { it.copy(scrollActive = active) }
    }

    fun toggleScrollMode() {
        _state.update { it.copy(scrollActive = !it.scrollActive) }
    }

    fun createSession() {
        val surface = currentSurface
        if (surface == null || !surface.isValid) {
            android.util.Log.e("TerminalViewModel", "createSession: surface null or invalid, currentSurface=$currentSurface")
            return
        }
        val surfaceWidthPixels = surfaceWidth
        val surfaceHeightPixels = surfaceHeight
        if (surfaceWidthPixels <= 0 || surfaceHeightPixels <= 0) {
            android.util.Log.e("TerminalViewModel", "createSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels")
            return
        }

        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            val currentSurfaceNow = currentSurface
            if (currentSurfaceNow == null || !currentSurfaceNow.isValid) {
                android.util.Log.e("TerminalViewModel", "createSession: surface became invalid before launch")
                return@launch
            }
            try {
                val newId = runtime.createSession(currentSurfaceNow, surfaceWidthPixels, surfaceHeightPixels)
                if (newId > 0) {
                    _state.update { current ->
                        val sortedIds = (current.sessions.map { it.id } + newId).sorted()
                        val displayIndex = sortedIds.indexOf(newId) + 1
                        val sessions =
                            sortedIds.mapIndexed { index, id ->
                                SessionInfo(id = id, title = "Session ${index + 1}")
                            }
                        current.copy(
                            sessionId = newId,
                            isRunning = true,
                            title = "Session $displayIndex",
                            selection = SelectionState(),
                            sessions = sessions,
                            activeSessionId = newId,
                            selectionBg = runtime.selectionBgColor,
                            selectionAccent = runtime.accentColor,
                        )
                    }
                } else {
                    android.util.Log.e("TerminalViewModel", "createSession: runtime returned invalid id=$newId")
                }
            } catch (exception: Exception) {
                android.util.Log.e("TerminalViewModel", "createSession failed", exception)
            }
        }
    }

    fun switchSession(id: Long) {
        val surface = currentSurface
        if (surface == null || !surface.isValid) {
            android.util.Log.e("TerminalViewModel", "switchSession: surface null or invalid, currentSurface=$currentSurface")
            return
        }
        val surfaceWidthPixels = surfaceWidth
        val surfaceHeightPixels = surfaceHeight
        if (surfaceWidthPixels == 0 || surfaceHeightPixels == 0) {
            android.util.Log.e("TerminalViewModel", "switchSession: invalid dimensions ${surfaceWidthPixels}x$surfaceHeightPixels")
            return
        }

        viewModelScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            try {
                runtime.switchSession(id, surface, surfaceWidthPixels, surfaceHeightPixels)
            } catch (exception: Exception) {
                android.util.Log.e("TerminalViewModel", "switchSession failed for id=$id", exception)
                return@launch
            }
            _state.update { current ->
                current.copy(
                    sessionId = id,
                    isRunning = true,
                    title =
                    runtime.state.value.title
                        .ifEmpty {
                            val sortedIds =
                                current.sessions
                                    .map { it.id }
                                    .sorted()
                            "Session ${sortedIds.indexOf(id) + 1}"
                        },
                    activeSessionId = id,
                    selection = SelectionState(),
                    selectionBg = runtime.selectionBgColor,
                    selectionAccent = runtime.accentColor,
                )
            }
        }
    }

    fun closeSession() {
        closeSession(_state.value.activeSessionId)
    }

    fun closeSession(id: Long) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                runtime.closeSession(id)
            } catch (exception: Exception) {
                // closeSession must never escape to the main-thread uncaught
                // handler: BootGuard would treat it as a crash and kill the
                // process. The native side tolerates unknown/dead sessions.
                android.util.Log.e("TerminalViewModel", "closeSession failed for id=$id", exception)
                return@launch
            }
            withContext(Dispatchers.Main) {
                _state.update { current ->
                    val remaining = current.sessions.filter { it.id != id }
                    if (remaining.isEmpty()) {
                        current.copy(
                            isRunning = false,
                            sessions = emptyList(),
                            activeSessionId = 0L,
                            selection = SelectionState(),
                        )
                    } else {
                        val renumbered =
                            remaining.sortedBy { it.id }.mapIndexed { index, session ->
                                session.copy(title = "Session ${index + 1}")
                            }
                        val newActive =
                            if (current.activeSessionId == id) {
                                remaining.last().id
                            } else {
                                current.activeSessionId
                            }
                        val newActiveIndex = renumbered.indexOfFirst { it.id == newActive }
                        current.copy(
                            sessions = renumbered,
                            activeSessionId = newActive,
                            sessionId = newActive,
                            title =
                            runtime.state.value.title
                                .ifEmpty { "Session ${newActiveIndex + 1}" },
                            selection = SelectionState(),
                        )
                    }
                }
            }
        }
    }

    fun setSessionTitle(title: String) {
        _state.update { it.copy(title = title) }
    }
}
