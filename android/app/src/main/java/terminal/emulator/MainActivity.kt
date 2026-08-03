package terminal.emulator

import android.annotation.SuppressLint
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.ColorDrawable
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.LogcatDumpWriter
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.runtime.TestBackdoorReceivers
import terminal.emulator.ui.SettingsScreen
import terminal.emulator.ui.TerminalScreen
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveMaterialColorScheme
import java.io.File
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    companion object {
        private const val TAG = "MainActivity"
    }

    @Inject
    @Suppress("LateinitUsage") // Dagger injection
    lateinit var runtime: TerminalRuntime

    private val logcatDumpWriter = LogcatDumpWriter(this)

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

    // Round-210 P2-14: the visible AlertDialog instance, held so a native
    // MCP timeout (DialogCancel event) can dismiss it.
    private var activeDialog: android.app.AlertDialog? = null

    internal val terminalViewModel: terminal.emulator.TerminalViewModel by viewModels()

    private val testBackdoorReceivers =
        TestBackdoorReceivers(
            context = this,
            onDumpTerminal = { ctx ->
                Thread {
                    try {
                        val bridge = runtime.bridge()
                        val text =
                            if (bridge != null) {
                                bridge.getTerminalText() ?: "(empty)"
                            } else {
                                "(no active session)"
                            }
                        val file = java.io.File(ctx.cacheDir, "terminal_dump.txt")
                        file.writeText(text)
                        Log.d("T", "Terminal dump: ${file.absolutePath} (${text.length} chars)")
                    } catch (exception: Exception) {
                        Log.e("T", "Terminal dump failed", exception)
                    }
                }.apply {
                    isDaemon = true
                    start()
                }
            },
            onInput = { text ->
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
            },
            onSelectAll = {
                terminalViewModel.selectAll()
                Log.d("T", "selectAll called via broadcast, active=${terminalViewModel.state.value.selection.active}")
            },
            onPartialSelect = { startRow, startCol, endRow, endCol ->
                terminalViewModel.startSelection(startRow, startCol)
                terminalViewModel.updateSelection(endRow, endCol)
                terminalViewModel.endSelection()
                Log.d("T", "partialSelect: ($startRow,$startCol)->($endRow,$endCol)")
            },
            onShowPaste = { row, col ->
                terminalViewModel.showPastePopup(row, col)
                Log.d("T", "showPaste: row=$row col=$col")
            },
        )

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        previousNightMode =
            resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
        androidx.core.view.WindowCompat
            .setDecorFitsSystemWindows(window, false)
        window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
        window.setFormat(PixelFormat.TRANSPARENT)
        logcatDumpWriter.start()
        testBackdoorReceivers.register()
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
                        activeDialog = builder.show()
                    } catch (exception: Exception) {
                        Log.e(TAG, "MCP dialog display failed", exception)
                        // The request event was already consumed; without a
                        // reply the native MCP tool call hangs forever.
                        pendingDialogRequest = null
                        terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                    }
                }
            }
        runtime.dialogCancelHandler =
            { sessionId, requestId ->
                runOnUiThread {
                    // The native MCP tool call gave up (300s timeout);
                    // dismiss the dialog if it matches the pending request.
                    if (pendingDialogRequest == sessionId to requestId) {
                        pendingDialogRequest = null
                        activeDialog?.dismiss()
                        activeDialog = null
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
        activeDialog?.dismiss()
        activeDialog = null
        super.onDestroy()
        testBackdoorReceivers.unregister()
        logcatDumpWriter.stop()
        // Stop the foreground service when no session is running. Without
        // this, the service (and its PARTIAL_WAKE_LOCK) stays alive forever
        // after the user leaves the app, draining the battery and pinning a
        // permanent notification. With live sessions it must keep running.
        // The runtime re-checks under its session lock: a background session
        // created on the IO thread may have raced the (older) state snapshot.
        runtime.stopForegroundServiceIfIdle()
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
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val appThemeMode = settings.appThemeMode
    val isDarkTheme = androidx.compose.foundation.isSystemInDarkTheme()

    val forceDark = resolveAppDarkMode(appThemeMode, isDarkTheme)

    val colorScheme = resolveMaterialColorScheme(appThemeMode, forceDark, isDarkTheme)

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
