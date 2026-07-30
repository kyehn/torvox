package terminal.emulator.runtime

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Surface
import androidx.compose.ui.graphics.Color
import dagger.hilt.android.qualifiers.ApplicationContext
import terminal.emulator.bridge.Bridge
import terminal.emulator.bridge.BridgeTheme
import terminal.emulator.bridge.NativeBridge
import terminal.emulator.bridge.Shell
import terminal.emulator.bridge.TerminalConfig
import terminal.emulator.bridge.createBridge
import terminal.emulator.monitor.RenderWatchDog
import terminal.emulator.settings.SettingsRepository
import terminal.emulator.ui.theme.BuiltInThemes
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.concurrent.thread

data class RuntimeState(
    val isRunning: Boolean = false,
    val title: String = "Terminal",
    val rows: Int = 24,
    val cols: Int = 80,
    val activeSessionId: Long = 0L,
    val sessionIds: List<Long> = emptyList(),
)

/**
 * Session state is encoded in two booleans:
 * - running=true, renderThreadExited=false  → alive
 * - running=true, renderThreadExited=true   → dead (needs cleanup)
 * - running=false, renderThreadExited=*     → stopped (flag is stale; skip)
 * renderThreadExited is set by the render thread after loop exit;
 * always read under sessionLock alongside running.
 */
private data class SessionEntry(
    val id: Long,
    var bridge: Bridge?,
    var renderThreadRef: Thread?,
    @Volatile var running: Boolean,
    @Volatile var blitCallback: (() -> Unit)? = null,
    @Volatile var renderThreadExited: Boolean = false,
    @Volatile var restartAttempts: Int = 0,
    @Volatile var nextRestartDelayMs: Long = 200L,
) {
    // per-frame `CountDownLatch`, which had a lost-wakeup race: after
    // `bridge.render()` the loop published a fresh latch and waited, but a
    // producer `countDown()` on the stale latch during the render left the new
    // latch unsignaled, so the thread waited the full timeout. A coalescing
    // flag under a lock/condition avoids both the race and the per-frame
    // allocation.

    val renderSignaled =
        java.util.concurrent.atomic
            .AtomicBoolean(false)

    @Volatile var forceRenderRequested: Boolean = false

    @Volatile var lastRenderStart: Long = 0L

    @Volatile var lastRenderDone: Long = 0L
    var renderWatchDog: RenderWatchDog? = null

    @Volatile var lastSignalNanos: Long = System.nanoTime()

    fun notifyRender() {
        lastSignalNanos = System.nanoTime()
        renderSignaled.set(true)
        renderThreadRef?.let {
            java.util.concurrent.locks.LockSupport
                .unpark(it)
        }
    }

    @Volatile var scrollOffset: Int = 0
}

// ═══════════════════════════════════════════════════════════════════════════
// SECTION 1: Fields & injected dependencies
// ═══════════════════════════════════════════════════════════════════════════

@Singleton
class TerminalRuntime
@Inject
constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepository: SettingsRepository,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow(RuntimeState())
    val state: StateFlow<RuntimeState> = _state.asStateFlow()

    private val sessions = ConcurrentHashMap<Long, SessionEntry>()

    // MCP dialog / file-pick requests from the embedded MCP server.
    // Wired by the UI layer (e.g. MainActivity) via setDialogRequestHandler;
    // responses are sent back through NativeBridge.dialogResult.
    @Volatile
    var dialogRequestHandler:
        ((sessionId: Long, requestId: Long, dialogType: String, title: String, message: String, options: List<String>) -> Unit)? =
        null

    @Volatile
    var pickFileRequestHandler:
        ((sessionId: Long, requestId: Long, startingPath: String, filter: String) -> Unit)? =
        null

    @Volatile var accentColor: Int = 0xFF2196F3.toInt()

    @Volatile var selectionBgColor: Int = 0xFF45475A.toInt()

    @Volatile var cellWidth: Float = 0f

    @Volatile var cellHeight: Float = 0f

    private val renderGeneration =
        java.util.concurrent.atomic
            .AtomicInteger(0)

    @Volatile private var activeSessionId: Long = 0L

    @Volatile private var stopped = false

    @Volatile private var starting = false
    private val stopLock = Any()
    private val sessionLock = Any()

    private var foregroundServiceRunning = false

    @Volatile private var renderMonitorJob: Job? = null

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2: Render thread lifecycle
    // ══════════════════════════════════════════════════════════════════════

    private fun startRenderMonitor() {
        if (renderMonitorJob?.isActive == true) return
        renderMonitorJob =
            scope.launch {
                while (isActive) {
                    delay(RENDER_MONITOR_INTERVAL_MS)
                    checkSessions()
                }
            }
    }

    private fun stopRenderMonitor() {
        renderMonitorJob?.cancel()
        renderMonitorJob = null
    }

    private fun checkSessions() {
        val deadSessions = mutableListOf<SessionEntry>()
        val healthySessions = mutableListOf<SessionEntry>()
        synchronized(sessionLock) {
            scanSessionsForDeath(deadSessions, healthySessions)
        }
        decayRestartCounts(healthySessions)
        for (entry in deadSessions) {
            scope.launch {
                handleDeadRenderThread(entry)
            }
        }
    }

    private fun scanSessionsForDeath(
        deadSessions: MutableList<SessionEntry>,
        healthySessions: MutableList<SessionEntry>,
    ) {
        for (entry in sessions.values) {
            if (!entry.running) continue
            if (entry.bridge == null) continue
            if (entry.renderThreadExited) {
                deadSessions.add(entry)
            } else {
                val thread = entry.renderThreadRef
                if (thread != null && !thread.isAlive) {
                    deadSessions.add(entry)
                }
                if (entry.restartAttempts > 0) {
                    healthySessions.add(entry)
                }
            }
        }
    }

    private fun decayRestartCounts(healthySessions: MutableList<SessionEntry>) {
        for (entry in healthySessions) {
            synchronized(sessionLock) {
                if (!entry.running || entry.renderThreadExited) continue
                if (entry.restartAttempts > 0) {
                    entry.restartAttempts = 0
                    entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                }
            }
        }
    }

    private suspend fun handleDeadRenderThread(entry: SessionEntry) {
        LogUtil.w(
            "Runtime",
            "session ${entry.id} render thread exited, restart attempt ${entry.restartAttempts + 1}",
        )

        val delayMs =
            synchronized(sessionLock) {
                if (!entry.running) return
                if (!entry.renderThreadExited) {
                    val thread = entry.renderThreadRef
                    if (thread != null && thread.isAlive) return
                    entry.renderThreadExited = true
                }
                if (entry.bridge == null) {
                    entry.running = false
                    entry.renderThreadExited = false
                    return
                }
                stopDeadRenderThreadResources(entry)
                entry.restartAttempts++
                if (entry.restartAttempts > RENDER_MAX_RESTART_ATTEMPTS) {
                    closeDeadSession(entry)
                    return
                }
                val d = entry.nextRestartDelayMs
                entry.nextRestartDelayMs = (entry.nextRestartDelayMs * 2).coerceAtMost(MAX_RESTART_DELAY_MS)
                d
            }

        delay(delayMs)
        restartRenderThreadAfterDelay(entry)
        delay(GRACE_PERIOD_AFTER_RESTART_MS)
        confirmRestartGrace(entry)
    }

    private fun stopDeadRenderThreadResources(entry: SessionEntry) {
        entry.renderWatchDog?.stop()
        entry.renderWatchDog = null
        entry.running = false
        entry.renderThreadRef?.let { t ->
            t.interrupt()
            t.join(THREAD_JOIN_TIMEOUT_MS)
            if (t.isAlive) {
                LogUtil.w("Runtime", "Render thread did not exit within timeout, continuing anyway")
            }
        }
        entry.renderThreadRef = null
        entry.renderSignaled.set(false)
        // Skip releaseGpuSurface here — the new render thread will
        // reconfigure the surface via setNativeWindow/updateNativeWindow.
    }

    private fun handleSessionExit(entry: SessionEntry, exitCode: Int) {
        LogUtil.i("Runtime", "session ${entry.id} exited with code $exitCode")
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            entry.running = false
            try {
                entry.bridge?.close()
            } catch (e: Exception) {
                LogUtil.e("Runtime", "session ${entry.id} bridge close on exit failed", e)
            }
            sessions.remove(entry.id)
            if (entry.id == activeSessionId) {
                activeSessionId = sessions.keys.sorted().lastOrNull() ?: 0L
            }
            terminal.emulator.service.TerminalForegroundService
                .updateSessionCount(context, sessions.size)
            updateState()
        }
    }

    private fun closeDeadSession(entry: SessionEntry) {
        LogUtil.e("Runtime", "session ${entry.id} exceeded max restart attempts, closing session")
        LogcatFileWriter.write("Runtime", "session ${entry.id} exceeded max restart attempts, closing session")
        try {
            entry.bridge?.close()
        } catch (e: Exception) {
            LogUtil.e("Runtime", "session ${entry.id} bridge close during cleanup failed", e)
        }
        sessions.remove(entry.id)
        if (entry.id == activeSessionId) {
            val remaining = sessions.keys.sorted()
            activeSessionId = remaining.lastOrNull() ?: 0L
        }
        updateState()
    }

    private suspend fun restartRenderThreadAfterDelay(entry: SessionEntry) {
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            if (entry.running) return
            if (entry.bridge == null) return
            entry.renderThreadExited = false
            startRenderThread(entry)
            LogUtil.d("Runtime", "session ${entry.id} render thread restarted (attempt ${entry.restartAttempts})")
        }
    }

    private suspend fun confirmRestartGrace(entry: SessionEntry) {
        synchronized(sessionLock) {
            if (!sessions.containsKey(entry.id)) return
            if (!entry.running) return
            if (entry.renderThreadExited) return
            val thread = entry.renderThreadRef
            if (thread != null && thread.isAlive) {
                entry.restartAttempts = 0
                entry.nextRestartDelayMs = INITIAL_RESTART_DELAY_MS
                LogUtil.d("Runtime", "session ${entry.id} render thread healthy after restart")
            }
        }
    }

    private fun startForegroundServiceIfNeeded() {
        if (!foregroundServiceRunning) {
            terminal.emulator.service.TerminalForegroundService
                .start(context)
            foregroundServiceRunning = true
            LogUtil.d("Runtime", "foreground service started")
        }
    }

    private fun stopForegroundService() {
        if (foregroundServiceRunning) {
            terminal.emulator.service.TerminalForegroundService
                .stop(context)
            foregroundServiceRunning = false
            LogUtil.d("Runtime", "foreground service stopped")
        }
    }

    private data class SelectionStateSnapshot(
        val startRow: Int,
        val startCol: Int,
        val endRow: Int,
        val endCol: Int,
        val hasSelection: Boolean,
        val mode: Byte,
    )

    private val selectionState =
        java.util.concurrent.atomic.AtomicReference(
            SelectionStateSnapshot(0, 0, 0, 0, false, 0),
        )

    fun setScrollOffset(offset: Int) {
        val entry = sessions[activeSessionId] ?: return
        entry.scrollOffset = offset
        // The render thread already reads entry.scrollOffset and calls
        // bridge.setScrollOffset() under the surface lock, so calling it here
        // would be a redundant JNA round-trip + surface-lock acquisition on the
        // calling thread (often the UI thread during scroll). Just signal the
        // render thread to pick up the change.
        entry.notifyRender()
    }

    fun getScrollOffset(): Int = sessions[activeSessionId]?.scrollOffset ?: 0

    fun forceRender() {
        val entry = sessions[activeSessionId] ?: return
        entry.forceRenderRequested = true
        entry.notifyRender()
    }

    private fun sessionSavePath(id: Long): String {
        val sessionsDirectory = context.getDir("sessions", Context.MODE_PRIVATE)
        return java.io.File(sessionsDirectory, "session_$id.bin").absolutePath
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 3: Session lifecycle
    // ══════════════════════════════════════════════════════════════════════

    private suspend fun buildConfig(
        rows: Int = 24,
        cols: Int = 80,
    ): TerminalConfig {
        val configReads =
            coroutineScope {
                val shellDeferred = async { settingsRepository.shell.first() }
                val scrollbackDeferred = async { settingsRepository.scrollbackLines.first() }
                val fontDeferred = async { computeFontSizeTenths() }
                val themeDeferred = async { resolveThemeName() }
                ConfigReads(
                    shellPath = shellDeferred.await(),
                    scrollbackLines = scrollbackDeferred.await(),
                    fontSizeTenths = fontDeferred.await(),
                    themeName = themeDeferred.await(),
                )
            }
        val resolvedTheme = BuiltInThemes.byName(configReads.themeName)
        val shell = resolveShell(configReads.shellPath)
        val bridgeTheme = makeBridgeTheme(resolvedTheme)
        accentColor = bridgeTheme.ansi5
        selectionBgColor = bridgeTheme.selectionBg
        val prefixDir = java.io.File(context.filesDir, "bootstrap/usr").absolutePath
        val homeDir = java.io.File(context.filesDir, "home").absolutePath
        val bashFile = java.io.File("$prefixDir/bin/bash")
        val bashComplete =
            bashFile.exists() &&
                java.io.File("$prefixDir/lib").isDirectory &&
                java.io.File("$prefixDir/etc").isDirectory
        val effectivePrefix = if (bashComplete) prefixDir else ""
        val effectiveShell = if (bashComplete) Shell.Custom("$prefixDir/bin/bash") else shell
        val effectiveHome =
            if (bashComplete) {
                homeDir
            } else {
                java.io
                    .File(context.filesDir, "home")
                    .apply {
                        if (!exists() && !mkdirs()) {
                            Log.w("Runtime", "Failed to create home directory: $this")
                        }
                    }.absolutePath
            }
        val effectivePath: String =
            if (bashComplete) {
                "$prefixDir/bin:${System.getenv("PATH").orEmpty().ifEmpty { "/system/bin:/system/xbin" }}"
            } else {
                System.getenv("PATH").orEmpty().ifEmpty { "/system/bin:/system/xbin" }
            }
        return TerminalConfig(
            shell = effectiveShell,
            rows = rows,
            cols = cols,
            scrollbackLines = configReads.scrollbackLines,
            font_size_tenths = configReads.fontSizeTenths,
            theme = bridgeTheme,
            home = effectiveHome,
            user = System.getProperty("user.name") ?: "shell",
            path = effectivePath,
            workingDirectory = effectiveHome,
            prefix = effectivePrefix,
        )
    }

    private companion object {
        private const val TENTHS_PER_UNIT = 10
        private const val FONT_SIZE_DISPLAY_RATIO = 0.6f
        private const val FONT_SIZE_MIN_PX = 300
        private const val FONT_SIZE_MAX_PX = 600
        private const val FONT_SIZE_HEIGHT_RATIO = 0.5f
        private const val FONT_SIZE_HEIGHT_MIN_PX = 250
        private const val FONT_SIZE_HEIGHT_MAX_PX = 500
        private const val RENDER_ERROR_LOG_FREQUENCY = 60
        private const val RENDER_MAX_CONSECUTIVE_ERRORS = 100
        private const val RENDER_MAX_TRANSIENT_ERRORS = 50 // ~2.5s of transient errors before thread exit
        private const val RENDER_ERROR_SLEEP_MS = 50L
        private const val RENDER_ERROR_BACKOFF_MS = 200L // Longer sleep after 10 consecutive transient errors
        private const val RENDER_LATCH_TIMEOUT_NANOS = 16_000_000L // 16ms for active (~60 FPS)
        private const val RENDER_LATCH_IDLE_TIMEOUT_NANOS = 500_000_000L // 500ms for idle (~2 FPS)
        private const val RENDER_IDLE_THRESHOLD_NANOS = 5_000_000_000L // 5s idle → switch to low-freq
        private const val RENDER_DIAGNOSTIC_FREQUENCY = 60
        private const val THREAD_JOIN_TIMEOUT_MS = 1000L
        private const val BEL_TONE_STREAM_TYPE = AudioManager.STREAM_NOTIFICATION
        private const val BEL_TONE_VOLUME = 50
        private const val BEL_TONE_TYPE = ToneGenerator.TONE_PROP_ACK
        private const val BEL_TONE_DURATION_MILLIS = 200
        private const val RENDER_HANG_TIMEOUT_NANOS = 10_000_000_000L // 10 seconds
        private const val RENDER_INITIAL_RETRY_MAX = 5
        private const val RENDER_INITIAL_RETRY_DELAY_MS = 150L

        // Render monitor — proactive death detection
        private const val RENDER_MONITOR_INTERVAL_MS = 500L
        private const val RENDER_MAX_RESTART_ATTEMPTS = 5
        private const val INITIAL_RESTART_DELAY_MS = 100L
        private const val MAX_RESTART_DELAY_MS = 1000L
        private const val GRACE_PERIOD_AFTER_RESTART_MS = 300L
        private val nextSessionId = AtomicLong(1L)

        /** Cached ToneGenerator for bell — avoids per-event allocation. */
        private val bellToneGenerator by lazy {
            ToneGenerator(BEL_TONE_STREAM_TYPE, BEL_TONE_VOLUME)
        }
    }

    private data class ConfigReads(
        val shellPath: String,
        val scrollbackLines: Int,
        val fontSizeTenths: Int,
        val themeName: String,
    )

    internal suspend fun computeFontSizeTenths(): Int {
        val userFontSize = settingsRepository.fontSize.first()
        val density = context.resources.displayMetrics.density
        val cellHeightPixels = userFontSize * density
        return (cellHeightPixels * TENTHS_PER_UNIT.toFloat()).toInt()
    }

    internal suspend fun resolveThemeName(): String {
        val themeMode = settingsRepository.themeMode.first()
        val dayTheme = settingsRepository.dayThemeName.first()
        val nightTheme = settingsRepository.nightThemeName.first()
        val singleTheme = settingsRepository.themeName.first()
        val systemDark =
            (
                context.resources.configuration.uiMode and
                    android.content.res.Configuration.UI_MODE_NIGHT_MASK
                ) ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES
        val effectiveDark =
            when (settingsRepository.appThemeMode.first()) {
                "day" -> false
                "night" -> true
                else -> systemDark
            }
        return when (themeMode) {
            "day" -> dayTheme
            "night" -> nightTheme
            "fixed" -> singleTheme
            else -> if (effectiveDark) nightTheme else dayTheme
        }
    }

    private fun resolveShell(shellPath: String): Shell = if (shellPath == "/system/bin/sh" || shellPath.isEmpty()) {
        Shell.SystemDefault
    } else {
        Shell.Custom(shellPath)
    }

    private fun makeBridgeTheme(resolvedTheme: terminal.emulator.ui.theme.TerminalTheme): BridgeTheme {
        fun colorToInt(color: androidx.compose.ui.graphics.Color): Int = ((color.alpha * 255).toInt() shl 24) or
            ((color.red * 255).toInt() shl 16) or
            ((color.green * 255).toInt() shl 8) or
            (color.blue * 255).toInt()
        val backgroundColor = colorToInt(resolvedTheme.background)
        val foregroundColor = colorToInt(resolvedTheme.foreground)
        val cursor = colorToInt(resolvedTheme.cursor)
        val ansiInts = resolvedTheme.ansi.map(::colorToInt)
        val resolvedSelectionBg = if (resolvedTheme.selectionBg == Color.Transparent) Color(0xFF45475A) else resolvedTheme.selectionBg
        return BridgeTheme(
            name = resolvedTheme.name,
            bg = backgroundColor,
            fg = foregroundColor,
            cursor = cursor,
            selectionBg = colorToInt(resolvedSelectionBg),
            ansi0 = ansiInts[0],
            ansi1 = ansiInts[1],
            ansi2 = ansiInts[2],
            ansi3 = ansiInts[3],
            ansi4 = ansiInts[4],
            ansi5 = ansiInts[5],
            ansi6 = ansiInts[6],
            ansi7 = ansiInts[7],
            ansi8 = ansiInts[8],
            ansi9 = ansiInts[9],
            ansi10 = ansiInts[10],
            ansi11 = ansiInts[11],
            ansi12 = ansiInts[12],
            ansi13 = ansiInts[13],
            ansi14 = ansiInts[14],
            ansi15 = ansiInts[15],
        )
    }

    suspend fun start(
        surface: Surface?,
        width: Int,
        height: Int,
        blitCallback: (() -> Unit)? = null,
    ) {
        synchronized(sessionLock) {
            if ((!stopped && sessions.isNotEmpty()) || starting) return
            stopped = false
            starting = true
        }
        LogUtil.d("Runtime", "start() called: surface=$surface width=$width height=$height")
        LogcatFileWriter.write("Runtime", "start() called: surface=$surface width=$width height=$height")
        val displayW = context.resources.displayMetrics.widthPixels
        val displayH = context.resources.displayMetrics.heightPixels
        val density = context.resources.displayMetrics.density
        LogUtil.d(
            "Runtime",
            "displayMetrics: w=$displayW h=$displayH density=$density",
        )

        val bypassMinSurface = System.getProperty("test.minSurface") != null

        if (!bypassMinSurface && (width <= 0 || height <= 0)) {
            LogUtil.e("Runtime", "start() called with non-positive dimensions, waiting for surfaceChanged")
            starting = false
            return
        }

        val minWidth = (displayW * FONT_SIZE_DISPLAY_RATIO).toInt().coerceIn(FONT_SIZE_MIN_PX, FONT_SIZE_MAX_PX)
        val minHeight = (displayH * FONT_SIZE_HEIGHT_RATIO).toInt().coerceIn(FONT_SIZE_HEIGHT_MIN_PX, FONT_SIZE_HEIGHT_MAX_PX)
        if (!bypassMinSurface && (width < minWidth || height < minHeight)) {
            LogUtil.w(
                "Runtime",
                "start() called with small surface ${width}x$height (display=${displayW}x$displayH min=${minWidth}x$minHeight), waiting for correct surfaceChanged",
            )
            starting = false
            return
        }

        var windowPointer = 0L
        if (surface != null) {
            windowPointer = getNativeWindowPtr(surface)
            LogUtil.d("Runtime", "windowPointer=0x${windowPointer.toString(16)}")
            if (windowPointer == 0L) {
                LogUtil.e("Runtime", "getNativeWindowPtr returned 0 - surface invalid!")
                val fallbackPointer = getNativeWindowPtrReflection(surface)
                LogUtil.d("Runtime", "reflection fallback: 0x${fallbackPointer.toString(16)}")
                if (fallbackPointer == 0L) {
                    LogUtil.e("Runtime", "all methods to get window ptr failed, aborting start")
                    starting = false
                    return
                }
                windowPointer = fallbackPointer
            }
        } else {
            LogUtil.d("Runtime", "no surface — using GPU offscreen rendering path")
        }

        try {
            // Allow test override via system property (no DataStore dependency)
            val testUrl = System.getProperty("test.bootstrapUrl")
            val bootstrapUrl = if (testUrl != null) testUrl else settingsRepository.bootstrapUrl.first()
            if (bootstrapUrl.isNotEmpty()) {
                LogUtil.d("Runtime", "Bootstrap URL set: $bootstrapUrl")
                val downloader = terminal.emulator.installer.BootstrapDownloader(context)
                val installer =
                    terminal.emulator.installer.BootstrapInstaller(
                        prefixDir = java.io.File(context.filesDir, "bootstrap/usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                        stagingDir = java.io.File(context.filesDir, "bootstrap/usr-staging"),
                    )
                val secondStage =
                    terminal.emulator.installer.SecondStageRunner(
                        prefixDir = java.io.File(context.filesDir, "bootstrap/usr"),
                        homeDir = java.io.File(context.filesDir, "home"),
                    )
                val orchestrator = terminal.emulator.installer.BootstrapOrchestrator(downloader, installer, secondStage)
                when (orchestrator.getInstallStatus()) {
                    terminal.emulator.installer.BootstrapOrchestrator.Status.NOT_INSTALLED -> {
                        LogUtil.d("Runtime", "Bootstrap not installed, starting install...")
                        val result = orchestrator.ensureBootstrap(bootstrapUrl)
                        LogUtil.d("Runtime", "Bootstrap result: $result")
                    }

                    terminal.emulator.installer.BootstrapOrchestrator.Status.INSTALLED -> {
                        LogUtil.d("Runtime", "Bootstrap already installed")
                    }

                    else -> {}
                }
            }
            val configStartNs = System.nanoTime()
            val config = buildConfig()
            LogUtil.d(
                "Runtime",
                "buildConfig: fontSizeTenths=${config.font_size_tenths} rows=${config.rows} cols=${config.cols} theme=${config.theme.name} elapsed=${(System.nanoTime() - configStartNs) / 1_000_000}ms",
            )
            val bridgeStartNs = System.nanoTime()
            val bridge = createBridge(config)
            LogUtil.d("Runtime", "bridge created: ${bridge.ping()} elapsed=${(System.nanoTime() - bridgeStartNs) / 1_000_000}ms")

            bridge.setSystemLocale(
                java.util.Locale
                    .getDefault()
                    .toLanguageTag(),
            )
            LogUtil.d("Runtime", "setSystemLocale: ${java.util.Locale.getDefault().toLanguageTag()}")

            val fontsDir = context.filesDir.resolve("fonts")
            fontsDir.apply {
                if (!exists() && !mkdirs()) {
                    Log.w("Runtime", "Failed to create fonts directory: $this")
                }
            }
            bridge.setExtraFontPaths(listOf(fontsDir.absolutePath))

            val sessionId = nextSessionId.getAndIncrement()

            bridge.setNativeWindow(windowPointer, width, height)
            LogUtil.d("Runtime", "setNativeWindow OK: width=$width height=$height")

            val spawnStartNs = System.nanoTime()
            val spawnResult = bridge.spawnTerminal(config.rows, config.cols)
            val spawnElapsedMs = (System.nanoTime() - spawnStartNs) / 1_000_000
            LogUtil.d(
                "Runtime",
                "spawnTerminal: rows=${config.rows} cols=${config.cols} result=$spawnResult elapsed=${spawnElapsedMs}ms",
            )

            try {
                val initialFontFamily = settingsRepository.fontFamily.first()
                val effectiveFont = terminal.emulator.resolveEffectiveFontFamily(initialFontFamily)
                bridge.setFontFamily(effectiveFont)
                bridge.setTheme(config.theme)
                val cursorStyle = settingsRepository.cursorStyle.first()
                bridge.setCursorStyle(cursorStyle)
                val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
                bridge.setCursorBlinkEnabled(cursorBlinkEnabled)
                val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
                bridge.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
                LogUtil.d(
                    "Runtime",
                    "settings applied: fontFamily=$effectiveFont theme=${config.theme.name} cursorStyle=$cursorStyle cursorBlink=$cursorBlinkEnabled cursorSpeed=$cursorBlinkSpeedMs",
                )
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "Failed to apply initial settings (continuing with defaults)", exception)
            }

            val entry =
                SessionEntry(
                    id = sessionId,
                    bridge = bridge,
                    renderThreadRef = null,
                    running = true,
                )
            sessions[sessionId] = entry
            activeSessionId = sessionId
            if (blitCallback != null) {
                entry.blitCallback = blitCallback
            }
            // First render: problematic GPUs (Mali-G57 w/ missing SURFACE_VIEW_FORMATS)
            // can hang get_current_texture() indefinitely. The previous approach of
            // spawning a daemon thread to call bridge.render() caused mutex starvation —
            // the daemon would acquire the surface Mutex and hang, permanently blocking
            // the real render thread. Instead, signal the render loop to produce the
            // first frame via forceRenderRequested, which will be picked up by the
            // real render thread once it starts.
            entry.forceRenderRequested = true
            startRenderThread(entry)

            _state.value =
                RuntimeState(
                    isRunning = true,
                    rows = config.rows,
                    cols = config.cols,
                    activeSessionId = sessionId,
                    sessionIds = listOf(sessionId),
                )
            LogUtil.d(
                "Runtime",
                "session $sessionId config: rows=${config.rows} cols=${config.cols} fontSizeTenths=${config.font_size_tenths}",
            )
            LogUtil.d("Runtime", "session $sessionId started")
            startForegroundServiceIfNeeded()
            startRenderMonitor()
        } catch (exception: Exception) {
            LogUtil.e("Runtime", "Failed to start terminal", exception)
            LogcatFileWriter.write("Runtime", "FAILED to start terminal: ${exception.message}\n${exception.stackTraceToString()}")
        } finally {
            starting = false
        }
    }

    // Architecture Note: each session currently creates its own bridge with a
    // separate GPU surface (surface.rs owns the wgpu pipeline per ANativeWindow).
    // Sharing a single pre-initialized GPU pipeline across sessions is a possible
    // future optimization (could cut session-creation time) but is not yet
    // implemented — do not assume a shared pipeline exists.
    suspend fun createSession(
        surface: Surface,
        width: Int,
        height: Int,
    ): Long {
        if (width <= 0 || height <= 0) {
            LogUtil.e("Runtime", "createSession: invalid dimensions ${width}x$height")
            return -1L
        }
        if (!surface.isValid) {
            LogUtil.e("Runtime", "createSession: surface is not valid")
            return -1L
        }
        val nextId = (sessions.keys.maxOrNull() ?: 0L) + 1
        LogUtil.d("Runtime", "createSession() id=$nextId")

        try {
            val configStartNs = System.nanoTime()
            val config = buildConfig()
            val bridgeStartNs = System.nanoTime()
            val bridge = createBridge(config)
            LogUtil.d(
                "Runtime",
                "createSession id=$nextId configElapsed=${(System.nanoTime() - configStartNs) / 1_000_000}ms bridgeElapsed=${(System.nanoTime() - bridgeStartNs) / 1_000_000}ms",
            )
            bridge.setSystemLocale(
                java.util.Locale
                    .getDefault()
                    .toLanguageTag(),
            )

            try {
                val initialFontFamily = settingsRepository.fontFamily.first()
                val effectiveFont = terminal.emulator.resolveEffectiveFontFamily(initialFontFamily)
                bridge.setFontFamily(effectiveFont)
                bridge.setTheme(config.theme)
                val cursorStyle = settingsRepository.cursorStyle.first()
                bridge.setCursorStyle(cursorStyle)
                val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
                bridge.setCursorBlinkEnabled(cursorBlinkEnabled)
                val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
                bridge.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "Failed to apply settings to new session (continuing with defaults)", exception)
            }


            val entry =
                SessionEntry(
                    id = nextId,
                    bridge = bridge,
                    renderThreadRef = null,
                    running = false,
                )
            synchronized(sessionLock) {
                sessions[nextId] = entry

                try {
                    switchSessionInternal(
                        nextId,
                        surface,
                        width,
                        height,
                        needsSpawn = true,
                        spawnRows = config.rows,
                        spawnCols = config.cols,
                    )
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "Failed to switch to new session $nextId, rolling back", exception)
                    sessions.remove(nextId)
                    throw exception
                }

                updateState()
            }
            LogUtil.d("Runtime", "session $nextId created and activated")
            return nextId
        } catch (exception: Exception) {
            LogUtil.e("Runtime", "Failed to create session $nextId", exception)
            LogcatFileWriter.write(
                "Runtime",
                "FAILED to create session $nextId: ${exception.message}\n${exception.stackTraceToString()}",
            )
            return -1L
        }
    }

    fun switchSession(
        id: Long,
        surface: Surface,
        width: Int,
        height: Int,
    ) {
        switchSessionInternal(id, surface, width, height)
        updateState()
    }

    private fun switchSessionInternal(
        id: Long,
        surface: Surface,
        width: Int,
        height: Int,
        needsSpawn: Boolean = false,
        spawnRows: Int = 24,
        spawnCols: Int = 80,
    ) {
        synchronized(sessionLock) {
            val target =
                sessions[id] ?: run {
                    LogUtil.e("Runtime", "switchSession: session $id not found")
                    return
                }
            if (id == activeSessionId && !needsSpawn) return
            if (stopped) {
                LogUtil.e("Runtime", "switchSession: runtime is stopped, aborting")
                return
            }

            val windowPointer = getNativeWindowPtr(surface)
            if (windowPointer == 0L) {
                LogUtil.e("Runtime", "switchSession: failed to get native window ptr")
                return
            }

            if (!surface.isValid) {
                LogUtil.e("Runtime", "switchSession: surface is no longer valid, aborting")
                return
            }

            val current = sessions[activeSessionId]
            if (current != null) {
                try {
                    stopRenderThread(current)
                    // Release the old bridge's GPU surface before the new bridge
                    // creates its own on the same ANativeWindow. This avoids
                    // VK_ERROR_NATIVE_WINDOW_IN_USE_KHR from the Vulkan driver
                    // when two wgpu surfaces share the same ANativeWindow.
                    current.bridge?.releaseGpuSurface()
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "switchSession: error stopping current session", exception)
                }
            }

            try {
                if (stopped) {
                    LogUtil.e("Runtime", "switchSessionInternal: runtime stopped, aborting")
                    return
                }
                if (needsSpawn) {
                    target.bridge?.setNativeWindow(windowPointer, width, height)
                    val spawnResult = target.bridge?.spawnTerminal(spawnRows, spawnCols)
                    LogUtil.d(
                        "Runtime",
                        "switchSessionInternal spawnTerminal for session $id rows=$spawnRows cols=$spawnCols result=$spawnResult",
                    )
                } else {
                    target.bridge?.setNativeWindow(windowPointer, width, height)
                    // Always update the GPU surface after setNativeWindow.
                    // If releaseGpuSurface was called on this bridge during a
                    // previous session switch, the wgpu surface is gone and must
                    // be recreated from the current ANativeWindow via updateNativeWindow.
                    target.bridge?.updateNativeWindow(windowPointer, width, height)
                }
                target.running = true
                // Render the new session's first frame SYNCHRONOUSLY before the
                // event-driven render thread starts, so the reconfigured
                // swapchain shows real content immediately instead of a brief
                // blank/clear frame. Reconfiguring the swapchain (above) discards
                // the previous session's backbuffer, and the render thread's
                // first frame is only presented after OS thread scheduling —
                // that gap is exactly the blank flash. Presenting here closes it.
                // The render thread takes over right after (it re-renders once,
                // then latches on the RENDER_LATCH_IDLE_TIMEOUT_NANOS cadence, so
                // the event-driven model is preserved).
                try {
                    // The GPU surface may not be fully configured immediately after
                    // spawnTerminal/setNativeWindow (a transient race on shared
                    // ANativeWindow). Retry the first synchronous render a few times
                    // with a short delay so we present real content instead of
                    // starting a render thread on a not-yet-ready surface (which would
                    // block and trip the hang watchdog).
                    var initialRender = target.bridge?.render() ?: 0
                    var attempts = 1
                    while (initialRender < 0 && attempts < RENDER_INITIAL_RETRY_MAX) {
                        Thread.sleep(RENDER_INITIAL_RETRY_DELAY_MS)
                        initialRender = target.bridge?.render() ?: 0
                        attempts++
                    }
                    LogUtil.d(
                        "Runtime",
                        "switchSession: initial render for session $id result=$initialRender (attempts=$attempts)",
                    )
                    target.forceRenderRequested = true
                    target.notifyRender()
                } catch (exception: Exception) {
                    LogUtil.e(
                        "Runtime",
                        "switchSession: initial render failed for session $id",
                        exception,
                    )
                }
                startRenderThread(target)
                activeSessionId = id
                // Sync the native ACTIVE_SESSION_ID so pollEvent/process_output
                // operate on the new session. Without this, all sessions except
                // the first share one native-active session and multi-session
                // output/exit detection silently breaks.
                try {
                    terminal.emulator.bridge.NativeBridge.switchSession(id)
                } catch (exception: Exception) {
                    LogUtil.e("Runtime", "switchSession: native switchSession failed for session $id", exception)
                }
                target.bridge?.let { syncGridDimensions(it) }
                LogUtil.d("Runtime", "switched to session $id")
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "switchSession: setNativeWindow failed for session $id", exception)
            }
        }
    }

    fun closeSession(
        id: Long,
        surface: Surface? = null,
        width: Int = 0,
        height: Int = 0,
    ) {
        synchronized(sessionLock) {
            val entry = sessions[id] ?: return
            LogUtil.d("Runtime", "closeSession($id)")

            // Stop the render thread for any session, active or not.
            // Non-active sessions still have live render threads started by
            // resumeRendering(); leaking them keeps polling the global event
            // queue and consuming other sessions' events.
            if (entry.running) {
                stopRenderThread(entry)
            }
            entry.bridge?.releaseGpuSurface()
            entry.bridge?.close()
            sessions.remove(id)
            terminal.emulator.service.TerminalForegroundService
                .updateSessionCount(context, sessions.size)

            // If we closed the active session, switch to another
            if (id == activeSessionId) {
                val remaining = sessions.keys.sorted()
                if (remaining.isNotEmpty()) {
                    val newId = remaining.last()
                    activeSessionId = newId
                    val newEntry =
                        sessions[newId] ?: run {
                            LogUtil.w("Runtime", "closeSession: new active session $newId already removed")
                            activeSessionId = 0L
                            updateState()
                            return
                        }
                    newEntry.running = true
                    val bridge =
                        newEntry.bridge ?: run {
                            LogUtil.w("Runtime", "closeSession: new active session $newId has no bridge")
                            activeSessionId = 0L
                            updateState()
                            return
                        }
                    if (surface != null && width > 0 && height > 0) {
                        val windowPointer = getNativeWindowPtr(surface)
                        if (windowPointer != 0L) {
                            try {
                                bridge.setNativeWindow(windowPointer, width, height)
                                bridge.updateNativeWindow(windowPointer, width, height)
                            } catch (exception: Exception) {
                                LogUtil.e("Runtime", "closeSession: failed to rebind GPU for session $newId", exception)
                            }
                        }
                    }
                    startRenderThread(newEntry)
                    bridge.let { syncGridDimensions(it) }
                    LogUtil.d("Runtime", "closeSession: restarted render for session $newId")
                } else {
                    activeSessionId = 0L
                }
            }
            updateState()
        }
    }

    suspend fun applySettings() {
        val config = buildConfig()
        val fontFamily = settingsRepository.fontFamily.first()
        val effectiveFontFamily = terminal.emulator.resolveEffectiveFontFamily(fontFamily)
        val cursorStyle = settingsRepository.cursorStyle.first()
        val cursorBlinkEnabled = settingsRepository.cursorBlink.first()
        val cursorBlinkSpeedMs = settingsRepository.cursorSpeed.first()
        sessions.values.forEach { entry ->
            entry.bridge?.setFontSize(config.font_size_tenths)
            entry.bridge?.setFontFamily(effectiveFontFamily)
            entry.bridge?.setTheme(config.theme)
            entry.bridge?.setCursorStyle(cursorStyle)
            entry.bridge?.setCursorBlinkEnabled(cursorBlinkEnabled)
            entry.bridge?.setCursorBlinkSpeedMs(cursorBlinkSpeedMs)
            entry.bridge?.resize(config.rows, config.cols)
            entry.notifyRender()
        }
    }

    suspend fun applyFontSettings() {
        val fontSizeTenths = computeFontSizeTenths()
        val fontFamily = settingsRepository.fontFamily.first()
        val effectiveFontFamily = terminal.emulator.resolveEffectiveFontFamily(fontFamily)
        LogUtil.d(
            "Runtime",
            "applyFontSettings: fontFamily='$fontFamily' effective='$effectiveFontFamily' fontSizeTenths=$fontSizeTenths sessions=${sessions.size}",
        )
        sessions.values.forEach { entry ->
            try {
                val familyResult = entry.bridge?.setFontFamily(effectiveFontFamily)
                LogUtil.d("Runtime", "setFontFamily result: $familyResult")
                entry.bridge?.setFontSizeInPlace(fontSizeTenths)
                entry.bridge?.let { syncGridDimensions(it) }
            } catch (exception: Exception) {
                LogUtil.e("Runtime", "applyFontSettings failed for session", exception)
            }
        }
    }

    fun loadFontFile(path: String): String? {
        val entry = sessions.values.firstOrNull() ?: return null
        return entry.bridge?.loadFontFile(path)
    }

    fun writeToPty(data: ByteArray): Boolean {
        val entry = sessions[activeSessionId]
        if (entry != null && entry.running) {
            val written = entry.bridge?.writeToPty(data) ?: false
            entry.notifyRender()
            return written
        }
        LogUtil.w("Runtime", "writeToPty: no active running session to receive write")
        return false
    }

    fun bridge(): Bridge? = sessions[activeSessionId]?.bridge

    fun activeSessionBridge(id: Long): Bridge? = sessions[id]?.bridge

    fun focusChange(focused: Boolean) {
        sessions.forEach { (_, entry) ->
            entry.bridge?.focusEvent(focused)
        }
    }

    fun currentCwd(): String {
        val entry = sessions[activeSessionId] ?: return ""
        return entry.bridge?.cwd() ?: ""
    }

    fun currentSessionIds(): List<Long> = sessions.keys.sorted()

    fun currentActiveSessionId(): Long = activeSessionId



    fun stop() {
        stopRenderMonitor()
        synchronized(stopLock) {
            if (stopped) return
            stopped = true
        }
        synchronized(sessionLock) {
            sessions.values.forEach { entry ->
                stopRenderThread(entry)
                entry.bridge?.close()
            }
            sessions.clear()
            activeSessionId = 0L
        }
        _state.value = RuntimeState()
        stopForegroundService()
    }

    fun pauseRendering() {
        synchronized(sessionLock) {
            sessions.values.forEach { entry ->
                if (entry.running) {
                    stopRenderThread(entry)
                    entry.running = false
                    LogUtil.d("Runtime", "pauseRendering: session ${entry.id} stopped")
                }
            }
        }
    }

    fun resumeRendering() {
        synchronized(sessionLock) {
            sessions.values.forEach { entry ->
                if (!entry.running && entry.bridge != null) {
                    try {
                        entry.running = true
                        startRenderThread(entry)
                        LogUtil.d("Runtime", "resumeRendering: session ${entry.id} restarted")
                    } catch (exception: Exception) {
                        LogUtil.e("Runtime", "resumeRendering failed for session ${entry.id}", exception)
                    }
                }
            }
        }
        startRenderMonitor()
    }

    fun setSelection(
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean,
        mode: Byte = 0,
    ) {
        LogUtil.d("Runtime", "setSelection: start=($startRow,$startCol) end=($endRow,$endCol) active=$hasSelection mode=$mode")
        selectionState.set(SelectionStateSnapshot(startRow, startCol, endRow, endCol, hasSelection, mode))
        val entry = sessions[activeSessionId]
        entry?.bridge?.setSelection(startRow, startCol, endRow, endCol, hasSelection, mode)
        entry?.notifyRender()
    }

    fun expandAndSetSelection(
        row: Int,
        col: Int,
        mode: Byte = 0,
    ): Pair<Pair<Int, Int>, Pair<Int, Int>>? {
        LogUtil.d("Runtime", "expandAndSetSelection: row=$row col=$col mode=$mode")
        val entry = sessions[activeSessionId] ?: return null
        val bounds = entry.bridge?.expandAndSetSelection(row, col, mode) ?: return null
        val (start, end) = bounds
        selectionState.set(SelectionStateSnapshot(start.first, start.second, end.first, end.second, true, mode))
        entry.notifyRender()
        return bounds
    }

    /**
     * Drag-grow: move one endpoint of the active selection to [anchorRow]/[anchorCol] while keeping
     * the opposite endpoint fixed. Routes through the core [Selection] expansion on the Rust side so
     * the result matches long-press. [handleSide] is 0 for the start endpoint, 1 for the end.
     */
    fun setSelectionEndpoint(
        handleSide: Byte,
        anchorRow: Int,
        anchorCol: Int,
        otherRow: Int,
        otherCol: Int,
        mode: Byte,
        originRow: Int,
        originCol: Int,
    ): Pair<Pair<Int, Int>, Pair<Int, Int>>? {
        LogUtil.d(
            "Runtime",
            "setSelectionEndpoint: side=$handleSide anchor=($anchorRow,$anchorCol) " +
                "other=($otherRow,$otherCol) mode=$mode origin=($originRow,$originCol)",
        )
        val entry = sessions[activeSessionId] ?: return null
        entry.bridge?.setSelectionEndpoint(
            handleSide,
            anchorRow,
            anchorCol,
            hasSelection = true,
        )
        val bounds = Pair(
            Pair(anchorRow, anchorCol),
            Pair(otherRow, otherCol),
        )
        selectionState.set(
            SelectionStateSnapshot(
                bounds.first.first,
                bounds.first.second,
                bounds.second.first,
                bounds.second.second,
                true,
                mode,
            ),
        )
        entry.notifyRender()
        return bounds
    }

    fun resize(
        rows: Int,
        cols: Int,
    ) {
        val entry = sessions[activeSessionId] ?: return
        entry.bridge?.resize(rows, cols)
        _state.value = _state.value.copy(rows = rows, cols = cols)
        entry.notifyRender()
    }

    fun recomputeGrid(
        width: Int,
        height: Int,
    ) {
        val bridge = sessions[activeSessionId]?.bridge ?: return
        bridge.recomputeGrid(width, height)
        syncGridDimensions(bridge)
    }

    fun updateNativeWindow(
        windowPointer: Long,
        width: Int,
        height: Int,
    ) {
        val entry = sessions[activeSessionId] ?: return
        try {
            entry.bridge?.updateNativeWindow(windowPointer, width, height)
            entry.bridge?.let { syncGridDimensions(it) }
            if (entry.renderThreadRef?.isAlive != true) {
                LogUtil.d("Runtime", "updateNativeWindow: render thread dead, restarting for session ${entry.id}")
                startRenderThread(entry)
            } else {
                entry.forceRenderRequested = true
                java.util.concurrent.locks.LockSupport
                    .unpark(entry.renderThreadRef)
            }
        } catch (exception: Exception) {
            LogUtil.e("Runtime", "updateNativeWindow failed", exception)
        }
    }

    private fun syncGridDimensions(bridge: Bridge) {
        val packed = bridge.getGridRowsColsPacked()
        val rows = (packed shr 32).toInt()
        val cols = packed.toInt()
        cellWidth = bridge.getCellWidth()
        cellHeight = bridge.getCellHeight()
        val sizeChanged = rows != _state.value.rows || cols != _state.value.cols
        if (rows > 0 && cols > 0 && sizeChanged) {
            _state.value = _state.value.copy(rows = rows, cols = cols)
        }
    }

    fun setBlitCallback(callback: () -> Unit) {
        val entry = sessions[activeSessionId] ?: return
        entry.blitCallback = callback
    }

    fun destroy() {
        stop()
        scope.cancel()
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4: Render loop (extract target)
    // ══════════════════════════════════════════════════════════════════════

    private fun startRenderThread(entry: SessionEntry) {
        entry.renderWatchDog?.stop()
        entry.renderWatchDog = null
        entry.renderThreadExited = false
        entry.running = false
        entry.renderSignaled.set(false)
        val oldThread = entry.renderThreadRef
        entry.renderThreadRef = null
        oldThread?.let { t ->
            t.interrupt()
            t.join(THREAD_JOIN_TIMEOUT_MS)
            if (t.isAlive) {
                LogUtil.w(
                    "Runtime",
                    "session ${entry.id} previous render thread still alive after join — forcing new thread anyway",
                )
            }
        }
        val generation = renderGeneration.incrementAndGet()
        entry.running = true
        val renderThread =
            Thread({
                var diagCount = 0
                var consecutiveErrors = 0
                var lastScrollOffset = Int.MAX_VALUE
                var lastSelection = SelectionStateSnapshot(0, 0, 0, 0, false, 0)
                LogUtil.d("Runtime", "render thread started for session ${entry.id} generation=$generation")
                // First iteration always processes ghostty output. Subsequent
                // iterations skip output on blink/force-rendered frames to avoid
                // the ~50ms per-frame ghostty tick when there's no new PTY data.
                var shouldSkipOutput = false
                while (entry.running && renderGeneration.get() == generation) {
                    try {
                        val bridge = entry.bridge ?: break
                        val selectionSnapshot = selectionState.get()
                        if (selectionSnapshot != lastSelection) {
                            bridge.setSelection(
                                selectionSnapshot.startRow,
                                selectionSnapshot.startCol,
                                selectionSnapshot.endRow,
                                selectionSnapshot.endCol,
                                selectionSnapshot.hasSelection,
                                selectionSnapshot.mode,
                            )
                            lastSelection = selectionSnapshot
                        }
                        val currentScrollOffset = entry.scrollOffset
                        if (currentScrollOffset != lastScrollOffset) {
                            bridge.setScrollOffset(currentScrollOffset)
                            lastScrollOffset = currentScrollOffset
                        }
                        entry.lastRenderStart = System.nanoTime()
                        val count = bridge.render(shouldSkipOutput)
                        if (count < 0) {
                            // Transient render error (surface not ready, snapshot unavailable, etc.)
                            // These resolve on their own; don't count them toward the fatal limit.
                            if (consecutiveErrors == 0) {
                                LogUtil.w("Runtime", "session ${entry.id} transient render error code=$count")
                            }
                            consecutiveErrors++
                            if (consecutiveErrors > RENDER_MAX_TRANSIENT_ERRORS) {
                                LogUtil.e(
                                    "Runtime",
                                    "session ${entry.id} too many transient render errors ($consecutiveErrors), stopping render thread",
                                )
                                break
                            }
                            // Adaptive backoff: 50ms for first 10, then 200ms
                            val sleepMs = if (consecutiveErrors > 10) RENDER_ERROR_BACKOFF_MS else RENDER_ERROR_SLEEP_MS
                            Thread.sleep(sleepMs)
                        } else {
                            if (consecutiveErrors > 0) {
                                LogUtil.i(
                                    "Runtime",
                                    "session ${entry.id} recovered after $consecutiveErrors errors",
                                )
                            }
                            consecutiveErrors = 0
                            try {
                                val poll = bridge.pollAll()
                                if (poll.bel) {
                                    bellToneGenerator.startTone(BEL_TONE_TYPE, BEL_TONE_DURATION_MILLIS)
                                }
                                if (poll.notification != null) {
                                    val (title, body) = poll.notification
                                    val toastText = if (title.isNotEmpty()) "$title: $body" else body
                                    Handler(Looper.getMainLooper()).post {
                                        android.widget.Toast
                                            .makeText(context, toastText, android.widget.Toast.LENGTH_LONG)
                                            .show()
                                    }
                                    terminal.emulator.ui
                                        .TerminalNotificationHelper(context)
                                        .showNotification(title, body)
                                }
                                if (poll.clipboard != null) {
                                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                    val clipData = ClipData.newPlainText("terminal clipboard", poll.clipboard)
                                    clipboard.setPrimaryClip(clipData)
                                }
                                if (poll.exit) {
                                    // Full cleanup (bridge close, session removal,
                                    // state update) happens here; the render monitor
                                    // skips !running entries so it would never reap
                                    // an exited session.
                                    handleSessionExit(entry, poll.exitCode)
                                    break
                                }
                                poll.dialog?.let { request ->
                                    LogUtil.i("Runtime", "MCP dialog request session=${request.sessionId} type=${request.dialogType}")
                                    dialogRequestHandler?.invoke(
                                        request.sessionId,
                                        request.requestId,
                                        request.dialogType,
                                        request.title,
                                        request.message,
                                        request.options,
                                    )
                                }
                                poll.pickFile?.let { request ->
                                    LogUtil.i("Runtime", "MCP pick_file request session=${request.sessionId}")
                                    pickFileRequestHandler?.invoke(
                                        request.sessionId,
                                        request.requestId,
                                        request.startingPath,
                                        request.filter,
                                    )
                                }
                            } catch (exception: Exception) {
                                LogUtil.e(
                                    "Runtime",
                                    "pollAll failed for session ${entry.id}; deferred events dropped",
                                    exception,
                                )
                            }
                            diagCount++
                            if (diagCount == 1) {
                                LogUtil.d("Runtime", "session ${entry.id} first render OK")
                            }
                            if (diagCount % RENDER_DIAGNOSTIC_FREQUENCY == 0) {
                                val title =
                                    try {
                                        bridge.getActiveSessionTitle()
                                    } catch (exception: Exception) {
                                        LogUtil.e("Runtime", "title query failed", exception)
                                        ""
                                    }
                                if (title.isNotEmpty() && title != _state.value.title) {
                                    _state.value = _state.value.copy(title = title)
                                }
                            }
                            entry.lastRenderDone = System.nanoTime()
                            if (!entry.forceRenderRequested) {
                                val idleNanos = System.nanoTime() - entry.lastSignalNanos
                                val timeoutNanos =
                                    if (idleNanos > RENDER_IDLE_THRESHOLD_NANOS) {
                                        RENDER_LATCH_IDLE_TIMEOUT_NANOS
                                    } else {
                                        RENDER_LATCH_TIMEOUT_NANOS
                                    }
                                if (!entry.renderSignaled.get() && !entry.forceRenderRequested) {
                                    val timeoutMs = timeoutNanos / 1_000_000L
                                    bridge.waitOutput(timeoutMs)
                                    if (Thread.interrupted()) throw InterruptedException()
                                }
                                shouldSkipOutput = false
                                entry.renderSignaled.set(false)
                            } else {
                                entry.forceRenderRequested = false
                                shouldSkipOutput = true
                            }
                        }
                    } catch (exception: InterruptedException) {
                        // The render thread was interrupted during shutdown
                        // (session switch / runtime stop). This is an expected
                        // signal, not a render failure — exit the loop cleanly.
                        Thread.currentThread().interrupt()
                        break
                    } catch (exception: Exception) {
                        consecutiveErrors++
                        if (consecutiveErrors == 1) {
                            LogUtil.e("Runtime", "session ${entry.id} first render exception", exception)
                        } else if (consecutiveErrors % RENDER_ERROR_LOG_FREQUENCY == 0) {
                            LogUtil.e("Runtime", "session ${entry.id} render exception (x$consecutiveErrors)", exception)
                        }
                        if (consecutiveErrors > RENDER_MAX_CONSECUTIVE_ERRORS) {
                            LogUtil.e(
                                "Runtime",
                                "session ${entry.id} too many render exceptions ($consecutiveErrors), stopping render thread",
                                exception,
                            )
                            break
                        }
                        Thread.sleep(RENDER_ERROR_SLEEP_MS)
                    }
                }
                entry.renderThreadExited = true
                LogUtil.d("Runtime", "render thread stopped for session ${entry.id}")
            }, "Render-${entry.id}").apply {
                isDaemon = true
            }
        entry.renderThreadRef = renderThread
        renderThread.start()
        entry.renderWatchDog =
            RenderWatchDog(
                getStart = { entry.lastRenderStart },
                getDone = { entry.lastRenderDone },
                isRunning = { entry.running && !entry.renderThreadExited && activeSessionId == entry.id },
                onHangDetected = {
                    LogUtil.e("Runtime", "session ${entry.id} render thread hung (>${RENDER_HANG_TIMEOUT_NANOS / 1_000_000L}s)")
                    LogcatFileWriter.write(
                        "Runtime",
                        "session ${entry.id} render hang detected — marking thread for restart",
                    )
                    // Mark the thread as dead. The render monitor (checkSessions)
                    // will detect this and restart the thread with exponential backoff.
                    // This avoids killing the entire process for a GPU hang.
                    entry.renderThreadExited = true
                },
                hangTimeoutNanos = RENDER_HANG_TIMEOUT_NANOS,
            ).also { it.start() }
    }

    private fun stopRenderThread(entry: SessionEntry): Boolean {
        entry.renderWatchDog?.stop()
        entry.renderWatchDog = null
        entry.running = false
        val thread = entry.renderThreadRef
        entry.renderThreadRef = null
        entry.renderSignaled.set(false)
        thread?.let { t ->
            t.interrupt()
            t.join(THREAD_JOIN_TIMEOUT_MS)
            if (t.isAlive) {
                LogUtil.e("Runtime", "session ${entry.id} render thread still alive after join — possibly hung")
                return false
            }
        }
        return true
    }

    private fun updateState() {
        val currentTitle = sessions[activeSessionId]?.bridge?.getActiveSessionTitle() ?: _state.value.title
        _state.value =
            RuntimeState(
                isRunning = sessions.isNotEmpty(),
                title = currentTitle.ifEmpty { _state.value.title },
                rows = _state.value.rows,
                cols = _state.value.cols,
                activeSessionId = activeSessionId,
                sessionIds = sessions.keys.sorted(),
            )
    }

    fun getNativeWindowPtr(surface: Surface): Long = try {
        if (!terminal.emulator.bridge.NativeWindow
                .isNativeLoaded()
        ) {
            LogUtil.w("Runtime", "Native lib not loaded, using reflection fallback")
            return getNativeWindowPtrReflection(surface)
        }
        terminal.emulator.bridge.NativeWindow
            .getNativeWindowPtr(surface)
    } catch (exception: Throwable) {
        LogUtil.w("Runtime", "JNI getNativeWindowPtr not available, falling back to mNativeObject reflection", exception)
        getNativeWindowPtrReflection(surface)
    }

    private fun getNativeWindowPtrReflection(surface: Surface): Long {
        try {
            val method = surface.javaClass.getMethod("getNativeWindow")
            return (method.invoke(surface) as? Long) ?: 0L
        } catch (exception: Exception) {
            LogUtil.w("Runtime", "getNativeWindow method reflection failed", exception)
        }
        try {
            val field = surface.javaClass.getDeclaredField("mNativeObject")
            field.isAccessible = true
            return field.getLong(surface)
        } catch (exception: Exception) {
            LogUtil.w("Runtime", "mNativeObject field reflection failed", exception)
        }
        LogUtil.e("Runtime", "All methods to get native window pointer failed")
        return 0L
    }

    fun onSurfaceDestroyed() {
        for (entry in sessions.values) {
            entry.bridge?.setRenderPaused(true)
        }
    }

    fun releaseAllGpuSurfaces() {
        // Surface destroyed: pause render threads cleanly instead of marking
        // them dead. Marking renderThreadExited on live threads makes the
        // monitor treat them as crashed and eventually close the sessions
        // after RENDER_MAX_RESTART_ATTEMPTS (~20s background).
        pauseRendering()
    }
}
