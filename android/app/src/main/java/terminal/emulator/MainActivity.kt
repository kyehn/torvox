package terminal.emulator

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
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
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.launch
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.runtime.TestBackdoorReceivers
import terminal.emulator.ui.SettingsScreen
import terminal.emulator.ui.TerminalScreen
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveMaterialColorScheme
import java.io.File
import javax.inject.Inject

/** 冷启动后，设置覆盖层的暂停逻辑为等待 bridge 出现而放弃的时长（50ms × 50 = 2.5s）。 */
private const val BRIDGE_READY_POLL_INTERVAL_MS = 50L
private const val BRIDGE_READY_POLL_ATTEMPTS = 50

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    companion object {
        private const val TAG = "MainActivity"

        /**
         * 兼容 termux 的应急 extra：应用快捷方式「新建会话（应急）」与第三方
         * 启动器/任务器随 ACTION_RUN 发送 `com.termux.app.failsafe_session=true`。保留 termux
         * 原名（applicationId 即 com.termux），以便现有快捷方式与 tasker 任务继续工作。
         */
        const val EXTRA_FAILSAFE_SESSION = "com.termux.app.failsafe_session"

        /** 本应用专有 extra：在启动时打开设置页。 */
        const val EXTRA_OPEN_SETTINGS = "terminal.emulator.open_settings"

        /**
         * 仅测试用的 extra：从本地路径安装引导 zip，供 NixExecRealTerminalTest 使用
         * ——instrumentation 进程无法写应用 filesDir（SELinux app_data 类目）。
         * 对应 INSTALL_BOOTSTRAP 广播后门。
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
            onDumpTerminal = { dumpContext ->
                Thread {
                    try {
                        val bridge = runtime.bridge()
                        val text =
                            if (bridge != null) {
                                bridge.getTerminalText() ?: "(empty)"
                            } else {
                                "(no active session)"
                            }
                        val file = java.io.File(dumpContext.cacheDir, "terminal_dump.txt")
                        file.writeText(text)
                        LogUtil.d("T", "Terminal dump: ${file.absolutePath} (${text.length} chars)")
                    } catch (exception: Exception) {
                        LogUtil.e("T", "Terminal dump failed", exception)
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
                        LogUtil.d("T", "VT_WRITE received (len=${text.length})")
                        val processed = text.replace("\\x1b", "\u001b").replace("\\033", "\u001b")
                        terminalViewModel.feedTerminal(processed.toByteArray(Charsets.ISO_8859_1))
                    } catch (exception: Exception) {
                        LogUtil.e("T", "VT_WRITE failed", exception)
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
                        // 绝不记录输入内容：可能含密码/token，logcat 无差别记录。仅记长度。
                        LogUtil.d("T", "Input received (len=${text.length})")
                        val processed =
                            text
                                .replace("\\n", "\n")
                                .replace("\\r", "\r")
                                .replace("\\t", "\t")
                                .replace("\\x1b", "\u001b")
                                .replace("\\033", "\u001b")
                        // RAW 模式（rawInput）：逐字节原样写入、不追加换行，供测试注入
                        // 转义序列（OSC 8 链接、DECSET），避免被当作 shell 命令行解释。
                        val data =
                            (if (rawInput) processed else processed + "\n")
                                .byteInputStream()
                                .readBytes()
                        runtime.writeToPty(data)
                        LogUtil.d("T", "Input sent: ${data.size} bytes raw=$rawInput")
                    } catch (exception: Exception) {
                        LogUtil.e("T", "Input failed", exception)
                    }
                }
                    .apply {
                        isDaemon = true
                        start()
                    }
            },
            onSelectAll = {
                terminalViewModel.selectAll()
                LogUtil.d(
                    "T",
                    "selectAll called via broadcast, active=${terminalViewModel.state.value.selection.active}",
                )
            },
            onPartialSelect = { startRow, startCol, endRow, endCol ->
                terminalViewModel.startSelection(startRow, startCol)
                terminalViewModel.updateSelection(endRow, endCol)
                terminalViewModel.endSelection()
                LogUtil.d("T", "partialSelect: ($startRow,$startCol)->($endRow,$endCol)")
            },
            onShowPaste = { row, col ->
                terminalViewModel.showPastePopup(row, col)
                LogUtil.d("T", "showPaste: row=$row col=$col")
            },
            onInstallBootstrap = { installContext, zipPath ->
                installBootstrapFromPath(zipPath)
            },
        )

    override fun onCreate(savedInstanceState: Bundle?) {
        // 系统启动屏：冷启动首帧前显示主题背景，与 windowBackground 同色，
        // 首帧就绪后自动切回 Theme.Terminal。
        installSplashScreen()
        super.onCreate(savedInstanceState)
        // activity 1.14.0-alpha03 弃用 ComponentActivity.enableEdgeToEdge()，
        // 改用 core 的 WindowCompat.enableEdgeToEdge(window)。
        androidx.core.view.WindowCompat.enableEdgeToEdge(window)
        previousNightMode =
            resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
        // Color.TRANSPARENT 是 ARGB 整数而非资源 id —— lint 建议的 KTX
        // Int.toDrawable() 会把它当作资源 id (0) 并解析到错误的 drawable。
        @SuppressLint("UseKtx")
        window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
        window.setFormat(PixelFormat.TRANSPARENT)
        // 测试后门广播仅用于 instrumentation：非 debug 构建一律不注册
        // （release APK 不得暴露 DUMP_TERMINAL/INPUT/SELECT_ALL/VT_WRITE/
        // INSTALL_BOOTSTRAP 钩子）。
        if (BuildConfig.DEBUG) {
            testBackdoorReceivers.register()
        }
        terminal.emulator.service.TerminalForegroundService.start(this)
        // TerminalForegroundService.start() 静态方法无条件启动服务（启动即获取
        // PARTIAL_WAKE_LOCK）；onDestroy 经 runtime.stopForegroundServiceIfIdle()
        // 在无会话时无条件停止服务，不设标志门控，因此离开应用后不会长期持锁。
        // Android 13+ 需要 POST_NOTIFICATIONS 运行时权限，缺失时每次 notify() 都抛
        // SecurityException 且会话通知不再出现。启动时申请一次；拒绝不影响终端功能。
        if (
            checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        handleLaunchIntent(intent)
        setContent {
            TerminalNavHost(openSettingsOnLaunch = launchOpenSettings)
        }
    }

    /**
     * 应用快捷方式/第三方 intent 处理（兼容 termux）：`ACTION_RUN` +
     * EXTRA_FAILSAFE_SESSION 把下个会话切换到应急系统 shell；
     * EXTRA_OPEN_SETTINGS 打开设置面板。对应 termux-app TermuxActivity.java:401-425
     * （快捷方式 intent 在重建时会重新投递，故应急标志必须在 onNewIntent 中
     * 重新应用，而不只是 onCreate）。
     */
    private var launchOpenSettings = false

    private fun handleLaunchIntent(intent: Intent?) {
        if (intent == null) {
            LogUtil.d("MainActivity", "handleLaunchIntent: null intent")
            return
        }
        LogUtil.d(
            "MainActivity",
            "handleLaunchIntent: action=${intent.action} failsafe=${intent.getBooleanExtra(
                EXTRA_FAILSAFE_SESSION,
                false,
            )}",
        )
        if (
            Intent.ACTION_RUN == intent.action && intent.getBooleanExtra(EXTRA_FAILSAFE_SESSION, false)
        ) {
            runtime.requestFailsafeSession()
        }
        if (intent.getBooleanExtra(EXTRA_OPEN_SETTINGS, false)) {
            launchOpenSettings = true
        }
        // 仅测试用的引导安装入口（NixExecRealTerminalTest）。
        // 仅限 debug 构建：release APK 绝不能携带该安装后门 extra
        // （否则任何应用都能把我们指向任意 zip）。
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
     * 从本地路径安装引导程序 zip。委托给 [terminal.emulator.installer.BootstrapInstallService]，
     * 该服务运行在独立的 `:install` 进程：主进程为 instrumentation 目标时携带的是测试包的
     * SELinux 分类，无法写入 filesDir（已在模拟器验证）。安装状态一律从已安装的条目
     * （usr/bin/login）读回，不读标记文件。
     */
    private fun installBootstrapFromPath(zipPath: String) {
        LogUtil.d("MainActivity", "delegating bootstrap install to :install process")
        startService(
            Intent(this, terminal.emulator.installer.BootstrapInstallService::class.java)
                .putExtra(terminal.emulator.installer.BootstrapInstallService.EXTRA_ZIP_PATH, zipPath),
        )
    }

    override fun onDestroy() {
        LogUtil.d(TAG, "onDestroy")
        super.onDestroy()
        if (BuildConfig.DEBUG) {
            testBackdoorReceivers.unregister()
        }
        // 无会话时停止前台服务：否则用户离开应用后服务（及其 PARTIAL_WAKE_LOCK）
        // 永久存活，持续耗电并常驻通知。有活跃会话时必须继续运行。运行时在会话锁内
        // 重新核对：IO 线程上新建的后台会话可能已越过（较旧的）状态快照。
        runtime.stopForegroundServiceIfIdle()
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        val currentNightMode = newConfig.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
        if (currentNightMode != previousNightMode) {
            lifecycleScope.launch(terminal.emulator.util.TerminalDispatchers.inputOutput) {
                runtime.applySettings()
            }
        }
        previousNightMode = currentNightMode
    }

    @SuppressLint("RestrictedApi")
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val handled = terminalViewModel.handleLayoutAwareHardwareKey(event)
        if (handled) {
            LogUtil.d(TAG, "dispatchKeyEvent: consumed physical-key layout-aware char")
            return true
        }
        return super.dispatchKeyEvent(event)
    }
}

@Composable
private fun TerminalNavHost(
    openSettingsOnLaunch: Boolean = false,
    viewModel: TerminalViewModel = hiltViewModel(),
    viewModelReady: (TerminalViewModel) -> Unit = {},
) {
    LaunchedEffect(viewModel) { viewModelReady(viewModel) }
    var showSettings by remember { mutableStateOf(openSettingsOnLaunch) }
    LaunchedEffect(showSettings) {
        // 关闭设置时立即恢复渲染（此时 bridge 已存在）；
        // 只有「打开」转换才需要等待 bridge 出现的冷启轮询。
        if (!showSettings) {
            viewModel.runtime.bridge()?.setRenderPaused(false)
            return@LaunchedEffect
        }
        // bridge 由运行期在首个会话 spawn 之后异步创建；
        // 在此有界轮询可避免在带 `openSettingsOnLaunch` 的冷启动中
        // 静默丢失暂停状态。
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
                onSettings = { showSettings = true },
                isOverlayVisible = showSettings,
            )
            if (showSettings) {
                SettingsScreen(
                    onBack = { showSettings = false },
                )
            }
        }
    }
}
