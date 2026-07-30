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
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.ui.SettingsScreen
import terminal.emulator.ui.TerminalScreen
import kotlinx.coroutines.launch
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
    }

    @Inject
    @Suppress("LateinitUsage") // Dagger injection
    lateinit var runtime: TerminalRuntime

    private var logFile: File? = null
    private var logWriter: BufferedWriter? = null
    private var logcatThread: Thread? = null
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
                    val reader = process.inputStream.bufferedReader()
                    for (line in reader.lineSequence()) {
                        @Suppress("ComplexCondition")
                        if (line.contains(
                                "T",
                            ) || line.contains("TerminalSurface") || line.contains("TerminalRuntime") ||
                            line.contains("AndroidRuntime")
                        ) {
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
        }, "FileLog").apply { isDaemon = true }
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
            logcatThread?.interrupt()
            logcatThread = null
            logWriter?.close()
            logWriter = null
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
                        Log.d("T", "Input received: '$text' (len=${text.length})")
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

    private var pendingPickFileRequest: Triple<Long, Long, String>? = null

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
                val startRow = intent.getIntExtra("startRow", 0)
                val startCol = intent.getIntExtra("startCol", 0)
                val endRow = intent.getIntExtra("endRow", 2)
                val endCol = intent.getIntExtra("endCol", 10)
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
                val row = intent.getIntExtra("row", 10)
                val col = intent.getIntExtra("col", 0)
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
            Context.RECEIVER_EXPORTED,
        )
        registerReceiver(
            partialSelectReceiver,
            IntentFilter("terminal.emulator.PARTIAL_SELECT"),
            Context.RECEIVER_EXPORTED,
        )
        registerReceiver(
            showPasteReceiver,
            IntentFilter("terminal.emulator.SHOW_PASTE"),
            Context.RECEIVER_EXPORTED,
        )
        terminal.emulator.service.TerminalForegroundService
            .start(this)
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
                        when (dialogType) {
                            "confirm" -> {
                                builder.setMessage(message)
                                builder.setPositiveButton(android.R.string.ok) { _, _ ->
                                    terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "true")
                                }
                                builder.setNegativeButton(android.R.string.cancel) { _, _ ->
                                    terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "false")
                                }
                            }
                            "input" -> {
                                builder.setMessage(message)
                                val input = android.widget.EditText(this)
                                builder.setView(input)
                                builder.setPositiveButton(android.R.string.ok) { _, _ ->
                                    terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, input.text.toString())
                                }
                                builder.setNegativeButton(android.R.string.cancel) { _, _ ->
                                    terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                                }
                            }
                            "select" -> {
                                builder.setItems(options.toTypedArray()) { _, which ->
                                    val chosen = options.getOrElse(which) { "" }
                                    terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, chosen)
                                }
                                builder.setNegativeButton(android.R.string.cancel) { _, _ ->
                                    terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                                }
                            }
                            else -> {
                                builder.setMessage(message)
                                builder.setPositiveButton(android.R.string.ok) { _, _ ->
                                    terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                                }
                            }
                        }
                        builder.show()
                    } catch (exception: Exception) {
                        Log.e(TAG, "MCP dialog display failed", exception)
                    }
                }
            }
        runtime.pickFileRequestHandler =
            { sessionId, requestId, startingPath, filter ->
                runOnUiThread {
                    try {
                        pendingPickFileRequest = Triple(sessionId, requestId, filter)
                        pickFileLauncher.launch(arrayOf("*/*"))
                    } catch (exception: Exception) {
                        Log.e(TAG, "MCP pick_file launch failed", exception)
                        terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
                    }
                }
            }
    }

    private val pickFileLauncher =
        registerForActivityResult(androidx.activity.result.contract.ActivityResultContracts.OpenDocument()) { uri ->
            val pending = pendingPickFileRequest
            pendingPickFileRequest = null
            if (pending == null) return@registerForActivityResult
            val (sessionId, requestId, _) = pending
            if (uri == null) {
                terminal.emulator.bridge.NativeBridge.dialogResult(sessionId, requestId, "")
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
        }

    override fun onDestroy() {
        lifecycleScope.launch {
            try {
                Log.d(TAG, "onDestroy (session persistence removed)")
            } catch (exception: Exception) {
                Log.e(TAG, "Cleanup during destroy failed", exception)
            }
        }
        super.onDestroy()
        stopFileLogging()
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
