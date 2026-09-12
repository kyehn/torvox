package terminal.emulator

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.ColorDrawable
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
import java.io.File
import javax.inject.Inject
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.runtime.TestBackdoorReceivers
import terminal.emulator.ui.SettingsScreen
import terminal.emulator.ui.TerminalScreen
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveMaterialColorScheme

/**
 * How long the settings-overlay pause waits for the bridge to appear after a cold start before
 * giving up (50ms × 50 = 2.5s).
 */
private const val BRIDGE_READY_POLL_INTERVAL_MS = 50L
private const val BRIDGE_READY_POLL_ATTEMPTS = 50

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
  companion object {
    private const val TAG = "MainActivity"

    /**
     * termux-compatible failsafe extra: app shortcut "New session (Failsafe)" and third-party
     * launchers/taskers send `com.termux.app.failsafe_session=true` with ACTION_RUN. 保留 termux
     * 原名（applicationId 即 com.termux），以便现有快捷方式与 tasker 任务继续工作。
     */
    const val EXTRA_FAILSAFE_SESSION = "com.termux.app.failsafe_session"

    /** 本应用专有 extra：在启动时打开设置页。 */
    const val EXTRA_OPEN_SETTINGS = "terminal.emulator.open_settings"

    /**
     * Test-only extra: install a bootstrap zip from a local path used by
     * NixBootstrapInstrumentedTest — the instrumentation process cannot write the app filesDir,
     * SELinux app_data category). Mirrors the INSTALL_BOOTSTRAP broadcast backdoor.
     */
    const val EXTRA_INSTALL_BOOTSTRAP = "terminal.emulator.install_bootstrap"
  }

  @Inject
  @Suppress("LateinitUsage") // Dagger injection
  lateinit var runtime: TerminalRuntime

  private var previousNightMode: Int? = null

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
                }
                .apply {
                  isDaemon = true
                  start()
                }
          },
          onVtWrite = { text ->
            Thread {
                  try {
                    Log.d("T", "VT_WRITE received (len=${text.length})")
                    val processed = text.replace("\\x1b", "\u001b").replace("\\033", "\u001b")
                    terminalViewModel.feedTerminal(processed.toByteArray(Charsets.ISO_8859_1))
                  } catch (exception: Exception) {
                    Log.e("T", "VT_WRITE failed", exception)
                  }
                }
                .apply {
                  isDaemon = true
                  start()
                }
          },
          onInput = { text, rawInput ->
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
                            .replace("\\x1b", "\u001b")
                            .replace("\\033", "\u001b")
                    // RAW mode (rawInput): write the bytes verbatim
                    // without appending '\n' — used by tests to inject
                    // escape sequences (OSC 8 links, DECSET) that must
                    // not be interpreted as a shell command line.
                    val data =
                        (if (rawInput) processed else processed + "\n")
                            .byteInputStream()
                            .readBytes()
                    runtime.writeToPty(data)
                    Log.d("T", "Input sent: ${data.size} bytes raw=$rawInput")
                  } catch (exception: Exception) {
                    Log.e("T", "Input failed", exception)
                  }
                }
                .apply {
                  isDaemon = true
                  start()
                }
          },
          onSelectAll = {
            terminalViewModel.selectAll()
            Log.d(
                "T",
                "selectAll called via broadcast, active=${terminalViewModel.state.value.selection.active}",
            )
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
          onInstallBootstrap = { ctx, zipPath ->
            installBootstrapFromPath(zipPath)
          },
      )

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    enableEdgeToEdge()
    previousNightMode =
        resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
    androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, false)
    // Color.TRANSPARENT is an ARGB int, not a resource id — the KTX
    // Int.toDrawable() the lint suggests would treat it as a res id (0)
    // and resolve the wrong drawable.
    @SuppressLint("UseKtx") window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
    window.setFormat(PixelFormat.TRANSPARENT)
    // Test-backdoor broadcasts are instrumentation-only: never register
    // them outside debug builds (a release APK must not expose the
    // DUMP_TERMINAL/INPUT/SELECT_ALL/VT_WRITE/INSTALL_BOOTSTRAP hooks).
    if (BuildConfig.DEBUG) {
      testBackdoorReceivers.register()
    }
    try {
      terminal.emulator.service.TerminalForegroundService.start(this)
    } catch (serviceException: Exception) {
      // Defensive: a ROM SecurityException or similar must
      // not crash onCreate. A live session created later starts the
      // service itself via the runtime's guarded path.
      LogUtil.e("MainActivity", "Failed to start foreground service in onCreate", serviceException)
    }
    // Note: TerminalForegroundService.start() (static) unconditionally
    // starts the service (which acquires a PARTIAL_WAKE_LOCK on start).
    // onDestroy routes through runtime.stopForegroundServiceIfIdle(),
    // which stops it unconditionally (no flag gate) when no
    // session is running, so the wake lock is not held forever after
    // the user leaves the app.
    // Android 13+ requires the POST_NOTIFICATIONS runtime permission;
    // without it every notify() throws SecurityException and session
    // notifications silently never appear. Ask once at startup; the
    // denial is non-fatal (notifications stay disabled).
    if (
        checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
    ) {
      requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 1)
    }
    checkShizukuAuthorization()
    handleLaunchIntent(intent)
    setContent {
      TerminalNavHost(openSettingsOnLaunch = launchOpenSettings)
    }
  }

  /**
   * DESIGN Shizuku startup gate: with the switch on but no actual grant, show a dialog offering to
   * turn the switch off or exit the app. Runs before the first session starts (terminal start
   * observes settings).
   */
  private fun checkShizukuAuthorization() {
    lifecycleScope.launch {
      val enabled =
          try {
            terminalViewModel.settings.first().shizukuEnabled
          } catch (exception: Exception) {
            LogUtil.w("MainActivity", "Shizuku gate settings read failed", exception)
            return@launch
          }
      if (!enabled || ShizukuGate.isAuthorized()) return@launch
      android.app.AlertDialog.Builder(this@MainActivity)
          .setTitle(R.string.shizuku_required_title)
          .setMessage(R.string.shizuku_required_message)
          .setCancelable(false)
          .setPositiveButton(R.string.shizuku_turn_off) { dialog, _ ->
            terminalViewModel.setShizukuEnabled(false)
            dialog.dismiss()
          }
          .setNegativeButton(R.string.shizuku_exit) { dialog, _ ->
            dialog.dismiss()
            finishAndRemoveTask()
          }
          .show()
    }
  }

  /**
   * App-shortcut / third-party intent handling (termux-compatible): `ACTION_RUN` +
   * EXTRA_FAILSAFE_SESSION switches the next session to the failsafe system shell;
   * EXTRA_OPEN_SETTINGS opens the settings sheet. Mirrors termux-app TermuxActivity.java:401-425
   * (shortcut intents are re-delivered on recreation, so the failsafe flag must be re-applied in
   * onNewIntent, not just onCreate).
   */
  private var launchOpenSettings = false

  private fun handleLaunchIntent(intent: Intent?) {
    if (intent == null) {
      LogUtil.d("MainActivity", "handleLaunchIntent: null intent")
      return
    }
    LogUtil.d(
        "MainActivity",
        "handleLaunchIntent: action=${intent.action} failsafe=${intent.getBooleanExtra(EXTRA_FAILSAFE_SESSION, false)}",
    )
    if (
        Intent.ACTION_RUN == intent.action && intent.getBooleanExtra(EXTRA_FAILSAFE_SESSION, false)
    ) {
      runtime.requestFailsafeSession()
    }
    if (intent.getBooleanExtra(EXTRA_OPEN_SETTINGS, false)) {
      launchOpenSettings = true
    }
    // Test-only bootstrap installer entry (NixBootstrapInstrumentedTest).
    // Debug builds only: release APKs must not carry the install-backdoor
    // extra (any app could otherwise point us at an arbitrary zip).
    if (BuildConfig.DEBUG) {
      intent.getStringExtra(EXTRA_INSTALL_BOOTSTRAP)?.let { zipPath ->
        installBootstrapFromPath(zipPath)
      }
    }
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    handleLaunchIntent(intent)
  }

  /**
   * Install a bootstrap zip from a local path. Delegates to
   * [terminal.emulator.installer.BootstrapInstallService], which runs in its own `:install`
   * process: when the main process is an instrumentation target it carries the TEST package's
   * SELinux category and cannot write filesDir, emulator-verified). Install state is read back from
   * the installed entry (usr/bin/login), never from a marker file.
   */
  private fun installBootstrapFromPath(zipPath: String) {
    LogUtil.d("MainActivity", "delegating bootstrap install to :install process")
    startService(
        Intent(this, terminal.emulator.installer.BootstrapInstallService::class.java)
            .putExtra(terminal.emulator.installer.BootstrapInstallService.EXTRA_ZIP_PATH, zipPath),
    )
  }

  override fun onDestroy() {
    Log.d(TAG, "onDestroy (session persistence removed)")
    super.onDestroy()
    if (BuildConfig.DEBUG) {
      testBackdoorReceivers.unregister()
    }
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
    val currentNightMode = newConfig.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
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
private fun TerminalNavHost(
    openSettingsOnLaunch: Boolean = false,
    viewModelReady: (TerminalViewModel) -> Unit = {},
) {
  val viewModel: TerminalViewModel = hiltViewModel()
  LaunchedEffect(viewModel) { viewModelReady(viewModel) }
  var showSettings by remember { mutableStateOf(openSettingsOnLaunch) }
  LaunchedEffect(showSettings) {
    // Closing settings resumes rendering immediately (the bridge exists
    // by then); only the "open" transition needs the cold-start poll
    // that waits for the bridge to appear.
    if (!showSettings) {
      viewModel.runtime.bridge()?.setRenderPaused(false)
      return@LaunchedEffect
    }
    // The bridge is created asynchronously by the runtime after the
    // first session spawns; polling for it here (bounded) prevents the
    // pause state from being silently dropped on a cold start with
    // `openSettingsOnLaunch`.
    var attempts = 0
    while (viewModel.runtime.bridge() == null && attempts < BRIDGE_READY_POLL_ATTEMPTS) {
      kotlinx.coroutines.delay(BRIDGE_READY_POLL_INTERVAL_MS)
      attempts++
    }
    viewModel.runtime.bridge()?.setRenderPaused(true)
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
