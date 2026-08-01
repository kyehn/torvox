package terminal.emulator

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.ColorDrawable
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.ActionMode
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.lifecycleScope
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.ui.SettingsScreen
import terminal.emulator.ui.TerminalScreen
import java.io.BufferedWriter
import java.io.File
import java.io.FileWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    companion object {
        private const val TAG = "MainActivity"
        private const val LOGCAT_RETRY_DELAY_MS = 5_000

        /**
         * Matches the tag column of `logcat -v time` output:
         * `MM-DD HH:MM:SS.mmm  PID  TID T TagName: message`
         */
        private val LOG_TAG_PATTERN =
            Regex("""\s+[A-Z]\s+([^:]+):""")
    }

    @Inject
    @Suppress("LateinitUsage") // Dagger injection
    lateinit var runtime: TerminalRuntime

    private var logFile: File? = null
    private var logWriter: BufferedWriter? = null

    @Volatile private var logcatThread: Thread? = null

    @Volatile private var logcatProcess: Process? = null
    private fun startLogcatThread() {
        if (logcatThread?.isAlive == true) return
        logcatThread = Thread({
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                Log.w(
                    "T",
                    "Logcat capture not supported on Android 11+ — READ_LOGS permission unavailable; this path is expected to fail",
                )
                return@Thread
            }
            while (!Thread.currentThread().isInterrupted) {
                try {
                    val process = Runtime.getRuntime().exec(arrayOf("logcat", "-v", "time", "*:D"))
                    synchronized(logLock) {
                        // stopFileLogging() may have run between exec() and
                        // this assignment (activity rotation): the new logcat
                        // process must be destroyed, not leaked.
                        if (logcatThread?.isAlive != true || Thread.currentThread().isInterrupted) {
                            process.destroy()
                            return@Thread
                        }
                        logcatProcess = process
                    }
                    val reader = process.inputStream.bufferedReader()
                    for (line in reader.lineSequence()) {
                        // Match the logcat tag column (`... 12345 12345 T TagName: message`)
                        // for the tags we care about. Only the exact tag
                        // column is matched — a substring check would match
                        // message bodies and persist unrelated (possibly
                        // sensitive) lines.
                        val tag =
                            LOG_TAG_PATTERN
                                .find(line)
                                ?.groupValues
                                ?.getOrNull(1)
                        if (tag == "TerminalSurface" || tag == "TerminalRuntime" || tag == "AndroidRuntime") {
                            val timestamp = SimpleDateFormat("HH:mm:ss.SSS", Locale.US).format(Date())
                            synchronized(logLock) {
                                logWriter?.write("$timestamp $line\n")
                                logWriter?.flush()
                            }
                        }
                    }
                    Log.w("T", "Logcat stream ended, restarting in 5s")
                    Thread.sleep(LOGCAT_RETRY_DELAY_MS.toLong())
                } catch (e: InterruptedException) {
                    Log.w("T", "Logcat thread interrupted, stopping")
                    Thread.currentThread().interrupt()
                    break
                } catch (e: Exception) {
                    Log.e("T", "Logcat capture failed, retrying in 5s: ${e.message}")
                    try {
                        Thread.sleep(LOGCAT_RETRY_DELAY_MS.toLong())
                    } catch (e: InterruptedException) {
                        Log.w("T", "Logcat sleep interrupted, stopping")
                        Thread.currentThread().interrupt()
                        break
                    }
                }
            }
        }, "FileLog").apply {
            isDaemon = true
        }.also { it.start() }
    }

    private val logLock = Any()

    private fun initFileLogging() {
        try {
            val logDir = getDir("logs", Context.MODE_PRIVATE)
            logDir.mkdirs()
            val timestamp = SimpleDateFormat("yyyyMMdd_HHmmss", Locale.US).format(Date())
            val logFilePath = File(logDir, "term_$timestamp.log")
            logFile = logFilePath
            logWriter = BufferedWriter(FileWriter(logFilePath, true), 8192)
            startLogcatThread()
            Log.d("T", "File logging: ${logFilePath.absolutePath}")
        } catch (exception: Exception) {
            Log.e("T", "Failed to init file logging", exception)
        }
    }

    private fun stopFileLogging() {
        try {
            synchronized(logLock) {
                // destroy() closes the process's stdin/stdout/stderr, which
                // unblocks readLine() — interrupt() alone cannot interrupt a
                // thread blocked on stream IO, so without this the old logcat
                // process leaks on every activity recreation (rotation).
                logcatProcess?.destroy()
                logcatProcess = null
                logcatThread?.interrupt()
                logcatThread = null
                logWriter?.close()
                logWriter = null
            }
        } catch (exception: Exception) {
            Log.w(TAG, "stopFileLogging failed", exception)
        }
    }

    private fun tryUnregisterReceiver(
        receiver: BroadcastReceiver,
        name: String,
    ) {
        try {
            unregisterReceiver(receiver)
        } catch (exception: IllegalArgumentException) {
            Log.w(TAG, "$name not registered", exception)
        }
    }

    private val terminalDumpReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(
                context: Context,
                intent: Intent,
            ) {
                Thread {
                    try {
                        val bridge = runtime.bridge()
                        val text =
                            if (bridge != null) {
                                bridge.getTerminalText() ?: "(empty)"
                            } else {
                                "(no active session)"
                            }
                        val file = java.io.File(context.cacheDir, "terminal_dump.txt")
                        file.writeText(text)
                        Log.d("T", "Terminal dump: ${file.absolutePath} (${text.length} chars)")
                    } catch (exception: Exception) {
                        Log.e("T", "Terminal dump failed", exception)
                    }
                }.apply {
                    isDaemon = true
                    start()
                }
            }
        }

    private val inputReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(
                context: Context,
                intent: Intent,
            ) {
                val text = intent.getStringExtra("text") ?: return
                terminalViewModel.clearSelection()
                Thread {
                    try {
                        // Never log the input payload: it may contain
                        // passwords/tokens and lands in the persisted logcat
                        // dump (term_*.log). Length only.
                        Log.d("T", "Input received (len=${text.length})")
                        val processed =
                            text
                                .replace("\\n", "\n")
                                .replace("\\r", "\r")
                                .replace("\\t", "\t")
                        val data = (processed + "\n").byteInputStream().readBytes()
                        runtime.writeToPty(data)
                        Log.d("T", "Input sent: ${data.size} bytes")
                    } catch (exception: Exception) {
                        Log.e("T", "Input failed", exception)
                    }
                }.apply {
                    isDaemon = true
                    start()
                }
            }
        }

    private var previousNightMode: Int? = null

    // In-flight pick_file requests keyed by (sessionId, requestId). Multiple
    // concurrent MCP pick_file calls would otherwise overwrite a single
    // slot, leaving the earlier tool call hanging until its 300s timeout.
    private val pendingPickFileRequests =
        java.util.concurrent.ConcurrentHashMap<Pair<Long, Long>, String>()

    // Launches whose ActivityResult callback is still outstanding, in launch
    // order. ActivityResult delivers exactly one callback per launch, in
    // order, so the queue head identifies the request being answered. A
    // single-slot key would misanswer the FIRST request when a second
    // launch happens before the first result returns (round-115). Only
    // touched on the main thread.
    private val pendingPickFileLaunchKeys = kotlin.collections.ArrayDeque<Pair<Long, Long>>()

    // The key of the picker currently on screen, if any. Guards against a
    // stale ActivityResult callback from a previous activity instance
    // consuming a new request's key after a configuration change
    // (round-117). Main thread only.
    private var inFlightPickFileKey: Pair<Long, Long>? = null

    // A dialog currently shown by wireMcpRequestHandlers. The dialog event
    // was already consumed from the native queue; if the activity dies while
    // it is on screen (rotation, back press), the native MCP tool call would
    // hang forever — onDestroy replies with an empty result.
    private var pendingDialogRequest: Pair<Long, Long>? = null

    internal val terminalViewModel: terminal.emulator.TerminalViewModel by viewModels()

    private val selectAllReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(
                context: Context,
                intent: Intent,
            ) {
                val viewModel = terminalViewModel
                viewModel.selectAll()
                Log.d("T", "selectAll called via broadcast, active=${viewModel.state.value.selection.active}")
            }
        }

    private val partialSelectReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(
                context: Context,
                intent: Intent,
            ) {
                val viewModel = terminalViewModel
                // Clamp: the receiver is NOT_EXPORTED, but instrumentation
                // (same-uid) can still broadcast; a hostile broadcast could
                // otherwise carry Int.MAX and trigger a multi-billion-
                // iteration main-thread loop (ANR). A generous upper bound
                // is enough since terminal grids are small (tens of rows /
                // hundreds of cols).
                val startRow = intent.getIntExtra("startRow", 0).coerceIn(0, 4095)
                val startCol = intent.getIntExtra("startCol", 0).coerceIn(0, 4095)
                val endRow = intent.getIntExtra("endRow", 2).coerceIn(0, 4095)
                val endCol = intent.getIntExtra("endCol", 10).coerceIn(0, 4095)
                viewModel.startSelection(startRow, startCol)
                viewModel.updateSelection(endRow, endCol)
                viewModel.endSelection()
                Log.d("T", "partialSelect: ($startRow,$startCol)->($endRow,$endCol)")
            }
        }

    private val showPasteReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(
                context: Context,
                intent: Intent,
            ) {
                val viewModel = terminalViewModel
                // Clamp defensively: the receiver is registered with
                // RECEIVER_NOT_EXPORTED (round-98) so only in-process
                // broadcasts can reach it, but clamping stays as cheap
                // defense against a buggy sender.
                val row = intent.getIntExtra("row", 10).coerceIn(0, 4095)
                val col = intent.getIntExtra("col", 0).coerceIn(0, 4095)
                viewModel.showPastePopup(row, col)
                Log.d("T", "showPaste: row=$row col=$col")
            }
        }

    override fun onWindowStartingActionMode(
        callback: ActionMode.Callback,
        type: Int,
    ): ActionMode? = null

    override fun onWindowStartingActionMode(callback: ActionMode.Callback): ActionMode? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        previousNightMode =
            resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
        androidx.core.view.WindowCompat
            .setDecorFitsSystemWindows(window, false)
        window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
        window.setFormat(PixelFormat.TRANSPARENT)
        initFileLogging()
        registerReceiver(
            terminalDumpReceiver,
            IntentFilter("terminal.emulator.DUMP_TERMINAL"),
            Context.RECEIVER_NOT_EXPORTED,
        )
        registerReceiver(
            inputReceiver,
            IntentFilter("terminal.emulator.INPUT"),
            Context.RECEIVER_NOT_EXPORTED,
        )
        registerReceiver(
            selectAllReceiver,
            IntentFilter("terminal.emulator.SELECT_ALL"),
            // Test backdoors must not be triggerable by third-party apps;
            // same-process (instrumentation) broadcasters still reach a
            // NOT_EXPORTED receiver.
            Context.RECEIVER_NOT_EXPORTED,
        )
        registerReceiver(
            partialSelectReceiver,
            IntentFilter("terminal.emulator.PARTIAL_SELECT"),
            Context.RECEIVER_NOT_EXPORTED,
        )
        registerReceiver(
            showPasteReceiver,
            IntentFilter("terminal.emulator.SHOW_PASTE"),
            Context.RECEIVER_NOT_EXPORTED,
        )
        try {
            terminal.emulator.service.TerminalForegroundService
                .start(this)
        } catch (serviceException: Exception) {
            // Defensive (round-96): a ROM SecurityException or similar must
            // not crash onCreate. A live session created later starts the
            // service itself via the runtime's guarded path.
            LogUtil.e("MainActivity", "Failed to start foreground service in onCreate", serviceException)
        }
        // Note: TerminalForegroundService.start() (static) unconditionally
        // starts the service (which acquires a PARTIAL_WAKE_LOCK on start).
        // onDestroy routes through runtime.stopForegroundServiceIfIdle(),
        // which stops it unconditionally (no flag gate, round-95) when no
        // session is running, so the wake lock is not held forever after
        // the user leaves the app.
        // Android 13+ requires the POST_NOTIFICATIONS runtime permission;
        // without it every notify() throws SecurityException and session
        // notifications silently never appear. Ask once at startup; the
        // denial is non-fatal (notifications stay disabled).
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        wireMcpRequestHandlers()
        setContent {
            TerminalNavHost()
        }
    }

    /**
     * Wire MCP dialog / pick_file requests from the embedded MCP server to UI.
     * Responses are sent back through NativeBridge.dialogResult.
     */
    private fun wireMcpRequestHandlers() {
        runtime.dialogRequestHandler =
            { sessionId, requestId, dialogType, title, message, options ->
                runOnUiThread {
                    try {
                        val builder = android.app.AlertDialog.Builder(this)
                        builder.setTitle(title.ifEmpty { "Terminal" })
                        builder.setCancelable(false)
                        pendingDialogRequest = sessionId to requestId
                        val reply = { result: String ->
                            pendingDialogRequest = null
                            terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, result)
                        }
                        when (dialogType) {
                            "confirm" -> {
                                builder.setMessage(message)
                                builder.setPositiveButton(android.R.string.ok) { _, _ -> reply("true") }
                                builder.setNegativeButton(android.R.string.cancel) { _, _ -> reply("false") }
                            }

                            "input" -> {
                                builder.setMessage(message)
                                val input = android.widget.EditText(this)
                                builder.setView(input)
                                builder.setPositiveButton(android.R.string.ok) { _, _ -> reply(input.text.toString()) }
                                builder.setNegativeButton(android.R.string.cancel) { _, _ -> reply("") }
                            }

                            "select" -> {
                                builder.setItems(options.toTypedArray()) { _, which ->
                                    reply(options.getOrElse(which) { "" })
                                }
                                builder.setNegativeButton(android.R.string.cancel) { _, _ -> reply("") }
                            }

                            else -> {
                                builder.setMessage(message)
                                builder.setPositiveButton(android.R.string.ok) { _, _ -> reply("") }
                            }
                        }
                        builder.show()
                    } catch (exception: Exception) {
                        Log.e(TAG, "MCP dialog display failed", exception)
                        // The request event was already consumed; without a
                        // reply the native MCP tool call hangs forever.
                        pendingDialogRequest = null
                        terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                    }
                }
            }
        runtime.pickFileRequestHandler =
            { sessionId, requestId, startingPath, filter ->
                runOnUiThread {
                    try {
                        pendingPickFileRequests[Pair(sessionId, requestId)] = filter
                        pendingPickFileLaunchKeys.addLast(Pair(sessionId, requestId))
                        // Serialized launches: ActivityResult callbacks arrive
                        // in completion order, NOT launch order, so two
                        // concurrent pickers could cross their results. Only
                        // launch when the queue was empty (i.e. this is the
                        // only outstanding picker); the callback launches the
                        // next one after answering (round-116).
                        if (pendingPickFileLaunchKeys.size == 1) {
                            inFlightPickFileKey = Pair(sessionId, requestId)
                            pickFileLauncher.launch(mimeTypesForFilter(filter))
                        }
                    } catch (exception: Exception) {
                        pendingPickFileRequests.remove(Pair(sessionId, requestId))
                        // The queued key must not linger: a later callback
                        // would otherwise consume it and misanswer a live
                        // request (round-116).
                        pendingPickFileLaunchKeys.removeAll { it == Pair(sessionId, requestId) }
                        inFlightPickFileKey = null
                        Log.e(TAG, "MCP pick_file launch failed", exception)
                        terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                        // Keep the serialization invariant: the next queued
                        // request gets its picker (round-117).
                        launchNextQueuedPickFile()
                    }
                }
            }
    }

    /**
     * Convert an MCP file pattern to an Android MIME-type filter array.
     * Comma-separated patterns are split and each item mapped: items that
     * are already MIME types (`image/*`) pass through; glob patterns
     * (`*.txt`) cannot be mapped reliably to MIME types, so they fall back
     * to the unfiltered picker (`*/*`).
     */
    private fun mimeTypesForFilter(filter: String): Array<String> {
        val result = LinkedHashSet<String>()
        var anyGlob = false
        for (raw in filter.split(',')) {
            val trimmed = raw.trim()
            when {
                trimmed.isEmpty() || trimmed == "*" || trimmed == "*.*" -> anyGlob = true
                trimmed.contains('/') -> result.add(trimmed)
                else -> anyGlob = true
            }
        }
        // A single unfiltered picker is strictly more useful than a bogus
        // MIME string that would match nothing in DocumentsUI.
        if (result.isEmpty()) return arrayOf("*/*")
        // A glob among the patterns cannot be represented alongside MIME
        // types, so fall back to the unfiltered picker for the whole set.
        if (anyGlob) return arrayOf("*/*")
        return result.toTypedArray()
    }

    private val pickFileLauncher =
        registerForActivityResult(androidx.activity.result.contract.ActivityResultContracts.OpenDocument()) { uri ->
            // Launches are serialized (see pickFileRequestHandler): at most
            // one picker is up, so the queue head is the request this result
            // answers. The next queued request is launched after answering
            // (round-116).
            val key = pendingPickFileLaunchKeys.removeFirstOrNull()
            if (key == null) return@registerForActivityResult
            // A stale callback from a PREVIOUS activity instance (picker
            // still up across a configuration change) would otherwise
            // consume a NEW request's key and cross-deliver the old
            // selection (round-117).
            if (key != inFlightPickFileKey) {
                Log.w(TAG, "pick_file: stale ActivityResult callback ignored (key=$key, in-flight=$inFlightPickFileKey)")
                inFlightPickFileKey = null
                return@registerForActivityResult
            }
            inFlightPickFileKey = null
            pendingPickFileRequests.remove(key)
            val (sessionId, requestId) = key
            if (uri == null) {
                // User cancelled: answer empty, then relay the next queued
                // request — the serialization invariant requires a picker in
                // flight whenever the queue is non-empty (round-117).
                terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                launchNextQueuedPickFile()
                return@registerForActivityResult
            }
            // content:// URIs are passed as-is; agents can resolve them
            // through the content provider. A raw filesystem path is not
            // always available for SAF documents.
            var path = uri.toString()
            try {
                val cursor =
                    contentResolver.query(
                        uri,
                        arrayOf(android.provider.OpenableColumns.DISPLAY_NAME),
                        null,
                        null,
                        null,
                    )
                cursor?.use { c ->
                    if (c.moveToFirst()) {
                        val name = c.getString(0)
                        path = if (name != null) "$path ($name)" else path
                    }
                }
            } catch (exception: Exception) {
                Log.e(TAG, "pick_file: failed to resolve display name", exception)
            }
            terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, path)
            launchNextQueuedPickFile()
        }

    /**
     * Launch the next queued pick_file request, if any. Called after a
     * picker callback answers the current request, keeping launches
     * serialized (round-116).
     */
    private fun launchNextQueuedPickFile() {
        val next = pendingPickFileLaunchKeys.firstOrNull() ?: return
        val filter = pendingPickFileRequests[next]
        if (filter == null) {
            // Invariant "queue ⊆ map" broken (should be unreachable): drop
            // the dangling head so it cannot stall every later request
            // (round-117).
            Log.w(TAG, "pick_file: dangling queue head $next without map entry, dropping")
            pendingPickFileLaunchKeys.removeAll { it == next }
            launchNextQueuedPickFile()
            return
        }
        try {
            inFlightPickFileKey = next
            pickFileLauncher.launch(mimeTypesForFilter(filter))
        } catch (exception: Exception) {
            inFlightPickFileKey = null
            pendingPickFileRequests.remove(next)
            pendingPickFileLaunchKeys.removeAll { it == next }
            Log.e(TAG, "MCP pick_file launch failed", exception)
            terminal.emulator.bridge.NativeBridge.dialogResult(next.first, next.second, "")
            launchNextQueuedPickFile()
        }
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy (session persistence removed)")
        // Unbind MCP request handlers: TerminalRuntime is a process-wide
        // singleton and would otherwise keep a strong reference to this
        // destroyed Activity (leak), and dialog/pick_file requests would
        // hang forever once the events are consumed.
        runtime.dialogRequestHandler = null
        runtime.pickFileRequestHandler = null
        // pick_file requests in flight when the activity is destroyed
        // (recreation, back press) would otherwise never receive their
        // result: the launcher callback runs on a fresh instance whose
        // queue is empty, so the native MCP calls hang forever. Reply with
        // an empty result to unblock them all.
        // Answer every outstanding request (map is the authoritative set;
        // the queue only mirrors keys whose picker may still be up).
        // Empty replies so the native MCP call does not hang until the 300s
        // request timeout (round-115).
        pendingPickFileRequests.forEach { (key, _) ->
            try {
                terminal.emulator.bridge.NativeBridge.dialogResult(key.first, key.second, "")
            } catch (exception: Exception) {
                // One failure must not strand the remaining requests
                // (round-117); native is a no-op for unknown requests, so
                // this is purely defensive.
                Log.e(TAG, "pick_file: onDestroy reply failed", exception)
            }
        }
        pendingPickFileRequests.clear()
        pendingPickFileLaunchKeys.clear()
        inFlightPickFileKey = null
        // Same for a dialog that is on screen (event already consumed).
        pendingDialogRequest?.let { (sessionId, requestId) ->
            terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
        }
        pendingDialogRequest = null
        super.onDestroy()
        stopFileLogging()
        // Stop the foreground service when no session is running. Without
        // this, the service (and its PARTIAL_WAKE_LOCK) stays alive forever
        // after the user leaves the app, draining the battery and pinning a
        // permanent notification. With live sessions it must keep running.
        // The runtime re-checks under its session lock: a background session
        // created on the IO thread may have raced the (older) state snapshot.
        runtime.stopForegroundServiceIfIdle()
        tryUnregisterReceiver(terminalDumpReceiver, "terminalDumpReceiver")
        tryUnregisterReceiver(inputReceiver, "inputReceiver")
        tryUnregisterReceiver(selectAllReceiver, "selectAllReceiver")
        tryUnregisterReceiver(partialSelectReceiver, "partialSelectReceiver")
        tryUnregisterReceiver(showPasteReceiver, "showPasteReceiver")
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        val currentNightMode =
            newConfig.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
        if (currentNightMode != previousNightMode) {
            lifecycleScope.launch(kotlinx.coroutines.Dispatchers.IO) {
                runtime.applySettings()
            }
        }
        previousNightMode = currentNightMode
    }

    @SuppressLint("RestrictedApi")
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val handled = terminalViewModel.handleLayoutAwareHardwareKey(event)
        if (handled) {
            Log.d(TAG, "dispatchKeyEvent: consumed physical-key layout-aware char")
            return true
        }
        return super.dispatchKeyEvent(event)
    }

    @Deprecated("Use View.OnKeyListener pattern")
    override fun onKeyDown(
        keyCode: Int,
        event: KeyEvent?,
    ): Boolean = super.onKeyDown(keyCode, event)
}

@Composable
private fun TerminalNavHost(viewModelReady: (TerminalViewModel) -> Unit = {}) {
    val viewModel: TerminalViewModel = hiltViewModel()
    LaunchedEffect(viewModel) { viewModelReady(viewModel) }
    var showSettings by remember { mutableStateOf(false) }
    LaunchedEffect(showSettings) {
        viewModel.runtime.bridge()?.setRenderPaused(showSettings)
    }
    val appThemeMode by viewModel.appThemeMode.collectAsState()
    val isDarkTheme = androidx.compose.foundation.isSystemInDarkTheme()
    val context = LocalContext.current

    val forceDark =
        when (appThemeMode) {
            "night" -> true
            "day" -> false
            else -> isDarkTheme
        }

    val colorScheme =
        when {
            appThemeMode == "follow_system" && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
                if (isDarkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
            }

            forceDark -> {
                darkColorScheme()
            }

            else -> {
                lightColorScheme()
            }
        }

    Box(Modifier.semantics { testTagsAsResourceId = true }) {
        MaterialTheme(colorScheme = colorScheme) {
            TerminalScreen(
                viewModel = viewModel,
                onSettings = { showSettings = true },
                isOverlayVisible = showSettings,
            )
            if (showSettings) {
                SettingsScreen(
                    viewModel = viewModel,
                    onBack = { showSettings = false },
                )
            }
        }
    }
}
