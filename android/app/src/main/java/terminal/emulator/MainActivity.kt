package terminal.emulator

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.os.SystemClock
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
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
import terminal.emulator.ui.SettingsScreen
import terminal.emulator.ui.TerminalScreen
import terminal.emulator.ui.theme.resolveAppDarkMode
import terminal.emulator.ui.theme.resolveMaterialColorScheme
import javax.inject.Inject

/** 冷启动后，设置覆盖层的暂停逻辑为等待 bridge 出现而放弃的时长（50ms × 50 = 2.5s）。 */
private const val BRIDGE_READY_POLL_INTERVAL_MS = 50L
private const val BRIDGE_READY_POLL_ATTEMPTS = 50

/** 系统启动屏的最长驻留：终端首帧迟迟不来（引导未装、会话起不来）时不得永久遮挡界面。 */
private const val SPLASH_MAX_WAIT_MS = 5_000L

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

    /**
     * 通知权限请求：AndroidX 契约替代裸 `requestPermissions` + 魔数请求码。
     * 拒绝不影响终端功能（只是会话通知不再出现），故结果无需处理。
     */
    private val notificationPermissionLauncher =
        registerForActivityResult(
            androidx.activity.result.contract.ActivityResultContracts.RequestPermission(),
        ) { _ -> }

    private var previousNightMode: Int? = null
    private var previousFontScale: Float = 0f

    internal val terminalViewModel: terminal.emulator.TerminalViewModel by viewModels()
    override fun onCreate(savedInstanceState: Bundle?) {
        // 系统启动屏：冷启动首帧前显示主题背景，与 windowBackground 同色，
        // 首帧就绪后自动切回 Theme.Terminal。
        // 撤下时机不交给默认的「内容视图首次绘制」——那一刻原生渲染器还没提交过缓冲，
        // SurfaceView 的开孔区透出黑底（真机 logcat：首帧在进程启动后约 0.85s 才呈现，
        // 两者之间即「Shell 启动前偶发黑屏」）。改为等到终端首帧已呈现，见
        // [shouldKeepSplashOnScreen]。
        val splashScreen = installSplashScreen()
        val splashStartMs = SystemClock.uptimeMillis()
        splashScreen.setKeepOnScreenCondition {
            // 闭包在内容视图首次预绘制时才求值，故此处读 runtime 一定晚于 Hilt 注入。
            shouldKeepSplashOnScreen(
                framePresented = runtime.hasPresentedFrame,
                waitedMs = SystemClock.uptimeMillis() - splashStartMs,
            )
        }
        super.onCreate(savedInstanceState)
        // activity 1.14.0-alpha03 弃用 ComponentActivity.enableEdgeToEdge()，
        // 改用 core 的 WindowCompat.enableEdgeToEdge(window)。
        androidx.core.view.WindowCompat.enableEdgeToEdge(window)
        previousNightMode =
            resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
        previousFontScale = resources.configuration.fontScale
        // Color.TRANSPARENT 是 ARGB 整数而非资源 id —— lint 建议的 KTX
        // Int.toDrawable() 会把它当作资源 id (0) 并解析到错误的 drawable。
        @SuppressLint("UseKtx")
        window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
        window.setFormat(PixelFormat.TRANSPARENT)
        terminal.emulator.service.TerminalForegroundService.start(this)
        // TerminalForegroundService.start() 无条件启动服务，但唤醒锁只在
        // sessionCount>=1 时获取（冷启动的裸 start() 此时计数为 0）；
        // onDestroy 经 runtime.stopForegroundServiceIfIdle() 在无会话时无条件
        // 停止服务，因此离开应用后不会长期持锁。
        // Android 13+ 需要 POST_NOTIFICATIONS 运行时权限，缺失时每次 notify() 都抛
        // SecurityException 且会话通知不再出现。启动时申请一次；拒绝不影响终端功能。
        if (
            checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            notificationPermissionLauncher.launch(android.Manifest.permission.POST_NOTIFICATIONS)
        }
        handleLaunchIntent(intent)
        // 最后一个会话结束后关闭应用（termux 语义：会话表空了就 finish，见 termux-app
        // TermuxTerminalSessionActivityClient#removeFinishedSession）。此前会话被移除后
        // 应用停在无会话的空界面，用户看到的是「exit 后什么都没关」。
        // 生命周期作用域内收集：Activity 销毁即取消，且 finish() 本就在主线程调用。
        lifecycleScope.launch {
            val sessions = SessionTableWatcher()
            runtime.state.collect { state ->
                if (sessions.onSessionCount(state.sessionIds.size)) {
                    LogUtil.d(TAG, "last session closed; finishing activity")
                    finish()
                }
            }
        }
        setContent {
            TerminalNavHost(openSettingsRequests = openSettingsRequests)
        }
    }

    /**
     * 应用快捷方式/第三方 intent 处理（兼容 termux）：`ACTION_RUN` +
     * EXTRA_FAILSAFE_SESSION 把下个会话切换到应急系统 shell；
     * EXTRA_OPEN_SETTINGS 打开设置面板。对应 termux-app TermuxActivity.java:401-425
     * （快捷方式 intent 在重建时会重新投递，故应急标志必须在 onNewIntent 中
     * 重新应用，而不只是 onCreate）。
     *
     * 打开设置请求计数：每次带 `EXTRA_OPEN_SETTINGS` 的 intent 到达即递增。
     * 用计数而不用布尔值——设置关闭后计数保持，再次到达仍能触发重组；
     * 布尔值在首次置 true 后不再变化，二次 intent 无法生效。
     */
    private var openSettingsRequests by mutableIntStateOf(0)

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
            openSettingsRequests++
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
        // 无会话时停止前台服务：否则用户离开应用后服务（及其 PARTIAL_WAKE_LOCK）
        // 永久存活，持续耗电并常驻通知。有活跃会话时必须继续运行。运行时在会话锁内
        // 重新核对：IO 线程上新建的后台会话可能已越过（较旧的）状态快照。
        runtime.stopForegroundServiceIfIdle()
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        val currentNightMode = newConfig.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK
        // 系统「字体大小」变化必须一并响应：它改变 sp→px 系数，而原生
        // setRasterScale 只在建会话与本回调里下发一次。系数漂移后 Kotlin 按新系数
        // 算字号与网格、原生仍按旧值光栅，字形被拉伸、度量与网格整体偏掉。
        // 故 manifest 声明了 fontScale（否则 Activity 重建，本回调根本不会到达，
        // 而 TerminalRuntime 作为 @Singleton 仍会活下来）。
        val currentFontScale = newConfig.fontScale
        if (currentNightMode != previousNightMode || currentFontScale != previousFontScale) {
            lifecycleScope.launch(terminal.emulator.util.TerminalDispatchers.inputOutput) {
                runtime.applySettings()
            }
        }
        previousNightMode = currentNightMode
        previousFontScale = currentFontScale
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
private fun TerminalNavHost(openSettingsRequests: Int = 0, viewModel: TerminalViewModel = hiltViewModel()) {
    var showSettings by remember { mutableStateOf(openSettingsRequests > 0) }
    LaunchedEffect(openSettingsRequests) {
        if (openSettingsRequests > 0) showSettings = true
    }
    // 设置页的渲染暂停走运行期的共享账本而不是直接写全局布尔：关设置页时
    // 无条件 setRenderPaused(false) 会把切后台那条暂停一并顶掉，在已被系统回收的
    // BufferQueue 上出帧即永久黑屏。账本保证只有全部来源都归还后才恢复。
    var settingsPauseToken by remember { mutableStateOf<Long?>(null) }
    LaunchedEffect(showSettings) {
        if (!showSettings) return@LaunchedEffect
        // bridge 由运行期在首个会话 spawn 之后异步创建；原生暂停标志落在全局渲染器
        // 上，渲染器尚未建立时写入即丢失，故打开设置页前先等它出现（有界轮询，
        // 只在带 `openSettingsOnLaunch` 的冷启动才真正等待）。
        var attempts = 0
        while (viewModel.runtime.bridge() == null && attempts < BRIDGE_READY_POLL_ATTEMPTS) {
            kotlinx.coroutines.delay(BRIDGE_READY_POLL_INTERVAL_MS)
            attempts++
        }
        settingsPauseToken = viewModel.runtime.acquireRenderPause()
    }
    DisposableEffect(showSettings) {
        onDispose {
            // 打开设置时也走这里（本次持有为空），那一次 MUST NOT 出帧：刚打开就
            // 出一帧，等于在「设置页遮住终端」的第一帧上白白跑一次 GPU。
            val token = settingsPauseToken
            settingsPauseToken = null
            if (token == null) return@onDispose
            viewModel.runtime.releaseRenderPause(token)
            // 关设置页要的是「立刻出一帧」：账本归还不是无条件恢复，Surface 已就绪时
            // 这里补一次，与 TerminalSurface.surfaceCreated 同源。
            viewModel.runtime.forceRender()
        }
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

/**
 * 系统启动屏是否继续遮挡。
 *
 * 终端首帧已呈现就立刻撤下；首帧迟迟不来（引导未装、会话起不来、原生库缺失）时以
 * [SPLASH_MAX_WAIT_MS] 兜底，把界面（及其错误提示）还给用户，而不是让启动屏永久占屏。
 *
 * 纯函数、无 Android 依赖，便于单测；判据只有「首帧」与「等了多久」两项，故不引入状态机。
 */
internal fun shouldKeepSplashOnScreen(framePresented: Boolean, waitedMs: Long): Boolean =
    !framePresented && waitedMs < SPLASH_MAX_WAIT_MS

/**
 * 会话表清空 → 关闭应用的判据。
 *
 * 冷启动时 [TerminalRuntime.state] 先以空会话表发布（会话由运行期异步创建），直接判空
 * 会在启动瞬间关掉应用，故必须记住「曾有过会话」；关闭只触发一次，使 finish 之后
 * 残留的状态发射不再重复调用 finish。纯 Kotlin、无 Android 依赖，便于单测。
 */
internal class SessionTableWatcher {
    private var sawSession = false
    private var fired = false

    /** 返回是否应关闭应用。 */
    fun onSessionCount(count: Int): Boolean {
        if (count > 0) {
            sawSession = true
            return false
        }
        if (!sawSession || fired) return false
        fired = true
        return true
    }
}
